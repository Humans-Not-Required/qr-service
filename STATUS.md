# QR Service - Status

## Current State: MVP Backend ✅ + Style Rendering ✅

The Rust/Rocket backend compiles, runs, and has passing tests. Core QR generation, decoding, raw image serving, and styled rendering all work end-to-end. All clippy warnings resolved, all code formatted.

### What's Done

- **Core API** (all routes implemented):
  - `POST /api/v1/qr/generate` — PNG and SVG generation with custom colors, sizes, error correction, **and styles**
  - `POST /api/v1/qr/decode` — Real QR decoding via `rqrr` crate (roundtrip verified)
  - `POST /api/v1/qr/batch` — Batch generation (up to 50 items) with style support
  - `POST /api/v1/qr/template/{type}` — WiFi, vCard, URL templates with style support
  - `GET /api/v1/qr/history` — Paginated history per API key
  - `GET /api/v1/qr/{id}` — Fetch specific QR code (base64 JSON)
  - `GET /api/v1/qr/{id}/image` — Raw image bytes (PNG/SVG) with proper Content-Type
  - `DELETE /api/v1/qr/{id}` — Delete QR code
  - `GET /api/v1/keys` — List API keys (admin only)
  - `POST /api/v1/keys` — Create API key (admin only)
  - `DELETE /api/v1/keys/{id}` — Revoke API key (admin only)
  - `GET /api/v1/health` — Health check
  - `GET /api/v1/openapi.json` — OpenAPI 3.0 spec (fully up to date)
- **Style Rendering** (NEW):
  - `square` — standard sharp-edge modules (default)
  - `rounded` — context-aware rounded corners (only rounds exposed corners where both adjacent edges are free; uses quadratic curves in SVG, pixel-level distance checks in PNG)
  - `dots` — circular modules (circles in SVG, distance-from-center in PNG)
  - All styles verified scannable via roundtrip encode/decode tests
- **Auth:** API key authentication via `Authorization: Bearer` or `X-API-Key` header
- **Database:** SQLite with WAL mode, auto-creates admin key on first run
- **Docker:** Dockerfile (multi-stage build) + docker-compose.yml
- **Config:** Environment variables via `.env` / `dotenvy` (DATABASE_PATH, ROCKET_ADDRESS, ROCKET_PORT)
- **Tests:** 18 integration tests passing (color parsing, PNG/SVG generation, templates, roundtrip, style rendering, style roundtrips)
- **Code Quality:** Zero clippy warnings, cargo fmt clean
- **OpenAPI spec** fully updated with all endpoints and style descriptions

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
- **Context-aware rounded corners** — corners only round where both adjacent edges lack neighbors, preserving QR scannability while looking smooth
- **No new dependencies for style rendering** — all drawing done with existing `image` crate

### What's Next (Priority Order)

1. **Push CI workflow** — needs `workflow` scope on GitHub token, or manual push via web UI
2. **Tracked QR / short URLs** — the `tracked_qr` and `scan_events` tables exist but routes aren't implemented
3. **Rate limiting** — per-key rate limit exists in schema but isn't enforced
4. **Frontend** — React dashboard for human users
5. **PDF output format** — mentioned in README, not yet implemented
6. **Logo/image overlay** — embed a small logo in the center of QR codes (requires high EC)

### ⚠️ Gotchas

- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CI workflow push blocked — GitHub token lacks `workflow` scope
- Styles accepted but style column in DB is informational only (not used for re-rendering)
- CORS wide open (all origins) — tighten for production

### Architecture Notes

- Single-threaded SQLite via `Mutex<Connection>` — fine for moderate load, would need connection pooling for high concurrency
- Images stored as BLOBs in SQLite — works for small-to-medium volume, consider filesystem storage for high volume
- CORS wide open (all origins) — tighten for production
- Style rendering is pixel-level for PNG (no external drawing lib needed) and SVG-native for SVG output

---

*Last updated: 2026-02-07 08:37 UTC — Session: style rendering (rounded + dots) for PNG and SVG*
