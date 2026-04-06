# Documentation Research: Trainer Discovery Phase B

## Overview

CrossHook has deep, well-structured documentation covering architecture, developer guidelines, and feature context. The existing trainer-discovery research corpus (`docs/plans/trainer-discovery/`) is comprehensive and production-ready. Key implementation constraints are encoded in `AGENTS.md` and `CLAUDE.md` — both are required reading. The authoritative spec is `feature-spec.md` — it supersedes earlier decisions in individual research files. This document is Phase B-focused and extends the prior general research with Phase B–specific details on HTTP clients, external APIs, cache patterns, security, and UX.

---

## Architecture Documentation

### AGENTS.md (Project-Wide Architecture Rules)

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — **REQUIRED reading**

Key conventions governing Phase B implementation:

- **Business logic lives in `crosshook-core`** — `src-tauri` is a thin IPC adapter; no business logic in command files.
- **Tauri IPC**: `#[tauri::command]` handlers use `snake_case` names matching frontend `invoke()` calls. All IPC-crossing types require `Serde` derives.
- **Rust conventions**: `snake_case`, modules as directories with `mod.rs`, errors via `Result` with `anyhow` or project error types.
- **React/TypeScript**: `PascalCase` components, `camelCase` hooks/functions, strict TS, `invoke()` wrapped in hooks.
- **Scroll containers (CRITICAL)**: Any new `overflow-y: auto` container **must** be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts`, or dual-scroll jank occurs in WebKitGTK.
- **Verification**: After Rust changes run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.
- **Persistence classification**: Every new datum must be classified as TOML settings, SQLite metadata, or runtime-only before writing code.
- **Commits**: Internal docs use `docs(internal): …`; user-facing changes use conventional commits.

**Directory map (Phase B relevant)**:

```
src/crosshook-native/
├── src-tauri/src/commands/   # add commands/discovery.rs
├── crates/crosshook-core/src/
│   ├── discovery/            # NEW Phase B: client.rs, matching.rs
│   ├── metadata/             # cache_store.rs, version_store.rs, migrations.rs
│   ├── protondb/             # HTTP client reference: OnceLock pattern, three-stage fetch
│   └── install/              # tokenize() / token_hits() for matching.rs
└── src/
    ├── hooks/                # useTrainerDiscovery.ts (Phase A), extend for Phase B
    └── components/           # TrainerDiscoveryPanel.tsx — progressive loading in Phase B
```

**SQLite state**: `user_version` is **17** in-tree. Phase A adds `migrate_17_to_18` (`trainer_sources`). Phase B uses `external_cache_entries` (already exists since v4). Phase C adds `migrate_18_to_19` (FTS5 virtual table — requires `bundled-full`).

---

## Feature Specification

### Phase B Section

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/feature-spec.md`, lines 602–616

**Focus**: Optional external trainer source queries with cache and degraded offline mode.
**Dependencies**: Phase A complete.

**Tasks**:

- Create `discovery/client.rs` following ProtonDB client pattern (`OnceLock` HTTP client, cache-first fetch)
- Create `discovery/matching.rs` (token scoring from `install/discovery.rs` pattern, advisory version comparison)
- Add `discovery_search_external` async IPC command
- Add `discovery_check_version_compatibility` on-demand IPC command
- Integrate external results into `TrainerDiscoveryPanel` with progressive loading
- Source trust indicators (Community vs External badges)
- Offline degraded mode with persistent banner
- Unit tests for client, cache, matching, aggregation

### External Dependencies (Phase B)

**FLiNG Trainer** (feature-spec.md lines 23–30):

- Site: `https://flingtrainer.com/`
- Authentication: None
- Access: RSS feed at `https://flingtrainer.com/category/trainer/feed/`
- Rate limits: Self-imposed; recommend ≥10s between requests
- Note: No public API; CrossHook links to trainer pages, not file host URLs

**PCGamingWiki Cargo API** (cross-reference only, not trainer sources):

- `GET /w/api.php?action=cargoquery&tables=Infobox_game&where=Steam_AppID HOLDS "{appid}"&format=json`
- Rate limits: Undocumented; cache aggressively

**IGDB API**: Deferred — requires OAuth infrastructure not present in crosshook-core (no token storage or refresh infrastructure exists).

**Libraries** (all already in Cargo.toml for Phase B):

