# Game Details Modal — External API Research (Workstream 1)

## Executive Summary

The **Game Details Modal** feature (#143) is primarily a **read-only aggregation** of data CrossHook already derives locally (profiles, SQLite metadata, launch preview fields) plus **optional live enrichment** where the app already integrates with third-party HTTP endpoints.

- **No new third-party API is strictly required** to ship the modal: launch method, Steam App ID, Proton/prefix paths, trainer details, launch history, health snapshots, collections, and offline readiness are all **in-repo sources** (Tauri IPC / `crosshook-core`), not new external contracts.
- The **only non-Valve, game-compatibility–specific remote dependency** called out in the issue is **ProtonDB rating**. CrossHook **already consumes ProtonDB-hosted JSON** over HTTPS from `crosshook-core` (`protondb/client.rs`); there is **no separate, documented “ProtonDB public API”** published by the ProtonDB project—only URLs the website appears to use for its own UI.
- **Steam Store JSON** (`store.steampowered.com/api/appdetails`) is **already used** in `crosshook-core` for optional metadata (e.g. names, header images). Valve does **not** ship first-party developer documentation for this JSON surface; community-maintained notes exist (linked below).
- **SteamGridDB** (`www.steamgriddb.com/api/v2/...`) is **already integrated** for optional game imagery when the user supplies an API key. It is **not** listed in the issue’s required modal fields; treat as **optional polish**, not a dependency for the modal’s core read-only summary.

**Modal UI / accessibility:** the native frontend does **not** use `@radix-ui/react-dialog`. Existing modals (e.g. launch preview, onboarding) use a **custom portal + `role="dialog"` + focus-scope pattern** and shared `.crosshook-modal-*` styles. **Radix** in this repo is limited to **Tabs, Select, and Tooltip** (`package.json`).

---

### Candidate APIs and Services

### 1. ProtonDB (compatibility tier / community signals)

| Aspect                                      | Detail                                                                                                                                                                                                                                                                             |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Purpose in feature**                      | Show **ProtonDB tier**, confidence, report counts, trending/best-reported tiers, and (when available) **aggregated recommendation groups** derived from community reports.                                                                                                         |
| **Official API docs**                       | **None found.** ProtonDB does not publish a supported public API contract comparable to documented partner APIs.                                                                                                                                                                   |
| **Concrete URLs already used by CrossHook** | See `crosshook-core` `protondb/client.rs`:                                                                                                                                                                                                                                         |
|                                             | • Summary JSON: `https://www.protondb.com/api/v1/reports/summaries/{appId}.json` (example: `https://www.protondb.com/api/v1/reports/summaries/730.json`) — **camelCase** fields such as `tier`, `bestReportedTier`, `trendingTier`, `score`, `confidence`, `total` (report count). |
|                                             | • Global counts: `https://www.protondb.com/data/counts.json` — fields `reports`, `timestamp` used to build hashed report-feed paths.                                                                                                                                               |
|                                             | • Per-app report feed: `https://www.protondb.com/data/reports/all-devices/app/{derivedId}.json` where `{derivedId}` is computed from Steam App ID + counts payload (implementation detail in `client.rs`).                                                                         |
|                                             | • Human-readable source link pattern: `https://www.protondb.com/app/{appId}`.                                                                                                                                                                                                      |
| **Third-party mirrors**                     | Community-hosted APIs exist (e.g. `https://protondb.max-p.me/` — author notes it may be temporary). **CrossHook does not use these**; prefer **first-party `protondb.com` hosts** already wired in core.                                                                           |

### 2. Steam Storefront JSON — `appdetails` (optional metadata)

| Aspect                                 | Detail                                                                                                                                                                                                                                                                                  |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Purpose**                            | Enrich UI with **store name, short description, header image, genres**, etc., when a numeric Steam App ID is known.                                                                                                                                                                     |
| **Endpoint**                           | `https://store.steampowered.com/api/appdetails` (used in `steam_metadata/client.rs`).                                                                                                                                                                                                   |
| **Official Valve documentation**       | **Not published** as a stable developer API. Behavior is described in community references.                                                                                                                                                                                             |
| **Community reference (concrete URL)** | [Team Fortress Wiki — User:RJackson/StorefrontAPI — `appdetails`](https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI#appdetails) — documents query parameters such as `appids`, optional `filters`, and response shape caveats (e.g. multi-`appids` + filter restrictions). |
| **Feature fit**                        | **Optional** for #143: the issue lists “Steam App ID” as data, not store copy. Reuse existing lookup if the modal should show **title/art** beyond the library card.                                                                                                                    |

### 3. SteamGridDB API v2 (optional imagery)

| Aspect          | Detail                                                                                                 |
| --------------- | ------------------------------------------------------------------------------------------------------ |
| **Purpose**     | Fetch **grids/heroes** etc. when user configures a SteamGridDB API key (`game_images` stack).          |
| **Docs URL**    | [https://www.steamgriddb.com/api/v2](https://www.steamgriddb.com/api/v2) (official site; Bearer auth). |
| **Feature fit** | **Not required** for the enumerated modal fields in #143.                                              |

### 4. Other issue-listed data (no new external API)

| Data                                                             | Source (in-app)                                                                                                          |
| ---------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Launch method, Proton path/prefix, Steam launch options text     | Existing **launch preview / profile** resolution in `crosshook-core` + IPC (same family as `LaunchPanel` preview modal). |
| Trainer details                                                  | Profile + preview models (local).                                                                                        |
| Launch history, health snapshots, collections, offline readiness | **SQLite metadata** and related core modules; **no third-party HTTP** implied.                                           |

---

## Libraries and SDKs

### Backend (Rust / `crosshook-core`)

| Library                            | Role                                                                  | Notes                                                                                                 |
| ---------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| **reqwest**                        | Async HTTP client for ProtonDB, Steam `appdetails`, SteamGridDB, etc. | Shared patterns: timeouts, `User-Agent` (ProtonDB client sets `CrossHook/{version}`), JSON via Serde. |
| **serde** / **serde_json**         | Deserialize remote JSON; serialize cache payloads.                    | ProtonDB summary uses `#[serde(rename_all = "camelCase")]` with `total` → `total_reports`.            |
| **rusqlite** (via `MetadataStore`) | Cache rows in `external_cache_entries`.                               | ProtonDB namespace key: `protondb:{appId}` (`PROTONDB_CACHE_NAMESPACE` in `protondb/models.rs`).      |

### Desktop shell (Tauri v2)

| Piece                           | Role                                                                                                         |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| **`@tauri-apps/api`**           | IPC `invoke` for commands; no new external API.                                                              |
| **`@tauri-apps/plugin-dialog`** | File/folder dialogs for export flows (already used); relevant to **Export launcher** quick action, not HTTP. |

### Frontend (React 18)

| Piece                                                                                   | Role                                                                                                                                                                                                                          |
| --------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`@radix-ui/react-tabs`**, **`@radix-ui/react-select`**, **`@radix-ui/react-tooltip`** | **Not** used for modals today.                                                                                                                                                                                                |
| **Custom modal components**                                                             | e.g. `LaunchPanel` `PreviewModal`, `OnboardingWizard`, `ProfilePreviewModal` — **portal to `document.body`**, `.crosshook-modal-open` on `<body>`, `role="dialog"`, `aria-modal="true"`, `data-crosshook-focus-root="modal"`. |
| **`src/utils/clipboard.ts`**                                                            | Uses **`navigator.clipboard.writeText`** with fallback for **Copy Steam launch options** — Web API, not a remote service.                                                                                                     |

**Conclusion:** **No new modal/a11y npm package is required** for parity with existing CrossHook modals; follow established patterns and ensure new scroll regions register in `useScrollEnhance` per repo rules.

---

## Integration Patterns

1. **Reuse existing ProtonDB command path**  
   The profile editor already surfaces ProtonDB via backend lookup + IPC. The modal should **call the same command(s)** (or a thin aggregate command) rather than duplicating fetch logic in the frontend.

2. **Cache-aware UX**  
   ProtonDB results in core carry **cache metadata** (`fetched_at`, `expires_at`, stale/offline flags). The modal should **surface stale/offline states** consistently with `ProtonDbLookupCard` / launch page behavior.

3. **Degraded recommendations**  
   If the report feed fails but the summary succeeds, core already **degrades** to tier-only plus a synthetic recommendation group message (`fetch_live_lookup` in `client.rs`). The modal should **treat this as normal**, not an error.

4. **Steam metadata**  
   If the modal displays store-backed strings or images, use the **existing** `lookup_steam_metadata` flow and cache TTL behavior from `steam_metadata/client.rs` (24h TTL in core), not ad-hoc fetches from React.

5. **Quick actions**  
   **Launch**, **Edit profile**, **Export launcher**, and **Copy Steam launch options** map to **existing IPC + clipboard utilities**, not new HTTP integrations.

---

## Constraints and Gotchas

### ProtonDB

- **Undocumented contract:** JSON shapes and URLs may change without notice; CrossHook already guards with typed deserialization, timeouts (**6s**), and **404** handling.
- **Rate limiting / ToS:** No published quotas; rely on **conservative caching** (core uses **6-hour TTL** for ProtonDB cache rows), **single-flight** discipline at the UI layer, and avoid hammering refresh.
- **Report feed dependency on `counts.json`:** If `counts.json` is stale vs. CDN report files, core **retries** after refetching counts (`fetch_recommendations` in `client.rs`). Modal should not assume recommendations load in one round trip.
- **App ID validity:** Non-numeric or empty IDs short-circuit to empty/default lookup results.

### Steam `appdetails`

- **Undocumented and occasionally quirky** (e.g. multi-`appids` + `filters` interactions per community wiki). CrossHook requests a **single** `appids` value in its client—keep that pattern.
- **Regional / language parameters** (`cc`, `l`) may affect strings; core’s behavior should be reviewed before exposing store text in a new surface.

### SteamGridDB

- **Requires user API key**; without it, imagery paths should **gracefully omit** remote art.

### SQLite cache payload size

- `external_cache_entries` enforces a **512 KiB** payload cap (per `AGENTS.md`). ProtonDB serialized payloads should remain small; if they grow, core may need trimming—**out of scope for this research file** but a known platform constraint.

### Tauri / WebView

- Clipboard via `navigator.clipboard` may need **secure context**; CrossHook already implements a **fallback** in `clipboard.ts`—reuse for modal copy actions.

---

## Open Decisions

1. **Does the modal re-fetch ProtonDB on every open, or only when stale / user clicks refresh?** (Affects perceived snappiness and respect for implicit rate limits.)
2. **Should the modal show Steam store title/header image** via existing `appdetails` integration, or **strictly** mirror library card text? (Determines whether Steam Store JSON is user-visible in this feature.)
3. **Link-out policy:** Is a prominent **“View on ProtonDB”** (`https://www.protondb.com/app/{id}`) link sufficient for depth, vs. embedding more report narrative in-app?
4. **Offline-first copy:** When metadata DB is unavailable, which sections show **placeholders** vs. **hide** vs. **error**? (Issue marks runtime-only scope; still a UX decision.)
5. **SteamGridDB art in modal:** Explicitly **in** or **out** of v1 scope?

---

_Document produced for `docs/plans/game-details-modal/research-external.md`. Sources: `crosshook-core` ProtonDB/Steam metadata/game image clients, `package.json`, native modal components, and the URLs cited above._
