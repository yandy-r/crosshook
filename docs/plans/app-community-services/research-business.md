# YAN-835 (M1-34) — Business-Logic Research: invariants, tests, gaps

Scope: behaviour that must survive moving community (taps, sync, import/export, indexed
list), Lutris import, discovery and ProtonDB commands from `src-tauri` into toolkit-free
`crosshook-app` services. Paths relative to `src/crosshook-native/`; `src-tauri/` = `tauri/`,
`crates/crosshook-core/src` = `core/`, `crates/crosshook-app/src` = `app/`.
Read-only research; only this report was written.

---

## 0. Cross-cutting facts

- Today's shells are thin but not empty: dedupe, settings read-modify-write, sync
  orchestration (index → offline upsert → `last_sync_at` backfill), workspace-containment
  checks, degraded-catalog fallback and ProtonDB dismiss/suggestion assembly all live in
  `tauri/commands/{community,discovery,protondb}.rs`. Lutris (`tauri/commands/lutris.rs:15-34`)
  and ProtonDB _accept_ already delegate to core / `ProfileWriter` (M1-06) — do not re-move accept.
- No command in scope reads `settings.offline_mode` today (`core/settings/types.rs:241`; only
  settings load/save round-trips it). The spec's offline behaviour is **new gating**, not a port.
- No sync progress/events exist today: `community_sync` emits nothing; the frontend only has a
  local `syncing` boolean (`src/hooks/useCommunityProfiles.ts:259-280`). "Sync progress is
  runtime-only" (issue) therefore means: do not invent persisted progress; job events from
  `run_job` (`app/services/mod.rs:69-117`) are the available runtime channel.
- Prior moves to mirror: `app/services/{storage,host_tools,offline}` — `#[derive(Clone)]`
  service struct, `async` methods, `AppError` only, blocking work via `run_blocking`/`run_job`.

## 1. Taps: add / dedupe / remove / persistence

### 1.1 Add + dedupe (backend)

- `community_add_tap` loads settings, pushes the raw tap, saves (`tauri/commands/community.rs:74-83`).
  **No validation or normalization happens in the shell** — an empty/whitespace URL is persisted.
  Validation (`normalize_subscription`) only runs later at workspace resolution
  (`core/community/taps/validation.rs:56-86`): trim URL, reject embedded whitespace, scheme
  allow-list (`https://`, `ssh://git@`, `git@…`, absolute `/…`; rejects `file://`, `git://`,
  relative), trim branch (empty → `None`, `validate_branch_name`: no leading `-`, ≤200 chars,
  `[A-Za-z0-9/._-]`), trim pinned commit (empty → `None`). Note pinned SHA is **not** validated
  in `normalize_subscription`; `is_valid_git_sha` (7–64 hex) is only enforced at checkout
  (`core/community/taps/git.rs:103-125`).
- Shell dedupe is exact-triple equality on raw values (`community.rs:18-34`): `(url, branch,
pinned_commit)` with `Option` compared literally. Consequences to preserve or deliberately fix:
  - `branch: None` ≠ `Some("main")`, although both resolve to branch `main`
    (`core/community/taps/types.rs:9,51-60`) and the _same workspace directory_
    (`directory_name()` slugifies URL + branch-if-present, `types.rs:25-40` — so `None` and
    `Some("main")` actually produce **different** directory names: slug vs slug-`main`).
  - `" url "` ≠ `"url"`, `branch: Some("")` ≠ `None`. The frontend normalizes first
    (`normalizeTap` trims, drops empties; `src/hooks/useCommunityProfiles.ts:186-224`), so the
    shipped UI never sends those, but the IPC contract does not guarantee it.
- Recommendation: service dedupes **after** `normalize_subscription`, on the normalized triple.
  That is a strict tightening (fewer persisted duplicates), not a behaviour change for UI-sent data.
- Persistence order (`community.rs:36-63` + `core/settings/store.rs:136-155`): `load()` →
  clone taps → push → `SettingsStore::update` (process-local lock, reloads current file,
  mutator sets `community_taps = dedupe_taps(taps)`, `preserve_extra_from(current)` so unknown
  TOML fields survive, writes atomically, file only written when mutator returns `Ok`).
  Returned list has every tap's `extra` cleared. Preserve: (a) extras of _other_ settings
  sections preserved, (b) returned taps extra-free, (c) no write on error.

### 1.2 Remove / pin / unpin — there is no backend command