- `reqwest` 0.12+ with `json` + `rustls-tls` features
- `rusqlite` 0.39
- `serde` / `serde_json` 1.x
- `sha2` 0.11
- `tokio` 1.x
- `scraper` 0.26+ — **Phase B only, if RSS unavailable** (only new dependency)

### IPC Commands (Phase B)

**`discovery_search_external`** (async, follows `commands/protondb.rs` pattern):

```typescript
// Frontend invoke — fires after user clicks "Search Online"
invoke<TrainerSearchResponse>('discovery_search_external', {
  query: 'Elden Ring',
  forceRefresh: false,
});
```

```rust
// Rust signature
#[tauri::command]
pub async fn discovery_search_external(
    query: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<TrainerSearchResponse, String>
```

**`discovery_check_version_compatibility`** (on-demand per expanded card):

```typescript
invoke<VersionMatchResult>('discovery_check_version_compatibility', {
  communityProfileId: 42,
  profileName: 'my-elden-ring',
});
```

Loads community profile `game_version`, looks up user's `version_snapshots` for the profile, computes advisory match status. Pure function pattern from `compute_correlation_status()` in `version_store.rs`.

### Decisions Section (Phase B)

**Decision 3: FLiNG RSS only** (feature-spec.md line 640):

> FLiNG RSS as the sole Phase B external source. PCGamingWiki for cross-reference metadata only (game name normalization, not trainer sources). No WeMod API (ToS risk), no CheatHappens (subscription-gated), no IGDB (requires OAuth infrastructure).

**Decision 2: Legal opt-in** (feature-spec.md line 638):

> Discovery is opt-in (`discovery_enabled = false`). First enable shows a consent dialog. This applies to Phase B external search as well.

### Recommendations Phasing (feature-spec.md lines 506–508)

> Phase B estimate: ~1–2 weeks. Tasks: `discovery/client.rs` (`OnceLock<reqwest::Client>`), `external_cache_entries` with `trainer:source:v1:{key}` namespace, multi-source aggregation (FLiNG RSS per Decision 3 + optional PCGW normalization), token scoring from `install/discovery.rs`, async IPC. Rollback: disable external commands; cache rows expire via TTL.

---

## Relevant Files

### Prior Research (Trainer-Discovery Corpus)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/feature-spec.md` — **REQUIRED: Authoritative merged feature spec**. Final decisions, exact data models, IPC API signatures, file-change manifest, phase task breakdown, SQL DDL.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/shared.md` — **REQUIRED: Canonical file-path and pattern reference**. Lists every relevant file path, table, pattern reference, and critical constraint. Most actionable single doc.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-external.md` — **REQUIRED for Phase B**: FLiNG RSS endpoint, HTTP client code patterns, cache key conventions, three-stage fetch implementation. All Phase B network code derives from this.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-practices.md` — **REQUIRED for Phase B**: Resolved design decisions (FTS5 unavailable, IPC command split rationale, `tokenize()` lifting plan, testability patterns), reusable code inventory with absolute file paths.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-security.md` — Security: S3 cache poisoning, S5 URL rendering, S9 SHA-256 integration, S10 trust indicators (all Phase B relevant).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-ux.md` — Phase B UX: trust badge design, version badge two-stage render, offline banner patterns, progressive loading, component reuse list.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-recommendations.md` — Resolved trade-offs, risk assessment, Phase B task breakdown with effort estimates.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-technical.md` — Technical depth: component diagram, Phase B async IPC request/response sketches, cache key namespace.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-business.md` — User stories (US-1 through US-8), business rules (BR-1 through BR-11), edge case table.

### Architecture & Agent Guidelines

- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — **REQUIRED reading**. Architecture rules, Tauri IPC conventions, scroll container rules, SQLite metadata DB state, persistence classification table.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md` — Agent rule precedence, MUST/MUST NOT constraints, quick command reference.

### Reference Implementation Files (Key Code)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — **PRIMARY reference for `discovery/client.rs`**: `OnceLock<reqwest::Client>` singleton, three-stage cache→live→stale fetch (lines 85–130), 6s timeout, CrossHook User-Agent.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `get_cache_entry()` / `put_cache_entry()` / `evict_expired_cache_entries()` — the only cache pattern to use.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs` — `compute_correlation_status()` and `upsert_version_snapshot()` — reference for `discovery_check_version_compatibility`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/discovery.rs` — `tokenize()`, `token_hits()`, `score_candidate()` — lift to `text_utils.rs` for `matching.rs`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/taps.rs` — `validate_tap_url()` (line 485), `slugify()` (line 533), `is_valid_git_sha()`, git env isolation.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs` — **Reference for async IPC handler**: `.inner().clone()` before `await`, `map_err(|e| e.to_string())`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbSuggestions.ts` — **Reference for `useTrainerDiscovery.ts`**: `requestIdRef` race guard, `loading`/`error` state.

