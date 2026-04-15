# Plan: umu-launcher Migration — Phase 5 (Flatpak host-shared runtime + install guidance)

## Summary

Implement the Flatpak-specific phase of UMU migration by adding host-shared runtime filesystem access and upgrading onboarding readiness from passive info text to actionable install guidance. Keep launch behavior backward-compatible: users without umu remain on direct Proton, while users who install umu through guided steps can immediately benefit from the existing Auto preference path.

## User Story

As a Flatpak CrossHook user, I want clear one-step guidance to install or verify host `umu-run`, so that non-Steam launches work with the same UMU flow available to native users without manual troubleshooting.

## Problem → Solution

Current state: Flatpak has host command bridging and umu path resolution support, but onboarding only reports "umu-run not found" as an informational check with no remediation or persisted dismissal state.

Desired state: Flatpak manifest explicitly allows host-shared `~/.local/share/umu`, readiness emits actionable remediation metadata, onboarding UI renders install guidance for Flatpak users, and a persisted dismiss timestamp prevents repetitive nags while preserving Proton fallback.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/umu-launcher-migration.prd.md`
- **PRD Phase**: Phase 5 — Flatpak host-shared umu runtime + install guidance
- **Estimated Files**: 10 (Rust core + Tauri commands + TS types/hooks/components + Flatpak manifest + tests)
- **Research Dispatch**: Parallel sub-agents (`--parallel`)

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | —          | 3              |
| B2    | 2.1, 2.2, 2.3 | B1         | 3              |
| B3    | 3.1, 3.2, 3.3 | B2         | 3              |
| B4    | 4.1           | B3         | 1              |

- **Total tasks**: 10
- **Total batches**: 4
- **Max parallel width**: 3

---

## UX Design

### Before

```
Onboarding -> Review -> Run Checks
  System Checks:
    [i] umu-run not found; CrossHook will use Proton directly.

No install actions, no "don't show again", no Flatpak-specific guidance.
```

### After

```
Onboarding -> Review -> Run Checks
  System Checks:
    [!] umu-run not found in Flatpak host environment
        [Copy command] [Open docs] [Dismiss reminder]

Flatpak users get host install guidance; native users keep current info-only messaging.
Dismissal is persisted (timestamp) and respected by onboarding reminders.
```

### Interaction Changes

| Touchpoint                      | Before                             | After                                                                    | Notes                                                       |
| ------------------------------- | ---------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------------------- |
| Flatpak manifest runtime access | No explicit `xdg-data/umu` binding | Adds `--filesystem=xdg-data/umu:create`                                  | Mirrors host-shared runtime pattern used by other launchers |
| `check_readiness` umu result    | Message-only informational check   | Structured install guidance metadata for Flatpak missing-umu case        | Still returns via existing onboarding IPC command           |
| Onboarding review panel         | Only check message text            | Adds actionable guidance row(s), copyable command hints, dismiss control | Uses existing `runChecks()` flow                            |
| Settings persistence            | No nag-dismiss field               | Adds `install_nag_dismissed_at` in TOML                                  | Default `None`, backward compatible                         |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                       | Lines            | Why                                                       |
| -------------- | -------------------------------------------------------------------------- | ---------------- | --------------------------------------------------------- |
| P0 (critical)  | `docs/prps/prds/umu-launcher-migration.prd.md`                             | 204-216, 236-272 | Phase 5 scope, storage classification, fallback rules     |
| P0 (critical)  | `packaging/flatpak/dev.crosshook.CrossHook.yml`                            | 26-55            | `finish-args` permissions and filesystem access pattern   |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`   | 125-151          | Existing `umu_run_available` check to extend              |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | 613-653          | Flatpak host PATH umu resolution contract                 |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/onboarding.rs`                | 9-23             | Current readiness and onboarding-dismiss IPC pattern      |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | 171-249          | Add new TOML field with serde defaults and `Default` impl |
| P1 (important) | `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`       | 56-93            | Existing rendering path for readiness checks              |
| P1 (important) | `src/crosshook-native/src/hooks/useOnboarding.ts`                          | 107-115          | Error and state handling for readiness checks             |
| P1 (important) | `src/crosshook-native/src/types/onboarding.ts`                             | 1-30             | Onboarding IPC type contract extension                    |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/settings.rs`                  | 24-175           | Settings IPC DTO + merge/save pattern for new field       |
| P2 (reference) | `docs/prps/plans/completed/umu-migration-phase-4-auto-default.plan.md`     | all              | Existing UMU planning conventions and task granularity    |
| P2 (reference) | `src/crosshook-native/src/context/PreferencesContext.tsx`                  | 103-112          | Frontend settings patch persistence workflow              |

