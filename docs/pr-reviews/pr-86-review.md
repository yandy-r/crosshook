# PR #86 Review: feat(launch): add dry run / preview launch mode

**Branch:** `feat/dryrun-preview` -> `main`
**Closes:** #40
**Reviewed:** 2026-03-27
**Verdict:** Approve with suggestions

---

## Overview

Adds three preview modals: Launch Preview (terraform-plan style dry run), Profile Preview (shareable TOML), and Launcher Preview (script + desktop entry). Includes backend types, exhaustive validation, Tauri IPC commands, React modals with focus traps, clipboard utility, and 18 new tests.

**Stats:** ~2,600 lines of source code changes (excluding docs/plans), 41 files changed, 18 new tests (all 185 pass).

## Remediation Status

| Issue | Status | Validation |
| --- | --- | --- |
| C1 | Fixed | `npm run build`; manual clipboard-failure UI smoke test still pending |
| C2 | Fixed | `npm run build`; manual profile-preview error UI smoke test still pending |
| I1 | Fixed | `npm run build`; manual malformed-timestamp UI smoke test still pending |
| I2 | Fixed | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core preview` |
| S1 | Fixed | Rust serialization tests; `npm run build` |
| S2 | Fixed | Rust serialization tests; `npm run build` |
| S3 | Fixed | Code review + Rust test pass |
| S4 | Fixed | Rust tests; `npm run build` |
| S5 | Fixed | New Rust regression test; full Rust workspace test pass |
| S6 | Fixed | New Rust regression test; full Rust workspace test pass |

---

## Critical Issues (2)

### C1. `copyToClipboard()` never throws; callers show "Copied!" on failure

**Files:** `src/utils/clipboard.ts`, `LaunchPanel.tsx:302-309`, `ProfilePreviewModal.tsx:172-179`, `LauncherPreviewModal.tsx:181-198`

The clipboard utility has two layers of silent failure:

1. `navigator.clipboard.writeText` failure is caught with an empty catch and falls through to the `execCommand` fallback
2. `document.execCommand('copy')` return value (`false` on failure) is completely ignored
3. The function returns `Promise<void>` and never throws

All four modal callers also catch and swallow, then show "Copied!" feedback regardless of success. The user clicks "Copy", sees "Copied!", pastes, and gets nothing.

**Note:** `SteamLaunchOptionsPanel.tsx` already handles this correctly with `setCopyLabel('Copy failed')` on error.

**Recommendation:** Make `copyToClipboard()` throw on failure (check `execCommand` return value, re-throw if both methods fail). Update callers to show "Copy failed" on error, matching the existing `SteamLaunchOptionsPanel` pattern.

**Status (2026-03-27):** Fixed on `feat/dryrun-preview`. `copyToClipboard()` now throws when both copy mechanisms fail, and the Launch Preview, Profile Preview, and Launcher Preview modals all show explicit failure feedback instead of false success. Validated by `npm run build`; live clipboard-failure UI smoke testing is still pending.

### C2. `handlePreviewProfile()` swallows errors with `console.error` only

**File:** `src/components/pages/ProfilesPage.tsx:233-247`

When `invoke('profile_export_toml', ...)` fails, the error is caught at line 242 with only `console.error`. The user gets zero feedback -- the button returns to normal state and nothing indicates why the preview didn't appear.

**Recommendation:** Add a `previewError` state and render it in the UI with `role="alert"`, matching the `LaunchPanel` pattern that already renders `previewError` at line 781-785.

**Status (2026-03-27):** Fixed on `feat/dryrun-preview`. `ProfilesPage` now keeps a local `previewError`, clears it on retry/success, and renders a visible `role="alert"` message near the profile action area. Validated by `npm run build`; live UI smoke testing is still pending.

---

## Important Issues (2)

### I1. `isStale()` returns `false` on unparseable timestamps

**File:** `src/components/LaunchPanel.tsx:113-119`

If `generatedAt` is malformed and `new Date()` throws, the catch returns `false` (meaning "fresh"). The conservative default should be `true` (treat unknown-age previews as stale).

**Status (2026-03-27):** Fixed on `feat/dryrun-preview`. Invalid timestamps now default to stale, Launch Preview derives readiness from `issues.length === 0`, and the footer falls back to `time unavailable` instead of rendering a misleading invalid time. Validated by `npm run build`; live UI smoke testing is still pending.

### I2. Wine prefix resolution logic duplicated between `build_proton_setup` and `collect_runtime_proton_environment`

**File:** `crates/crosshook-core/src/launch/preview.rs:345-368` vs `456-474`

Both functions call `resolve_wine_prefix_path()` and derive compat_data with the same pfx-parent heuristic. If the heuristic changes in one place and not the other, the env vars and proton_setup sections will show different paths.

**Recommendation:** Extract a shared helper like `fn resolve_proton_paths(prefix: &Path) -> (PathBuf, String)` used by both functions.

**Status (2026-03-27):** Fixed on `feat/dryrun-preview`. Shared proton path derivation now lives in `runtime_helpers.rs` and is reused by both runtime env collection and Proton setup preview generation. Validated by new Rust regression tests plus `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core preview`.

---

## Suggestions (6)

### S1. `PreviewTrainerInfo.loading_mode` should use `TrainerLoadingMode` enum, not `String`

**File:** `preview.rs:54`, `launch.ts:88`

`TrainerLoadingMode` is imported in this file and already derives `Serialize`/`Deserialize` with `snake_case` rename. The builder at line 524 calls `.as_str().to_string()` -- deliberately discarding the typed enum into a stringly-typed field. Trivial fix, adds compile-time safety.

**Status (2026-03-27):** Implemented. `PreviewTrainerInfo.loading_mode` now uses `TrainerLoadingMode` on the Rust side and in the TypeScript preview contract while preserving the existing serialized `snake_case` wire format. Validated by new preview serialization tests and `npm run build`.

### S2. `LaunchPreview.resolved_method` should use an enum, not `String`

**File:** `preview.rs:74`

The TypeScript side already defines this as `'steam_applaunch' | 'proton_run' | 'native'` -- stricter than the Rust side. A `ResolvedLaunchMethod` enum with three variants would make match arms exhaustive.

**Status (2026-03-27):** Implemented. Rust now uses a serialized `ResolvedLaunchMethod` enum, and the TypeScript preview contract mirrors that typed method shape. Validated by new preview serialization tests, Rust test suites, and `npm run build`.

### S3. Remove `Deserialize` from output-only types

**Files:** `preview.rs` (all preview types)

`LaunchPreview`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation`, `PreviewEnvVar` all derive `Deserialize` but are never deserialized from JS. Removing it communicates intent and prevents accidental deserialization bypassing the builder.

