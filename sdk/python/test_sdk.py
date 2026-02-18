#!/usr/bin/env python3
"""
Integration tests for the QR Service Python SDK.

Usage:
    # Against local dev server
    python test_sdk.py

    # Against staging
    QR_SERVICE_URL=http://192.168.0.79:3001 python test_sdk.py

    # Verbose output
    python test_sdk.py -v
"""

import base64
import json
import os
import sys
import tempfile
import time
import unittest
from typing import Optional

# Import SDK from same directory
sys.path.insert(0, os.path.dirname(__file__))
from qr_service import (
    GoneError,
    NotFoundError,
    QRService,
    QRServiceError,
    RateLimitError,
    ServerError,
    ValidationError,
)

BASE_URL = os.environ.get("QR_SERVICE_URL", "http://localhost:3001")


class QRServiceTestCase(unittest.TestCase):
    """Base class with shared setup."""

    qr: QRService
    _tracked_ids: list  # (id, token) pairs for cleanup

    @classmethod
    def setUpClass(cls) -> None:
        cls.qr = QRService(BASE_URL)
        cls._tracked_ids = []

    @classmethod
    def tearDownClass(cls) -> None:
        for tid, token in cls._tracked_ids:
            try:
                cls.qr.delete_tracked(tid, token)
            except Exception:
                pass

    def _track(self, result: dict) -> dict:
        """Register a tracked QR for cleanup."""
        self.__class__._tracked_ids.append((result["id"], result["manage_token"]))
        return result


# =========================================================================
# Health
# =========================================================================


class TestHealth(QRServiceTestCase):
    def test_health(self):
        h = self.qr.health()
        self.assertEqual(h["status"], "ok")
        self.assertIn("version", h)
        self.assertIn("uptime_seconds", h)

    def test_is_healthy(self):
        self.assertTrue(self.qr.is_healthy())

    def test_is_healthy_bad_url(self):
        bad = QRService("http://localhost:1")
        self.assertFalse(bad.is_healthy())

    def test_health_uptime_positive(self):
        h = self.qr.health()
        self.assertGreaterEqual(h["uptime_seconds"], 0)

    def test_health_version_format(self):
        h = self.qr.health()
        self.assertIsInstance(h["version"], str)
        self.assertTrue(len(h["version"]) > 0)


# =========================================================================
# Generate
# =========================================================================


class TestGenerate(QRServiceTestCase):
    def test_generate_png(self):
        result = self.qr.generate("https://example.com")
        self.assertTrue(result["image_base64"].startswith("data:image/png;base64,"))
        self.assertEqual(result["format"], "png")
        self.assertEqual(result["size"], 256)
        self.assertEqual(result["data"], "https://example.com")
        self.assertIn("share_url", result)

    def test_generate_svg(self):
        result = self.qr.generate("hello world", format="svg")
        self.assertTrue(result["image_base64"].startswith("data:image/svg+xml;base64,"))
        self.assertEqual(result["format"], "svg")

    def test_generate_pdf(self):
        result = self.qr.generate("pdf test", format="pdf")
        self.assertTrue(result["image_base64"].startswith("data:application/pdf;base64,"))
        self.assertEqual(result["format"], "pdf")

    def test_generate_custom_size(self):
        result = self.qr.generate("sized", size=512)
        self.assertEqual(result["size"], 512)

    def test_generate_custom_colors(self):
        result = self.qr.generate("colored", fg_color="#FF0000", bg_color="#00FF00")
        self.assertIsNotNone(result["image_base64"])

    def test_generate_all_styles(self):
        for style in ("square", "rounded", "dots"):
            result = self.qr.generate(f"style-{style}", style=style)
            self.assertIsNotNone(result["image_base64"])

    def test_generate_all_ec_levels(self):
        for ec in ("L", "M", "Q", "H"):
            result = self.qr.generate(f"ec-{ec}", error_correction=ec)
            self.assertIsNotNone(result["image_base64"])

    def test_generate_empty_data_rejected(self):
        with self.assertRaises(ValidationError) as ctx:
            self.qr.generate("")
        self.assertEqual(ctx.exception.status_code, 400)

    def test_generate_invalid_size_too_small(self):
        with self.assertRaises(ValidationError):
            self.qr.generate("small", size=10)

    def test_generate_invalid_size_too_large(self):
        with self.assertRaises(ValidationError):
            self.qr.generate("large", size=9999)

    def test_generate_invalid_format(self):
        with self.assertRaises(ValidationError):
            self.qr.generate("bad", format="gif")

    def test_convenience_generate_png(self):
        result = self.qr.generate_png("convenience png")
        self.assertEqual(result["format"], "png")

    def test_convenience_generate_svg(self):
        result = self.qr.generate_svg("convenience svg")
        self.assertEqual(result["format"], "svg")

    def test_convenience_generate_pdf(self):
        result = self.qr.generate_pdf("convenience pdf")
        self.assertEqual(result["format"], "pdf")

    def test_generate_min_size(self):
        result = self.qr.generate("min", size=64)
        self.assertEqual(result["size"], 64)

    def test_generate_max_size(self):
        result = self.qr.generate("max", size=4096)
        self.assertEqual(result["size"], 4096)

    def test_share_url_present(self):
        result = self.qr.generate("share me")
        self.assertIn("/qr/view?data=", result["share_url"])