---

## Research Artifacts

### shared.md — Canonical Source of Truth

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/shared.md`

This is the **single most actionable reference file** for implementation. Key Phase B excerpts:

**Phase B files to create** (shared.md line 20):

- `crosshook-core/src/discovery/client.rs` — HTTP client following `protondb/client.rs` reference
- `crosshook-core/src/discovery/matching.rs` — version comparison, token scoring

**Phase B tables** (shared.md lines 44–51):

- `external_cache_entries` — keys `trainer:source:v1:{normalized_game_key}`; TTL-based; existing since v4
- `version_snapshots` — used by `discovery_check_version_compatibility` via `version_store.rs`
- `trainer_hash_cache` — launch-time verification; discovery does NOT substitute this

**Critical constraints** (shared.md lines 87–97):

- Phase A/B search: LIKE on `trainer_sources` until Phase C enables FTS5
- `discovery_search_trainers` stays sync; external calls are separate async commands
- `MetadataStore::disabled()` must return empty results, no panic
- Trainer versions are not semver — display strings as provided
- Import via existing `community_import_profile` only — no parallel import mechanism

**Pattern references** (shared.md lines 53–65):

- Cache-First Fetch: `protondb/client.rs:85–130`
- IPC Contract Tests: Mandatory `#[cfg(test)]` function-pointer assertions per command file
- Domain Module Layout: Mirror `protondb/` — `discovery/mod.rs`, `models.rs`, `search.rs`, `client.rs` (Phase B)

### research-practices.md — Resolved Design Decisions

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-practices.md`

**Resolved decisions relevant to Phase B** (lines 162–176):

1. `tokenize()` sharing — Lift `tokenize`/`token_hits` to `crosshook-core/src/text_utils.rs` (new module) in same effort as Phase B ranking; re-export from `install/discovery.rs` via `use`. Acceptance: single implementation, unit tests in `text_utils` + one install regression test.

2. IPC command split — Two separate commands: `discovery_search_trainers` (sync SQLite) and `discovery_search_external` (async HTTP). Phase B is additive; Phase A command remains unchanged.

3. FTS5 is NOT available — `rusqlite` uses `features = ["bundled"]` only. Any FTS5 SQL would silently fail at runtime. LIKE is the only correct search approach for Phase B.

4. `matchesQuery` pattern — Duplicate from `CommunityBrowser.tsx` into the Discovery component; do not abstract prematurely (only two use-sites).

5. Testability — Pure functions (`score_trainer_sources_for_game`, `matching.rs`) tested with `#[test]` unit tests, no I/O. Cache store tested with `MetadataStore::open_in_memory()`. HTTP fetch layer: use `wiremock`/`httpmock` or compile-time trait object in tests — do not make real HTTP calls.

**FTS5 Confirmation** (lines 182–187):

> `rusqlite` in `Cargo.toml` uses `features = ["bundled"]` only — no `bundled-full`, no FTS5. The tech spec initially proposed FTS5 SQL. This would silently fail at runtime. LIKE-based alternative is the only correct Phase 1/B search approach. FTS5 as a Phase 2/C enhancement requires an explicit `rusqlite` feature flag change.

---