**Status (2026-03-27):** Implemented. Output-only preview types now derive `Serialize` only.

### S4. Consider removing `PreviewValidation.passed` field

**File:** `preview.rs:62`

`passed` is always `issues.is_empty()` at construction time. Redundant derived state that could drift. Consumers can derive it trivially: `issues.is_empty()` in Rust, `issues.length === 0` in TS.

**Status (2026-03-27):** Implemented. `PreviewValidation.passed` was removed, preview rendering now derives readiness from `issues.length === 0`, and `to_display_toml()` computes pass/fail from the issue list. Validated by updated Rust tests and `npm run build`.

### S5. `.ok()` discards error from `build_steam_launch_options_command` in preview

**File:** `preview.rs:272-276`

`.ok()` converts `Result<String, ValidationError>` to `Option<String>`, discarding the error. If building the steam launch options fails, the preview silently shows no launch options. Consider capturing the error or folding it into `directives_error`.

**Status (2026-03-27):** Implemented. Steam launch-options generation errors are now folded into `directives_error` instead of being discarded, and the preview returns partial results without silently hiding the failure. Validated by a new Rust regression test and the full Rust workspace test suite.

### S6. `.unwrap_or_else` silently falls back to `%command%` in `build_effective_command_string`

**File:** `preview.rs:447-449`

When optimization resolution fails for steam_applaunch, the effective command silently falls back to `%command%`. The user sees a clean command string with no indication that their optimizations didn't resolve. Validation issues will surface separately, but the command chain section is misleading.

**Status (2026-03-27):** Implemented. Steam preview command generation no longer falls back to a fake `%command%` success path when command construction fails; the preview now surfaces the concrete error and leaves the command fields empty. Validated by a new Rust regression test and the full Rust workspace test suite.

---

## Strengths

- **Partial-result architecture is excellent.** `Option<T>` for directive-dependent sections while always returning validation and game info is thoughtful UX design. Well-documented at `preview.rs:66-70` and well-tested at line 751-783.
- **Exhaustive validation.** `validate_all()` and `collect_*_issues()` correctly mirror the existing `validate()` dispatch but continue collecting instead of short-circuiting. Tests verify multi-issue collection.
- **Accessibility.** All three modals use `role="dialog"`, `aria-modal`, `aria-labelledby`, focus trapping with Tab/Shift+Tab wrap, Escape to close, backdrop click, `inert` + `aria-hidden` on siblings, and focus restoration on close.
- **TypeScript/Rust type contract is exact.** All field names match across IPC, `Option<T>` maps to `T | null`, enum variants match string literal unions.
- **Test coverage is solid (8/10).** 18 new tests covering all three launch methods, validation pass/fail, partial results on directive failure, trainer staging, method-specific field visibility, launcher preview, and TOML roundtrip.
- **Clean Tauri command layer.** All four new commands follow the existing thin-wrapper pattern -- one-liners delegating to `crosshook-core`.
- **Shared clipboard utility.** Extracted from `SteamLaunchOptionsPanel` into `utils/clipboard.ts` with `execCommand` fallback for Tauri webview contexts.

---

## Test Coverage Assessment

**Rating: 8/10** -- Well-covered for a read-only preview feature.

**Coverage highlights:**

- All three launch methods tested (steam_applaunch, proton_run, native)
- Validation pass/fail with multi-issue collection
- Partial results on directive failure (standout test)
- Trainer copy-to-prefix staging path computation
- Method-specific field visibility (proton_setup, working_directory, steam_launch_options)
- TOML export roundtrip fidelity
- Launcher preview placement headers and validation failures

**Minor gaps (rating 5-6, not blocking):**

- `resolve_working_directory()` proton_run/native paths untested
- `build_proton_setup()` steam_applaunch path not asserted on
- `build_effective_command_string()` output format not directly tested
- `to_display_toml()` formatting not directly tested

---

## Notes

- Focus-trap code (~100 lines) is duplicated across three modals. The PR notes this matches the existing codebase pattern (no shared modal primitive exists). A future `useModalFocusTrap` hook would reduce this.
- `chrono = "0.4"` added for `Utc::now().to_rfc3339()`. Rust's `std::time::SystemTime` has no built-in RFC 3339 formatter; `chrono` is the standard solution.
- `build_launch_preview()` always returns `Ok(...)` -- all failures are captured as partial results. The `Result` wrapper is a forward-compatible API surface.
- No frontend test framework exists in this project.
