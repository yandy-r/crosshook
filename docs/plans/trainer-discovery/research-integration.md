# Integration Research: Trainer Discovery Phase B

## Overview

Phase A is fully implemented: `trainer_sources` table (schema v18), `discovery_search_trainers` sync IPC command, `useTrainerDiscovery` hook, and `TrainerDiscoveryPanel`. Phase B adds FLiNG RSS HTTP fetch, `external_cache_entries` caching, PCGamingWiki name normalization, async `discovery_search_external` IPC command, `useExternalTrainerSearch` hook, and progressive loading in the panel. All infrastructure (reqwest, SQLite cache, OnceLock HTTP client singleton, async Tauri command pattern) is already present — Phase B adds a new client module and frontend hook following existing patterns exactly.

---

## External APIs

### FLiNG RSS Feed

- **URL**: `https://flingtrainer.com/category/trainer/feed/`
- **Format**: WordPress RSS 2.0 — standard `application/rss+xml` response
- **Direct HTTP access**: Returns 403 when fetched without a real browser user-agent. Must use a proper `User-Agent` header (e.g. `CrossHook/{version}`) — the existing ProtonDB client already sets this via `reqwest::Client::builder().user_agent(...)`.
- **XML structure** (standard WordPress RSS 2.0):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:content="http://purl.org/rss/1.0/modules/content/"
     xmlns:dc="http://purl.org/dc/elements/1.1/">
  <channel>
    <title>FLiNG Trainer - PC Game Cheats and Mods</title>
    <link>https://flingtrainer.com</link>
    <description>...</description>
    <item>
      <title>Elden Ring Trainer</title>
      <link>https://flingtrainer.com/trainer/elden-ring-trainer/</link>
      <pubDate>Sat, 04 Apr 2026 00:00:00 +0000</pubDate>
      <description><![CDATA[Short summary...]]></description>
      <content:encoded><![CDATA[Full HTML content...]]></content:encoded>
      <dc:creator>FLiNG</dc:creator>
      <guid isPermaLink="true">https://flingtrainer.com/trainer/elden-ring-trainer/</guid>
      <category>Trainer</category>
    </item>
  </channel>
</rss>
```

- **Key fields per item**:
  - `<title>` — Game name + " Trainer" suffix (strip suffix to get normalized game name)
  - `<link>` — Trainer page URL (this is what CrossHook links to — not a direct download)
  - `<pubDate>` — RFC 822 date string; parse with `chrono` for recency scoring
  - `<description>` / `<content:encoded>` — HTML content; skip for Phase B (don't parse HTML)
  - `<guid>` — Same as `<link>` on this site; use as dedup key
- **Rate limit**: Self-imposed ≥10s between requests. The feed is cached for 1h TTL (see Cache Settings below), so rate limiting is handled implicitly — only one live fetch per cache window.
- **Parsing approach**: Use `quick-xml` crate (already available via transitive deps in `reqwest`) **or** add `quick-xml = "0.36"` to `crosshook-core/Cargo.toml`. No `scraper` crate needed — HTML body parsing is out of scope for Phase B.
- **Result shape**: Each RSS item maps to an `ExternalTrainerResult` — `game_name` (stripped title), `source_name: "FLiNG"`, `source_url` (item link), `pub_date` (parsed or raw string), `source: "fling_rss"`.
- **CrossHook policy**: Links to trainer PAGES only. Never links to direct download URLs. The RSS `<link>` field is exactly the trainer page — this is what `TrainerResultCard` should open via `shellOpen()`.

### PCGamingWiki Cargo API

- **URL pattern**: `https://www.pcgamingwiki.com/w/api.php?action=cargoquery&tables=Infobox_game&fields=Infobox_game.pageName%3Dpage_name%2CInfobox_game.Steam_AppID%3Dsteam_appid&where=Infobox_game.Steam_AppID+HOLDS+%22{appid}%22&format=json&limit=1`
  - Note: `_pageName` is invalid (underscore prefix rejected). Use `Infobox_game.pageName` aliased as `page_name`.
