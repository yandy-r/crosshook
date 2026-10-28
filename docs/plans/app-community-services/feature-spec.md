# Feature Spec: community, Lutris, discovery and ProtonDB services in `crosshook-app`

Tracking: Linear YAN-835 / GitHub #543 (M1-34). Target `crosshook v0.7.0-alpha.1`, one PR to `main`, worktree `crosshook-yan-835`. Paths relative to `src/crosshook-native/`. Binding [decisions.md](./decisions.md) D1–D12 override research recommendations. Frozen [b0-contracts.md](./b0-contracts.md); batched [app-community-services.plan.md](./app-community-services.plan.md).

## Problem

19 Tauri commands currently contain tap persistence, import authorization, sync/index/offline orchestration, discovery and ProtonDB assembly. Synchronous `community_sync` calls git `.output()` on the caller thread (`core/community/taps/git.rs:38-177`); moving that call into `run_job` alone cannot kill git. Import containment has no tests, the indexer follows symlinks, and Lutris config/database reads have no root containment. Native consumers need reusable toolkit-independent services, not Tauri `State` orchestration.

## Scope

Four `#[derive(Clone)]` services in `crates/crosshook-app/src/services/{community,lutris,discovery,protondb}/`, positional constructors, async operation methods returning `AppError`, `run_blocking` for store/filesystem work, tests subdirectories and files around 500 lines. Add core containment plus an injected-runner async tap-sync driver. Keep the same core DTOs, TOML/SQLite writes, cache behavior and shared `ProfileWriter` mutation/event path (D1, D8, D10).

## Non-goals

- No schema/TOML migration, new events, new Classic command, response envelope, job-ID IPC response or CLI adoption.
- ProtonDB accept stays directly on `ProfileWriter`; tap remove is service-only and leaves clones/SQLite rows intact.
- No Classic sync cancel UI (no generic cancel IPC). Backend `JobHandle`/registry cancellation is real; route unmount is not cancellation.
- D11 follow-ups remain out of scope: URL private-host/redirect/stream limits, strict numeric ProtonDB ids, fetch-time source revalidation, slug collisions, git env stripping/hooksPath, handle-pinned race-free filesystem reads, manual native UI transaction checks.

## Acceptance criteria and test mapping

| #   | Criterion from issue                                                                                                         | Evidence required                                                                                                                                                                                                                                                                                                                                                                                         |
| --- | ---------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AC1 | Containment rejects `..`, absolute outside, outward symlinks, case/Unicode-normalization tricks; accepts nested valid paths. | Core `fs_util` tempdir matrix: relative/absolute traversal (including ends-inside), outside/root-evil, outward intermediate/final links, dangling link, in-root link, real case/NFC/NFD siblings with JSON; nested relative/absolute success. Index symlink skip and Lutris root-read tests. App `community/tests/imports.rs`: preview outside accepted, commit outside rejected, nested commit succeeds. |
| AC2 | `community_sync` no longer blocks the caller thread; sync can be cancelled.                                                  | App gated-runner current-thread runtime remains responsive; queued jobs do not Conflict; local supervised sleep child is killed/reaped, job yields `Cancelled`, no index/offline/backfill after cancellation, no cached-success fallback.                                                                                                                                                                 |
| AC3 | Tests cover tap dedupe, `last_sync_at` and `trainer_loading_mode` backfill.                                                  | App normalized triple/add-validation tests, preserved settings extras; local bare git fixture proves clone/update/index/offline/read-back, cache fallback preserves old timestamp; set old watermark NULL then re-index and assert loading-mode column populated.                                                                                                                                         |
| AC4 | Tauri commands delegate; Community Browse, Discover and Compatibility smoke pass.                                            | All 19 names and mixed argument keys/serde unchanged; command-name pins including new ProtonDB pin and rewritten Lutris export pin; mock coverage; frontend unit/typecheck + full Playwright smoke. Smoke is route rendering, not native git/containment proof.                                                                                                                                           |

## Architecture

### Shared path primitive (D3–D4)

`crosshook_core::fs_util::ensure_within_workspace(root, candidate) -> Result<PathBuf, WorkspacePathError>` canonicalizes a trusted existing root, rejects raw ParentDir, resolves relative input from root, checks absolute spelling prefix then canonical component containment. No folding/normalization/lossy strings. Missing and dangling candidates fail; in-root symlinks can pass this helper. Callers use the returned canonical path. `WorkspacePathError` Display exposes no paths.