## Security Documentation

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-security.md`

### Phase B Relevant Security Findings

**S3 (WARNING): Cache poisoning via `external_cache_entries`**

Existing defenses:

- `rustls-tls` provides strong TLS validation; `MAX_CACHE_PAYLOAD_BYTES = 524_288` caps payload
- `evict_expired_cache_entries` handles TTL-based eviction

Required Phase B mitigations:

1. Apply short TTLs (6–24 hours) for trainer source metadata
2. Validate `Content-Type` header is `application/json` before parsing — reject HTML captive portal responses
3. Apply response size limit at HTTP layer before reading full body (abort above 1 MB)

**S9 (ADVISORY): SHA-256 integration chain**

Implementation path for Phase B:

1. Store `sha256` from `trainer-sources.json` in `trainer_sources.sha256`
2. Surface in discovery UI as "Verified by community" label (collapsible raw hash via `CollapsibleSection`)
3. After user downloads trainer and imports as profile: existing `verify_and_cache_trainer_hash()` handles on-disk verification
4. No new verification at discovery layer — hash is informational only at Phase B

Reuse: `offline/hash.rs::normalize_sha256_hex()` for canonicalizing SHA-256 strings from tap metadata.

**S10 (ADVISORY): Trust indicators for Phase B external results**

Two-tier model — Community (tap) vs External:

- Community tap result: filled badge, accent color, label "Community"
- External result: chain-link icon, muted, no label
- Display tooltip on hover explaining indicators
- Never block link opening based on trust level — informational only
- Reserve modal-level warnings for non-https URLs only (alert fatigue if overused)

**S5 (WARNING): URL rendering in WebKitGTK**

Phase B introduces external URLs not validated by tap indexing:

```tsx
// CORRECT
import { open } from '@tauri-apps/plugin-shell';
function TrainerSourceLink({ url }: { url: string }) {
  const isHttps = url.startsWith('https://');
  if (!isHttps) return <span>Invalid source URL</span>;
  return <button onClick={() => open(url)}>Open Download Page</button>;
}
// NEVER use <a href={url}> or dangerouslySetInnerHTML
```

**S1 (WARNING): DMCA §1201 legal risk**

Phase B increases exposure by adding live external source queries. Hard requirements remain:

- Link-only architecture — CrossHook never fetches or caches trainer binaries
- `discovery_enabled = false` default with consent dialog on first enable
- No auto-download or auto-execution in any Phase B path

### Phase B Secure Coding Patterns

```rust
// URL validation before caching external results
let validated_url = validate_trainer_source_url(&raw_url)
    .map_err(|e| { tracing::warn!("skipping invalid trainer source URL: {e}"); })?;

// Content-type check on external fetch (new requirement for Phase B)
let resp = client.get(&url).send().await?;
let content_type = resp.headers()
    .get("content-type")
    .and_then(|v| v.to_str().ok())
    .unwrap_or("");
if !content_type.contains("application/json") && !content_type.contains("text/xml") {
    tracing::warn!("unexpected content-type from external source: {}", content_type);
    return Err(TrainerDiscoveryError::InvalidResponse);
}

// IPC error messages must NOT expose internal paths or SQL
// map_err(|e| e.to_string()) at command boundary — ensure Display impls are clean
```

---

## UX Documentation

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-ux.md`

### Progressive Loading Pattern (Phase B)

Phase B adds external results on top of Phase A tap results. The two-stage loading model is:

```
1. Panel opens → tap results from SQLite render immediately (no spinner)
2. Network check → offline: show tap-only results with persistent offline banner
                 → online: "Search Online" button appears (enabled)
3. User clicks "Search Online" → external query fires → external results merge below tap results
4. Per-card version check fires on expand → badge updates from "Checking…" to status
```

This mirrors the ProtonDB card pattern: user controls when network is hit.

### Trust Badges (New in Phase B)

Two-tier trust model from `research-ux.md` (lines 160–168):

| Source type                  | Visual                     | Label         | CTA behavior                             |
| ---------------------------- | -------------------------- | ------------- | ---------------------------------------- |
| Community tap result         | Filled badge, accent color | "Community"   | No confirmation needed                   |
| External / unverified result | Chain-link icon, muted     | _(icon only)_ | Optional confirmation for non-https URLs |

### Offline Banner (Phase B External Unavailable)

When external search fails (not tap search — tap search is always available offline):

- Persistent inline banner above results, `--unavailable` modifier
- Message: `"Online search unavailable. Showing local results only."`
- "Retry" button

Reuse: `crosshook-protondb-card__banner` with `--unavailable` modifier.

### Version Compatibility Badges (Phase B On-Demand via `discovery_check_version_compatibility`)

Two-stage render per result card:

1. Render card immediately (no wait for version check)
2. Fire `discovery_check_version_compatibility` on card expand
3. Show gray `"Checking…"` placeholder while in-flight
4. Replace with colored badge on resolution

| Backend status    | Badge label        | Color token                   |
| ----------------- | ------------------ | ----------------------------- |
| `exact`           | "Exact match"      | `--crosshook-color-success`   |
| `compatible`      | "Compatible"       | `--crosshook-color-success`   |
| `newer_available` | "Update available" | `--crosshook-color-warning`   |
| `outdated`        | "Outdated"         | `--crosshook-color-danger`    |
| `unknown`         | "Unknown"          | `--crosshook-offline-unknown` |

