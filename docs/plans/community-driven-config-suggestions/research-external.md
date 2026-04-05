# External APIs, Libraries, and Integration Patterns

## ML-Assisted Configuration — Research

---

## Executive Summary

> **Revision note**: This document was updated after cross-referencing the actual `crosshook-core` source. The initial draft assumed more work remained than is the case. Significant infrastructure is already implemented.

ProtonDB has no official public API. CrossHook already consumes **three** undocumented ProtonDB endpoints in `crosshook-core/src/protondb/`:

1. **Summary** — `https://www.protondb.com/api/v1/reports/summaries/{app_id}.json` (tier/score)
2. **Counts** — `https://www.protondb.com/data/counts.json` (used to derive the report feed hash)
3. **Report feed** — `https://www.protondb.com/data/reports/all-devices/app/{hash}.json` (per-game individual reports including `launchOptions` and `concludingNotes`)

The HTTP client (`OnceLock<reqwest::Client>`, 6s timeout, `CrossHook/{version}` UA), caching (`external_cache_entries` SQLite table, 6h TTL, stale-on-error fallback), and launch option extraction/aggregation (`aggregation.rs`: env var parsing, frequency grouping, copy-only launch string separation, injection-safe validation) are all **already implemented**. The `ProtonDbRecommendationGroup` / `ProtonDbEnvVarSuggestion` / `ProtonDbLaunchOptionSuggestion` model types from `models.rs` are the normalized output already flowing to the frontend.

For the ML-assisted configuration feature, **no new HTTP client, cache primitive, or regex extraction layer needs to be built**. The work is in extending `aggregation.rs` with improved grouping/ranking logic and surfacing the existing `recommendation_groups` data through the IPC and UI layers.

PCGamingWiki exposes a Cargo-backed MediaWiki API with no authentication, useful for supplemental Linux/Proton metadata. The Steam Store API provides game metadata at ~200 req/5 min free of charge, no key required.

**Confidence**: High — findings confirmed by reading `client.rs`, `models.rs`, and `aggregation.rs` directly.

---

## Primary APIs

### 1. ProtonDB

**Documentation**: No official docs. Community-documented via:

- Data dumps repo: [github.com/bdefore/protondb-data](https://github.com/bdefore/protondb-data)
- Community API: [github.com/Trsnaqe/protondb-community-api](https://github.com/Trsnaqe/protondb-community-api)
- Community API (self-hosted): [github.com/maxpoulin64/protondb-api](https://github.com/maxpoulin64/protondb-api)

**Authentication**: None required for live endpoints or data dump downloads.

**Pricing**: Free. Data released under ODbL (Open Database License).

**Rate Limits**: No documented rate limits on the summary endpoint. Community convention is to avoid aggressive polling; data changes at most monthly with dump releases.

#### Endpoint 1: Per-Game Summary

```
GET https://www.protondb.com/api/v1/reports/summaries/{app_id}.json
```

**Already implemented** in `client.rs:SUMMARY_URL_BASE` via `fetch_summary()`. Deserializes into `ProtonDbSummaryResponse` with fields: `tier`, `bestReportedTier`, `trendingTier`, `score`, `confidence`, `total` (renamed `totalReports`). No authentication. Undocumented but stable — used by browser extensions, Decky plugins, and CLI tools across the ecosystem.

#### Endpoint 2: Global Report Counts (Hash Seed)

```
GET https://www.protondb.com/data/counts.json
```

**Already implemented** in `client.rs:COUNTS_URL` via `fetch_counts_json()`. Returns `{ "reports": i64, "timestamp": i64 }`. The `reports` count and `timestamp` are fed into `report_feed_id()` to derive the hash for Endpoint 3. This is a **fully undocumented** quirk — the hash formula is a custom Java-style string hash over a composed format string (see `report_feed_id`, `compose_hash_part`, `hash_text` in `client.rs:444–469`). No external documentation for this pattern exists; it was reverse-engineered.

The client retries hash resolution once on 404 (re-fetching counts) before returning `HashResolutionFailed`.

#### Endpoint 3: Per-Game Report Feed

```
GET https://www.protondb.com/data/reports/all-devices/app/{hash}.json
```

**Already implemented** in `client.rs:REPORTS_URL_BASE` via `fetch_recommendations()`. The `{hash}` is computed from `report_feed_id(app_id, reports_count, counts_timestamp, page_selector=1)`. Returns a JSON object with a `reports` array. Each entry deserializes into `ProtonDbReportEntry` (in `aggregation.rs`):

```rust
// Actual structs from aggregation.rs
struct ProtonDbReportEntry {
    id: String,
    timestamp: i64,
    responses: ProtonDbReportResponses,
}

struct ProtonDbReportResponses {  // camelCase from JSON
    concluding_notes: String,     // "concludingNotes"
    launch_options: String,       // "launchOptions"
    proton_version: String,       // "protonVersion"
    variant: String,
    notes: ProtonDbReportNotes,
}
```

This endpoint provides the `launchOptions` and `concludingNotes` freeform text fields that are the primary input for the suggestion engine. `launchOptions` is a string in `KEY=VALUE %command%` format (when present); `concludingNotes` is freeform community text.

#### Data Dumps: Full Historical Corpus (Not Currently Used)

- Published monthly at [github.com/bdefore/protondb-data](https://github.com/bdefore/protondb-data)
- Format: gzipped JSON (`reports_piiremoved.json`) — cumulative, not incremental
- License: ODbL — attribution, share-alike on adapted databases, keep open
- CrossHook currently does **not** use the dumps — it fetches the live report feed per game on demand
- Dumps are relevant only if bulk/offline corpus analysis is pursued (ML Phase 2+)

**Known dump schema issues** (not relevant to the live feed path CrossHook uses):

- `result` field removed from dumps since December 2019
- Boolean fields encoded inconsistently (0/1, "yes"/"no", bool)
- Schema changed significantly after October 28, 2019 questionnaire update

**Confidence**: High — confirmed by reading the actual implementation in `client.rs` and `aggregation.rs`.

---

### 2. PCGamingWiki

**Documentation**: [pcgamingwiki.com/wiki/PCGamingWiki:API](https://www.pcgamingwiki.com/wiki/PCGamingWiki:API)

**Authentication**: None required for read queries.

**Rate Limits**: Not explicitly documented. MediaWiki API standard — no known hard limit for reasonable read usage.

**Pricing**: Free.

**Base URL**: `https://www.pcgamingwiki.com/w/api.php`

#### Steam AppID Redirect (Simple Lookup)

```
GET https://pcgamingwiki.com/api/appid.php?appid={steamAppId}
```

Redirects to the game's wiki article page. Useful for discovering the `pageID` to use in Cargo queries.

#### Cargo Query (Structured Data)

```
GET https://www.pcgamingwiki.com/w/api.php
  ?action=cargoquery
  &tables=Infobox_game
  &fields=Infobox_game._pageName=Page,Infobox_game.Developers,Infobox_game.Released
  &where=Infobox_game.Steam_AppID HOLDS "{appId}"
  &format=json
```

The Cargo extension exposes structured data from PCGamingWiki's game articles. Available tables include `Infobox_game`, `Video`, `Sound`, `Input`, and others. The `_pageName` and `_pageID` columns require aliasing in the `fields` parameter (e.g., `Infobox_game._pageName=Page`).

#### Wikitext Retrieval (Raw Content)

```
GET https://www.pcgamingwiki.com/w/api.php
  ?action=parse
  &format=json
  &pageid={pageId}
  &prop=wikitext
```

Returns the raw MediaWiki markup for a game article, from which Linux/Proton compatibility sections, launch option recommendations, and workaround notes can be extracted with regex or wikitext parsing.

**Value for CrossHook**: PCGamingWiki's game articles often contain documented launch options and known Proton workarounds that supplement ProtonDB community reports. The Cargo API is more stable for structured data; wikitext extraction is more fragile but richer.

**Confidence**: High for API availability and structure; Medium for consistent presence of Linux/Proton data across all games.

---

### 3. Steam Store API (App Details)

**Documentation**: Unofficial community documentation at [steamapi.xpaw.me](https://steamapi.xpaw.me/); official Steamworks: [partner.steamgames.com/doc/webapi_overview](https://partner.steamgames.com/doc/webapi_overview)

**Authentication**: Not required for the store appdetails endpoint. Some Steamworks endpoints require a publisher API key.

**Rate Limits**:

- ~200 successful requests per 5 minutes on the store appdetails endpoint
- Recommended delay: 600ms between requests
- HTTP 429 returned when rate-limited; HTTP 403 requires 5-minute backoff

**Pricing**: Free.

#### App Details Endpoint

```
GET https://store.steampowered.com/api/appdetails?appids={appId}
```

Returns: game name, description, genres, categories, screenshots, price, supported platforms, release date, PC requirements. Does **not** include launch options or Proton compatibility data — that lives in ProtonDB.

**Value for CrossHook**: Provides game name, cover image URL, genres, and categories. Useful for enriching cached game metadata alongside ProtonDB compatibility data.

**Confidence**: High — well-documented by community, stable for years.

---

## Libraries and SDKs

### Rust Crates

#### Already in `crosshook-core` (confirmed by source)

| Crate                  | Purpose            | How Used                                                                    |
| ---------------------- | ------------------ | --------------------------------------------------------------------------- |
| `reqwest`              | Async HTTP client  | `OnceLock<reqwest::Client>` singleton in `client.rs`; 6s timeout, custom UA |
| `serde` + `serde_json` | JSON parsing       | All ProtonDB response structs; `#[serde(default)]` throughout               |
| `tokio`                | Async runtime      | Drives all `async fn` in the ProtonDB client                                |
| `rusqlite`             | SQLite bindings    | `MetadataStore` / `external_cache_entries` cache layer                      |
| `chrono`               | Timestamps / TTL   | RFC3339 timestamps for `fetched_at`, `expires_at` in cache entries          |
| `tracing`              | Structured logging | Warn-level logging on network/cache failures                                |

**No new HTTP, caching, or JSON dependencies are needed for Phase 1.**

#### Not Present — Needed Only for ML Path (Phase 2+)

| Crate        | Version | Purpose              | Notes                                                                                  |
| ------------ | ------- | -------------------- | -------------------------------------------------------------------------------------- |
| `gllm`       | latest  | Pure Rust embeddings | 60+ models, GPU via WGPU, no C deps — preferred over `rust-bert` for AppImage          |
| `rust-bert`  | latest  | Transformer NLP      | BERT/sentence embeddings; requires LibTorch ~1GB — **avoid for AppImage distribution** |
| `clustering` | latest  | K-means clustering   | For grouping similar launch configurations by embedding distance                       |

#### Explicitly Not Needed

- `reqwest-middleware` / `http-cache-reqwest` — CrossHook uses application-level SQLite caching, not HTTP-level caching
- `regex` — `aggregation.rs::safe_env_var_suggestions` already tokenizes env vars by splitting on whitespace and `=` without regex; the `is_safe_env_key` / `is_safe_env_value` validators use character-level checks

---

## Integration Patterns

### How CrossHook Already Consumes ProtonDB Data (Confirmed by Source)

CrossHook uses a **live-feed-per-game** pattern, not data dumps:

1. `lookup_protondb(metadata_store, app_id, force_refresh)` in `client.rs` is the single entry point
2. Checks `external_cache_entries` for a valid (non-expired) cached `ProtonDbLookupResult` by key `"protondb:{app_id}"`
3. On cache miss: calls `fetch_live_lookup()` which fetches all three endpoints in sequence
4. Falls back to stale cache on any network error, returning `ProtonDbLookupState::Stale`
5. Returns `ProtonDbLookupState::Unavailable` only when no cached data exists and network fails
6. Cache TTL is **6 hours** (not 24h); key format is `PROTONDB_CACHE_NAMESPACE + ":" + app_id`

### How `aggregation.rs` Extracts and Groups Launch Options (Confirmed by Source)

`normalize_report_feed()` processes the report feed in a single pass:

1. **Env-var-extractable reports** (`safe_env_var_suggestions` finds at least one `KEY=VALUE` token before `%command%`):
   - Groups by _env var signature_ (sorted `KEY=VALUE\n` joined string)
   - Counts supporting reports per group
   - Also collects raw launch strings that can't be parsed into env vars (`launch_string_needs_copy_only`) as copy-only fallbacks
   - Collects `concludingNotes` into per-group advisory notes

2. **Copy-only launch strings** (non-empty `launchOptions` with no parseable env vars):
   - Grouped by exact raw string
   - Presented as `ProtonDbLaunchOptionSuggestion` without decomposed env vars

3. **Notes-only reports** (no `launchOptions`, non-empty `concludingNotes`):
   - Deduplicated by text, frequency-sorted
   - Emitted as a single `"community-notes"` group

Output caps: `MAX_ENV_GROUPS=3`, `MAX_LAUNCH_GROUPS=3`, `MAX_NOTE_GROUPS=4`, `MAX_GROUP_NOTES=3`.

**Security validation already implemented** in `safe_env_var_suggestions`:

- `is_safe_env_key`: key must start with `[A-Z_]`, contain only `[A-Z0-9_]`
- `is_safe_env_value`: rejects null bytes, whitespace, shell metacharacters (`$;"\'\`|&<>()%`)
- Blocklist: `WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, and any `STEAM_COMPAT_*` prefix

**Known env var families already handled by this parser**:

- `PROTON_*` — Proton runtime flags (`PROTON_NO_ESYNC=1`, `PROTON_USE_WINED3D=1`, etc.)
- `WINE_*` — Wine flags (`WINE_FULLSCREEN_FSR=1`, `WINE_FULLSCREEN_FSR_STRENGTH=2`, etc.)
- `DXVK_*` — DXVK renderer flags
- `VKD3D_*` — VKD3D-Proton flags
- `MANGOHUD=1` — Performance overlay
- `__GL_*`, `__NV_*`, `RADV_*`, `ANV_*` — GPU driver flags

### How Other Projects Consume ProtonDB Data (External Reference)

**Community tools (for context only — not relevant to CrossHook's architecture):**

- protondb-cli (Rust): live summary endpoint only, no report feed
- ProtonDB Badges (Decky plugin): live summary endpoint only
- Trsnaqe community API: data dump ingestion into MongoDB; terminated April 2023
- maxpoulin64 community API: data dump ingestion into MySQL; abandoned

CrossHook's pattern — live report feed per game, SQLite cache, local aggregation — is architecturally superior to any of these for a desktop app: no external service dependency, full offline fallback, richer data than summary-only.

### ML-Based Extension Path (Phase 2+)

The existing `aggregation.rs` groups reports by env-var signature. An ML extension would replace or augment the grouping step:

1. **Embedding + clustering**: Encode full `launchOptions` strings with sentence embeddings (`gllm`), cluster with K-means, extract cluster centroids as representative suggestions — useful for catching semantically similar but syntactically different option sets
2. **LLM extraction from `concludingNotes`**: Use a local model (via `langextract-rust` + Ollama) to extract structured env vars from freeform text — covers buried option mentions not in the `launchOptions` field

Phase 2 is out of scope for initial implementation. The existing whitespace-tokenizing parser in `safe_env_var_suggestions` already handles the structured case with high precision and zero ML overhead.

---

## Constraints and Gotchas

### ProtonDB

1. **No official API or SLA.** All three endpoints are undocumented and could change or be removed without notice. The hash derivation for the report feed (`report_feed_id`) is the highest-risk coupling: if ProtonDB changes the hash formula, `fetch_recommendations` will return 404s on every game. The retry-on-404 in `fetch_recommendations` mitigates transient hash staleness but not a formula change.

2. **Report feed hash is fragile.** The hash is derived from `counts.json` which changes with every new report submitted globally. Requests must always fetch a fresh `counts.json` before constructing the report feed URL — this adds a sequential HTTP dependency on every non-cached lookup.

3. **`launchOptions` is sparse.** Many reports include no launch options at all. `safe_env_var_suggestions` returning an empty `Vec` is the normal path for many games, not an error. The `degraded_recommendation_group` message handles the no-data case gracefully.

4. **Output caps are conservative.** `MAX_ENV_GROUPS=3`, `MAX_LAUNCH_GROUPS=3` may need tuning for games with many distinct configuration patterns. These are constants in `aggregation.rs` — easy to adjust but currently not configurable at runtime.

5. **ODbL share-alike for dump-based features.** CrossHook currently uses the live feed (no dump ingestion) — no ODbL obligation applies to the current implementation. If bulk offline corpus analysis is added (ML Phase 2+), shipping the processed results would trigger ODbL share-alike.

### PCGamingWiki

1. **Wikitext parsing is fragile.** Templates and formatting change. Prefer Cargo queries for structured data over wikitext extraction where possible.
2. **Not all games have Linux/Proton sections.** Coverage is community-driven and uneven.
3. **Rate limits unspecified.** Treat as a supplemental source, not primary. Cache aggressively.

### Steam Store API

1. **200 req/5 min limit.** At batch scale this is a bottleneck. CrossHook should fetch game metadata lazily (on first access per game) and cache for 7+ days.
2. **No launch options or Proton data.** This API is metadata-only.
3. **Terms of Use**: [steamcommunity.com/dev/apiterms](https://steamcommunity.com/dev/apiterms) — Valve may terminate access. Use conservatively; cache responses.

### General

1. **Offline behavior**: ProtonDB dumps and all fetched data must be cached in SQLite before network availability is required. The 24h TTL on `external_cache_entries` should use a staleness indicator pattern, not a hard block on access.
2. **Privacy**: ProtonDB dump filenames include `_piiremoved` — PII stripping is done upstream. No additional scrubbing needed for CrossHook's use case.

---

## Code Examples

All examples below reference actual types and functions from `crosshook-core`. Nothing here is speculative.

### Entry Point: Looking Up ProtonDB Data for a Game

```rust
// crosshook-core/src/protondb/client.rs — already implemented
// Called from Tauri command handlers in src-tauri/src/commands/protondb.rs

use crosshook_core::protondb::lookup_protondb;
use crosshook_core::metadata::MetadataStore;

async fn example(metadata_store: &MetadataStore, app_id: &str) {
    let result = lookup_protondb(metadata_store, app_id, false).await;
    // result.state: ProtonDbLookupState (Idle | Loading | Ready | Stale | Unavailable)
    // result.snapshot: Option<ProtonDbSnapshot> — tier + recommendation_groups
    // result.cache: Option<ProtonDbCacheState> — fetched_at, expires_at, is_stale, is_offline
}
```

### Accessing Extracted Suggestions from the Snapshot

```rust
// Types from crosshook-core/src/protondb/models.rs

use crosshook_core::protondb::models::{ProtonDbSnapshot, ProtonDbRecommendationGroup};

fn display_suggestions(snapshot: &ProtonDbSnapshot) {
    for group in &snapshot.recommendation_groups {
        // group.group_id: "supported-env-1", "copy-only-launch-1", "community-notes", etc.
        // group.title: human-readable
        // group.summary: "Seen in N ProtonDB reports."

        for env_var in &group.env_vars {
            // env_var.key: "PROTON_NO_ESYNC", "WINE_FULLSCREEN_FSR", etc.
            // env_var.value: "1", etc.
            // env_var.supporting_report_count: Some(N)
            // env_var.source_label: "Proton 9.0-1", "Custom Proton: GE-Proton9-8", etc.
        }

        for launch_opt in &group.launch_options {
            // launch_opt.text: raw launch string (copy-only, not decomposed)
            // launch_opt.supporting_report_count: Some(N)
        }

        for note in &group.notes {
            // note.text: freeform community text from concludingNotes
            // note.source_label: "Proton 9.0-1" or "2 reports"
        }
    }
}
```

### Report Feed Hash Derivation (Reverse-Engineered, Already Implemented)

```rust
// crosshook-core/src/protondb/client.rs:444–469 — the undocumented hash formula

fn report_feed_id(app_id: i64, reports_count: i64, counts_timestamp: i64, page_selector: i64) -> i64 {
    hash_text(format!(
        "p{}*vRT{}",
        compose_hash_part(app_id, reports_count, counts_timestamp),
        compose_hash_part(page_selector, app_id, counts_timestamp)
    ))
}

fn compose_hash_part(left: i64, right: i64, modulus: i64) -> String {
    format!("{right}p{}", left * (right % modulus))
}

fn hash_text(value: String) -> i64 {
    value.chars().chain(std::iter::once('m'))
        .fold(0_i32, |acc, ch| acc.wrapping_mul(31).wrapping_add(ch as i32))
        .unsigned_abs() as i64
}
// Report feed URL: https://www.protondb.com/data/reports/all-devices/app/{hash}.json
```

### Env Var Safety Validation (Already Implemented in `aggregation.rs`)

```rust
// crosshook-core/src/protondb/aggregation.rs:300–322 — already in production

fn is_safe_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(ch) if ch == '_' || ch.is_ascii_uppercase() => {}
        _ => return false,
    }
    chars.all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

fn is_safe_env_value(value: &str) -> bool {
    if value.contains('\0') { return false; }
    !value.chars().any(|ch| {
        ch.is_whitespace()
            || matches!(ch, '$' | ';' | '"' | '\'' | '\\' | '`' | '|' | '&' | '<' | '>' | '(' | ')' | '%')
    })
}
```

### Cache Key Convention (Already Established)

```rust
// crosshook-core/src/protondb/models.rs:9–22
pub const PROTONDB_CACHE_NAMESPACE: &str = "protondb";

pub fn cache_key_for_app_id(app_id: &str) -> String {
    format!("{PROTONDB_CACHE_NAMESPACE}:{}", app_id.trim())
    // e.g. "protondb:1245620"
}

// Cache TTL: CACHE_TTL_HOURS = 6 (client.rs:23)
// Column schema: cache_key, payload_json, fetched_at, expires_at (RFC3339 strings)
// Valid read: WHERE cache_key = ?1 AND (expires_at IS NULL OR expires_at > ?2)
// Stale read: WHERE cache_key = ?1  (no expiry check — allows offline fallback)
```

---

## Open Questions

1. **Should `MAX_ENV_GROUPS` / `MAX_LAUNCH_GROUPS` be runtime-configurable?** Currently hardcoded constants in `aggregation.rs`. Games with rich report histories (e.g., popular titles like Elden Ring) may have more meaningful distinct configurations than the cap allows.

2. **Is the report feed hash formula stable enough to depend on?** The reverse-engineered formula in `report_feed_id` has no upstream documentation. A monitoring/alerting mechanism (e.g., log a warning when 404s exceed a threshold across games) would help detect breakage early.

3. **Should PCGamingWiki be integrated as a supplemental source?** The Cargo API could provide curated workaround notes per game, cross-referencing `Steam_AppID`. This would augment `community-notes` groups with higher-quality editorial content. Not needed for Phase 1 but a viable Phase 1.5 addition.

4. **What threshold of supporting reports warrants surfacing an env-var suggestion?** Currently all groups from `normalize_report_feed` are returned regardless of `count`. A minimum threshold (e.g., ≥2 supporting reports) would filter noise from single-reporter edge cases.

5. **Is there a Tauri-safe approach for ONNX/LibTorch for the ML path?** `rust-bert` requires LibTorch (~1GB). For AppImage distribution, `gllm` (pure Rust, no C deps, WGPU acceleration) is the viable alternative. Neither is in scope until Phase 2+.

---

## Sources

- [ProtonDB](https://www.protondb.com/)
- [ProtonDB data dumps — bdefore/protondb-data](https://github.com/bdefore/protondb-data)
- [ProtonDB Community API — Trsnaqe/protondb-community-api](https://github.com/Trsnaqe/protondb-community-api)
- [ProtonDB Community API (maxpoulin64)](https://github.com/maxpoulin64/protondb-api)
- [ProtonDB Community API hosted instance](https://protondb.max-p.me/)
- [PCGamingWiki API documentation](https://www.pcgamingwiki.com/wiki/PCGamingWiki:API)
- [PCGamingWiki API GitHub](https://github.com/PCGamingWiki/api)
- [Steam Web API Overview — Steamworks](https://partner.steamgames.com/doc/webapi_overview)
- [Steam Web API Terms of Use](https://steamcommunity.com/dev/apiterms)
- [ODbL License — Open Data Commons](https://opendatacommons.org/licenses/odbl/)
- [reqwest crate](https://docs.rs/reqwest/)
- [reqwest GitHub](https://github.com/seanmonstar/reqwest)
- [http-cache-reqwest crate](https://crates.io/crates/http-cache-reqwest)
- [serde field attributes](https://serde.rs/field-attrs.html)
- [rust-bert — guillaume-be/rust-bert](https://github.com/guillaume-be/rust-bert)
- [gsdmm-rust — rwalk/gsdmm-rust](https://github.com/rwalk/gsdmm-rust)
- [clustering crate](https://crates.io/crates/clustering)
- [protondb-cli (Rust)](https://github.com/hypeedev/protondb-cli)
- [protondb-data-analyzer](https://github.com/hellsworth/protondb-data-analyzer)
- [Proton Launch Parameter Wizard](https://drraccoony.github.io/protonLaunParam/)
- [SQLite cache TTL pattern — DEV Community](https://dev.to/sjdonado/building-a-fast-and-compact-sqlite-cache-store-2h9g)
