# Research: prefix-deps Windows-version restore journal, per-prefix lock, tool fallback, Steam App ID (YAN-815 / YAN-1538)

Paths are relative to `src/crosshook-native/`. Abbreviations: `cmd` means `src-tauri/src/commands/prefix_deps.rs`, `runner` means `crates/crosshook-core/src/prefix_deps/runner.rs`, and `store` means `crates/crosshook-core/src/metadata/prefix_version_restore_store.rs`.

## 1. Data format and storage

- **Table**: `prefix_version_restore_journal`, created by migration v26→v27 (`crates/crosshook-core/src/metadata/migrations/v26_v27.rs:9-22`) in `metadata.db` (SQLite with `foreign_keys=ON`, see `metadata/db.rs:240`).

  | column                    | type / constraint                      | meaning                                                                                                                                                                         |
  | ------------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
  | `prefix_path`             | TEXT PK                                | canonical **`pfx` dir**, set by `runner::normalize_prefix_path` (`runner:85-89`: flatpak host-path normalize → `resolve_wine_prefix_path` → must be a dir → `fs::canonicalize`) |
  | `profile_id`              | TEXT FK `profiles` ON DELETE SET NULL  | informational owner. The row survives a profile delete (shared prefix)                                                                                                          |
  | `binary_path`             | TEXT NOT NULL                          | the tool binary that mutated the prefix (flatpak-normalized). Restore runs **this** binary and does not re-detect it                                                            |
  | `tool_type`               | CHECK IN (`winetricks`,`protontricks`) |                                                                                                                                                                                 |
  | `steam_app_id`            | TEXT NULL                              | required (positive decimal) when tool is protontricks                                                                                                                           |
  | `restore_verb`            | TEXT NOT NULL                          | Windows-version verb captured from `system.reg` before mutation (`win10`, `win7`, …)                                                                                            |
  | `state`                   | CHECK IN (`pending`,`failed`)          | **no `succeeded` state**: success means the row is deleted                                                                                                                      |
  | `last_error`              | TEXT NULL                              | sanitized restore error (`failed` only)                                                                                                                                         |
  | `created_at`/`updated_at` | RFC 3339 TEXT                          |                                                                                                                                                                                 |

  Index `idx_prefix_version_restore_profile(profile_id)`.

- **Row type**: `metadata::PrefixVersionRestoreJournalRow` (`metadata/models.rs:545`). All fields are strings.
- **Store fns** (`store`):
  - `upsert_restore` (:6-40) always writes `state='pending', last_error=NULL`. On conflict it replaces the plan and resets to pending.
  - `load_restore` (:42-71).
  - `mark_restore_failed` (:73-96) returns a `Validation` error if `affected != 1`.
  - `delete_restore` (:98-114) also returns a `Validation` error if `affected != 1`.
- **Facade**: `MetadataStore::{upsert,load,mark…failed,delete}_prefix_version_restore` (`metadata/prefix_ops.rs:11-61`). Writes fail closed with `ReadOnlyDisabled` when the store is unavailable. Loads go through `with_sqlite_conn`, which can read a newer schema. **No list/scan function exists**, so startup recovery needs a new read-only `list_prefix_version_restores()` (SELECT only, no schema change).
- **Plan type in core**: `PrefixVersionRestorePlan {binary_path, prefix_path, tool_type, steam_app_id, restore_verb}` (`prefix_deps/models.rs:56-62`).
- **State enum**: `PrefixVersionRestoreState {NotRequired, Pending, Succeeded, Failed}` (`models.rs:47-52`, snake_case serde, also the IPC shape).
- **IPC status**: `PrefixVersionRepairStatus {required, state: Option<Pending|Failed>, last_error}` (`cmd:51-55`).

## 2. Current flow (src-tauri `install_prefix_dependency`, `cmd:304-498`)

