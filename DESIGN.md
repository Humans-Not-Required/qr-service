# QR Service — Design Document

> See also: [Shared Design Principles](../docs/design-principles.md)

## Overview

A QR code generation and decoding service. Stateless by default — no accounts, no API keys, no storage needed for basic operations.

## Auth Model: None (Stateless)

QR generation is a **utility** — it transforms input into an image. There is no reason to gate this behind authentication.

### Access Rules

| Operation | Auth Required | Rationale |
|-----------|--------------|-----------|
| Generate QR code | ❌ No | Stateless transformation |
| Decode QR code | ❌ No | Stateless transformation |
| Batch generate | ❌ No | Just multiple generations |
| Generate from template | ❌ No | Convenience wrapper |
| View QR via share URL | ❌ No | Public by design |
| Download QR image | ❌ No | Direct download |
| Create tracked QR | ⚡ Optional | Needs storage for analytics |
| View tracked QR stats | ⚡ Token | Scoped to the tracked QR |
| Delete tracked QR | ⚡ Token | Scoped to the tracked QR |

### Tracked QR Codes (Future/Optional)

Tracked QR codes provide scan analytics (how many scans, when, where). Since these require persistent storage:

- Creating a tracked QR returns a `{ id, manage_token, short_url, manage_url }`
- The `manage_token` grants access to stats and deletion
- The short URL (`/t/{uuid}`) redirects to the target and records the scan
- No global account needed — token is per-resource

## Share URL Strategy: Stateless, Self-Contained

The share URL for a basic QR code encodes everything in the URL itself:

```
/qr/view?data={base64_content}&size=300&color=000000&bg=ffffff&format=png
```

- **No database lookup needed** — the QR is regenerated on-the-fly from URL params
- **Works forever** — no expiry, no storage, no cleanup
- **Bookmarkable** — save it, share it, it always works
- The page renders a preview of the QR code with a download button

For tracked QR codes, use UUID-based short URLs: `/t/{uuid}`

### URL Length Consideration

QR content is typically short (URLs, text, vCards). Base64 encoding keeps URL length manageable. For very long content, the share URL may be impractical — but the API still returns the image directly, so the agent can just send the image file instead.

## User Flows

### AI Agent Flow
1. `POST /qr/generate` with `{ content: "https://example.com", size: 300 }`
2. Response: image bytes + `share_url`
3. Agent sends image directly to human, OR shares the URL

### Human Flow
1. Open the web UI
2. Paste/type content
3. Click "Generate"
4. See QR code, click "Download" or copy share URL

## API Changes Needed (from current state)

Current implementation requires `AuthenticatedKey` on every route. Changes needed:

1. **Remove auth from all generation/decode routes** — these are stateless utilities
2. **Remove the API key management system entirely** (or keep only for tracked QR)
3. **Add share URL generation** — return a `/qr/view?data=...` URL with every generation
4. **Add `/qr/view` GET endpoint** — renders QR from URL params with download UI
5. **Remove history tracking for basic QR** — stateless means no history (tracked QR is the history feature)

## Rate Limiting

Keep basic IP-based rate limiting to prevent abuse. No auth needed — just throttle by IP address. Generous limits (e.g., 100 requests/minute per IP).

## Python SDK

Zero-dependency Python client (`sdk/python/qr_service.py`). Covers all API endpoints: generate (PNG/SVG/PDF), decode, batch, templates (WiFi/vCard/URL), tracked QR CRUD, and discovery (llms.txt, OpenAPI, well-known skills). Typed error hierarchy. Convenience helpers for file save and byte extraction. 74 integration tests.
