# QR Service - Status

## Current State: MVP Backend ✅

The Rust/Rocket backend compiles, runs, and has passing tests. Core QR generation and decoding works end-to-end.

### What's Done

- **Core API** (all routes implemented):
  - `POST /api/v1/qr/generate` — PNG and SVG generation with custom colors, sizes, error correction
  - `POST /api/v1/qr/decode` — Real QR decoding via `rqrr` crate (roundtrip verified)
  - `POST /api/v1/qr/batch` — Batch generation (up to 50 items)
  - `POST /api/v1/qr/template/{type}` — WiFi, vCard, URL templates
  - `GET /api/v1/qr/history` — Paginated history per API key
  - `GET /api/v1/qr/{id}` — Fetch specific QR code
  - `DELETE /api/v1/qr/{id}` — Delete QR code
  - `GET /api/v1/keys` — List API keys (admin only)
  - `POST /api/v1/keys` — Create API key (admin only)
  - `DELETE /api/v1/keys/{id}` — Revoke API key (admin only)
  - `GET /api/v1/health` — Health check
  - `GET /api/v1/openapi.json` — OpenAPI 3.0 spec
- **Auth:** API key authentication via `Authorization: Bearer` or `X-API-Key` header
- **Database:** SQLite with WAL mode, auto-creates admin key on first run
- **Docker:** Dockerfile (multi-stage build) + docker-compose.yml
- **Config:** Environment variables via `.env` / `dotenvy` (DATABASE_PATH, ROCKET_ADDRESS, ROCKET_PORT)
- **Tests:** 11 integration tests passing (color parsing, PNG/SVG generation, templates, roundtrip encode/decode)
- **OpenAPI spec** included

### Tech Stack

- Rust 1.83+ / Rocket 0.5 / SQLite (rusqlite)
- QR generation: `qrcode` crate
- QR decoding: `rqrr` crate
- Image processing: `image` crate

### Key Decisions

- **SQLite over Postgres** — simplicity for a self-hosted service, no external deps
- **Base64 data URIs in JSON responses** — agents can embed directly, no secondary download
- **Admin key auto-generated** — printed to stdout on first run (save it!)
- **No style rendering yet** — "rounded" and "dots" styles accepted but render as square

### What's Next (Priority Order)

1. **GitHub Actions CI** — automated test + build on push
2. **Raw image endpoint** — `GET /api/v1/qr/{id}/image` returning actual PNG/SVG (not base64 JSON)
3. **Style rendering** — implement rounded corners and dot patterns
4. **Tracked QR / short URLs** — the `tracked_qr` and `scan_events` tables exist but routes aren't implemented
5. **Rate limiting** — per-key rate limit exists in schema but isn't enforced
6. **Frontend** — React dashboard for human users
7. **PDF output format** — mentioned in README, not yet implemented

### Architecture Notes

- Single-threaded SQLite via `Mutex<Connection>` — fine for moderate load, would need connection pooling for high concurrency
- Images stored as BLOBs in SQLite — works for small-to-medium volume, consider filesystem storage for high volume
- CORS wide open (all origins) — tighten for production

---

*Last updated: 2026-02-07 08:00 UTC — Session: initial assessment + QR decode implementation + Docker + tests*
