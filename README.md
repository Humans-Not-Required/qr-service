# QR Service 🔲

**Agent QR Code Service** — Generate, customize, and decode QR codes via REST API with a human-friendly web dashboard.

Part of the [Humans-Not-Required](https://github.com/Humans-Not-Required) organization.

## Overview

A self-hosted QR code service built for AI agents first, humans second. Every feature is available via REST API with OpenAPI documentation. The web dashboard provides a visual interface for humans who prefer clicking over curling.

## Features

### Core QR Generation
- **Text/URL encoding** — Any text, URLs, or structured data
- **Multiple formats** — PNG, SVG, PDF output
- **Configurable size** — Custom dimensions and resolution
- **Error correction levels** — Low, Medium, Quartile, High
- **Batch generation** — Generate multiple QR codes in one request

### Customization
- **Colors** — Custom foreground/background colors
- **Logo overlay** — Embed a logo/image in the center
- **Border/quiet zone** — Configurable margins
- **Rounded corners** — Dot style customization (square, round, dots)
- **Templates** — Pre-built templates for common use cases:
  - vCard (contact info)
  - WiFi network credentials
  - URL with UTM tracking
  - Cryptocurrency address
  - Agent identity card

### QR Decoding
- **Image upload** — Decode QR from uploaded images (PNG, JPG, GIF, WebP)
- **URL fetch** — Decode QR from a remote image URL
- **Batch decode** — Multiple images in one request

### Analytics & Tracking (Optional)
- **Short URLs** — Generate tracked short URLs with QR codes
- **Scan counting** — Track how many times a QR code is scanned
- **Scan metadata** — Timestamp, user-agent, referrer (privacy-respecting)
- **Expiring QR codes** — Set TTL on tracked QR codes

### Management
- **API key authentication** — Secure access with API keys
- **Usage quotas** — Configurable rate limits per key
- **Generation history** — Browse previously generated QR codes
- **Favorites** — Save frequently-used QR configurations

## Tech Stack

- **Backend:** Rust + Rocket + SQLite
- **Frontend:** React + TypeScript + Tailwind CSS
- **AI Interface:** REST API + OpenAPI 3.0 spec

## API Quick Start

```bash
# Generate a simple QR code
curl -X POST https://your-instance/api/v1/qr/generate \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"data": "https://example.com", "format": "png", "size": 512}' \
  --output qr.png

# Generate with customization
curl -X POST https://your-instance/api/v1/qr/generate \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "data": "https://example.com",
    "format": "svg",
    "size": 512,
    "fg_color": "#000000",
    "bg_color": "#FFFFFF",
    "error_correction": "H",
    "style": "rounded"
  }' \
  --output qr.svg

# Decode a QR code
curl -X POST https://your-instance/api/v1/qr/decode \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -F "image=@photo.png"

# Generate from template
curl -X POST https://your-instance/api/v1/qr/template/wifi \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"ssid": "MyNetwork", "password": "secret", "encryption": "WPA2"}'

# Batch generate
curl -X POST https://your-instance/api/v1/qr/batch \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"items": [{"data": "https://a.com"}, {"data": "https://b.com"}], "format": "png"}'
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/qr/generate` | Generate a QR code |
| `POST` | `/api/v1/qr/decode` | Decode a QR code from image |
| `POST` | `/api/v1/qr/batch` | Batch generate QR codes |
| `POST` | `/api/v1/qr/template/{type}` | Generate from template |
| `GET` | `/api/v1/qr/history` | List generated QR codes |
| `GET` | `/api/v1/qr/{id}` | Get a specific QR code |
| `DELETE` | `/api/v1/qr/{id}` | Delete a QR code |
| `POST` | `/api/v1/qr/track` | Create tracked QR + short URL |
| `GET` | `/api/v1/qr/track/{id}/stats` | Get scan statistics |
| `GET` | `/api/v1/keys` | List API keys |
| `POST` | `/api/v1/keys` | Create API key |
| `DELETE` | `/api/v1/keys/{id}` | Revoke API key |
| `GET` | `/api/v1/openapi.json` | OpenAPI specification |
| `GET` | `/api/v1/health` | Health check |

## Running

```bash
# Backend
cd backend
cargo run

# Frontend
cd frontend
npm install
npm run dev

# Docker (both)
docker compose up
```

## License

MIT

## Contributing

See [CONTRIBUTING.md](../humans-not-required/CONTRIBUTING.md) in the main repo.