- Issue scope says "tap add and remove", but **no `community_remove_tap` exists**. Remove, pin
  and unpin are frontend-only: filter/map by normalized identity key
  `` `${url}::${branch ?? ''}::${pinned_commit ?? ''}` `` then whole-settings
  `settings_load` → `settings_save` (`useCommunityProfiles.ts:301-375`). Pin uses the last
  synced `head_commit` keyed by `url::branch` only (`tapSyncKey`) and errors
  `"No synced commit found for this tap. Sync taps first, then pin."` when unknown.
- Remove does **not** delete the workspace clone, `community_taps`/`community_profiles` rows or
  `community_tap_offline_state`: the SQLite catalog/indexed list keep showing a removed tap
  until some later re-index. Preserve this (or flag as a deliberate fix — silently purging DB
  rows in the new service would change Discover/Browse behaviour).
- The service should own remove/pin/unpin semantics (normalized identity match, dedupe after
  mutation, same persistence order as §1.1); the frontend currently re-implements them three times.

## 2. `community_sync`: sequence, failure, backfills

### 2.1 Exact sequence today (`tauri/commands/community.rs:219-258`)

1. Load taps from settings.
2. `tap_store.sync_many(&taps)` — **sequential, first error aborts everything**
   (`core/community/taps/store.rs:73-84`: `results.push(self.sync_tap(subscription)?)`).
   Side effects of earlier taps (clones/resets) persist, but the caller gets `Err` and
   steps 3–5 never run for _any_ tap.
3. Per result, in order, **warn-only** (`tracing::warn!`, sync still succeeds):
   `metadata_store.index_community_tap_result(result)` then
   `upsert_community_tap_offline_from_sync_result(result)`.
4. Second loop — the `last_sync_at` **backfill**: `lookup_community_tap_id(url, branch ?? "")`
   → `lookup_community_tap_offline_state_row(tap_id)` → copy `row.last_sync_at` into
   `result.last_sync_at` (core always constructs it as `None`,
   `core/community/taps/store.rs:137-144` / `types.rs:74-79`).
5. Clear `extra` on every returned subscription.

- Ordering dependency: the offline upsert is a **no-op when the tap has no `community_taps`
  row yet** (`core/metadata/community_ops.rs:40-72`), i.e. indexing must run first. Both use
  `with_conn_mut`, which silently no-ops on a disabled store — so with the DB down, sync still
  succeeds git-only, step 4 finds nothing (`Ok(None)`), `last_sync_at` stays `None`.

### 2.2 Per-tap git behaviour (`core/community/taps/store.rs:102-145`, `git.rs`)

- Workspace missing → `git clone --branch <branch|main> --single-branch <url> <path>`, then
  `checkout --detach <pinned>` if pinned. Clone failure is a hard `Err`.
- Workspace exists → pinned ? `fetch_and_checkout_pinned` : `fetch_and_reset`
  (`fetch --prune origin -- <branch>`; `reset --hard FETCH_HEAD`; `clean -fdx`;
  pinned replaces reset with `checkout --detach`). **Any** error in that chain with the path
  still existing → `CachedFallback`, `from_cache = true`; otherwise hard `Err`. (So even a
  `reset`/`clean` failure degrades to cache, and a non-git directory at the path degrades
  until `rev_parse_head` then fails hard.)
- Then unconditionally `rev-parse HEAD` and filesystem indexing (`index_tap`). A malformed
  `community-profile.json` is a hard `Json` error (`core/community/index.rs:187-200`) and
  therefore aborts the whole multi-tap sync; unsupported `schema_version` and unparseable
  `trainer-sources.json` only add diagnostics / warn-and-skip (`index.rs:170-185,138-168`).
- Git runs through `host_std_command_with_env` on Flatpak, else `std::process::Command`
  (`git.rs:26-38`), with `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`,
  `GIT_TERMINAL_PROMPT=0`, 1 KB/s/30 s low-speed abort. All of this blocks the caller thread
  today (`pub fn`, not async) — the issue's first acceptance criterion.
- Job-move recommendation [INFERENCE]: wrap the whole sequence in
  `run_job(jobs, JobSpec::new("community-sync"), …)` **without an exclusive `JobKey`** (UI +
  any future startup caller must not see spurious `Conflict`), and serialize taps with a
  service-held mutex instead — two concurrent syncs would otherwise race `reset --hard`/
  `clean -fdx` on the same workspace. Cancellation caveat: `run_job` cannot interrupt the
  blocking closure; cancellation resolves the caller as `Cancelled` while git finishes in the
  background (`app/services/mod.rs:69-83`). "Sync can be cancelled" is therefore satisfied at
  the caller level only, unless the loop checks a token between taps — `run_job`'s `work`
  closure receives no `JobContext`, so a between-taps check needs a shared `AtomicBool`/token
  captured into the closure. Decide in B0; do not claim per-tap abort without it.

