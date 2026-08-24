# Prefix Windows-Version Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make trainer dependency installation preserve Wine prefix compatibility across success, failure, cancellation, application shutdown, and machine restart.

**Architecture:** Add a v27 SQLite repair journal written before Winetricks starts and keyed by the canonical physical `pfx`, refactor the runner into prepare/spawn/restore phases with a structured outcome and 90-second timeout, and expose repair state through thin Tauri commands. The existing Prefix Dependencies panel and launch gate surface pending repair and prevent UI launches until recovery succeeds.

**Tech Stack:** Rust, Tokio, rusqlite, Tauri v2 IPC/events, React 18, TypeScript, Vitest, Flatpak host-command gateway.

**Spec:** `docs/superpowers/specs/2026-08-24-prefix-version-recovery-design.md`

## Global Constraints

- Work directly on `main`; do not create a worktree or PR.
- Route Winetricks/Protontricks through `crosshook-core/src/platform.rs` host-command helpers.
- Treat repair journal rows as SQLite operational metadata; no TOML setting changes.
- Persist the repair row before spawning the dependency process and clear it only after verified restoration.
- Preserve repair rows independently of profile lifetime; `profile_id` is nullable informational context.
- Bound restore attempts to 90 seconds.
- Bind Flatpak dependency mutations to the proxy lifecycle with `flatpak-spawn --host --watch-bus`.
- Treat journal cleanup failure as unsafe, even after successful registry verification.
- Require a positive decimal Steam App ID for Protontricks install and recovery plans.
- Explicit Windows-version verbs are intentional and create no repair journal.
- Keep Tauri commands thin and use snake_case command names matching frontend calls.
- Do not launch games or GPU-heavy GUI verification after the observed AMD pageflip lockup; the user performs the final game/trainer smoke test.

---

### Task 1: SQLite v27 repair journal

**Files:**

- Create: `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/v26_v27.rs`
- Create: `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/tests/v26_v27.rs`
- Create: `src/crosshook-native/crates/crosshook-core/src/metadata/prefix_version_restore_store.rs`
- Modify: `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/mod.rs`
- Modify: `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/tests/mod.rs`
- Modify: `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`
- Modify: `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`
- Modify: `src/crosshook-native/crates/crosshook-core/src/metadata/prefix_ops.rs`

**Interfaces:**

- Produces: `PrefixVersionRestoreJournalRow` with profile/tool/restore/state/error/timestamp fields.
- Produces: `MetadataStore::{upsert_prefix_version_restore, load_prefix_version_restore, mark_prefix_version_restore_failed, delete_prefix_version_restore}`.

- [x] **Step 1: Write failing migration and store tests**

Create v27 tests that assert `user_version = 27`, the table constraints, one row per canonical physical `pfx`, preservation with a null owner after profile deletion, and round-trip store behavior:

```rust
assert_eq!(version, 27);
store.upsert_prefix_version_restore(&record).unwrap();
assert_eq!(store.load_prefix_version_restore("/games/pfx").unwrap().unwrap().restore_verb, "win10");
store.mark_prefix_version_restore_failed("/games/pfx", "restore timed out").unwrap();
store.delete_prefix_version_restore("/games/pfx").unwrap();
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core metadata::migrations::tests::v26_v27
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core prefix_version_restore_store
```

Expected: compilation/test failure because schema v27 and store APIs do not exist.

- [x] **Step 3: Implement migration and metadata facade**

Create the additive table:

```sql
CREATE TABLE prefix_version_restore_journal (
    prefix_path TEXT PRIMARY KEY,
    profile_id TEXT REFERENCES profiles(profile_id) ON DELETE SET NULL,
    binary_path TEXT NOT NULL,
    tool_type TEXT NOT NULL CHECK (tool_type IN ('winetricks','protontricks')),
    steam_app_id TEXT,
    restore_verb TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending','failed')),
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_prefix_version_restore_profile ON prefix_version_restore_journal(profile_id);
```

Use `INSERT ... ON CONFLICT(prefix_path) DO UPDATE` so a retry preserves one authoritative row while refreshing tool details and state. The key is the canonical resolved `pfx`, not a user-entered alias.

- [ ] **Step 4: Run focused metadata tests and verify GREEN**

Run the two commands from Step 2. Expected: PASS.

### Task 2: Prepared install and structured restoration

**Files:**

- Modify: `src/crosshook-native/crates/crosshook-core/src/prefix_deps/models.rs`
- Modify: `src/crosshook-native/crates/crosshook-core/src/prefix_deps/runner.rs`

**Interfaces:**

- Consumes: `PrefixVersionRestoreJournalRow` metadata fields.
- Produces: `prepare_install_packages(...) -> Result<PreparedPrefixDependencyInstall, PrefixDepsError>`.
- Produces: `PreparedPrefixDependencyInstall::{restore_plan, spawn}`.
- Produces: `PrefixDependencyInstall::wait_and_restore() -> PrefixDependencyInstallOutcome`.
- Produces: `restore_prefix_windows_version(&PrefixVersionRestorePlan) -> PrefixVersionRestoreOutcome` for restart recovery.

