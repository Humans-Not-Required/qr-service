# QR Service - Status

## Current State: Feature-Complete ✅

Stateless QR code generation/decoding service with tracked QR analytics, logo overlay, PDF output, and React frontend. All 122 tests passing, zero clippy warnings, CI green.

### What's Done

- **Core API (stateless, no auth needed):**
  - `POST /api/v1/qr/generate` — PNG, SVG, PDF output with custom colors, sizes, error correction, styles (square/rounded/dots), and logo overlay
  - `POST /api/v1/qr/decode` — QR decoding from base64 image
  - `POST /api/v1/qr/batch` — Batch generation (up to 50 items)
  - `POST /api/v1/qr/template/{type}` — WiFi, vCard, URL templates
  - `GET /qr/view?data=...` — Stateless share URL (regenerates from params)
- **Style Rendering:**
  - `square` — standard sharp-edge modules (default)
  - `rounded` — context-aware rounded corners
  - `dots` — circular modules
  - All styles verified scannable via roundtrip tests
- **Logo Overlay:**
  - Optional `logo` field (base64/data URI, max 512KB) + `logo_size` (5-40%, default 20%)
  - Auto-upgrades EC to H for maximum redundancy
  - PNG alpha-blended overlay with white rounded-rect background
  - SVG embedded `<image>` element
  - Roundtrip scannable verified
- **PDF Output:**
  - Vector PDF via `printpdf` — all 3 styles rendered as PDF paths/shapes
  - Available in generate, batch, template, view, and tracked endpoints
- **Tracked QR / Short URLs (per-resource manage_token):**
  - `POST /api/v1/qr/tracked` — Create tracked QR with short URL
  - `GET /api/v1/qr/tracked/{id}/stats` — Scan analytics with recent events
  - `DELETE /api/v1/qr/tracked/{id}` — Delete tracked QR
  - `GET /r/{code}` — Short URL redirect (records scans)
  - Custom or auto-generated short codes, optional expiry
- **Rate Limiting:** IP-based, 100 req/min, configurable via `RATE_LIMIT_WINDOW_SECS`
- **Frontend:** React + Vite SPA with Generate, Decode, Templates, Tracked tabs
  - Logo upload with preview and size slider
  - PDF format support with download
  - Tracked QR dashboard with analytics (timeline chart, device breakdown)
  - Dark theme matching HNR design system
  - Responsive layout
- **Discovery:** `/api/v1/openapi.json`, `/llms.txt` (130 lines, comprehensive)
- **Docker:** Multi-stage build (frontend + backend), unified serving on single port
- **CI/CD:** GitHub Actions → ghcr.io + Watchtower auto-deploy
- **Tests:** 122 total (88 HTTP + 28 integration + 3 unit + 3 lib), zero clippy warnings

### Tech Stack

- Rust 1.83+ / Rocket 0.5 / SQLite (rusqlite)
- QR generation: `qrcode` + `image` + `printpdf`
- QR decoding: `rqrr`
- React 18 + Vite
- Port: 3001 external, 8000 internal

### What's Next

- All roadmap items complete — feature-complete
- Cloudflare tunnel for public access (Jordan action)

### ⚠️ Gotchas

- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"`
- Styles accepted but style column in DB is informational only
- CORS wide open (all origins) — tighten for production
- BASE_URL defaults to `http://localhost:8000` — must be set in production
- Rate limiter state is in-memory — resets on restart
- ~~Rate limit response headers not wired to stateless endpoints~~ ✅ Fixed — `RateLimited<T>` responder attaches X-RateLimit-Limit/Remaining/Reset headers on all rate-limited endpoints. 429 responses include retry_after_secs/limit/remaining in body.
- ~~Batch endpoint does NOT apply logo overlay~~ ✅ Fixed — batch now supports logo field with auto EC-H upgrade, PNG overlay, and SVG embedded image. PDF batch still skips logo (no logo support in PDF renderer).

### Recent Completed

- **Rate limit headers + batch logo overlay bug fixes** (2026-02-17) — Fixed two documented bugs: (1) Rate limit response headers (X-RateLimit-Limit/Remaining/Reset) now attached to all stateless endpoints via `RateLimited<T>` responder pattern, replacing the broken fairing approach. 429 responses include retry_after_secs/limit/remaining in body. (2) Batch endpoint now applies logo overlay (PNG alpha-blend, SVG embedded image) with auto EC-H upgrade, matching single-generate behavior. Extended `ApiError` with optional rate limit fields. Removed dead `RateLimitHeaders` fairing. 9 new tests (122 total). Commit: pending.
- **Logo overlay UI** (2026-02-16) — Frontend file picker, preview, size slider (5-40%), EC upgrade notice. Fixed vCard template fields to match API (name, not first_name/last_name). Added title + website vCard fields. 
- **26 new tests** (2026-02-16) — Batch edge cases (>50 rejected, mixed formats, single item), generate edge cases (min/max size, all EC levels, all styles SVG), decode edge cases (empty image, non-QR image), view params (style, colors, size, missing data), tracked QR (expiry, short code validation, delete without token), template edge cases (vcard minimal/missing, wifi nopass, url SVG), logo+PDF, response field validation. Commit: 87d4980.
- **PDF output format** (2026-02-16) — Vector PDF via printpdf. 7 new tests.
- **Logo overlay** (2026-02-16) — Base64 logo with auto EC-H. PNG/SVG support. 16 new tests.
- **llms.txt expansion** (2026-02-16) — 45→130 lines with full endpoint reference.
- **Analytics dashboard enhancements** (2026-02-13) — Scan timeline, device breakdown, relative times.
- **Full UI reevaluation** (2026-02-11) — CSS extraction, responsive design, toast system, animations.

*Last updated: 2026-02-17 03:30 UTC. 122 tests, zero clippy warnings, CI green.*

## Incoming Directions (Work Queue)

<!-- WORK_QUEUE_DIRECTIONS_START -->
(Cleared — UI reevaluation completed 2026-02-11, direction 1de0175c addressed)
<!-- WORK_QUEUE_DIRECTIONS_END -->