### 2.3 What "backfill" means, precisely

Two distinct things are both called backfill; do not conflate them:

1. **`trainer_loading_mode` (v25)** — there is _no_ SQL UPDATE backfill. Migration 24→25
   adds the nullable column and resets watermarks:
   `ALTER TABLE community_profiles ADD COLUMN trainer_loading_mode TEXT; UPDATE community_taps SET last_head_commit = NULL;`
   (`core/metadata/migrations/v24_v25.rs:4-10`). The next `community_sync` then misses the
   watermark skip (`core/metadata/community_index/indexing.rs:56-60`) and re-indexes,
   DELETE+INSERT-ing rows with `trainer_loading_mode = manifest.profile.trainer.loading_mode.as_str()`
   (`indexing.rs:158-180`). Before that sync the column is `NULL` → catalog buckets it as
   `"unknown"` (`core/discovery/catalog.rs:45-85`). Re-index is transactional, recomputes
   `profile_count`, and skips entries violating A6 field bounds with a warning.
2. **`last_sync_at`** — written by the offline upsert: `from_cache` → preserve the previous
   row's `last_sync_at` (possibly `None` → writes NULL); otherwise `Utc::now().to_rfc3339()`
   (`community_ops.rs:52-69`). Always `has_local_clone = 1`, `clone_size_bytes = directory_size_bytes(workspace)`;
   upsert overwrites all three columns on conflict (`core/metadata/offline_store.rs:213-233`).
   The shell then reads the row back into the IPC result (§2.1 step 4) — the frontend's
   "Cached data … Last successful sync" banner keys off `from_cache` + `last_sync_at`
   (`src/components/CommunityBrowser.tsx:89-105`).

## 3. Indexed list & catalog shapes; stale markers

- `community_list_indexed_profiles` → `list_community_tap_profiles(None)`: **all** rows joined
  with `community_taps`, **no ORDER BY** (order unspecified — do not promise one)
  (`core/metadata/community_index/queries.rs:12-80`). Row shape
  (`core/metadata/models.rs:352-371`): `id, tap_id, tap_url, relative_path, manifest_path,
game_name/game_version/trainer_name/trainer_version/proton_version/compatibility_rating/
author/description/platform_tags/trainer_loading_mode: Option<String>, schema_version: i64,
created_at`. **No stale/degraded marker on rows.** Disabled store → silently `Ok([])`
  (`with_conn`), unlike the catalog (below). Watermark: re-sync with unchanged HEAD keeps old
  rows, including for profiles deleted upstream.
- Filesystem index (`community_list_profiles`, `core/community/index.rs`): recursive walk
  skipping dot-directories, only files named exactly `community-profile.json` (+
  `trainer-sources.json` collection), sorted by `(game_name, manifest_path)`; missing workspace
  → diagnostic `"tap '<url>' is not synced yet"`, not an error.
- Discovery catalog (`discovery_catalog`, `tauri/commands/discovery.rs:12-30`): DB path
  `query_community_catalog` uses `with_sqlite_conn` and **errors** on a disabled store
  (pinned by `core/metadata/community_ops.rs:99-107` test) → shell warns and falls back to
  `degraded_catalog_from_taps`, which per-tap resolves/indexes from disk, warn-skips bad taps
  (partial catalog, never blank), same pure `build_catalog_page` with `degraded: true`
  (`core/metadata/community_index/catalog.rs:236-275`). `CatalogPage`
  (`core/discovery/catalog.rs:87-97`): `entries, facets, total_count` (post-filter,
  pre-pagination), `tap_count` (distinct taps, unfiltered corpus), `degraded`.
  `CatalogEntry.id` is `None` in degraded mode and is "NOT a list key". Pagination clamps:
  default limit 50, max 200 (`catalog.rs:12-14`).

## 4. Community import/export

### 4.1 Containment — the two validators diverge, and it matters

- `community_import_profile`: `validate_import_path_in_workspace` (`community.rs:186-217`) —
  canonicalize path, require `canonical.starts_with(canonicalized workspace root)` for **any**
  known tap. No file/extension check (core parsing rejects later). This is the strict check
  the issue wants factored into `ensure_within_workspace(root, candidate)`.
