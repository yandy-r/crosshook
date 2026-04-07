# PR Review: #167 — feat(ui): Install Game flow parity with wizard (phase 3)

**Reviewed**: 2026-04-06
**Author**: yandy-r
**Branch**: feat/ui-standardization-phase-3 → main
**Decision**: REQUEST CHANGES

## Summary

Solid refactor that aligns the Install Game flow with the profile wizard's tabbed
composition and reuses canonical `profile-sections/*`. Rust IPC parity is correct,
new unit tests cover `reviewable_profile` routing for the three runner methods,
and the validation gate is sensibly relaxed for pre-install (executable not yet
known). Build, type check, and `crosshook-core` tests all pass.

The blockers are scoped:

1. The "first missing field" message exposes raw machine IDs (e.g.
   `installer_path`, `game-executable-path`) to end users.
2. Native runner flows through the entire installer pipeline even though it
   makes no sense for native games.
3. `useInstallGame.ts` ships a freshly-introduced _and_ immediately deprecated
   API surface (`setRequest` / `updateRequest` / `patchRequest`) with no
   callers.

These should be addressed before merge. Everything else is medium/low.

## Findings

### CRITICAL

None.

### HIGH

#### H1 — User-facing copy leaks raw field IDs

**File**: `src/crosshook-native/src/components/InstallGamePanel.tsx:432-438`

```tsx
{
  !installValidation.isReady ? (
    <span id={installRequiredHintId} className="crosshook-help-text">
      {installValidation.firstMissingId
        ? `Complete required fields (first missing: ${installValidation.firstMissingId}).`
        : 'Complete required fields before installing.'}
    </span>
  ) : null;
}
```

`firstMissingId` is one of `profile-name`, `game-name`, `runtime-prefix-path`,
`installer_path`, etc. The wizard treats this id as an `aria-describedby` target
(see `OnboardingWizard.tsx:395-396`), never as user-visible copy. This panel
surfaces it as English text, so users will literally read
`first missing: installer_path`. It is also inconsistent: wizard ids are
kebab-case while the new `installer_path` is snake_case.

**Fix**: resolve to the human label.

```tsx
const firstMissingField = installValidation.fields.find((field) => field.id === installValidation.firstMissingId);

{
  firstMissingField
    ? `Complete required fields (first missing: ${firstMissingField.label}).`
    : 'Complete required fields before installing.';
}
```

While there, normalise the new id in `installValidation.ts:36` to kebab-case
(`'installer-path'`) so the install fields stay consistent with the wizard set.

---

#### H2 — Native runner method walks through Windows installer pipeline

**Files**:

- `src/crosshook-native/src/components/InstallGamePanel.tsx:86-99` (tab visibility)
- `src/crosshook-native/src/components/install/installValidation.ts:33-40` (still requires `installer_path`)
- `src/crosshook-native/crates/crosshook-core/src/install/models.rs:107-114, 164-171` (`native` branch in `reviewable_profile`)

When the user selects `native` as runner method, the panel only hides the
Trainer tab. The Installer & Review tab still asks for "Installer EXE", the
validation gate still demands `installer_path`, the backend still tries to
spawn `proton.exe installer.exe`, and the discovered-candidates UI still talks
about `drive_c/.../Game.exe`. None of that maps to a native Linux game.

The install pipeline is a Proton-only concept. Two reasonable options:

- **Preferred**: hide `native` from the runner-method picker _inside the install
  flow_ (the wizard can still expose it). The simplest fix is a prop on
  `RunnerMethodSection` (`hideNative?: boolean`) and pass it from
  `InstallGamePanel.tsx:261`.
- **Alternative**: if `native` must remain selectable from this panel, switch
  the Installer & Review tab into a "Manual setup" mode (no installer EXE,
  collect game executable directly) when `launchMethod === 'native'`. That is a
  bigger change.

Either way, the current state lets a user start an installer subprocess for a
runner mode that has no meaningful artefacts. Combine that with the fact that
`reviewable_profile` quietly tolerates `native` plus an empty proton path
(models.rs:163-170 — runtime gets a populated prefix but `proton_path: ""`),
and you get a profile that will fail to launch later.

---

#### H3 — Newly-introduced API immediately marked `@deprecated`

