#!/usr/bin/env python3
"""
qr_service — Python SDK for HNR QR Service

Zero-dependency client library for the QR Service API.
Works with Python 3.8+ using only the standard library.

Quick start:
    from qr_service import QRService

    qr = QRService("http://localhost:3001")

    # Generate a QR code
    result = qr.generate("https://example.com")
    print(result["image_base64"][:50])  # data:image/png;base64,...
    print(result["share_url"])

    # Generate SVG with custom colors
    result = qr.generate("Hello", format="svg", fg_color="#FF0000", style="dots")

    # Batch generate
    results = qr.batch([
        {"data": "https://a.com"},
        {"data": "https://b.com", "format": "svg"},
    ])

    # Templates
    wifi = qr.wifi("MyNetwork", "secret123")
    vcard = qr.vcard("Alice", email="alice@example.com", phone="+1234567890")
    url = qr.url("https://example.com", utm_source="campaign")

    # Tracked QR with analytics
    tracked = qr.create_tracked("https://example.com")
    token = tracked["manage_token"]
    stats = qr.tracked_stats(tracked["id"], token)
    qr.delete_tracked(tracked["id"], token)

    # Decode a QR code from image bytes
    with open("qr.png", "rb") as f:
        decoded = qr.decode(f.read())
    print(decoded["data"])

Full docs: GET /api/v1/llms.txt or /.well-known/skills/qr-service/SKILL.md
"""

from __future__ import annotations

import base64
import json
import os
import urllib.error
import urllib.parse
import urllib.request
from typing import (
    Any,
    Dict,
    List,
    Optional,
    Union,
)


__version__ = "1.0.0"


# ---------------------------------------------------------------------------
# Exceptions
# ---------------------------------------------------------------------------


class QRServiceError(Exception):
    """Base exception for QR Service API errors."""

    def __init__(self, message: str, status_code: int = 0, body: Any = None):
        super().__init__(message)
        self.status_code = status_code
        self.body = body


class NotFoundError(QRServiceError):
    """Resource not found (404)."""
    pass


class ValidationError(QRServiceError):
    """Invalid request parameters (400/422)."""
    pass


class RateLimitError(QRServiceError):
    """Rate limit exceeded (429)."""

    def __init__(
        self,
        message: str,
        status_code: int = 429,
        body: Any = None,
        retry_after_secs: Optional[int] = None,
        limit: Optional[int] = None,
        remaining: Optional[int] = None,
    ):
        super().__init__(message, status_code, body)
        self.retry_after_secs = retry_after_secs
        self.limit = limit
        self.remaining = remaining


class GoneError(QRServiceError):
    """Resource expired (410)."""
    pass


class ServerError(QRServiceError):
    """Internal server error (5xx)."""
    pass


# ---------------------------------------------------------------------------
# Client
# ---------------------------------------------------------------------------


