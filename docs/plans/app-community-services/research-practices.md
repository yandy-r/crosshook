# YAN-835 practices research and implementer checklist

Read-only research of PR #540 (`4a02977`, YAN-847) and PR #542 (`e2afa7f`, YAN-831), their landed code, and the current community code. Paths are relative to `src/crosshook-native/` unless noted. No builds or tests were run.

## 1. Landed conventions to copy

### Module shape and public API

- Use `services/<name>/mod.rs` plus one responsibility-named file per operation group and `tests/{mod.rs,helpers.rs,<slice>.rs}`. `mod.rs` owns private submodule declarations, the `#[derive(Clone)]` service struct, positional `new()`, public re-exports, `assert_impl_all!(Service: Send, Sync)`, and `#[cfg(test)] mod tests;`. Evidence: `app/src/services/storage/mod.rs:4-6,25-50,68-71`; storage decisions D1 (`docs/plans/app-storage-services/decisions.md:7`).
- Keep files around 500 lines; this is a soft cap, but meaningful overruns require decomposition. Split security/containment tests into their own file rather than recreating a 500+ line command module. Evidence: repo `CLAUDE.md:50-52`; `docs/plans/app-onboarding-services/research-tests.md:122`.
- `crosshook-app` has `#![deny(missing_docs)]`; document every public item. Keep service methods `async` except genuinely synchronous in-memory getters. Return `Result<_, AppError>`, not a new per-service error enum. Use one `run_blocking` closure per multi-step blocking operation, not one per store call. Evidence: `app/src/lib.rs:8`; `app/src/services/mod.rs:1-8,59-67`; storage decisions D1/D11.
- Add module and service re-exports in `app/src/services/mod.rs`; follow existing crate-root re-export practice in `app/src/lib.rs:30-33`. Preserve IPC DTO serde names; reusable response DTOs belong in the service, Tauri-only wrappers stay in the adapter (storage decisions D13).

### Dependencies and wiring (there is no `AppContext`)

- Services receive cheap cloned concrete stores directly; `Arc` is used for shared mutable state or trait-object test seams, not as a blanket store wrapper. Examples: `StorageService::new(profiles, metadata, jobs)` (`storage/mod.rs:36-45`) and `OnboardingService::new(settings, metadata, jobs)` (`bootstrap/stores.rs:115-120`).
- Add fields to `bootstrap::Services` and construct them in `Services::new(&Stores, EventBus, Handle, JobRegistry)` (`bootstrap/stores.rs:49-93`). Reuse the app's `JobRegistry` clone, not `JobRegistry::default()` in production, so shutdown cancellation propagates. Extend `services_new_builds_bundle_from_stores` (`bootstrap/stores/tests.rs:118-160`) with the new services and registry-sharing behavior.
- Register `.manage(services.<name>.clone())` in `src-tauri/src/lib.rs` (current examples at lines 42-58). Do **not** remove raw store `.manage()` calls merely because these commands stop using them; other out-of-scope commands still need them (`lib.rs:37-41`).
- Do not invent `Arc<AppContext>` or storage-port traits. The established dependency injection exception is a narrow external-effect seam such as `Arc<dyn HostProbe>` with production `new()` delegating to test-only `with_probe()` (`services/host_tools/mod.rs:32-38`). Use a similar seam only if network/process behavior genuinely needs deterministic faking.

## 2. Test harness and job tests