- `community_prepare_import`: `validate_import_path_for_prepare` (`community.rs:140-184`) —
  canonicalize, must be a file, extension `.json` case-insensitive, then computes
  `_is_workspace_path` and **discards it**: any local file is accepted, deliberately, for the
  file-picker flow. Consequences:
  - Applying the strict check to prepare would break file-picker imports. The spec's "use it
    for every import path" must be read as every _workspace_ import path; prepare keeps
    existence + `.json`, only the tap-manifest entry point
    (`CommunityBrowser.tsx:193`, `entry.manifestPath`) exercises workspace containment.
  - The Browser draft flow never calls `community_import_profile`: prepare → user edits →
    `profile_save` (`CommunityBrowser.tsx:298-308`). `community_import_profile` is only used by
    `TrainerDiscoveryPanel` (`src/hooks/useImportCommunityProfile.ts:4-8`).
  - Both validators canonicalize the candidate, and canonicalize each workspace root per call
    (a root that fails to canonicalize is silently non-matching: `.map(..).unwrap_or(false)`).
    `starts_with` on canonicalized paths already defeats `..` and symlink escapes; the new
    helper must keep canonicalize-both-sides (per-AC: `..`, absolute-outside, symlink-out,
    case/Unicode tricks, accept nested valid). Canonicalization also requires the candidate
    to _exist_ — preserve "cannot resolve community profile path" error ordering.
- Import itself (`ProfileWriter::import_community`, `app/services/profiles/imports.rs:47-137`):
  caller "has already checked that `path` is allowed". Core import derives name, copies
  `manifest.metadata.trainer_sha256` (trimmed, non-empty) into
  `profile.trainer.community_trainer_sha256`, saves, then writer effects: observe
  (`SyncSource::Import`) → `Import` revision → health invalidation → display-only version
  snapshot seed (`game_version → human_game_ver`, trainer version; only if either non-empty).

### 4.2 Name derivation & collisions — community overwrites, Lutris doesn't