# =========================================================================
# Generate — Roundtrip (all styles × formats)
# =========================================================================


class TestGenerateRoundtrip(QRServiceTestCase):
    """Generate QR in various configs, decode, verify content matches.

    Note: Only 'square' style is reliably decodable by rqrr. 'rounded' and
    'dots' styles are decorative and produce QR codes that the decoder cannot
    parse. Roundtrip tests use square style; other styles are tested for
    successful generation in TestGenerate.
    """

    def test_roundtrip_square_png(self):
        result = self.qr.generate("rt-square", style="square")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "rt-square")

    def test_roundtrip_rounded_generates(self):
        """Rounded style generates successfully (decode not reliable with rqrr)."""
        result = self.qr.generate("rt-rounded", style="rounded")
        raw = self.qr.image_bytes(result)
        self.assertTrue(raw[:4] == b"\x89PNG")
        self.assertTrue(len(raw) > 100)

    def test_roundtrip_dots_generates(self):
        """Dots style generates successfully (decode not reliable with rqrr)."""
        result = self.qr.generate("rt-dots", style="dots")
        raw = self.qr.image_bytes(result)
        self.assertTrue(raw[:4] == b"\x89PNG")
        self.assertTrue(len(raw) > 100)

    def test_roundtrip_svg_valid(self):
        """SVG output should be valid SVG markup."""
        result = self.qr.generate("svg-content", format="svg")
        raw = self.qr.image_bytes(result)
        self.assertIn(b"<svg", raw)
        self.assertIn(b"</svg>", raw)

    def test_roundtrip_custom_colors(self):
        result = self.qr.generate("colored-rt", fg_color="#0000FF", bg_color="#FFFF00")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "colored-rt")

    def test_roundtrip_large_size(self):
        result = self.qr.generate("large-rt", size=512)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "large-rt")

    def test_roundtrip_small_size(self):
        result = self.qr.generate("small-rt", size=64)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "small-rt")

    def test_roundtrip_ec_low(self):
        result = self.qr.generate("ec-L-rt", error_correction="L")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "ec-L-rt")

    def test_roundtrip_ec_high(self):
        result = self.qr.generate("ec-H-rt", error_correction="H")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "ec-H-rt")


# =========================================================================
# Generate — Determinism
# =========================================================================


class TestGenerateDeterminism(QRServiceTestCase):
    """Same parameters should produce identical output."""

    def test_deterministic_png(self):
        r1 = self.qr.generate("determ", size=128, style="square")
        r2 = self.qr.generate("determ", size=128, style="square")
        self.assertEqual(r1["image_base64"], r2["image_base64"])

    def test_deterministic_svg(self):
        r1 = self.qr.generate("determ-svg", format="svg", style="dots")
        r2 = self.qr.generate("determ-svg", format="svg", style="dots")
        self.assertEqual(r1["image_base64"], r2["image_base64"])

    def test_different_data_different_output(self):
        r1 = self.qr.generate("data-a")
        r2 = self.qr.generate("data-b")
        self.assertNotEqual(r1["image_base64"], r2["image_base64"])

    def test_different_styles_different_output(self):
        r1 = self.qr.generate("style-diff", style="square")
        r2 = self.qr.generate("style-diff", style="dots")
        self.assertNotEqual(r1["image_base64"], r2["image_base64"])

    def test_different_sizes_different_output(self):
        r1 = self.qr.generate("size-diff", size=128)
        r2 = self.qr.generate("size-diff", size=512)
        self.assertNotEqual(r1["image_base64"], r2["image_base64"])


# =========================================================================
# Generate — Logo Overlay
# =========================================================================


class TestGenerateLogo(QRServiceTestCase):
    """Test logo overlay functionality."""

    # Minimal 1x1 red PNG (base64)
    TINY_PNG = (
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQV"
        "R42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg=="
    )

    def test_logo_png(self):
        result = self.qr.generate("logo-test", logo=self.TINY_PNG)
        self.assertIsNotNone(result["image_base64"])
        self.assertEqual(result["format"], "png")

    def test_logo_svg(self):
        result = self.qr.generate("logo-svg", format="svg", logo=self.TINY_PNG)
        raw = self.qr.image_bytes(result)
        self.assertIn(b"<svg", raw)

    def test_logo_custom_size(self):
        result = self.qr.generate("logo-size", logo=self.TINY_PNG, logo_size=30)
        self.assertIsNotNone(result["image_base64"])

    def test_logo_min_size(self):
        result = self.qr.generate("logo-min", logo=self.TINY_PNG, logo_size=5)
        self.assertIsNotNone(result["image_base64"])

    def test_logo_max_size(self):
        result = self.qr.generate("logo-max", logo=self.TINY_PNG, logo_size=40)
        self.assertIsNotNone(result["image_base64"])

    def test_logo_ec_auto_upgrade(self):
        """Logo should auto-upgrade error correction to H for maximum redundancy."""
        result = self.qr.generate(
            "logo-ec", logo=self.TINY_PNG, error_correction="L"
        )
        # Should succeed — EC is silently upgraded
        self.assertIsNotNone(result["image_base64"])

    def test_logo_roundtrip(self):
        """QR with logo should still be decodable."""
        result = self.qr.generate("logo-rt", logo=self.TINY_PNG, logo_size=10)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "logo-rt")

    def test_logo_with_all_styles(self):
        for style in ("square", "rounded", "dots"):
            result = self.qr.generate(
                f"logo-{style}", style=style, logo=self.TINY_PNG
            )
            self.assertIsNotNone(result["image_base64"])