Always pair color with text label (WCAG requirement — never rely on color alone).

### Existing UI Components to Reuse in Phase B

- `crosshook-protondb-tier-badge` color tokens — reuse for compatibility rating badges
- `crosshook-protondb-card__banner` with `--neutral`, `--stale`, `--loading`, `--unavailable` modifiers
- `crosshook-skeleton` class with `--crosshook-skeleton-duration: 1.8s` variable
- `CollapsibleSection` component for SHA-256 and expanded detail section
- `formatRelativeTime()` utility for stale cache timestamps
- `useProtonDbSuggestions.ts` — `requestIdRef` race guard pattern for `useTrainerDiscovery.ts`

---

## API Documentation

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-external.md`

### FLiNG RSS (Primary Phase B External Source)

**Decision 3** makes FLiNG RSS the sole Phase B external source.

```
GET https://flingtrainer.com/category/trainer/feed/
```

- Returns XML with latest trainer posts
- Title format: `"Game Name v{version} (+{N} Trainer)"`
- Parse: `game_name` (normalized), `trainer_version`, `trainer_page_url`, `option_count`
- Cache key: `"trainer:source:v1:fling_index"` with **1h TTL**
- On fetch failure: fall back to expired cache row (same pattern as `protondb/client.rs`)
- Store trainer **page** URL, not file host URL (FLiNG download links point to OneDrive/Google Drive which expire)

**Status**: RSS availability inferred from XenForo standard behavior — **needs live verification** before depending on it.

**HTML fallback** (if RSS unavailable): `GET https://flingtrainer.com/category/trainer/` — parse with `scraper` crate.

### PCGamingWiki (Cross-Reference / Name Normalization Only)

```
GET https://pcgamingwiki.com/api/appid.php?appid={steam_appid}
```

Returns 302 redirect to wiki article page. Use for game name normalization only.

### HTTP Client Pattern for Phase B

From `research-external.md` — do NOT share ProtonDB or Steam metadata clients:

```rust
// In crosshook-core/src/discovery/client.rs
use std::sync::OnceLock;
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 6;
static TRAINER_DISCOVERY_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn trainer_discovery_http_client() -> Result<&'static reqwest::Client, TrainerDiscoveryError> {
    if let Some(client) = TRAINER_DISCOVERY_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(TrainerDiscoveryError::Network)?;
    let _ = TRAINER_DISCOVERY_HTTP_CLIENT.set(client);
    Ok(TRAINER_DISCOVERY_HTTP_CLIENT.get().expect("trainer discovery HTTP client initialized"))
}
```

### Cache Key Convention (from `research-external.md`)

```rust
const TRAINER_DISCOVERY_CACHE_NAMESPACE: &str = "trainer:source:v1";

fn cache_key_for_fling_index() -> String {
    format!("{TRAINER_DISCOVERY_CACHE_NAMESPACE}:fling_index")
}
fn cache_key_for_game(normalized_game_name: &str) -> String {
    format!("{TRAINER_DISCOVERY_CACHE_NAMESPACE}:{normalized_game_name}")
}
// Cache TTLs: FLiNG RSS feed index = 1 hour; individual page scrapes = 24 hours
```

### Three-Stage Cache-First Pattern (Required for All Phase B Fetches)

Every external fetch must implement this pattern (from `protondb/client.rs:85–130`):

```
1. get_cache_entry(allow_expired=false)   → return if fresh hit
2. fetch_live_*()                          → attempt network
3. get_cache_entry(allow_expired=true)    → return stale on network failure
```

### Existing Infrastructure (Do Not Re-Implement)

| Need                    | Existing                                                    | File                                             |
| ----------------------- | ----------------------------------------------------------- | ------------------------------------------------ |
| Steam version detection | `parse_manifest_full()`                                     | `steam/manifest.rs`                              |
| Steam app metadata      | `lookup_steam_metadata()`                                   | `steam_metadata/client.rs`                       |
| Cache read/write        | `get_cache_entry()` / `put_cache_entry()`                   | `metadata/cache_store.rs`                        |
| Hash verification       | `verify_and_cache_trainer_hash()`, `normalize_sha256_hex()` | `offline/hash.rs`                                |
| URL validation          | `validate_tap_url()`                                        | `community/taps.rs:485`                          |
| Token scoring           | `tokenize()`, `token_hits()`                                | `install/discovery.rs` (lift to `text_utils.rs`) |

