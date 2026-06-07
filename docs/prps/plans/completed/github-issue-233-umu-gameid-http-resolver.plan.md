# Plan: GitHub Issue 233 - umu GAMEID HTTP Resolver and SQLite Cache

## Summary

Implement the optional online umu GAMEID resolver requested in GitHub issue
[#233](https://github.com/yandy-r/crosshook/issues/233). When a profile launches
through `proton_run`/`umu-run` and does not already have a user-provided
`runtime.umu_game_id` or Steam app id, CrossHook can, only when explicitly enabled
in settings, resolve `(store, codename)` through the official umu-database HTTP
API and cache the result in SQLite for seven days.

The resolver must be non-blocking from the user's perspective: cache hits are used
without network I/O, concurrent misses for the same key are deduplicated, network
or parse failures fall back to `umu-0`, and launch must never fail because the
remote service is unavailable. The resolved GAMEID is runtime-only launch state;
it is never written back into profile TOML. Launch Preview must show which GAMEID
will be used and whether it came from a user override, Steam id, HTTP cache,
fresh HTTP lookup, stale cache fallback, or `umu-0`.

Current checkout note: CrossHook already contains `crosshook_core::umu_database`
CSV refresh and coverage code plus an existing global `UmuPreference`. This plan
adds a separate opt-in GAMEID lookup setting and does not replace the existing CSV
coverage flow.

## User Story

As a CrossHook user launching non-Steam games through umu, I want CrossHook to
optionally map a store/codename pair to the correct upstream umu GAMEID, cache
that answer, and show it in Launch Preview, so that non-Steam profiles can receive
the same protonfix targeting as manually configured `runtime.umu_game_id` values
without making every launch depend on the network.

## Metadata

- **Complexity**: Large. The feature spans settings TOML, profile TOML, SQLite
  metadata, launch request enrichment, backend preview, Tauri IPC, frontend
  profile/settings/preview UI, mocks, and tests.
- **New dependency**: None expected. Reuse existing `reqwest`, `rusqlite`,
  `tokio`, `serde`, and frontend test tooling. Do not add a future-sharing crate
  unless implementation proves the existing `tokio::sync` primitives cannot cover
  in-flight dedupe cleanly.
- **Source PRD**: [`docs/prps/prds/umu-launcher-migration.prd.md`](../prds/umu-launcher-migration.prd.md)
- **Tracking issue**: [#233](https://github.com/yandy-r/crosshook/issues/233)
- **Mode**: `--parallel --no-worktree --enhanced`. Stay in the current checkout.
  Do not create a git worktree.
- **Estimated Files**: 35-45 files including new backend modules, migrations,
  tests, frontend types/components, mocks, and schema reference docs.

## Batches

Tasks are dependency-batched for parallel implementation. Tasks inside a batch can
run concurrently when they own disjoint files; batches should run in order.

| Batch | Tasks              | Depends On | Parallel Width | File Ownership Summary                                                                |
| ----- | ------------------ | ---------- | -------------- | ------------------------------------------------------------------------------------- |
| B1    | 1.1, 1.2, 1.3, 1.4 | none       | 4              | Foundation: SQLite cache, settings opt-in, profile hints, launch request/runtime DTOs |
| B2    | 2.1, 2.2           | B1         | 2              | HTTP API client and resolver orchestration                                            |
| B3    | 3.1, 3.2, 3.3      | B2         | 3              | Backend launch enrichment, command environment parity, preview diagnostics            |
| B4    | 4.1, 4.2, 4.3      | B1, B3     | 3              | Frontend settings, profile editor, launch preview UI                                  |
| B5    | 5.1, 5.2           | B4         | 2              | Browser mocks/fixtures and schema reference docs                                      |
| B6    | 6.1, 6.2, 6.3      | B1-B5      | 3              | Rust tests, frontend tests, validation                                                |

- **Total tasks**: 17
- **Total batches**: 6
- **Max parallel width**: 4

## UX Design

### Settings

Add a Runner setting for the new feature, separate from the existing runner
preference:

```text
Settings -> Runner

  umu launcher preference: [Auto | umu | Proton]
  umu GAMEID lookup:      [Disabled | Enabled]
```

Default is Disabled. Disabled means preview and launch never perform the new
store/codename HTTP lookup. Existing CSV refresh behavior remains unchanged.

Add a Settings Advanced action:

```text
Settings -> Advanced

  [Clear umu GAMEID lookup cache]
  Last cleared / status text only after the action runs.
```

The clear action deletes only rows owned by the GAMEID lookup cache and must not
clear the existing CSV metadata/cache.

### Profile Runtime Section

Add optional fields under the existing Runtime section:

```text
Runtime

  umu store:    [gog]
  umu codename: [cyberpunk_2077]
```

These fields are only hints for the optional resolver. Empty strings are omitted
from TOML and do not make `RuntimeSection::is_empty()` return false unless a
trimmed value is present.

### Launch Preview

Extend the existing launch preview command details without changing copy behavior:

```text
umu GAMEID: UMU-12345
source: cache hit for gog/cyberpunk_2077
expires: 6 days
```

Fallback examples:

```text
umu GAMEID: umu-0
source: lookup disabled

umu GAMEID: umu-0
source: API unavailable, using fallback
```

The "Copy command" action continues copying only `preview.effective_command`.
It must not copy explanatory UI text.

## Storage And Persistence Plan

| Datum                                                                   | Layer              | Reason                                              | User Visibility                                         |
| ----------------------------------------------------------------------- | ------------------ | --------------------------------------------------- | ------------------------------------------------------- |
| `settings.umu_database_lookup`                                          | TOML settings      | User-editable opt-in preference; default Disabled   | Settings Runner control                                 |
| `profile.runtime.umu_store`                                             | Profile TOML       | User-editable store hint for non-Steam lookup       | Profile Runtime editor                                  |
| `profile.runtime.umu_codename`                                          | Profile TOML       | User-editable codename hint for non-Steam lookup    | Profile Runtime editor                                  |
| Cached `(store,codename)->umu_id` rows, timestamps, last error metadata | SQLite metadata DB | Operational cache/history; not user-authored config | Clearable from Settings Advanced; summarized in preview |
| Resolved GAMEID and resolution source for a launch/preview              | Runtime-only       | Derived launch state; must not mutate TOML          | Launch Preview only                                     |
| In-flight lookup dedupe map                                             | Runtime-only       | Process-local network suppression                   | Not user visible                                        |

Persistence and usability requirements:

- Add a SQLite migration from schema v23 to v24 for the GAMEID cache table.
- Preserve backward compatibility for existing settings/profile TOML files by
  using serde defaults.
- Offline behavior is cache-first: fresh cache hit succeeds; stale cache can be
  used as a degraded fallback; full miss falls back to `umu-0`.
- SQLite unavailable must degrade to no persistent cache and `umu-0`/fresh lookup
  behavior, not launch failure.
- Users can edit store/codename hints and clear resolver cache, but cannot edit
  individual cache rows through the UI.

## External Documentation

| Topic                | Source                                                                                                          | Key Takeaway                                                                                      |
| -------------------- | --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| umu-database project | `https://github.com/Open-Wine-Components/umu-database`                                                          | Official database for umu protonfix GAMEID mapping.                                               |
| API endpoint         | `https://github.com/Open-Wine-Components/umu-database/blob/main/README.md#current-available-database-endpoints` | Query `umu_api.php?store=...&codename=...`; response is JSON and may be an empty array on misses. |
| API implementation   | `https://github.com/Open-Wine-Components/umu-database/blob/main/umu.openwinecomponents.org/umu_api.php`         | Server reads lowercase `store` and `codename`; non-GET is rejected.                               |
| Open Wine Components | `https://openwinecomponents.org/`                                                                               | Confirms upstream ownership context for umu components.                                           |

Implementation implications:

- Build URLs with a URL API and query pairs, not string concatenation.
- Treat `200 []` as "not found" and fall back to `umu-0`.
- Do not assume documented rate limits, authentication, pagination, or response
  versioning exist.
- Keep CrossHook's own guardrails: opt-in setting, TTL, dedupe, timeout, and
  cache-first behavior.

## Mandatory Reading

Read these files before implementation. Line numbers are intentionally omitted
because this branch already contains active umu work; use the file and symbol
names as the contract.

| Priority | File                                                                                    | Why                                                                                    |
| -------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| P0       | `src/crosshook-native/crates/crosshook-core/src/umu_database/mod.rs`                    | Existing module boundary for umu CSV refresh/coverage; add resolver exports here.      |
| P0       | `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs`                 | Existing reqwest timeout, user agent, cache metadata, and test URL override patterns.  |
| P0       | `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs`               | Existing `(store,codename)` normalization for CSV coverage.                            |
| P0       | `src/crosshook-native/crates/crosshook-core/src/metadata/store.rs`                      | `MetadataStore` connection helpers and disabled/in-memory modes.                       |
| P0       | `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`                | Generic cache TTL semantics; note that expired rows are hidden by normal read helpers. |
| P0       | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/mod.rs`             | Schema versioning and migration registration.                                          |
| P0       | `src/crosshook-native/crates/crosshook-core/src/settings/types.rs`                      | `AppSettingsData` and `UmuPreference` patterns.                                        |
| P0       | `src/crosshook-native/crates/crosshook-core/src/profile/models/runtime.rs`              | Runtime TOML fields and `is_empty()` behavior.                                         |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`               | IPC launch request DTOs and serde defaults.                                            |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs`            | Current GAMEID precedence and `umu-0` fallback logic.                                  |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`    | Game launch environment construction.                                                  |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs` | Trainer launch environment construction; parity with game launches is required.        |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/preview/types.rs`                | `UmuDecisionPreview` type definition.                                                  |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`              | Preview construction and umu decision population.                                      |
| P0       | `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`                       | Tauri game/trainer launch command flow.                                                |
| P0       | `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`                         | Tauri preview command flow.                                                            |
| P0       | `src/crosshook-native/src-tauri/src/commands/umu_database.rs`                           | Existing umu database command registration; add clear-cache command here.              |
| P0       | `src/crosshook-native/src/types/settings.ts`                                            | Frontend settings DTO mirror.                                                          |
| P0       | `src/crosshook-native/src/types/profile.ts`                                             | Frontend profile runtime DTO mirror.                                                   |
| P0       | `src/crosshook-native/src/types/launch.ts`                                              | Frontend launch preview DTO mirror.                                                    |
| P0       | `src/crosshook-native/src/components/settings/RunnerSection.tsx`                        | Runner settings UI.                                                                    |
| P0       | `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`               | Profile runtime editor UI.                                                             |
| P0       | `src/crosshook-native/src/components/library/launch/HeroLaunchCommandSection.tsx`       | Launch command/preview rendering.                                                      |
| P1       | `src/crosshook-native/src/hooks/useUmuDatabaseRefresh.ts`                               | Existing hook shape for umu database IPC.                                              |
| P1       | `src/crosshook-native/src/lib/mocks/handlers/umu_database.ts`                           | Browser mock command coverage.                                                         |
| P1       | `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                                 | Preview mock data.                                                                     |
| P1       | `AGENTS.md`                                                                             | Schema inventory and repository-specific reference material to update after migration. |
| P1       | `CLAUDE.md`                                                                             | Canonical repo rules/reference summary; keep schema version references synchronized.   |

## Patterns to Mirror

### URL Query Construction

Mirror `src/crosshook-native/crates/crosshook-core/src/discovery/client/fetch.rs`
by using a URL builder and `query_pairs_mut().append_pair(...)`.

```rust
let mut url = reqwest::Url::parse(UMU_API_URL)?;
url.query_pairs_mut()
    .append_pair("store", store.as_str())
    .append_pair("codename", codename.as_str());
```

Do not build the endpoint with `format!("{base}?store={store}&codename={codename}")`.

### Reqwest Client

Mirror `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs`:

```rust
static UMU_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn umu_http_client() -> &'static reqwest::Client {
    UMU_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("build umu API HTTP client")
    })
}
```

Keep the GAMEID lookup timeout shorter than the existing background CSV refresh.

### Metadata Store Split

Mirror the existing `metadata/*_store.rs` plus `metadata/*_ops.rs` pattern:

```rust
// metadata/umu_gameid_cache_store.rs
pub(crate) fn put_umu_gameid_cache_entry(...)
pub(crate) fn get_umu_gameid_cache_entry(...)
pub(crate) fn get_stale_umu_gameid_cache_entry(...)
pub(crate) fn clear_umu_gameid_cache(...)

// metadata/umu_gameid_cache_ops.rs
impl MetadataStore {
    pub fn put_umu_gameid_cache_entry(&self, ...)
    pub fn get_umu_gameid_cache_entry(&self, ...)
    pub fn get_stale_umu_gameid_cache_entry(&self, ...)
    pub fn clear_umu_gameid_cache(&self) -> Result<usize>
}
```

Do not force this feature through `external_cache_entries` if stale fallback
requires different semantics than `get_cache_entry()`.

### Serde Default Pattern

Mirror `UmuPreference` and `RuntimeSection` field defaults in
`settings/types.rs` and `profile/models/runtime.rs`. New settings/profile fields
must be optional by default for existing TOML files.

### Backend-Only Enrichment Pattern

The frontend may send profile hints and settings, but it must not be trusted to
send a resolved GAMEID. Populate resolution details on the backend after IPC
deserialization, either by adding a backend-only field with `skip_deserializing`
or by introducing a narrow enriched request wrapper used by the launch and preview
builders.

### Browser Mock Pattern

Mirror command handlers in `src/crosshook-native/src/lib/mocks/handlers/*.ts` so
browser dev mode and CI tests exercise the same `callCommand()` path as native
Tauri. Do not use raw frontend HTTP for the umu API.

## Files to Change

| Action | File                                                                                              | Purpose                                                                                           |
| ------ | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| CREATE | `src/crosshook-native/crates/crosshook-core/src/umu_database/api_client.rs`                       | Official HTTP API client for `umu_api.php`; URL construction, timeout, response parsing.          |
| CREATE | `src/crosshook-native/crates/crosshook-core/src/umu_database/resolver.rs`                         | Cache-first resolver, stale fallback, opt-in gate, in-flight dedupe, and public resolution types. |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/umu_database/mod.rs`                              | Export resolver/client APIs.                                                                      |
| CREATE | `src/crosshook-native/crates/crosshook-core/src/metadata/umu_gameid_cache_store.rs`               | SQL helpers for typed GAMEID lookup cache rows.                                                   |
| CREATE | `src/crosshook-native/crates/crosshook-core/src/metadata/umu_gameid_cache_ops.rs`                 | `MetadataStore` facade methods for resolver and clear-cache command.                              |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`                                  | Register new metadata modules and public facade methods.                                          |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/mod.rs`                       | Bump schema version to 24 and add the new table/indexes.                                          |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/settings/types.rs`                                | Add opt-in enum/field for `umu_database_lookup`, default Disabled.                                |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/profile/models/runtime.rs`                        | Add `umu_store` and `umu_codename` runtime hints.                                                 |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`                         | Add runtime hints and backend-only resolved GAMEID state used by preview/launch.                  |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`                     | Validate/canonicalize store/codename length, trimming, and control characters.                    |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs`                      | Preserve GAMEID precedence and `umu-0` fallback semantics.                                        |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`              | Use backend-resolved GAMEID and STORE for game launch env.                                        |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs`           | Apply the same GAMEID/STORE behavior for trainer launches.                                        |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/preview/types.rs`                          | Extend `UmuDecisionPreview` with GAMEID resolution diagnostics.                                   |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`                        | Populate preview diagnostics from resolver output.                                                |
| MODIFY | `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`                                 | Resolve GAMEID before `launch_game` and `launch_trainer`.                                         |
| MODIFY | `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`                                   | Resolve GAMEID before `preview_launch`.                                                           |
| MODIFY | `src/crosshook-native/src-tauri/src/commands/umu_database.rs`                                     | Add `clear_umu_gameid_lookup_cache` command.                                                      |
| MODIFY | `src/crosshook-native/src-tauri/src/lib.rs`                                                       | Register any new command handler.                                                                 |
| MODIFY | `src/crosshook-native/src/types/settings.ts`                                                      | Mirror new opt-in setting enum/field.                                                             |
| MODIFY | `src/crosshook-native/src/types/profile.ts`                                                       | Mirror runtime `umu_store` and `umu_codename`.                                                    |
| MODIFY | `src/crosshook-native/src/types/launch.ts`                                                        | Mirror `UmuDecisionPreview` GAMEID diagnostics.                                                   |
| MODIFY | `src/crosshook-native/src/utils/launch.ts`                                                        | Include runtime hints when building launch requests.                                              |
| MODIFY | `src/crosshook-native/src/hooks/profile/profileNormalize.ts`                                      | Normalize optional runtime fields.                                                                |
| MODIFY | `src/crosshook-native/src/hooks/profile/`                                                         | Update the empty-profile helper so new runtime fields initialize consistently.                    |
| MODIFY | `src/crosshook-native/src/components/settings/RunnerSection.tsx`                                  | Add opt-in control for `umu_database_lookup`.                                                     |
| CREATE | `src/crosshook-native/src/components/settings/AdvancedSettingsSection.tsx`                        | Add clear-cache action and status handling.                                                       |
| MODIFY | `src/crosshook-native/src/components/SettingsPanel.tsx`                                           | Render the new Advanced section and wire save/update handlers.                                    |
| MODIFY | `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`                         | Add store/codename text inputs.                                                                   |
| MODIFY | `src/crosshook-native/src/components/library/launch/HeroLaunchCommandSection.tsx`                 | Render GAMEID/source/cache details near the command block.                                        |
| MODIFY | `src/crosshook-native/src/utils/launchPreviewPresentation.ts`                                     | Add presentation helpers for resolution source labels.                                            |
| MODIFY | `src/crosshook-native/src/hooks/useUmuDatabaseRefresh.ts`                                         | Add clear-cache command wrapper or split into a small dedicated hook if cleaner.                  |
| MODIFY | `src/crosshook-native/src/lib/mocks/handlers/settings.ts`                                         | Include default disabled lookup setting.                                                          |
| MODIFY | `src/crosshook-native/src/lib/mocks/handlers/profile.ts`                                          | Include profile runtime hint defaults if fixtures require it.                                     |
| MODIFY | `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                                           | Include resolver source scenarios in preview mock output.                                         |
| MODIFY | `src/crosshook-native/src/lib/mocks/handlers/umu_database.ts`                                     | Mock `clear_umu_gameid_lookup_cache`.                                                             |
| CREATE | `src/crosshook-native/crates/crosshook-core/tests/umu_gameid_resolver_integration.rs`             | Cache, API, fallback, TTL, dedupe, and disabled-mode coverage.                                    |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/settings/tests.rs`                                | Settings default/serde roundtrip for Disabled.                                                    |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/profile/models/tests/runtime_section.rs`          | Runtime TOML roundtrip and `is_empty()` behavior.                                                 |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/request/tests/serde_roundtrip.rs`          | Launch request serde coverage.                                                                    |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/request/tests/method_validation.rs`        | Launch request validation coverage.                                                               |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/preview/tests/environment.rs`              | Preview diagnostics coverage.                                                                     |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_game_umu.rs`    | Game launch GAMEID/STORE env parity.                                                              |
| MODIFY | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_trainer_umu.rs` | Trainer launch GAMEID/STORE env parity.                                                           |
| MODIFY | `src/crosshook-native/src/components/__tests__/SettingsPanel.test.tsx`                            | Settings section inventory and clear-cache UI coverage.                                           |
| MODIFY | `src/crosshook-native/src/components/library/__tests__/HeroLaunchCommandSection.test.tsx`         | Preview rendering coverage.                                                                       |
| MODIFY | `src/crosshook-native/src/components/library/__tests__/HighlightedCommandBlock.test.tsx`          | Copy behavior regression coverage.                                                                |
| MODIFY | `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`                         | Add profile field coverage near implementation if no existing focused test exists.                |
| MODIFY | `./AGENTS.md`                                                                                     | Update SQLite schema version/table inventory after migration.                                     |
| MODIFY | `./CLAUDE.md`                                                                                     | Keep canonical schema and version references synchronized with AGENTS.md.                         |

## Not Building

- No broad game metadata search or autocomplete UI.
- No always-on startup API lookup fan-out for store/codename pairs.
- No direct frontend HTTP calls to `umu.openwinecomponents.org`.
- No override of user-provided `runtime.umu_game_id`.
- No resolver attempt when a Steam id is already available.
- No generated GAMEID writes back into profile TOML.
- No binary/blob cache in SQLite.
- No clear-all-metadata action.
- No changes to the existing CSV coverage warning other than coexistence with
  the new resolver diagnostics.
- No exported-launcher retrofit unless existing request-building code requires
  type updates for compilation; exported launchers should continue documenting
  current behavior and not depend on a live HTTP lookup.

## Step-by-Step Tasks

### Task 1.1: Add SQLite GAMEID Cache -- Depends on none

**BATCH**: B1

**ACTION**: Add a typed SQLite cache for `(store, codename) -> umu_id` resolver
results and misses.

**IMPLEMENT**:

- Create `metadata/umu_gameid_cache_store.rs` and
  `metadata/umu_gameid_cache_ops.rs`.
- Add a schema v24 migration with a table shaped for cache-first lookups:
  `store`, `codename`, `umu_id`, `status`, `payload_json`, `fetched_at`,
  `expires_at`, `last_error`, `updated_at`.
- Add a unique index on `(store, codename)`.
- Store normalized lowercase `store` and trimmed `codename`.
- Support:
  - fresh hit lookup,
  - stale hit lookup for degraded fallback,
  - upsert of found/missing/error rows,
  - clear all rows owned by this cache.
- Keep cache methods nonfatal to callers by returning typed errors that the
  resolver can log and degrade around.

**MIRROR**:

- `metadata/cache_store.rs` for SQLite access style.
- `metadata/proton_release_catalog_store.rs` for typed domain cache tables.

**GOTCHA**:

- `external_cache_entries::get_cache_entry()` hides expired rows, which is not
  enough for stale fallback. Use a dedicated table or explicit stale accessor.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core metadata::`

### Task 1.2: Add Settings Opt-In -- Depends on none

**BATCH**: B1

**ACTION**: Add a separate opt-in setting for online GAMEID lookup.

**IMPLEMENT**:

- Add `UmuDatabaseLookupPreference` or equivalent enum with `Disabled` and
  `Enabled` values.
- Add `umu_database_lookup` to `AppSettingsData` with default Disabled.
- Use serde naming consistent with existing settings enums.
- Keep the field separate from `UmuPreference`; runner choice and HTTP lookup
  opt-in are different decisions.
- Update settings defaults, load/save tests, and TypeScript settings type.

**MIRROR**:

- Existing `UmuPreference` enum and defaulting pattern in `settings/types.rs`.

**GOTCHA**:

- Default Disabled must mean no new resolver HTTP request during startup,
  preview, game launch, or trainer launch.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core settings`
- `npm run typecheck`

### Task 1.3: Add Profile Runtime Store/Codename Hints -- Depends on none

**BATCH**: B1

**ACTION**: Add optional profile-level resolver hints.

**IMPLEMENT**:

- Add `umu_store` and `umu_codename` to `RuntimeSection`.
- Add serde defaults and skip-empty behavior.
- Update `RuntimeSection::is_empty()` so whitespace-only values count as empty.
- Update profile TS types, normalize helpers, empty profile helpers, and relevant
  tests.
- Apply conservative validation:
  - trim leading/trailing whitespace,
  - reject control characters,
  - cap each field length,
  - store normalized lowercase store for cache keys,
  - keep codename case/characters unless upstream requires normalization.

**MIRROR**:

- Existing optional runtime string fields in `profile/models/runtime.rs`.

**GOTCHA**:

- These fields are hints only; they must not trigger lookup when the global
  setting is Disabled or when user/Steam GAMEID precedence already resolves.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile`
- `npm run typecheck`

### Task 1.4: Add Backend-Only Launch Resolution DTOs -- Depends on none

**BATCH**: B1

**ACTION**: Extend runtime launch request data so backend launch and preview code
can share the same resolved GAMEID without trusting frontend-supplied values.

**IMPLEMENT**:

- Add runtime hint fields to `RuntimeLaunchConfig` if they are not already
  present through profile flattening.
- Add a backend-only resolution field or introduce an `EnrichedLaunchRequest`
  wrapper.
- Include:
  - effective GAMEID,
  - effective STORE,
  - resolution source enum,
  - cache freshness metadata,
  - optional normalized lookup key,
  - optional nonfatal error category.
- Ensure serde prevents frontend injection of resolved fields.
- Update launch request serde tests and frontend request builders.

**MIRROR**:

- Existing `LaunchRequest` serde-default and preview DTO patterns.

**GOTCHA**:

- Keep the generated resolved value runtime-only. Do not add it to profile TOML
  or settings TOML.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::request`
- `npm run typecheck`

### Task 2.1: Add Official umu API Client -- Depends on B1

**BATCH**: B2

**ACTION**: Implement a narrow HTTP client for the official store/codename API.

**IMPLEMENT**:

- Create `umu_database/api_client.rs`.
- Use a hard-coded HTTPS base URL:
  `https://umu.openwinecomponents.org/umu_api.php`.
- Build query strings with `reqwest::Url` and `query_pairs_mut()`.
- Use lowercase query keys `store` and `codename`.
- Use a short timeout, target 2 seconds.
- Parse JSON array responses and accept:
  - first valid `umu_id` on found,
  - empty array as not found,
  - HTTP errors/timeouts/invalid JSON as nonfatal lookup failure.
- Validate returned `umu_id` for safe length and characters before use.
- Add a test URL override only for tests if needed; keep it clearly named and
  scoped like the existing umu CSV test URL override.

**MIRROR**:

- `umu_database/client.rs` for user agent and reqwest singleton.
- `discovery/client/fetch.rs` for URL query construction.

**GOTCHA**:

- Do not log full query URLs or response bodies. Store/codename may reveal a
  user's game library.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core umu_database`

### Task 2.2: Add Cache-First Resolver With Dedupe -- Depends on B1, 2.1

**BATCH**: B2

**ACTION**: Implement resolver orchestration around settings, profile hints,
cache TTL, stale fallback, and API calls.

**IMPLEMENT**:

- Create `umu_database/resolver.rs`.
- Resolution precedence:
  1. user-provided `runtime.umu_game_id`,
  2. Steam id from existing launch request semantics,
  3. opt-in API resolver using `runtime.umu_store` and `runtime.umu_codename`,
  4. stale cache fallback when remote lookup fails,
  5. `umu-0`.
- Gate resolver path on:
  - launch will use `umu-run`,
  - setting is Enabled,
  - no explicit/user GAMEID,
  - no Steam id,
  - both store and codename are present after trimming.
- Cache found and not-found results with a seven-day TTL.
- Deduplicate concurrent lookups for the same normalized key inside the process.
- Preserve launch progress if SQLite is disabled or unavailable.
- Return a typed `UmuGameIdResolution` for preview and env builders.

**MIRROR**:

- `protondb/client.rs` for cache-first/live/stale fallback concepts.
- `launch/script_runner/umu.rs` for existing GAMEID precedence.

**GOTCHA**:

- A cached not-found response is useful. It prevents repeated every-launch
  refetches for games not present in upstream umu-database.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core umu_gameid`

### Task 3.1: Enrich Launch And Preview Commands -- Depends on B2

**BATCH**: B3

**ACTION**: Call the resolver in backend command paths before game launch,
trainer launch, and launch preview.

**IMPLEMENT**:

- Update `src-tauri/src/commands/launch/execution.rs` and
  `src-tauri/src/commands/launch/queries.rs`.
- Load settings and metadata once per command path.
- Resolve/enrich request before calling core launch or preview builders.
- Apply identical enrichment to:
  - `launch_game`,
  - `launch_trainer`,
  - `preview_launch`.
- Keep `preview_launch` async if needed; update frontend command wrappers/tests
  accordingly.
- Ensure disabled lookup does not create a network future.

**MIRROR**:

- Existing async Tauri command style in `commands/launch/execution.rs` and
  preview query style in `commands/launch/queries.rs`.

**GOTCHA**:

- Preview and actual launch must agree. Do not let preview use a mock/default
  path while launch computes a different GAMEID.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch`
- `npm run typecheck`

### Task 3.2: Apply GAMEID/STORE In Game And Trainer Environments -- Depends on B2, 3.1

**BATCH**: B3

**ACTION**: Use the resolved GAMEID and store consistently in command builders.

**IMPLEMENT**:

- Update `launch/script_runner/umu.rs` helper functions so resolver output slots
  into existing precedence without changing user override or Steam behavior.
- Update `proton_game.rs` to set `GAMEID` from the enriched resolution when
  `should_use_umu` is true.
- Set `STORE` when a normalized resolver store is available and useful for umu.
- Update `proton_trainer.rs` with the exact same behavior.
- Keep `umu-0` fallback explicit and test-covered.

**MIRROR**:

- Existing game/trainer parity from trainer execution rules in `AGENTS.md`.

**GOTCHA**:

- Steam profiles can still launch trainers through Proton. Do not create a
  trainer-only path with different GAMEID semantics.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core proton_game_umu proton_trainer_umu`

### Task 3.3: Extend Launch Preview Diagnostics -- Depends on B2, 3.1

**BATCH**: B3

**ACTION**: Show resolution details in `UmuDecisionPreview`.

**IMPLEMENT**:

- Extend Rust `UmuDecisionPreview` with a nested GAMEID resolution diagnostic.
- Include source labels for:
  - explicit profile override,
  - Steam id,
  - fresh cache hit,
  - stale cache fallback,
  - fresh HTTP lookup,
  - cached not-found,
  - missing hints,
  - lookup disabled,
  - API unavailable/error,
  - fallback `umu-0`.
- Keep existing `csv_coverage` field and semantics.
- Update TS types and mock preview data.

**MIRROR**:

- Existing `UmuDecisionPreview` serde naming and frontend mirror.

**GOTCHA**:

- Do not expose noisy raw errors in the UI. Surface stable categories and keep
  detailed diagnostics in trace/debug logs.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core launch::preview`
- `npm run typecheck`

### Task 4.1: Add Settings Controls -- Depends on B1, B3

**BATCH**: B4

**ACTION**: Add UI for enabling lookup and clearing cache.

**IMPLEMENT**:

- Add Runner section control for `umu_database_lookup`.
- Create an Advanced settings section with `Clear umu GAMEID lookup cache`.
- Add `clear_umu_gameid_lookup_cache` command binding through the shared
  `callCommand()` adapter.
- Show concise success/error status after clearing.
- Keep the existing "Refresh umu protonfix database" CSV control separate.

**MIRROR**:

- `RunnerSection.tsx` control and save patterns.
- `useUmuDatabaseRefresh.ts` IPC hook pattern.

**GOTCHA**:

- Settings tests may assert the list/number of sections. Update expected section
  inventory intentionally.

**VALIDATE**:

- `npm run typecheck`
- `npm test -- SettingsPanel`

### Task 4.2: Add Profile Runtime Fields -- Depends on B1, B3

**BATCH**: B4

**ACTION**: Expose `umu_store` and `umu_codename` in the profile Runtime editor.

**IMPLEMENT**:

- Add two compact text inputs to `RuntimeSection.tsx`.
- Use existing BEM-like `crosshook-*` classes and form patterns.
- Normalize empty strings to omitted/empty runtime values.
- Keep helper text short and concrete; do not add a broad docs block in-app.
- Ensure long codenames fit on mobile without overflowing.

**MIRROR**:

- Existing runtime string input controls in `RuntimeSection.tsx`.

**GOTCHA**:

- These fields are only meaningful when lookup is enabled and no stronger
  GAMEID source exists, but users should be able to set them before enabling.

**VALIDATE**:

- `npm run typecheck`
- `npm test -- RuntimeSection`

### Task 4.3: Render Preview GAMEID Details -- Depends on B3

**BATCH**: B4

**ACTION**: Add concise resolver diagnostics to Launch Preview.

**IMPLEMENT**:

- Update `HeroLaunchCommandSection.tsx` and presentation helpers.
- Render effective GAMEID, source, normalized key when relevant, and cache
  freshness/expiry when available.
- Keep copy-to-clipboard behavior scoped to `effective_command`.
- Use neutral styling for disabled/missing hints and warning styling only for
  degraded/error fallback states that matter to users.

**MIRROR**:

- Existing command detail presentation in `HeroLaunchCommandSection.tsx`.

**GOTCHA**:

- Do not duplicate the existing CSV coverage message. GAMEID resolution and CSV
  coverage are related but distinct diagnostics.

**VALIDATE**:

- `npm run typecheck`
- `npm test -- HeroLaunchCommandSection HighlightedCommandBlock`

### Task 5.1: Update Browser Mocks And Fixtures -- Depends on B4

**BATCH**: B5

**ACTION**: Keep browser dev mode and tests aligned with new IPC/types.

**IMPLEMENT**:

- Add default Disabled setting to settings mocks.
- Add profile runtime hint defaults where fixtures construct profiles.
- Add preview mock scenarios for override, cache hit, not found, disabled, and
  fallback.
- Mock `clear_umu_gameid_lookup_cache`.
- Ensure mocks continue using `callCommand()`/`subscribeEvent()` adapter paths.

**MIRROR**:

- Existing mock handler registration in `src/lib/mocks/handlers/index.ts`.

**GOTCHA**:

- Raw `invoke()` bypasses browser mocks and is disallowed for this path.

**VALIDATE**:

- `npm run typecheck`
- `npm test`

### Task 5.2: Update Schema Reference Docs -- Depends on B1

**BATCH**: B5

**ACTION**: Keep repository schema documentation synchronized with the migration.

**IMPLEMENT**:

- Update `AGENTS.md` SQLite metadata schema version from 23 to 24.
- Add the new `umu_gameid_lookup_cache` table to the inventory.
- Update `CLAUDE.md` if it repeats the schema version/table summary.
- Keep policy prose unchanged; only update ground-truth reference material.

**MIRROR**:

- Existing table inventory format in `AGENTS.md`.

**GOTCHA**:

- `AGENTS.md` says `CLAUDE.md` is canonical for rules. Do not duplicate policy
  prose or rewrite unrelated sections.

**VALIDATE**:

- `rg "schema version|umu_gameid|umu GAMEID" AGENTS.md CLAUDE.md`

### Task 6.1: Add Rust Resolver And Launch Tests -- Depends on B1-B5

**BATCH**: B6

**ACTION**: Cover resolver behavior, cache semantics, and launch/trainer parity.

**IMPLEMENT**:

- Add integration tests with a local mock HTTP server or existing test override.
- Cover:
  - default Disabled does no HTTP and returns fallback diagnostics,
  - explicit `runtime.umu_game_id` suppresses lookup,
  - Steam id suppresses lookup,
  - missing hints suppress lookup,
  - found response caches for seven days,
  - empty array caches not-found,
  - fresh cache hit does no HTTP,
  - stale cache is used when remote fails,
  - concurrent lookups dedupe to one HTTP request,
  - SQLite disabled/unavailable is nonfatal,
  - game and trainer env contain identical GAMEID/STORE behavior,
  - preview and launch use the same resolution path.

**MIRROR**:

- Existing `umu_database` integration tests and launch script runner tests.

**GOTCHA**:

- Avoid tests that hit the real upstream API. All network behavior must be local
  or mocked.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

### Task 6.2: Add Frontend Tests -- Depends on B4-B5

**BATCH**: B6

**ACTION**: Cover settings/profile/preview UI and copy behavior.

**IMPLEMENT**:

- Update Settings tests for Runner opt-in and Advanced clear-cache action.
- Add RuntimeSection tests for editing store/codename.
- Add Launch Preview tests for resolution display states.
- Add a regression test that command copy output excludes GAMEID explanation
  text.
- Update mock fixtures to keep existing tests deterministic.

**MIRROR**:

- Existing Vitest/happy-dom component test patterns.

**GOTCHA**:

- There is no separate frontend test framework beyond the configured npm scripts.
  Use existing Vitest commands and browser mocks.

**VALIDATE**:

- `npm test`
- `npm run typecheck`

### Task 6.3: Run Full Validation And Host Gateway Check -- Depends on B1-B5

**BATCH**: B6

**ACTION**: Run the repo validation commands expected for this kind of change.

**IMPLEMENT**:

- Run Rust tests for `crosshook-core`.
- Run frontend typecheck and tests.
- Run host gateway check because launch path code is touched.
- Run lint/format checks if time allows; prefer fixing lint in the same pass.

**MIRROR**:

- Commands listed in `AGENTS.md`.

**GOTCHA**:

- The host-tool gateway check must remain clean. This feature should not add any
  direct `Command::new("umu-run")` or other denylisted host tool execution.

**VALIDATE**:

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- `npm run typecheck`
- `npm test`
- `./scripts/check-host-gateway.sh`
- `./scripts/lint.sh`

## Testing Strategy

### Unit And Integration Tests

- Settings serde/default tests prove existing settings files load with lookup
  Disabled.
- Profile runtime tests prove `umu_store`/`umu_codename` roundtrip and empty
  values stay omitted.
- Metadata tests prove schema v24 migration, upsert, fresh hit, stale hit,
  cached miss, and clear-cache behavior.
- API client tests prove URL encoding, empty array handling, malformed JSON,
  non-2xx status, timeout/error classification, and returned `umu_id`
  validation.
- Resolver tests prove precedence, opt-in gate, TTL, stale fallback, no-SQLite
  degradation, and in-flight dedupe.
- Launch script tests prove game and trainer `GAMEID`/`STORE` parity.
- Preview tests prove diagnostics match actual resolver outcomes.
- Frontend tests prove UI controls, preview rendering, and copy behavior.

### Manual Smoke

- Start browser dev mode with `./scripts/dev-native.sh --browser`.
- Verify Settings Runner shows lookup Disabled by default.
- Enable lookup and set profile Runtime store/codename.
- Use a mocked preview scenario to verify GAMEID/source details render.
- Clear cache from Settings Advanced and verify status text.
- For native Tauri behavior, re-verify with `./scripts/dev-native.sh` before
  merging UI work.

## Validation Commands

Run these after implementation:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
npm run typecheck
npm test
./scripts/check-host-gateway.sh
./scripts/lint.sh
```

If frontend layout or browser mock behavior changes materially, also run:

```bash
./scripts/dev-native.sh --browser
npm run test:smoke
```

## Acceptance Criteria

- The new umu GAMEID HTTP lookup setting exists, persists to settings TOML, and
  defaults to Disabled.
- Disabled lookup performs no new store/codename resolver HTTP request in
  startup, preview, game launch, or trainer launch.
- Profiles can store optional `runtime.umu_store` and `runtime.umu_codename`
  hints without breaking existing TOML files.
- Lookup runs only when umu launch is selected/effective, lookup is Enabled,
  no explicit `runtime.umu_game_id` exists, no Steam id exists, and both hints
  are present.
- Explicit `runtime.umu_game_id` always wins over resolver output.
- Steam id behavior remains unchanged and suppresses store/codename lookup.
- API lookup uses the official HTTPS endpoint with safe query construction.
- API misses (`[]`) and failures never block launch and fall back to `umu-0`.
- Cache rows have a seven-day TTL and include found and not-found results.
- Fresh cache hits do not perform HTTP requests.
- Stale cache can be used as degraded fallback when remote lookup fails.
- Concurrent same-key misses dedupe to one remote request.
- SQLite unavailable or disabled degrades without launch failure.
- Game and trainer launch paths use identical resolved GAMEID/STORE semantics.
- Launch Preview shows effective GAMEID and source/cache status.
- Preview and actual launch use the same backend resolver path.
- The clear-cache action deletes only the GAMEID lookup cache.
- Browser mocks cover the new commands/types.
- Schema reference material is updated to version 24.
- Host gateway check remains clean.

## Completion Checklist

- [ ] SQLite migration and metadata facade complete.
- [ ] Settings opt-in defaults to Disabled and roundtrips.
- [ ] Profile store/codename hints roundtrip and validate.
- [ ] API client uses official endpoint safely.
- [ ] Resolver is cache-first, TTL-aware, deduped, and nonfatal.
- [ ] Launch, trainer, and preview paths share resolver output.
- [ ] GAMEID/STORE env behavior is parity-tested for game and trainer launches.
- [ ] Settings Runner and Advanced UI controls are wired.
- [ ] Runtime editor exposes store/codename.
- [ ] Launch Preview renders GAMEID diagnostics.
- [ ] Browser mocks and TS types are updated.
- [ ] Rust and frontend tests cover the acceptance criteria.
- [ ] Validation commands pass.

## Risks

| Risk                                 | Impact                                       | Mitigation                                                                           |
| ------------------------------------ | -------------------------------------------- | ------------------------------------------------------------------------------------ |
| Upstream API changes response shape  | Resolver could misclassify results           | Parse defensively, validate `umu_id`, treat unknown shape as nonfatal failure.       |
| Launch path blocks on network        | Bad user experience                          | Cache-first, opt-in, short timeout, no retries, stale fallback, `umu-0`.             |
| Resolver hammers upstream for misses | Upstream load and slow repeated launches     | Cache not-found rows for seven days and dedupe in-flight lookups.                    |
| Frontend and backend preview diverge | User sees one GAMEID but launch uses another | Resolve only in backend command paths and reuse enriched request for preview/launch. |
| Trainer path drifts from game path   | Regression against trainer execution parity  | Add explicit game/trainer env tests and use shared helper functions.                 |
| User privacy leakage in logs         | Store/codename may reveal library details    | Log stable categories and keys only at debug when necessary; avoid full URLs/body.   |
| Schema docs drift                    | Future agents see stale schema version       | Update `AGENTS.md` and `CLAUDE.md` with v24 table inventory.                         |

## Implementation Notes

- Use `store` and `codename` as normalized cache key columns. Store should be
  lowercase. Codename should be trimmed and preserved otherwise unless upstream
  docs require additional normalization.
- Consider accepting the official store names currently used by umu-database
  (`amazon`, `battlenet`, `ea`, `egs`, `gog`, `humble`, `itchio`, `steam`,
  `ubisoft`, `umu`, `zoomplatform`, `none`) while allowing unknown future store
  strings to degrade gracefully if validation is intentionally permissive.
- Keep errors as values in resolver output. The UI needs a source category, not
  a raw transport error.
- Do not touch unrelated untracked files while implementing this plan.