**File**: `src/crosshook-native/src/hooks/useInstallGame.ts:23-67`, plus
`298-417`.

```ts
/** @deprecated Use `updateDraftProfile` / `updateInstallerInputs`. */
setRequest: (request: InstallGameRequest) => void;
/** @deprecated Use `updateDraftProfile` / `updateInstallerInputs`. */
updateRequest: <Key extends keyof InstallGameRequest>(...);
/** @deprecated Use `updateDraftProfile` / `updateInstallerInputs`. */
patchRequest: (patch: Partial<InstallGameRequest>) => void;
```

`grep -rn 'setRequest\|updateRequest\|patchRequest' src/crosshook-native/src`
returns no consumers. CLAUDE.md is explicit: "Prefer clean refactors over
layering temporary compatibility shims." Adding a brand-new hook API and
marking it deprecated in the same commit is a contradiction — there is no
legacy caller because this code is new in this PR. Delete `setRequest`,
`updateRequest`, and `patchRequest` (and the runtime branches in `updateRequest`
that have no test coverage), or, if external future callers are anticipated,
remove the `@deprecated` tag and document the contract.

The runtime cast `const v = value as string;` (line 336) makes this worse — it
silently lies to the type system. Today every `InstallGameRequest` field is a
string so the lie is harmless, but it will rot the moment a non-string field is
added.

### MEDIUM

#### M1 — `useInstallGame.ts` is now 729 lines

**File**: `src/crosshook-native/src/hooks/useInstallGame.ts`

The repo coding rules cap files at 800 lines and call out 200-400 as the
typical band. This hook crams together: state, error mapping, derived
status/hint text, request building, default-prefix resolution, deprecated
shims, and the install command. After H3 is removed it gets meaningfully
smaller, but it would still benefit from extraction:

- `installValidationMapping.ts` — `mapValidationErrorToField`
- `installStatusText.ts` — `deriveStatusText` / `deriveHintText` /
  `deriveResultStage`
- `installRequest.ts` — `buildInstallGameRequest`

These are pure functions and trivially testable in isolation.

---

#### M2 — `resolveDefaultPrefixPath` closes over stale `defaultPrefixPath` and triggers redundant IPC

**File**: `src/crosshook-native/src/hooks/useInstallGame.ts:508-571, 639-666`

`resolveDefaultPrefixPath` is wrapped with `useCallback(..., [defaultPrefixPath])`
because it reads `defaultPrefixPath` on line 539. The debouncing effect on
line 666 then depends on `resolveDefaultPrefixPath`. As a result:

1. user types → effect schedules timeout
2. timeout fires → `setDefaultPrefixPath(resolved)` updates state
3. `resolveDefaultPrefixPath` is recreated with new deps
4. effect re-runs → schedules another debounced IPC call to the backend

The second invocation usually returns the same value so React bails on the
re-render, but it still issues a redundant `install_default_prefix_path` IPC
per profile-name change. Use a ref to read `defaultPrefixPath` inside the
callback (or move the "should apply resolved prefix" logic to compare against
the _resolved_ value the IPC just returned). The effect can then depend only
on `profileName`.

---

#### M3 — `prefixStateLabel` duplicated in two files

**Files**:

- `src/crosshook-native/src/components/InstallGamePanel.tsx:36-48`
- `src/crosshook-native/src/components/install/InstallReviewSummary.tsx:50-62`

Identical bodies. Move to `src/components/install/installLabels.ts` (or
similar) and import in both spots.

---

#### M4 — Test gap on `steam_applaunch` routing

**File**: `src/crosshook-native/crates/crosshook-core/src/install/models.rs:401-427`

`reviewable_profile_routes_steam_app_id_for_steam_applaunch` only asserts
`launch.method`, `steam.app_id`, and `runtime.steam_app_id`. The same routing
also moves `prefix_path` → `steam.compatdata_path`, `proton_path` →
`steam.proton_path`, and zeroes the runtime equivalents. Add asserts for those
so a future regression that mis-routes proton/prefix in steam_applaunch mode
breaks the test.

```rust
assert_eq!(profile.steam.compatdata_path, prefix_path.to_string_lossy());
assert_eq!(profile.steam.proton_path, "/proton");
assert!(profile.runtime.prefix_path.is_empty());
assert!(profile.runtime.proton_path.is_empty());
```