---

## Code Documentation

### protondb/client.rs — Phase B HTTP Client Reference

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`

Key constants to mirror in `discovery/client.rs`:

- `REQUEST_TIMEOUT_SECS: u64 = 6`
- `CACHE_TTL_HOURS: i64 = 6` (adjust for trainer data — 1h for RSS, 24h for page scrapes)
- `static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client>` → create separate `TRAINER_DISCOVERY_HTTP_CLIENT`

Three-stage fetch pattern at lines 85–130.

### metadata/cache_store.rs — Phase B Cache Read/Write

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`

Phase B usage:

```rust
pub fn get_cache_entry(conn: &Connection, cache_key: &str) -> Result<Option<String>, MetadataStoreError>
pub fn put_cache_entry(conn: &Connection, source_url: &str, cache_key: &str, payload: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError>
pub fn evict_expired_cache_entries(conn: &Connection) -> Result<usize, MetadataStoreError>
```

Note: `MAX_CACHE_PAYLOAD_BYTES = 524_288` — when exceeded, `payload_json` stores as `NULL`. FLiNG RSS payloads are well under this limit. `get_cache_entry` returns `None` when `payload_json` is NULL — callers must handle this.

### metadata/version_store.rs — Phase B Version Matching Reference

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`

`compute_correlation_status()` — pure function comparing game build ID vs snapshot. Advisory use in `discovery_check_version_compatibility`. Note: this function is build-ID centric; `discovery/matching.rs` needs new advisory version comparison logic — do not directly reuse this function for trainer version strings.

### install/discovery.rs — Token Scoring Reference for Phase B matching.rs

**Location**: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`

`tokenize()`, `token_hits()`, `score_candidate()` — these functions should be lifted to a new `crosshook-core/src/text_utils.rs` module (resolved decision in `research-practices.md`) and re-exported from `install/discovery.rs` via `use`. Do not duplicate the implementation.

---

## Must-Read Documents

### Required Before Writing Phase B Code

1. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/shared.md`
   - File paths, table list, pattern references, critical constraints. Most actionable single doc.

2. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/feature-spec.md`
   - Authoritative Phase B task list (lines 602–616), Decision 3 (FLiNG only), IPC command signatures, SQL DDL for `trainer_sources`.

3. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-external.md`
   - FLiNG RSS endpoint details, HTTP client code pattern, cache key conventions, three-stage fetch implementation. All Phase B network code derives from this.

4. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-practices.md`
   - Resolved design decisions: FTS5 unavailable, IPC command split, `tokenize()` lifting, testability patterns.

5. `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md`
   - Architecture rules, scroll container requirement, IPC conventions, commit format.

### Required for Phase B Security Compliance

6. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-security.md`
   - S3 cache poisoning mitigation, S5 URL rendering rules, S9 SHA-256 integration, S10 trust indicators.

### Required for Phase B UI

7. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-ux.md`
   - Trust badge design, version badge two-stage render, offline banner, progressive loading, existing component reuse.

### Nice-to-Have

8. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-recommendations.md`
   - Risk assessment, resolved trade-offs, Phase B task breakdown with effort estimates.

9. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-technical.md`
   - Technical depth: component diagram, Phase B async IPC sketches, cache key namespace alignment.

---

## Architectural Patterns

- **Business logic separation**: All discovery logic in `crosshook-core/src/discovery/`. `src-tauri/src/commands/discovery.rs` is a thin adapter (~20-50 lines). No business logic in command handlers.
- **IPC naming**: `snake_case` (e.g., `discovery_search_external`). Frontend `invoke()` must match exactly. All IPC-crossing types need `#[derive(Serialize, Deserialize)]`.
- **Module structure**: New modules follow `crosshook-core/src/{domain}/mod.rs` + focused subfiles. Mirror `protondb/` layout.
- **Cache pattern**: All external data flows through `metadata/cache_store.rs` on `external_cache_entries`. Never build a bespoke cache.
- **HTTP client singleton**: Separate `TRAINER_DISCOVERY_HTTP_CLIENT: OnceLock<reqwest::Client>` — do not share ProtonDB or Steam metadata clients.
- **React hooks wrap IPC**: `useTrainerDiscovery.ts` mirrors `useProtonDbSuggestions.ts`. Request-ID race guard (`requestIdRef`) is the cancellation pattern.
- **Scroll containers**: Register new scrollable containers in `useScrollEnhance.ts` `SCROLLABLE` selector. Inner containers use `overscroll-behavior: contain`.
- **Persistence classification** (mandatory): TOML settings = user preferences, SQLite metadata = operational/cache/history, in-memory = ephemeral UI state.

