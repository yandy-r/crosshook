# Documentation Research: SQLite Metadata Layer Phase 3 — Catalog and Intelligence

## Overview

Phases 1 and 2 are fully implemented. Phase 1 established `MetadataStore` with `Arc<Mutex<Connection>>`, schema v2 (`profiles` + `profile_name_history`), and profile sync hooks. Phase 2 added `launchers` and `launch_operations` tables, three new API methods, and integration hooks in Tauri launch/export commands (schema v3). Phase 3 ("Catalog and Intelligence") adds five capabilities: community tap indexing with HEAD commit watermark skip, collections/favorites UX, usage insights queries, external metadata cache with payload validation and size bounds, and optional FTS5 for community search.

---

## Feature Spec — Phase 3 Sections

**File**: `docs/plans/sqlite3-addition/feature-spec.md`

### Phase 3 Schema Declaration (line 223–225)

The spec names five new tables:

```
community_taps, community_profiles, external_cache_entries, collections, collection_profiles
```

These are declared at line 223 as additive to Phase 1 and Phase 2 tables. No DDL inline at the declaration — full DDL must be derived from the data model section and technical research.

### Phase 1/2 Schema Tables (lines 153–221) — Remain Unchanged

Phase 3 adds schema version 4 on top of existing v3 tables (`profiles`, `profile_name_history`, `launchers`, `launch_operations`).

### API Design — MetadataStore Public API Extension (lines 230–249)

Existing public method signatures are at lines 230–249. Phase 3 adds:
- `sync_tap_index()` — called from `commands/community.rs` after `community_sync()`
- Collection CRUD methods
- Usage insights queries (`most_launched`, `last_successful_launch`, etc.)
- Cache get/set methods for external metadata

The naming convention follows existing Phase 1/2 methods: snake_case, `profile_name: &str` as input (not UUID), fail-soft delegation through `with_conn`.

### Security Considerations (lines 358–413)