- [x] **Step 1: Write failing registry parser tests**

Add literal `system.reg` fixtures asserting:

```rust
assert_eq!(snapshot_prefix_windows_version(&win7_pfx).unwrap(), "win7");
assert_eq!(snapshot_prefix_windows_version(&win10_pfx).unwrap(), "win10");
assert_eq!(snapshot_prefix_windows_version(&win11_pfx).unwrap(), "win11");
assert!(snapshot_prefix_windows_version(&ambiguous_pfx).is_err());
```

The Windows 11 fixture must use `CurrentBuild=22000` even if `ProductName` says Windows 10, matching the observed Proton registry.

- [ ] **Step 2: Run parser tests and verify RED**

Run:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core prefix_deps::runner::tests::snapshots_windows
```

Expected: Windows 11 and ambiguous cases fail against ProductName-only detection.

- [x] **Step 3: Implement build-aware compatibility parsing**

Parse the target registry section into a small field struct and map build thresholds before product/version fallbacks. Return `PrefixDepsError::ValidationError` when no supported restore verb can be derived.

- [x] **Step 4: Write failing lifecycle/outcome tests**

Use fake executable scripts to assert separate calls and outcomes:

```rust
assert!(outcome.install_succeeded);
assert_eq!(outcome.restore_state, PrefixVersionRestoreState::Succeeded);
assert_eq!(calls, "-q dotnet48\n-q win10\n");

assert!(!failed_install.install_succeeded);
assert_eq!(failed_install.install_exit_code, Some(42));
assert_eq!(failed_install.restore_state, PrefixVersionRestoreState::Succeeded);

assert_eq!(timed_out.restore_state, PrefixVersionRestoreState::Failed);
assert!(timed_out.restore_error.unwrap().contains("timed out"));
```

Also assert `prepare_install_packages` exposes its restore plan before `spawn()`, explicit `win10` produces no plan, and post-restore registry verification mismatch is failure.

- [ ] **Step 5: Run lifecycle tests and verify RED**

Run:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core prefix_deps::runner::tests
```

Expected: failure because prepared install, structured outcome, timeout, and verification do not exist.

- [x] **Step 6: Implement prepared install and bounded restoration**

Move command spawn out of preparation. Preserve validated per-argument construction and `kill_on_drop(true)`. Resolve the canonical physical `pfx`, require a positive nonzero decimal App ID for Protontricks paths, and use `--watch-bus` for Flatpak mutation commands. Use `tokio::time::timeout(Duration::from_secs(90), child.wait())`, terminate on timeout, sanitize stderr, then re-read the registry and require the expected compatibility verb.

- [ ] **Step 7: Run runner tests and verify GREEN**

Run the command from Step 5. Expected: PASS.

### Task 3: Tauri journal orchestration and repair IPC

**Files:**

- Modify: `src/crosshook-native/src-tauri/src/commands/prefix_deps.rs`
- Modify: `src/crosshook-native/src-tauri/src/lib.rs`

**Interfaces:**

- Consumes: prepared install, structured outcomes, metadata journal facade.
- Produces IPC: `get_prefix_version_repair_status(profile_name, prefix_path) -> PrefixVersionRepairStatus`.
- Produces IPC: `repair_prefix_windows_version(profile_name, prefix_path) -> PrefixVersionRepairStatus`.
- Extends event: `prefix-dep-complete` with `install_succeeded`, `install_exit_code`, `restore_state`, and `restore_error` while retaining `succeeded` for compatibility.

- [x] **Step 1: Write failing Tauri command-contract and outcome tests**

Add tests for snake_case command names and pure outcome-to-persistence decisions:

```rust
assert_eq!(repair_status.state, Some(PrefixVersionRestoreState::Failed));
assert!(repair_status.last_error.unwrap().contains("timed out"));
assert!(journal_should_be_deleted(&successful_outcome));
assert!(!journal_should_be_deleted(&failed_restore_outcome));
```

- [ ] **Step 2: Run Tauri tests and verify RED**