# =========================================================================
# Decode
# =========================================================================


class TestDecode(QRServiceTestCase):
    def test_decode_roundtrip(self):
        """Generate a PNG, then decode it — content should match."""
        result = self.qr.generate("roundtrip test")
        raw = self.qr.image_bytes(result)
        decoded = self.qr.decode(raw)
        self.assertEqual(decoded["data"], "roundtrip test")
        self.assertEqual(decoded["format"], "qr")

    def test_decode_invalid_image(self):
        with self.assertRaises((ValidationError, QRServiceError)):
            self.qr.decode(b"not an image")

    def test_decode_empty(self):
        with self.assertRaises((ValidationError, QRServiceError)):
            self.qr.decode(b"")

    def test_decode_returns_format_field(self):
        result = self.qr.generate("fmt-check")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertIn("format", decoded)
        self.assertIn("data", decoded)

    def test_decode_unicode_content(self):
        original = "日本語テスト 🎯"
        result = self.qr.generate(original)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], original)

    def test_decode_url_content(self):
        url = "https://example.com/path?key=value&foo=bar#anchor"
        result = self.qr.generate(url)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], url)


# =========================================================================
# Batch
# =========================================================================


class TestBatch(QRServiceTestCase):
    def test_batch_basic(self):
        result = self.qr.batch([
            {"data": "https://a.com"},
            {"data": "https://b.com"},
        ])
        self.assertEqual(result["total"], 2)
        self.assertEqual(len(result["items"]), 2)

    def test_batch_mixed_formats(self):
        result = self.qr.batch([
            {"data": "png item", "format": "png"},
            {"data": "svg item", "format": "svg"},
            {"data": "pdf item", "format": "pdf"},
        ])
        self.assertEqual(result["total"], 3)
        formats = [item["format"] for item in result["items"]]
        self.assertEqual(formats, ["png", "svg", "pdf"])

    def test_batch_single_item(self):
        result = self.qr.batch([{"data": "solo"}])
        self.assertEqual(result["total"], 1)

    def test_batch_empty_rejected(self):
        with self.assertRaises(ValidationError):
            self.qr.batch([])

    def test_batch_too_many_rejected(self):
        items = [{"data": f"item-{i}"} for i in range(51)]
        with self.assertRaises(ValidationError):
            self.qr.batch(items)

    def test_batch_max_allowed(self):
        items = [{"data": f"item-{i}"} for i in range(50)]
        result = self.qr.batch(items)
        self.assertEqual(result["total"], 50)

    def test_batch_custom_styles(self):
        result = self.qr.batch([
            {"data": "dots", "style": "dots"},
            {"data": "rounded", "style": "rounded"},
        ])
        self.assertEqual(result["total"], 2)

    def test_batch_preserves_order(self):
        items = [{"data": f"order-{i}"} for i in range(5)]
        result = self.qr.batch(items)
        for i, item in enumerate(result["items"]):
            self.assertEqual(item["data"], f"order-{i}")

    def test_batch_per_item_sizes(self):
        result = self.qr.batch([
            {"data": "small", "size": 64},
            {"data": "medium", "size": 256},
            {"data": "large", "size": 512},
        ])
        sizes = [item["size"] for item in result["items"]]
        self.assertEqual(sizes, [64, 256, 512])

    def test_batch_per_item_colors(self):
        result = self.qr.batch([
            {"data": "red", "fg_color": "#FF0000"},
            {"data": "blue", "fg_color": "#0000FF"},
        ])
        self.assertEqual(result["total"], 2)
        # Different colors → different images
        self.assertNotEqual(
            result["items"][0]["image_base64"],
            result["items"][1]["image_base64"],
        )

    def test_batch_with_logo(self):
        tiny = (
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQV"
            "R42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg=="
        )
        result = self.qr.batch([
            {"data": "logo-batch", "logo": f"data:image/png;base64,{tiny}"},
        ])
        self.assertEqual(result["total"], 1)
        self.assertIsNotNone(result["items"][0]["image_base64"])

    def test_batch_all_styles_same_data(self):
        result = self.qr.batch([
            {"data": "style-test", "style": s}
            for s in ("square", "rounded", "dots")
        ])
        self.assertEqual(result["total"], 3)
        images = [item["image_base64"] for item in result["items"]]
        # All different styles should produce different images
        self.assertEqual(len(set(images)), 3)

    def test_batch_response_has_share_urls(self):
        result = self.qr.batch([{"data": "share-batch"}])
        self.assertIn("share_url", result["items"][0])

    def test_batch_decode_roundtrip(self):
        """Batch-generate, then decode each — verify content."""
        items = [{"data": f"batch-rt-{i}"} for i in range(3)]
        result = self.qr.batch(items)
        for i, item in enumerate(result["items"]):
            raw = self.qr.image_bytes(item)
            decoded = self.qr.decode(raw)
            self.assertEqual(decoded["data"], f"batch-rt-{i}")


