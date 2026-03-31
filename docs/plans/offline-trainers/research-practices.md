# Practices Research: offline-trainers

## Executive Summary

CrossHook has extensive reusable infrastructure for the offline-trainers feature: `sha2` is already a direct dependency, `sha256_hex` and `hash_trainer_file` are already public in `crosshook-core::metadata`, and the `TrainerSection.kind` field in `GameProfile` is an empty free-form string that can be extended to a typed enum without a migration. The health scoring system (`profile/health.rs`), profile validation path (`launch/request.rs:ValidationError`), and SQLite migration pattern are all established templates to follow. No new top-level module is needed for v1; offline concerns should be incrementally layered into existing modules.

---

## Existing Reusable Code

### Hashing — Already Present, Already Public

- `/src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs:307` — `pub fn sha256_hex(data: &[u8]) -> String` — generic SHA-256 hex utility, re-exported from `metadata::mod.rs`.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs:215` — `pub fn hash_trainer_file(path: &Path) -> Option<String>` — reads a file and SHA-256 hashes it; already used by the Tauri command layer (`commands/launch.rs:17`). No duplication needed: the offline feature can call this function directly to populate its hash cache.
- `sha2 = "0.11.0"` is already in `/src/crosshook-native/crates/crosshook-core/Cargo.toml` — no new dependency required.

### Profile Model — Trainer Kind Field Ready to Extend

- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs:157-164` — `TrainerSection` has a `kind: String` field serialized as `type` in TOML. Currently a free-form string with no validation. This is the natural insertion point for `TrainerKind::Fling | TrainerKind::Aurora | TrainerKind::Unknown`.
- `TrainerLoadingMode` (same file, line 51) demonstrates the exact pattern to follow: a `Copy` enum with `FromStr`, `as_str`, serde `rename_all = "snake_case"`, `Default`, and unit tests.

### Validation Infrastructure — Copy the Pattern

- `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs:173-222` — `ValidationError` enum is the established error taxonomy for pre-flight checks. Offline readiness errors (trainer not found, hash mismatch, tap not cached) fit naturally as new variants here or as a parallel `OfflineValidationError` enum in the same file.
- `/src/crosshook-native/crates/crosshook-core/src/profile/health.rs` — `check_file_path`, `HealthIssue`, `HealthStatus`, `ProfileHealthReport` — the whole health check machinery already models the "path validity + config completeness + staleness" scoring that offline readiness scores would parallel. The offline readiness score is structurally the same problem with different predicates.

### Metadata / SQLite — Migration Pattern Is Trivial to Extend