Phase 3-relevant security findings from the spec:
- **W3**: `external_cache_entries.payload_json` max **512 KB** per entry (separate from Phase 2's 4 KB diagnostic limit)
- **W6**: stored paths used in filesystem ops must be re-validated — applies to any `community_profiles.manifest_path` used in tap scan comparisons
- **W8**: community tap `description` and `game_name` rendered in React WebView — audit `dangerouslySetInnerHTML` usage in `CommunityBrowser.tsx`
- **A6**: `community_profiles` string length bounds: `game_name` ≤ 512 bytes, `description` ≤ 4 KB, `platform_tags_json` ≤ 2 KB

### Success Criteria for Phase 3 (lines 111–119)

Relevant success criteria:
- Community manifest browsing uses indexed local metadata instead of repeated recursive scans
- Tap syncs skip re-indexing when HEAD commit is unchanged
- The authority boundary between TOML/filesystem and SQLite is explicit in code and documentation
- Startup reconciliation scan detects and repairs SQLite/TOML name mismatches

### Risk Assessment for Phase 3

From feature-spec.md lines 358–393 and recommendations:
- Phase 3 has the most scope risk of the three phases
- FTS5 should be deferred unless search UX evidence demands it — `LIKE` queries sufficient for v1
- `CommunityTapSyncResult` already includes `head_commit` and full `CommunityProfileIndex` — upsert is a data copy, not a new scan
- SQLite augments `index_taps()` as read cache with HEAD watermark; git workspace scan remains source of truth (RF-4)

---

## Existing Research Artifacts

### `research-business.md` — Phase 3-Relevant Extracts

**Business Rule 13 — Tap Sync Idempotency Rule**: SQLite tracks HEAD commit per tap; re-indexing skips taps where HEAD has not changed. This is the primary optimization for `community_taps` table.

**Workflow: Community Tap Sync** (lines 108–118):
1. After sync, `rev-parse HEAD` captures HEAD commit hash via `CommunityTapSyncResult.head_commit`
2. SQLite: compare HEAD against stored value → if unchanged, skip re-indexing → if changed, upsert manifest rows, update HEAD record, emit sync event
3. `community_sync` Tauri command is the natural upsert point (TC-6)

**UX-5 — Collections Write** (optimistic with 30-second undo window): Collection and favorites writes are optimistic — UI updates immediately, write confirms in background. Destructive metadata actions have a 30-second undo window. No undo window exists anywhere in the current codebase — must be built new.

**UX-6 — Community Tap and Cache Freshness Defaults**:
- ProtonDB / external metadata cache: **48-hour** default staleness threshold
- Community tap index: **7-day** default ("stale" badge in CommunityBrowser)
- These become new fields in `AppSettingsData` or a new `CacheSettingsData` struct, persisted in `settings.toml`

**Domain Model for Phase 3 entities** (lines 122–136):
| Entity | Key Fields | Authority |
|---|---|---|
| `community_tap_state` | `tap_url`, `tap_branch`, `head_commit`, `last_sync_at`, `last_sync_status` | SQLite |
| `community_catalog_entry` | `tap_url`, `relative_path`, `game_name`, `trainer_name`, `compatibility_rating`, `schema_version`, `manifest_json_cache` | SQLite |
| `collection_membership` | `collection_name`, `profile_id` | SQLite |
| `external_metadata_cache` | `cache_key`, `source`, `payload`, `fetched_at`, `expires_at` | SQLite |

**TC-3 — Community Tap Identity** (lines 267–270):
- `community_tap_state` primary key is `(tap_url, tap_branch)`
- `tap_branch = NULL` maps to `DEFAULT_TAP_BRANCH`
- Authority split: `AppSettingsData.community_taps` in `settings.toml` remains the canonical subscription list; SQLite stores only sync state and catalog cache

**RF-4 — Community Tap — Augment vs Replace** (lines 314–316):
- SQLite augments, not replaces `index_taps()`. Git workspace scan remains source of truth.
- Skip-on-unchanged: after sync, compare `head_commit` to stored value → if equal, return cached `CommunityProfileIndex` → if changed or absent, run `index_taps()` and upsert
- In-memory `CommunityProfileIndex` type continues to be used throughout the app

**Open Questions** still relevant to Phase 3 (lines 326–332):
- Should collections stay local-only or be designed for future export/import?
- Should launch logs be retained indefinitely or managed by SQLite rotation?

### `research-external.md` — Phase 3-Relevant Extracts

**FTS5 documentation** (lines 74–86):
- Requires `SQLITE_ENABLE_FTS5` compile flag — included in `bundled` feature by default
- External-content mode: stores only FTS index; actual rows live in normal table with triggers
- Trigram tokenizer: enables substring and LIKE/GLOB without prefix knowledge — useful for fuzzy game-name search
- Prefix indexes with `prefix='2 3'` option speed up prefix queries
- Auxiliary functions: `bm25()` for relevance ranking, `highlight()`, `snippet()`
- Maintenance: `INSERT INTO fts(fts) VALUES('optimize')` to compact; `rebuild` to regenerate

**JSON Functions** (lines 67–72):
- `json_valid(payload)` — validate before storage in `external_cache_entries`
- `json_extract()`, `json_each()` — for querying cached payloads
- **JSONB** (`jsonb_*` functions): available in SQLite ≥ 3.45.0; bundled 3.51.3 includes it; reduces parse overhead for blob storage
- `external_cache_entries.payload_json` can use JSONB for efficient re-reads

**Open Decision on FTS5** (lines 222–223):
- Deferring FTS5 to v2 is reasonable; schema can be extended later without data loss
- Regular B-tree indexes with `LIKE` queries cover basic game/trainer name lookup

**rusqlite `serde_json` feature** (line 107): enables `FromSql`/`ToSql` for `serde_json::Value` — use for `external_cache_entries.payload_json` storage

### `research-security.md` — Phase 3 Security Findings

**W3 — Unbounded cached payload sizes** (line 27):
- `external_cache_entries.payload_json` max **512 KB** per entry
- `launch_operations.diagnostic_summary` max 4 KB (Phase 2 already implemented)
- Enforce before INSERT — reject or truncate
- Define constant: `pub const MAX_CACHE_PAYLOAD_BYTES: usize = 512_000;` in `models.rs`

**W6 — SQLite names from DB used in filesystem operations** (line 30):
- `community_profiles.manifest_path` paths stored from tap scan used in filesystem operations
- Re-apply `validate_name()` / path-safety checks on SQLite-sourced paths before any `fs::` call
- Apply `validate_stored_path()` (introduced in Phase 1 `metadata/db.rs` or `models.rs`) before every filesystem operation using a path retrieved from SQLite

**W8 — Community tap fields rendered in React WebView** (line 32):
- `game_name`, `trainer_name`, `author`, `description` from untrusted git repos displayed in `CommunityBrowser.tsx`
- React JSX renders text content safely by default via `textContent`
- **Audit required**: verify no `dangerouslySetInnerHTML` in `CommunityBrowser.tsx` or `useCommunityProfiles.ts` for any manifest-sourced field
- Current `CommunityBrowser.tsx` uses standard JSX interpolation (`{entry.manifest.metadata.game_name}`) — W8 is currently safe but must be maintained

**A6 — Community tap manifest content injection into SQLite** (lines 45–46):
- Validate string lengths before inserting `community_profiles` rows: `game_name` ≤ 512 bytes, `description` ≤ 4 KB, `platform_tags_json` ≤ 2 KB
- Return diagnostic entry for manifests exceeding limits rather than silently truncating

**Data Sensitivity for Phase 3 tables** (lines 55–66):
- `community_profiles.manifest_json_cache`: **High** — caches full `GameProfile` payloads from community taps including trainer paths, game paths, dll_paths. Same sensitivity as profile TOML.
- `external_cache_entries.payload_json`: Low–Medium — cached ProtonDB/cover-art; may contain Steam App IDs and game names
- `community_taps.url`: Low — git repository URLs the user subscribes to

### `research-ux.md` — Phase 3 UX Patterns

**Community Browsing workflow** (lines 88–93):
1. User searches from local indexed metadata (SQLite). No network call required for filter/search within already-synced taps.
2. Tap refresh indicators: non-blocking status chip ("Last synced 2 hours ago")
3. When HEAD commit unchanged after refresh check: skip re-indexing entirely. Optionally show "Up to date" badge (auto-dismiss 2s)
4. If tap stale (no refresh in 7+ days): passive nudge "Tap data may be outdated. Refresh?" — not blocking warning
5. Search results appear optimistically; background tap refresh updates list in-place without resetting scroll

**Collections, Favorites, Undo** (lines 182–199):
- Option B (sidebar filter) recommended for primary collection interaction on Steam Deck; Option C (collection chip on card) for visual indicator
- Optimistic writes: `is_favorite` / collection membership updates SQLite immediately, confirms in background
- Destructive metadata actions: 30-second undo window within session — not persisted across sessions
- No undo window exists anywhere in current codebase — must be built new alongside collections

**Freshness defaults** (from business rules UX-6):
- 48h for external metadata (ProtonDB, cover art)
- 7d for community tap index (triggers "stale" badge)
- New fields in `AppSettingsData` or `CacheSettingsData`, persisted in `settings.toml`

### `research-practices.md` — Phase 3 Patterns

**KISS Assessment for Phase 3 tables** (lines 88–96):
- `community_profiles` + `community_taps`: keep `index_taps()` as source of truth; SQLite is a read cache; only add when tap size grows large enough to feel slow. Per KISS, Phase 3 is the right phase (not Phase 1/2).
- `external_cache_entries`: no current UI feature drives this — belongs exclusively in Phase 3
- `collections`/`collection_profiles`: building schema before UI is over-engineering. The stable `profile_id` prerequisite (Phase 1) is already in place.

**Phase 3 module file structure** (lines 37–51):
```
src/metadata/
  community_index.rs  — tap manifest indexing (Phase 3)
  cache_store.rs      — external metadata cache (Phase 3)
```
These files should NOT be created as stubs in Phase 1 or Phase 2 — defer file creation until Phase 3.

**Existing reusable code for Phase 3**:
- `CommunityTapSyncResult` (`community/taps.rs:41-46`): contains `head_commit` and full `CommunityProfileIndex` — `sync_tap_index()` should accept a slice of these directly
- `sanitize_display_path()` (already promoted to `commands/shared.rs` in Phase 1): apply to manifest paths before storing
- `chrono::Utc::now().to_rfc3339()` (existing dep): freshness timestamps in `external_cache_entries`
- `serde_json` (existing dep): payload storage in `external_cache_entries`
- `validate_stored_path()` (introduced in Phase 1): apply at every SQLite-to-filesystem boundary

### `research-recommendations.md` — Phase 3 Guidance

**Phase 3 task prerequisites** (lines 186–191):
- `CommunityTapSyncResult` already includes `head_commit` and full `CommunityProfileIndex` — upsert is a data copy
- SQLite augments `index_taps()` as read cache with HEAD watermark; git workspace scan remains source of truth
- FTS should be deferred entirely unless search UX evidence demands it

**Required utilities for Phase 3** (lines 159–169):
- `MAX_CACHE_PAYLOAD_BYTES = 512_000` constant in `metadata/models.rs` (W3)
- `validate_stored_path()` must already be implemented from Phase 1 — apply to manifest paths retrieved from DB

**Technology decisions** (line 120):
- FTS5 deferred unless proven necessary — LIKE queries sufficient for v1 community search

### `research-technical.md` — Phase 3 Schema Details

**Phase 3 tables** (lines 48–54):
| Table | Purpose | Key Columns |
|---|---|---|
| `collections` | User curation groups | `collection_id TEXT PK` (ULID), `name TEXT UNIQUE`, `created_at TEXT` |
| `collection_profiles` | M:N join | `collection_id TEXT FK`, `profile_id TEXT FK`, `added_at TEXT` — composite PK |
| `profile_preferences` | favorites, pins, usage | `profile_id TEXT PK FK`, `is_favorite BOOLEAN`, `is_pinned BOOLEAN`, `usage_count INTEGER`, `last_launched_at TEXT` |
| `community_taps` | subscribed taps | `tap_id TEXT PK` (ULID), `url TEXT`, `branch TEXT`, `local_path TEXT`, `last_synced_commit TEXT`, `last_synced_at TEXT` |
| `community_profiles` | indexed manifest rows | `id INTEGER PK`, `tap_id TEXT FK`, `manifest_path TEXT`, `relative_path TEXT`, `game_name TEXT`, `trainer_name TEXT`, `compatibility_rating TEXT`, `author TEXT`, `platform_tags_json TEXT` |
| `external_cache_entries` | ProtonDB/art cache | `id INTEGER PK`, `cache_bucket TEXT`, `cache_key TEXT`, `payload_json TEXT`, `fetched_at TEXT`, `expires_at TEXT` |

**Rust Type-to-Table mapping for Phase 3** (lines 76–84):
| Rust Type | Source File | Phase 3 Table(s) |
|---|---|---|
| `CommunityTapSubscription` | `community/taps.rs` | `community_taps` |
| `CommunityTapSyncResult` | `community/taps.rs` | `community_taps.last_synced_commit` |
| `CommunityTapWorkspace` | `community/taps.rs` | `community_taps.local_path` |
| `CommunityProfileIndexEntry` | `community/index.rs` | `community_profiles` |
| `CommunityProfileMetadata` | `profile/community_schema.rs` | `community_profiles` fields |
| `CompatibilityRating` | `profile/community_schema.rs` | `community_profiles.compatibility_rating` |
| `AppSettingsData` | `settings/mod.rs` | `community_taps` (taps list extracted) |

**Recommended Stable ID Strategy for Phase 3**: `tap_id` and `collection_id` use ULID (same as `profile_id` pattern). Primary match key for `community_taps` is `(url, branch)` per TC-3. `community_profiles` unique constraint on `(tap_id, relative_path)`.

### `dependency-analysis.md` — Phase 3 Tasks

Phase 3 task IDs from dependency graph: **3.1, 3.2, 3.3, 3.4**

```
3.1 (community tap state) → 3.2 (community catalog upsert)
                          → 3.3 (collections/favorites)
                          → 3.4 (external cache)
```

Tasks 3.2 and 3.3 can be parallelized after 3.1 is complete. 3.4 (external cache) is independent once 3.1 models are established.

---

## Business Rules for Phase 3

From `feature-spec.md` business rules + `research-business.md`:

1. **Tap Sync Idempotency Rule (Rule 13)**: SQLite tracks HEAD commit per tap as `(tap_url, tap_branch)`. Re-indexing skips taps where HEAD has not changed. Skip-on-unchanged is the primary performance optimization for Phase 3.

2. **Tap Authority Rule (TC-3)**: `AppSettingsData.community_taps` in `settings.toml` remains the canonical subscription list. SQLite `community_taps` stores only sync state (HEAD, last sync time, status) and catalog cache. SQLite never adds or removes subscriptions.

3. **Augment-Not-Replace Rule (RF-4)**: SQLite augments `index_taps()` as a read cache. The git workspace scan remains source of truth. If `head_commit` matches stored value, return cached `CommunityProfileIndex` from SQLite. If changed or absent, run `index_taps()` and upsert catalog rows.

4. **Collection Portability (open question)**: Should collections stay purely local in v1 or need future export semantics? Research recommends local-only for Phase 3; design `collection_profiles` schema to support future portability by using `profile_id` (stable UUID) not profile filename.

5. **Cache Staleness Policy (UX-6)**: Default thresholds: ProtonDB/external: 48h; community tap index: 7d. These are configurable via new `AppSettingsData` or `CacheSettingsData` fields in `settings.toml`. SQLite `external_cache_entries` and `community_tap_state` rows use these thresholds when computing freshness in queries.

6. **Community Manifest Schema Version Gate**: Current `index_taps()` skips entries where `manifest.schema_version != COMMUNITY_PROFILE_SCHEMA_VERSION`. The SQLite upsert path must apply the same version check before inserting `community_profiles` rows. Confirmed at `community/index.rs:145`.

7. **Optimistic Collection Writes (UX-5)**: Collection and favorites writes to SQLite are optimistic: UI updates immediately and write confirms in background. On write failure, UI rolls back visually with inline error. Destructive metadata actions have a 30-second undo window. No undo window exists anywhere in the current codebase — must be built new.

8. **Cache Offline-First Rule (Rule 7)**: Cached external metadata is optional and stale-tolerant. Missing cache data must never block launching or editing a profile.

9. **Payload Validation Rule (W3)**: `external_cache_entries.payload_json` must be validated via `json_valid()` before storage and bounded at 512 KB. Reject or truncate payloads exceeding limits.

10. **Phase 3 Sync Hook Location**: `community_sync` Tauri command in `src-tauri/src/commands/community.rs` is the natural upsert point for `community_profiles` rows after `community_sync()` completes. This mirrors Phase 2's pattern of hooking into the Tauri command layer (TC-6).

---

## Security Findings for Phase 3

### W3 — Unbounded Cached Payload Sizes (Must Address)

**Scope**: `external_cache_entries.payload_json`
**Risk**: malformed or adversarial external API response inserting megabytes of JSON into local DB; memory pressure during deserialization; unbounded disk growth
**Required mitigations**:
- Enforce maximum payload size before writing: **512 KB per entry**
- Validate `json_valid(payload_json)` before INSERT (SQLite built-in)
- Deserialize cached payloads lazily with error handling — never `unwrap()`
- Define constant: `pub const MAX_CACHE_PAYLOAD_BYTES: usize = 512_000;` in `metadata/models.rs`
- Reject oversized payloads with typed `MetadataStoreError` — never silently truncate

### W6 — SQLite Names from DB Used in Filesystem Operations (Must Address)

**Scope**: `community_profiles.manifest_path`, `community_taps.local_path`
**Risk**: corrupted/tampered DB causing path traversal in tap scan comparisons or manifest file reads
**Required mitigations**:
- Apply `validate_stored_path()` before any `fs::` call using a path from `community_profiles` or `community_taps`
- `validate_stored_path()` must be absolute, no `..` components, resolves within expected directory prefix (e.g., tap workspace root)
- Never assume stored paths are safe by virtue of having been stored

### W8 — Community Tap Fields Rendered in React WebView (Must Address)

**Scope**: `CommunityBrowser.tsx` rendering `game_name`, `trainer_name`, `author`, `description`, `platform_tags`
**Risk**: XSS if `dangerouslySetInnerHTML` is used for community manifest fields
**Current status**: `CommunityBrowser.tsx` uses standard JSX interpolation (safe). But Phase 3 may add new components.
**Required action**: Audit all new React components rendering community catalog data. Never use `dangerouslySetInnerHTML` for user-supplied or externally-sourced strings. Prefer `{value}` interpolation.

### A6 — String Length Bounds on Community Manifest Rows (Advisory)

**Required before inserting `community_profiles` rows**:
- `game_name`: ≤ 512 bytes
- `description`: ≤ 4,096 bytes
- `platform_tags_json`: ≤ 2,048 bytes
- `trainer_name`, `author`: ≤ 512 bytes each
- Return a diagnostic entry (not panic/error) for manifests exceeding limits
- Schema version gating already rejects unknown versions — A6 adds a secondary size gate

### A3 — Error Opacity at IPC Boundary (Advisory, Phase 3 Scope)

Community catalog query commands (`community_catalog_search`, etc.) must map rusqlite errors to opaque `MetadataError` variants before returning over IPC. Never expose raw SQL error text to the frontend.

---

## Edge Cases for Phase 3

| Scenario | Expected Behavior | Source |
|---|---|---|
| Tap HEAD unchanged after sync | Skip re-indexing entirely; return cached `CommunityProfileIndex` from SQLite | Rule 13, RF-4 |
| Tap HEAD changed but index is empty (fresh clone) | Run `index_taps()` fully; upsert all discovered manifest rows | RF-4 |
| Community manifest exceeds A6 length bounds | Record diagnostic entry; skip INSERT; continue indexing remaining manifests | A6, research-security.md |
| Community manifest has unsupported schema version | Skip (matches existing `index_taps()` behavior at `community/index.rs:145`) | Must match existing behavior |
| External cache payload > 512 KB | Reject with typed error; do not store; return stale or absent cache result | W3 |
| External cache `json_valid()` fails | Reject with typed error; treat as cache miss | W3 |
| `community_taps` tap subscription removed from settings | SQLite `community_taps` row becomes orphaned; `community_profiles` rows remain but are not browseable; cleanup is optional (Phase 3+ concern) | TC-3 |
| Collection write fails | UI rolls back visually with inline error; 30-second undo window for destructive actions | UX-5 |
| Profile in collection is deleted (tombstoned) | `collection_profiles` FK reference points to tombstoned `profiles` row; collection membership row is orphaned; tombstone display rules apply | Business Rule 12 (delete cascade) |
| SQLite unavailable when user opens Collections view | Show empty state with "metadata unavailable" message; no crash; collections UX individually suppressed per Fail-Soft Rule | Rule 7, UX-4 |
| Cached ProtonDB data expired | Use stale cache with freshness label; never block launch or profile edit | Rule 7, UX-6 |
| FTS5 table sync lag on large tap | FTS5 is additive — if out of sync, fallback to `LIKE` queries; never block browse on FTS5 failure | research-external.md |

---

## Community Module Documentation

### `community/taps.rs` (full path: `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`)

Key types for Phase 3 integration:

- **`CommunityTapSubscription`** (lines 19–25): `url: String`, `branch: Option<String>`. This is the canonical subscription identity — `(url, branch)` is the composite key for `community_taps` table (TC-3).
- **`CommunityTapWorkspace`** (lines 27–31): `subscription: CommunityTapSubscription`, `local_path: PathBuf`. The `local_path` maps to `community_taps.local_path`.
- **`CommunityTapSyncResult`** (lines 40–46): `workspace`, `status: CommunityTapSyncStatus`, `head_commit: String`, `index: CommunityProfileIndex`. **`head_commit`** is the watermark for skip-on-unchanged optimization. This struct is returned by both `sync_tap()` and `sync_many()`.
- **`CommunityTapStore::sync_many()`** (line 141): returns `Vec<CommunityTapSyncResult>` — the natural input for `sync_tap_index()` in `community_index.rs`.
- **`rev_parse_head()`** (line 237): captures HEAD commit after sync. Already sets `result.head_commit` — Phase 3 reads from `CommunityTapSyncResult.head_commit`, does not re-run `rev-parse`.

**Gotcha**: `CommunityTapSubscription` with `branch = None` maps to `DEFAULT_TAP_BRANCH = "main"` at runtime, but `None` is stored in settings TOML as absent. The `community_taps` SQLite row must store the explicit `branch` string (or `NULL` for `None`) consistently with how `settings.toml` represents it. Never normalize `None` to `"main"` in the SQLite layer — always mirror the settings representation.

### `community/index.rs` (full path: `src/crosshook-native/crates/crosshook-core/src/community/index.rs`)

- **`CommunityProfileIndex`** (line 9): `entries: Vec<CommunityProfileIndexEntry>`, `diagnostics: Vec<String>`. The in-memory representation that `index_taps()` builds. Phase 3 reads from SQLite cache or builds this and upserts.
- **`CommunityProfileIndexEntry`** (lines 16–25): `tap_url: String`, `tap_branch: Option<String>`, `tap_path: PathBuf`, `manifest_path: PathBuf`, `relative_path: PathBuf`, `manifest: CommunityProfileManifest`. All fields map to `community_profiles` table columns.
- **`index_taps()`** (line 59): full directory walk; called when SQLite cache is absent or stale (HEAD changed). Returns `CommunityProfileIndex`. Phase 3: SQLite augments this function's output, does not replace it.
- **Schema version check at line 145**: `manifest.schema_version != COMMUNITY_PROFILE_SCHEMA_VERSION` → push diagnostic message, skip entry. Phase 3 SQLite upsert must apply the same guard before inserting.

**Key insight**: `index_taps()` already handles errors gracefully (pushes to `diagnostics`, continues). Phase 3 SQLite upsert path should follow the same pattern: a failed upsert for one manifest row should log to diagnostics and continue, not abort the entire sync.

### `profile/community_schema.rs` (full path: `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`)

Contains `CommunityProfileManifest`, `CommunityProfileMetadata`, and `CompatibilityRating`:

- **`CommunityProfileMetadata`** fields: `game_name`, `game_version`, `trainer_name`, `trainer_version`, `proton_version`, `platform_tags: Vec<String>`, `compatibility_rating: CompatibilityRating`, `author`, `description`. All map directly to `community_profiles` table columns (strings) or serialized as JSON (`platform_tags_json`).
- **`CompatibilityRating`**: `Unknown`, `Broken`, `Partial`, `Working`, `Platinum`. Must be stored as TEXT in SQLite via `as_str()` or serde. Matches the frontend's `CommunityCompatibilityRating` type in `useCommunityProfiles.ts`.
- **`COMMUNITY_PROFILE_SCHEMA_VERSION`**: integer constant used for schema version gating. Phase 3 must reference this when inserting `community_profiles.schema_version` column.

---

## Frontend Documentation

### `CommunityBrowser.tsx` (full path: `src/crosshook-native/src/components/CommunityBrowser.tsx`)

**Current state**: performs client-side filtering via `matchesQuery()` (line 30) over `index.entries` from `community_list_profiles` invoke. Does not query SQLite directly — all data comes through existing Tauri commands.

**Phase 3 impact**:
- `community_list_profiles` Tauri command will need to return SQLite-backed data if Phase 3 changes its implementation to serve from cache
- The `matchesQuery()` function (lines 30–52) currently searches all manifest fields including `description` — if Phase 3 adds FTS5, the search can be moved server-side (Tauri command), but the frontend filter logic would remain as a client-side fallback
- **W8 verification**: Component renders community data via JSX interpolation (`{entry.manifest.metadata.game_name}`, `{entry.manifest.metadata.description}`, etc.) at lines 358, 361, 369–380, 391 — no `dangerouslySetInnerHTML` present. This is safe and must remain safe for new Phase 3 components.

**Collections UX gaps**: `CommunityBrowser.tsx` has no collection or favorites UI. Phase 3 must add new components or extend existing cards to support:
- Favorite toggle button on profile cards
- Collection assignment UI (from research-ux.md Option B/C recommendation)
- Undo toast for destructive collection actions (30-second window)

### `useCommunityProfiles.ts` (full path: `src/crosshook-native/src/hooks/useCommunityProfiles.ts`)

**Current Tauri commands invoked**:
- `community_list_profiles` (line 170): returns `CommunityProfileIndex`
- `community_sync` (line 179): returns `Vec<CommunityTapSyncResult>`
- `community_add_tap` (line 195): returns `CommunityTapSubscription[]`
- `community_import_profile` (line 229): returns `CommunityImportResult`

**Phase 3 impact**:
- `community_sync` result already contains `head_commit` — Phase 3 SQLite hook fires after `community_sync` returns
- After Phase 3, `community_list_profiles` may return SQLite-cached data rather than re-scanning; the frontend type `CommunityProfileIndex` stays the same
- **New commands needed for Phase 3**:
  - `community_catalog_search` (optional, if FTS5 implemented)
  - `get_collections` / `create_collection` / `add_to_collection` / `remove_from_collection`
  - `get_cache_freshness` (for freshness labels in UI)

**TypeScript types for Phase 3** (not yet defined):
```typescript
// New types needed in src/types/index.ts or src/types/metadata.ts:
export interface CollectionInfo { collection_id: string; name: string; created_at: string; }
export interface CacheFreshness { last_synced_at: string | null; is_stale: boolean; }
```

### `types/` directory

- `src/types/index.ts`: re-exports from other type files. Phase 3 types should be added to a new `src/types/metadata.ts`.
- `CommunityTapSyncResult` already typed in `useCommunityProfiles.ts` (line 77): includes `head_commit: string` — this is the key field for skip-on-unchanged display in UI ("Last synced commit: abc1234")

---

## External Documentation References

### SQLite FTS5 — `https://www.sqlite.org/fts5.html`

Key points for Phase 3:
- FTS5 enabled by default in `rusqlite`'s `bundled` feature (already used)
- **External-content mode** is the recommended pattern: FTS index only, main table holds actual rows, triggers keep them in sync
  ```sql
  CREATE TABLE community_profiles_fts_shadow AS SELECT ... FROM community_profiles WHERE 0;
  CREATE VIRTUAL TABLE community_profiles_fts USING fts5(game_name, trainer_name, content='community_profiles');
  CREATE TRIGGER ... AFTER INSERT ON community_profiles ...
  ```
- **Trigram tokenizer** for substring search: `content='community_profiles', tokenize='trigram'`
- `bm25(fts)` for ranking results
- Maintenance: run `INSERT INTO fts(fts) VALUES('optimize')` after bulk inserts
- **Decision confirmed in spec**: defer FTS5 unless query performance demands it; `LIKE` queries sufficient for v1

### SQLite JSON Functions — `https://www.sqlite.org/json1.html`

Key functions for Phase 3:
- `json_valid(payload)`: returns 1 if valid JSON — use before INSERT into `external_cache_entries`
- `json_extract(payload, '$.field')`: extract specific fields from cached payload without full deserialization
- `jsonb_*` functions (SQLite ≥ 3.45.0, bundled 3.51.3 included): `jsonb()` to convert JSON text to binary BLOB for storage; `json()` to convert back
- **JSONB recommended** for `external_cache_entries.payload_json` column: use `BLOB` type with `jsonb()` to store and `json()` to retrieve

### rusqlite `serde_json` Feature Flag

Enable in `Cargo.toml` if not already present:
```toml
rusqlite = { version = "0.39", features = ["bundled", "serde_json"] }
```
Provides `FromSql`/`ToSql` for `serde_json::Value` — allows storing/retrieving JSON payloads directly without manual serialization in `external_cache_entries`.

### SQLite UPSERT — `https://sqlite.org/lang_upsert.html`

Phase 3 community index upsert pattern:
```sql
INSERT INTO community_profiles
  (tap_id, manifest_path, relative_path, game_name, trainer_name, compatibility_rating, author, platform_tags_json)
  VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
  ON CONFLICT(tap_id, relative_path) DO UPDATE SET
    game_name = excluded.game_name,
    trainer_name = excluded.trainer_name,
    compatibility_rating = excluded.compatibility_rating,
    author = excluded.author,
    platform_tags_json = excluded.platform_tags_json;
```

---

## Must-Read Documents (Priority Order)

Priority order for Phase 3 implementers:

1. **`docs/plans/sqlite3-addition/feature-spec.md`** — REQUIRED. Phase 3 schema declaration (line 223), security findings W3/W6/W8/A6 (lines 358–413), Phase 3 recommendations (lines 433–451), success criteria (lines 111–119), edge case table (lines 99–109).

2. **`docs/plans/sqlite3-addition/shared.md`** — REQUIRED. This is the compressed context document for Phase 2 and includes all patterns, locked design decisions, and cross-cutting rules. Phase 3 must extend these patterns, not deviate from them.

3. **`src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`** — REQUIRED. Understand `with_conn()` before writing any Phase 3 method. All new public methods follow the same delegation pattern.

4. **`src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`** — REQUIRED. Understand the version runner before adding Phase 3 migrations. Phase 3 adds `migrate_3_to_4()` for community and collections tables. Pattern: `if version < N { migrate(conn)?; pragma_update(N)?; }`.

5. **`src/crosshook-native/src-tauri/src/commands/community.rs`** — REQUIRED. Phase 3 hooks `sync_tap_index()` into `community_sync` command. Read current command signatures before adding metadata hooks.

6. **`docs/plans/sqlite3-addition/research-security.md`** — REQUIRED. W3 (512 KB cache payload bound), W6 (re-validate stored paths), W8 (community tap fields in WebView), A6 (manifest string length bounds).

7. **`src/crosshook-native/crates/crosshook-core/src/community/taps.rs`** — REQUIRED. `CommunityTapSyncResult` and `CommunityTapWorkspace` types — Phase 3 SQLite input types.

8. **`src/crosshook-native/crates/crosshook-core/src/community/index.rs`** — REQUIRED. `CommunityProfileIndexEntry` field layout and `index_taps()` behavior. Schema version check at line 145 must be replicated in SQLite upsert path.

9. **`docs/plans/sqlite3-addition/research-business.md`** — RECOMMENDED. Business Rule 13 (tap idempotency), UX-5 (collections optimistic writes + undo), UX-6 (freshness defaults), RF-4 (augment-not-replace), TC-3 (tap identity), TC-6 (sync is the upsert point).

10. **`docs/plans/sqlite3-addition/research-technical.md`** — RECOMMENDED. Type-to-table mapping for Phase 3 types. Authority boundaries for Phase 3 schema.

11. **`src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`** — RECOMMENDED. `CommunityProfileMetadata` and `CompatibilityRating` field layout — maps directly to `community_profiles` table columns.

12. **`docs/plans/sqlite3-addition/research-external.md`** — RECOMMENDED. FTS5 external-content mode pattern, JSON function usage, JSONB for payload storage.

13. **`src/crosshook-native/src/components/CommunityBrowser.tsx`** — RECOMMENDED. W8 audit target; understand current search/filter implementation before modifying.

14. **`docs/plans/sqlite3-addition/research-ux.md`** — RECOMMENDED. Collections UI placement recommendations (Option B/C), community browse UX, freshness indicator patterns, undo window requirements.

15. **`CLAUDE.md`** (repo root) — RECOMMENDED. Commit message format, test commands (`cargo test -p crosshook-core`), build commands.

---

## Documentation Gaps

| Gap | Impact | Notes |
|---|---|---|
| No Phase 3 shared context document | **High** — Phase 2 has `shared.md`; Phase 3 has no equivalent compressed context | Must be created before implementation starts. Should follow the same structure as `shared.md`: relevant files, patterns, schema DDL, locked design decisions, cross-cutting rules. |
| No Phase 3 analysis-tasks.md | High | Phase 2 has `analysis-tasks.md` with task DAG and dependency analysis. Phase 3 needs equivalent decomposition before parallel implementation. |
| Phase 3 schema DDL not fully specified | High | Feature spec declares table names (line 223) but does not provide inline DDL for Phase 3 tables. Full DDL must be derived from `research-technical.md` type mapping. See the table schemas in the "research-technical.md" section above. |
| No `community_index.rs` file | High | Designated Phase 3 file per `research-practices.md`. Does not exist yet; must be created in Phase 3 (never as a Phase 1/2 stub). |
| No `cache_store.rs` file | High | Same as `community_index.rs` — Phase 3 file, must not be created early. |
| No TypeScript types for Phase 3 responses | Medium | `src/types/metadata.ts` does not exist. Collections, cache freshness, and catalog search responses need new interfaces before frontend work begins. |
| No `commands/community.rs` metadata hook documentation | Medium | Phase 3 hooks into `community_sync` command. Current `commands/community.rs` has no metadata wiring yet (analogous to how `commands/profile.rs` had no wiring before Phase 1). Read current command signatures before implementing. |
| Collections undo window implementation unclear | Medium | UX-5 specifies a 30-second undo window for destructive collection actions. No equivalent exists in the codebase. Implementation details not documented. |
| External metadata fetch source not specified | Medium | `external_cache_entries` has `source TEXT` and `cache_key TEXT` columns but the external APIs (ProtonDB, cover art) are not yet integrated. Phase 3 cache storage can be implemented independently of the actual fetch logic. |
| FTS5 decision criteria not documented | Low | Spec says "defer unless query performance demands it." No documented threshold or benchmark criteria for deciding when to implement FTS5. Define the decision gate before Phase 3 closes. |
| `AppSettingsData` freshness fields not added | Low | UX-6 requires new fields (`cache_ttl_external_hours`, `tap_stale_days`) in `AppSettingsData` or a new `CacheSettingsData` struct. These fields do not currently exist. Phase 3 must add them to `settings/mod.rs` and `settings.toml` persistence. |
| No retention policy for `external_cache_entries` | Low | No pruning strategy documented. Phase 3 should at minimum define an `expires_at`-based eviction query. Add a `prune_expired_cache_entries()` call to the startup sweep in `startup.rs`. |

---

## Architecture-Confirmed Facts (Cross-Reference with research-architecture.md)

The following facts were confirmed or corrected by the architecture researcher after reviewing the live codebase. They supersede any earlier inferences.

### Actual Schema v3 Table Definitions

The current post-Phase-2 schema (verified from source):

- `profiles` (v1): `profile_id TEXT PK`, `current_filename TEXT NOT NULL UNIQUE`, `current_path`, `game_name`, `launch_method`, `content_hash`, `is_favorite INTEGER DEFAULT 0`, `is_pinned INTEGER DEFAULT 0`, `source_profile_id TEXT FK`, `deleted_at`, `created_at`, `updated_at`
- `launchers` (v3): `launcher_id TEXT PK`, `profile_id FK NULLABLE`, `launcher_slug NOT NULL UNIQUE`, `display_name`, `script_path`, `desktop_entry_path`, `drift_state NOT NULL DEFAULT 'unknown'`, `created_at`, `updated_at`
- `launch_operations` (v3): `operation_id TEXT PK`, `profile_id FK NULLABLE`, `profile_name`, `launch_method`, `status DEFAULT 'started'`, `exit_code`, `signal`, `log_path`, `diagnostic_json` (max 4KB), `severity`, `failure_mode`, `started_at`, `finished_at`

### `is_favorite` / `is_pinned` Already in Schema

**Important**: `is_favorite INTEGER NOT NULL DEFAULT 0` and `is_pinned INTEGER NOT NULL DEFAULT 0` are already in the `profiles` table (migration v1). Phase 3 does NOT need to add these columns. What is missing is:
- A `MetadataStore` API method to expose `set_favorite` / `set_pinned`
- A Tauri command to wire these to the frontend
- Frontend component rendering/toggling them

Phase 3 wires up the already-present columns — it does not add new schema for basic favorites.

### rusqlite Version Discrepancy

**Actual**: `rusqlite = { version = "0.38", features = ["bundled"] }` in `crosshook-core/Cargo.toml`
**Spec recommends**: 0.39.0 (bundles SQLite 3.51.3 with WAL-reset bug fix)
**Impact**: The WAL write+checkpoint data-corruption race (SQLite 3.7.0–3.51.2) is present in the current bundled SQLite. Phase 3 should upgrade to 0.39.0 as part of Phase 3 Cargo.toml changes. The `serde_json` feature for rusqlite can be added at the same time if needed for `payload_json` storage.

### Confirmed Community Tauri Command Signatures

From `src-tauri/src/commands/community.rs`:

| Command | Current Signature | Phase 3 Change |
|---|---|---|
| `community_sync` | `(settings_store: State<SettingsStore>, tap_store: State<CommunityTapStore>) -> Result<Vec<CommunityTapSyncResult>, String>` | Add `metadata_store: State<MetadataStore>`; call `sync_tap_index()` after `sync_many()` |
| `community_list_profiles` | `(settings_store, tap_store) -> Result<CommunityProfileIndex, String>` | Optionally add `State<MetadataStore>` for SQLite cache fast path |
| `community_add_tap` | `(tap, settings_store) -> Result<Vec<CommunityTapSubscription>, String>` | No Phase 3 change |
| `community_import_profile` | `(path, profile_store, settings_store, tap_store) -> Result<CommunityImportResult, String>` | No Phase 3 change |

`MetadataStore` is already registered via `.manage(metadata_store)` in `lib.rs:80` — no new `.manage()` call needed.

### `community.rs` State Injection Gap

`community.rs` currently takes `State<SettingsStore>` and `State<CommunityTapStore>` but **not** `State<MetadataStore>`. Phase 3 adds `State<'_, MetadataStore>` to `community_sync` as the minimum required change for tap indexing. Adding it to `community_list_profiles` for the SQLite cache fast path is optional for Phase 3.

### HEAD Watermark Not Yet Persisted

`CommunityTapSyncResult.head_commit` is populated at `taps.rs:179` via `rev_parse_head()` but is never written anywhere durable in the codebase today. The entire HEAD watermark optimization is a Phase 3 addition — no existing code needs to be modified to remove conflicting HEAD tracking, only new code needs to be added.

### External Cache Architecture Boundary

No HTTP client dependency exists in `crosshook-core`. **External fetch must remain in the Tauri command layer** — `cache_store.rs` only reads and writes SQLite. The separation mirrors Phase 2's pattern: `commands/launch.rs` runs the process and passes `DiagnosticReport` to `record_launch_finished()`. For external cache: Tauri command fetches, validates, then calls `metadata_store.put_cache_entry(source, key, payload, expires_at)`. No new Cargo dependency needed in crosshook-core for Phase 3.

### Usage Insights Are SQL Aggregates Over Phase 2 Data

No new table is needed for usage insights. All queries run against the existing `launch_operations` table (Phase 2). The `profile_name TEXT` column (denormalized) and `profile_id FK` column provide both resilient and precise lookups. Example insight queries use `COUNT(*)`, `MAX(started_at)`, and `FILTER (WHERE status = ...)` — all standard SQLite aggregate functions with no extension required.

### FTS5 Schema Pattern (Deferred, But Specified)

If FTS5 is implemented, the correct pattern is content-table mode:

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS community_profiles_fts USING fts5(
    game_name,
    trainer_name,
    author,
    description,
    platform_tags,
    content='community_profiles',
    content_rowid='rowid'
);
```

INSERT/UPDATE/DELETE triggers on `community_profiles` keep the FTS index synchronized. FTS5 is available in the `bundled` feature (already used) — no new feature flag needed in Cargo.toml.

### Additional Documentation Gap (Architecture-Identified)

| Gap | Impact | Notes |
|---|---|---|
| `community_list_profiles` two-tier fast path not specified | Medium | Architecture researcher identified a two-tier pattern: (1) serve from SQLite if all subscribed taps have a `last_head_commit` row; (2) fall back to `index_workspaces()` full scan otherwise. This behavior is not documented in the feature spec but is the correct degraded-mode implementation. Needs specification before `community_list_profiles` is modified. |
| No `profile_list_with_metadata` command design | Medium | Phase 3 favorites/pins in the profile list require either a new `profile_list_with_metadata` command or a separate `metadata_get_profile_flags` command. Neither is designed or specified yet. |
| rusqlite 0.38 → 0.39 upgrade not in Phase 3 task list | Low | The WAL-reset bug fix requires upgrading rusqlite. This is not listed as a Phase 3 task in the feature spec or dependency analysis but should be included to match the security recommendation (research-security.md A1). |