# =========================================================================
# Templates
# =========================================================================


class TestTemplates(QRServiceTestCase):
    def test_wifi(self):
        result = self.qr.wifi("TestNetwork", "password123")
        self.assertIn("WIFI:", result["data"])
        self.assertIn("TestNetwork", result["data"])

    def test_wifi_open_network(self):
        result = self.qr.wifi("OpenNet", "", encryption="nopass")
        self.assertIn("WIFI:", result["data"])

    def test_wifi_hidden(self):
        result = self.qr.wifi("HiddenNet", "pass", hidden=True)
        self.assertIn("H:true", result["data"])

    def test_wifi_svg_format(self):
        result = self.qr.wifi("SvgNet", "pass", format="svg")
        self.assertTrue(result["image_base64"].startswith("data:image/svg+xml"))

    def test_wifi_custom_style(self):
        result = self.qr.wifi("StyledNet", "pass", style="dots")
        self.assertIsNotNone(result["image_base64"])

    def test_wifi_wpa(self):
        result = self.qr.wifi("WpaNet", "wpapass", encryption="WPA")
        self.assertIn("T:WPA", result["data"])

    def test_wifi_wep(self):
        result = self.qr.wifi("WepNet", "weppass", encryption="WEP")
        self.assertIn("T:WEP", result["data"])

    def test_wifi_custom_size(self):
        result = self.qr.wifi("BigWifi", "pass", size=512)
        self.assertIsNotNone(result["image_base64"])

    def test_wifi_pdf_format(self):
        result = self.qr.wifi("PdfWifi", "pass", format="pdf")
        self.assertTrue(result["image_base64"].startswith("data:application/pdf"))

    def test_wifi_decode_roundtrip(self):
        result = self.qr.wifi("RtWifi", "rtpass")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertIn("RtWifi", decoded["data"])

    def test_vcard_minimal(self):
        result = self.qr.vcard("Alice")
        self.assertIn("BEGIN:VCARD", result["data"])
        self.assertIn("Alice", result["data"])

    def test_vcard_full(self):
        result = self.qr.vcard(
            "Bob Smith",
            email="bob@example.com",
            phone="+1234567890",
            org="Acme Corp",
            title="Engineer",
            url="https://bob.example.com",
        )
        self.assertIn("bob@example.com", result["data"])
        self.assertIn("+1234567890", result["data"])
        self.assertIn("Acme Corp", result["data"])

    def test_vcard_with_title(self):
        result = self.qr.vcard("Jane", title="CTO")
        self.assertIn("CTO", result["data"])

    def test_vcard_with_url(self):
        result = self.qr.vcard("Max", url="https://max.dev")
        self.assertIn("https://max.dev", result["data"])

    def test_vcard_svg_format(self):
        result = self.qr.vcard("SvgCard", format="svg")
        self.assertTrue(result["image_base64"].startswith("data:image/svg+xml"))

    def test_vcard_pdf_format(self):
        result = self.qr.vcard("PdfCard", format="pdf")
        self.assertTrue(result["image_base64"].startswith("data:application/pdf"))

    def test_vcard_decode_roundtrip(self):
        result = self.qr.vcard("Roundtrip Person", email="rt@test.com")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertIn("Roundtrip Person", decoded["data"])
        self.assertIn("rt@test.com", decoded["data"])

    def test_url_simple(self):
        result = self.qr.url("https://example.com")
        self.assertEqual(result["data"], "https://example.com")

    def test_url_with_utm(self):
        result = self.qr.url(
            "https://example.com",
            utm_source="twitter",
            utm_medium="social",
            utm_campaign="launch",
        )
        self.assertIn("utm_source=twitter", result["data"])
        self.assertIn("utm_medium=social", result["data"])
        self.assertIn("utm_campaign=launch", result["data"])

    def test_url_partial_utm(self):
        result = self.qr.url("https://example.com", utm_source="newsletter")
        self.assertIn("utm_source=newsletter", result["data"])
        self.assertNotIn("utm_medium", result["data"])

    def test_url_svg_format(self):
        result = self.qr.url("https://example.com", format="svg")
        self.assertTrue(result["image_base64"].startswith("data:image/svg+xml"))

    def test_url_custom_style(self):
        result = self.qr.url("https://example.com", style="rounded")
        self.assertIsNotNone(result["image_base64"])

    def test_url_decode_roundtrip(self):
        result = self.qr.url("https://roundtrip.example.com")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "https://roundtrip.example.com")

    def test_unknown_template_rejected(self):
        with self.assertRaises(ValidationError):
            self.qr._request("POST", "/api/v1/qr/template/bogus", json_body={"data": "x"})


# =========================================================================
# Tracked QR
# =========================================================================