1. Load settings, then `detect_binary(settings.protontricks_binary_path)` (`detection.rs:18-94`). Priority: settings override (tool type comes from the file name containing "protontricks"), then `winetricks` on PATH, then `protontricks` on PATH. Uses `host_command_exists`, which is gateway-safe.
2. Steam App ID: `resolve_profile_steam_app_id` (`cmd:193-201`) uses the effective profile `steam.app_id`, falling back to `runtime.steam_app_id`, each through `parse_nonzero_steam_app_id` (`cmd:178-191`). That parser trims, accepts ASCII digits only, parses as `u32` and rejects 0, and returns the canonical form (`" 0012 "` becomes `"12"`). If the profile fails to load, the result is `None` with no error.
3. **Fallback** (`cmd:322-329`, duplicated in `check_prefix_dependencies` at `cmd:240-247`): if the tool is protontricks and there is no valid app id, use `resolve_winetricks_path()` (winetricks on PATH) if it exists. Otherwise return `Err("protontricks requires a valid nonzero Steam App ID…")`.
   - Practical effect: protontricks is only auto-detected when winetricks is **not** on PATH, so the fallback succeeds only when the settings override names protontricks.
   - The `resolve_tool_type` name heuristic (`cmd:203-211`) is effectively dead code, because detection always sets `tool_type` when `found`.
4. `normalize_prefix_path` (canonical `pfx`) (`cmd:331`).
5. **Lock**: `install_state.lock.try_acquire(normalized)` (`cmd:336-340`). `PrefixDepsInstallLock` (`prefix_deps/lock.rs:9-80`) is a **global single slot** (`Mutex<Option<String>>`), not per-prefix: any second install or repair, on _any_ prefix, gets `AlreadyInstalling{prefix_path: <holder>}`. The guard is moved into the waiter task (`cmd:450`), so it covers install, restore, persistence and the `prefix-dep-complete` emit. `is_locked`/`active_prefix` are only used in tests. Repair takes the same lock (`cmd:527-531`). `check_prefix_dependencies` takes no lock (read-only `list-installed`).
6. If a journal row exists, the install is refused with "This prefix requires Windows version repair…" (`cmd:342-351`). **No auto-recovery.**
7. `lookup_profile_id` is required; the install errors if the profile is not indexed (`cmd:353-356`).
8. `runner::prepare_install_packages` (`runner:567-620`) does the following, without spawning:
   - `validate_protontricks_verbs`
   - builds the lifecycle mutation command (`host_command_with_env_and_directory_lifecycle`, `runner:41-50`, `WINEPREFIX` from the env map)
   - adds `[app_id] -q verbs…` and `kill_on_drop(true)`
   - builds a restore plan **unless any verb is a Windows-version verb** (`runner:599-609`, allowlist `is_windows_version_verb` at `runner:209-232`); `restore_verb` comes from `snapshot_prefix_windows_version(system.reg)` (`runner:103-140`, mapping at `runner:147-207`). An unknown version gives a `ValidationError`, so the install aborts before the journal is written.
   - Note: no `process_group(0)`.
9. **Record**: `upsert_prefix_version_restore(pending)` (`cmd:367-386`). On error the command returns `Err` and nothing is spawned.
10. `prepared.spawn()`. A spawn error returns `Err` and **intentionally leaves the pending row** (`cmd:388`).
11. Two tasks stream stdout and stderr line by line to `prefix-dep-log` (sanitized, `runner::sanitize_output_for_ui`) (`cmd:405-444`).
12. The waiter calls `wait_and_restore` (`runner:297-327`): wait for the install, then **always** restore, even if the install failed (`restore_prefix_windows_version_with_timeout`, `runner:338-423`). The restore:
    - checks the verb allowlist
    - uses the lifecycle builder with `[app_id] -q <verb>`, `kill_on_drop`, stdout null and piped stderr
    - has a 90 s timeout (`RESTORE_TIMEOUT_SECS`, `runner:94`) and kills only the direct child on expiry
    - **verifies** by re-snapshotting `system.reg` and checking it equals `restore_verb`.
13. Per-package `upsert_prefix_dep_state` (fail-soft) (`cmd:454-477`).
14. **Decision** `journal_decision` (`cmd:127-140`):
    - `Succeeded` → `delete`
    - `Failed` → `mark_failed(err or "Windows version restoration failed")`
    - `NotRequired|Pending` → no mutation
    - Persistence error: logged; `outcome_after_journal_persistence` (`cmd:163-176`) downgrades `Succeeded` to `Failed` with "…restored, but repair journal cleanup failed: …". The row is then still `pending`, because install-path delete errors are **not** followed by `mark_failed`.