class QRService:
    """Client for the HNR QR Service API.

    Args:
        base_url: Service URL (default: ``$QR_SERVICE_URL`` or ``http://localhost:3001``).
        timeout: HTTP timeout in seconds (default 30).
    """

    def __init__(
        self,
        base_url: Optional[str] = None,
        *,
        timeout: int = 30,
    ):
        self.base_url = (
            base_url or os.environ.get("QR_SERVICE_URL") or "http://localhost:3001"
        ).rstrip("/")
        self.timeout = timeout

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------

    def _request(
        self,
        method: str,
        path: str,
        *,
        json_body: Any = None,
        raw_body: Optional[bytes] = None,
        headers: Optional[Dict[str, str]] = None,
        query: Optional[Dict[str, str]] = None,
    ) -> Any:
        url = f"{self.base_url}{path}"
        if query:
            filtered = {k: v for k, v in query.items() if v is not None}
            if filtered:
                url += "?" + urllib.parse.urlencode(filtered)

        hdrs = dict(headers or {})
        body: Optional[bytes] = None

        if json_body is not None:
            body = json.dumps(json_body).encode()
            hdrs.setdefault("Content-Type", "application/json")
        elif raw_body is not None:
            body = raw_body

        req = urllib.request.Request(url, data=body, headers=hdrs, method=method)

        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                ct = resp.headers.get("Content-Type", "")
                raw = resp.read()
                if "json" in ct:
                    return json.loads(raw)
                return raw
        except urllib.error.HTTPError as exc:
            self._raise_for_status(exc)

    def _raise_for_status(self, exc: urllib.error.HTTPError) -> None:
        status = exc.code
        try:
            body = json.loads(exc.read())
        except Exception:
            body = None

        msg = ""
        if isinstance(body, dict):
            msg = body.get("error", "") or body.get("message", "")
        if not msg:
            msg = f"HTTP {status}"

        if status == 404:
            raise NotFoundError(msg, status, body)
        if status == 410:
            raise GoneError(msg, status, body)
        if status == 429:
            retry = body.get("retry_after_secs") if isinstance(body, dict) else None
            limit = body.get("limit") if isinstance(body, dict) else None
            remaining = body.get("remaining") if isinstance(body, dict) else None
            raise RateLimitError(
                msg, status, body,
                retry_after_secs=retry, limit=limit, remaining=remaining,
            )
        if status in (400, 422):
            raise ValidationError(msg, status, body)
        if status >= 500:
            raise ServerError(msg, status, body)
        raise QRServiceError(msg, status, body)

    # ------------------------------------------------------------------
    # Health
    # ------------------------------------------------------------------

    def health(self) -> Dict[str, Any]:
        """``GET /api/v1/health`` — service health check."""
        return self._request("GET", "/api/v1/health")

    def is_healthy(self) -> bool:
        """Return ``True`` if the service is reachable and healthy."""
        try:
            h = self.health()
            return h.get("status") == "ok"
        except Exception:
            return False

    # ------------------------------------------------------------------
    # Generate
    # ------------------------------------------------------------------

    def generate(
        self,
        data: str,
        *,
        format: str = "png",
        size: int = 256,
        fg_color: str = "#000000",
        bg_color: str = "#FFFFFF",
        error_correction: str = "M",
        style: str = "square",
        logo: Optional[str] = None,
        logo_size: Optional[int] = None,
    ) -> Dict[str, Any]:
        """``POST /api/v1/qr/generate`` — generate a single QR code.

        Args:
            data: Content to encode (URL, text, etc.).
            format: Output format — ``"png"``, ``"svg"``, or ``"pdf"``.
            size: Image size in pixels (64–4096, default 256).
            fg_color: Foreground hex color (default ``"#000000"``).
            bg_color: Background hex color (default ``"#FFFFFF"``).
            error_correction: EC level — ``"L"``, ``"M"``, ``"Q"``, ``"H"`` (default ``"M"``).
                Auto-upgraded to ``"H"`` when logo is present.
            style: Module style — ``"square"``, ``"rounded"``, ``"dots"`` (default ``"square"``).
            logo: Optional logo as base64 string or ``data:`` URI (max 512 KB).
            logo_size: Logo size as percentage of QR dimensions (5–40, default 20).

        Returns:
            Dict with ``image_base64``, ``share_url``, ``format``, ``size``, ``data``.
        """
        body: Dict[str, Any] = {
            "data": data,
            "format": format,
            "size": size,
            "fg_color": fg_color,
            "bg_color": bg_color,
            "error_correction": error_correction,
            "style": style,
        }
        if logo is not None:
            body["logo"] = logo
        if logo_size is not None:
            body["logo_size"] = logo_size
        return self._request("POST", "/api/v1/qr/generate", json_body=body)

    def generate_png(self, data: str, **kwargs: Any) -> Dict[str, Any]:
        """Convenience: generate PNG QR code."""
        return self.generate(data, format="png", **kwargs)

    def generate_svg(self, data: str, **kwargs: Any) -> Dict[str, Any]:
        """Convenience: generate SVG QR code."""
        return self.generate(data, format="svg", **kwargs)

    def generate_pdf(self, data: str, **kwargs: Any) -> Dict[str, Any]:
        """Convenience: generate PDF QR code."""
        return self.generate(data, format="pdf", **kwargs)

    # ------------------------------------------------------------------
    # Decode
    # ------------------------------------------------------------------

    def decode(self, image_bytes: bytes) -> Dict[str, Any]:
        """``POST /api/v1/qr/decode`` — decode a QR code from image bytes.

        Args:
            image_bytes: Raw PNG/JPEG/etc. bytes of the image to decode.

        Returns:
            Dict with ``data`` (decoded content) and ``format``.
        """
        return self._request("POST", "/api/v1/qr/decode", raw_body=image_bytes)

    # ------------------------------------------------------------------
    # Batch
    # ------------------------------------------------------------------

    def batch(
        self,
        items: List[Dict[str, Any]],
    ) -> Dict[str, Any]:
        """``POST /api/v1/qr/batch`` — generate multiple QR codes (max 50).

        Args:
            items: List of generation request dicts. Each requires at least ``"data"``.
                Optional per-item: ``format``, ``size``, ``fg_color``, ``bg_color``,
                ``error_correction``, ``style``, ``logo``, ``logo_size``.

        Returns:
            Dict with ``items`` (list of QR responses) and ``total``.
        """
        return self._request("POST", "/api/v1/qr/batch", json_body={"items": items})

    # ------------------------------------------------------------------
    # Templates
    # ------------------------------------------------------------------

    def wifi(
        self,
        ssid: str,
        password: str = "",
        *,
        encryption: str = "WPA2",
        hidden: bool = False,
        format: str = "png",
        size: int = 256,
        style: str = "square",
    ) -> Dict[str, Any]:
        """``POST /api/v1/qr/template/wifi`` — WiFi network QR code.

        Args:
            ssid: Network name.
            password: Network password (empty for open networks).
            encryption: Encryption type (``"WPA2"``, ``"WPA"``, ``"WEP"``, ``"nopass"``).
            hidden: Whether the network is hidden.
            format: Output format.
            size: Image size.
            style: Module style.

        Returns:
            QR response dict.
        """
        body: Dict[str, Any] = {
            "ssid": ssid,
            "password": password,
            "encryption": encryption,
            "hidden": hidden,
            "format": format,
            "size": size,
            "style": style,
        }
        return self._request("POST", "/api/v1/qr/template/wifi", json_body=body)

    def vcard(
        self,
        name: str,
        *,
        email: Optional[str] = None,
        phone: Optional[str] = None,
        org: Optional[str] = None,
        title: Optional[str] = None,
        url: Optional[str] = None,
        format: str = "png",
        size: int = 256,
        style: str = "square",
    ) -> Dict[str, Any]:
        """``POST /api/v1/qr/template/vcard`` — vCard contact QR code.

        Args:
            name: Full name (required).
            email: Email address.
            phone: Phone number.
            org: Organization.
            title: Job title.
            url: Website URL.
            format: Output format.
            size: Image size.
            style: Module style.

        Returns:
            QR response dict.
        """
        body: Dict[str, Any] = {"name": name, "format": format, "size": size, "style": style}
        if email is not None:
            body["email"] = email
        if phone is not None:
            body["phone"] = phone
        if org is not None:
            body["org"] = org
        if title is not None:
            body["title"] = title
        if url is not None:
            body["url"] = url
        return self._request("POST", "/api/v1/qr/template/vcard", json_body=body)

    def url(
        self,
        target_url: str,
        *,
        utm_source: Optional[str] = None,
        utm_medium: Optional[str] = None,
        utm_campaign: Optional[str] = None,
        format: str = "png",
        size: int = 256,
        style: str = "square",
    ) -> Dict[str, Any]:
        """``POST /api/v1/qr/template/url`` — URL QR code with optional UTM params.

        Args:
            target_url: URL to encode.
            utm_source: UTM source parameter.
            utm_medium: UTM medium parameter.
            utm_campaign: UTM campaign parameter.
            format: Output format.
            size: Image size.
            style: Module style.

        Returns:
            QR response dict.
        """
        body: Dict[str, Any] = {"url": target_url, "format": format, "size": size, "style": style}
        if utm_source is not None:
            body["utm_source"] = utm_source
        if utm_medium is not None:
            body["utm_medium"] = utm_medium
        if utm_campaign is not None:
            body["utm_campaign"] = utm_campaign
        return self._request("POST", "/api/v1/qr/template/url", json_body=body)

    # ------------------------------------------------------------------
    # Tracked QR Codes
    # ------------------------------------------------------------------

    def create_tracked(
        self,
        target_url: str,
        *,
        format: str = "png",
        size: int = 256,
        fg_color: str = "#000000",
        bg_color: str = "#FFFFFF",
        error_correction: str = "M",
        style: str = "square",
        short_code: Optional[str] = None,
        expires_at: Optional[str] = None,
    ) -> Dict[str, Any]:
        """``POST /api/v1/qr/tracked`` — create a tracked QR code with analytics.

        The returned ``manage_token`` is needed for stats and deletion — save it!

        Args:
            target_url: Destination URL for the short redirect.
            format: QR image format.
            size: QR image size.
            fg_color: Foreground color.
            bg_color: Background color.
            error_correction: EC level.
            style: Module style.
            short_code: Custom short code (auto-generated if omitted).
            expires_at: Optional expiry as ISO-8601 string.

        Returns:
            Dict with ``id``, ``manage_token``, ``short_url``, ``short_code``,
            ``target_url``, ``scan_count``, ``qr`` (nested QR response), etc.
        """
        body: Dict[str, Any] = {
            "target_url": target_url,
            "format": format,
            "size": size,
            "fg_color": fg_color,
            "bg_color": bg_color,
            "error_correction": error_correction,
            "style": style,
        }
        if short_code is not None:
            body["short_code"] = short_code
        if expires_at is not None:
            body["expires_at"] = expires_at
        return self._request("POST", "/api/v1/qr/tracked", json_body=body)

    def tracked_stats(self, tracked_id: str, manage_token: str) -> Dict[str, Any]:
        """``GET /api/v1/qr/tracked/{id}/stats`` — get scan analytics.

        Args:
            tracked_id: Tracked QR ID.
            manage_token: Token returned on creation.

        Returns:
            Dict with ``id``, ``short_code``, ``target_url``, ``scan_count``,
            ``recent_scans`` (list), etc.
        """
        return self._request(
            "GET",
            f"/api/v1/qr/tracked/{tracked_id}/stats",
            headers={"Authorization": f"Bearer {manage_token}"},
        )

    def delete_tracked(self, tracked_id: str, manage_token: str) -> Dict[str, Any]:
        """``DELETE /api/v1/qr/tracked/{id}`` — delete a tracked QR code.

        Args:
            tracked_id: Tracked QR ID.
            manage_token: Token returned on creation.

        Returns:
            Dict with ``deleted: true`` and ``id``.
        """
        return self._request(
            "DELETE",
            f"/api/v1/qr/tracked/{tracked_id}",
            headers={"Authorization": f"Bearer {manage_token}"},
        )

    # ------------------------------------------------------------------
    # Discovery
    # ------------------------------------------------------------------

    def view(
        self,
        data: str,
        *,
        size: Optional[int] = None,
        fg: Optional[str] = None,
        bg: Optional[str] = None,
        format: Optional[str] = None,
        style: Optional[str] = None,
    ) -> bytes:
        """``GET /api/v1/qr/view`` — stateless share URL (returns HTML page or image).

        Args:
            data: Content to encode.
            size: Image size.
            fg: Foreground hex color.
            bg: Background hex color.
            format: Output format.
            style: Module style.

        Returns:
            Raw response bytes (HTML page).
        """
        q: Dict[str, str] = {"data": data}
        if size is not None:
            q["size"] = str(size)
        if fg is not None:
            q["fg"] = fg
        if bg is not None:
            q["bg"] = bg
        if format is not None:
            q["format"] = format
        if style is not None:
            q["style"] = style
        return self._request("GET", "/qr/view", query=q)

    # ------------------------------------------------------------------
    # Discovery
    # ------------------------------------------------------------------

    def llms_txt(self) -> str:
        """``GET /api/v1/llms.txt`` — AI-readable service documentation."""
        data = self._request("GET", "/api/v1/llms.txt")
        return data.decode() if isinstance(data, bytes) else str(data)

    def llms_txt_root(self) -> str:
        """``GET /llms.txt`` — root-level AI-readable service documentation."""
        data = self._request("GET", "/llms.txt")
        return data.decode() if isinstance(data, bytes) else str(data)

    def openapi(self) -> Dict[str, Any]:
        """``GET /api/v1/openapi.json`` — OpenAPI 3.0 specification."""
        return self._request("GET", "/api/v1/openapi.json")

    def skills(self) -> Dict[str, Any]:
        """``GET /.well-known/skills/index.json`` — Cloudflare RFC skill discovery."""
        return self._request("GET", "/.well-known/skills/index.json")

    def skill_md(self) -> str:
        """``GET /.well-known/skills/qr-service/SKILL.md`` — agent integration guide."""
        data = self._request("GET", "/.well-known/skills/qr-service/SKILL.md")
        return data.decode() if isinstance(data, bytes) else str(data)

    def skill_md_v1(self) -> str:
        """``GET /api/v1/skills/SKILL.md`` — alternate agent integration guide path."""
        data = self._request("GET", "/api/v1/skills/SKILL.md")
        return data.decode() if isinstance(data, bytes) else str(data)

    # ------------------------------------------------------------------
    # Convenience helpers
    # ------------------------------------------------------------------

    def save_qr(self, result: Dict[str, Any], filepath: str) -> None:
        """Save a QR code result to a file.

        Decodes the ``image_base64`` field and writes raw bytes to disk.

        Args:
            result: Response from ``generate()``, ``wifi()``, etc.
            filepath: Destination file path.
        """
        b64 = result["image_base64"]
        # Strip data URI prefix if present
        if "," in b64:
            b64 = b64.split(",", 1)[1]
        raw = base64.b64decode(b64)
        with open(filepath, "wb") as f:
            f.write(raw)

    def image_bytes(self, result: Dict[str, Any]) -> bytes:
        """Extract raw image bytes from a QR code result.

        Args:
            result: Response from ``generate()``, ``wifi()``, etc.

        Returns:
            Raw image bytes (PNG, SVG, or PDF).
        """
        b64 = result["image_base64"]
        if "," in b64:
            b64 = b64.split(",", 1)[1]
        return base64.b64decode(b64)

    def __repr__(self) -> str:
        return f"QRService(base_url={self.base_url!r})"