- Retain a `tempfile::TempDir` in each harness for its whole test. Build `ProfileStore::with_base_path(root/profiles)`, `SettingsStore::with_base_path(root/config)`, `CommunityTapStore::with_base_path(root/taps)`, and `MetadataStore::open_in_memory()` or `MetadataStore::disabled()`. These are real temp filesystems/stores, not fake-store traits. Examples: `storage/tests/helpers.rs:24-46`; `launcher_export/tests/helpers.rs:38-45`.
- The profile test `Harness` already combines profiles/settings/metadata/EventBus/ProfileWriter (`profiles/tests/helpers.rs:37-44`); reuse that composition pattern for community/Lutris imports. Exercise both enabled and disabled metadata where behavior differs. Never mutate process-wide environment in tests; inject values/paths instead.
- Use `#[tokio::test]` for ordinary services. For the non-blocking proof, copy the landed gated-probe test: current-thread runtime, `max_blocking_threads(1)`, a blocking gate, a watchdog that releases it on failure, assert the caller can progress, then release and await. `host_tools/tests.rs:55-96` is the canonical concise version.
- For `community_sync`, retain a clone of the same `JobRegistry` in the test. While the gated sync is running assert `jobs.job_counts() == (1, 0)`; after completion assert `(0, 0)` to prove `run_job` reaped it. The helper is `#[cfg(test)] pub(crate)` (`jobs/registry.rs:272-278`), so this assertion belongs in unit tests inside `crosshook-app`, not an external integration test.
- `FakeHostProbe` and its gate/call counters live in `services/host_tools/fake.rs:10-19,39-53,88-114`. It is useful only for host-probe work; do not force community network/Git behavior through it. If a community process/network seam is needed, make a comparably small fake with call counts/gating.
- Use subscribe-before-act, channels/terminal events, and bounded timeouts; never sleeps to make races pass. Existing import tests in `profiles/tests/imports.rs` and community tap tests in `core/src/community/taps/tests.rs` should be moved/reused rather than duplicated.

### Local bare-Git tap fixture

**Yes, existing Rust tests invoke real Git.** `core/src/community/taps/tests.rs:15-93` provides `init_repo`, `commit_file` (local user config plus explicit author/committer env), and `rev_parse_head`; `syncs_and_indexes_local_tap_repo` at lines 95-149 clones from a local source path and syncs again. `pinned_tap_stays_on_selected_commit` at lines 261-341 commits an update and re-syncs.

**CI availability:** `lint.yml` runs `actions/checkout@v4` before `cargo test -p crosshook-app -p crosshook-native -p crosshook-cli` (`.github/workflows/lint.yml:21,45-52`), so Git is present on that runner. Core already relies on it; no workflow change is needed. `git` is not in the host-gateway denylist (`scripts/check-host-gateway.sh:7-18`).

For the requested **bare** fixture, adapt the proven helpers into `services/community/tests/helpers.rs`:

1. Create `remote.git`, `seed/`, and `store/` below one retained TempDir. Run `git init --bare --initial-branch=main <remote.git>` (or `git init --bare <remote.git>` then set HEAD for older Git compatibility) and `git init -b main <seed>`.
2. Configure `user.name`/`user.email` locally in `seed`; keep the explicit `GIT_AUTHOR_*` and `GIT_COMMITTER_*` environment from `commit_file` so no global developer/CI identity is required.
3. Write a valid manifest at `profiles/<slug>/community-profile.json`, `git add <relative>`, `git commit -m ...`, then `git -C seed push -u origin main` with `remote.git` as origin. Assert every command status and include stderr on failure.
4. Point `CommunityTapSubscription.url` at `remote.git`'s **absolute filesystem path**, not `file://`; URL validation accepts absolute paths and rejects `file://` (`core/community/taps/validation.rs:31-49`). `CommunityTapStore::sync_workspace`/`sync_many` performs the clone; no test-only bypass is needed.
5. To test updates, commit a second manifest version in `seed`, push, and sync again. For pinned behavior, capture `rev-parse HEAD` before the second commit and assert the original pin remains.
6. Production sync must continue to use core's existing `CommunityTapStore`/Git functions, including `git_security_env_pairs` and Flatpak host handling (`core/community/taps/git.rs:14-35`). Do not spawn Git in `crosshook-app` or Tauri, and do not weaken URL/security validation for tests.

## 3. Tauri adapters, errors, mocks, and docs

