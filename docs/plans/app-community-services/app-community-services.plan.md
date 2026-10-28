# Plan: app-community-services — four services in `crosshook-app` (YAN-835 / M1-34)

Tracking: Linear YAN-835 / GitHub #543. Release `crosshook v0.7.0-alpha.1`, one PR to `main`. Paths relative to `src/crosshook-native/`; `app/` = `crates/crosshook-app`, `core/` = `crates/crosshook-core`, `tauri/` = `src-tauri`. Binding [decisions.md](./decisions.md) D1–D12; [feature-spec.md](./feature-spec.md); exact [b0-contracts.md](./b0-contracts.md).

## Summary

Move the orchestration of 19 Tauri commands into community, Lutris, discovery and ProtonDB services. Add core root containment and an injected-runner async git driver. Community sync becomes a non-exclusive cancellable job with queued serialization and real child/process-group supervision, not an uncancellable blocking closure. All existing DTO/IPC contracts and ProfileWriter events remain; D5 normalized tap validation and D7 offline policy are intentional changes.

## Batches and ownership contract

Tasks in each batch own **disjoint files, including tests/mod.rs**. Batches run in order. All agents use the absolute worktree root `/home/yandy/Projects/github.com/yandy-r/crosshook-yan-835`; never stash/checkout/reset/commit. Subagents skip build/test/lint/format mid-flight; main runs gates once after each batch lands. If ownership/contract must change, ask integration owner; never edit another slice's files.

| Batch | Tasks              | Dependencies      | Width | Contents                                                                         |
| ----- | ------------------ | ----------------- | ----- | -------------------------------------------------------------------------------- |
| B0    | planning artifacts | research + D1–D12 | 1     | Freeze these contracts and existing 19-command inventory in research-technical.  |
| B1    | 1.1, 1.2, 1.3, 1.4 | B0                | 4     | Core containment; core async sync; discovery + ProtonDB service; Lutris service. |
| B2    | 2.1                | B1                | 1     | Complete community service, runner, job orchestration and critical tests.        |
| B3    | 3.1, 3.2           | B2                | 2     | Single shared wiring/error/adapter owner; independent documentation owner.       |

**Shared-file ownership is exact:** `app/src/services/mod.rs`, `app/src/error.rs`, `app/src/error/tests.rs`, `app/src/bootstrap/stores.rs`, `app/src/bootstrap/stores/tests.rs`, `app/src/lib.rs`, and `tauri/src/lib.rs` belong **only to B3 task 3.1**. No skeletons/placeholders or temporary dead-code attributes. B1/B2 service modules are complete files but deliberately not registered until B3; intermediate app gates exercise the old registered graph, not the new modules. Do not claim those modules compile/test before B3. Core remains compilable at intermediate gates using shared-builder sync entry points; B3 deletes them at final cutover. No compatibility shim remains afterward.

## Task 1.1: Core containment, no-follow indexer and Lutris reads (B1)

- **Owns:** `core/src/fs_util.rs`, new `core/src/fs_util/workspace_tests.rs`, `core/src/community/index.rs` (including inline tests), `core/src/profile/lutris_import/paths.rs`, `core/src/profile/lutris_import/tests/paths_tests.rs`.
- **Change:** implement `WorkspacePathError` and `ensure_within_workspace` exactly per b0 §1/D3; root canonicalization, raw ParentDir rejection, relative-root resolution, lexical absolute prefix then canonical component-prefix check, existing targets only. Return canonical path, never case-fold/Unicode-normalize. Indexer uses `DirEntry::file_type` to skip all symlink files/directories before recursion/read. Lutris validates `games/`, nested scan dirs, YAML candidates and `pga.db` against root before read; skip with diagnostic, use resolved path. Preserve current entry/YAML size caps and legitimate external game/prefix paths.
- **Tests first:** real tempdir nested relative/absolute success; relative/absolute `..` including ends-inside; outside absolute; `root-evil`; outward intermediate/final links; dangling links; in-root intermediate/final links; real case and NFC/NFD sibling JSON rejection (assert filesystem supports distinct names); root file/nonexistent/missing target. Indexer skips linked manifest and linked directory/cycle; Lutris cannot read outward linked YAML/DB while valid nested config remains. Do not change import authorization in app here.
- **Acceptance:** AC1 core matrix, no outside fixture parsed; generic Display contains no raw paths. Existing `fs_util` atomic-write behavior unchanged.

## Task 1.2: Core async tap-sync and injected git runner (B1)