- Community: `derive_import_name` (`core/profile/exchange/utils.rs:191-204`) — sanitized
  `metadata.game_name` (alphanumeric → lowercase, runs of separators → `-`, trimmed; empty →
  sanitized parent-directory name → fallback `"community-profile"`). **No uniquification**:
  `ProfileStore::save` overwrites a same-name profile ("overwriting a profile of the derived
  name", `imports.rs:40-46`). Preview and import derive the same name; preview also returns
  `required_prefix_deps` (= `profile.trainer.required_protontricks`) and the _hydrated_
  profile (effective profile, hooks force-disabled — `utils.rs:55-80`).
- Validation: root must be a JSON object with integer `schema_version` ≤ current, `metadata`
  must contain 9 named fields, `profile` must contain `game/trainer/injection/steam/launch`
  (`core/profile/exchange/validation.rs:4-45`).
- Export (`community_export_profile`, `community.rs:95-103`; `core/profile/exchange/export.rs:9-26`):
  only the shell trims `profile_name`. Metadata is built from the on-disk profile **before**
  sanitizing; export strips to `portable_profile()`, clears extras, hook paths + disables all
  hooks, `injection.dll_paths`/`inject_on_launch`, launcher icon, runtime proton path/working
  dir, custom art paths, and drops pre/post hooks entirely (`utils.rs:23-53`). `write_manifest`
  creates parent dirs, writes pretty JSON with `fs::write` — **not atomic, no containment on
  `output_path`** (renderer/save-dialog supplied; the service must not add workspace
  containment here).

### 4.3 Lutris (`tauri/commands/lutris.rs`, `core/profile/lutris_import/import.rs`)

- Prepare: `directory: Option<String>` — `Some` non-directory is treated as `None` →
  auto-discover; not found is **not** an error: `Ok(preview { entries: [], lutris_root: None,
diagnostics: ["Lutris library directory was not found."] })`. `pga.db` read failure → diagnostic
  - continue with YAML only; unparseable config → `"Skipped '<path>': …"` diagnostic; non-Wine/Proton
    runners silently skipped. Preview suggests unique names against existing + pending entries.
- Import (`ProfileWriter::import_lutris`): per entry — not importable → `Skipped`; name
  validation (trim-aware, before uniqueness) or save failure → `Failed`; else `Imported`.
  Never fails wholesale. Uniqueness (`derive_unique_profile_name`, `import.rs:219-247`):
  `base`, `base-2 … base-1000`, `base-copy`, `base-copy-2 …`, `base-copy-1001` fallback;
  case-sensitive comparison; apply re-derives against stored + batch-reserved names.
  Effects per imported entry: observe (`SyncSource::LutrisImport`) → revision → health.

## 5. Discovery: external search, version compat, sources

- `discovery_search_external`: empty/whitespace `game_name` or zero enabled sources → empty
  response with `offline: false` (`core/discovery/client/search.rs:23-54`). Per source,
  bounded concurrency 4, 3-stage: fresh cache (`external_cache_entries`, key
  `trainer:source:v1:{source_id}:{query lowercased, spaces→_}`, TTL **1 h**) → live fetch
  (10 s timeout, ≤1 MB, XML/RSS content-type required) → stale cache (marked `is_stale`) →
  `offline_response` (`results: [], offline: true`). Merged results sort relevance desc,
  then source/game_name/url. Aggregate `offline = all_offline && any_offline`,
  `cached`/`is_stale` = any, `cache_age_secs` = first cached. Response is a value, **never an
  IPC error** — see §7 for the spec tension this creates.
- `discovery_check_version_compatibility`: unknown profile → `Err("unknown profile: {name}")`
  (exact string); no snapshot / no `human_game_ver` → `Unknown`
  (`core/discovery/matching.rs:183-200`). Equal-after-trim → `Exact`; one dot-segment list a
  true prefix of the other → `Compatible`; otherwise `Outdated`. `NewerAvailable` exists in
  the enum but this function never returns it. Advisory only.
- External sources (`discovery.rs:73-135`, `core/discovery/models.rs:116-189`): list returns
  settings list extra-cleared; add validates (`source_id` 1–64 `[A-Za-z0-9_-]`, display name
  ≤128 non-blank, `base_url` HTTPS with host, `source_type` ∈ `wordpress_rss`), clears
  `source.extra`, rejects duplicate `source_id` (exact match) **inside** `SettingsStore::update`
  so no write occurs (`"source with id {:?} already exists"`); remove retains by id, errors
  `"no source with id {:?} found"` when absent. Both return the full list, extras cleared.
  Default source FLiNG is only a settings default, not synthesized by these commands.

## 6. ProtonDB: lookup, suggestions, dismiss (`tauri/commands/protondb.rs`)

- `protondb_lookup` **never errors** (`Ok(lookup_protondb(...))`, force_refresh default false).
  `normalize_app_id`: trim; empty → default result. Cache: `external_cache_entries`, key
  `protondb:{app_id}`, TTL **6 h** (`core/protondb/client.rs:22`). Fresh hit → `Ready`,
  `from_cache: true, is_stale: false, is_offline: false`. Live: summary JSON (404 → `NotFound`),
  recommendations failure degrades to a message group while tier still returns `Ready`.
  Live failure → stale cache → `Stale` (`is_offline: true`); no cache → `Unavailable`,
  `snapshot: None`, `cache.is_offline: true`. 6 s request timeout. Cache persist is best-effort
  (warn only). Disabled DB → cache reads return `None`, so every lookup goes live.
- `protondb_get_suggestions`: lookup first, **then** profile load errors propagate; dismissed
  keys only when DB available _and_ profile has a metadata id, else empty set (no error).
  `derive_suggestions` (`core/protondb/suggestions.rs:134-…`): no snapshot → empty set;
  statuses `Dismissed` (key = catalog entry id for env matches, env key otherwise) >
  `AlreadyApplied` > `Conflict` (same env key, different value) > `New`.
- `protondb_dismiss_suggestion`: trims name, empty → `"profile_name is required"`; profile
  must load; must have a metadata id else
  `"profile '<n>' has no metadata record — cannot dismiss suggestion"`. Retention **30 days**
  (`protondb.rs:13`), upserted via `dismiss_suggestion`; reads exclude expired rows
  (`core/metadata/suggestion_store.rs:10-70`). `evict_expired_dismissals` exists but has **no
  caller** — expired rows linger, filtered at read. Don't add eviction as a side effect.
- Accept already lives in `ProfileWriter` (M1-06); only lookup/suggestions/dismiss move.

## 7. Offline behaviour: today vs spec

| Flow                   | Today                                                                | Spec target                                                         |
| ---------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `community_sync`       | Attempts git; fetch failure + local clone → `CachedFallback` success | `offline_mode` → `Unavailable` (no git at all)                      |
| External search        | Soft `offline: true` value on total failure; stale cache renders     | "report `Unavailable`" offline                                      |
| Indexed list / catalog | No offline check; DB or degraded-from-disk renders                   | Cached lists still render, stale markers (`degraded`, `from_cache`) |
| ProtonDB               | Cache-first, stale on failure — already renders offline              | Unchanged; cached data still renders                                |

Decisions needed in B0 [INFERENCE from spec wording]: (a) sync gate = `AppErrorKind::Unavailable`
returned before any git work when `settings.offline_mode`; (b) for external search, a thrown
`Unavailable` conflicts with the existing soft-value contract (`useExternalTrainerSearch`
nulls data on throw) and would hide stale-cache results the core flow could have returned —
recommend gating _before live fetch but after fresh/stale cache_, or keep the value contract
and map `offline_mode` to the all-offline response; either way this is a frontend-visible
choice, not a mechanical port. Do **not** gate ProtonDB lookup/catalog/indexed list.

## 8. Existing tests pinning this behaviour

| Area                          | Tests                                                                                                                                                                                         |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tap sync/git/validation       | `core/community/taps/tests.rs` — local-repo sync+index (`:96`), pinned stays (`:262`), SHA/branch/URL validation (`:168-424`), git env isolation (`:183`), offline availability (`:192`)      |
| Shell dedupe + IPC signatures | `tauri/commands/community.rs:309-339` (`dedupes_taps_by_url_and_branch`), `discovery.rs:137-181` — signature pins only                                                                        |
| Indexing/backfill             | `core/metadata/community_index/tests.rs` (13) incl. `index_backfills_trainer_loading_mode` (`:341`); `community_index_tests.rs`; `catalog.rs` tests (degraded rows, NULL loading mode `:414`) |
| v25 migration                 | `core/metadata/migrations/tests/v24_v25.rs` — column added, watermarks reset                                                                                                                  |
| Offline upsert                | `core/metadata/offline_*` tests; disabled-catalog-errors pin `community_ops.rs:99-107`                                                                                                        |
| Exchange                      | `core/profile/exchange/mod.rs` (8: export→import roundtrip etc.), `lutris_import/import.rs` tests (unique names, skip/fail)                                                                   |
| Discovery                     | `core/discovery/client/tests.rs` (10: cache/stale/offline), `matching.rs` tests, `catalog.rs` tests                                                                                           |
| ProtonDB                      | `core/protondb/tests.rs` (15), suggestions tests, `suggestion_store` via metadata tests                                                                                                       |
| Frontend                      | `CommunityBrowser.test.tsx`, `TrainerDiscoveryPanel.test.tsx`, mock handlers `src/lib/mocks/handlers/community.ts`; smoke = Playwright (`package.json:19`)                                    |

## 9. Gaps the move must close (acceptance criteria)

1. **Containment**: zero tests for `validate_import_path_*` today (issue states this). New
   `ensure_within_workspace` table tests: `..` traversal, absolute outside, symlink resolving
   outside, case/Unicode-normalization tricks, nested valid accept — against _multiple_
   workspace roots, plus prepare's divergent file-picker path pinned as intentionally looser.
2. **Sync as a job**: no test covers shell sync orchestration — per-tap failure abort,
   index-before-offline-upsert ordering, warn-only index failure, `from_cache` preserving
   `last_sync_at`, disabled-DB git-only success, cancellation surfacing as `Cancelled`.
   Issue test plan's local bare-repo fixture should extend `taps/tests.rs:15-95` helpers.
3. **Tap dedupe**: only the raw-triple case is pinned. Add normalized cases
   (whitespace URL, `""` vs `None` branch, `None` vs `Some("main")` decision) and add/remove
   round-trips through settings persistence (extras preserved, no write on error).
4. **Backfill**: `trainer_loading_mode` re-index and migration watermark reset are pinned;
   `last_sync_at` backfill (fresh vs `from_cache` vs no prior row) is **not** pinned anywhere.
5. **Remove/pin/unpin** have no backend to test — new service tests must pin identity matching
   and the "DB rows/workspace survive removal" behaviour.
6. **Offline gating** (§7) has no precedent tests; add service tests for `offline_mode` sync
   refusal and the chosen external-search contract, plus cached catalog/ProtonDB still rendering.
7. Community import **overwrite-on-collision** vs Lutris uniquification are pinned only in
   core; add one service-level pin each so the move cannot silently unify them.