Run:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native commands::prefix_deps
```

Expected: compilation failure because repair commands and structured completion mapping do not exist.

- [x] **Step 3: Implement journal-first install orchestration**

Prepare the install, persist `pending`, then spawn. In the background task, persist package status from `install_succeeded`; delete the journal only for verified restoration, otherwise mark it failed with the sanitized restore error. A spawn failure leaves the pending row recoverable. A failed journal deletion changes the completion outcome back to unsafe so the UI cannot treat restoration as complete.

- [x] **Step 4: Implement synchronous repair commands**

Load the row by canonical physical prefix identity, acquire the existing prefix lock, reconstruct `PrefixVersionRestorePlan`, validate any Protontricks App ID as a positive decimal value, run bounded restoration, delete on success, and return current status. Missing rows return `required: false` idempotently; cleanup failure remains repair-required.

- [ ] **Step 5: Register commands and run Tauri tests**

Run the command from Step 2. Expected: PASS.

### Task 4: Repair UI and non-skippable launch gate

**Files:**

- Modify: `src/crosshook-native/src/types/prefix-deps.ts`
- Modify: `src/crosshook-native/src/hooks/usePrefixDeps.ts`
- Modify: `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts`
- Modify: `src/crosshook-native/src/components/PrefixDepsPanel.tsx`
- Modify: `src/crosshook-native/src/components/library/launch/useLaunchDepGate.ts`
- Modify: `src/crosshook-native/src/components/library/launch/LaunchDepGateModal.tsx`
- Modify: `src/crosshook-native/src/lib/mocks/handlers/system.ts`
- Test: `src/crosshook-native/src/components/__tests__/PrefixDepsPanel.test.tsx`
- Test: `src/crosshook-native/src/components/library/__tests__/useLaunchDepGate.test.tsx`
- Test: `src/crosshook-native/src/components/library/__tests__/LaunchDepGateModal.test.tsx`
- Create: `src/crosshook-native/src/utils/prefixPath.ts`
- Test: `src/crosshook-native/src/utils/__tests__/prefixPath.test.ts`

**Interfaces:**

- Consumes IPC repair status/retry commands.
- Produces `PrefixVersionRepairStatus` TypeScript type and repair methods in both hooks.
- Produces launch-gate state `depGateRepair` and `repairPrefixVersion`.

- [x] **Step 1: Write failing panel and launch-gate tests**

Assert the real UI behavior:

```tsx
expect(screen.getByRole('alert')).toHaveTextContent('Prefix repair required');
await user.click(screen.getByRole('button', { name: 'Repair Prefix' }));
expect(callCommand).toHaveBeenCalledWith('repair_prefix_windows_version', expectedArgs);
```

For launch gating, return `required: true` from the repair-status command and assert `handleBeforeLaunch('game')` returns false without querying packages. Assert the modal has `Repair + Launch` and `Cancel`, but no `Skip and Launch`; successful repair resumes the remembered action.

- [ ] **Step 2: Run frontend tests and verify RED**

Run:

```bash
npm test -- src/components/__tests__/PrefixDepsPanel.test.tsx src/components/library/__tests__/useLaunchDepGate.test.tsx
```

from `src/crosshook-native`. Expected: FAIL because repair state and actions are absent.

- [x] **Step 3: Implement types, hooks, mocks, panel state, and gate**

Load repair state alongside package state. Render a danger-status repair card with sanitized details and an explicit retry. Keep dependency mutations disabled until repair status is known. Check repair before packages in `handleBeforeLaunch`; unlike missing dependencies, repair cannot be skipped. Scope async checks and completion events with generation tokens so stale responses cannot launch an old profile/action or trigger a duplicate launch.

- [ ] **Step 4: Run focused frontend tests and typecheck**

Run from `src/crosshook-native`:

```bash
npm test -- src/components/__tests__/PrefixDepsPanel.test.tsx src/components/library/__tests__/useLaunchDepGate.test.tsx
npm run typecheck
```

Expected: PASS.

### Task 5: Documentation, full verification, review, and direct-main commit

**Files:**

- Modify: `docs/getting-started/quickstart.md`
- Modify: `docs/features/steam-proton-trainer-launch.doc.md`
- Modify: `AGENTS.md`
- Modify: relevant inline module/schema documentation.

**Interfaces:**

- Consumes all completed behavior.
- Produces current schema-v27 and recovery documentation.

- [x] **Step 1: Update documentation and schema inventory**

Document the v27 table, fail-closed journal establishment, non-skippable repair gate, offline recovery, and explicit Windows-version verb exception.

- [ ] **Step 2: Run full verification**

Run:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native
npm test
npm run typecheck
./scripts/lint.sh
./scripts/check-host-gateway.sh
./scripts/build-flatpak.sh --rebuild --strict
```

Use escalation for the core suite's loopback mock servers and strict AppStream URL checks if sandbox restrictions recur. Do not run the game.

- [ ] **Step 3: Run read-only code review and fix actionable findings**

Review correctness, migration safety, cancellation recovery, timeout semantics, user-visible error state, and gateway compliance. Re-run focused tests after each correction.

- [ ] **Step 4: Install the verified Flatpak**

```bash
flatpak install --user --reinstall -y /home/yandy/.local/share/crosshook/artifacts/CrossHook_amd64.flatpak
flatpak info --user --show-commit dev.crosshook.CrossHook
```

- [ ] **Step 5: Commit implementation directly on main**

```bash
git add AGENTS.md docs/getting-started/quickstart.md docs/features/steam-proton-trainer-launch.doc.md src/crosshook-native
git commit -m "fix(prefix): recover interrupted dependency version changes"
```

- [ ] **Step 6: Hand off manual end-to-end confirmation**

Ask the user to verify Ghost of Tsushima reaches the game and the FLiNG trainer still opens. Report the AMD pageflip crash separately from the CrossHook prefix fix.