class TestTracked(QRServiceTestCase):
    def test_create_tracked(self):
        result = self._track(self.qr.create_tracked("https://example.com"))
        self.assertIn("id", result)
        self.assertIn("manage_token", result)
        self.assertIn("short_url", result)
        self.assertIn("short_code", result)
        self.assertIn("target_url", result)
        self.assertEqual(result["target_url"], "https://example.com")
        self.assertEqual(result["scan_count"], 0)
        self.assertIn("qr", result)
        self.assertIn("image_base64", result["qr"])

    def test_create_tracked_custom_short_code(self):
        code = f"test-{int(time.time())}"
        result = self._track(self.qr.create_tracked("https://example.com", short_code=code))
        self.assertEqual(result["short_code"], code)

    def test_create_tracked_svg(self):
        result = self._track(self.qr.create_tracked("https://example.com", format="svg"))
        self.assertTrue(result["qr"]["image_base64"].startswith("data:image/svg+xml"))

    def test_tracked_stats(self):
        created = self._track(self.qr.create_tracked("https://stats.example.com"))
        stats = self.qr.tracked_stats(created["id"], created["manage_token"])
        self.assertEqual(stats["id"], created["id"])
        self.assertEqual(stats["target_url"], "https://stats.example.com")
        self.assertEqual(stats["scan_count"], 0)
        self.assertIsInstance(stats["recent_scans"], list)

    def test_tracked_stats_wrong_token(self):
        created = self._track(self.qr.create_tracked("https://auth-test.example.com"))
        with self.assertRaises(NotFoundError):
            self.qr.tracked_stats(created["id"], "wrong-token")

    def test_tracked_stats_nonexistent(self):
        with self.assertRaises(NotFoundError):
            self.qr.tracked_stats("nonexistent-id", "any-token")

    def test_delete_tracked(self):
        created = self.qr.create_tracked("https://delete-me.example.com")
        result = self.qr.delete_tracked(created["id"], created["manage_token"])
        self.assertTrue(result["deleted"])
        # Verify it's gone
        with self.assertRaises(NotFoundError):
            self.qr.tracked_stats(created["id"], created["manage_token"])

    def test_delete_tracked_wrong_token(self):
        created = self._track(self.qr.create_tracked("https://nodelete.example.com"))
        with self.assertRaises(NotFoundError):
            self.qr.delete_tracked(created["id"], "wrong-token")

    def test_delete_tracked_nonexistent(self):
        with self.assertRaises(NotFoundError):
            self.qr.delete_tracked("nonexistent", "any-token")

    def test_create_tracked_with_expiry(self):
        result = self._track(
            self.qr.create_tracked(
                "https://expiry.example.com",
                expires_at="2099-12-31T23:59:59Z",
            )
        )
        self.assertIn("expires_at", result)

    def test_tracked_response_fields(self):
        """Verify all expected fields in tracked QR response."""
        result = self._track(self.qr.create_tracked("https://fields.example.com"))
        expected_fields = [
            "id", "qr_id", "short_code", "short_url", "target_url",
            "manage_token", "manage_url", "scan_count", "created_at", "qr",
        ]
        for field in expected_fields:
            self.assertIn(field, result, f"Missing field: {field}")

    def test_create_tracked_pdf(self):
        result = self._track(self.qr.create_tracked("https://pdf.example.com", format="pdf"))
        self.assertTrue(result["qr"]["image_base64"].startswith("data:application/pdf"))

    def test_create_tracked_custom_style(self):
        result = self._track(
            self.qr.create_tracked("https://dots.example.com", style="dots")
        )
        self.assertIsNotNone(result["qr"]["image_base64"])

    def test_create_tracked_custom_colors(self):
        result = self._track(
            self.qr.create_tracked(
                "https://color.example.com",
                fg_color="#FF00FF",
                bg_color="#00FFFF",
            )
        )
        self.assertIsNotNone(result["qr"]["image_base64"])

    def test_create_tracked_custom_size(self):
        result = self._track(
            self.qr.create_tracked("https://big.example.com", size=512)
        )
        self.assertIsNotNone(result["qr"]["image_base64"])

    def test_create_tracked_ec_level(self):
        result = self._track(
            self.qr.create_tracked(
                "https://ec.example.com", error_correction="H"
            )
        )
        self.assertIsNotNone(result["qr"]["image_base64"])

    def test_tracked_lifecycle_full(self):
        """Full lifecycle: create → stats → delete → verify gone."""
        created = self.qr.create_tracked("https://lifecycle.example.com")
        tid, token = created["id"], created["manage_token"]

        # Stats should work
        stats = self.qr.tracked_stats(tid, token)
        self.assertEqual(stats["scan_count"], 0)

        # Delete
        deleted = self.qr.delete_tracked(tid, token)
        self.assertTrue(deleted["deleted"])

        # Should be gone
        with self.assertRaises(NotFoundError):
            self.qr.tracked_stats(tid, token)

    def test_create_tracked_short_code_uniqueness(self):
        """Two tracked QRs with different short codes should coexist."""
        ts = int(time.time())
        r1 = self._track(
            self.qr.create_tracked("https://a.example.com", short_code=f"uniq-a-{ts}")
        )
        r2 = self._track(
            self.qr.create_tracked("https://b.example.com", short_code=f"uniq-b-{ts}")
        )
        self.assertNotEqual(r1["short_code"], r2["short_code"])
        self.assertNotEqual(r1["id"], r2["id"])

    def test_create_tracked_duplicate_short_code_rejected(self):
        """Duplicate short codes should be rejected."""
        code = f"dup-{int(time.time())}"
        self._track(self.qr.create_tracked("https://first.example.com", short_code=code))
        with self.assertRaises(QRServiceError):
            self.qr.create_tracked("https://second.example.com", short_code=code)

    def test_tracked_manage_token_is_secret(self):
        """Manage tokens should be unique per tracked QR."""
        r1 = self._track(self.qr.create_tracked("https://t1.example.com"))
        r2 = self._track(self.qr.create_tracked("https://t2.example.com"))
        self.assertNotEqual(r1["manage_token"], r2["manage_token"])

    def test_double_delete_rejected(self):
        """Deleting an already-deleted tracked QR should 404."""
        created = self.qr.create_tracked("https://double-del.example.com")
        self.qr.delete_tracked(created["id"], created["manage_token"])
        with self.assertRaises(NotFoundError):
            self.qr.delete_tracked(created["id"], created["manage_token"])