- A moved command should be only `pub async fn name(args..., service: State<'_, XService>) -> Result<T, String> { service.method(args...).await.map_err(|e| e.message().to_string()) }`. `commands/storage.rs:13-36` is the exact model. Async commands taking `State` must return `Result`.
- Keep command names, camelCase argument keys, result/error serde shapes, and intentional legacy error text unchanged. Use `AppError` for all service failures and sanitized `.message()` at the boundary, not raw store `Display` (onboarding decisions D8/D12; storage decisions D8/D12). Preserve special mappings explicitly when required by an existing contract.
- Replace obsolete sync fn-pointer signature pins with name-only `let _ = command_name;` pins for every touched command module. Examples: `commands/storage.rs:39-50`, `commands/onboarding.rs:108-120`. Current community/discovery modules still contain sync signature pins and must be converted when their functions become async.
- Profile prepare/import/export must remain separate orchestration from mutation: all actual community/Lutris profile mutations go through the injected existing `ProfileWriter` (`profiles/imports.rs:22-53`), never direct `ProfileStore` writes. Do not create a second writer.
- `scripts/check-mock-coverage.sh` extracts every `#[tauri::command]` and compares names bidirectionally against `map.set(...)` in `src/lib/mocks/handlers` (script lines 54-112,136-140). With names frozen, mocks should remain unchanged. It checks **names only**, not argument/return compatibility (`docs/plans/app-onboarding-services/research-tests.md:81-87`), so service DTO/adapter pins and frontend tests remain necessary.
- Existing relevant mocks: `handlers/community.ts`, `discovery.ts`, `lutris.ts`, and `protondb.ts`. Preserve their stateful semantics, including import events (`mocks/README.md:124-133`). `npm run dev:browser:check` is the same gate as the shell script (`check-mock-coverage.sh:9-11`). Do not add `_mock_` handlers or change mock data to mask backend drift.
- Documentation updates that landed in **both** prior moves: repo `AGENTS.md` architecture bullet, Stack Overview rows, and services directory map; repo `CLAUDE.md` architecture bullet; and `app/README.md` service/API/delegating-command table. See the #540/#542 doc diffs and current `AGENTS.md:44,153-155,177-182`, `app/README.md:93-115`. Add community, Lutris, discovery, and ProtonDB to those lists/tables and record job/cancellation behavior. No targeted change to `docs/architecture/` was required by those moves; ADR-0001 already records core tap Git handling and remains accurate if orchestration moves but Git stays in core.

## 4. Pitfalls from the landed decisions

- **Cancellation is cooperative for blocking work.** `run_job` resolves `Cancelled`, but `spawn_blocking` cannot interrupt the closure; it finishes in the background (`services/mod.rs:69-80,87-112`). Check cancellation between sync/index/offline/backfill phases. Do not claim an in-flight Git subprocess is killed unless a real process-lifecycle implementation proves that. Ensure no post-cancellation writes/events occur.
- **Use the shared, non-exclusive job helper for UI sync.** `run_job` is the #542 convention (`docs/plans/app-onboarding-services/decisions.md:8`). Do not copy storage's exclusive scan key: a second sync caller would get a user-visible `Conflict`. If `JobHandle::cancel()` is the cancellation path, keep the handle/token reachable; a bare `run_job(...).await` only supports registry/shutdown cancellation and cannot be individually cancelled through a returned handle.
- **Avoid side channels when a result can be returned.** Storage needed a JobId-keyed result slot because its start/result APIs were separate; `run_job` already returns the closure value. Community sync should return `Vec<CommunityTapSyncResult>` directly and not reproduce that slot machinery.
- **Keep operation ordering and fail-soft behavior deliberate.** Current sync indexes, upserts offline state, backfills `last_sync_at`, then clears subscription extras (`commands/community.rs:228-256`). Preserve observable ordering and fail-soft indexing/upserts while adding the required tests; do not silently make them atomic or hard failures.
- **Do not broaden scope or change persistence.** Keep TOML taps, SQLite `community_profiles`/`external_cache_entries`, and schemas unchanged. No new events without a consumer (storage practice §1.4); sync progress belongs on job channels, not the global AppEvent bus.
- **Preserve errors without leaking paths.** `AppError::new` sanitizes messages and its `From` implementations deliberately suppress raw I/O paths/panic payloads (`app/src/error.rs:27-41,88-101`). Check legacy strings before mapping and add regression tests for any intentional differences.
- **No Tauri or direct host-tool execution in app services.** `check-app-deps.sh` rejects UI toolkit dependencies; `check-host-gateway.sh` scans app source. Keep core domain/process logic in core and adapters in `src-tauri`.