- **Use case**: Cross-reference only. Given a Steam App ID, resolve the canonical game title for fuzzy-matching against FLiNG RSS titles. NOT a trainer source.
- **Response structure** (successful query):

```json
{
  "cargoquery": [
    {
      "title": {
        "page_name": "Terraria",
        "steam_appid": "105600"
      }
    }
  ]
}
```

- **Response on no match**: `{"cargoquery": []}` — empty array.
- **Error response**: JSON with `"error"` key (e.g., invalid field alias returns `cargoquery-invalidfieldalias`).
- **Cache key**: `trainer:pcgw:game:{steam_app_id}` — cache for 24h (game titles rarely change).
- **Phase B scope**: Optional enrichment path. If the FLiNG RSS title does not fuzzy-match the profile's game name directly, query PCGamingWiki with the profile's Steam App ID to get the canonical title, then retry the fuzzy match. Skip if PCGamingWiki is unavailable — it is not a hard dependency.

---

## Database Schema

### `external_cache_entries` (existing — migration v4)

Used by ProtonDB, Steam metadata, and Phase B trainer discovery. Schema defined in `migrate_3_to_4` in `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:264-274`.

| Column         | Type             | Constraints | Notes                                                    |
| -------------- | ---------------- | ----------- | -------------------------------------------------------- |
| `cache_id`     | TEXT PRIMARY KEY | —           | UUID (via `db::new_id()`)                                |
| `source_url`   | TEXT NOT NULL    | —           | Origin URL of the fetched resource                       |
| `cache_key`    | TEXT NOT NULL    | UNIQUE      | Namespace-prefixed lookup key (see Cache Settings)       |
| `payload_json` | TEXT             | nullable    | JSON payload; NULL if payload exceeds 512 KiB            |
| `payload_size` | INTEGER NOT NULL | DEFAULT 0   | Byte count of the original payload (even if NULL-stored) |
| `fetched_at`   | TEXT NOT NULL    | —           | RFC3339 timestamp of the fetch                           |
| `expires_at`   | TEXT             | nullable    | RFC3339 TTL boundary; NULL means never expires           |
| `created_at`   | TEXT NOT NULL    | —           | RFC3339                                                  |
| `updated_at`   | TEXT NOT NULL    | —           | RFC3339                                                  |

- **Upsert semantics**: `put_cache_entry()` uses `ON CONFLICT(cache_key) DO UPDATE SET ...` — safe to call repeatedly; updates `updated_at`, `payload_json`, `payload_size`, `fetched_at`, `expires_at`.
- **Size cap**: `MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB) enforced in `cache_store.rs:37-46`. Oversized payloads are stored as NULL with a `tracing::warn!`. FLiNG RSS feed is expected to be well under this limit.
- **Public API**: `MetadataStore::get_cache_entry(cache_key)` and `MetadataStore::put_cache_entry(source_url, cache_key, payload, expires_at)` in `metadata/mod.rs:523-539`.
- **Implementation file**: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`

### `version_snapshots` (existing — migration v8→v9)

Schema defined in `migrate_8_to_9` in `migrations.rs:481-506`.

| Column              | Type          | Notes                                          |
| ------------------- | ------------- | ---------------------------------------------- | ------- | ------------ | --------------- | ------------ | ------- | ------------------- |
| `id`                | INTEGER PK    | Auto-increment                                 |
| `profile_id`        | TEXT NOT NULL | FK → `profiles(profile_id)` ON DELETE CASCADE  |
| `steam_app_id`      | TEXT NOT NULL | DEFAULT `''`                                   |
| `steam_build_id`    | TEXT          | Nullable; from `.acf` manifest                 |
| `trainer_version`   | TEXT          | Nullable; from community profile or manual set |
| `trainer_file_hash` | TEXT          | Nullable; SHA-256 of trainer executable        |
| `human_game_ver`    | TEXT          | Nullable; display label (e.g., "1.12.3")       |
| `status`            | TEXT NOT NULL | `untracked\|matched\|game_updated\|trainer_changed\|both_changed\|unknown\|update_in_progress` |
| `checked_at`        | TEXT NOT NULL | RFC3339                                        |