15. Emit `prefix-dep-complete`. `succeeded = install_succeeded && restore ∈ {NotRequired, Succeeded}` (`cmd:142-161`). The lock is released when the task ends.

**Repair** (`repair_prefix_windows_version`, `cmd:518-593`, manual "Repair Prefix" button):

- Normalize, take the lock, then reload the row under the lock; a missing row is an idempotent `required:false`.
- `repair_status_from_row` and `restore_plan_from_row` (`cmd:98-125`): parse the tool, then `validate_protontricks_verbs([verb])`; protontricks needs a valid app id. An invalid row returns `Err` and **the row is not touched**.
- `restore_prefix_windows_version`, then branch on the outcome:
  - `Succeeded`: reload. If the row is gone, return Ok. Otherwise delete; if the delete fails, `mark_failed("…cleanup failed…")`.
  - `Failed`: `mark_failed`.
  - `NotRequired|Pending`: `mark_failed("…invalid state")`.
  - Return the re-loaded status.
- `get_prefix_version_repair_status` (`cmd:502-514`) normalizes, loads, and ignores `profile_name`.

**Frontend consumers**:

- `usePrefixDeps.ts:58,136`.
- `PrefixDepsPanel.tsx:94,181-199`: the repair banner blocks Install and Check when `required` or status unknown.
- `useLaunchDepGate.ts:390-416`: launch is blocked when `required`; fail-soft only when the session has no known repair.
- `useLaunchPrefixDependencyGate.ts:54-65`.
- Mocks: `lib/mocks/handlers/system.ts:651-658`.

## 3. Today's state machine

```
                 (no row)  ──record (upsert pending)──▶  PENDING ──spawn install──▶ PENDING(running)
                    ▲                                        │                          │ install exits (any code)
                    │ delete (verified restore)              │ spawn error              ▼
                    │                                        ▼ (row kept)        PENDING(restoring)
                    │                                     PENDING                       │
                    ├──────────── restore Succeeded ◀───────────────────────────────────┤
                    │      + delete ok                                                   │ restore Failed / timeout /
   cleanup failed ◀─┘ (install path: stays PENDING;                                     │ verification mismatch
                       repair path: mark FAILED "cleanup failed")                       ▼
                                                                                  FAILED(last_error)
   FAILED ──repair──▶ restore ──Succeeded+delete──▶ (no row)
   FAILED ──repair──▶ restore ──Failed──▶ FAILED(new error)       PENDING ──repair──▶ same as FAILED
   Any row ──install──▶ refused ("requires repair");  launch gate ──▶ blocked
```

## 4. Crash analysis (app process dies: SIGKILL, power loss)

There is no in-memory persistence: the lock and logs are lost, and only the row survives.

| #   | Crash point                                                           | Prefix state | Row after restart    | Today on restart                              | Notes                                                                                                                                                                                                                                                                                               |
| --- | --------------------------------------------------------------------- | ------------ | -------------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C0  | before upsert (`cmd:369`)                                             | untouched    | none                 | clean                                         | safe                                                                                                                                                                                                                                                                                                |
| C1  | after upsert, before/at spawn                                         | untouched    | pending              | banner + manual Repair (re-applies same verb) | friction only                                                                                                                                                                                                                                                                                       |
| C2  | install running (version may already be changed, e.g. dotnet → winxp) | **changed**  | pending              | manual Repair                                 | **orphan risk**: `kill_on_drop` does not run on SIGKILL. Natively the winetricks child is re-parented and keeps mutating. Under Flatpak `--watch-bus` only fires when `flatpak-spawn` dies, and the orphaned proxy lives on [INFERENCE]. The in-memory lock is gone, so Repair can race the orphan. |
| C3  | restore child running                                                 | unknown      | pending              | manual Repair                                 | idempotent re-run                                                                                                                                                                                                                                                                                   |
| C4  | restore verified, before delete                                       | **restored** | pending              | manual Repair re-runs restore → delete        | AC "crash after restore"                                                                                                                                                                                                                                                                            |
| C5  | restore failed, before `mark_failed`                                  | changed      | pending (error lost) | manual Repair                                 |                                                                                                                                                                                                                                                                                                     |
| C6  | after `mark_failed`                                                   | changed      | failed + last_error  | manual Repair                                 |                                                                                                                                                                                                                                                                                                     |
| C7  | after delete, before emit                                             | restored     | none                 | clean                                         | UI missed completion, irrelevant after restart                                                                                                                                                                                                                                                      |

