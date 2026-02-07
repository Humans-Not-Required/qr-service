# QR Service - Status

## Current State: MVP Backend ✅ + Raw Image Endpoint ✅

The Rust/Rocket backend compiles, runs, and has passing tests. Core QR generation, decoding, and raw image serving work end-to-end. All clippy warnings resolved, all code formatted.

### What's Done

- **Core API** (all routes implemented):
  - `POST /api/v1/qr/generate` — PNG and SVG generation with custom colors, sizes, error correction
  - `POST /api/v1/qr/decode` — Real QR decoding via `rqrr` crate (roundtrip verified)
  - `POST /api/v1/qr/batch` — Batch generation (up to 50 items)
  - `POST /api/v1/qr/template/{type}` — WiFi, vCard, URL templates
  - `GET /api/v1/qr/history` — Paginated history per API key
  - `GET /api/v1/qr/{id}` — Fetch specific QR code (base64 JSON)
  - `GET /api/v1/qr/{id}/image` — **NEW** Raw image bytes (PNG/SVG) with proper Content-Type
  - `DELETE /api/v1/qr/{id}` — Delete QR code
  - `GET /api/v1/keys` — List API keys (admin only)
  - `POST /api/v1/keys` — Create API key (admin only)
  - `DELETE /api/v1/keys/{id}` — Revoke API key (admin only)
  - `GET /api/v1/health` — Health check
  - `GET /api/v1/openapi.json` — OpenAPI 3.0 spec (updated with /image endpoint)
- **Auth:** API key authentication via `Authorization: Bearer` or `X-API-Key` header
- **Database:** SQLite with WAL mode, auto-creates admin key on first run
- **Docker:** Dockerfile (multi-stage build) + docker-compose.yml
- **Config:** Environment variables via `.env` / `dotenvy` (DATABASE_PATH, ROCKET_ADDRESS, ROCKET_PORT)
- **Tests:** 11 integration tests passing (color parsing, PNG/SVG generation, templates, roundtrip encode/decode)
- **Code Quality:** Zero clippy warnings, cargo fmt clean
- **OpenAPI spec** updated with all endpoints

### GitHub Actions CI (Ready but Blocked)

- `.github/workflows/ci.yml` exists locally but can't be pushed — OAuth token lacks `workflow` scope
- Workflow includes: fmt check, clippy with -D warnings, test suite, release build, Docker build
- **Action needed:** Either add `workflow` scope to token or push the file manually via GitHub web UI
- File location: `.github/workflows/ci.yml`

### Tech Stack

- Rust 1.83+ / Rocket 0.5 / SQLite (rusqlite)
- QR generation: `qrcode` crate
- QR decoding: `rqrr` crate
- Image processing: `image` crate

### Key Decisions

- **SQLite over Postgres** — simplicity for a self-hosted service, no external deps
- **Base64 data URIs in JSON responses** — agents can embed directly, no secondary download
- **Raw image endpoint** — `/qr/{id}/image` returns actual bytes for efficient downloads
- **Admin key auto-generated** — printed to stdout on first run (save it!)
- **No style rendering yet** — "rounded" and "dots" styles accepted but render as square

### What's Next (Priority Order)

1. **Push CI workflow** — needs `workflow` scope on GitHub token, or manual push via web UI
2. **Style rendering** — implement rounded corners and dot patterns
3. **Tracked QR / short URLs** — the `tracked_qr` and `scan_events` tables exist but routes aren't implemented
4. **Rate limiting** — per-key rate limit exists in schema but isn't enforced
5. **Frontend** — React dashboard for human users
6. **PDF output format** — mentioned in README, not yet implemented

### Architecture Notes

- Single-threaded SQLite via `Mutex<Connection>` — fine for moderate load, would need connection pooling for high concurrency
- Images stored as BLOBs in SQLite — works for small-to-medium volume, consider filesystem storage for high volume
- CORS wide open (all origins) — tighten for production

---

*Last updated: 2026-02-07 08:10 UTC — Session: raw image endpoint + CI workflow + clippy/fmt cleanup*