- `/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — Schema is at version 12 with a consistent sequential `migrate_N_to_M` pattern. Adding an `offline_cache` table (SHA-256 hash per trainer file path, cached `tap_local_path` flag, `cached_at` timestamp) requires one new migration function and one new `version < 13` block.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `put_cache_entry` / `get_cache_entry` shows the pattern for expiry-aware SQLite caching. The tap offline cache is the same shape: keyed entry, optional `expires_at`, `cached_at` timestamp.
- `/src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs` — `upsert_health_snapshot` / `load_health_snapshots` shows the upsert pattern for per-profile score persistence (three functions, ~100 LOC). An offline readiness score store is identical in shape.

### Community Tap Infrastructure

- `/src/crosshook-native/crates/crosshook-core/src/community/taps.rs` — `CommunityTapStore` already handles clone/fetch/reset via `std::process::Command`. The offline cache detection is a property of whether the `workspace.local_path` exists on disk — no network call needed, and `CommunityTapWorkspace.local_path` is already accessible.
- `sync_tap` / `sync_many` are the sync entry points. An offline-safe path would simply skip these calls and read from the already-cloned `local_path`. The check `workspace.local_path.exists()` is all that's needed.

### Frontend Hooks — `useProfileHealth` as Template

- `/src/crosshook-native/src/hooks/useProfileHealth.ts` — `useReducer` + `invoke` + Tauri event listener pattern. The offline readiness hook would follow the same `batch-loading / batch-complete / single-complete / error` action shape. The `batchValidate` + `revalidateSingle` API surface is the right model.
- `/src/crosshook-native/src/hooks/useLaunchState.ts` — Shows `validateLaunchRequest` → `invoke("validate_launch")` as a pre-flight gate before launch. The offline pre-flight check can slot in the same position, running before `launchGame()` / `launchTrainer()`.

---

## Modularity Design

### Recommended Module Boundaries

**Do not create a standalone `offline/` module for v1.** The offline-trainers feature is a cross-cutting concern that touches existing module surfaces. Creating a new top-level module would duplicate error types, force artificial re-exports, and add indirection for no gain until there are at least three collaborating offline-specific types that have no home elsewhere.

Instead:

| Concern                                      | Where to Add                                                                             |
| -------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `TrainerKind` enum (Fling, Aurora, Unknown)  | `profile/models.rs` — extend `TrainerSection.kind` type                                  |
| Offline readiness score computation          | `profile/health.rs` — add `check_offline_readiness()` alongside `check_profile_health()` |
| SHA-256 hash cache SQLite table + read/write | `metadata/` — new `offline_cache_store.rs` + migration 13                                |
| Tap offline availability check               | `community/taps.rs` — add `is_tap_available_offline()` method on `CommunityTapStore`     |
| Network probe (optional, see KISS section)   | New `metadata/connectivity.rs` or inline in `metadata/mod.rs`                            |
| Pre-flight validation errors                 | `launch/request.rs` — new `ValidationError` variants                                     |
| Offline readiness Tauri commands             | `src-tauri/src/commands/health.rs` or new `offline.rs` in commands/                      |

### Shared vs. Feature-Specific Code

**Shared (already exists, just call it):**

- `sha256_hex` — reuse for trainer hash caching
- `hash_trainer_file` — reuse directly
- `open_in_memory` — reuse for unit tests
- `HealthIssue` / `HealthStatus` — reuse types directly for offline issues

**Feature-specific (new, ~200 LOC total):**

- `TrainerKind` enum + `impl FromStr + as_str`
- `offline_readiness_score(profile: &GameProfile) -> OfflineReadinessScore`
- `OfflineReadinessScore { trainer_cached: bool, tap_cached: bool, hash_verified: bool, score: u8 }`
- Migration 13 + `offline_cache_store.rs` (~100 LOC following `health_store.rs` shape)

---

## KISS Assessment

### Risk: Over-engineering the network detection

The feature description includes "network connectivity detection." This is a KISS trap. Checking actual connectivity (`TcpStream::connect("8.8.8.8:53")` or similar) adds test complexity, is unreliable on captive portals, and is not the actual question the user cares about. The real question is: "Are the trainer files and tap data already on disk?" That check is purely local filesystem state — no network probe needed for v1. If network probing is ever needed, it should be a single, testable function that returns `bool` without external crates.

**Recommendation:** Skip live network probing entirely. Define offline readiness purely as filesystem availability: `trainer_path.exists() && tap_local_path.exists() && stored_hash == hash_trainer_file(trainer_path)`.

### Risk: Separate "offline module" from day one

Creating `crosshook-core/src/offline/` before there are three distinct offline types with no natural home in existing modules is premature. Add to existing modules first; extract only when the rule of three is met (see below).

### Risk: Aurora key modal as a backend concern

The "Aurora offline key info modal" is a UI concern — the key is already present on the user's machine or it isn't. The backend only needs to know `trainer.kind == TrainerKind::Aurora` and whether `offline_key_path` exists (if we even store that). Do not model the key validation logic in the backend for v1; surface it as a UI-layer flag.

### What is actually simple for v1

1. Add `TrainerKind` enum to `profile/models.rs` (20 LOC, follows `TrainerLoadingMode` exactly).
2. Add `offline_readiness_score` to `profile/health.rs` (30 LOC, queries filesystem only).
3. Add migration 13 + `offline_cache_store.rs` to `metadata/` (100 LOC, follows `health_store.rs`).
4. Add `is_tap_available_offline` to `community/taps.rs` (10 LOC, checks `local_path.exists()`).
5. Wire into `useLaunchState.ts` pre-flight check (20 LOC, follows existing `validateLaunchRequest` call).

**Total: ~180 LOC of new code. Zero new dependencies.**

---

## Abstraction vs. Repetition

### Rule of Three Analysis

**Hash computation** — `sha256_hex` (profile sync) and `hash_trainer_file` (version store) already both use SHA-256. A third call site for offline cache hashing justifies the current public API. The abstraction already exists; just call it.

**Health-like scoring** — `check_profile_health` (health.rs), `batch_check_health` (health.rs), and the proposed `offline_readiness_score` would be three scoring functions over profiles. At three, consider whether they share a common `ProfileCheckResult` trait or whether they stay as flat functions. Given current code style (flat public functions, not trait objects), keep them as flat functions until a fourth scoring dimension appears.

**SQLite store modules** — `health_store.rs`, `cache_store.rs`, `version_store.rs`, `launch_history.rs` all follow the same ~100 LOC pattern: a data struct, 2-3 CRUD functions taking `&Connection`. This is repetition by design, not a DRY violation — the repetition is in boilerplate, not logic. An `offline_cache_store.rs` fits this pattern without introducing a new abstraction.

**Pre-flight validators** — `validate_install_request` (install/service.rs) and `validate` (launch/request.rs) are two distinct validators. A third offline-specific validator is not yet abstractable; add offline validation errors as new `ValidationError` variants in the existing enum.

---

## Interface Design

### Rust API Surface (minimal v1)

```rust
// In profile/models.rs
pub enum TrainerKind { Fling, Aurora, Unknown }