**Startup recovery today: none.**

- Rows are found lazily: Prefix deps panel mount, launch gate pre-check, or an install refusal.
- Rows for prefixes that are never opened stay forever.
- A newer-schema DB still reads rows but cannot write them, so Repair fails with `ReadOnlyNewerSchema` (`db_tests.rs:53`).

## 5. Invariants to preserve (data integrity)

- **I1 Record-before-mutate**: the `pending` row is durable before `spawn()`. If the upsert fails, nothing is spawned. A spawn failure keeps the row. The restore plan is captured before mutation (`PreparedPrefixDependencyInstall`, `runner:234-264`).
- **I2 Delete only after verified restore**: delete only when the restore exit is 0 **and** the `system.reg` re-snapshot equals `restore_verb`. Never delete on `Failed`, `Pending`, `NotRequired`, a parse error, or a corrupt row.
- **I3 Cleanup failure is never success**: a delete error after a verified restore gives outcome `Failed` (`succeeded=false`), and status stays `required`. Try `mark_failed("…cleanup failed…")`; if that also fails, the row stays `pending`.
- **I4 Mutations require the row**: `mark_failed`/`delete` with `affected != 1` is an error (`store:90,108`). A row vanishing under the lock is treated as clean (repair path `cmd:547-553`).
- **I5 One row per canonical pfx**: every journal call uses `normalize_prefix_path` output. Status queries normalize caller input, so symlink and `..` aliases converge (tests `runner:1149,1166`).
- **I6 Serialization**: journal check → record → install → restore → persist → completion event must all run under one exclusive per-prefix hold. Repair takes the same hold. `check_installed` stays lock-free.
- **I7 Fail-closed gate**: no install and no launch while a row exists or while status is unknown (UI tests at `PrefixDepsPanel.test.tsx:136-143`, `useLaunchDepGate.test.tsx:387-456`).
- **I8 Restore always runs** after install exit, including a failed install. New: also after user cancel (SIGTERM); see §6.
- **I9 Restore argv safety**: the verb must pass `validate_protontricks_verbs` **and** `is_windows_version_verb` before spawn (test `runner:1043`). For protontricks the persisted app id must be a positive decimal (`cmd:106-115`). A row that fails validation never executes and is never deleted.
- **I10 Restore with the recorded tool**: persisted `binary_path`/`tool_type`, not current detection.
- **I11 No storage change**: same table, same `pending|failed` CHECK, no migration. Log tails and registries stay runtime-only.
- **I12** An explicit Windows-version verb in the batch means no plan and no row (user intent).
- **I13** `profile_id` is metadata only. A shared prefix row survives an owner delete (`migrations/tests/v26_v27.rs:76`).
- **I14** Restore output is sanitized (`failed_restore` → `sanitize_stderr`, `runner:425-430`) before it reaches `last_error` or the IPC.

## 6. Proposal: `PrefixVersionJournal` (crosshook-app `services/prefix_deps/journal.rs`)

It owns all journal policy that currently lives in `cmd`. The `MetadataStore` journal fns stay as they are; the only addition is `list_prefix_version_restores()`.