- **Owns:** `core/src/community/taps/{mod,store,git,types,validation,tests}.rs`, new `core/src/community/taps/runner.rs`, optional new `core/src/community/taps/async_tests.rs` (module declared by this task), `core/src/community/mod.rs`.
- **Change:** b0 §3 exact boxed-future `TapGitRunner`, invocation/output/error structs, `CommunityTapError::Cancelled`, async `sync_many_with_runner`/`sync_tap_with_runner`/workspace driver. Reuse step builders for synchronous APIs until B3. Core owns five git security env pairs and argv including clone `--`, branch/pin validation and result construction. Add public `CommunityTapStore::normalize_and_validate`; normalize_subscription also validates nonempty pin with existing `is_valid_git_sha`. Do not add dependencies (core has async-trait; app does not; explicit boxed trait avoids macro dep).
- **Flow:** sequential taps, first hard error aborts; existing workspace fetch/reset/clean or fetch/pinned-checkout/clean, failures cache-fallback as today; cancellation never fallback. Before every git step and before/in blocking index closure check runner cancellation. Tokio filesystem work and spawn_blocking indexing avoid caller-thread stall; map join failure with fixed text. Retain result serde exactly.
- **Tests:** fake runner records env/argv/order (clone separator, pinned sequence), nonzero/failed fetch becomes CachedFallback only for existing clone; Cancelled before/during fetch aborts without index; invalid URL/branch/pin rejected before invocation. Existing real-git sync/pin/validation tests remain until B3 ports them to async test runner. Do not edit index.rs (1.1 owns it).
- **Acceptance:** async driver is core→runner only, no app dependency, no raw process policy duplication; cancellation distinguishable and existing sync caller still builds during staging.

## Task 1.3: Discovery and ProtonDB services (B1)

- **Owns:** new `app/src/services/discovery/{mod,catalog,search,sources,compatibility}.rs`, `discovery/tests/{mod,helpers,offline,sources}.rs`; new `app/src/services/protondb/{mod,lookup,suggestions}.rs`, `protondb/tests/{mod,helpers,cache}.rs`; `core/src/discovery/{mod.rs,client/mod.rs,client/search.rs,client/tests.rs}`.
- **Change:** exact b0 §5 APIs. Port all six discovery and three ProtonDB bodies; source mutations atomically update SettingsStore, preserve other extras and return cleared extras. Catalog any DB error falls back to tap catalog. Keep compatibility/duplicate/missing-source/dismiss texts, 30-day dismissal retention, no eviction side effect. ProtonDB accept remains ProfileWriter, no new accept implementation.
- **Offline search:** add core `ExternalSearchMode` and `search_external_trainers_with_mode`, preserving existing Online entry. CacheOnly uses same cache key/builders/TTL/stale path, cannot live-fetch even with forceRefresh; fresh/stale cached response accepted, active no-cache search Unavailable; empty query/zero enabled sources unchanged. Do not duplicate private cache-key/parser logic in app. Core constants remain 1h TTL, 4 concurrency, 50 items, 1MiB response. ProtonDB stays 6h with current stale fallback, ungated by settings.
- **Threading:** reuse `services/steam/metadata.rs:24-29` runtime-in-run_blocking bridge so synchronous SQLite inside core async clients cannot stall IPC thread; simple catalog/settings/profile/suggestion assembly use one blocking closure per operation. No provider/library rewrite.
- **Tests:** CacheOnly fresh/stale/empty cached payload, forceRefresh cannot fetch, missing/disabled cache Unavailable and no server requests; source CRUD texts/extras; minimal cache/stale and disabled metadata regression for ProtonDB; catalog degraded fallback. Existing core tests carry the rest of D10, avoid broad test expansion beyond D12.
- **Acceptance:** reusable async APIs returning existing DTOs; no Tauri types; files complete but registered at B3. Mappers are frozen in b0 and implemented by 3.1, not here.

## Task 1.4: Lutris service (B1)

- **Owns:** new `app/src/services/lutris/{mod,import}.rs`, `app/src/services/lutris/tests/{mod,helpers,import}.rs`.
- **Change:** positional `new(profiles, writer)`, prepare uses `run_blocking(preview_lutris_import(directory, Some(&profiles)))`, import awaits existing writer and returns `ProfileCommit::into_output`. Do not reopen provenance source paths on apply, constrain game paths, write profiles directly or construct another writer.
- **Tests:** minimal real Lutris YAML preview/import fixture; imported names/result semantics and shared Imported event preserved; missing root stays diagnostic empty preview. Reuse real stores/tempdir writer harness pattern, no store traits/environment mutation.
- **Acceptance:** complete Clone + Send + Sync service and tests; no edits to core scanner (1.1 owns it), mappers (3.1), registration (3.1).

### B1 gate (main agent; after all four slices land)

