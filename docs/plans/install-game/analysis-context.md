# Context Analysis: install-game

## Executive Summary

Install-game adds a new workflow, not a new product area: it lives inside the current profile editor, uses existing Proton discovery and TOML profile persistence, and ends by producing a normal `proton_run` `GameProfile`. The core architectural move is adding a dedicated `install` backend domain plus a thin Tauri command surface, while reusing and extracting the shared direct-Proton runtime helpers that already exist in the launch stack. The plan should be organized so backend contracts and runtime helper extraction land first, then the installer flow and UI wiring converge on a final review-and-save step with bounded executable discovery.

## Architecture Context

- **System Structure**: React components and hooks in `/src/crosshook-native/src/` drive Tauri IPC commands in `/src/crosshook-native/src-tauri/src/commands/`, which delegate to shared Rust domain modules in `/src/crosshook-native/crates/crosshook-core/src/`.
- **Data Flow**: The install form should gather input in the frontend, send a typed request through a new Tauri install command, run prefix provisioning and installer execution in shared Rust, then return a generated profile plus discovered executable candidates for review before final save.
- **Integration Points**: The feature plugs into `ProfileEditor.tsx`, `useProfile.ts`, Tauri command registration in `src-tauri/src/lib.rs`, and new `crosshook-core::install` code that reuses patterns from `launch`, `profile`, and `steam`.

## Critical Files Reference

- /src/crosshook-native/src/components/ProfileEditor.tsx: Install tab belongs here beside the existing profile editor.
- /src/crosshook-native/src/hooks/useProfile.ts: Existing profile hydration/save path the install review step must reuse.
- /src/crosshook-native/src/App.tsx: Current app-level state boundary and likely place for install result handoff.
- /src/crosshook-native/src/types/profile.ts: Generated profile shape must remain compatible with this contract.
- /src/crosshook-native/src-tauri/src/lib.rs: New install commands must be registered here.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Best existing pattern for async command results and log streaming.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Final profile persistence boundary pattern.
- /src/crosshook-native/src-tauri/src/commands/steam.rs: Existing Proton discovery command to reuse, not duplicate.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Runtime env and process-launch logic to extract and reuse.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Typed validation error pattern to mirror for install requests.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Final generated data must stay within this schema.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: Profile naming, save/list/load semantics, and persistence location.
- /src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: Existing Proton scan logic to use for selector data.

## Patterns to Follow

- **New Domain Instead of Extending Unrelated Modules**: Add `crosshook-core/src/install/` and `src-tauri/src/commands/install.rs` rather than embedding installer behavior in `launch.rs`.
- **Thin Tauri Commands**: Keep request validation, prefix logic, executable ranking, and profile generation in shared Rust.
- **Editable Detected Paths**: Keep the detected-Proton selector filling an editable field, matching the existing ProfileEditor behavior.
- **Typed Rust Validation Errors**: Install validation should mirror `launch/request.rs` with explicit variants and user-facing messages.
- **Inline Rust Unit Tests**: Prefix creation, executable ranking, and generated profile behavior should be tested close to the new install domain.

## Cross-Cutting Concerns

- Profile output must remain a standard `GameProfile`; the install flow cannot introduce a parallel durable profile concept.
- The product decision is direct `proton run` only in v1, so there should be no architectural branching around `umu-run`.
- Prefixes default under `~/.local/share/crosshook/prefixes/<slug>`, which means backend path resolution must be canonical and shared by UI preview and save flow.
- Executable discovery must be assistive, not authoritative; ranking logic and review UI must work together so wrong guesses do not silently persist.
- Gamepad-safe input handling and editable detected-path fields are already recorded lessons and must remain intact.

## Parallelization Opportunities

- Backend contract definition and frontend sub-tab shell can start in parallel once the install request/result shape is agreed.
- Runtime helper extraction from `script_runner.rs` can proceed independently from most UI work.
- Executable discovery/profile generation logic can be built in parallel with install form UI after the install result contract is stable.
- Documentation updates should wait until the storage-path and review flow are implemented, but they do not block core code work.

## Implementation Constraints

- **Technical constraints**:
  - No frontend test framework exists, so backend-heavy behavior should stay in Rust where unit tests are already standard.
  - Existing `launch::validate()` cannot be reused directly because install allows prefix creation and starts without a final runtime target.
  - Tauri command registration is centralized; missing registration in `src-tauri/src/lib.rs` would silently strand the feature.
- **Business constraints**:
  - No install drafts in v1.
  - Save occurs only from the final review step.
  - Default runtime strategy is direct `proton run`.
  - Prefix root is `~/.local/share/crosshook/prefixes/<slug>`.

## Key Recommendations

- Put the first implementation phase on backend contracts, runtime helper extraction, and Tauri command plumbing so UI work has a stable target.
- Treat executable discovery and final profile review as a distinct integration phase, not as a small tail on installer execution.
- Keep tasks scoped to 1-3 files so backend contract work, UI shell work, and runtime helper refactors can proceed in parallel without file conflicts.
- Preserve the existing profile editor as the source of truth for final save; the install flow should hand into it cleanly rather than inventing a second persistence path.