```rust
/// Seam for tests; prod impl wraps MetadataStore (fns above + new list).
pub(crate) trait JournalStore: Send + Sync + 'static {
    fn load(&self, prefix: &str) -> Result<Option<PrefixVersionRestoreJournalRow>, MetadataStoreError>;
    fn list(&self) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError>;
    fn upsert_pending(&self, row: &PrefixVersionRestoreJournalRow) -> Result<(), MetadataStoreError>;
    fn mark_failed(&self, prefix: &str, error: &str) -> Result<(), MetadataStoreError>;
    fn delete(&self, prefix: &str) -> Result<(), MetadataStoreError>;
}
/// Seam for tests; prod = core `runner::restore_prefix_windows_version` (restore + verify).
pub(crate) trait PrefixVersionRestorer: Send + Sync + 'static {
    fn restore(&self, plan: &PrefixVersionRestorePlan) -> BoxFuture<'static, PrefixVersionRestoreOutcome>;
}

/// Validated row (replaces repair_status_from_row/restore_plan_from_row).
pub struct JournalEntry { pub plan: PrefixVersionRestorePlan, pub state: PendingOrFailed,
                          pub last_error: Option<String>, pub profile_id: Option<String> }
impl TryFrom<PrefixVersionRestoreJournalRow> for JournalEntry { /* tool, state, verb allowlist, app id */ }

#[must_use = "resolve the ticket after the install finishes"]
pub struct JournalTicket { prefix: String }      // proof that record() succeeded (I1)

pub enum RecoveryResult {
    Clean,                          // no row (or vanished under the key)
    Restored,                       // verified restore + row deleted
    RestoreFailed { error: String },// row now `failed`
    CleanupFailed { error: String },// verified restore, delete failed; row `failed` (or `pending` if mark also failed)
    Corrupt { error: String },      // row fails validation: untouched, never executed
    SkippedFailed,                  // startup only: `failed` rows wait for explicit Repair
}

impl PrefixVersionJournal {
    pub fn new(store: Arc<dyn JournalStore>, restorer: Arc<dyn PrefixVersionRestorer>) -> Self;
    /// Normalized prefix → IPC status (replaces get_prefix_version_repair_status). Corrupt → Err.
    pub fn status(&self, prefix: &str) -> Result<PrefixVersionRepairStatus, AppError>;
    /// Persist `pending` BEFORE spawn. Err ⇒ caller must not spawn.
    pub fn record(&self, plan: &PrefixVersionRestorePlan, profile_id: &str) -> Result<JournalTicket, AppError>;
    /// Apply the delete/mark decision for an install's restore outcome; returns the
    /// outcome to report (Succeeded downgraded to Failed on cleanup error, I3).
    pub fn resolve(&self, ticket: JournalTicket, restore: PrefixVersionRestoreOutcome) -> PrefixVersionRestoreOutcome;
    /// Restore + verify + resolve one prefix. CALLER HOLDS the prefix JobKey.
    pub async fn recover(&self, prefix: &str, include_failed: bool) -> RecoveryResult;
    /// Called inside the install job (key held) before prepare/record:
    /// Clean|Restored → Ok; otherwise Err(Conflict "prefix requires Windows version repair") + status.
    pub async fn recover_before_install(&self, prefix: &str) -> Result<(), AppError>;
    /// Startup: list rows; for each `pending` row start a keyed job
    /// (JobSpec::new("prefix-version-recovery").with_key(prefix_key(p))) running recover(p,false);
    /// Conflict ⇒ skip (the holder will recover_before_install). Store disabled ⇒ no-op + warn.
    pub async fn recover_on_startup(&self, jobs: &JobRegistry) -> Vec<(String, RecoveryResult)>;
}
pub(crate) fn decide(state: PrefixVersionRestoreState) -> JournalDecision; // pure, ported from cmd:127-140
```

Design notes:

- **Explicit user Repair** is `recover(prefix, include_failed = true)` run in a keyed job. That keeps today's semantics, including mark-failed on an invalid `NotRequired|Pending` outcome.
- **Per-prefix key**:
  - Use `JobKey::new(format!("prefix:{normalized_pfx}"))`, shared by install, repair and startup recovery, so each prefix has one exclusivity domain. Today's lock is global; the new key keeps I6 but lets different prefixes run in parallel, which is the intended behavior change.
  - Coordinate with run-executable if it ever targets a non-throwaway prefix.
  - Delete `PrefixDepsInstallLock`, `PrefixDepsInstallState` and `lib.rs:73` (clean cutover).
- **Sync errors vs job failures**: validation that needs no hold (detection, app id, verbs, `normalize_prefix_path`) runs **before** `jobs.start`, as synchronous `Err`. The journal check, record, spawn, restore and resolve run **inside** the keyed job, because the key is acquired atomically in `JobRegistry::start` (`jobs/registry.rs:150-159`).
- **Cancel semantics (I8)**:
  - The install child is `process_group(0)` plus `jobs::supervise_child(ctx.cancel_token(), grace)`: SIGTERM, then SIGKILL. This needs core `prepare_install_packages` to set `process_group(0)` (currently only `kill_on_drop`).
  - The restore must **not** be bound to the job token: a user cancel still restores, bounded by its own 90 s timeout. App shutdown drops the task, `kill_on_drop` fires, and the row stays `pending` for startup recovery.
  - The terminal job `Outcome` is `Cancelled`, with the restore result carried in the completion payload.
