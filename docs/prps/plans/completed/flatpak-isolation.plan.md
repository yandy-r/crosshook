# Plan: Flatpak Per-App Isolation & First-Run Migration (Phase 4)

## Summary

Retire the Phase 1 XDG host-override as the _default_ Flatpak behaviour and replace it with proper per-app isolation plus a first-run migration from the host AppImage data tree. Config (`~/.config/crosshook/`) is fully copied into the sandbox (`$XDG_CONFIG_HOME/crosshook/`); data (`~/.local/share/crosshook/`) is copied selectively (metadata DB + community taps + media + launchers; skip prefixes/artifacts/cache/logs/runtime-helpers); wine prefixes keep pointing at the host path via a prefix-root override. An opt-in `CROSSHOOK_FLATPAK_HOST_XDG=1` env var restores Phase 1 shared-mode for power users. Unblocks Flathub submission (#206).

## User Story

As a Flathub reviewer / Fedora Silverblue user installing CrossHook as a Flatpak, I want the app to honour standard Flatpak per-app data isolation (`~/.var/app/dev.crosshook.CrossHook/`) while still importing my existing AppImage settings and metadata on first launch, so that CrossHook is eligible for Flathub and my profiles/games aren't silently orphaned.

## Problem → Solution

**Current (Phase 1)**: `crosshook_core::platform::xdg::override_xdg_for_flatpak_host_access()` is called unconditionally at the top of `crosshook_native::run()` and rewrites `XDG_{CONFIG,DATA,CACHE,STATE}_HOME` back to host defaults so the Flatpak shares one directory tree with the AppImage. Flathub reviewers will reject this.

**Desired (Phase 4)**: When running inside a Flatpak sandbox:

1. Leave XDG env vars at Flatpak defaults (sandbox per-app dirs) **by default**.
2. On first run, if `$XDG_CONFIG_HOME/crosshook/` is empty and `$HOME/.config/crosshook/` exists, copy host config into sandbox config (staged + atomic rename, mirroring `app_id_migration.rs`).
3. Selectively copy the "small & hot" host data subtrees (`metadata.db{,-wal,-shm}`, `community/`, `media/`, `launchers/`) into sandbox `$XDG_DATA_HOME/crosshook/`. Skip `prefixes/`, `artifacts/`, `cache/`, `logs/`, `runtime-helpers/`.
4. Keep wine prefixes on the host via a prefix-root override that re-derives the default from `$HOME/.local/share/crosshook/prefixes/` regardless of the sandbox `XDG_DATA_HOME`. Existing profiles already store absolute `runtime.prefix_path` values (Phase 1) and stay valid without rewrites.
5. Emit a `flatpak-migration-complete` Tauri event so the UI can show a one-time toast.
6. `CROSSHOOK_FLATPAK_HOST_XDG=1` restores Phase 1 shared-mode (opt-in, undocumented on Flathub) for users who explicitly want the AppImage and Flatpak to share one tree.

## Metadata

- **Complexity**: Large (11 tasks, ~15 files, new module + cross-cutting startup wiring)
- **Source**: GitHub issue [#212](https://github.com/yandy-r/crosshook/issues/212) (per-app isolation tracking); unblocks [#206](https://github.com/yandy-r/crosshook/issues/206) (Flathub submission); child of [#210](https://github.com/yandy-r/crosshook/issues/210) (Phase 4 tracker) and [#69](https://github.com/yandy-r/crosshook/issues/69) (Flatpak distribution).
- **Source PRD**: `docs/prps/prds/flatpak-distribution.prd.md` (§10.2 decision, §10.3 Phase 4 follow-up)
- **PRD Phase**: Phase 4 — Flathub submission prerequisite
- **Estimated Files**: ~15 (6 CREATE in `crosshook-core/src/flatpak_migration/` + `fs_util.rs` + integration test; ~9 UPDATE across `lib.rs`, `platform/xdg.rs`, `install/service.rs`, `run_executable/service/adhoc_prefix.rs`, `app_id_migration.rs`, frontend toast hook, 3 docs files)

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2      | —          | 2              |
| B2    | 2.1, 2.2, 2.3 | B1         | 3              |
| B3    | 3.1           | B2         | 1              |
| B4    | 4.1, 4.2      | B3         | 2              |
| B5    | 5.1, 5.2, 5.3 | B4         | 3              |

- **Total tasks**: 11
- **Total batches**: 5
- **Max parallel width**: 3

### Safety checks

- [x] Every task has exactly one `BATCH` assignment.
- [x] Every `Depends on` reference points to a prior task.
- [x] Files touched per batch are disjoint (B2 tasks all create new submodule files; B4 tasks touch disjoint files; B5 tasks touch disjoint files).
- [x] No cycles.

---

## Worktree Setup

- **Parent**: `~/.claude-worktrees/crosshook-flatpak-isolation/` (branch: `feat/flatpak-isolation`)
- **Children** (per parallel task; merged back at the end of each batch):
  - Task 1.1 → `~/.claude-worktrees/crosshook-flatpak-isolation-1-1/` (branch: `feat/flatpak-isolation-1-1`)
  - Task 1.2 → `~/.claude-worktrees/crosshook-flatpak-isolation-1-2/` (branch: `feat/flatpak-isolation-1-2`)
  - Task 2.1 → `~/.claude-worktrees/crosshook-flatpak-isolation-2-1/` (branch: `feat/flatpak-isolation-2-1`)
  - Task 2.2 → `~/.claude-worktrees/crosshook-flatpak-isolation-2-2/` (branch: `feat/flatpak-isolation-2-2`)
  - Task 2.3 → `~/.claude-worktrees/crosshook-flatpak-isolation-2-3/` (branch: `feat/flatpak-isolation-2-3`)
  - Task 3.1 → `~/.claude-worktrees/crosshook-flatpak-isolation-3-1/` (branch: `feat/flatpak-isolation-3-1`)
  - Task 4.1 → `~/.claude-worktrees/crosshook-flatpak-isolation-4-1/` (branch: `feat/flatpak-isolation-4-1`)
  - Task 4.2 → `~/.claude-worktrees/crosshook-flatpak-isolation-4-2/` (branch: `feat/flatpak-isolation-4-2`)
  - Task 5.1 → `~/.claude-worktrees/crosshook-flatpak-isolation-5-1/` (branch: `feat/flatpak-isolation-5-1`)
  - Task 5.2 → `~/.claude-worktrees/crosshook-flatpak-isolation-5-2/` (branch: `feat/flatpak-isolation-5-2`)
  - Task 5.3 → `~/.claude-worktrees/crosshook-flatpak-isolation-5-3/` (branch: `feat/flatpak-isolation-5-3`)

**Setup prerequisites** (per CLAUDE.md §Worktree setup prerequisites): in each worktree root, run `npm install -D --no-save typescript@<project-version> biome`, then `cd src/crosshook-native && npm ci`. Required for `./scripts/lint.sh` and `./scripts/format.sh` to succeed from a worktree.

---

## UX Design

### Before

```
┌───── Flatpak launch (Phase 1) ─────┐
│ 1. override_xdg_for_flatpak_host_  │
│    access() → XDG_* = host         │
│ 2. BaseDirs resolves to host paths │
│ 3. SettingsStore/MetadataStore     │
│    read/write at host paths        │
│ 4. (Flathub rejects — no sandbox   │
│    isolation)                      │
└────────────────────────────────────┘
```

### After

```
┌───── Flatpak launch (Phase 4 default) ─────┐
│ 1. CROSSHOOK_FLATPAK_HOST_XDG?             │
│    ├─ set=1 → Phase 1 override (opt-in)    │
│    └─ unset → per-app isolation (default)  │
│ 2. flatpak_migration::run()                │
│    ├─ detect first run (sandbox empty +    │
│    │   host populated)                     │
│    ├─ copy host config → sandbox config    │
│    ├─ selective data copy (metadata.db,    │
│    │   community, media, launchers)        │
│    ├─ skip prefixes/artifacts/cache/logs   │
│    └─ emit "flatpak-migration-complete"    │
│ 3. BaseDirs resolves to sandbox paths      │
│ 4. Stores open inside sandbox; prefixes    │
│    resolve to host $HOME/.local/share/...  │
│    /crosshook/prefixes/ via override       │
│ 5. UI shows one-time toast (sessionStorage │
│    dedup)                                  │
└────────────────────────────────────────────┘
```

### Interaction Changes

| Touchpoint                     | Before                                                   | After                                                                                               | Notes                                                                                         |
| ------------------------------ | -------------------------------------------------------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| First Flatpak launch (fresh)   | Empty UI, data orphaned (pre-Phase-1 bug)                | Data imported from host tree; toast confirms                                                        | Only fires if host tree exists and sandbox tree is empty; idempotent on re-launch.            |
| First Flatpak launch (no data) | Empty UI                                                 | Empty UI (no migration, no toast)                                                                   | Fresh install from Flathub with no AppImage history — nothing to import.                      |
| Flatpak re-launch              | Reads host tree                                          | Reads sandbox tree                                                                                  | Host tree stays untouched after one-way import.                                               |
| Wine prefix launch             | Resolves `runtime.prefix_path` (absolute, already works) | Same (absolute path); new default prefix root → host `$HOME/.local/share/crosshook/prefixes/<slug>` | New profiles created under Flatpak land on host tree, keeping them AppImage-interop-friendly. |
| Host-shared opt-in             | Always-on (Phase 1)                                      | `flatpak override --user --env=CROSSHOOK_FLATPAK_HOST_XDG=1 dev.crosshook.CrossHook`                | Documented in `packaging/flatpak/README.md`; not on Flathub.                                  |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                                    | Lines      | Why                                                                                                             |
| -------------- | --------------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs`                    | 1-471      | Canonical one-time migration pattern — `copy_dir_recursive`, staged rename, `_for_roots` test seam, error enum. |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/platform/xdg.rs`                        | 1-118      | The function this plan gates; `EnvSink` test seam; `apply_xdg_host_override` contract.                          |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/lib.rs`                                             | 28-160     | Startup sequence; where migration must sit; Tauri `emit` pattern at lines 207-248.                              |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/settings/store.rs`                      | 15-147     | `SettingsStoreError` pattern, `try_new` base-path derivation (first store opened after migration).              |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/metadata/store.rs`                      | 17-23      | Metadata DB path derivation (second store opened after migration).                                              |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/install/service.rs`                     | 163-172    | `resolve_prefix_root()` — target for the host-prefix-root override.                                             |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/run_executable/service/adhoc_prefix.rs` | 1-80       | Secondary prefix-root consumer; must stay aligned with install/service override.                                |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/platform/tests/common.rs`               | 1-84       | `ScopedEnv`/`FakeEnv` helpers for env-var-scoped tests.                                                         |
| P1 (important) | `docs/prps/prds/flatpak-distribution.prd.md`                                            | §10.1-10.3 | Full Phase 1 decision log and Phase 4 follow-up spec.                                                           |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/mod.rs`             | 14-28      | Schema-version bookkeeping (won't be extended here — DB is copied as-is — but confirms v23 contract).           |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/settings/types.rs`                      | 14-252     | `AppSettingsData` pattern in case a future toggle field is added (out of scope for this plan).                  |
| P2 (reference) | `packaging/flatpak/dev.crosshook.CrossHook.yml`                                         | all        | Manifest `finish-args` — confirms `--filesystem=home` is present (required for host copy + prefix access).      |
| P2 (reference) | `docs/architecture/adr-0001-platform-host-gateway.md`                                   | all        | Scope boundary for host-gateway rules; this plan stays on the in-sandbox-Rust side of the boundary.             |

## External Documentation

| Topic                              | Source                                                                             | Key Takeaway                                                                                                              |
| ---------------------------------- | ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| Flatpak per-app data layout        | https://docs.flatpak.org/en/latest/sandbox-permissions.html#filesystem-access      | Default XDG\_\* remap to `~/.var/app/<app-id>/{config,data,cache}/`; `--filesystem=home` exposes host `$HOME` to sandbox. |
| Flatpak `HOST_XDG_*_HOME` env vars | https://docs.flatpak.org/en/latest/sandbox-permissions.html#environment-variables  | Carry the host's real XDG values (possibly custom). Already consumed by `apply_xdg_host_override`.                        |
| `flatpak override --user --env=…`  | https://docs.flatpak.org/en/latest/flatpak-command-reference.html#flatpak-override | Canonical way for users to set `CROSSHOOK_FLATPAK_HOST_XDG=1` persistently per-app without editing the manifest.          |

No crate-level external research needed — all filesystem + migration work uses `std::fs`, `directories`, `tracing`, and `tempfile` (all already deps).

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly — no inventing new conventions.

### ERROR_HANDLING — hand-rolled typed enum (no `anyhow`, no `thiserror`)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:13-48
pub enum AppIdMigrationError {
    Io { path: PathBuf, source: std::io::Error },
    DestinationNotEmpty(PathBuf),
}
impl std::error::Error for AppIdMigrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self { Self::Io { source, .. } => Some(source), _ => None }
    }
}
```

Apply to `FlatpakMigrationError` with variants `Io { path, source }`, `SourceMissing(PathBuf)`, `DestinationNotEmpty(PathBuf)`, `HomeDirectoryUnavailable`. `Cargo.toml:45` explicitly avoids `anyhow`/`thiserror` — do NOT add either.

### RECURSIVE_COPY — symlink-preserving, std-only

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:60-102
fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        // file_type.is_symlink() → copy_symlink; is_dir() → recurse; else fs::copy
    }
}
```

