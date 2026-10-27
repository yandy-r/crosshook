# Practices Research: app-storage-services (M1-35 / YAN-847)

> Target file: `docs/plans/app-storage-services/research-practices.md`. Read-only
> research; no repo files were modified, no builds run. Paths are relative to
> `src/crosshook-native/` unless absolute; line numbers describe this checkout.
> Primary sources: the two shipped migrations — profile-writer (PR #533,
> `8ba3b42`) and launch-service (PR #535, `b9021d2`) — plus their plan docs in
> `docs/plans/profile-writer/` and `docs/plans/launch-service/`.

## 0. Executive summary

- Every primitive the six new services need already exists in `crosshook-app`:
  the `#[derive(Clone)]` service-over-stores shape, `services/mod.rs::run_blocking`,
  `AppError`, `EventBus`, `JobRegistry` (with `Publisher::{log, progress}` and
  retained `Outcome`s), `Services::new(&Stores, EventBus)`, and the
  `.manage(services.<x>.clone())` wiring in `src-tauri/src/lib.rs:42-49`.
  Nothing new needs inventing except the service modules themselves and,
  for storage, one decision about how "scan as a job" reaches the registry.
- **The single biggest structural question:** the `JobRegistry` lives on
  `crosshook_app::App` (`app.rs:93,118`), not on `Services`. The bootstrap
  `startup-scans` job (`bootstrap/scans.rs:92`) is the only in-tree consumer.
  YAN-847 wants `scan_prefix_storage` and `list_proton_installs` to be
  registry jobs; the service therefore needs a `JobRegistry` (or a lazily
  captured `Handle`-fallback registry) injected into its constructor. Options
  in §4.
- The 41 commands being moved (190–576-line files) are already thin-ish
  wrappers: they clone stores out of `State<>` and mostly call one
  `crosshook_core` function. The migration is a **delegation swap**, not an
  orchestration transplant like profile-writer was. This keeps each service
  small and makes the batch plan easy to keep disjoint.
- The storage-cleanup allow-list logic (`commands/storage.rs:56-145`) already
  exists and is correct in shape — the gap named by the issue ("no test
  protects it") is a **test gap**, not missing logic. The service move must
  carry that logic verbatim and add the four acceptance tests in the service's
  `tests/` dir.

## 1. Conventions established by the two latest migrations

### 1.1 Module layout and file-size discipline

CLAUDE.md:50 sets a **~500-line soft cap**; exceeding it "must be refactored
into smaller modules". Rust modules are directories with `mod.rs` (CLAUDE.md:62).
Both migrations ship exactly that shape:

```
services/profiles/   (16 files + tests/)     services/launch/   (16 files + tests/)
  mod.rs        struct, ctor, re-exports        mod.rs      166  struct, ctor, re-exports
  writer.rs     commit pipeline                 types.rs    354  session/event types
  lifecycle.rs 419  save/duplicate/delete…      run.rs      319  shared start path
  history.rs   341                              finalizer.rs 353
  sections.rs  …                                streaming.rs 227
  tests/{mod,helpers,<slice>}.rs                gamescope.rs, queries.rs, …
```

Observed rules worth copying verbatim:

1. `mod.rs` holds the service struct, `new()` (positional args — no builder),
   `pub use` re-exports, and `#[cfg(test)] mod tests;`. It documents the module
   ownership map in the header (launch/mod.rs:9-13).
2. One file per concern, named after the responsibility; private modules
   (`mod diagnostics;`) with `pub(super)` items; the only public surface is
   what `mod.rs` re-exports and what `services/mod.rs` re-re-exports
   (`services/mod.rs:21-36`).
3. Tests live in `<service>/tests/{mod.rs,helpers.rs,<slice>.rs}` — one test
   file per acceptance-criterion slice, a shared `helpers.rs` Harness
   (tempdir stores, `MetadataStore::open_in_memory()`/`disabled()`, subscribe
   before acting). Model: `services/profiles/tests/helpers.rs:35-56`.
4. File sizes stay honest: launch's largest non-test file is 354 lines;
   profiles' largest is 419. Settings splits tests into their own files
   (`settings/tests.rs` 451, `ui_writer_tests.rs` 358) to stay under the cap.
5. `static_assertions::assert_impl_all!(Service: Send, Sync)` at the bottom of
   `mod.rs` (launch/mod.rs:166) — required because Tauri `.manage()` demands
   `Send + Sync`.
6. `#![deny(missing_docs)]` is on in `lib.rs`; every `pub` item needs a doc
   comment.

### 1.2 Service shape and Tauri thinning

- Constructor precedent (`profiles/mod.rs:58-71`): positional store args plus
  `EventBus`, cheap clones, `Arc<Mutex<()>>` only where writes must serialize
  (ProfileWriter). `LaunchService::new` adds `with_spawner`/`with_env`/
  `with_helper_source` consuming builders for test seams (launch/mod.rs:113-134).
- Public methods are `async` and run blocking store I/O inside
  `run_blocking` (`services/mod.rs:38-46`, header doc lines 3-8). "Do not nest
  a blocking task per pipeline step" (profile-writer research): one
  `run_blocking` closure per operation, ordering preserved inside.
- Error type is always `AppError`; no per-service error enums (profile-writer
  D11). Adapter maps once: `.map_err(|e| e.to_string())`.
- Tauri thinning target (`commands/health/batch.rs:6-11`):

  ```rust
  pub async fn collection_list(service: State<'_, CollectionsService>)
      -> Result<Vec<CollectionRow>, String> {
      service.list().await.map_err(|e| e.to_string())
  }
  ```

  `State<>` then holds **the service**, not `ProfileStore`/`MetadataStore`.
  Today the target commands hold raw stores
  (`storage.rs:19-20`, `version.rs:99-100`, `mods.rs:59-60` …); the cutover
  replaces those with `State<'_, StorageService>` etc. Note: some commands keep
  a store `State<>` where the store is already managed globally — check
  `src-tauri/src/lib.rs` before removing store `.manage()` calls; stores are
  shared with commands not in scope.

- Wire DTOs stay adapter-side when they are serde-shaped for IPC
  (`PrefixStorageHistoryResponse` at `storage.rs:147-152` can stay, or move to
  the service as a domain type if the CLI will need it; decide per type and
  keep serde names unchanged). The IPC-name-pinning test
  (`commands/settings.rs:182`, the `command_names_match_expected_ipc_contract`
  idiom) must exist per touched command module — **mandatory** here because
  `check-mock-coverage.sh` diffs `#[tauri::command]` names against the
  browser-dev mock registry (`src/lib/mocks/`): names and counts must not change.

### 1.3 Services wiring

- `bootstrap/stores.rs:46-104`: `Services` is a flat `#[derive(Clone)]` struct;
  `Services::new(stores: &Stores, events: EventBus)` builds each service from
  cloned stores. Add one field per new service (six), construct in dependency
  order, extend the `services_new_builds_bundle_from_stores` test.
- `src-tauri/src/lib.rs:42-49`: one `.manage(services.<x>.clone())` line each.
- If a service needs the `JobRegistry` (storage scan, proton scan), the
  constructor must take it — see §4.

### 1.4 Events

- Global `AppEvent` bus (`events.rs`, capacity 256, `#[non_exhaustive]`) is for
  coarse domain notifications only (e.g. `ProfilesChanged`). Job progress/logs
  never go on it — they flow through the job's own broadcast channel.
- Launch precedent: per-session bounded `broadcast` with lag → `Dropped{n}`,
  terminal event delivered exactly once, subscribe-before-start
  (jobs/registry.rs:134-200 is the canonical implementation).
- For this migration: only services that change user-visible data need an
  `AppEvent` (collections CRUD and mods add/remove are candidates; the issue
  does not demand new events — **do not add any without a consumer**; a
  silent event nobody listens to is dead code). Job-based scans get progress
  via `JobContext::progress/log`, not `AppEvent`.

### 1.5 Golden tests and fixtures

- `tests/profile_writer_golden.rs` is a `[[test]] harness = false` binary
  (Cargo.toml:42-44) because it must set `HOME` to a tempdir before any
  thread/runtime exists. Fixtures: `tests/fixtures/profile_writer/*.toml`,
  loaded with `include_bytes!`/`env!("CARGO_MANIFEST_DIR")`, compared
  **byte-for-byte** (`fs::read` vs fixture bytes; never re-serialize the
  expected output). Fixtures are frozen pre-refactor captures: "a mismatch is
  a regression, never a reason to regenerate" (profile-writer D15).
- Apply to YAN-847 AC3: launcher-export golden fixtures for the generated
  shell script and `.desktop` file, captured from the **current**
  `export_launchers_core` output before the cutover, compared byte-identical.
  Core's `launch_request_golden.rs:34-46` (`tests/fixtures/launch_request/`)
  is the sorted-directory pattern; the profile-writer binary is the
  env-isolation pattern. The export golden probably does **not** need
  `harness = false` if output paths are passed explicitly — prefer a normal
  `#[test]` under `services/launcher_export/tests/` unless `$HOME` leaks into
  the output.

### 1.6 Script gates (the verification list)

From the launch-service plan §Validation Commands and `scripts/`:

| Gate                                    | What it enforces                                                                                                         | Relevance here                                                                                                                                                                          |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `./scripts/check-app-deps.sh`           | `crosshook-app` pulls no UI toolkit (tauri/wry/gtk/…) — `cargo tree` scan, `BANNED_RE`                                   | Hard gate: services must not `use tauri`.                                                                                                                                               |
| `./scripts/check-host-gateway.sh`       | no `Command::new("<host tool>")` / `open::that` outside the core gateway; scans `crates/crosshook-app/src` (lines 11-18) | Storage cleanup deletes files (fine); steam/export services must only call core functions, never spawn host tools. `# allow: host-gateway` suppression exists but should not be needed. |
| `./scripts/check-mock-coverage.sh`      | every `#[tauri::command]` has a browser-dev mock handler (`src/lib/mocks/`)                                              | Command names/signatures must not change; mocks stay valid.                                                                                                                             |
| `./scripts/lint.sh`                     | rustfmt + clippy `-D warnings` + Biome + ShellCheck + the other checks                                                   | Standard.                                                                                                                                                                               |
| `./scripts/check-schema-version.sh`     | SQLite schema drift vs `SUPPORTED_MAX_VERSION`                                                                           | No-op here (issue forbids schema change) but must stay green.                                                                                                                           |
| `./scripts/check-startup-path.sh`       | startup ordering contract                                                                                                | Unaffected unless `Services::new` order changes the bootstrap path — it doesn't; services are built after stores.                                                                       |
| `./scripts/format.sh --modified --docs` | prettier/rustfmt no-diff                                                                                                 | Run at the end.                                                                                                                                                                         |

Rust/TS test commands (launch-service plan, lines 687-704):

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::storage
# … per service
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native   # adapter tests
cd src/crosshook-native && npm run typecheck && npm test && npm run lint \
  && npm run dev:browser:check && npm run test:smoke
```

Frontend commands are only needed if TS hooks/mocks change — for a pure
backend delegation swap the mock names stay identical, but `npm test` and
`test:smoke` cover AC5 (Library, collections, Settings storage smoke).

### 1.7 Test conventions

From both migrations' research + shipped code:

- `#[tokio::test]` (current-thread) for service logic;
  `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` when real
  child processes or `spawn_blocking` races are involved
  (`jobs/process/tests.rs:38-160`). Job-registry tests: `jobs/tests.rs`.
- Harness per service in `tests/helpers.rs`: retained `TempDir`,
  `ProfileStore::with_base_path`, `SettingsStore::with_base_path`,
  `MetadataStore::open_in_memory()` or `::disabled()` — the latter gives the
  "DB disabled → `Unavailable`, no partial writes" acceptance criterion its
  test double with **no trait seam** (profile-writer §3.2: do not add
  `MetadataPort` traits).
- Pure function + injected inputs, no `set_var` in tests (`gamemode.rs:41`
  pattern). For storage cleanup: the allow-list check should be a pure
  function over `(scan_result, requested_targets)` so the symlink/prefix-root
  rejection cases are table-testable.
- Subscribe-before-acting with a bounded `timeout` on recv; never sleep.
- Real tempdir filesystems over fake-fs traits; `std::fs` inside
  `run_blocking`.

## 2. JobRegistry API for "scan as a job"

Canonical source: `crates/crosshook-app/src/jobs/` (registry.rs 532 lines,
types.rs 141, process.rs 402, tests.rs 444). Re-exports in `jobs.rs`:

```rust
pub use registry::{JobContext, JobEvents, JobHandle, JobRegistry, JobRegistryConfig, DEFAULT_JOB_RETENTION};
pub use types::{JobEvent, JobEventKind, JobId, JobKey, JobProgress, JobSpec, Outcome};
pub use process::{create_log_path, kill_processes_using_prefix, supervise_child, …};
```

Key API (`jobs/registry.rs`):

- `JobRegistry::start(spec: JobSpec, job: F) -> Result<JobHandle, AppError>`
  (lines 134-195): `F: FnOnce(JobContext) -> Fut`, `Fut::Output: Into<Outcome>`.
  Channel + receiver exist **before** spawn (subscribe-before-start; race-tested
  at `jobs/tests.rs:53`). Errors: `Conflict` if `spec.key` is already held
  (use `JobKey` = e.g. `"storage-scan"` or the prefix root to make scans
  exclusive), `Unavailable` when no runtime.
- `JobContext` (lines 386-435): `id()`, `cancel_token()`, `is_cancelled()`,
  `cancelled().await`, `log(Vec<String>)`, `log_batch(LogBatch)`,
  `progress(JobProgress)`. Terminal `Finished` is published only by the
  registry; a panic finishes the job as `Failed(Internal)` via `FinishGuard`.
- `JobHandle`: `id()`, `cancel_token()`, `events()` → `JobEvents`
  (`recv().await` maps `Lagged(n)` → `LogBatch::Dropped{n}`, terminal
  `Finished` always delivered).
- Result retrieval: `outcome(&id) -> Result<Option<Outcome>, AppError>`
  (None while running), `subscribe(&id)` replays the retained `Finished` for
  finished-unreaped jobs (retention 64), `reap(&id) -> Result<Outcome, _>`,
  `cancel(&id)`.
- `Outcome` (types.rs:98-108): `Succeeded{..}/Failed{..}/Cancelled` — it does
  **not** carry a payload. A scan job that must hand back
  `PrefixStorageScanResult` cannot return it via `Outcome`; it must publish it
  through a side channel (e.g. store it in a `Mutex<Option<…>>` the service
  owns, or have the command call the scan function directly and use the job
  only for progress). **Decide this in the plan**: the clean shape is
  `StorageService::start_scan() -> Result<JobHandle, AppError>` plus
  `StorageService::scan_result(&JobId)`/taking the result from shared state,
  or simply `scan()` async-with-progress if the UI doesn't need job semantics.

### How launch used it

It didn't — launch deliberately does **not** run as `JobRegistry` jobs
(launch-service research §3.2: "`Outcome` cannot carry exit code, signal or
report, and `JobKey` cannot express parent/child linking"). The only in-tree
user is the bootstrap scan pipeline:

```rust
// crates/crosshook-app/src/bootstrap/scans.rs:92
let result = app.jobs().start(JobSpec::new("startup-scans"), move |ctx| {
    async move {
        if ctx.is_cancelled() { …; return Outcome::Cancelled; }
        mark_running(&startup, StartupJobKind::HealthBatch);
        let health_result = health.batch_summary().await;   // service inside a job
        …
    }
});
```

So the established pattern is: **a job closure that calls service methods,
checking `ctx.is_cancelled()` between steps and reporting progress via
`ctx.progress(...)`**, with results routed through the startup report rather
than `Outcome`. Storage's scan-as-job should follow this: the job closure
runs `scan_prefix_storage_core` inside `run_blocking`, publishes progress, and
the service keeps the last scan result (or returns it via a oneshot/shared
slot) for the command to fetch.

### How the service gets the registry

`JobRegistry` is `pub` and cheap-`Clone`. Construction options:

1. `JobRegistry::default()` — ambient-runtime fallback at `start`
   (registry.rs:140-145). Works under Tauri and the CLI with zero wiring.
   Jobs don't get the app shutdown token as parent (acceptable for a user-
   initiated scan; cancellation is still available via the handle).
2. Inject `App::jobs().clone()` through `Services::new` — needs `App` before
   `Services` exists; `App::new` builds services first (`app.rs:93` builds
   jobs after runtime). Would require reordering or a setter.
3. Pass `Option<JobRegistry>` in the constructor with a `with_jobs()` builder
   (LaunchService's `with_spawner` precedent, launch/mod.rs:113-134).

Recommendation: **option 1** (`JobRegistry::default()` constructed lazily or
in `new`) unless the plan demands shutdown-token parenting. It keeps
`Services::new(&Stores, EventBus)` signature stable and the CLI usable. Record
the trade-off in decisions.md (jobs not cancelled on app shutdown — a scan is
read-only, so this is benign; cleanup stays a direct non-job call so no
destructive op outlives shutdown).

## 3. What the seven source files actually contain (sizing input)

| File (lines)                     | Commands | State<> today                                    | Core deps (do not re-implement)                                                                                                                                                                             |
| -------------------------------- | -------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `commands/storage.rs` (190)      | 3        | `ProfileStore`, `MetadataStore`                  | `crosshook_core::storage::{scan_prefix_storage, cleanup_prefix_storage, collect_profile_prefix_references, …}`; metadata `insert/list_prefix_storage_cleanup_audit`, `list_latest_prefix_storage_snapshots` |
| `commands/mods.rs` (258)         | 6        | `ProfileStore`, `MetadataStore`                  | `crosshook_core::mods::{…}`, `launch::analyze_profile_mod_coexistence`                                                                                                                                      |
| `commands/collections.rs` (166)  | 13       | `MetadataStore`, `ProfileStore`                  | `MetadataStore::{list/create/delete/rename/…collections…}`, `collection_defaults` ops; `profile::collection::{export_collection_preset_to_toml, preview_collection_preset_import}`                          |
| `commands/export.rs` (576)       | 12       | `MetadataStore`, `ProfileStore`, `SettingsStore` | `crosshook_core::export::{export_launchers, validate, check_launcher_exists…}`, `export::launcher::sanitize_launcher_slug`                                                                                  |
| `commands/version.rs` (261)      | 4        | `MetadataStore`, `ProfileStore`                  | `steam::{discover_steam_libraries, manifest::parse_manifest_full, …}`, `metadata::{compute_correlation_status, hash_trainer_file, …}`                                                                       |
| `commands/steam.rs` (58)         | 3        | none (env + fs)                                  | `steam::{attempt_auto_populate, discover_compat_tools, discover_steam_root_candidates}`                                                                                                                     |
| `commands/game_metadata.rs` (65) | 4        | `MetadataStore`, `SettingsStore`                 | `game_images::{download_and_cache_image, import_custom_*}`, `steam_metadata::lookup_steam_metadata`                                                                                                         |

Plus the `apply_collection_defaults` merge at
`commands/profile/shared.rs:171-225` with tests at
`commands/profile/tests/optimizations.rs:66-193` — profile-writer M1-35
explicitly left these in Tauri; this issue moves them into
`services/collections/`, and `profile/lifecycle.rs:33` becomes a delegate.

Notable today:

- `storage.rs:56-145` already implements rescan-then-allow-list
  (`target_signature` + `BTreeSet` intersection, dedupe) and fail-soft audit
  rows — move verbatim, add the missing security tests (AC1). The
  symlink-after-scan and outside-prefix-root cases must be **new** tests; the
  current code has no explicit symlink check visible in the command — verify
  whether `cleanup_prefix_storage_core` guards symlinks (research-security
  should confirm) and add the guard in the service if absent.
- `steam.rs:35` `list_proton_installs` is `pub fn` (sync!) — it blocks the
  Tauri IPC worker today. AC4 wants it non-blocking: as a registry job or at
  minimum `run_blocking`. Same for `export.rs:195` `export_launchers`
  (`pub fn`, writes files).
- `export.rs:188-462` keeps wire DTOs and metadata side-effects
  (`observe_launcher_exported` fail-soft at :200-209) in the command; the
  side-effect belongs in the service.
- `version.rs:96-160` mixes Steam manifest IO + metadata reads — all blocking,
  currently sync commands.

## 4. Recommended module layout

Six new directories under `crates/crosshook-app/src/services/`, all following
the profiles/launch shape. Budgets are targets under the 500-line soft cap.

| Directory          | Files (≈ lines)                                                                                                                                                                                                                                                                                                                                               | Owns                    |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- |
| `storage/`         | `mod.rs` (90: `StorageService` struct, ctor incl. `JobRegistry`, re-exports); `scan.rs` (150: scan fn + job wrapper + progress); `cleanup.rs` (200: rescan, `target_signature` allow-list, `filter_targets`, cleanup call); `audit.rs` (120: audit-row construction + fail-soft persistence + history reads); `tests/{mod,helpers,cleanup_security,audit}.rs` | AC1, AC4(scan), history |
| `mods/`            | `mod.rs` (70); `crud.rs` (180: list/add/update/remove with profile resolution); `detect.rs` (100); `coexistence.rs` (80: wraps `analyze_profile_mod_coexistence`); `tests/{mod,helpers,crud,coexistence}.rs`                                                                                                                                                  | 6 commands              |
| `collections/`     | `mod.rs` (70); `crud.rs` (150: 11 thin metadata ops); `defaults.rs` (120: get/set with `clear_extra`/`preserve_extra_from` + **`apply_collection_defaults` merge** moved from `commands/profile/shared.rs`); `transfer.rs` (100: TOML export/import preview); `tests/{mod,helpers,crud,defaults_merge,transfer}.rs`                                           | 13 commands + AC2       |
| `launcher_export/` | `mod.rs` (80); `export.rs` (200: export/validate/check/delete/rename + metadata observe, `run_blocking` for the file writes — AC4); `list.rs` (120: list + orphan detection + stale flags); `preview.rs` (80: script + `.desktop` previews); `tests/{mod,helpers,export,list_orphans,preview}.rs`                                                             | 12 commands             |
| `version/`         | `mod.rs` (60); `status.rs` (150: `check_version_status` + `locate_manifest` helper + snapshot); `ops.rs` (80: set trainer version, acknowledge); `tests/{mod,helpers,status}.rs`                                                                                                                                                                              | 4 commands              |
| `steam/`           | `mod.rs` (60); `client.rs` (80: auto-populate + default client path); `proton.rs` (90: `list_proton_installs` as job or `run_blocking` scan — AC4); `metadata.rs` (120: game metadata, cover-art fetch w/ offline→`Unavailable`, custom-art import; image-cache paths untouched); `tests/{mod,helpers,proton,metadata}.rs`                                    | 3 + 4 commands          |

`services/mod.rs` gains 6 `pub mod` lines + grouped re-exports.
`bootstrap/stores.rs::Services` gains 6 fields, constructed in one block;
`src-tauri/src/lib.rs` gains 6 `.manage()` lines.

Notes:

- `game_metadata` folds into `steam/` per the issue's `services/steam.rs`
  scope ("game metadata + cover-art fetch + custom-art import" listed there).
- Offline rule (art fetch → `Unavailable`, cached art still served) is a
  pure mapping over the core result; test with the core fetch failing.
- DB-disabled rule: `MetadataStore::disabled()` in the harness; collections
  and mods methods must return `AppErrorKind::Unavailable` before any write —
  table-test per method.
- Keep `PrefixStorageHistoryResponse`-style serde DTOs in the adapter unless
  the CLI needs them; serde field names must not change (mock-coverage gate +
  golden payloads).

## 5. Batching plan (disjoint files per batch)

Modeled on the launch-service plan (B0 capture → B1 contracts/tree → leaves →
orchestration → cutover → docs). Tasks in one batch touch disjoint files;
subagents skip builds; main agent compiles once per batch gate.

| Batch  | Tasks    | Width | Contents                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------ | -------- | ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **B0** | 0.1, 0.2 | 2     | Freeze goldens from unmodified code: 0.1 capture launcher-export script/`.desktop` fixtures into `crates/crosshook-app/tests/fixtures/launcher_export/` (AC3) and copy the `apply_collection_defaults` merge table expectations (AC2). 0.2 capture current command list + IPC names (`rg '#\[tauri::command\]'`) as the cutover checklist.                                                                                                                                                                         |
| **B1** | 1.1      | 1     | Contracts + skeleton: six `mod.rs` files with struct/`new`/re-exports (signatures frozen in a b0-contracts.md), six `tests/{mod,helpers}.rs`, `services/mod.rs` module lines, `Services` fields + construction, Tauri `.manage()` lines. Nobody else may edit these files later.                                                                                                                                                                                                                                   |
| **B2** | 2.1–2.6  | 6     | Six service implementations in parallel, one per service directory (storage, mods, collections, launcher_export, version, steam+metadata). Each includes its `tests/` slices. Disjoint by construction.                                                                                                                                                                                                                                                                                                            |
| **B3** | 3.1–3.4  | 4     | Adapter cutover, grouped to avoid file collisions: 3.1 `commands/storage.rs` + `commands/mods.rs`; 3.2 `commands/collections.rs` + `apply_collection_defaults` move out of `commands/profile/shared.rs` (+ `profile/lifecycle.rs:33` delegate swap + move `commands/profile/tests/optimizations.rs:66-193` merge tests); 3.3 `commands/export.rs`; 3.4 `commands/version.rs` + `commands/steam.rs` + `commands/game_metadata.rs`. Each adds/keeps the `command_names_match_expected_ipc_contract` test per module. |
| **B4** | 4.1, 4.2 | 2     | 4.1 security test hardening: storage cleanup AC1 table (missing-from-scan, symlink-after-scan, outside-prefix-root, audit rows written) in `services/storage/tests/cleanup_security.rs` — owned separately from B2.1's functional tests to keep B2 slices small, or fold into 2.1 if sizing allows. 4.2 dead-code sweep: delete moved helpers, verify no `spawn_blocking`/`pub fn` blocking leftovers in the 7 files, `use` cleanups in `commands/mod.rs` and `lib.rs`.                                            |
| **B5** | 5.1      | 1     | Docs: app README service list, decisions.md/feature-spec finalization, `docs(internal)` commit conventions.                                                                                                                                                                                                                                                                                                                                                                                                        |

Collision watch-list (from launch-service risk table):

- `services/mod.rs`, `bootstrap/stores.rs`, `src-tauri/src/lib.rs`,
  `crates/crosshook-app/Cargo.toml`: **B1-only** edits.
- `commands/profile/shared.rs` is touched by 3.2 only; profile-writer's rule
  ("do not widen scope") applies — only the collection-defaults block moves.
- 3.1–3.4 each own distinct command files; `commands/mod.rs` re-exports: give
  its edit to B4 (or B1 if registration order is unchanged — registrations in
  `lib.rs` invoke the same `commands::<m>::<cmd>` paths, so `mod.rs` may not
  need edits at all if module names stay).
- Fixture capture (B0) must finish before B2 writes the golden test that
  consumes it.

## 6. Verification commands (final gate)

```bash
# static analysis (run from repo root)
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
./scripts/lint.sh
./scripts/check-host-gateway.sh && ./scripts/check-host-gateway.sh --self-test
./scripts/check-app-deps.sh
./scripts/check-startup-path.sh
./scripts/check-schema-version.sh
./scripts/check-mock-coverage.sh

# rust tests
cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::storage
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::collections
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launcher_export
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app --test launcher_export_golden   # AC3
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native   # adapter + IPC-name tests

# frontend (AC5: Library, collections, Settings storage smoke)
cd src/crosshook-native
npm run typecheck && npm test && npm run lint && npm run dev:browser:check && npm run test:smoke

# formatting no-diff at the end
./scripts/format.sh --modified --docs
```

Worktree prerequisites for the script gates (CLAUDE.md:20-25):
`npm install -D --no-save typescript@{project-version} biome` at the worktree
root, then `cd src/crosshook-native && npm ci` — `./scripts/lint.sh` and
`format.sh` fail on a bare worktree otherwise.

## 7. Anti-patterns to avoid (carried from both migrations)

- No per-service error enums — `AppError` only.
- No store-trait seams (`MetadataPort`, fake fs, clock traits) — tempdir
  stores + `MetadataStore::open_in_memory()/disabled()`.
- No new `AppEvent` variants without a consumer; job progress goes through
  `JobContext`, not the bus.
- No `tauri::` imports, `AppHandle`, or `State<>` inside services
  (`check-app-deps.sh` fails the build).
- No host-tool `Command::new` in services (`check-host-gateway.sh` scans
  `crates/crosshook-app/src`).
- Don't regenerate frozen fixtures to make a test pass.
- Don't leave `pub fn` blocking commands (steam/export) after cutover —
  AC4 is explicit; route through `run_blocking` or a job.
- Don't change command names/args — the mock-coverage gate and golden IPC
  contract depend on it.
- Don't nest `run_blocking` per pipeline step; one closure per operation.
- Subscribe before acting; never `sleep` in tests.

## 8. Open questions for the plan/decisions docs

1. **JobRegistry plumbing** (§2): `JobRegistry::default()` inside the service
   vs injected `App::jobs()`. Default recommended; needs a decision entry
   recording the shutdown-token trade-off.
2. **Scan result delivery**: `Outcome` carries no payload — service-owned
   shared slot vs returning the result directly from the (async) scan method
   and using the job only for progress. Pick one before B1 freezes contracts.
3. **Symlink guard location**: confirm whether
   `cleanup_prefix_storage_core` rejects symlinks; if not, the guard is new
   logic in the service (AC1 demands the test either way) — security research
   should pin the exact check (e.g. `symlink_metadata` before delete).
4. **DTO placement**: which of `PrefixStorageHistoryResponse`,
   `VersionCheckResult`, `LauncherInfo`, `CollectionExportResult` move into
   the service (needed by CLI/native UI) vs stay adapter-side. Rule of thumb:
   if `crosshook-cli` or the native UI will call it, the type lives in the
   service; serde names frozen either way.