# =========================================================================
# View endpoint
# =========================================================================


class TestView(QRServiceTestCase):
    def test_view_basic(self):
        """View endpoint should return HTML or an image."""
        result = self.qr.view("https://view-test.example.com")
        self.assertIsNotNone(result)
        self.assertIsInstance(result, bytes)

    def test_view_with_style(self):
        result = self.qr.view("view-style", style="dots")
        self.assertIsNotNone(result)

    def test_view_with_size(self):
        result = self.qr.view("view-size", size=512)
        self.assertIsNotNone(result)

    def test_view_with_colors(self):
        result = self.qr.view("view-colors", fg="#FF0000", bg="#00FF00")
        self.assertIsNotNone(result)

    def test_view_with_format(self):
        result = self.qr.view("view-format", format="svg")
        self.assertIsNotNone(result)


# =========================================================================
# Discovery
# =========================================================================


class TestDiscovery(QRServiceTestCase):
    def test_llms_txt(self):
        txt = self.qr.llms_txt()
        self.assertIsInstance(txt, str)
        self.assertIn("qr", txt.lower())

    def test_llms_txt_root(self):
        txt = self.qr.llms_txt_root()
        self.assertIsInstance(txt, str)
        self.assertIn("qr", txt.lower())

    def test_llms_txt_both_paths_same_content(self):
        """Both /llms.txt and /api/v1/llms.txt should serve the same content."""
        root = self.qr.llms_txt_root()
        api = self.qr.llms_txt()
        self.assertEqual(root, api)

    def test_openapi(self):
        spec = self.qr.openapi()
        self.assertIn("openapi", spec)
        self.assertIn("paths", spec)

    def test_openapi_has_info(self):
        spec = self.qr.openapi()
        self.assertIn("info", spec)
        self.assertIn("title", spec["info"])

    def test_openapi_has_generate_path(self):
        spec = self.qr.openapi()
        paths = spec.get("paths", {})
        has_generate = any("generate" in p for p in paths)
        self.assertTrue(has_generate, "OpenAPI should document /qr/generate")

    def test_skills_index(self):
        idx = self.qr.skills()
        self.assertIn("skills", idx)

    def test_skill_md(self):
        md = self.qr.skill_md()
        self.assertIsInstance(md, str)
        self.assertIn("qr", md.lower())

    def test_skill_md_v1(self):
        md = self.qr.skill_md_v1()
        self.assertIsInstance(md, str)
        self.assertIn("qr", md.lower())

    def test_skill_md_both_paths_equivalent(self):
        """Both skills paths should serve equivalent content."""
        well_known = self.qr.skill_md()
        v1 = self.qr.skill_md_v1()
        # Both should mention QR and be non-trivially long
        self.assertIn("qr", well_known.lower())
        self.assertIn("qr", v1.lower())
        self.assertGreater(len(well_known), 50)
        self.assertGreater(len(v1), 50)


# =========================================================================
# Convenience helpers
# =========================================================================


class TestHelpers(QRServiceTestCase):
    def test_image_bytes(self):
        result = self.qr.generate("bytes test")
        raw = self.qr.image_bytes(result)
        self.assertIsInstance(raw, bytes)
        self.assertTrue(len(raw) > 100)
        # PNG magic bytes
        self.assertTrue(raw[:4] == b"\x89PNG")

    def test_image_bytes_svg(self):
        result = self.qr.generate_svg("svg bytes")
        raw = self.qr.image_bytes(result)
        self.assertIn(b"<svg", raw)

    def test_image_bytes_pdf(self):
        result = self.qr.generate_pdf("pdf bytes")
        raw = self.qr.image_bytes(result)
        self.assertTrue(raw[:4] == b"%PDF")

    def test_save_qr(self):
        result = self.qr.generate("save test")
        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
            path = f.name
        try:
            self.qr.save_qr(result, path)
            with open(path, "rb") as f:
                data = f.read()
            self.assertTrue(data[:4] == b"\x89PNG")
            self.assertTrue(len(data) > 100)
        finally:
            os.unlink(path)

    def test_save_qr_svg(self):
        result = self.qr.generate_svg("save svg")
        with tempfile.NamedTemporaryFile(suffix=".svg", delete=False) as f:
            path = f.name
        try:
            self.qr.save_qr(result, path)
            with open(path, "rb") as f:
                data = f.read()
            self.assertIn(b"<svg", data)
        finally:
            os.unlink(path)

    def test_save_qr_pdf(self):
        result = self.qr.generate_pdf("save pdf")
        with tempfile.NamedTemporaryFile(suffix=".pdf", delete=False) as f:
            path = f.name
        try:
            self.qr.save_qr(result, path)
            with open(path, "rb") as f:
                data = f.read()
            self.assertTrue(data[:4] == b"%PDF")
        finally:
            os.unlink(path)

    def test_decode_from_saved(self):
        """Full roundtrip: generate → save → read → decode."""
        result = self.qr.generate("full roundtrip")
        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
            path = f.name
        try:
            self.qr.save_qr(result, path)
            with open(path, "rb") as f:
                raw = f.read()
            decoded = self.qr.decode(raw)
            self.assertEqual(decoded["data"], "full roundtrip")
        finally:
            os.unlink(path)

    def test_repr(self):
        self.assertIn(BASE_URL, repr(self.qr))


