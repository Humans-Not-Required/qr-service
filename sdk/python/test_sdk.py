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


# =========================================================================
# Discovery
# =========================================================================


class TestDiscovery(QRServiceTestCase):
    def test_llms_txt(self):
        txt = self.qr.llms_txt()
        self.assertIsInstance(txt, str)
        self.assertIn("qr", txt.lower())

    def test_openapi(self):
        spec = self.qr.openapi()
        self.assertIn("openapi", spec)
        self.assertIn("paths", spec)

    def test_skills_index(self):
        idx = self.qr.skills()
        self.assertIn("skills", idx)

    def test_skill_md(self):
        md = self.qr.skill_md()
        self.assertIsInstance(md, str)
        self.assertIn("qr", md.lower())


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

    def test_batch_preserves_order(self):
        items = [{"data": f"order-{i}"} for i in range(5)]
        result = self.qr.batch(items)
        for i, item in enumerate(result["items"]):
            self.assertEqual(item["data"], f"order-{i}")

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


if __name__ == "__main__":
    print(f"\n🔲 QR Service Python SDK Tests")
    print(f"   Server: {BASE_URL}\n")
    unittest.main(verbosity=2)