- **Startup**: add `schedule_prefix_journal_recovery` next to `schedule_scans` (`bootstrap/scans.rs:38-50`, same `JobSpec`/`finish*` pattern; optionally a new `StartupJobKind`). Recovery needs no network (local winetricks verb), so it works offline.
  - Publish an event on each resolution (e.g. reuse `prefix-dep-complete` or add `prefix-version-repair-changed {prefix_path}`), so a panel or launch gate that is already mounted refetches status. Frontend peer.
  - While recovery runs, status is still `pending`, so launch stays blocked (fail-closed).
- **C2 orphan guard (recommended)**: before running a startup or pre-install restore, check whether a live process carries the prefix in `/proc/*/environ`, reusing the boundary matcher behind `jobs::kill_processes_using_prefix` (`jobs/process.rs:335-399`) **without killing**. If one is found, return `RestoreFailed{"prefix is in use by another process"}` (the row is kept; the user retries).
- **Steam App ID / fallback as a pure fn**: `resolve_tool(detection, profile_app_ids: [&str;2], winetricks_lookup: impl Fn()->Option<String>) -> Result<ResolvedTool{binary, tool, app_id}, AppError>`.
  - Move `parse_nonzero_steam_app_id` to core `prefix_deps::validation` and use it in both `resolve_tool` and `JournalEntry::try_from`.
  - Use it for check and install (removes the duplicate at `cmd:237-247` / `cmd:319-329`).
  - Set `app_id=None` when the tool is winetricks, so rows don't store a stray id.
  - Note the divergence: the profile's `validate_steam_app_id` allows 12 digits (`profile/models/resolve.rs:58`), but `u32` parsing rejects values above 4294967295, which then fall back to winetricks. Keep `u32` (it matches Steam IDs) and document it.
- **YAN-1538**: `check_installed` (`runner:504`) uses `build_prefix_dep_command`, which goes through the **non-lifecycle** `host_command_with_env_and_directory` (`runner:28-37`).
  - Fix: delete `build_prefix_dep_command` and the `HostCommandBuilder` indirection, so check, install and restore all use `host_command_with_env_and_directory_lifecycle`.
  - Test: argv under Flatpak is `flatpak-spawn --host --watch-bus … winetricks list-installed`. The inner builder is `pub(super)` in `platform`, so either add a `pub(crate)` `_with(flatpak)` seam or assert at the builder seam. Existing lifecycle argv tests: `platform/tests/gateway.rs:113,143`.

## 7. Test matrix (new, crosshook-app unless noted)

Harness:

- `MetadataStore::open_in_memory()` + seeded profile (`observe_profile_write` → `lookup_profile_id`, pattern in `services/launch/tests/snapshot.rs:25-36`).
- Temp prefix with a `system.reg` fixture (copy `write_registry_fixture`, `runner:732-741`).
- Fake winetricks shell script that appends `"$*"` to `calls.txt` and rewrites `system.reg` on `-q win10` (copy `make_install_restore_binary`, `runner:743-764`).
- Use the real `CoreRestorer` where possible. The fake `JournalStore` is only for fault injection, and it **must assert the prefix and args it receives** (unfalsifiable-fake lesson).
- Verify each test fails when its guarded branch is broken.