Extracted into `crosshook-core/src/fs_util.rs` (Task 1.2) and re-used by both `app_id_migration.rs` and the new `flatpak_migration::copier`. No `walkdir`, no `fs_extra` — keep dep surface minimal.

### STAGED_ATOMIC_RENAME — invariant "new_root non-empty ⇒ migration succeeded"

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:160-192
let stage = new_root.parent().unwrap_or(&...).join(&stage_name);
if let Err(copy_err) = copy_dir_recursive(old_root, &stage) {
    let _ = fs::remove_dir_all(&stage);
    return Err(...);
}
fs::rename(&stage, new_root)?;
```

`flatpak_migration::copier::copy_tree_or_rollback` mirrors this: copy into `<dest>.migrating` sibling, rename into place on success, `remove_dir_all` on failure. Partial copies never land at the final path.

### IDEMPOTENCY — filesystem-state driven, no sentinel file

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:115-141
if !old_root.exists() { return Ok(()); }  // nothing to migrate
if new_root.exists() && !dir_is_empty(new_root)? {
    return Err(DestinationNotEmpty(new_root.to_path_buf()));
}
```

The migration is self-extinguishing: after the first successful run, `new_root` is non-empty, so subsequent launches short-circuit. No marker file, no DB row, no settings flag. `flatpak_migration` inherits this.

### ENV_SINK — test-injectable env-var seam

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/platform/xdg.rs:50-71
pub(crate) trait EnvSink {
    fn set(&mut self, key: &str, value: &OsString);
    fn get(&self, key: &str) -> Option<OsString>;
}
struct SystemEnv; impl EnvSink for SystemEnv { /* unsafe env::set_var */ }
```

Phase 4 reads `CROSSHOOK_FLATPAK_HOST_XDG` at startup. Inject through `EnvSink` to keep tests deterministic — reuse `platform::tests::common::FakeEnv` + `ScopedEnv`.

### LOGGING — `tracing` structured fields + dual-output during startup

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:194-198
tracing::warn!(old = %old_root.display(), new = %new_root.display(), error = %e, "migration failed");
eprintln!("CrossHook: flatpak first-run migration failed: {e}");  // tracing subscriber not up yet
```

Startup-ordered code runs BEFORE `logging::init_logging` — always pair `tracing::*` with `eprintln!`. Migration success logs a single `tracing::info!` with counts (bytes copied, subtrees skipped).

### TAURI_EVENT_EMIT — one-time startup signal

```rust
// SOURCE: src/crosshook-native/src-tauri/src/lib.rs:207-248
#[derive(serde::Serialize)] struct FlatpakMigrationCompletePayload { imported_config: bool, imported_subtrees: Vec<String> }
if let Err(error) = app_handle.emit("flatpak-migration-complete", &payload) {
    tracing::warn!(%error, "failed to emit flatpak-migration-complete event");
}
```

Emit from within the Tauri `setup` closure (after handle is available). Event name follows the kebab-case convention already in use (`onboarding-check`, `startup-health-scan-complete`). Frontend handles toast UI + `sessionStorage` dedup via the `HEALTH_BANNER_DISMISSED_SESSION_KEY` pattern.

### FRONTEND_TOAST_DEDUP — sessionStorage key

