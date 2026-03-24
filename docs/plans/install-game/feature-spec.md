# Feature Spec: Install Proton Games From Profile Panel

## Executive Summary

This feature adds an `Install Game` sub-tab inside the existing Profile panel so users can install a Proton-based Windows game and leave with a normal saved `proton_run` profile. It reduces setup friction for non-Steam games by combining Proton selection, prefix creation, installer launch, optional trainer capture, and profile generation in one flow. The recommended implementation is a dedicated install domain that reuses current Proton discovery, logging, and TOML profile persistence, and executes installers through direct `proton run` for a simpler support model. The main risks are final executable confirmation, prefix reuse clarity, and keeping executable auto-discovery aggressive enough to help without blocking profile review.

## External Dependencies

### APIs and Services

#### Proton Runtime

- **Documentation**: [ValveSoftware/Proton README](https://github.com/ValveSoftware/Proton)
- **Authentication**: None
- **Key Endpoints**: None; this is a local compatibility-tool runtime, not a network API.
- **Rate Limits**: None
- **Pricing**: None
- **Integration Notes**:
  - CrossHook already detects Proton installs by scanning Steam and system compatibility-tool roots.
  - Proton installs are directory-based compatibility tools that expose a `proton` executable.
  - CrossHook should keep persisting the executable path for runtime launch consistency.

#### umu-launcher / `umu-run`

- **Documentation**: [umu-launcher README](https://github.com/Open-Wine-Components/umu-launcher)
- **FAQ**: [umu-launcher FAQ](https://github.com/Open-Wine-Components/umu-launcher/wiki/Frequently-asked-questions-%28FAQ%29)
- **Authentication**: None
- **Key Endpoints**: None; local CLI/runtime wrapper only.
- **Rate Limits**: None
- **Pricing**: None
- **Integration Notes**:
  - Mirrors the reference script behavior by launching installers with `WINEPREFIX`, `GAMEID=0`, and `PROTONPATH`.
  - Better matches Steam-style container/runtime behavior than plain `proton run`.
  - Not selected for v1. Keep it as future compatibility headroom rather than part of the initial support surface.

#### Steam Linux Runtime / pressure-vessel

- **Documentation**: [Open Wine Components docs](https://www.openwinecomponents.org/)
- **Authentication**: None
- **Key Endpoints**: None; local runtime/container environment only.
- **Rate Limits**: None
- **Pricing**: None
- **Integration Notes**:
  - Relevant only if CrossHook uses `umu-run`.
  - Files outside `$HOME` may need explicit exposure through `STEAM_COMPAT_LIBRARY_PATHS` or related environment variables.
  - This matters for installer media on removable or mounted paths such as `/mnt/...`.

### Libraries and SDKs

| Library | Version | Purpose | Installation |
| --- | --- | --- | --- |
| Existing `tauri` stack | existing repo version | Typed frontend/backend command surface | already installed |
| Existing `@tauri-apps/plugin-dialog` | existing repo version | File and directory pickers for installer, Proton, prefix, trainer | already installed |
| Existing `tokio::process` usage | existing repo version | Installer process launch and log capture | already installed |
| Existing `directories` crate | existing repo version | Canonical XDG path resolution for default prefix path | already installed |
| Optional `umu-run` host binary | host-managed | Future compatibility fallback only, not part of v1 | optional host dependency |

### External Documentation

- [ValveSoftware/Proton README](https://github.com/ValveSoftware/Proton): Proton install layout and compatibility-tool expectations.
- [umu-launcher FAQ](https://github.com/Open-Wine-Components/umu-launcher/wiki/Frequently-asked-questions-%28FAQ%29): `PROTONPATH`, `GAMEID`, `PROTON_VERB`, and filesystem/container behavior.
- [GNOME HIG Tabs](https://developer.gnome.org/hig/patterns/nav/tabs.html): Guidance for shallow in-window secondary navigation.
- [GNOME HIG Progress Bars](https://developer.gnome.org/hig/patterns/feedback/progress-bars.html): Guidance for long-running feedback.
- [W3C Error Identification](https://www.w3.org/WAI/WCAG22/Understanding/error-identification): Form validation and error messaging baseline.

## Business Requirements

### User Stories

**Primary User: Linux user installing a non-Steam Windows game**

- As a user, I want to install a Proton game without leaving CrossHook for a shell script.
- As a user, I want to choose a detected Proton version instead of typing a path from scratch.
- As a user, I want CrossHook to suggest a default prefix location but still let me override it.
- As a user, I want to attach an optional trainer during setup so the generated profile is launch-ready.
- As a user, I want the generated profile to appear in the normal profile list and behave like any other saved CrossHook profile.

**Secondary User: Returning or advanced user**

- As a user, I want to reuse an existing prefix deliberately, not accidentally.
- As a user, I want to recover from a failed install attempt without re-entering all fields.
- As a user, I want CrossHook to distinguish the installer executable from the final game executable so I do not save a broken profile.

### Business Rules

1. **Proton-only scope**: The install sub-tab must not expose Steam app launch or native Linux runner choices.
   - Validation: the generated profile always uses `launch.method = "proton_run"`.
   - Exception: none in v1.

2. **Required runtime selection**: Proton selection is required before install can start.
   - Validation: selected/manual Proton path must exist and be executable.
   - Exception: none.

3. **Required installation media**: Installer media is required and must be a Windows `.exe`.
   - Validation: file exists, is a file, and path ends with `.exe`.
   - Exception: none in v1.

4. **Required prefix destination**: Prefix path is required and should default to `~/.local/share/crosshook/prefixes/<slug>`.
   - Validation: path is resolvable and creatable by CrossHook.
   - Exception: existing non-empty prefixes are allowed only with explicit confirmation.

5. **Trainer is optional**: Trainer selection must not block installation or profile generation.
   - Validation: if provided, path must exist and be a file.
   - Exception: none.

6. **Installer media is not the runtime target**: CrossHook must never silently save installer media as `game.executable_path`.
   - Validation: final profile cannot be marked complete unless the game executable is explicitly confirmed or reliably derived.
   - Exception: none.

7. **Generated profile stays standard**: Install output must be a standard persisted `GameProfile`, not a separate permanent profile type.
   - Validation: profile loads through existing `ProfileStore` and appears in the current list.
   - Exception: none; v1 does not persist install drafts.

### Edge Cases

| Scenario | Expected Behavior | Notes |
| --- | --- | --- |
| Selected prefix path does not exist | CrossHook creates it before install | Differs intentionally from existing launch validation |
| Selected prefix already contains content | Warn and require explicit reuse confirmation | Avoid silent reuse |
| Installer finishes but final game exe is unknown | Show the generated profile with discovered candidates and require final review before save | Prevent broken profile |
| Duplicate profile name | Require rename or overwrite confirmation | Existing profiles remain authoritative |
| Auto-discovery finds multiple plausible executables | Rank candidates, prefill the best guess, and require confirmation | Assistive, not authoritative |

### Success Criteria

- [ ] Users can launch a Proton game installer from the main-tab Profile panel without using a terminal.
- [ ] The UI defaults a prefix path and Proton selection in a way that reduces manual typing for common cases.
- [ ] The resulting saved profile appears in the existing profile list and is valid for normal `proton_run` launch flow.
- [ ] The flow prevents installer media from being saved as the final runtime executable.
- [ ] Failed install attempts preserve user-entered fields so retry is fast.
- [ ] Optional trainer selection works without making install a prerequisite-heavy workflow.

## Technical Specifications

### Architecture Overview

```text
[ProfileEditorView]
      |
      v
[Install Game Sub-Tab] ---> [useInstallGame hook]
      |                              |
      v                              v
[Tauri commands/install.rs] ---> [crosshook_core::install]
                                      |
                     +----------------+----------------+
                     |                                 |
                     v                                 v
              [Proton discovery reuse]         [Profile generation]
                     |                                 |
                     v                                 v
              [runtime spawn/logging] -------> [ProfileStore save/load]
```

**Recommended approach**: add a dedicated install domain instead of overloading the existing launch domain.

- Existing `launch::validate()` assumes the game executable is already known and that the prefix already exists.
- Installer execution has different lifecycle semantics: prefix provisioning, installer process spawn, optional post-install executable confirmation, and profile generation.
- Reuse should happen below the command boundary by extracting shared Proton environment and log setup helpers from [script_runner.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs).

### Data Models

#### `InstallGameRequest`

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `profile_name` | `String` | required; valid profile name | Saved profile identifier and default prefix slug source |
| `display_name` | `String` | optional | Friendly game name |
| `installer_path` | `String` | required; existing `.exe` file | Installer media path |
| `trainer_path` | `String` | optional; existing file if set | Optional trainer to persist into generated profile |
| `proton_path` | `String` | required; executable | Selected Proton runtime path |
| `prefix_path` | `String` | required; creatable | Target Wine prefix directory |
| `installed_game_executable_path` | `String` | optional in request, required before final success | Final runtime target if user already knows it |
| `discovered_game_executable_candidates` | `Vec<String>` | response-derived only | Ranked candidates found after install scan |

**Indexes:**

- None; request is transient.

**Relationships:**

- Produces a `GameProfile` and an `InstallGameResult`.

#### `InstallGameResult`

| Field | Type | Constraints | Description |
| --- | --- | --- | --- |
| `succeeded` | `bool` | required | Install command outcome |
| `message` | `String` | required | User-facing result summary |
| `helper_log_path` | `String` | required | Install log file for diagnostics |
| `profile_name` | `String` | required | Saved/generated profile identifier |
| `profile` | `GameProfile` | required | Generated profile snapshot |
| `needs_executable_confirmation` | `bool` | required | Indicates follow-up is still required |
| `discovered_game_executable_candidates` | `Vec<String>` | optional | Ranked likely game executables for review |

**Indexes:**

- None; response is transient.

**Relationships:**

- `profile` reuses the existing persisted `GameProfile` schema.

#### `GameProfile` reuse

| Field | Source | Value |
| --- | --- | --- |
| `game.name` | install flow | derived from explicit display/profile name |
| `game.executable_path` | install flow | confirmed final executable, never installer path |
| `trainer.path` | install flow | optional trainer path |
| `runtime.prefix_path` | install flow | selected/default prefix |
| `runtime.proton_path` | install flow | selected Proton executable |
| `runtime.working_directory` | install flow | optional derived executable parent |
| `launch.method` | fixed | `proton_run` |

### API Design

#### `install_default_prefix_path`

**Purpose**: Resolve the canonical default prefix path for the entered profile/game name.
**Authentication**: Not required

**Request:**

```json
{
  "profileName": "god-of-war-ragnarok"
}
```

**Response (200):**

```json
{
  "prefixPath": "/home/user/.local/share/crosshook/prefixes/god-of-war-ragnarok"
}
```

**Errors:**

| Status | Condition | Response |
| --- | --- | --- |
| 400 | invalid profile name | validation error string |

#### `validate_install_request`

**Purpose**: Validate installer flow inputs before running a long-lived process.
**Authentication**: Not required

**Request:**

```json
{
  "profile_name": "god-of-war-ragnarok",
  "installer_path": "/mnt/media/setup.exe",
  "trainer_path": "/mnt/trainers/gowr.exe",
  "proton_path": "/home/user/.steam/root/steamapps/common/Proton - Experimental/proton",
  "prefix_path": "/home/user/.local/share/crosshook/prefixes/god-of-war-ragnarok"
}
```

**Response (200):**

```json
{
  "valid": true
}
```

**Errors:**

| Status | Condition | Response |
| --- | --- | --- |
| 400 | missing or invalid installer/proton/prefix/profile fields | validation error string |

#### `install_game`

**Purpose**: Provision the prefix, execute the installer, generate a profile, and persist it when valid.
**Authentication**: Not required

**Request:**

```json
{
  "profile_name": "god-of-war-ragnarok",
  "display_name": "God of War Ragnarok",
  "installer_path": "/mnt/media/setup.exe",
  "trainer_path": "/mnt/trainers/gowr.exe",
  "proton_path": "/home/user/.steam/root/steamapps/common/Proton - Experimental/proton",
  "prefix_path": "/home/user/.local/share/crosshook/prefixes/god-of-war-ragnarok",
  "installed_game_executable_path": ""
}
```

**Response (200):**

```json
{
  "succeeded": true,
  "message": "Installer completed. Review the generated profile.",
  "helper_log_path": "/tmp/crosshook-logs/install-god-of-war-ragnarok-123.log",
  "profile_name": "god-of-war-ragnarok",
  "needs_executable_confirmation": true,
  "discovered_game_executable_candidates": [
    "C:\\\\GOWR\\\\GoWR.exe",
    "C:\\\\GOWR\\\\launcher.exe"
  ],
  "profile": {
    "game": {
      "name": "God of War Ragnarok",
      "executable_path": ""
    },
    "trainer": {
      "path": "/mnt/trainers/gowr.exe",
      "type": ""
    },
    "runtime": {
      "prefix_path": "/home/user/.local/share/crosshook/prefixes/god-of-war-ragnarok",
      "proton_path": "/home/user/.steam/root/steamapps/common/Proton - Experimental/proton",
      "working_directory": ""
    },
    "launch": {
      "method": "proton_run"
    }
  }
}
```

**Errors:**

| Status | Condition | Response |
| --- | --- | --- |
| 400 | invalid request | validation error string |
| 500 | install spawn or profile generation failed | install error string |

### System Integration

#### Files to Create

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/mod.rs`: Install domain exports.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs`: Request/result models and validation types.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs`: Prefix provisioning, installer execution, and profile generation orchestration.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/profile_generation.rs`: Converts install input/result into `GameProfile`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/install.rs`: Tauri command handlers.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx`: New install sub-tab UI.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts`: Install form state and command orchestration.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts`: Frontend types for install request/result.

#### Files to Modify

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx`: Add secondary sub-tab navigation and embed install panel.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts`: Add a way to hydrate/load a generated profile result cleanly.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx`: Load the generated profile and optionally route install-log events.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: Register install commands.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Re-export install module.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: Extract shared runtime environment and log helpers.

#### Configuration

- `prefix_root_default`: backend-resolved default path rooted at `~/.local/share/crosshook/prefixes`.

## UX Considerations

### User Workflows

#### Primary Workflow: Install And Generate Profile

1. **Switch To Install Sub-Tab**
   - User: opens the Profile panel and chooses `Install Game`.
   - System: shows a sibling sub-tab layout, not a modal or extra top-level tab.

2. **Enter Inputs**
   - User: selects installer `.exe`, chooses or confirms Proton, accepts or edits the prefix path, optionally selects a trainer, and enters a profile/game name.
   - System: immediately previews the resulting profile identity and default prefix path.

3. **Run Installer**
   - User: clicks `Install and Generate Profile`.
   - System: validates fields, provisions the prefix, launches the installer, and shows inline status with visible logs/details.

4. **Review Result**
   - User: confirms the final game executable if needed.
   - System: shows the generated profile with ranked executable candidates, then offers `Save Profile`, `Back To Install`, or `Reset For Another Install`.

5. **Success State**
   - User leaves the install flow with a normal saved profile in the existing list.
   - System surfaces a success summary and keeps the generated profile editable through the standard profile tab.

#### Error Recovery Workflow

1. **Validation Error**: required installer/Proton/prefix fields are invalid.
2. **User Sees**: inline field errors plus a top summary; focus moves to the first invalid field.
3. **Recovery**: correct the invalid field and retry without losing other input.

#### Partial Completion Workflow

1. **Installer Succeeds But Runtime Target Is Unknown**
   - User: returns from installer without a final executable selected.
   - System: marks the result as `Installed, review executable`, preloads ranked candidates, and routes the user to review the generated profile before final save.

### UI Patterns

| Component | Pattern | Notes |
| --- | --- | --- |
| Profile sub-navigation | secondary tab row / segmented control | One extra level only, inside `ProfileEditorView` |
| Proton picker | detected dropdown plus editable path field | Reuse current proven pattern |
| Prefix field | editable directory field with default note | Default path resolved in backend, displayed immediately |
| Optional trainer | visually secondary section | Label explicitly as optional |
| Install status | single inline status panel | Stage text plus expandable log/details |
| Completion state | summary card with next actions | Avoid silent handoff back to profile editor |

### Accessibility Requirements

- Every field must have one clear label and any helper/error copy must be textual and persistent until corrected.
- Validation must identify the exact field in error and, when possible, provide a corrective suggestion.
- The sub-tab switcher must expose correct tab semantics and keyboard navigation.
- Runtime state changes such as `Preparing`, `Running Installer`, `Generating Profile`, and `Failed` should be announced through a live region.
- New controls must respect the existing 42-44px target sizing and not interfere with current gamepad-safe typing behavior.

### Performance UX

- **Loading States**: use a delayed spinner for short validation/provisioning work and a more explicit status panel for long-running installer execution.
- **Optimistic Updates**: do not claim success before the installer has exited and the profile state is reviewable.
- **Error Feedback**: preserve all entered values and keep the most relevant diagnostic lines visible near the action area.

## Recommendations

### Implementation Approach

**Recommended Strategy**: Build a dedicated install orchestration path that produces a standard `proton_run` profile and embeds the user flow as a sibling sub-tab inside the existing Profile panel.

**Phasing:**

1. **Phase 1 - Foundation**: typed install models, default prefix resolver, shared Proton environment extraction, request validation.
2. **Phase 2 - Core Features**: backend install command, prefix creation, installer execution, generated profile result, initial UI integration.
3. **Phase 3 - Polish**: existing-prefix warnings, partial-success handoff, better diagnostics, optional runtime strategy toggle, targeted tests.

### Technology Decisions

| Decision | Recommendation | Rationale |
| --- | --- | --- |
| Install orchestration | separate install domain | Keeps launch request model coherent |
| Profile persistence | reuse existing `GameProfile` | Avoids migration and keeps output ordinary |
| Runtime strategy | direct `proton run` only in v1 | Simplest support model and closest to current codebase |
| Prefix default | backend-resolved `~/.local/share/crosshook/prefixes/<slug>` | Better XDG fit for large mutable prefix data |
| Draft persistence | no drafts in v1 | Keeps the flow and persistence model simpler |
| Final exe resolution | aggressive bounded discovery plus mandatory review | Helpful without stalling completion |

### Quick Wins

- Reuse the current detected-Proton dropdown behavior exactly instead of inventing a new picker interaction.
- Show the default prefix path as soon as the user enters a profile/game name.
- Reuse existing log file/event patterns so install diagnostics feel consistent with launch diagnostics.
- Route successful installs directly into the existing profile list/editor instead of inventing a separate profile management area.

### Future Enhancements

- Add prefix scanning heuristics that suggest likely installed game executables after installer exit.
- Persist optional install provenance metadata only if reinstall/debug workflows justify the schema expansion.
- Add recent installer/prefix history similar to existing recent game/trainer paths.
- Offer an advanced debug toggle for `UMU_LOG`/`PROTON_LOG`.
- Consider optional Steam shortcut/export integration for installed non-Steam games after the core flow stabilizes.

## Risk Assessment

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| Some installers behave differently under direct `proton run` than under Steam-like wrappers | Medium | Medium | Stay on one execution model in v1 and add `umu-run` only if real failures justify it |
| Installer path is mistakenly saved as game path | Medium | High | Require explicit runtime target confirmation when unknown |
| Auto-discovery promotes the wrong executable | Medium | Medium | Rank candidates conservatively, de-rank uninstallers/setup tools, and require review |
| Large prefixes consume space in the wrong XDG root | Low | Medium | Default to `~/.local/share/crosshook/prefixes` from the start |
| Existing launch helpers are tightly coupled and hard to reuse | Medium | Medium | Extract shared environment/log setup before adding install service |

### Integration Challenges

- The install flow must coexist cleanly with the current profile editor without turning the panel into nested-navigation clutter.
- The backend needs to create prefixes on demand, which intentionally differs from current `proton_run` validation logic.
- Log streaming should feel unified whether the user is installing or launching.

### Security Considerations

- Use direct process arguments, never shell-concatenated user input.
- Validate all file and directory inputs before spawn.
- Be explicit about any host path exposure required for containerized runtime access.
- Avoid persisting installer media paths into normal runtime profile fields.

## Decisions Needed

- Installer execution uses direct `proton run` in v1. `umu-run` is deferred unless real compatibility issues justify it later.
- V1 does not persist install drafts. The profile is shown for review at the end and saved only from that review step.
- Default prefix root is `~/.local/share/crosshook/prefixes/<slug>`.
- Executable discovery should be aggressive but bounded: rank likely candidates, prefill the best guess, and always require user review before final save.

## Research References

- [research-external.md](./research-external.md)
- [research-business.md](./research-business.md)
- [research-technical.md](./research-technical.md)
- [research-ux.md](./research-ux.md)
- [research-recommendations.md](./research-recommendations.md)
- Reference shell behavior: `/mnt/sdb/Games/game-install.sh`
- Reference Proton selector script being replaced: `/mnt/sdb/Games/proton-versions.sh`

## Task Breakdown Preview

### Phase 1: Install Foundation

**Focus**: Establish install-specific request/result types and reusable backend helpers.
**Tasks**:

- Create `crosshook_core::install` module and typed request/result models.
- Add backend default prefix path resolver.
- Extract shared Proton environment/log setup from current launch runtime code.
- Add request validation and install-specific error types.
**Parallelization**: backend model work and frontend type definitions can proceed in parallel once command shapes are agreed.

### Phase 2: Core Install Flow

**Focus**: Implement installer execution and profile generation.
**Dependencies**: Phase 1 backend contracts and helper extraction.
**Tasks**:

- Add Tauri install commands and register them.
- Implement prefix provisioning and installer process spawn.
- Generate `GameProfile` output and persist when valid.
- Handle partial-success state when executable confirmation is still needed.
**Parallelization**: Tauri command wiring and profile generation logic can proceed in parallel after request/result types land.

### Phase 3: UI Integration

**Focus**: Add the install sub-tab and user-facing state model.
**Dependencies**: install commands available.
**Tasks**:

- Add Profile-panel sub-tab navigation.
- Build install form and status panel.
- Route successful results into the existing profile editor/list flow.
- Add field-level validation, completion summary, and retry behavior.
**Parallelization**: sub-tab UI and hook/state management can be split if they own different files.

### Phase 4: Validation And Polish

**Focus**: Reduce failure ambiguity and harden the user experience.
**Dependencies**: core install flow end-to-end working.
**Tasks**:

- Add tests for path validation, default prefix derivation, and profile generation.
- Add existing-prefix warnings and duplicate-name handling.
- Implement bounded executable discovery and candidate ranking after installer exit.
- Improve diagnostics and accessibility details.
**Parallelization**: Rust tests, frontend UX polish, and error-copy refinement can run concurrently.