| ID  | AC                                         | Arrange                                                                                     | Act                                             | Assert                                                                                                              |
| --- | ------------------------------------------ | ------------------------------------------------------------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| J1a | crash after version change, before restore | `pending` row plan `win10`, `system.reg` = win7 (changed by install)                        | `recover_on_startup`                            | calls = `-q win10` only; row deleted; `Restored`; `status().required == false`                                      |
| J1b | same, protontricks                         | row tool=protontricks, app_id `"12345"`                                                     | `recover_on_startup`                            | argv `12345 -q win10`; row deleted                                                                                  |
| J1c | same, before install                       | J1a row                                                                                     | start install `dotnet48`                        | `recover_before_install` restores first; calls `-q win10` then `-q dotnet48` then `-q win10`; row deleted at end    |
| J2a | crash after restore (before delete)        | `pending` row, `system.reg` already win10                                                   | `recover_on_startup`                            | one restore call (idempotent), verified, row deleted, `Restored`                                                    |
| J2b | crash before spawn (C1)                    | `pending` row, reg unchanged                                                                | `recover_on_startup`                            | same as J2a; no install verb ever called                                                                            |
| J3a | failed restore (exit≠0)                    | script exits 1 on `win10`                                                                   | `recover_on_startup`                            | row `failed`, `last_error` contains "failed"/exit code, sanitized (no `/home/` paths); `RestoreFailed`              |
| J3b | failed restore (verification mismatch)     | script does not rewrite reg                                                                 | recover                                         | `failed`, error contains "verification"                                                                             |
| J3c | failed restore (timeout)                   | restorer with short timeout (test seam like `spawn_with_restore_timeout`, `runner:266-273`) | recover                                         | `failed`, "timed out"; child reaped                                                                                 |
| J3d | failed row is not auto-retried             | `failed` row                                                                                | `recover_on_startup`                            | `SkippedFailed`; `calls.txt` absent; row unchanged                                                                  |
| J3e | failed restore blocks install              | J3a fixture                                                                                 | install job                                     | `Conflict`/repair-required error; **no `-q dotnet48`** in calls; row still `failed`                                 |
| J3f | explicit Repair on failed row              | `failed` row, working script                                                                | `recover(p, true)`                              | row deleted; status not required                                                                                    |
| J4a | delete decisions (pure)                    | —                                                                                           | `decide(*)`                                     | Succeeded→Delete; Failed→MarkFailed(default msg when None); NotRequired/Pending→NoMutation (port `cmd:696-712`)     |
| J4b | delete fails after verified restore        | fake store: delete Err                                                                      | `resolve(Succeeded)`                            | `mark_failed` called with "cleanup failed"; returned outcome `Failed` (port `cmd:757-773`)                          |
| J4c | delete + mark both fail                    | fake store both Err                                                                         | `resolve(Succeeded)`                            | outcome `Failed`; row left `pending`; no panic                                                                      |
| J4d | row vanished under key                     | fake store load→None after restore                                                          | `recover`                                       | `Clean`; delete **not** called                                                                                      |
| J4e | corrupt row never executes/deletes         | protontricks row app_id ∈ {None,"","0","abc","--help"}; verb `-q`                           | `recover` / `status`                            | `Corrupt`/Err; calls absent; row unchanged (port `cmd:743-754`, `runner:1043`)                                      |
| J4f | record fails ⇒ no spawn                    | `MetadataStore::disabled()` or fake upsert Err                                              | install job                                     | Err; calls absent (I1)                                                                                              |
| J4g | spawn failure keeps pending                | binary path missing after record                                                            | install job                                     | row `pending`; status required                                                                                      |
| J4h | explicit version verb ⇒ no row             | install `["win7"]`                                                                          | install job                                     | no `record` call; `NotRequired` (port `runner:1009`)                                                                |
| L1  | per-prefix exclusivity                     | install job A blocked (script `sleep`/fifo)                                                 | 2nd install same prefix; also via symlink alias | `Conflict`; one spawn; `jobs.job_counts()` running == 1                                                             |
| L2  | other prefix allowed                       | while L1 holds A                                                                            | install on prefix B                             | starts (behavior change vs global lock)                                                                             |
| L3  | repair/startup vs install                  | while L1 holds A                                                                            | user Repair A; `recover_on_startup` for A       | Repair → `Conflict`; startup skips A (row left for `recover_before_install`)                                        |
| C1  | cancel still restores                      | install script `sleep 60`                                                                   | `cancel(job)`                                   | SIGTERM to group; restore `-q win10` ran; row deleted; job `Cancelled`                                              |
| S1  | app id parse                               | —                                                                                           | `parse_nonzero_steam_app_id`                    | None for `""`,`"  "`,`"0"`,`"00"`,`"abc"`,`"-1"`,`"+5"`,`"--help"`,`"4294967296"`; `" 12345 "`→`12345`, `"007"`→`7` |
| S2  | app id source order                        | profile steam.app_id invalid, runtime.steam_app_id `"570"`                                  | `resolve_tool`                                  | protontricks with `570`; steam.app_id wins when both valid                                                          |
| S3  | fallback                                   | detection=protontricks, no id, lookup→Some(winetricks)                                      | `resolve_tool`                                  | tool winetricks, app_id None, argv has no id; journal row tool `winetricks`                                         |
| S4  | fallback unavailable                       | detection=protontricks, no id, lookup→None                                                  | `resolve_tool`                                  | `Validation` error; nothing spawned                                                                                 |
| S5  | winetricks ignores id                      | detection=winetricks, id `570`                                                              | install                                         | argv `-q dotnet48` (no id); row `steam_app_id` NULL                                                                 |
| G1  | YAN-1538 (core)                            | flatpak=true                                                                                | check command builder                           | argv `flatpak-spawn --host --watch-bus … list-installed`                                                            |

