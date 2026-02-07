# QR Service - Status

## Current State: MVP Backend ✅ + Style Rendering ✅ + Tracked QR / Short URLs ✅ + OpenAPI Complete ✅

The Rust/Rocket backend compiles, runs, and has passing tests. Core QR generation, decoding, raw image serving, styled rendering, and tracked QR codes with scan analytics all work end-to-end. All clippy warnings resolved, all code formatted.

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
  - `GET /api/v1/openapi.json` — OpenAPI 3.0 spec (needs update for tracked QR endpoints)
- **Style Rendering:**
  - `square` — standard sharp-edge modules (default)
  - `rounded` — context-aware rounded corners
  - `dots` — circular modules
  - All styles verified scannable via roundtrip encode/decode tests
- **Tracked QR / Short URLs** (NEW):
  - `POST /api/v1/qr/tracked` — Create a tracked QR code that encodes a short URL
    - Custom or auto-generated short codes (3-32 chars, alphanumeric + hyphens/underscores)
    - Optional expiry (ISO-8601 timestamp)
    - Configurable BASE_URL for short URL generation
    - Returns full QR response + tracking metadata
  - `GET /api/v1/qr/tracked` — List tracked QR codes (paginated)
  - `GET /api/v1/qr/tracked/{id}/stats` — Scan analytics with last 100 events
  - `DELETE /api/v1/qr/tracked/{id}` — Delete tracked QR + scan events + underlying QR code
  - `GET /r/{code}` — Short URL redirect (mounted at root, not /api/v1)
    - Records scan events with User-Agent and Referer
    - Increments scan count atomically
    - Checks expiry (returns 410 Gone if expired)
    - Returns 302 Temporary Redirect to target URL
- **Auth:** API key authentication via `Authorization: Bearer` or `X-API-Key` header
- **Database:** SQLite with WAL mode, auto-creates admin key on first run
- **Docker:** Dockerfile (multi-stage build) + docker-compose.yml
- **Config:** Environment variables via `.env` / `dotenvy` (DATABASE_PATH, ROCKET_ADDRESS, ROCKET_PORT, BASE_URL)
- **Tests:** 22 integration tests passing (color parsing, PNG/SVG generation, templates, roundtrip, style rendering, style roundtrips, tracked QR DB roundtrip, short code uniqueness, cascade delete, expiry check)
- **Code Quality:** Zero clippy warnings, cargo fmt clean

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
- **Context-aware rounded corners** — corners only round where both adjacent edges lack neighbors
- **No new dependencies for style rendering** — all drawing done with existing `image` crate
- **Short URLs at root `/r/`** — clean, short redirects outside the API prefix
- **ScanMeta request guard** — captures User-Agent/Referer without accessing raw Request object
- **Configurable BASE_URL** — short URLs work in any deployment environment

### What's Next (Priority Order)

1. **Push CI workflow** — needs `workflow` scope on GitHub token, or manual push via web UI
2. **Rate limiting** — per-key rate limit exists in schema but isn't enforced
3. **README update** — update README to document tracked QR and style features
4. **Frontend** — React dashboard for human users
5. **PDF output format** — mentioned in README, not yet implemented
6. **Logo/image overlay** — embed a small logo in the center of QR codes (requires high EC)

### ⚠️ Gotchas

- `cargo` not on PATH by default — use `export PATH="$HOME/.cargo/bin:$PATH"` before building
- CI workflow push blocked — GitHub token lacks `workflow` scope
- Styles accepted but style column in DB is informational only (not used for re-rendering)
- CORS wide open (all origins) — tighten for production
- OpenAPI spec is now complete (v0.2.0) — 14 paths, 18 schemas
- BASE_URL defaults to `http://localhost:8000` — must be set in production for correct short URLs

### Architecture Notes

- Single-threaded SQLite via `Mutex<Connection>` — fine for moderate load, would need connection pooling for high concurrency
- Images stored as BLOBs in SQLite — works for small-to-medium volume, consider filesystem storage for high volume
- CORS wide open (all origins) — tighten for production
- Style rendering is pixel-level for PNG (no external drawing lib needed) and SVG-native for SVG output
- Redirect route mounted at root (`/`), API routes at `/api/v1` — clean separation
- ScanMeta uses Rocket's `FromRequest` trait for clean header extraction in redirect handler

---

*Last updated: 2026-02-07 08:55 UTC — Session: OpenAPI spec update (tracked QR, API keys, templates)*