```ts
// SOURCE: src/crosshook-native/src/components/pages/profiles/useProfilesPageNotifications.ts:32-46
const [dismissed, setDismissed] = useState(() => {
  try {
    return sessionStorage.getItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY) === '1';
  } catch {
    return false;
  }
});
```

Reuse the exact shape. Key constant lives beside the new hook (`useFlatpakMigrationToast.ts`).

### TEST_STRUCTURE — `_for_roots` seam + tempdir fixtures

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:247-276
#[cfg(test)] fn migrate_legacy_tauri_app_id_xdg_directories_for_roots(
    config_dir: &Path, data_local_dir: &Path, cache_dir: &Path,
) -> Vec<AppIdMigrationError> { /* same body, test-injected roots */ }
```

`flatpak_migration::run_for_roots(host_root: &Path, sandbox_root: &Path) -> Result<_, _>` is the analogous test seam. Unit tests use `tempfile::tempdir()` + this helper; integration tests under `crates/crosshook-core/tests/flatpak_migration_integration.rs` exercise the full include/skip matrix.

### TEST_MATRIX — failure-policy coverage

```
// SOURCE: src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs:284-400
migrates_when_destination_missing
no_op_when_source_missing
skips_when_destination_non_empty
migrates_when_destination_exists_empty
one_root_failure_does_not_stop_others
```

Mirror this exact matrix per migrated subtree: config, each data subtree in the include list, the skipped subtrees (assert NOT copied).

### NAMING_CONVENTION — directory modules are facade-only

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/platform/mod.rs:10-29
mod detect; mod gateway; mod host_fs; mod steam_deck; mod xdg;
pub use detect::{is_flatpak, normalize_flatpak_host_path};
pub use xdg::override_xdg_for_flatpak_host_access;
```

New `flatpak_migration/mod.rs` follows: one `mod` line per submodule, selective `pub use` re-exports, no logic in `mod.rs`.

### STARTUP_ORDERING — XDG decision before any BaseDirs::new()

```rust
// SOURCE: src/crosshook-native/src-tauri/src/lib.rs:28-42
// Must run before any BaseDirs::new() call (including the migration below
// and every store try_new), because directories crate reads XDG env vars
// at construction time.
unsafe { override_xdg_for_flatpak_host_access() };  // becomes conditional (Task 4.1)
migrate_legacy_tauri_app_id_xdg_directories();
// ... then SettingsStore::try_new(), MetadataStore::try_new(), etc.
```

Task 4.1 replaces line 39 with the gated decision: env-var set → Phase 1 path; unset → call `flatpak_migration::run()`. Both branches must complete before line 42 and every subsequent `*Store::try_new`.

---

## Files to Change

| File                                                                                    | Action | Justification                                                                                                                                                                          |
| --------------------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/mod.rs`               | CREATE | Facade + submodule declarations + public `run` / `run_for_roots` / `host_prefix_root` API.                                                                                             |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/types.rs`             | CREATE | `FlatpakMigrationError` enum, `MigrationOutcome` struct, include/skip manifest constants.                                                                                              |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/detector.rs`          | CREATE | First-run detector: sandbox-config-empty + host-config-populated predicates.                                                                                                           |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/copier.rs`            | CREATE | `copy_tree_or_rollback` (staged rename) + selective-data orchestrator that enumerates the include/skip manifest.                                                                       |
| `src/crosshook-native/crates/crosshook-core/src/flatpak_migration/prefix_root.rs`       | CREATE | `host_prefix_root() -> PathBuf` — derives `$HOME/.local/share/crosshook/prefixes/` regardless of sandbox `XDG_DATA_HOME`.                                                              |
| `src/crosshook-native/crates/crosshook-core/src/fs_util.rs`                             | CREATE | Shared `copy_dir_recursive`, `copy_symlink`, `dir_is_empty` extracted from `app_id_migration.rs`; re-used by the new migrator.                                                         |
| `src/crosshook-native/crates/crosshook-core/src/app_id_migration.rs`                    | UPDATE | Re-use `fs_util` helpers instead of private copies. No behavior change.                                                                                                                |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs`                                 | UPDATE | Register `pub mod flatpak_migration;` and `mod fs_util;`.                                                                                                                              |
| `src/crosshook-native/crates/crosshook-core/src/platform/xdg.rs`                        | UPDATE | Update doc comment to reflect Phase 4 (function becomes opt-in, not default). No API change.                                                                                           |
| `src/crosshook-native/crates/crosshook-core/src/install/service.rs`                     | UPDATE | `resolve_prefix_root()` consults `flatpak_migration::host_prefix_root()` when in Flatpak + isolation mode.                                                                             |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/service/adhoc_prefix.rs` | UPDATE | Same prefix-root override for ad-hoc run-executable flow.                                                                                                                              |
| `src/crosshook-native/src-tauri/src/lib.rs`                                             | UPDATE | Replace unconditional `override_xdg_for_flatpak_host_access()` with env-var-gated decision; call `flatpak_migration::run()`; emit `flatpak-migration-complete` event in setup closure. |
| `src/crosshook-native/crates/crosshook-core/tests/flatpak_migration_integration.rs`     | CREATE | End-to-end integration test: synthetic host tree + sandbox root; asserts include/skip, idempotency, rollback on forced failure, prefix-root resolution.                                |
| `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts`                            | CREATE | Listens for `flatpak-migration-complete` event, shows one-time toast with sessionStorage dedup.                                                                                        |
| `src/crosshook-native/src/App.tsx` (or the equivalent shell component)                  | UPDATE | Mount `useFlatpakMigrationToast()` once near the root.                                                                                                                                 |
| `docs/architecture/adr-0003-flatpak-per-app-isolation.md`                               | CREATE | ADR documenting the isolation contract, env-var opt-in, prefix-root override, migration invariants.                                                                                    |
| `docs/prps/prds/flatpak-distribution.prd.md`                                            | UPDATE | §10.3 status → "in-progress"; add forward reference to this plan.                                                                                                                      |
| `packaging/flatpak/README.md`                                                           | UPDATE | Document `flatpak override --user --env=CROSSHOOK_FLATPAK_HOST_XDG=1 dev.crosshook.CrossHook` for power users.                                                                         |
| `AGENTS.md`                                                                             | UPDATE | SQLite Metadata DB / data classification section: note the Flatpak per-app isolation default and host-prefix-root override.                                                            |

## NOT Building

- **Settings-field UI toggle for shared-mode**: The issue proposes either a settings field OR env var. This plan uses env var only (simpler, no chicken-and-egg during startup, and Flathub reviewers may object to a hidden settings field per PRD §10.3). A future settings UI affordance is out of scope.
- **Metadata DB schema bump**: The existing `metadata.db` file is copied verbatim; no new table, no new `user_version`. Migration bookkeeping is filesystem-state driven, not DB-driven.
- **Two-way sync between AppImage and Flatpak**: Migration is explicitly one-way. Host edits after migration do not propagate.
- **Wine prefix migration**: Prefixes stay on host. No copy path, no prompt, no multi-GB transfer.
- **Changes to `is_flatpak()` callsites in `launch/`, `steam/`, `protonup/`, `community/taps/git.rs`, `settings/paths.rs:31`**: These are host-vs-sandbox subprocess dispatch decisions, not storage-path decisions. Out of scope.
- **AppImage-side behaviour changes**: The AppImage continues to use host XDG paths exactly as today.
- **Removing `override_xdg_for_flatpak_host_access()`**: Kept as a gated opt-in (`CROSSHOOK_FLATPAK_HOST_XDG=1`) per issue item 5. Do NOT delete the function; only change when it runs.
- **New dependencies**: No `fs_extra`, no `walkdir`, no `anyhow`, no `thiserror`. Use `std::fs` + existing patterns.
- **`app_id_migration.rs` behaviour change**: Only the source of `copy_dir_recursive` moves (to `fs_util`). The migration logic itself is untouched.

---

## Step-by-Step Tasks