## 5. Implementation acceptance checklist

### Structure and behavior

- [ ] Add the four cohesive service modules, each split below the file-size cap, documented, `Clone + Send + Sync`, and re-exported through the established surfaces.
- [ ] Inject cloned stores, `ProfileWriter` where needed, and the app `JobRegistry`; extend `Services`, bootstrap wiring tests, and Tauri `.manage()` registrations. Leave shared raw-store registrations intact.
- [ ] Make all blocking store/filesystem operations use `run_blocking`; run sync through non-exclusive `run_job`, preserve result DTOs, and make individual/registry cancellation observable without post-cancel side effects.
- [ ] Delegate every in-scope Tauri command and map `AppError::message()` to the legacy `String`; convert all sync IPC pins to name-only pins. Keep accept-suggestion in its existing ProfileWriter path unless the issue explicitly moves it.
- [ ] Keep all import containment in one helper and route every prepare/import path through it; use real tempdir/symlink adversarial tests for every listed acceptance case.

### Tests and contract coverage

- [ ] Add per-service tempdir harnesses with in-memory/disabled metadata as appropriate; test tap dedupe, `last_sync_at` and `trainer_loading_mode` backfill, import/export, discovery cache/offline behavior, ProtonDB cached data, and Lutris prepare/import through ProfileWriter.
- [ ] Build the local bare remote + seed repository with real Git, push initial and updated commits, and test clone/update/index behavior. Assert Git statuses/stderr; never depend on global Git config or network access.
- [ ] Add gated non-blocking sync proof plus `job_counts()` running/reaped assertions and cancellation coverage; no sleeps.
- [ ] Keep `src/lib/mocks` unchanged unless a separately demonstrated contract change requires otherwise; run mock coverage and existing Browse/Discover/Compatibility frontend tests, not just route rendering.

### Documentation and final gates (main agent after integration)

- [ ] Update `AGENTS.md`, `CLAUDE.md`, and `crosshook-app/README.md` service/command tables; record any observable cancellation/error changes in decisions/PR notes. No schema/changelog/ADR change is implied by this refactor.
- [ ] Run the issue-targeted tests: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app community discovery protondb` (add Lutris/containment filters as needed), `cargo test ... -p crosshook-native` for adapters, and `npm run test:smoke` from `src/crosshook-native`.
- [ ] Run full Rust regression: `cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace`; include `-p crosshook-core` whenever core is touched.
- [ ] Run `cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check` and `cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings` (CI uses all targets at `.github/workflows/lint.yml:45-52`).
- [ ] Run `./scripts/lint.sh`, `./scripts/check-host-gateway.sh` (+ `--self-test`), `./scripts/check-app-deps.sh`, `./scripts/check-startup-path.sh`, `./scripts/check-schema-version.sh`, and `./scripts/check-mock-coverage.sh`.
- [ ] From `src/crosshook-native`, run `npm run typecheck && npm test && npm run lint && npm run dev:browser:check && npm run test:smoke`; finish with repo `./scripts/format.sh --modified --docs`. Fresh worktree prerequisite: `npm ci` under `src/crosshook-native` (and root Biome/TypeScript setup described in `CLAUDE.md:22-26`). One-time smoke browser setup: `npm run test:smoke:install` (`tests/README.md:11-26`).