Mutating community import must be a regular `.json` inside some subscribed tap, then use shared `ProfileWriter::import_community`. File-picker preview deliberately accepts any local regular JSON, classifies through the same helper for debug logging only, and reads its canonical path. Indexer skips all symlink files/directories with `DirEntry::file_type`; Lutris validates games YAML, scanned directories and `pga.db` against the chosen/discovered library root. Game executable/prefix locations remain unrestricted (D4).

### Async sync job (D6, D8)

Core owns normalized validation, git env/argv and sync result construction. `TapGitRunner` is `Send + Sync` with an explicit boxed future; core already has async-trait but app does not, so no new dependencies/macros. Clone inserts `--` before URL. `Cancelled` is distinct from failure and never becomes `CachedFallback`.

`CommunityService::start_sync() -> Result<JobHandle, AppError>` uses `JobRegistry::start(JobSpec::new("community-sync"), …)` without a key. A clone-shared Tokio mutex queues overlapping syncs. App runner uses the lifecycle host gateway, process group, bounded retained pipe output and `supervise_child`; Flatpak host work dies with its proxy. Check cancellation before each git/index/persistence phase. Success lives in a take-once `JobId` slot; awaitable `sync()` returns the existing result array and reaps. No blocking `run_job` closure pretending to cancel git.

Sync first completes all taps (first hard error aborts), then each index → offline upsert (warning-only), then timestamp read-back and extra clearing, exactly as today. Metadata-disabled sync still works; cache fallback preserves previous `last_sync_at`. v25 NULL watermark forces loading-mode re-index (D8).

### Wiring, services and errors

`Services::new(&Stores, EventBus, Handle, JobRegistry)` builds one `ProfileWriter`, clones it into profiles/community/Lutris, adds four fields and four Tauri `.manage()` lines; leave unrelated raw-store registrations. All operation methods are async; start/result are synchronous runtime-only lifecycle accessors (storage precedent).

| Service          | Owns                                                                                                  |
| ---------------- | ----------------------------------------------------------------------------------------------------- |
| CommunityService | normalized tap add/remove, list/indexed list, export, preview/contained import, cancellable sync      |
| LutrisService    | preview scan off-thread and import through shared writer                                              |
| DiscoveryService | degraded catalog, cache-only offline external search, version compatibility and external source CRUD  |
| ProtonDbService  | existing six-hour lookup/stale fallback, suggestion assembly, 30-day dismissal; no accept duplication |

New error mappers suppress raw paths, git stderr and SQL (D9); traversal/outside → path Validation, missing → NotFound, git failure → Io, cancelled → Cancelled. Preserve user validation/source-id/unknown-profile/dismiss texts. Tauri maps `.message().to_string()` once.

## Data, persistence and usability

No migration. User-editable taps and external-source subscriptions remain TOML (`SettingsStore::update` preserves extras, response extras cleared). Community index/offline state, search/ProtonDB cache and dismissals remain existing SQLite tables. Job ids/progress/results/tokens/mutex are runtime-only. No persisted busy state or new AppEvent.

Offline sync returns Unavailable before git. External search serves fresh or stale cache without any live fetch; only an active search with no cache returns Unavailable (empty query/no enabled sources stays empty success). Catalog and both community lists remain readable; ProtonDB is not newly gated and retains current cache/stale markers. Keep 1-hour discovery TTL/key/concurrency/size limits, 6-hour ProtonDB TTL and dismissal retention unchanged (D7, D10).

## Risks

| Risk                                                      | Mitigation                                                                                                |
| --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Cancellation becomes cached success or leaves git running | Typed Cancelled + actual supervisor/lifecycle gateway; process test and no-write assertions.              |
| Shared-file parallel conflicts                            | Exhaustive ownership per batch; services/mod.rs, error.rs and stores.rs belong only to B3 integration.    |
| Async client still blocks on embedded SQLite              | Offload existing core async clients through blocking-pool runtime bridge, not direct caller-thread await. |
| Preview regression or IPC drift                           | D4 preview asymmetry explicit; D2 inventory and pin tests; unchanged mocks unless D7 modeled.             |
| Canonicalize/open race                                    | Point-in-time containment only; handle-pinned reads are D11 follow-up, not claimed solved.                |

## Research references

[technical](./research-technical.md) · [business](./research-business.md) · [practices](./research-practices.md) · [security](./research-security.md) · [UX](./research-ux.md). Where older research recommends preview rejection, exclusive/run_job sync, or unconditional offline-search error, binding decisions take precedence.