---

#### M5 — `setResult` "merge in user art paths" logic is fragile

**File**: `src/crosshook-native/src/hooks/useInstallGame.ts:454-481`

`setResult` overwrites the entire draft profile with the backend's
`nextResult.profile`, then attempts to preserve user-entered art paths by
spreading them back in. Anything else the user typed in the draft (custom env
vars, optimization presets, working directory if they didn't accept the
discovered candidate, etc.) is silently dropped. That contradicts the panel
copy which says "the generated profile stays editable until the modal save
step".

Either:

- merge the backend result into the draft field-by-field (only fields the
  backend actually owns: `game.executable_path`, `runtime.working_directory`),
  or
- document explicitly which fields the backend authoritatively replaces.

### LOW

#### L1 — Empty CSS hook class

**File**: `src/crosshook-native/src/styles/theme.css:604-607`

```css
/* Trainer tab omitted when launch method is native — hook class for future layout tweaks. */
.crosshook-install-flow-tabs--skip-trainer {
}
```

YAGNI — empty rulesets are stylelint warnings on most configs and add
maintenance noise. The class is also conditionally added in
`InstallGamePanel.tsx:213-214` despite doing nothing. Remove both, add when
there is an actual style to apply.

---

#### L2 — `reset()` does not bump `prefixResolutionRequestIdRef`

**File**: `src/crosshook-native/src/hooks/useInstallGame.ts:626-637`

If the user clicks Reset while a debounced prefix-resolve is in flight, the
in-flight resolve will still call `setDefaultPrefixPath` because the effect's
guard (`requestId !== prefixResolutionRequestIdRef.current`) compares against
the value at scheduling time. Bumping the ref in `reset()` closes the race.

---

#### L3 — Inline styles where a CSS class would carry intent

**File**: `src/crosshook-native/src/components/install/InstallReviewSummary.tsx:102-149, 162-166`

Several inline `style={{ ... }}` blocks duplicate styling that could live in
`theme.css` (`crosshook-install-candidate-list`, candidate selected state,
"Installer log path" row). Web rules in this repo prefer design tokens via CSS
custom properties.

---

#### L4 — `display: none` pattern is verbose but consistent

**File**: `src/crosshook-native/src/components/InstallGamePanel.tsx:247-379`

Each `<Tabs.Content forceMount style={{ display: ... }}>` block repeats the
pattern five times. It matches existing usage in `InstallPage.tsx` so this is a
note, not a request — but a small wrapper component would clean it up if you
ever touch this file again.

## Validation Results

| Check                                             | Result                                     |
| ------------------------------------------------- | ------------------------------------------ |
| `cargo test -p crosshook-core`                    | Pass (720 + 3 + 0 tests)                   |
| `cargo clippy -p crosshook-core` (install module) | Pass (no warnings on changed files)        |
| `npm run build` (`tsc && vite build`)             | Pass                                       |
| Frontend tests                                    | Skipped (no framework configured per repo) |

Pre-existing clippy errors in `protonup/mod.rs` are unrelated to this PR (they
exist on `main` as well).

## Files Reviewed

| File                                                                   | Change   |
| ---------------------------------------------------------------------- | -------- |
| `.claude/PRPs/plans/completed/ui-standardization-phase-3.plan.md`      | Moved    |
| `.claude/PRPs/reports/ui-standardization-phase-3-report.md`            | Added    |
| `src/crosshook-native/crates/crosshook-core/src/install/models.rs`     | Modified |
| `src/crosshook-native/crates/crosshook-core/src/install/service.rs`    | Modified |
| `src/crosshook-native/src/components/InstallGamePanel.tsx`             | Modified |
| `src/crosshook-native/src/components/install/InstallReviewSummary.tsx` | Added    |
| `src/crosshook-native/src/components/install/installValidation.ts`     | Added    |
| `src/crosshook-native/src/hooks/useInstallGame.ts`                     | Modified |
| `src/crosshook-native/src/styles/theme.css`                            | Modified |
| `src/crosshook-native/src/types/install.ts`                            | Modified |
| `src/crosshook-native/src/types/profile.ts`                            | Modified |