- **Max rows**: `MAX_VERSION_SNAPSHOTS_PER_PROFILE = 20` — older rows pruned in same transaction as each insert.
- **Indexes**: `idx_version_snapshots_profile_checked` on `(profile_id, checked_at DESC)` and `idx_version_snapshots_steam_app_id` on `(steam_app_id)`.
- **Key function**: `lookup_latest_version_snapshot(conn, profile_id)` in `version_store.rs:75-111` — returns `Option<VersionSnapshotRow>` with the most recent row.
- **Version compatibility check for Phase B**: Compare `VersionSnapshotRow.steam_build_id` (current game build) against the game version string from a FLiNG RSS item title or `ExternalTrainerResult.game_version`. This is a string equality check (or fuzzy match) — not a semver comparison. The pure comparison function `compute_correlation_status()` in `version_store.rs:185` handles build_id vs snapshot comparisons; adapt it for FLiNG-sourced version strings.
- **Implementation file**: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`

### `trainer_sources` (Phase A — existing — migration v17→v18)

Schema defined in `migrate_17_to_18` in `migrations.rs:782-811`. Added in Phase A; Phase B reads from this table but does not modify the schema.

| Column            | Type          | Constraints                                     | Notes                                      |
| ----------------- | ------------- | ----------------------------------------------- | ------------------------------------------ |
| `id`              | INTEGER PK    | AUTOINCREMENT                                   | —                                          |
| `tap_id`          | TEXT NOT NULL | FK → `community_taps(tap_id)` ON DELETE CASCADE | —                                          |
| `game_name`       | TEXT NOT NULL | —                                               | Searchable; matched against FLiNG titles   |
| `steam_app_id`    | INTEGER       | nullable                                        | Used for PCGamingWiki cross-reference      |
| `source_name`     | TEXT NOT NULL | —                                               | e.g., "FLiNG Trainer"                      |
| `source_url`      | TEXT NOT NULL | —                                               | Trainer page URL                           |
| `trainer_version` | TEXT          | nullable                                        | Optional version label                     |
| `game_version`    | TEXT          | nullable                                        | Optional game version this trainer targets |
| `notes`           | TEXT          | nullable                                        | Free-text notes                            |
| `sha256`          | TEXT          | nullable                                        | SHA-256 of the trainer binary (optional)   |
| `relative_path`   | TEXT NOT NULL | —                                               | Path within the tap workspace              |
| `created_at`      | TEXT NOT NULL | —                                               | RFC3339                                    |

- UNIQUE constraint on `(tap_id, relative_path, source_url)`.
- Indexes: `idx_trainer_sources_game` on `(game_name)` and `idx_trainer_sources_app_id` on `(steam_app_id)`.
- Phase B does not add new tables. External results from FLiNG RSS are returned as in-memory `ExternalTrainerResult` structs and cached in `external_cache_entries` as serialized JSON — they are NOT written to `trainer_sources`.

---

## IPC Layer

### Existing Sync Command (Phase A)

**`discovery_search_trainers`** — registered in `lib.rs:318`.

```rust
// src-tauri/src/commands/discovery.rs
#[tauri::command]
pub fn discovery_search_trainers(
    query: TrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<TrainerSearchResponse, String> { ... }
```

- Synchronous (`pub fn`, not `async fn`).
- Searches `trainer_sources` joined with `community_taps` via LIKE matching on `game_name`, `source_name`, `notes`.
- Input: `TrainerSearchQuery { query, compatibility_filter, platform_filter, limit, offset }`.
- Output: `TrainerSearchResponse { results: Vec<TrainerSearchResult>, total_count }`.
- Limit capped at 50 in `search.rs:35`.

### New Async Commands (Phase B)

**`discovery_search_external`** — new command to add.

```rust
// src-tauri/src/commands/discovery.rs (add here)
#[tauri::command]
pub async fn discovery_search_external(
    query: ExternalTrainerSearchQuery,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ExternalTrainerSearchResponse, String> {
    let metadata_store = metadata_store.inner().clone(); // required for async
    crosshook_core::discovery::search_external(&metadata_store, &query.game_name, query.steam_app_id.as_deref())
        .await
        .map_err(|e| e.to_string())
}
```

- Must be `async fn` — performs HTTP fetch.
- Must call `metadata_store.inner().clone()` before the first `.await` (same pattern as `protondb_lookup` in `commands/protondb.rs:55`).
- Input: `ExternalTrainerSearchQuery { game_name: String, steam_app_id: Option<String> }`.
- Output: `ExternalTrainerSearchResponse { results: Vec<ExternalTrainerResult>, source: String, cached: bool, cache_age_secs: Option<i64> }`.

**`discovery_check_version_compatibility`** — new command to add (optional Phase B).

```rust
#[tauri::command]
pub fn discovery_check_version_compatibility(
    profile_name: String,
    trainer_game_version: Option<String>,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<VersionMatchResult, String> { ... }
```

- Synchronous — reads from SQLite only.
- Looks up `version_snapshots` for the profile, compares against the provided `trainer_game_version` string.
- Returns existing `VersionMatchResult` type from `discovery/models.rs`.

### Command Registration

Both new commands must be added to `tauri::generate_handler![]` in `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` in the `// Trainer discovery` block (currently line 317-319):

```rust
// Trainer discovery
commands::discovery::discovery_search_trainers,
commands::discovery::discovery_search_external,        // Phase B
commands::discovery::discovery_check_version_compatibility, // Phase B
```

No new entries needed in `commands/mod.rs` — `discovery` module is already declared at line 5.

---

## Frontend Integration

### Existing Hook (Phase A)

**`useTrainerDiscovery`** — `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useTrainerDiscovery.ts`

- Wraps `discovery_search_trainers` (sync IPC, fast, local-only).
- Debounces query input at 300ms.
- Uses `requestIdRef.current` pattern for stale request cancellation.
- Returns `{ data: TrainerSearchResponse | null, loading, error, refresh }`.
- Phase B does NOT modify this hook — it remains the local-results source.

### New Hook (Phase B)

**`useExternalTrainerSearch`** — create at `src/crosshook-native/src/hooks/useExternalTrainerSearch.ts`.

Pattern to follow exactly from `useTrainerDiscovery.ts`:

```typescript
import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ExternalTrainerSearchResponse } from '../types/discovery';

export interface UseExternalTrainerSearchReturn {
  data: ExternalTrainerSearchResponse | null;
  loading: boolean;
  error: string | null;
  refresh: () => Promise<void>;
}

export function useExternalTrainerSearch(gameName: string, steamAppId?: string): UseExternalTrainerSearchReturn {
  // requestIdRef pattern for stale cancellation
  // invoke('discovery_search_external', { query: { gameName, steamAppId } })
  // No debounce needed — triggered manually or on game context change
}
```

- Returns `ExternalTrainerSearchResponse` with `results`, `source` (e.g., `"fling_rss"`), `cached`, `cacheAgeSecs`.
- Does NOT auto-fire on every keystroke — triggered by explicit user action ("Search Online") or once per game context load.
- Loading state is independent from `useTrainerDiscovery` loading state.

### Component Updates — TrainerDiscoveryPanel

**File**: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`

Progressive loading strategy:

1. Local results (from `useTrainerDiscovery`) render immediately as the user types.
2. External results (from `useExternalTrainerSearch`) load when the user clicks "Search Online" or after local results are displayed.
3. Local and external results are rendered in separate sections — local first, then external below with a "From FLiNG (external)" label.
4. External section shows a spinner while `useExternalTrainerSearch.loading` is true — does NOT block local results display.
5. External results use the same `TrainerResultCard` component — `result.sourceUrl` opens the FLiNG trainer page via `shellOpen()`.

New UI elements needed in `TrainerDiscoveryPanel`:

- "Search Online" button (only visible when `settings.discovery_enabled` and a query is entered).
- External results section with `crosshook-discovery-badge--external` badge on result cards.
- Cache age indicator (e.g., "Results from cache, 23 min ago") when `data.cached === true`.

### TypeScript Types (Phase B additions)

Add to `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/discovery.ts`:

```typescript
export interface ExternalTrainerSearchQuery {
  gameName: string;
  steamAppId?: string;
}

export interface ExternalTrainerResult {
  gameName: string;
  sourceName: string; // e.g., "FLiNG"
  sourceUrl: string; // Trainer page URL (never a direct download)
  pubDate?: string; // ISO8601 or raw RFC822 string
  source: string; // e.g., "fling_rss"
}

export interface ExternalTrainerSearchResponse {
  results: ExternalTrainerResult[];
  source: string; // e.g., "fling_rss"
  cached: boolean;
  cacheAgeSecs?: number;
}
```

---

## Configuration

### Cache Settings

| Key                     | Namespace                            | TTL      | Notes                                         |
| ----------------------- | ------------------------------------ | -------- | --------------------------------------------- |
| FLiNG RSS feed index    | `trainer:source:v1:fling_rss_index`  | 1 hour   | Single cached entry for the full feed         |
| Per-game FLiNG lookup   | `trainer:source:v1:{normalized_key}` | 6 hours  | Where `normalized_key` is from `normalize_game_slug()` |
| PCGamingWiki game title | `trainer:pcgw:game:{steam_app_id}`   | 24 hours | Game titles are stable; long TTL is safe      |

**Namespace format**: `trainer:source:v1:{normalized_game_key}` as specified in the task brief. The `v1` segment allows future namespace bumps without manual cache invalidation.

**Normalized game key**: Produced by `normalize_game_slug(name)` — lowercase, whitespace collapsed to hyphens (e.g., "Elden Ring" → `elden-ring`). See "Cache Key Normalization Helper" in Architectural Patterns below.

**TTL implementation**: Compute `expires_at` as RFC3339 string:

```rust
use chrono::{Duration as ChronoDuration, Utc};
let expires_at = (Utc::now() + ChronoDuration::hours(1)).to_rfc3339();
metadata_store.put_cache_entry(source_url, cache_key, &payload, Some(&expires_at))?;
```

### Rate Limiting

- Self-imposed 10s minimum between FLiNG RSS requests.
- The 1h cache TTL makes this implicit: a cache hit skips the HTTP fetch entirely.
- On cache miss: make one HTTP request, cache result, return. The rate limit is only relevant if cache is bypassed (e.g., `force_refresh=true`).
- Implement as a tokio `Mutex`-guarded `Instant` stored in a `OnceLock<Mutex<Instant>>` — or simply rely on the TTL cache and not implement an explicit rate limiter in Phase B.

---

## Dependencies

### reqwest (already present)

`crosshook-core/Cargo.toml` line 22:

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

This is sufficient for Phase B HTTP fetches. The FLiNG RSS response body is text/XML, not JSON. Use `response.text().await?` instead of `.json()`. The `rustls-tls` feature handles HTTPS.

### XML Parsing

`quick-xml` is NOT currently in `crosshook-core/Cargo.toml`. Add it for RSS parsing:

```toml
quick-xml = { version = "0.36", features = ["serialize"] }
```

Alternative approach: parse the RSS `<title>` and `<link>` fields with simple string extraction using the existing `serde` + string operations if the feed structure is predictable. However, `quick-xml` is the correct, robust choice.

`scraper` is NOT needed. Phase B does not parse HTML content from trainer pages.

### serde, chrono, uuid (already present)

All present in `crosshook-core/Cargo.toml`:

- `serde = { version = "1", features = ["derive"] }` — for `ExternalTrainerResult` serialization
- `chrono = "0.4"` — for `pubDate` parsing and `expires_at` TTL calculation
- `uuid = { version = "1", features = ["v4", "serde"] }` — for `cache_id` generation via `db::new_id()`

---

## Relevant Files (Phase B touch points)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/discovery/mod.rs` — Add `pub mod client;` and re-export `search_external`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/discovery/models.rs` — Add `ExternalTrainerResult`, `ExternalTrainerSearchResponse`, `ExternalTrainerSearchQuery` structs
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — Reference implementation for `OnceLock` HTTP client, 3-stage cache pattern, `put_cache_entry` persistence
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `get_cache_entry`, `put_cache_entry`, `evict_expired_cache_entries`; **add `get_cache_entry_allow_stale()` for Phase B stale fallback**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs` — `lookup_latest_version_snapshot`, `compute_correlation_status`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml` — Add `quick-xml`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/discovery.rs` — Add `discovery_search_external`, `discovery_check_version_compatibility`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — Register new commands in `generate_handler![]`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useTrainerDiscovery.ts` — No changes; Phase A hook remains
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/discovery.ts` — Add Phase B TypeScript types
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx` — Progressive loading, external results section, "Search Online" button

---

## Architectural Patterns (Phase B)

### HTTP Client Singleton (copy from ProtonDB)

```rust
// discovery/client.rs
use std::sync::OnceLock;
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 10; // FLiNG may be slower than ProtonDB
static FLING_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn fling_http_client() -> Result<&'static reqwest::Client, TrainerDiscoveryError> {
    if let Some(client) = FLING_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(TrainerDiscoveryError::Network)?;
    let _ = FLING_HTTP_CLIENT.set(client);
    Ok(FLING_HTTP_CLIENT.get().expect("initialized"))
}
```

### 3-Stage Cache Pattern (copy from ProtonDB)

```
1. get_cache_entry(cache_key) → return immediately if valid (not expired)
2. HTTP fetch from FLiNG RSS → parse XML → build ExternalTrainerSearchResponse
3. put_cache_entry(source_url, cache_key, payload, expires_at) → persist
4. On HTTP error: stale fallback (see below)
5. On total failure: return ExternalTrainerSearchResponse { results: [], source: "fling_rss", cached: false }
```

**Stale cache fallback design decision**: The public `MetadataStore::get_cache_entry(key)` always filters `expires_at > now` — it cannot return stale (expired) rows. Two options for the stale fallback path:

- **(a) Direct `with_sqlite_conn` query** — bypass the public API and query `external_cache_entries` directly inside a `metadata_store.with_sqlite_conn(|conn| { ... })` closure, the same approach used in `protondb/client.rs:346-394`. Requires raw SQL in the caller.
- **(b) Add `get_cache_entry_allow_stale()` public method** (**preferred**) — add to `cache_store.rs` a function that omits the `expires_at > now` filter and returns a `CachedEntryRow { payload_json, fetched_at, expires_at }`. This keeps the raw SQL in `cache_store.rs` where it belongs, and the returned `fetched_at` field enables computing `cache_age_secs` for the `ExternalTrainerSearchResponse` without a second query.

Option (b) is cleaner: the raw SQL stays in `cache_store.rs`, the caller receives a typed struct, and `fetched_at` is available for the cache age indicator in the frontend.

### Cache Key Normalization Helper

The cache key for per-game lookups must be produced by a named helper function — never inlined — so the key is identical on every cache write and cache read path.

Add to `discovery/client.rs` (or a shared `discovery/models.rs` location):

```rust
pub fn normalize_game_slug(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}
```

Usage:

```rust
let cache_key = format!("trainer:source:v1:{}", normalize_game_slug(&game_name));
```

This mirrors the pattern of `cache_key_for_app_id()` in `protondb/models.rs`, which centralizes ProtonDB key construction. Phase B must follow the same convention.

### Async Tauri Command Pattern (copy from ProtonDB)

```rust
// CRITICAL: call .inner().clone() before the first .await
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String> {
    let metadata_store = metadata_store.inner().clone(); // <-- this
    Ok(lookup_protondb(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}
```

Without `.inner().clone()`, the `State<>` reference cannot be held across await points.

---

## Gotchas and Edge Cases

- **FLiNG RSS title parsing**: Titles follow the pattern `"{Game Name} Trainer"`. Strip `Trainer` suffix (case-insensitive) to get the game name. Some titles may include version info: `"Elden Ring v1.12 Trainer"` — strip from the last `v` or `Trainer` occurrence.
- **403 on FLiNG RSS without User-Agent**: The feed blocks requests without a real User-Agent. The existing ProtonDB HTTP client already sets `CrossHook/{version}` — FLiNG HTTP client must do the same.
- **PCGamingWiki field alias restriction**: `_pageName` is rejected (underscore prefix not allowed as alias). Use `Infobox_game.pageName` with alias `page_name` (no leading underscore). The query string must URL-encode field aliases: `fields=Infobox_game.pageName%3Dpage_name`.
- **PCGamingWiki `HOLDS` operator**: Steam App IDs are stored as a multi-value field. Use `WHERE Infobox_game.Steam_AppID HOLDS "{appid}"` (not `=`) to handle games with multiple App IDs.
- **Mutex not across await**: `MetadataStore` uses `Arc<Mutex<Connection>>`. Never hold the mutex lock across an `.await`. The `with_sqlite_conn` method acquires and releases the lock synchronously per call — this is safe. Any pre-fetch cache read must complete (releasing the lock) before the async HTTP fetch begins.
- **Oversized RSS payload → NULL cache**: If the FLiNG RSS response exceeds 512 KiB (`MAX_CACHE_PAYLOAD_BYTES`), `put_cache_entry()` stores NULL and logs a warning. Subsequent `get_cache_entry()` returns `None` for a NULL payload (see `cache_store.rs:26`: `Ok(row.flatten())`). This means a very large RSS feed would cause a cache miss on every request despite an entry existing. Mitigation: truncate to the top N items before serializing to JSON.
- **Stale cache not accessible via public API**: `MetadataStore::get_cache_entry()` filters `expires_at > now` and returns `None` for expired rows. The stale fallback path CANNOT use this method — it must either call `with_sqlite_conn` directly (ProtonDB approach) or use the new `get_cache_entry_allow_stale()` method (preferred). Do not add `allow_expired: bool` to the existing `get_cache_entry()` signature — add a separate method.
- **Cache key reproducibility**: The cache key MUST be produced by `normalize_game_slug()` on every read and write path. Never inline the normalization logic. A mismatch between the write-path slug and the read-path slug causes a permanent cache miss, forcing a live HTTP fetch on every call.
- **Schema version**: The DB is at v18 after Phase A. Phase B adds NO new migrations — it reuses `external_cache_entries` (v4) and `version_snapshots` (v9).
- **`quick-xml` not in Cargo.toml**: Must be added before Phase B can compile. No other missing dependencies.
- **FLiNG RSS `<link>` vs `<guid>`**: On FLiNG's WordPress site, `<link>` and `<guid isPermaLink="true">` are the same URL. Use `<link>` as the canonical `source_url`. Do not use any URL from `<description>` or `<content:encoded>` — those contain HTML with relative or direct download links that CrossHook must not surface.

---

## Relevant Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/feature-spec.md` — Full Phase B feature specification and acceptance criteria
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-architecture.md` — System architecture and component boundaries
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-patterns.md` — Code patterns and conventions
- [PCGamingWiki Cargo API docs](https://www.pcgamingwiki.com/wiki/PCGamingWiki:API) — MediaWiki Cargo extension query syntax
- [WordPress RSS 2.0 feed format](https://wordpress.com/support/feeds/) — Standard feed structure reference
- [quick-xml crate docs](https://docs.rs/quick-xml/latest/quick_xml/) — Rust XML parsing
