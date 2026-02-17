# QR Service Python SDK

Zero-dependency Python client for the [HNR QR Service](../../README.md). Works with Python 3.8+ using only the standard library.

## Install

Copy `qr_service.py` into your project — no pip install needed.

```python
from qr_service import QRService
```

## Quick Start

```python
from qr_service import QRService

qr = QRService("http://localhost:3001")

# Generate a QR code
result = qr.generate("https://example.com")
print(result["share_url"])

# Save to file
qr.save_qr(result, "my-qr.png")

# Generate SVG with custom colors and dot style
result = qr.generate_svg("Hello World", fg_color="#FF6600", style="dots")

# Decode a QR code from image bytes
with open("qr.png", "rb") as f:
    decoded = qr.decode(f.read())
print(decoded["data"])
```

## Configuration

```python
# Explicit URL
qr = QRService("http://192.168.0.79:3001")

# From environment variable
# export QR_SERVICE_URL=http://192.168.0.79:3001
qr = QRService()  # reads QR_SERVICE_URL

# Custom timeout
qr = QRService(timeout=60)
```

## Generate QR Codes

### Single Generation

```python
# PNG (default)
result = qr.generate("https://example.com")

# SVG
result = qr.generate("Hello", format="svg")

# PDF
result = qr.generate("PDF content", format="pdf")

# Custom options
result = qr.generate(
    "https://example.com",
    size=512,                    # 64-4096 pixels
    fg_color="#1a1a2e",          # Foreground color
    bg_color="#e2e2e2",          # Background color
    error_correction="H",        # L, M, Q, H
    style="rounded",             # square, rounded, dots
)

# Convenience shortcuts
result = qr.generate_png("data")
result = qr.generate_svg("data")
result = qr.generate_pdf("data")
```

### Logo Overlay

```python
import base64

with open("logo.png", "rb") as f:
    logo_b64 = base64.b64encode(f.read()).decode()

result = qr.generate(
    "https://example.com",
    logo=logo_b64,       # base64 or data: URI, max 512KB
    logo_size=25,        # 5-40% of QR dimensions
)
```

### Batch Generation

```python
# Up to 50 items per request
result = qr.batch([
    {"data": "https://a.com"},
    {"data": "https://b.com", "format": "svg", "style": "dots"},
    {"data": "https://c.com", "size": 512, "fg_color": "#FF0000"},
])

for item in result["items"]:
    print(f"{item['data']} → {item['share_url']}")
```

## Templates

### WiFi Network

```python
result = qr.wifi("MyNetwork", "password123")
result = qr.wifi("OpenNet", "", encryption="nopass")
result = qr.wifi("HiddenNet", "secret", hidden=True, style="dots")
```

### vCard Contact

```python
result = qr.vcard(
    "Alice Smith",
    email="alice@example.com",
    phone="+1234567890",
    org="Acme Corp",
    title="Engineer",
    url="https://alice.example.com",
)
```

### URL with UTM Parameters

```python
result = qr.url(
    "https://example.com/landing",
    utm_source="twitter",
    utm_medium="social",
    utm_campaign="launch",
)
```

## Tracked QR Codes (Analytics)

```python
# Create tracked QR with short URL redirect
tracked = qr.create_tracked("https://example.com")
print(f"Short URL: {tracked['short_url']}")
print(f"Short code: {tracked['short_code']}")
token = tracked["manage_token"]  # Save this!

# Custom short code
tracked = qr.create_tracked(
    "https://example.com",
    short_code="my-custom-code",
    expires_at="2025-12-31T23:59:59Z",
)

# Get scan analytics
stats = qr.tracked_stats(tracked["id"], token)
print(f"Scans: {stats['scan_count']}")
for scan in stats["recent_scans"]:
    print(f"  {scan['scanned_at']} — {scan.get('user_agent', 'unknown')}")

# Delete tracked QR
qr.delete_tracked(tracked["id"], token)
```

## Decode QR Codes

```python
# From file
with open("qr.png", "rb") as f:
    decoded = qr.decode(f.read())
print(decoded["data"])

# From generate result (roundtrip)
result = qr.generate("test content")
raw_bytes = qr.image_bytes(result)
decoded = qr.decode(raw_bytes)
assert decoded["data"] == "test content"
```

## Convenience Helpers

```python
# Save QR to file
result = qr.generate("save me")
qr.save_qr(result, "output.png")

# Extract raw image bytes
raw = qr.image_bytes(result)  # PNG/SVG/PDF bytes

# Full roundtrip: generate → save → read → decode
result = qr.generate("roundtrip")
qr.save_qr(result, "test.png")
with open("test.png", "rb") as f:
    decoded = qr.decode(f.read())
```

## Discovery Endpoints

```python
# AI-readable docs
txt = qr.llms_txt()

# OpenAPI spec
spec = qr.openapi()

# Agent skills discovery
skills = qr.skills()
skill_doc = qr.skill_md()
```

## Error Handling

```python
from qr_service import (
    QRServiceError,    # Base exception
    ValidationError,   # 400/422 — bad input
    NotFoundError,     # 404 — resource not found
    RateLimitError,    # 429 — too many requests
    GoneError,         # 410 — expired resource
    ServerError,       # 5xx — server errors
)

try:
    result = qr.generate("")
except ValidationError as e:
    print(f"Bad input: {e}")
    print(f"Status: {e.status_code}")
    print(f"Body: {e.body}")

try:
    qr.tracked_stats("bad-id", "bad-token")
except NotFoundError:
    print("Not found or invalid token")

try:
    # Heavy usage
    for i in range(200):
        qr.generate(f"flood-{i}")
except RateLimitError as e:
    print(f"Rate limited! Retry in {e.retry_after_secs}s")
    print(f"Limit: {e.limit}, Remaining: {e.remaining}")
```

## Response Format

All generate methods return:

```python
{
    "image_base64": "data:image/png;base64,...",  # Full data URI
    "share_url": "/qr/view?data=...",             # Stateless share URL
    "format": "png",                               # png, svg, or pdf
    "size": 256,                                   # Image dimensions
    "data": "https://example.com",                 # Original content
}
```

Tracked QR creation returns additional fields:

```python
{
    "id": "...",
    "manage_token": "...",      # Save this for stats/delete!
    "short_url": "http://...",
    "short_code": "abc123",
    "target_url": "https://...",
    "scan_count": 0,
    "qr": { ... },             # Nested QR response
}
```

## Running Tests

```bash
# Against local server
python test_sdk.py

# Against staging
QR_SERVICE_URL=http://192.168.0.79:3001 python test_sdk.py -v
```