---

## Gotchas & Edge Cases

- **FTS5 is NOT available in Phase B**: `rusqlite` uses `features = ["bundled"]` only. FTS5 SQL silently fails at runtime. LIKE is the only correct search approach for Phase A and B.
- **Trainer versions are not semver**: Real trainer versions look like "v1.0 +DLC", "Build 12345", "2024.12.05". The `semver` crate rejects these. Use advisory text (display the community-provided string as-is).
- **FLiNG RSS not verified live**: The RSS endpoint is inferred from XenForo standard behavior. Verify `https://flingtrainer.com/category/trainer/feed/` returns valid XML before implementing. Have the `scraper` HTML fallback ready.
- **FLiNG download links are not stable**: FLiNG trainer download links point to OneDrive/Google Drive — these expire. Store the trainer **page** URL (`https://flingtrainer.com/{trainer-slug}/`), not the file host URL.
- **`MAX_CACHE_PAYLOAD_BYTES = 524_288`**: `external_cache_entries` silently stores NULL for payloads exceeding 512 KiB. Per-game cache entries are small (~1-5 KiB). Do not attempt to cache a full game list in a single entry.
- **IPC command split (sync vs async)**: `discovery_search_trainers` (Phase A, sync SQLite) must remain separate from `discovery_search_external` (Phase B, async HTTP). Tap results must not wait on network.
- **`useScrollEnhance` registration**: Any scrollable results container in the discovery panel must be added to `SCROLLABLE` in `useScrollEnhance.ts` — easy to miss, causes dual-scroll jank.
- **`compute_correlation_status()` is build-ID centric**: Do not reuse it directly in `matching.rs` for trainer version comparison. Write new advisory logic in `discovery/matching.rs`.
- **WebKitGTK XSS**: External URLs from Phase B are user-controlled. Always use Tauri `open` plugin; never `<a href>` navigation; never `dangerouslySetInnerHTML` with external content.
- **Anti-scraping on FLiNG**: FLiNG may use Cloudflare anti-bot. Whether `reqwest` with the `CrossHook/{version}` user-agent is sufficient is untested.
- **PCGamingWiki scope is name normalization only**: Decision 3 explicitly excludes PCGamingWiki as a trainer source. It is only for game name cross-reference/normalization in Phase B.

---

## Documentation Gaps

1. **FLiNG RSS live verification needed**: Marked as unverified in `research-external.md`. Must verify `https://flingtrainer.com/category/trainer/feed/` returns valid XML before committing to RSS as primary approach.

2. **FLiNG HTML structure not documented**: CSS selectors for version strings and option lists from individual trainer pages not confirmed. Required if scraper fallback is needed.

3. **`discovery_search_external` exact response type**: `feature-spec.md` defines `TrainerSearchResponse` for Phase A. Phase B async command may reuse this or require a distinct type (e.g., with trust indicators, source attribution). Needs decision at implementation time.

4. **Advisory version matching algorithm**: `research-practices.md` says do NOT directly reuse `compute_correlation_status()` for trainer version strings. No algorithmic specification exists for `matching.rs` advisory comparison. Must be designed during implementation.

5. **`text_utils.rs` does not exist yet**: `research-practices.md` resolved decision requires lifting `tokenize()`/`token_hits()` to a new `text_utils.rs` module. This module must be created as part of Phase B (or a preparatory PR).

6. **PCGamingWiki integration design**: Decision 3 mentions PCGW for "game name normalization" but no concrete integration design or IPC surface is specified. Implementers must determine whether this is a call inside `client.rs` or a separate optional step.

7. **`MetadataStore` public API not documented**: Discoverable only by reading `crosshook-core/src/metadata/mod.rs` directly. Phase B will need to expose cache access methods on `MetadataStore` for `discovery/client.rs` to use.

8. **No frontend test strategy**: No configured frontend test framework. `TrainerDiscoveryPanel.tsx` and progressive loading behavior cannot be unit-tested. Only Rust pure functions and `MetadataStore::open_in_memory()` tests have a clear path.