### Task 1.1: Module skeleton + error enum + include/skip manifest — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-1-1/` (branch: `feat/flatpak-isolation-1-1`)
- **ACTION**: Create `crosshook-core/src/flatpak_migration/` with `mod.rs`, `types.rs`, and empty stubs for `detector.rs`, `copier.rs`, `prefix_root.rs`. Register the module in `crates/crosshook-core/src/lib.rs`.
- **IMPLEMENT**: In `types.rs`: define `FlatpakMigrationError` (variants `Io { path, source }`, `SourceMissing(PathBuf)`, `DestinationNotEmpty(PathBuf)`, `HomeDirectoryUnavailable`) + `Display` / `std::error::Error` impls with `source()` delegation; `MigrationOutcome { imported_config: bool, imported_subtrees: Vec<&'static str>, skipped_subtrees: Vec<&'static str> }`. Constants: `CONFIG_ROOT_SEGMENT = "crosshook"`, `DATA_INCLUDE_SUBTREES: &[&str] = &["crosshook/community", "crosshook/media", "crosshook/launchers"]`, `DATA_INCLUDE_FILES: &[&str] = &["crosshook/metadata.db", "crosshook/metadata.db-wal", "crosshook/metadata.db-shm"]`, `DATA_SKIP_SUBTREES: &[&str] = &["crosshook/prefixes", "crosshook/artifacts", "crosshook/cache", "crosshook/logs", "crosshook/runtime-helpers"]`. `mod.rs` declares submodules and adds placeholder `pub fn run() -> Result<MigrationOutcome, FlatpakMigrationError> { unimplemented!() }` so downstream batches compile.
- **MIRROR**: `ERROR_HANDLING`, `NAMING_CONVENTION` from Patterns to Mirror. Cross-reference `app_id_migration.rs:13-48` and `platform/mod.rs:10-29`.
- **IMPORTS**: `std::io`, `std::path::{Path, PathBuf}`, `std::fmt`.
- **GOTCHA**: `mod.rs` must `pub use types::{FlatpakMigrationError, MigrationOutcome};` — other batches will `use crate::flatpak_migration::FlatpakMigrationError`. If `lib.rs` registration is missed, downstream tasks fail to compile.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` (no errors; placeholder `unimplemented!()` in `run` is fine at this stage). `cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings`.

### Task 1.2: Extract `copy_dir_recursive` into shared `fs_util` — Depends on [none]

- **BATCH**: B1
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-1-2/` (branch: `feat/flatpak-isolation-1-2`)
- **ACTION**: Create `crosshook-core/src/fs_util.rs` with `pub(crate) fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()>`, `fn copy_symlink(src: &Path, dst: &Path) -> std::io::Result<()>`, and `pub(crate) fn dir_is_empty(path: &Path) -> std::io::Result<bool>`. Register `mod fs_util;` in `lib.rs`. Update `app_id_migration.rs` to `use crate::fs_util::{copy_dir_recursive, dir_is_empty};` and remove the local copies. Preserve the existing symlink-handling behaviour verbatim (Unix path uses `std::os::unix::fs::symlink`).
- **IMPLEMENT**: Copy the bodies from `app_id_migration.rs:60-102` as-is into `fs_util.rs`. Add `#[cfg(test)] mod tests` inside `fs_util.rs` with at least: copies empty dir, copies nested files, copies symlinks verbatim, handles unicode names.
- **MIRROR**: `RECURSIVE_COPY` from Patterns to Mirror.
- **IMPORTS**: `std::fs`, `std::io`, `std::path::Path`. On Unix: `std::os::unix::fs::symlink`.
- **GOTCHA**: The existing `app_id_migration.rs` tests at lines 278-471 use the old private helper — they should continue to pass once the import is updated. Do NOT delete those tests. Use `pub(crate)` visibility on helpers so they are reachable from both `app_id_migration.rs` and `flatpak_migration::copier`.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core app_id_migration` passes (all 5 pre-existing test cases green). `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core fs_util` passes new tests.

### Task 2.1: First-run detector — Depends on [1.1]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-2-1/` (branch: `feat/flatpak-isolation-2-1`)
- **ACTION**: Implement `flatpak_migration/detector.rs` — `pub(crate) fn needs_first_run(sandbox_config_root: &Path, host_config_root: &Path) -> Result<bool, FlatpakMigrationError>`. Returns `Ok(true)` only when host config root exists and is non-empty AND sandbox config root either does not exist or is empty. Add `pub(crate) fn host_config_dir(home: &Path) -> PathBuf` → `home.join(".config").join("crosshook")` and `pub(crate) fn host_data_dir(home: &Path) -> PathBuf` → `home.join(".local/share").join("crosshook")`.
- **IMPLEMENT**: Use `crate::fs_util::dir_is_empty` (already extracted in 1.2 — B2 depends on B1). Return `FlatpakMigrationError::Io { path, source }` on metadata failures that are not `NotFound`. `NotFound` → treat as absent. Include `#[cfg(test)] mod tests` covering: both dirs exist non-empty → false; host exists + sandbox empty → true; host missing → false; host present + sandbox present non-empty → false.
- **MIRROR**: `IDEMPOTENCY`, `ERROR_HANDLING`.
- **IMPORTS**: `std::path::{Path, PathBuf}`, `std::fs`, `crate::flatpak_migration::FlatpakMigrationError`, `crate::fs_util::dir_is_empty`.
- **GOTCHA**: Host path derivation here is intentionally `home.join(".config")` / `home.join(".local/share")` — NOT via `BaseDirs`, because `BaseDirs` in the sandbox resolves to `~/.var/app/<id>/config/` etc. The detector only runs when `HOME` points at the real user home (Flatpak behaviour).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core flatpak_migration::detector` passes all 4 cases.

### Task 2.2: Selective copier with staged rename — Depends on [1.1]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-2-2/` (branch: `feat/flatpak-isolation-2-2`)
- **ACTION**: Implement `flatpak_migration/copier.rs` — `pub(crate) fn copy_tree_or_rollback(src: &Path, dst: &Path) -> Result<(), FlatpakMigrationError>` (staged rename, mirroring `app_id_migration::migrate_one_app_id_root`) and `pub(crate) fn copy_data_subtrees(host_data_root: &Path, sandbox_data_root: &Path) -> (Vec<&'static str>, Vec<&'static str>, Vec<FlatpakMigrationError>)` returning (imported, skipped-but-existed, errors). Copy each `DATA_INCLUDE_SUBTREES` entry and each `DATA_INCLUDE_FILES` entry; record but NEVER copy items in `DATA_SKIP_SUBTREES`.
- **IMPLEMENT**: For directories: `copy_tree_or_rollback` copies `src → <dst>.migrating` via `fs_util::copy_dir_recursive`, renames into place, cleans up on failure. For files (metadata DB trio): simple `fs::copy` with parent-dir creation. Idempotency: if target already exists non-empty, skip that entry (don't error the whole migration — log `tracing::debug!`). Continue-on-error failure policy (like `app_id_migration` line 171-192): one subtree failing must not stop others.
- **MIRROR**: `STAGED_ATOMIC_RENAME`, `RECURSIVE_COPY`, `LOGGING`.
- **IMPORTS**: `std::fs`, `std::path::{Path, PathBuf}`, `tracing`, `crate::fs_util::{copy_dir_recursive, dir_is_empty}`, `crate::flatpak_migration::types::{FlatpakMigrationError, DATA_INCLUDE_SUBTREES, DATA_INCLUDE_FILES, DATA_SKIP_SUBTREES}`.
- **GOTCHA**: The metadata DB is SQLite with WAL — the three files (`.db`, `.db-wal`, `.db-shm`) must be copied together under the same rename transaction OR be copied while no process has the DB open. Since migration runs at startup before `MetadataStore::try_new`, the DB is guaranteed closed. Copy `.db` first, then `-wal` / `-shm` (if present) — never copy `-wal` without its `.db` parent. `.migrating` sibling strategy does not apply to the individual DB files (they live inside the `crosshook/` subtree); stage the enclosing subtree OR accept that the DB files are copied one-by-one and document the atomicity caveat in a module comment.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core flatpak_migration::copier` covers: empty include dirs → no-op; include dirs copied; skipped dirs NEVER materialize at dst; rollback on injected write failure (use `tempfile` + a read-only target subdir); metadata.db trio copied; idempotent second run is a no-op.

### Task 2.3: Host-prefix-root resolver — Depends on [1.1]

- **BATCH**: B2
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-2-3/` (branch: `feat/flatpak-isolation-2-3`)
- **ACTION**: Implement `flatpak_migration/prefix_root.rs` — `pub fn host_prefix_root() -> Option<PathBuf>` that returns the host-side prefix-root path when `platform::is_flatpak() == true` AND the isolation mode is active (env var `CROSSHOOK_FLATPAK_HOST_XDG` is NOT set). Returns `None` otherwise. The path: `<HOME>/.local/share/crosshook/prefixes`. Add a sibling `pub fn is_isolation_mode_active(env: &dyn EnvSink) -> bool` helper that consolidates the env-var check (`CROSSHOOK_FLATPAK_HOST_XDG` unset → true) so it can be reused by Task 4.1 and Task 4.2.
- **IMPLEMENT**: Reuse the `EnvSink` trait from `platform::xdg` (promote from `pub(crate)` to `pub(crate) use super::…` export if needed; do NOT change its `EnvSink` contract). `host_prefix_root` reads `HOME` via `EnvSink::get("HOME")`. Include test-only variant `pub(crate) fn host_prefix_root_with(env: &dyn EnvSink, is_flatpak: bool) -> Option<PathBuf>` for deterministic tests.
- **MIRROR**: `ENV_SINK`.
- **IMPORTS**: `std::path::PathBuf`, `crate::platform::is_flatpak`, and `crate::platform::xdg::EnvSink` (if accessible; otherwise define a local `pub(crate) trait EnvSink` — DO NOT duplicate; prefer re-exporting).
- **GOTCHA**: `platform::xdg::EnvSink` is currently `pub(crate)` to the `platform` module. Two options: (a) move `EnvSink` to a new `platform/env.rs` with `pub(crate)` visibility at `platform` crate scope and re-export; (b) keep it local and add a sibling trait in `flatpak_migration`. Prefer (a) — single source of truth. Document this in the module doc comment of the relocated trait.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core flatpak_migration::prefix_root` covers: Flatpak + no env var → returns `Some(<home>/.local/share/crosshook/prefixes)`; Flatpak + env var set → `None`; not Flatpak → `None`; no HOME → `None`.

### Task 3.1: `flatpak_migration::run()` orchestrator + public API — Depends on [2.1, 2.2]

- **BATCH**: B3
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-3-1/` (branch: `feat/flatpak-isolation-3-1`)
- **ACTION**: Replace the placeholder `run` in `flatpak_migration/mod.rs` with the real implementation. Public surface: `pub fn run() -> Result<MigrationOutcome, FlatpakMigrationError>` (calls `BaseDirs::new()` + derives `HOME`); `#[cfg(test)] pub(crate) fn run_for_roots(host_home: &Path, sandbox_config_root: &Path, sandbox_data_root: &Path) -> Result<MigrationOutcome, Vec<FlatpakMigrationError>>` (test seam mirroring `app_id_migration::migrate_legacy_tauri_app_id_xdg_directories_for_roots`).
- **IMPLEMENT**: Flow — (1) resolve host config/data dirs (via `detector::host_config_dir`/`host_data_dir`) and sandbox equivalents (via `BaseDirs::new()`); (2) if `!is_flatpak() { return Ok(MigrationOutcome::default()); }`; (3) call `detector::needs_first_run(sandbox_config, host_config)`; (4) if true: `copier::copy_tree_or_rollback(host_config, sandbox_config)?`; set `outcome.imported_config = true`; (5) call `copier::copy_data_subtrees(host_data, sandbox_data)`; populate `outcome.imported_subtrees` / `outcome.skipped_subtrees`; (6) return outcome; (7) continue-on-error: accumulate errors in a `Vec`, log each with `tracing::warn!` + `eprintln!` (see `LOGGING` pattern), return `Ok(outcome)` if any subtree succeeded, `Err(first_fatal_error)` only when config copy itself failed.
- **MIRROR**: `STAGED_ATOMIC_RENAME`, `LOGGING`, `IDEMPOTENCY`, `TEST_STRUCTURE`.
- **IMPORTS**: `directories::BaseDirs`, `std::path::PathBuf`, `tracing`, `crate::platform::is_flatpak`, submodules `detector`, `copier`, `types`.
- **GOTCHA**: `BaseDirs::new()` reads XDG env vars at construction — by the time `run()` is called (Task 4.1 wires it in BEFORE `override_xdg_for_flatpak_host_access`), env vars are Flatpak defaults. This is desired: sandbox paths resolve via `BaseDirs`, host paths resolve via `HOME`. Do NOT add `override_xdg_for_flatpak_host_access()` call here — that decision lives in `lib.rs` startup. `run_for_roots` MUST accept all roots as explicit args so tests never depend on env/`BaseDirs`.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core flatpak_migration` passes. Scenarios: fresh host + empty sandbox → full import; host + populated sandbox → no-op; partial failure in one subtree → outcome reports it + others imported; not-flatpak early return.

### Task 4.1: Wire migration into `src-tauri/src/lib.rs` startup + emit event — Depends on [3.1]

- **BATCH**: B4
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-4-1/` (branch: `feat/flatpak-isolation-4-1`)
- **ACTION**: Replace line 39 of `src-tauri/src/lib.rs` (`unsafe { override_xdg_for_flatpak_host_access() };`) with a gated decision: read `CROSSHOOK_FLATPAK_HOST_XDG`; if `Some("1" | "true")` → call the Phase 1 override (opt-in); else → call `crosshook_core::flatpak_migration::run()` and stash the `MigrationOutcome` in a startup-scoped local for later event emission. Then inside the Tauri `setup` closure (lines 152-250), after `app_handle` is available, `emit("flatpak-migration-complete", &payload)` if `outcome.imported_config || !outcome.imported_subtrees.is_empty()`.
- **IMPLEMENT**: Use `std::env::var_os("CROSSHOOK_FLATPAK_HOST_XDG").as_deref().map(|s| s == OsStr::new("1") || s == OsStr::new("true")).unwrap_or(false)`. Payload: `#[derive(serde::Serialize)] struct FlatpakMigrationCompletePayload { imported_config: bool, imported_subtrees: Vec<String>, skipped_subtrees: Vec<String> }`. Wrap emit in `if let Err(error) = app_handle.emit(...) { tracing::warn!(%error, "failed to emit flatpak-migration-complete"); }`. Log the startup decision with `tracing::info!(mode = %(...), "flatpak startup mode")` (+ `eprintln!` fallback since `logging::init_logging` runs later).
- **MIRROR**: `TAURI_EVENT_EMIT`, `STARTUP_ORDERING`, `LOGGING`.
- **IMPORTS**: `crosshook_core::flatpak_migration`, `crosshook_core::platform::{is_flatpak, override_xdg_for_flatpak_host_access}`, `tauri::Emitter`.
- **GOTCHA**: The migration MUST run before line 42 (`migrate_legacy_tauri_app_id_xdg_directories`) and before every `*Store::try_new` (lines 106-130) because `directories::BaseDirs` reads env vars at construction time. See the comment at lib.rs:29-33 — preserve it and extend it to document the new gated decision. Also: the `unsafe` block around `override_xdg_for_flatpak_host_access` must be kept when taking that branch. Stashing the `MigrationOutcome` between startup top-level and the `setup` closure requires a `std::sync::Mutex<Option<MigrationOutcome>>` or moving the outcome into the closure via capture — pick the simpler one (mutex in a `OnceLock` matches repo convention per `Patterns` discovery).
- **VALIDATE**: `cargo build --manifest-path src/crosshook-native/Cargo.toml` green. Manual: (a) launch outside Flatpak (`FLATPAK_ID` unset) → migration early-returns, no event emitted; (b) launch with `CROSSHOOK_TEST_FLATPAK_ID=1 CROSSHOOK_FLATPAK_HOST_XDG=1` → Phase 1 override path taken, XDG vars remapped to host; (c) launch with `CROSSHOOK_TEST_FLATPAK_ID=1` only → `flatpak_migration::run` invoked. Use `tracing_test` or log-capture asserts if a unit test is feasible; otherwise rely on the integration test in Task 5.1.

### Task 4.2: Prefix-root override in `install/service.rs` + ad-hoc path — Depends on [2.3]

- **BATCH**: B4
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-4-2/` (branch: `feat/flatpak-isolation-4-2`)
- **ACTION**: Update `install/service.rs::resolve_prefix_root()` (line 163-172) to consult `flatpak_migration::host_prefix_root()` first; if `Some(p)`, return `Ok(p)`. Apply the same override in `run_executable/service/adhoc_prefix.rs` wherever it derives the prefix root from `BaseDirs::data_local_dir()`.
- **IMPLEMENT**: `fn resolve_prefix_root() -> Result<PathBuf, InstallGameError> { if let Some(host) = crate::flatpak_migration::host_prefix_root() { return Ok(host); } let base_dirs = BaseDirs::new().ok_or(InstallGameError::HomeDirectoryUnavailable)?; Ok(resolve_default_prefix_path_from_data_local_dir(base_dirs.data_local_dir())) }`. Same wrap in `run_executable/service/adhoc_prefix.rs` around the equivalent call. Add unit test in each affected file that injects `host_prefix_root` results via an internal seam (prefer wrapping the helper in a `pub(crate) fn resolve_prefix_root_with(host_override: Option<PathBuf>)` signature).
- **MIRROR**: `ERROR_HANDLING`.
- **IMPORTS**: `crate::flatpak_migration::host_prefix_root` (from `crosshook_core` crate root).
- **GOTCHA**: Do NOT rewrite already-persisted absolute `runtime.prefix_path` values. Existing Phase 1 Flatpak profiles already point at host paths and stay valid. The override only affects NEW prefix-root derivations (new profile default + ad-hoc runs). Also: `DEFAULT_PREFIX_ROOT_SEGMENT = "crosshook/prefixes"` must match between the override path and the default — keep the segment string centralized in one constant.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core install::service::tests resolve_prefix_root` + matching test in `run_executable::service::adhoc_prefix`. Scenarios: not-flatpak → returns `BaseDirs`-derived path unchanged; Flatpak + isolation mode → returns host path; Flatpak + env-var opt-in → returns `BaseDirs`-derived path (which under opt-in equals host path via XDG override).

### Task 5.1: End-to-end integration test — Depends on [3.1, 4.2]

- **BATCH**: B5
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-5-1/` (branch: `feat/flatpak-isolation-5-1`)
- **ACTION**: Create `crates/crosshook-core/tests/flatpak_migration_integration.rs`. Set up a synthetic host home (tempdir) populated with realistic fixtures: `~/.config/crosshook/settings.toml` (valid minimal TOML), `~/.config/crosshook/profiles/example.toml`, `~/.local/share/crosshook/metadata.db` (open sqlite via `rusqlite` and write a pragma), `~/.local/share/crosshook/community/taps/example/README.md`, `~/.local/share/crosshook/media/cover.png` (empty bytes), `~/.local/share/crosshook/prefixes/example/drive_c/placeholder` (must NOT be copied), `~/.local/share/crosshook/artifacts/logfile.log` (must NOT be copied). Set up a separate synthetic sandbox root (tempdir, empty). Call `flatpak_migration::run_for_roots(host, sandbox_config, sandbox_data)` and assert: config tree copied verbatim; metadata.db copied; community/media/launchers copied; prefixes/artifacts/cache/logs/runtime-helpers NOT copied; running twice is a no-op (second call returns `imported_config == false`).
- **IMPLEMENT**: Use `tempfile::TempDir` for both roots. Helper `fn populate_host_fixture(root: &Path)` builds the tree. Assert using `assert!(sandbox_data.join("crosshook/metadata.db").exists())` + `assert!(!sandbox_data.join("crosshook/prefixes").exists())` etc. Also test: rollback on failure by making a sandbox subdir read-only and confirming the failed subtree does NOT leave a `.migrating` sibling behind. Test prefix-root resolver: `host_prefix_root_with(&FakeEnv{HOME=host}, is_flatpak=true)` returns `host/.local/share/crosshook/prefixes`.
- **MIRROR**: `TEST_STRUCTURE`, `TEST_MATRIX`.
- **IMPORTS**: `tempfile::TempDir`, `rusqlite::Connection`, `crosshook_core::flatpak_migration`, `crosshook_core::fs_util`, `std::fs`.
- **GOTCHA**: Integration tests run with the real process env — do NOT touch `HOME`, `XDG_*`, or `FLATPAK_ID` process-wide. Use `run_for_roots` (test seam) + `host_prefix_root_with(FakeEnv, …)` to keep tests hermetic. If you must touch env vars, use `platform::tests::common::ScopedEnv` (re-export if needed) with its `FLATPAK_ID_LOCK` mutex — otherwise parallel test runs race.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test flatpak_migration_integration`. All assertions green. Re-run with `--release` to catch any debug-only assumptions.

### Task 5.2: Frontend toast handler + sessionStorage dedup — Depends on [4.1]

- **BATCH**: B5
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-5-2/` (branch: `feat/flatpak-isolation-5-2`)
- **ACTION**: Create `src/crosshook-native/src/hooks/useFlatpakMigrationToast.ts` that subscribes to the `flatpak-migration-complete` Tauri event and dispatches a one-time toast. Wire the hook into the root shell component (likely `src/App.tsx` or the nearest layout component). Dedup with `sessionStorage` key `FLATPAK_MIGRATION_TOAST_SHOWN`.
- **IMPLEMENT**: Follow `useProfilesPageNotifications.ts:32-46` pattern: `useState(() => sessionStorage.getItem(KEY) === '1')`; `useEffect` subscribes via `listen('flatpak-migration-complete', event => { ... })` from `@tauri-apps/api/event`; on fire, sets sessionStorage, calls `toast.info(...)` with the imported-subtrees list. Toast text: `"Imported your existing CrossHook data ({N} items). Your settings and game library are ready."`. Add Vitest covering: unsubscribe on unmount; dedup prevents double-fire; imported_config=false + empty subtrees path does not fire.
- **MIRROR**: `FRONTEND_TOAST_DEDUP`.
- **IMPORTS**: `@tauri-apps/api/event`, existing toast primitive (audit via grep — likely `sonner` or custom `useToast`). The existing `RENAME_TOAST_DISMISSED_SESSION_KEY` constant is the template for the new key name.
- **GOTCHA**: `listen()` returns an `UnlistenFn` — must be awaited and called from the `useEffect` cleanup; otherwise the listener leaks across HMR reloads. Also: `useFlatpakMigrationToast` must not run during `--browser` dev mode (the backend doesn't emit there) — check `AGENTS.md` § "Browser Dev Mode" for the mock-layer sentinel; either gate on the mock flag or ensure the hook is inert when no event ever arrives. BEM-like class name required if the toast uses custom styling: `crosshook-toast--flatpak-migration`.
- **VALIDATE**: `npm test -- useFlatpakMigrationToast` passes new Vitest cases. `npm run typecheck` green. Manual smoke: `./scripts/dev-native.sh --browser` — hook should compile without error even though the event never arrives.

### Task 5.3: Documentation + ADR + PRD update + packaging note — Depends on [4.1, 4.2]

- **BATCH**: B5
- **Worktree**: `~/.claude-worktrees/crosshook-flatpak-isolation-5-3/` (branch: `feat/flatpak-isolation-5-3`)
- **ACTION**: Create `docs/architecture/adr-0003-flatpak-per-app-isolation.md`; update `docs/prps/prds/flatpak-distribution.prd.md` §10.3 status to "in-progress — see `docs/prps/plans/flatpak-isolation.plan.md`"; update `packaging/flatpak/README.md` with a "Shared mode (advanced users)" section documenting the `flatpak override --user --env=CROSSHOOK_FLATPAK_HOST_XDG=1 dev.crosshook.CrossHook` incantation; update `AGENTS.md` (SQLite Metadata DB / data classification section) to note the Flatpak isolation default and the host-prefix-root override contract.
- **IMPLEMENT**: ADR structure: Context (Phase 1 override trade-off, Flathub precedent), Decision (per-app isolation default + env-var opt-in + host-prefix-root override), Consequences (positive: Flathub eligibility, user data preserved on upgrade; negative: one-way migration means host edits don't sync). Include the include/skip manifest verbatim. Reference `adr-0001` for scope relationship ("host-gateway rules are orthogonal; this ADR covers in-sandbox storage layout"). Commit with `docs(internal): …` per CLAUDE.md for the `docs/prps/...` edits; the ADR and `packaging/flatpak/README.md` + `AGENTS.md` updates are user-facing → use `docs(flatpak): …` subject.
- **MIRROR**: Existing ADR format at `docs/architecture/adr-0001-platform-host-gateway.md`, `docs/architecture/adr-0002-flatpak-portal-contracts.md` — identical front matter (Status, Date, Context, Decision, Consequences).
- **IMPORTS**: n/a (markdown).
- **GOTCHA**: `CLAUDE.md` commit-scope rule: `docs/prps/**` and `docs/plans/**` MUST use `docs(internal): …`. The ADR itself (`docs/architecture/...`) is user-facing documentation → `docs(flatpak): …` or `docs(architecture): …`. Split the commits if the PR bundles both; otherwise the release-prep workflow warning fires.
- **VALIDATE**: `test -f docs/architecture/adr-0003-flatpak-per-app-isolation.md`. Markdown lint via `./scripts/lint.sh` green. Cross-reference check: `grep -n "flatpak-isolation.plan.md" docs/prps/prds/flatpak-distribution.prd.md` returns the new reference. Verify PRD §10.3 status line was updated (not left pending).

---

## Testing Strategy

### Unit Tests

| Test                                                       | Input                                                               | Expected Output                                                       | Edge Case?  |
| ---------------------------------------------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------- | ----------- |
| `fs_util::copies_nested_files`                             | tempdir with `a/b/c.txt`                                            | same tree at dst                                                      | —           |
| `fs_util::preserves_symlinks`                              | tempdir with symlink → sibling                                      | symlink copied as symlink, not dereferenced                           | yes         |
| `fs_util::dir_is_empty_true_for_nonexistent`               | Path that does not exist                                            | `Err(NotFound)` propagated (not `Ok(true)`)                           | yes         |
| `detector::host_present_sandbox_empty`                     | host dir populated, sandbox empty                                   | `Ok(true)`                                                            | golden path |
| `detector::host_missing`                                   | host missing, sandbox empty                                         | `Ok(false)`                                                           | yes         |
| `detector::both_populated`                                 | host + sandbox both populated                                       | `Ok(false)` (idempotent re-launch)                                    | yes         |
| `copier::include_list_copied_and_skip_list_not`            | host data with include + skip subtrees                              | include subtrees at dst; skip subtrees absent                         | golden path |
| `copier::rollback_removes_migrating_sibling_on_failure`    | read-only dst forces rename failure                                 | `<dst>.migrating` cleaned up, dst untouched                           | yes         |
| `copier::metadata_db_trio_copied_together`                 | host has `.db`, `.db-wal`, `.db-shm`                                | all three at dst                                                      | yes         |
| `copier::second_run_is_noop`                               | run once, then re-run with same args                                | second outcome has empty `imported_subtrees`                          | yes         |
| `prefix_root::flatpak_and_no_env_var_returns_host_path`    | `FakeEnv{HOME=/h}`, `is_flatpak=true`                               | `Some(PathBuf::from("/h/.local/share/crosshook/prefixes"))`           | golden path |
| `prefix_root::env_var_set_returns_none`                    | `FakeEnv{HOME=/h, CROSSHOOK_FLATPAK_HOST_XDG=1}`, `is_flatpak=true` | `None`                                                                | yes         |
| `prefix_root::not_flatpak_returns_none`                    | `is_flatpak=false`                                                  | `None`                                                                | yes         |
| `run::early_returns_when_not_flatpak`                      | `is_flatpak=false` (mocked)                                         | `Ok(MigrationOutcome::default())`, no I/O                             | golden path |
| `run::partial_subtree_failure_does_not_abort`              | one subtree unwritable, others OK                                   | `Ok(outcome)` with error in `outcome.errors`, other subtrees imported | yes         |
| `install::service::resolve_prefix_root_with_host_override` | `host_prefix_root() = Some(/h/...)`                                 | returns `/h/.local/share/crosshook/prefixes`                          | golden path |
| `adhoc_prefix::resolve_uses_host_override`                 | same                                                                | same                                                                  | golden path |
| `useFlatpakMigrationToast::dedup_via_sessionStorage`       | event fires twice in same session                                   | toast shown once                                                      | yes         |

### Edge Cases Checklist

- [ ] Empty host tree (fresh Flatpak install) → no migration, no toast.
- [ ] Host tree with unreadable files (permission denied) → log + continue; other subtrees still imported.
- [ ] Sandbox tree with partial data (e.g., only `metadata.db`) → skip the populated subtree; copy the missing ones.
- [ ] Sandbox disk full → rollback cleanup; no `.migrating` litter.
- [ ] Host metadata.db open by another CrossHook process (concurrent AppImage launch) → SQLite WAL copy is NOT safe during an open write transaction. Document as known-race caveat; fail gracefully if copy produces an inconsistent trio (integration test should assert we don't silently half-copy).
- [ ] `HOME` unset in Flatpak sandbox (unlikely, but Flatpak's runtime guarantees it) → `run()` returns `Err(HomeDirectoryUnavailable)`; startup logs + continues with sandbox-only empty state.
- [ ] `CROSSHOOK_FLATPAK_HOST_XDG=0` / `="false"` / `=""` → treated as unset (isolation default).
- [ ] Running outside Flatpak (dev mode, AppImage) → migration is a no-op; override function is never called.
- [ ] Symlinks in host config → copied as symlinks (verified by `fs_util` tests); do not dereference.
- [ ] Unicode paths → recursive copy handles them (existing `app_id_migration` tests cover this).
- [ ] Re-run after user manually cleared sandbox tree → migration re-imports (filesystem-state-driven idempotency).
- [ ] Launching an AppImage after a Flatpak migration → AppImage still reads host tree (untouched by one-way migration); no regressions on AppImage-only users.

---

## Validation Commands

### Static Analysis

```bash
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native-app -- -D warnings
./scripts/check-host-gateway.sh
./scripts/lint.sh
./scripts/format.sh
npm run typecheck
```

EXPECT: Zero warnings/errors. `check-host-gateway.sh` is unaffected by this plan (no new host-tool invocations), but must stay green.

### Unit Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core fs_util
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core flatpak_migration
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core app_id_migration
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core install::service
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core run_executable::service::adhoc_prefix
npm test -- useFlatpakMigrationToast
```

EXPECT: All new tests pass; existing `app_id_migration` and `platform::xdg` tests stay green.

### Integration Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --test flatpak_migration_integration
```

EXPECT: Full fixture scenario passes; rollback + idempotency + skip-list assertions all green.

### Full Test Suite

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
npm test
```

EXPECT: No regressions in existing suites.

### Build

```bash
./scripts/build-native.sh --binary-only
./scripts/build-native-container.sh
```

EXPECT: Both binary and Flatpak bundle build cleanly. (`build-native-container.sh` exercises the Flatpak manifest; a manifest mismatch around `finish-args` would surface here.)

### Manual Validation

- [ ] **Not-Flatpak**: launch the AppImage build on a dev host; confirm no migration log lines, startup time unchanged.
- [ ] **Flatpak — fresh install, no host data**: `flatpak install` + launch in a user account that has never run the AppImage; confirm sandbox `~/.var/app/dev.crosshook.CrossHook/{config,data}/` starts empty and the app shows the onboarding flow, no migration toast.
- [ ] **Flatpak — migration from AppImage**: on a machine with existing `~/.config/crosshook/` + `~/.local/share/crosshook/` data, install + launch the Flatpak; confirm (1) toast appears, (2) `~/.var/app/dev.crosshook.CrossHook/config/crosshook/` mirrors host config, (3) `~/.var/app/dev.crosshook.CrossHook/data/crosshook/` has metadata.db + community + media + launchers, (4) `prefixes/` / `artifacts/` / `cache/` / `logs/` absent from sandbox, (5) existing profiles still launch (prefix paths point to host), (6) second launch → no toast (sessionStorage dedup AND filesystem idempotency).
- [ ] **Flatpak — opt-in shared mode**: `flatpak override --user --env=CROSSHOOK_FLATPAK_HOST_XDG=1 dev.crosshook.CrossHook`; relaunch; confirm no migration runs, XDG remap active, sandbox paths mirror host paths.
- [ ] **Flatpak — rollback from opt-in to isolation**: `flatpak override --user --unset-env=CROSSHOOK_FLATPAK_HOST_XDG dev.crosshook.CrossHook`; relaunch; confirm migration detects empty sandbox and re-imports (or no-op if sandbox tree was seeded by prior shared-mode session — document expected state in ADR).
- [ ] **Flathub manifest audit**: run `appstreamcli validate packaging/flatpak/dev.crosshook.CrossHook.metainfo.xml`; no new warnings introduced.
- [ ] **Flatpak Steam-library launch across drives**: with games on `/mnt/nvme1/SteamLibrary`, launch via Flatpak + isolation mode; confirm host prefix resolves, trainer injection works end-to-end.

---

## Acceptance Criteria

- [ ] Task 1.1–5.3 all completed and merged to `feat/flatpak-isolation`.
- [ ] All validation commands pass.
- [ ] Unit + integration tests cover every item in the Testing Strategy matrix.
- [ ] No clippy warnings; no new deps in `crosshook-core/Cargo.toml`.
- [ ] `./scripts/check-host-gateway.sh` green (contract unchanged).
- [ ] `override_xdg_for_flatpak_host_access()` only runs when `CROSSHOOK_FLATPAK_HOST_XDG=1` (or `=true`) is set inside a Flatpak sandbox.
- [ ] `flatpak_migration::run()` is idempotent (second run → `imported_config == false` and empty `imported_subtrees`).
- [ ] Wine prefixes resolve to host `$HOME/.local/share/crosshook/prefixes/` under isolation mode.
- [ ] Manual validation matrix (above) executed at least once on a real Flatpak install.
- [ ] ADR-0003 published, PRD §10.3 status updated, packaging README documents the opt-in env-var override.
- [ ] PR title follows Conventional Commits (`feat(flatpak): …` or `refactor(flatpak): …`); linked with `Part of #210`, `Closes #212`.

## Completion Checklist

- [ ] Code follows discovered patterns (ERROR_HANDLING, RECURSIVE_COPY, STAGED_ATOMIC_RENAME, IDEMPOTENCY, LOGGING, ENV_SINK, TAURI_EVENT_EMIT, FRONTEND_TOAST_DEDUP, TEST_STRUCTURE, NAMING_CONVENTION, STARTUP_ORDERING).
- [ ] Error handling uses hand-rolled typed enums (no `anyhow`, no `thiserror`).
- [ ] Logging uses `tracing::{info,warn,debug}!` with structured fields AND pairs startup-time logs with `eprintln!` fallback.
- [ ] Tests follow the `_for_roots` test-seam pattern and `FakeEnv`/`ScopedEnv` helpers.
- [ ] No hardcoded host-path literals outside the `types.rs` manifest and `detector.rs::host_config_dir`/`host_data_dir` helpers.
- [ ] ADR + PRD + AGENTS.md + packaging README updated in the same PR (or a follow-up PR tracked by the same parent issue) with the correct commit-scope prefix.
- [ ] No unnecessary scope additions (stays within the 15-file budget).
- [ ] Self-contained — implementor never needs to ask "what pattern should I use?" or "which subtrees are included?"

---

## Risks

| Risk                                                                                                  | Likelihood | Impact | Mitigation                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------------------------------------------------- | ---------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SQLite WAL trio copied mid-write → corrupt DB in sandbox.                                             | Low        | High   | Migration runs at startup before `MetadataStore::try_new` (no handle open). If a concurrent AppImage has the host DB open, still copy — the `.db` file reflects the last committed checkpoint; `.wal` may be stale but SQLite recovers. Document caveat in ADR; add integration test asserting that the sandbox DB opens cleanly after copy. |
| Rename across filesystems inside Flatpak (tempdir vs. real XDG) fails with `EXDEV`.                   | Medium     | Medium | `app_id_migration::migrate_one_app_id_root` already handles this — fallback is copy + `remove_dir_all`. Reuse that path exactly in `copy_tree_or_rollback`.                                                                                                                                                                                  |
| User manually clears sandbox tree after first run → migration re-imports unexpectedly.                | Low        | Low    | This is by-design (filesystem-state idempotency). Document in ADR and the toast copy so users aren't surprised by a second import.                                                                                                                                                                                                           |
| `CROSSHOOK_FLATPAK_HOST_XDG` env var set but user no longer has host data (fresh account).            | Low        | Low    | Opt-in path calls `apply_xdg_host_override`, which resolves to empty host dirs if user has none. Settings store creates the config on first save — no data loss.                                                                                                                                                                             |
| Prefix-root override miscomputes for non-standard `$HOME` layouts (e.g. `/data/home/<user>`).         | Low        | Medium | `host_prefix_root` reads `HOME` directly (not `BaseDirs`) — whatever Flatpak sets `HOME` to is what the user considers home. Same mechanism the Phase 1 override uses.                                                                                                                                                                       |
| Flathub reviewer still pushes back on `--filesystem=home`.                                            | Medium     | Medium | Out of scope for this plan (manifest permissions are separate). Note: Lutris precedent makes `--filesystem=home` an accepted pattern. Track any reviewer pushback as a follow-up PR with reduced scope (targeted Steam library paths).                                                                                                       |
| Integration test races with other tests that touch `HOME` / `FLATPAK_ID`.                             | Medium     | Low    | Use `ScopedEnv` + `FLATPAK_ID_LOCK` (`platform/tests/common.rs:11-62`) or avoid env-var tests entirely in favour of `_for_roots` seam + `FakeEnv`.                                                                                                                                                                                           |
| `override_xdg_for_flatpak_host_access` marked `unsafe` — gating change must preserve safety contract. | Low        | High   | Keep the `unsafe` block exactly where it is in `lib.rs` (single-threaded startup, before Tauri builder). Do NOT move the call into a helper that could be invoked later.                                                                                                                                                                     |
| Frontend toast hook leaks listener on HMR reload.                                                     | Medium     | Low    | `useEffect` must return `() => unlisten()`. Vitest asserts unsubscribe is called on unmount.                                                                                                                                                                                                                                                 |

## Notes

### Open questions resolved by this plan (from issue #212)

1. **Automatic vs user-prompted migration**: **Automatic**. Friendlier for the 99% case; the toast makes the data path change visible without blocking the user on a modal.
2. **Settings field vs env-var opt-in**: **Env-var only**. Avoids startup chicken-and-egg (reading a setting requires opening the settings file, which requires knowing the XDG root). Also avoids Flathub reviewer friction. A settings-side UI affordance that writes a `flatpak override --user --env=…` via `flatpak-spawn --host` is a future enhancement, out of scope here.
3. **Prefix root override interaction with community-tap relative paths**: Community taps are cloned into `$XDG_DATA_HOME/crosshook/community/` (now sandbox). Taps that reference prefix-relative paths (`$PREFIX/...`) use the profile's absolute `runtime.prefix_path`, which points to the host tree. No conflict.

### Decisions carried from PRD §10.3

- One-way migration only (no continuous sync).
- Keep `override_xdg_for_flatpak_host_access()` alive as an opt-in escape hatch.
- Prefix root stays on host to avoid multi-GB copies.
- Fail loudly on migration errors (do not silently continue and mask data issues).

### Out-of-scope but adjacent work to track separately

- Flathub submission PR (#206) — requires this plan's completion + a clean Flathub manifest + `appstreamcli` green.
- Surfacing migration state in the UI beyond the first-run toast (e.g. "Advanced → Storage → Reset sandbox data") — future feature, file a new issue if user demand appears.
- Bundled Proton versions inside the Flatpak — explicit non-goal per PRD §3.3.

### Persistence classification summary (per CLAUDE.md "Persistence planning")

- **TOML settings**: no changes. The Phase 1 opt-in toggle is env-var-only in this plan (settings field explicitly deferred per Open Question #2).
- **SQLite metadata (`metadata.db`)**: existing file is copied verbatim from host to sandbox on first run. No schema change, no new table.
- **Runtime-only memory**: the `MigrationOutcome` struct lives only during startup (stashed in a `OnceLock<Mutex<Option<MigrationOutcome>>>` until the Tauri setup closure emits the event). Not persisted.
- **Filesystem-backed, host-referenced**: wine prefixes stay at `$HOME/.local/share/crosshook/prefixes/` via the prefix-root override.
- **Migration bookkeeping**: filesystem-state-driven (sandbox tree populated = migration done). No sentinel file, no DB row.

### Persistence & usability notes

- **Migration is explicit**: user is notified via a one-time toast; the data path change is visible.
- **Backward compatible**: AppImage users continue to see their data on AppImage; Flatpak gets its own tree with imported copy. No breaking change to existing AppImage profiles.
- **Offline**: both host and sandbox paths are local disk. No network dependency.
- **Degraded behavior**: migration fails loudly (logs + error in `MigrationOutcome.errors`). Does not silently half-copy; `.migrating` sibling is cleaned up on failure. If config copy fails outright, the app starts with empty sandbox state (same as a fresh Flathub install) rather than reverting to host — user sees onboarding flow and can retry after fixing whatever permission/disk issue caused the failure.
- **User visibility/editability**: users can inspect imported data under `~/.var/app/dev.crosshook.CrossHook/{config,data}/crosshook/`. Editing those files directly works (they're just TOML + SQLite). Advanced users can opt into shared mode via the documented `flatpak override` command.