// In profile/health.rs
pub struct OfflineReadinessScore {
    pub trainer_cached: bool,
    pub tap_cached: bool,
    pub hash_verified: bool,
    pub score: u8,          // 0-100, computed from above booleans
}
pub fn offline_readiness_score(profile: &GameProfile, stored_hash: Option<&str>) -> OfflineReadinessScore;

// In metadata/offline_cache_store.rs
pub fn upsert_trainer_hash(conn: &Connection, trainer_path: &str, hash: &str, cached_at: &str) -> Result<(), MetadataStoreError>;
pub fn load_trainer_hash(conn: &Connection, trainer_path: &str) -> Result<Option<String>, MetadataStoreError>;

// In community/taps.rs
impl CommunityTapStore {
    pub fn is_tap_available_offline(&self, subscription: &CommunityTapSubscription) -> bool;
}
```

### Tauri Command Layer

- `get_offline_readiness(profile_name: String) -> Result<OfflineReadinessScore, String>` — new command in `commands/health.rs` or `commands/offline.rs`
- `cache_trainer_hash(trainer_path: String) -> Result<String, String>` — calls `hash_trainer_file` + `upsert_trainer_hash`

### Frontend Hook

A `useOfflineReadiness(profileName: string)` hook following the `useProfileHealth` reducer pattern, with actions: `loading | ready | error`. Tauri event `offline-readiness-updated` can reuse the existing `profiles-changed` listener pattern.

---

## Testability Patterns

### Established Patterns in This Codebase

1. **In-memory SQLite** — `db::open_in_memory()` (`metadata/db.rs:53`) provides a clean, isolated DB for every test. The offline cache store tests should call `open_in_memory()` + `run_migrations()` and assert upsert/load round-trips.

2. **`tempfile::tempdir()`** — Used in `community/taps.rs` tests for filesystem-level testing of tap sync. Offline readiness tests can use `tempdir()` to create mock trainer executables and test `hash_trainer_file` against them.

3. **Pure-function testing for scores** — `offline_readiness_score` should be a pure function of `profile: &GameProfile` and `stored_hash: Option<&str>`. No I/O inside the scoring function — I/O (file existence, hash loading) happens at the call site in the Tauri command. This keeps the scoring function unit-testable without mocking.

4. **Existing test coverage gap** — `TrainerSection.kind` has no validation today (it's a free-form String). Adding `TrainerKind` enum with `FromStr` tests follows the `TrainerLoadingMode` test pattern in `models.rs:455-510`.

### Offline-Specific Test Scenarios to Cover

- `offline_readiness_score` returns `score: 0` when trainer path is empty
- `offline_readiness_score` returns `hash_verified: false` when `stored_hash` is `None`
- `upsert_trainer_hash` + `load_trainer_hash` round-trips in in-memory DB
- `is_tap_available_offline` returns `false` when workspace directory does not exist
- `TrainerKind::from_str("fling")` and unknown string falls back to `TrainerKind::Unknown`

---

## Build vs. Depend

### `sha2` — Already a Direct Dependency

`sha2 = "0.11.0"` is in `crosshook-core/Cargo.toml`. `sha256_hex` and `hash_trainer_file` are already public. **No action needed.** Do not add a second SHA crate.

### `git2` — Not Needed, Do Not Add

`community/taps.rs` invokes `git` as a subprocess (`std::process::Command`) deliberately. The comments in the file explain the low-speed-limit environment variable strategy. `git2` (libgit2 FFI) would add ~2MB of compiled weight, require a C toolchain, and complicate the AppImage build. For offline cache detection, `workspace.local_path.exists()` is sufficient — no git library call needed.

### Network Detection — No External Crate Needed

If a network probe is ever desired, `std::net::TcpStream::connect_timeout` against a known endpoint is sufficient and requires no crate. However, see KISS section above — recommend skipping this for v1 entirely.

### Dependency Audit Summary

| Need                       | Crate    | Verdict                                      |
| -------------------------- | -------- | -------------------------------------------- |
| SHA-256 hashing            | `sha2`   | Already present — reuse                      |
| Git operations             | `git2`   | Do not add — subprocess approach is correct  |
| Network connectivity probe | Any      | Skip for v1 — filesystem check is sufficient |
| UUID generation            | `uuid`   | Already present — reuse for cache_id         |
| Datetime                   | `chrono` | Already present — reuse for cached_at        |

---

## Open Questions

1. **Is `trainer.kind` user-editable or auto-detected?** If auto-detected from the trainer filename/path, the detection heuristic needs to be documented. If user-editable, the TOML field migration from free-string to typed enum needs to be graceful (unknown strings → `TrainerKind::Unknown`).

2. **What is the Aurora offline key model?** Does CrossHook need to store a key path, or is the Aurora key knowledge purely in the UI layer? If a key path is stored, it would go in `TrainerSection` alongside `path` and `kind`.

3. **Should offline readiness score be persisted to SQLite (like health snapshots) or computed on demand?** Computing on demand (pure function) avoids staleness issues. Persisting avoids recomputation across app restarts. Given the small computation cost (one `fs::metadata` call + one hash comparison), on-demand is simpler.

4. **What is the scope of "community tap offline caching"?** The taps are already cloned to `~/.local/share/crosshook/community/taps/`. If "offline caching" means ensuring this directory exists before disconnecting, that's just documentation. If it means bundling taps into the AppImage, that's a different design problem.

5. **Two-step launch guard placement:** Should offline validation block the "Launch Game" button (step 1) or the "Launch Trainer" button (step 2)? Since the trainer is step 2, validating trainer readiness before step 1 would block game launch unnecessarily. The guard should be in `launchTrainer()` in `useLaunchState.ts`, not `launchGame()`.

---

## Addenda (from business analysis cross-check)

### `VersionCorrelationStatus` as Pre-flight Signal

`compute_correlation_status` in `metadata/version_store.rs` already returns `VersionCorrelationStatus::TrainerChanged` when the stored hash differs from the current file hash. The offline pre-flight check should call this function and surface `TrainerChanged` as a readiness warning — do not re-implement the hash comparison inline.

### `trainer.kind` Classification — Preferred v1 Approach

A pure interpretation function `fn classify_trainer_type(kind: &str) -> TrainerOfflineCapability` is less disruptive than introducing a `TrainerKind` enum for v1:

- No TOML schema change
- Backward compatible with any existing free-form `kind` values in user profiles
- Can be promoted to an enum in v2 once the set of recognized values is stable

Both approaches are compatible — the function can be the public API with an internal enum if desired.

### `batch_check_health()` Composition

The offline readiness sweep should call `batch_check_health()` first, then layer trainer-type and hash checks only on top of profiles that pass the base health check. This avoids redundant work and keeps offline readiness as a strict superset of the existing health check, not a parallel implementation.

---

## Addenda (from recommendations-agent cross-check)

### `onboarding/readiness.rs` — Most Direct Reuse Target (Missed in Initial Research)

`check_system_readiness()` / `evaluate_checks()` in `crates/crosshook-core/src/onboarding/readiness.rs` is the closest existing pattern to an offline pre-flight check and was not identified in the initial pass.

Key structural points:

- `evaluate_checks(steam_roots, proton_tools)` accepts explicit inputs — no hidden I/O inside the evaluation logic. I/O happens at the `check_system_readiness()` entry point, which then delegates to `evaluate_checks`. This is the exact testability pattern to follow for offline readiness.
- Returns `ReadinessCheckResult { checks: Vec<HealthIssue>, all_passed: bool, critical_failures: usize, warnings: usize }` from `onboarding/mod.rs`. This type is already serializable and IPC-safe.
- `offline_readiness_check(profile: &GameProfile, stored_hash: Option<&str>) -> ReadinessCheckResult` should return this same type — consistent return type across system readiness (onboarding) and offline readiness (launch pre-flight).

**Revised interface recommendation for offline pre-flight:**

```rust
// In profile/health.rs or onboarding/ — returns the same ReadinessCheckResult type
pub fn check_offline_readiness(
    profile: &GameProfile,
    stored_hash: Option<&str>,
    tap_local_paths: &[PathBuf],
) -> ReadinessCheckResult;
```

### `external_cache_entries` Table for Tap Offline Metadata

The `external_cache_entries` SQLite table (migration 3 to 4) already has `cache_key`, `payload_json`, `fetched_at`, `expires_at`. It is the right place to store community tap offline availability metadata without a new table. A dedicated `offline_cache_store` module is not needed if tap cache metadata fits within this existing generic cache.

**However:** trainer file hashes are not JSON payloads — they are single hex strings with a file-path key. Using `external_cache_entries.payload_json` for a plain hex string is a minor abuse of the schema. The simpler approach is to reuse `version_snapshots.trainer_file_hash` (already per-profile in migration 8 to 9) rather than adding any new table.

### `trainer_file_hash` Already in `version_snapshots`

`version_snapshots` table (migration 8 to 9) already has `trainer_file_hash TEXT` per-profile. The offline feature does not need a new SQLite table for trainer hash caching — it should read and write `version_snapshots.trainer_file_hash` through the existing `version_store.rs` functions.

This reduces the new code estimate further: migration 13 is not needed for trainer hash storage. The only remaining new SQLite concern is tap offline availability, which can use `external_cache_entries`.

---

## Addenda (from security-researcher cross-check)

### Proposed Security Utilities — Engineering Assessment

**#1 Constant-time hash comparison (`subtle` crate) — Defer**

All hash comparisons in the codebase (`version_store.rs:202`, `config_history_store.rs:53`) compare locally-computed hex strings against locally-stored hex strings over SQLite. Timing attacks on this comparison path are not a realistic threat for a desktop process running as the user. Adding `subtle` as a new dependency introduces supply-chain surface for a non-applicable threat. Decision: add a `// SAFETY: timing-safe comparison not required here — local process only` comment at comparison sites; do not add `subtle` for v1. Revisit if CrossHook ever exposes hash comparison over a network IPC boundary.