## 8. Existing tests (keep / port)

**Core `metadata`**:

- `store`: `journal_round_trip_failure_and_delete` (:124), `missing_restore_row_rejects_unsafe_mutations` (:156).
- `migrations/tests/v26_v27.rs`: `fresh_db_reaches_at_least_user_version_27` (:15), `prefix_version_restore_journal_enforces_one_row_per_prefix` (:24), `…survives_profile_delete_without_owner` (:49), `shared_prefix_repair_remains_discoverable_after_owner_profile_delete` (:76).
- `db_tests.rs`: `disabled_store_journal_operations_fail_closed_with_typed_errors` (:446), newer-schema delete is read-only (:53), v27 rung failure (:256-270).
- `migrations/tests/v24_v25.rs:64` (drops the table to rebuild v24).

**Core `prefix_deps`**:

- `runner`:
  - snapshots: `snapshots_windows_7/10` (:767, :779), `…11_from_build…` (:791), `…server_2008_r2…` (:803), `rejects_unknown_prefix_windows_version` (:815)
  - install and restore: `prepared_install_exposes_restore_plan_before_spawn` (:830), `failed_dependency_install_still_restores_prefix_windows_version` (:867), `restoration_timeout_is_reported_separately` (:897), `dropping_install_handle_kills_lifecycle_bound_process` (:929), `restoration_verification_mismatch_is_reported_as_failure` (:977), `explicit_windows_version_verb_has_no_restore_plan` (:1009), `restart_restore_rejects_invalid_restore_verb_before_spawn` (:1043), `install_rejects_uninitialized_prefix` (:1071)
  - commands and arguments: `check_installed_parses_whitespace_output` (:657), `install_packages_rejects_invalid_verbs` (:681), `install_packages_uses_winetricks_compatible_arguments` (:705), `mutation_command_targets_binary_with_resolved_wineprefix_natively` (:1113)
  - prefix normalization: `normalize_prefix_path_*` (:1132, :1149, :1166)
  - sanitize: (:1094, :1105).
- `lock.rs` tests (:87-128): 4 tests, deleted along with the lock. L1/L2 replace them.
- `detection.rs` tests (:128-178): 5 tests, kept.
- `mod.rs` integration tests (:36-141), kept.
- `validation.rs` tests (:93-161), kept.

**Core `platform`**: `platform/tests/gateway.rs:113,143` (lifecycle `--watch-bus` argv).

**src-tauri `cmd` tests (:667-787)**: these move to the app tests above, and the src-tauri copies are deleted with the command logic.

- `row_status_mapping_accepts_pending_and_failed_repairs`, `row_status_mapping_rejects_invalid_state_and_tool` → `JournalEntry`/`status`.
- `journal_decision_only_deletes_verified_success` → J4a.
- `completion_payload_preserves_legacy_success_semantics` → keep in the adapter or app payload.
- `persisted_protontricks_app_id_must_be_positive_decimal` → J4e/S1.
- `journal_delete_failure_keeps_completion_unsafe` → J4b.
- `repair_commands_keep_snake_case_function_contract` → keep in the adapter.

**Frontend**: `components/__tests__/PrefixDepsPanel.test.tsx` (repair flow :44-79, unavailable journal :136-143), `hooks/__tests__/usePrefixDeps.test.ts` (:44-126), `components/library/__tests__/useLaunchDepGate.test.tsx` (:387-456). IPC shapes are unchanged, so these should keep passing.

**Gaps today (no tests)**:

- protontricks→winetricks fallback
- `resolve_profile_steam_app_id` source order
- any crash/startup recovery
- per-prefix (vs global) exclusivity
- install-path delete failure leaving the row `pending`
- user cancel.