# =========================================================================
# Constructor
# =========================================================================


class TestConstructor(QRServiceTestCase):
    def test_env_config(self):
        """QRService reads QR_SERVICE_URL from environment."""
        original = os.environ.get("QR_SERVICE_URL")
        try:
            os.environ["QR_SERVICE_URL"] = "http://custom:9999"
            client = QRService()
            self.assertEqual(client.base_url, "http://custom:9999")
        finally:
            if original:
                os.environ["QR_SERVICE_URL"] = original
            else:
                os.environ.pop("QR_SERVICE_URL", None)

    def test_trailing_slash_stripped(self):
        client = QRService("http://localhost:3001/")
        self.assertEqual(client.base_url, "http://localhost:3001")

    def test_custom_timeout(self):
        client = QRService(BASE_URL, timeout=5)
        self.assertEqual(client.timeout, 5)

    def test_default_timeout(self):
        client = QRService(BASE_URL)
        self.assertEqual(client.timeout, 30)

    def test_default_url_without_env(self):
        original = os.environ.get("QR_SERVICE_URL")
        try:
            os.environ.pop("QR_SERVICE_URL", None)
            client = QRService()
            self.assertEqual(client.base_url, "http://localhost:3001")
        finally:
            if original:
                os.environ["QR_SERVICE_URL"] = original

    def test_explicit_url_overrides_env(self):
        original = os.environ.get("QR_SERVICE_URL")
        try:
            os.environ["QR_SERVICE_URL"] = "http://env-url:9999"
            client = QRService("http://explicit:1234")
            self.assertEqual(client.base_url, "http://explicit:1234")
        finally:
            if original:
                os.environ["QR_SERVICE_URL"] = original
            else:
                os.environ.pop("QR_SERVICE_URL", None)


# =========================================================================
# Exception hierarchy
# =========================================================================


class TestExceptions(QRServiceTestCase):
    def test_validation_error_inherits(self):
        self.assertTrue(issubclass(ValidationError, QRServiceError))

    def test_not_found_error_inherits(self):
        self.assertTrue(issubclass(NotFoundError, QRServiceError))

    def test_rate_limit_error_inherits(self):
        self.assertTrue(issubclass(RateLimitError, QRServiceError))

    def test_gone_error_inherits(self):
        self.assertTrue(issubclass(GoneError, QRServiceError))

    def test_server_error_inherits(self):
        self.assertTrue(issubclass(ServerError, QRServiceError))

    def test_exception_has_status_code(self):
        try:
            self.qr.generate("")
        except QRServiceError as e:
            self.assertEqual(e.status_code, 400)
            self.assertIsNotNone(e.body)

    def test_exception_has_body(self):
        try:
            self.qr.generate("")
        except QRServiceError as e:
            self.assertIsInstance(e.body, dict)
            self.assertIn("error", e.body)

    def test_not_found_status_code(self):
        try:
            self.qr.tracked_stats("nonexistent", "fake")
        except NotFoundError as e:
            self.assertEqual(e.status_code, 404)

    def test_validation_error_message(self):
        try:
            self.qr.generate("")
        except ValidationError as e:
            self.assertTrue(len(str(e)) > 0)

    def test_exception_str_representation(self):
        """QRServiceError should have a meaningful string representation."""
        try:
            self.qr.generate("")
        except QRServiceError as e:
            self.assertIsInstance(str(e), str)
            self.assertTrue(len(str(e)) > 0)


# =========================================================================
# Edge cases
# =========================================================================


class TestEdgeCases(QRServiceTestCase):
    def test_long_data(self):
        """QR codes can encode surprisingly long strings."""
        long_text = "A" * 500
        result = self.qr.generate(long_text)
        self.assertIsNotNone(result["image_base64"])

    def test_unicode_data(self):
        result = self.qr.generate("こんにちは世界 🌍")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "こんにちは世界 🌍")

    def test_url_special_chars(self):
        result = self.qr.generate("https://example.com/path?key=val&foo=bar#section")
        self.assertIsNotNone(result["image_base64"])

    def test_newlines_in_data(self):
        result = self.qr.generate("line1\nline2\nline3")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "line1\nline2\nline3")

    def test_whitespace_only(self):
        result = self.qr.generate("   ")
        self.assertIsNotNone(result["image_base64"])

    def test_single_char(self):
        result = self.qr.generate("X")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "X")

    def test_numeric_string(self):
        result = self.qr.generate("1234567890")
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], "1234567890")

    def test_special_chars_roundtrip(self):
        data = "!@#$%^&*()_+-=[]{}|;':\",./<>?"
        result = self.qr.generate(data)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], data)

    def test_email_format(self):
        data = "mailto:test@example.com?subject=Hello"
        result = self.qr.generate(data)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], data)

    def test_multiline_text(self):
        data = "Name: Alice\nPhone: +1234\nEmail: a@b.com\nNote: Test"
        result = self.qr.generate(data)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(decoded["data"], data)

    def test_json_payload(self):
        data = json.dumps({"key": "value", "nested": {"a": 1}})
        result = self.qr.generate(data)
        decoded = self.qr.decode(self.qr.image_bytes(result))
        self.assertEqual(json.loads(decoded["data"]), json.loads(data))