## External Documentation

| Topic                                | Source                                                        | Key Takeaway                                                        |
| ------------------------------------ | ------------------------------------------------------------- | ------------------------------------------------------------------- |
| Flatpak finish args (`--filesystem`) | <https://docs.flatpak.org/en/latest/sandbox-permissions.html> | Use explicit minimal filesystem grants for host-shared runtime data |
| umu-launcher runtime location        | PRD + existing repo decisions                                 | Keep host-shared approach; no bundled umu in CrossHook Flatpak      |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
#[serde(rename_all = "snake_case")]
pub enum UmuPreference {
    Auto,
    Umu,
```

### ERROR_HANDLING

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/onboarding.rs
let mut settings = store.load().map_err(|e| e.to_string())?;
settings.onboarding_completed = true;
store.save(&settings).map_err(|e| e.to_string())
```

### LOGGING_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs
tracing::debug!(
    root_count = steam_roots.len(),
    "Steam root discovery complete"
);
```

### CONTRACT_PATTERN

```rust
// SOURCE: src/crosshook-native/src-tauri/src/lib.rs
commands::umu_database::refresh_umu_database,
commands::umu_database::check_umu_coverage,
```

### FLATPAK_PERMISSION_PATTERN

```yaml
# SOURCE: packaging/flatpak/dev.crosshook.CrossHook.yml
- --filesystem=home
- --filesystem=/mnt
- --filesystem=~/.var/app/com.valvesoftware.Steam:ro
```

### TEST_STRUCTURE

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs
let result = evaluate_checks(&[steam_root], &[proton]);
assert!(result.all_passed, "expected all_passed; checks: {:?}", result.checks);
assert_eq!(result.checks.len(), 5);
```

### STATE_UPDATE_PATTERN

```typescript
// SOURCE: src/crosshook-native/src/hooks/useOnboarding.ts
const result = await callCommand<ReadinessCheckResult>('check_readiness');
setReadinessResult(result);
setCheckError(null);
```

---

## Files to Change

| File                                                                     | Action | Justification                                                                          |
| ------------------------------------------------------------------------ | ------ | -------------------------------------------------------------------------------------- |
| `packaging/flatpak/dev.crosshook.CrossHook.yml`                          | UPDATE | Add host-shared umu filesystem permission                                              |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`         | UPDATE | Add persisted dismissal timestamp field and defaults                                   |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`                | UPDATE | Expose new settings field via IPC DTO + save merge                                     |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` | UPDATE | Upgrade missing-umu readiness check from passive info to actionable guidance payload   |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs`              | UPDATE | Add command(s) to persist/read install-nag dismissal, preserve snake_case IPC contract |
| `src/crosshook-native/src/types/onboarding.ts`                           | UPDATE | Extend readiness contract with structured umu guidance metadata                        |
| `src/crosshook-native/src/hooks/useOnboarding.ts`                        | UPDATE | Track/install-guidance-specific state and dismiss action                               |
| `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`     | UPDATE | Render install guidance actions for Flatpak missing-umu check                          |
| `src/crosshook-native/src/types/settings.ts`                             | UPDATE | Add `install_nag_dismissed_at` to TS settings types/default transform                  |
| `src/crosshook-native/src/context/PreferencesContext.tsx`                | UPDATE | Ensure persisted settings patch flow supports new dismissal timestamp field            |

## NOT Building

- Steam-profile runtime migration to umu.
- Removal of direct `"$PROTON" run` fallback path.
- Bundling umu binaries or SLR assets inside CrossHook Flatpak.
- HTTP `GAMEID` resolver or telemetry pipeline changes (covered elsewhere).
- New onboarding wizard stage; this phase only enriches existing review/check UX.

## Persistence / Usability

| Datum                                         | Classification                    | Migration / compatibility                         | Offline / degraded behavior                                                         | User visibility                                                  |
| --------------------------------------------- | --------------------------------- | ------------------------------------------------- | ----------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| `install_nag_dismissed_at: Option<DateTime>`  | TOML settings (`AppSettingsData`) | New optional field defaults to `None` when absent | If unset or parse fails, UI may re-show guidance; launches still fallback to Proton | Not directly user-editable; set via dismiss action               |
| Flatpak host umu readiness guidance payload   | Runtime-only derived state        | No persisted migration required                   | Missing host umu still launches via Proton path                                     | Shown in onboarding system checks                                |
| `--filesystem=xdg-data/umu:create` finish arg | Packaging config                  | Requires rebuilt Flatpak artifact                 | Without permission, host-shared runtime may remain inaccessible in sandbox          | Not visible in UI; observable through improved readiness outcome |

---

## Step-by-Step Tasks

### Task 1.1: Add Flatpak host-shared umu filesystem permission — Depends on [none]

- **BATCH**: B1
- **ACTION**: Update Flatpak finish args to expose host `xdg-data/umu`.
- **IMPLEMENT**: In `packaging/flatpak/dev.crosshook.CrossHook.yml`, add `--filesystem=xdg-data/umu:create` near existing filesystem permissions and keep current comments consistent with host-runtime rationale.
- **MIRROR**: `FLATPAK_PERMISSION_PATTERN`.
- **IMPORTS**: none.
- **GOTCHA**: Do not weaken existing read-only Steam binding or add broad filesystem grants beyond this phase scope.
- **VALIDATE**: `rg "xdg-data/umu:create" "packaging/flatpak/dev.crosshook.CrossHook.yml"` returns exactly one result.

### Task 1.2: Add persisted install-nag dismissal setting field — Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend Rust and TS settings models with optional dismissal timestamp.
- **IMPLEMENT**: Add `install_nag_dismissed_at` to `AppSettingsData` in `settings/mod.rs` with serde-default behavior and `Default` initialization to `None`; mirror this field in `src/crosshook-native/src/types/settings.ts` and conversion helpers used by `settings_save`.
- **MIRROR**: `NAMING_CONVENTION`, `STATE_UPDATE_PATTERN`.
- **IMPORTS**: reuse existing serde traits; use existing timestamp representation style in TS (nullable string).
- **GOTCHA**: Keep backwards compatibility for existing `settings.toml` files; missing field must deserialize cleanly.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core settings_backward_compat_without_umu_preference` still passes, plus a new test for missing `install_nag_dismissed_at`.

### Task 1.3: Upgrade readiness umu check to actionable guidance payload — Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend readiness check output for Flatpak + missing-umu case.
- **IMPLEMENT**: In `onboarding/readiness.rs`, keep current check structure but attach guidance metadata/remediation text when `platform::is_flatpak()` and `resolve_umu_run_path()` returns `None`; preserve info-level semantics for native installs unless requirements call for warning severity.
- **MIRROR**: `LOGGING_PATTERN`, `ERROR_HANDLING`.
- **IMPORTS**: `crate::platform` helper(s) if needed for flatpak detection.
- **GOTCHA**: Do not regress `all_passed` semantics unexpectedly; warning/error severities affect onboarding pass/fail behavior.
- **VALIDATE**: Add/update unit tests in `readiness.rs` covering native-no-umu and flatpak-no-umu outputs.

### Task 2.1: Surface new settings field across settings IPC boundary — Depends on [1.2]

- **BATCH**: B2
- **ACTION**: Thread `install_nag_dismissed_at` through Tauri settings DTOs and save merge logic.
- **IMPLEMENT**: Update `AppSettingsIpcData`, `SettingsSaveRequest`, `from_parts`, and `merge_settings_from_request` in `src-tauri/src/commands/settings.rs` to include optional timestamp value without clobbering unrelated fields.
- **MIRROR**: `CONTRACT_PATTERN`, `ERROR_HANDLING`.
- **IMPORTS**: `serde::{Serialize, Deserialize}` already in file; add option field wiring only.
- **GOTCHA**: Preserve secret-handling and existing SteamGridDB key flow; do not expose sensitive fields.
- **VALIDATE**: Add/adjust command contract tests and run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native settings`.

### Task 2.2: Add onboarding IPC for install-guidance dismissal — Depends on [1.2, 1.3]

- **BATCH**: B2
- **ACTION**: Introduce `snake_case` onboarding command(s) to persist and optionally clear/read dismissal state.
- **IMPLEMENT**: In `src-tauri/src/commands/onboarding.rs`, add dedicated command handlers (for example `dismiss_umu_install_nag`) using `SettingsStore` load/update/save pattern, map errors to `String`, and register in command signature test.
- **MIRROR**: `ERROR_HANDLING`, `CONTRACT_PATTERN`.
- **IMPORTS**: `SettingsStore`, `State<'_, SettingsStore>`.
- **GOTCHA**: Keep existing `dismiss_onboarding` behavior unchanged; this is a separate state from onboarding completion.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native onboarding::tests`.

### Task 2.3: Extend onboarding TS contracts and hook state — Depends on [1.3, 2.2]

- **BATCH**: B2
- **ACTION**: Update frontend types and onboarding hook to consume guidance metadata and dismissal APIs.
- **IMPLEMENT**: Expand `src/types/onboarding.ts` for guidance payload shape and update `useOnboarding.ts` to store it, expose dismiss callback(s), and retain current `runChecks` error handling semantics.
- **MIRROR**: `STATE_UPDATE_PATTERN`.
- **IMPORTS**: `callCommand` in hook; new type exports.
- **GOTCHA**: Avoid breaking the existing stage flow (`identity_game` -> `completed`); guidance state should be additive.
- **VALIDATE**: `./scripts/lint.sh` passes with no TypeScript/Biome errors.

### Task 3.1: Render actionable UMU install guidance in onboarding review UI — Depends on [2.3]

- **BATCH**: B3
- **ACTION**: Update `WizardReviewSummary` to show guidance rows and dismissal action for Flatpak missing-umu checks.
- **IMPLEMENT**: Add conditional UI block in `WizardReviewSummary.tsx` under System Checks using readiness guidance metadata; include accessible labels and minimal CTA set (copy command/open docs/dismiss).
- **MIRROR**: existing readiness list rendering and badge helpers in same component.
- **IMPORTS**: any helper for clipboard/open external URL already used elsewhere; otherwise keep UI text-only and defer command execution to parent.
- **GOTCHA**: Keep component pure with no direct IPC; pass callbacks/flags as props from hook/container.
- **VALIDATE**: Browser smoke in `./scripts/dev-native.sh --browser` confirms guidance appears only in the intended scenario.

### Task 3.2: Ensure preferences/settings persistence flow handles timestamp patches — Depends on [1.2, 2.1]

- **BATCH**: B3
- **ACTION**: Verify `PreferencesContext` and settings save request helper include new field consistently.
- **IMPLEMENT**: Update `src/types/settings.ts` and any `toSettingsSaveRequest` mapping so `install_nag_dismissed_at` round-trips without resetting to null on unrelated saves; ensure `persistSettings` merge behavior remains immutable.
- **MIRROR**: existing `persistSettings` merge + save + reload pattern in `PreferencesContext.tsx`.
- **IMPORTS**: none beyond current settings types.
- **GOTCHA**: Avoid accidental loss of timestamp when other settings are saved from unrelated UI sections.
- **VALIDATE**: Add/adjust TS unit-type assertions if present; otherwise lint + manual check that save/load preserves timestamp.

### Task 3.3: Add/adjust tests across readiness, settings, and onboarding commands — Depends on [2.1, 2.2, 2.3]

- **BATCH**: B3
- **ACTION**: Add focused regression tests for new fields and guidance behavior.
- **IMPLEMENT**: Extend existing test modules in `settings/mod.rs`, `commands/settings.rs`, `onboarding/readiness.rs`, and `commands/onboarding.rs` for back-compat defaults, IPC contract fields, and dismissal mutation.
- **MIRROR**: `TEST_STRUCTURE`.
- **IMPORTS**: reuse existing tempdir and command signature test styles.
- **GOTCHA**: Keep tests deterministic; avoid dependence on host Flatpak state by using isolated helper functions and injected conditions where needed.
- **VALIDATE**: Run targeted cargo tests for each touched module, then full `crosshook-core` suite.

### Task 4.1: Run full validation and manual Flatpak readiness scenarios — Depends on [3.1, 3.2, 3.3]

- **BATCH**: B4
- **ACTION**: Execute project validation commands and manual checklist for Phase 5 acceptance.
- **IMPLEMENT**: Run lint, core tests, and browser dev smoke; manually verify three scenarios: Flatpak+umu present, Flatpak+umu missing+dismissed, native+umu missing.
- **MIRROR**: existing UMU phase validation cadence from prior phase plans.
- **IMPORTS**: n/a.
- **GOTCHA**: Browser mode uses mock IPC; re-verify readiness behavior in full native dev mode before merge.
- **VALIDATE**: All commands exit 0 and checklist items are completed.

---

## Testing Strategy

### Unit Tests

| Test                                                                | Input                          | Expected Output                                 | Edge Case? |
| ------------------------------------------------------------------- | ------------------------------ | ----------------------------------------------- | ---------- |
| `settings_backward_compat_without_install_nag_field`                | Legacy TOML missing new field  | `install_nag_dismissed_at == None`              | Yes        |
| `settings_save_roundtrip_preserves_install_nag_dismissed_at`        | Save/load with timestamp       | Timestamp survives roundtrip                    | Yes        |
| `readiness_reports_actionable_umu_guidance_for_flatpak_missing_umu` | Flatpak=true, no umu path      | Guidance payload present, remediation non-empty | Yes        |
| `readiness_native_missing_umu_keeps_info_path`                      | Flatpak=false, no umu path     | Existing fallback semantics preserved           | Regression |
| `dismiss_umu_install_nag_updates_settings`                          | IPC command call               | Field set to current timestamp                  | Yes        |
| `settings_ipc_includes_install_nag_field`                           | `settings_load` DTO conversion | Serialized output includes optional field       | Regression |

### Edge Cases Checklist

- [ ] Existing users with no `install_nag_dismissed_at` continue to load settings.
- [ ] Dismissal survives restart and unrelated settings saves.
- [ ] Flatpak host has `umu-run` -> no install nag shown.
- [ ] Flatpak host missing `umu-run` -> guidance shown exactly when expected.
- [ ] Native host missing `umu-run` does not get Flatpak-specific CTA noise.
- [ ] Onboarding completion and install nag dismissal remain independent flags.

---

## Validation Commands

### Static Analysis

```bash
./scripts/lint.sh
```

EXPECT: No lint, format, or type violations.

### Unit Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: `crosshook-core` suite passes with new readiness/settings tests.

### Tauri Command Contract Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native onboarding
```

EXPECT: Onboarding command signature and new dismiss command tests pass.

### Browser Validation

```bash
./scripts/dev-native.sh --browser
```

EXPECT: Review step shows actionable UMU guidance only when readiness data indicates Flatpak+missing-umu.

### Manual Validation

- [ ] Rebuild Flatpak and inspect effective permissions include `xdg-data/umu:create`.
- [ ] In Flatpak session with no host umu, run checks and verify guidance + dismiss CTA.
- [ ] Dismiss guidance, restart app, verify nag remains suppressed.
- [ ] Install host umu, rerun checks, verify readiness reports umu available.

---

## Acceptance Criteria

- [ ] Flatpak manifest includes `--filesystem=xdg-data/umu:create`.
- [ ] Onboarding readiness returns actionable install guidance when Flatpak cannot resolve `umu-run`.
- [ ] New optional settings field `install_nag_dismissed_at` persists and is backward-compatible.
- [ ] Frontend onboarding review UI renders/install-dismisses guidance without breaking existing check output.
- [ ] Existing Proton fallback behavior remains intact for missing umu scenarios.
- [ ] All validation commands pass.

## Completion Checklist

- [ ] Code follows existing readiness, settings IPC, and onboarding hook patterns.
- [ ] New persisted datum is correctly classified as TOML settings and documented.
- [ ] No secret handling regressions introduced in settings IPC.
- [ ] Tests cover both Flatpak and native missing-umu branches.
- [ ] No scope creep into Steam profile migration, telemetry, or HTTP resolver work.
- [ ] Plan remains self-contained for single-pass implementation.

## Risks

| Risk                                                        | Likelihood | Impact                                          | Mitigation                                                              |
| ----------------------------------------------------------- | ---------- | ----------------------------------------------- | ----------------------------------------------------------------------- |
| Flatpak permission addition insufficient on some hosts      | Medium     | Guidance may still fail to resolve host runtime | Keep Proton fallback and document host install alternatives             |
| Timestamp field accidentally reset by generic settings save | Medium     | Nag reappears unexpectedly                      | Thread field through TS + Rust save DTOs and add roundtrip tests        |
| Actionable guidance degrades onboarding signal-to-noise     | Low        | UI clutter, user confusion                      | Gate by Flatpak+missing-umu only and support explicit dismiss           |
| Readiness severity changes affect `all_passed` unexpectedly | Low        | Wizard behavior regressions                     | Preserve severity policy unless explicitly intended; test both branches |

## Notes

- Prefer extending existing readiness check contracts over adding a new onboarding service surface.
- Keep command names in `snake_case` and IPC payloads serde-compatible across Rust/TS boundaries.
- Do not remove or alter the current `UmuPreference::Proton` compatibility escape hatch.
- If Flathub publication status for `org.openwinecomponents.umu.umu-launcher` is still unresolved, guidance must include non-Flathub fallback instructions.