**#2 Bounded file permission setter — Accept**

`PermissionsExt` / `fs::set_permissions` appears in 12 source files: `metadata/db.rs`, `profile/health.rs`, `launch/script_runner.rs`, `launch/request.rs`, `launch/optimizations.rs`, `launch/preview.rs`, `install/service.rs`, `update/service.rs`, `export/launcher.rs`, `onboarding/readiness.rs` (test only). Rule of three satisfied many times over.

Add to `crates/crosshook-core/src/fs_util.rs` (~15 LOC):

- `fn set_private_file_permissions(path: &Path) -> io::Result<()>` — sets 0o600
- `fn set_private_dir_permissions(path: &Path) -> io::Result<()>` — sets 0o700

Migrate only production permission-setting sites; leave test helper `chmod` calls in place.

**#3 Path confinement utility — Defer**

Does not apply to user-configured profile paths (trainer, game, Proton paths are intentionally open-ended — `/mnt/games/`, `/home/user/Downloads/` etc.). Applicable only to CrossHook-internal generated paths (tap workspaces, prefix directories). Extract if/when two or more internal path confinement sites exist.

**#4 Git command hardening — Accept (in-place change)**

Extend `git_command()` in `community/taps.rs` with three additional env vars:

```rust
command.env("GIT_CONFIG_NOSYSTEM", "1")
       .env("GIT_CONFIG_GLOBAL", "/dev/null")
       .env("GIT_TERMINAL_PROMPT", "0");
```

No new utility needed — this is a 3-line addition to the existing function.

**#5 Cache key validation — Accept (inline guard, not shared utility)**

`put_cache_entry` in `metadata/cache_store.rs` has no key validation. Add an inline 3-line guard at function entry (non-empty, `len() <= 512`, no null bytes), returning `MetadataStoreError::Validation`. Do not extract as a shared utility until a second caller of `put_cache_entry` exists.