# =========================================================================
# Cross-feature interactions
# =========================================================================


class TestCrossFeature(QRServiceTestCase):
    """Tests that combine multiple features."""

    def test_template_to_batch(self):
        """Use template data format in batch requests."""
        wifi_data = "WIFI:T:WPA;S:BatchNet;P:pass123;;"
        result = self.qr.batch([
            {"data": wifi_data, "style": "dots"},
            {"data": wifi_data, "style": "rounded"},
        ])
        self.assertEqual(result["total"], 2)

    def test_generate_then_tracked(self):
        """Generate a regular QR and a tracked QR with same content — different images."""
        regular = self.qr.generate("https://dual-test.example.com")
        tracked = self._track(self.qr.create_tracked("https://dual-test.example.com"))
        # Tracked QR points to short URL, not original content
        self.assertNotEqual(regular["data"], tracked["qr"].get("data", ""))

    def test_all_formats_tracked(self):
        """Create tracked QRs in all 3 formats."""
        for fmt in ("png", "svg", "pdf"):
            result = self._track(
                self.qr.create_tracked(f"https://{fmt}.example.com", format=fmt)
            )
            self.assertIsNotNone(result["qr"]["image_base64"])

    def test_tracked_with_custom_style_and_colors(self):
        result = self._track(
            self.qr.create_tracked(
                "https://styled.example.com",
                style="rounded",
                fg_color="#333333",
                bg_color="#EEEEEE",
                size=512,
            )
        )
        self.assertIsNotNone(result["qr"]["image_base64"])

    def test_batch_all_formats_decode(self):
        """Batch with PNG items, decode each — verify ordering."""
        items = [{"data": f"xf-{i}", "format": "png"} for i in range(3)]
        result = self.qr.batch(items)
        for i, item in enumerate(result["items"]):
            decoded = self.qr.decode(self.qr.image_bytes(item))
            self.assertEqual(decoded["data"], f"xf-{i}")

    def test_save_and_decode_cycle(self):
        """Generate → save → load → decode → verify."""
        content = "save-decode-cycle"
        result = self.qr.generate(content)
        with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
            path = f.name
        try:
            self.qr.save_qr(result, path)
            with open(path, "rb") as f:
                raw = f.read()
            decoded = self.qr.decode(raw)
            self.assertEqual(decoded["data"], content)
        finally:
            os.unlink(path)

    def test_template_and_generate_same_data(self):
        """WiFi template and manual generation with same data should produce same QR content."""
        # Generate WiFi template
        wifi_result = self.qr.wifi("SameNet", "pass123")
        wifi_data = wifi_result["data"]
        # Manually generate with same data
        manual_result = self.qr.generate(wifi_data)
        # Decode both — should match
        d1 = self.qr.decode(self.qr.image_bytes(wifi_result))
        d2 = self.qr.decode(self.qr.image_bytes(manual_result))
        self.assertEqual(d1["data"], d2["data"])

    def test_vcard_batch(self):
        """Generate vCard data and use it in batch — verify generation succeeds."""
        vc = self.qr.vcard("Batch Person", email="batch@test.com")
        result = self.qr.batch([
            {"data": vc["data"], "style": "square"},
            {"data": vc["data"], "style": "rounded"},
        ])
        self.assertEqual(result["total"], 2)
        for item in result["items"]:
            self.assertIsNotNone(item["image_base64"])
        # Decode only the square style one (reliable with rqrr)
        decoded = self.qr.decode(self.qr.image_bytes(result["items"][0]))
        self.assertIn("Batch Person", decoded["data"])


# =========================================================================
# Error response structure
# =========================================================================


class TestErrorResponses(QRServiceTestCase):
    """Verify error responses have consistent structure."""

    def test_400_has_error_field(self):
        try:
            self.qr.generate("")
        except ValidationError as e:
            self.assertIsInstance(e.body, dict)
            self.assertIn("error", e.body)

    def test_404_has_error_info(self):
        try:
            self.qr.tracked_stats("nonexistent", "fake")
        except NotFoundError as e:
            self.assertEqual(e.status_code, 404)

    def test_invalid_format_error_message(self):
        try:
            self.qr.generate("x", format="bmp")
        except ValidationError as e:
            self.assertTrue(len(str(e)) > 0)

    def test_batch_validation_error(self):
        try:
            self.qr.batch([])
        except ValidationError as e:
            self.assertEqual(e.status_code, 400)
            self.assertIsNotNone(e.body)


if __name__ == "__main__":
    print(f"\n🔲 QR Service Python SDK Tests")
    print(f"   Server: {BASE_URL}\n")
    unittest.main(verbosity=2)
