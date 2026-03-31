# External Research: protondb-lookup

## Executive Summary

ProtonDB exposes a live summary JSON endpoint keyed by Steam App ID that is sufficient for exact compatibility tier lookup and basic metadata such as confidence, score, and report volume. Richer report data is also publicly fetchable from the live site, but the path discovered on March 31, 2026 is not keyed directly by Steam App ID and appears to be an internal page-data artifact, so CrossHook should treat it as opportunistic rather than contractual. The integration should therefore be backend-owned, aggressively cached, and designed so exact rating lookup still works when the richer report feed changes or disappears.

### Candidate APIs and Services

#### ProtonDB Summary Endpoint

- Documentation URL: no published API documentation was found; endpoint verified live at `https://www.protondb.com/api/v1/reports/summaries/1245620.json` on March 31, 2026
- Auth model: none
- Key endpoints/capabilities:
  - `GET https://www.protondb.com/api/v1/reports/summaries/{steamAppId}.json`
  - Returns `tier`, `bestReportedTier`, `trendingTier`, `score`, `total`, and `confidence`
- Rate limits/quotas: no public limit document was found
- Pricing notes: free, unauthenticated public endpoint

#### ProtonDB Report Feed

- Documentation URL: none found; feed discovered from the live ProtonDB app page network trace
- Auth model: none
- Key endpoints/capabilities:
  - `GET https://www.protondb.com/data/reports/all-devices/app/996607738.json`
  - Returns `page`, `perPage`, `total`, and a `reports[]` array
  - Each report includes `responses.launchOptions`, `responses.concludingNotes`, `responses.protonVersion`, verdict flags, and device metadata
- Rate limits/quotas: no published limits found
- Pricing notes: free, unauthenticated public resource
- Implementation caveat: the discovered path uses an internal numeric identifier that does not match the Steam App ID and was only visible after loading `https://www.protondb.com/app/1245620` in a browser

#### ProtonDB Steam Proxy

- Documentation URL: no public docs; endpoint observed in the live page network log
- Auth model: none
- Key endpoints/capabilities:
  - `GET https://www.protondb.com/proxy/steam/api/appdetails/?appids={steamAppId}`
  - Returns Steam store metadata including canonical title and media
- Rate limits/quotas: not documented
- Pricing notes: free
- Relevance: useful background context for issue `#52`, but not required to satisfy issue `#53`

### Libraries and SDKs

- Rust/backend: `reqwest` with `rustls-tls` and `json` features is the best fit because CrossHook does not already have an HTTP client, the integration belongs in `crosshook-core`, and explicit timeout/error handling matters more than minimizing one small dependency.
- Rust/backend alternative: `ureq` would keep the API synchronous, but it offers less natural reuse with Tauri async commands and provides a thinner ergonomics story for retries, headers, and response handling.
- Frontend: no direct browser SDK should be used because ProtonDB responds with `Access-Control-Allow-Origin: https://www.protondb.com`, which will block direct webview fetches from CrossHook.

### Integration Patterns

- Use a backend-owned read-through cache keyed by Steam App ID.
- Cache summary and recommendation payloads separately so the stable rating path remains useful even if richer report parsing breaks.
- Treat all ProtonDB responses as untrusted remote input and normalize them into a compact internal DTO before storing them.
- Prefer on-demand fetch with stale-cache fallback over startup-wide eager sync, because the feature only matters for profiles that already have a Steam App ID.
- Expose a single thin Tauri IPC lookup command with an optional force-refresh flag rather than multiple frontend-visible network primitives.

### Constraints and Gotchas

- ProtonDB does not publish the summary/report API in official docs, so route stability must be assumed to be lower than a documented API.
- The summary endpoint is keyed by Steam App ID; the richer report feed discovered from the live page is not.
- CrossHook cannot rely on browser `fetch()` because ProtonDB’s CORS policy only allows `https://www.protondb.com`.
- The current summary response contains exact ProtonDB tiers such as `gold` and `platinum`, which do not map 1:1 onto CrossHook’s existing `CompatibilityRating` enum.
- Rate limits are undocumented, so a conservative cache TTL and manual refresh flow are safer than repeated background polling.

### Resolved Integration Decisions

- **Summary-first scope**: issue `#53` is explicitly summary-first; hidden report-feed parsing is best-effort and must degrade gracefully without blocking core lookup.
- **Tier contract**: ProtonDB uses a distinct exact-tier contract end to end; lossy mapping to `CompatibilityRating` is derived/internal only.
- **Persistence policy**: metadata storage keeps normalized ProtonDB payloads and does not persist raw report payloads as long-lived cache rows.
- **Cross-issue boundary**: issue `#52` may reuse Steam App ID linkage and source provenance from `#53`, but does not own or duplicate ProtonDB network-fetch logic.