From `src/crosshook-native/`, one filter per cargo invocation (the issue's `community discovery protondb` multi-filter syntax is invalid):

```sh
cargo test -p crosshook-core fs_util
cargo test -p crosshook-core community
cargo test -p crosshook-core lutris_import
cargo test -p crosshook-core discovery
cargo test -p crosshook-app
cargo clippy --workspace --all-targets -- -D warnings
```

New app modules intentionally unregistered: these tests/clippy do not validate them yet. B3 full app gate is mandatory before delivery. No source skeletons/no-op stubs for intermediate compilation.

## Task 2.1: Community service, lifecycle runner and critical tests (B2)

- **Owns:** entire new `app/src/services/community/` directory: `{mod,taps,containment,import,sync,persist,runner}.rs`, `tests/{mod,helpers,taps,imports,sync,cancellation}.rs` (split further within directory if needed). No shared app file/core/Tauri edits.
- **Tap/import changes:** b0 §5/D4–D5: normalize before persistence, dedupe normalized URL/branch/pin triple, invalid taps write nothing; remove normalized identity only (no Classic IPC or clone/DB deletion); use SettingsStore::update and clear response extras. Preview accepts arbitrary local regular JSON, classifies with helper and reads canonical path; mutating import enforces some subscribed workspace and passes resolved regular JSON to writer. Export arbitrary user-selected destination as before.
- **Sync changes:** `start_sync`/`sync_result`/`sync` per b0. Non-exclusive `community-sync`, clone-shared Tokio mutex queues rather than Conflict, cancellation-aware lock wait. Load settings off-thread, reject offline before git. Production runner via lifecycle host gateway, process_group(0), kill_on_drop, bounded retained stdout/stderr but drain to EOF concurrently; supervise_child with default grace, cancellation is typed and never cached fallback. No raw stderr/URLs in progress/error messages.
- **Persistence:** all tap core results must succeed before metadata loops. `run_blocking` finalize: cancellation check before each index → offline upsert, warning-only; second loop read-back last_sync_at, clear extras. Disabled metadata remains usable. Slot inserted only before successful terminal outcome, take-once, evicted slots pruned, dropped waiters do not leak, sync convenience reaps jobs on all outcomes. Do not use `run_job` around old synchronous sync.
- **Critical tests (D12):** normalized dedupe incl. trim/empty branch/pin and distinct pin/branch, invalid no-write, extras retained; remove leaves clone/rows; preview outside accepts while import outside/traversal rejects and nested import succeeds. Local **bare** remote + seeded repository (tempdir, local git identity, no network/global config) clones and updates valid manifest; assert index/offline row/timestamp backfill. Set stored watermark `last_head_commit = NULL` and loading mode NULL through `MetadataStore::with_sqlite_conn`, resync same HEAD and assert `trainer_loading_mode` populated. Force fetch failure after successful clone and assert from_cache retains prior timestamp.
- **Cancellation/nonblocking tests:** gated fake runner on current-thread runtime proves another task progresses; registry running/reaped counts; second start queues and can cancel while queued (not Conflict). Controlled sleep child through production supervision helper proves killed/reaped process, Cancelled outcome, empty result slot and zero index/offline writes. Test fetch cancellation on existing cache never returns cached success. Subscribe before act, use channels/timeouts and watchdog release; no timing sleeps for synchronization.
- **Acceptance:** AC2–AC3 service evidence exists; no new global events, lock/result state persisted, fake fallback or network fixture. Existing writer supplies imports/revisions/events.

### B2 gate (main agent)

```sh
# from src/crosshook-native; new app module activation is deliberately deferred to B3
cargo test -p crosshook-core community
cargo test -p crosshook-app
cargo clippy --workspace --all-targets -- -D warnings
```

## Task 3.1: Integration, error mappings, adapters, final sync cutover (B3)

- **Owns:** `app/src/{services/mod.rs,error.rs,error/tests.rs,lib.rs,bootstrap/stores.rs,bootstrap/stores/tests.rs}`; `tauri/src/lib.rs`; `tauri/src/commands/{community,lutris,discovery,protondb,export}.rs`; final sync cutover only in `core/src/community/taps/{store,git,tests}.rs` and optional new test runner helper there. Conditional D7 mocks: `src/lib/mocks/handlers/{community,system}.ts` and focused mock tests only if offline_mode is modeled. No service leaf edits except resolving a documented integration error via owning agent.
- **Wire:** four module/re-export registrations; four Services fields/constructors; create one ProfileWriter before struct construction and clone into profiles/community/Lutris; shared app jobs cloned before storage consumes it. Four manage lines, retain raw store manages. Extend bootstrap test for shared registry and bundle construction. Implement exact three From mappers and sanitized-message tests in b0; ensure cancelled git maps Cancelled, paths/stderr/SQL do not cross IPC.
- **Adapters:** all 19 async names remain in Tauri and delegate through State<Service> except accept through ProfileWriter. Preserve String/Option args, mixed keys and existing DTOs/serde; convert PathBuf only internally; force_refresh.unwrap_or(false). Error mapping `.message().to_string()`, including existing accept. Delete moved helper bodies/dedupe test (now app test). Rewrite community/discovery sync pointer pins, Lutris cross-file pointer in export.rs, add all-four ProtonDB pin. Preserve snake export keys/profile_name/output_path and source_id; do not rename camel keys appId/profileName/forceRefresh/suggestionKey/trainerGameVersion.
- **Final cutover:** migrate remaining core sync tests to async injected test runner using existing core gateway and shared invocation builders; delete old sync_many/sync_tap/sync_workspace and synchronous git executor/helper wrappers once sole caller is gone. The test runner is test-only; no legacy production wrapper or deprecated API survives. Do not modify core indexer/scanner owned by 1.1.
- **Mocks:** source inspection found no offline_mode modeling in mocks; leave them unchanged under D7 unless integration discovers it. If modeled, mirror only sync/cache-only search offline policy, not new UI/progress. check-mock-coverage checks names only; it does not prove payload compatibility.
- **Acceptance:** AC4 delegation, all new service modules now compile and all app tests run; service methods use real existing types/one writer, schema unchanged, new native paths reachable.

## Task 3.2: Documentation and follow-up inventory (B3)

- **Owns:** repo-root `AGENTS.md` preserved reference sections, repo-root `CLAUDE.md` architecture service list, `app/README.md`; planning decisions status notes only if needed (never alter D1–D12). No overlap with 3.1.
- **Change:** add four services/command groups, one shared writer and supervised community sync behavior to architecture/API tables. Document intentional normalized validation/offline changes and canonical preview-vs-import policy. Record D11 follow-ups through the orchestrator's GitHub/Linear issue workflow, without changing their scoped-out behavior. No schema/ADR rewrite needed; changelog remains generated from eventual Conventional Commit.
- **Acceptance:** docs match shipped services, process cancellation is not described as a Classic UI cancel feature or race-free reads. PR evidence separates smoke route coverage from backend tests and manual follow-ups.

### B3 gate and AC mapping (main agent; full new graph)

```sh
# src/crosshook-native
cargo test -p crosshook-app community
cargo test -p crosshook-app discovery
cargo test -p crosshook-app protondb
cargo test -p crosshook-app lutris
cargo test -p crosshook-app services_new_builds_bundle_from_stores
cargo test -p crosshook-app error
cargo test -p crosshook-native command_names_match_expected_ipc_contract
cargo test -p crosshook-core community
cargo clippy --workspace --all-targets -- -D warnings
```

| AC  | Required tests/checks                                                                                                              | Batch                 |
| --- | ---------------------------------------------------------------------------------------------------------------------------------- | --------------------- |
| AC1 | core fs_util matrix + index/scanner symlink tests; app preview outside/commit outside/nested                                       | B1 + B2, activated B3 |
| AC2 | gated fake responsive runtime; queued sync; actual child-kill + Cancelled + no metadata row                                        | B2, activated B3      |
| AC3 | normalized dedupe/reject-no-write; bare repo clone/update, offline upsert/timestamp, NULL watermark loading-mode, cached timestamp | B2, activated B3      |
| AC4 | 19 async delegates + command pins + mock coverage + full frontend smoke                                                            | B3/final              |

## Final gate (main agent, once integrated)

```sh
# from repository root (scripts derive repo root themselves)
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace
./scripts/lint.sh
./scripts/check-mock-coverage.sh
./scripts/check-host-gateway.sh
./scripts/check-app-deps.sh
./scripts/check-schema-version.sh

# from src/crosshook-native (package.json scripts verified)
npm run typecheck && npm test
npm run test:smoke
```

Worktree prerequisites: repo-root formatter/linter dependencies per CLAUDE.md:22–25; `npm ci` in `src/crosshook-native`; first-time smoke Chromium setup `npm run test:smoke:install`. Format changed docs/code using repo formatter before final check. One cargo test filter per command. Full smoke covers Community/Discover/Compatibility route render/responsive sanity, not live git/ProtonDB transactions. Manual native checks remain D11 follow-ups, not claimed exercised.

## Risks and delivery

- Point-in-time canonical containment is not race-free open; do not claim D11 handle-pinning work done.
- Async core clients embed synchronous SQLite; use the existing Steam bridge pattern, not direct caller-thread await.
- Core async and index hardening share no files in B1; B3 touches only the explicitly handed-over sync files after its owner completes.
- Intermediate unregistered app modules are a staging limitation, not test evidence; B3 and final gates cannot be omitted.
- Squash title: `refactor(app): move community, Lutris, discovery and ProtonDB into services`; link `Closes #543`. Report checks actually run, blockers and the D5/D7 intentional deltas; no fabricated manual evidence.
