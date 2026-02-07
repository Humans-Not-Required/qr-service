# QR Service 🔲

**Agent QR Code Service** — Generate, customize, decode, and track QR codes via REST API.

Part of the [Humans-Not-Required](https://github.com/Humans-Not-Required) organization.

## Overview

A self-hosted QR code service built for AI agents first, humans second. Every feature is available via REST API with full OpenAPI 3.0 documentation. Designed for agents that need to create, decode, or track QR codes programmatically.

## Features

### Core QR Generation
- **Text/URL encoding** — Any text, URLs, or structured data
- **Multiple formats** — PNG and SVG output
- **Configurable size** — 64px to 4096px
- **Error correction** — L (7%), M (15%), Q (25%), H (30%)
- **Batch generation** — Up to 50 QR codes in one request

### Style Rendering
- **Square** — Standard sharp-edge modules (default)
- **Rounded** — Context-aware rounded corners (only rounds exposed edges)
- **Dots** — Circular modules

All styles produce scannable QR codes (verified via roundtrip encode/decode tests).

### Custom Colors
- Hex color codes for foreground and background
- Works with all styles and formats

### Templates
Pre-built templates for common use cases:
- **WiFi** — Network name, password, encryption, hidden flag
- **vCard** — Name, email, phone, org, title, URL
- **URL** — With optional UTM parameters (source, medium, campaign)

### QR Decoding
- **Image upload** — Decode QR from raw image bytes (PNG, JPEG, etc.)
- **Reliable detection** — Uses the `rqrr` crate for robust QR decoding

### Tracked QR Codes & Short URLs
- **Short URL redirects** — QR encodes a short URL (`/r/{code}`) that redirects to the target
- **Custom short codes** — Choose your own (3-32 chars) or auto-generate
- **Scan analytics** — Each scan records timestamp, User-Agent, and Referer
- **Expiring links** — Set an ISO-8601 expiry (returns 410 Gone after expiration)
- **Configurable BASE_URL** — Short URLs work in any deployment environment

### Management
- **API key auth** — `Authorization: Bearer` or `X-API-Key` header
- **Admin key management** — Create and revoke API keys (admin only)
- **Generation history** — Paginated list of previously generated QR codes
- **Raw image endpoint** — `/qr/{id}/image` returns raw bytes (no base64 overhead)

## Tech Stack

- **Language:** Rust 1.83+
- **Framework:** Rocket 0.5
- **Database:** SQLite with WAL mode (via `rusqlite`)
- **QR Generation:** `qrcode` crate
- **QR Decoding:** `rqrr` crate
- **Image Processing:** `image` crate (no external drawing dependencies)

## Quick Start

### Running Locally

```bash
cd backend
cargo run
```

On first run, an admin API key is auto-generated and printed to stdout — **save it!**

### Docker

```bash
docker compose up
```

### Configuration

Set via environment variables or `.env` file:

| Variable | Default | Description |
|----------|---------|-------------|
| `DATABASE_PATH` | `qr_service.db` | SQLite database file path |
| `ROCKET_ADDRESS` | `0.0.0.0` | Listen address |
| `ROCKET_PORT` | `8000` | Listen port |
| `BASE_URL` | `http://localhost:8000` | Base URL for short URL generation |

## API Quick Start

```bash
# Health check (no auth required)
curl http://localhost:8000/api/v1/health

# Generate a QR code (base64 JSON response)
curl -X POST http://localhost:8000/api/v1/qr/generate \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"data": "https://example.com", "format": "png", "size": 512}'

# Generate with style
curl -X POST http://localhost:8000/api/v1/qr/generate \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "data": "https://example.com",
    "format": "svg",
    "size": 512,
    "fg_color": "#1a1a2e",
    "bg_color": "#e0e0e0",
    "error_correction": "H",
    "style": "rounded"
  }'

# Get raw image (no base64 overhead)
curl http://localhost:8000/api/v1/qr/{id}/image \
  -H "Authorization: Bearer YOUR_API_KEY" \
  --output qr.png

# Decode a QR code from image
curl -X POST http://localhost:8000/api/v1/qr/decode \
  -H "Authorization: Bearer YOUR_API_KEY" \
  --data-binary @photo.png

# WiFi template
curl -X POST http://localhost:8000/api/v1/qr/template/wifi \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"ssid": "MyNetwork", "password": "secret", "encryption": "WPA2"}'

# Create a tracked QR code with short URL
curl -X POST http://localhost:8000/api/v1/qr/tracked \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "target_url": "https://example.com/campaign",
    "short_code": "summer-sale",
    "style": "dots",
    "error_correction": "H"
  }'

# Check scan analytics
curl http://localhost:8000/api/v1/qr/tracked/{id}/stats \
  -H "Authorization: Bearer YOUR_API_KEY"

# Batch generate
curl -X POST http://localhost:8000/api/v1/qr/batch \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"items": [{"data": "https://a.com"}, {"data": "https://b.com", "style": "dots"}]}'
```

## API Endpoints

### QR Codes (`/api/v1`)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/qr/generate` | Generate a QR code |
| `POST` | `/qr/decode` | Decode a QR code from image |
| `POST` | `/qr/batch` | Batch generate (up to 50) |
| `POST` | `/qr/template/{type}` | Generate from template (wifi/vcard/url) |
| `GET` | `/qr/history` | List generated QR codes (paginated) |
| `GET` | `/qr/{id}` | Get QR code (base64 JSON) |
| `GET` | `/qr/{id}/image` | Get raw image bytes |
| `DELETE` | `/qr/{id}` | Delete a QR code |

### Tracked QR / Short URLs (`/api/v1`)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/qr/tracked` | Create tracked QR with short URL |
| `GET` | `/qr/tracked` | List tracked QR codes (paginated) |
| `GET` | `/qr/tracked/{id}/stats` | Scan analytics (last 100 events) |
| `DELETE` | `/qr/tracked/{id}` | Delete tracked QR + scan events |

### Short URL Redirect (root)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/r/{code}` | Redirect to target URL (records scan) |

### Admin (`/api/v1`)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/keys` | List API keys (admin) |
| `POST` | `/keys` | Create API key (admin) |
| `DELETE` | `/keys/{id}` | Revoke API key (admin) |

### Meta (`/api/v1`)

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/health` | Health check (no auth) |
| `GET` | `/openapi.json` | OpenAPI 3.0 specification |

## Architecture Notes

- Single SQLite file with WAL mode — no external database needed
- Images stored as BLOBs — works well for moderate volume
- `Mutex<Connection>` for thread safety — fine for moderate concurrency
- Style rendering is pixel-level for PNG and SVG-native — no external drawing libraries
- Redirect routes at root (`/r/`), API routes at `/api/v1` — clean separation
- CORS is wide open (all origins) — tighten for production

## Roadmap

- [ ] GitHub Actions CI (blocked on token scope)
- [ ] Rate limiting (schema exists, enforcement pending)
- [ ] Frontend dashboard
- [ ] PDF output format
- [ ] Logo/image overlay (center of QR, requires high EC)

## License

MIT

## Contributing

See [CONTRIBUTING.md](https://github.com/Humans-Not-Required/humans-not-required/blob/main/CONTRIBUTING.md) in the main repo.
