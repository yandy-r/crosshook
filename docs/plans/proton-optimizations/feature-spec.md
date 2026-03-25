# Feature Spec: Proton Launch Optimizations

## Executive Summary

This feature adds a `Launch Optimizations` panel so users can enable common Proton tweaks with readable labels instead of raw variable names. CrossHook will persist a typed `launch.optimizations` profile subsection, autosave only that section for already-saved profiles, and translate selected options into env vars or wrapper commands in the backend. The required scope is `proton_run`; Steam parity is optional future work. The main risks are autosave boundaries for unsaved profiles, wrapper ordering, and keeping advanced or fork-specific options clearly labeled.

## External Dependencies

### APIs and Services

#### Proton Runtime Environment

- **Documentation**: [ValveSoftware/Proton](https://github.com/ValveSoftware/Proton)
- **Authentication**: None
- **Key Capabilities**:
  - Per-launch environment variables prepended before `%command%`
  - Global `user_settings.py` for persistent defaults outside CrossHook
- **Rate Limits**: None
- **Pricing**: None
- **Implementation Relevance**: This is the authoritative model for env-based Proton launch tuning such as HDR, Wayland, and input flags.

#### Steam Launch-Option Semantics

- **Documentation**: [Valve Developer Community: Command line options (Steam)](https://developer.valvesoftware.com/wiki/Command_line_options_%28Steam%29)
- **Authentication**: None
- **Key Capabilities**:
  - `%command%` placeholder expansion
  - Ordered prefix model for env vars, wrappers, and game arguments
- **Rate Limits**: None
- **Pricing**: None
- **Implementation Relevance**: CrossHook must respect single-chain `%command%` semantics when modeling wrappers like MangoHud or GameMode.

#### MangoHud

- **Documentation**: [flightlessmango/MangoHud](https://github.com/flightlessmango/MangoHud)
- **Authentication**: None
- **Key Capabilities**:
  - `mangohud %command%`
  - `MANGOHUD=1` for Vulkan-only flows
  - optional OpenGL caveats such as `MANGOHUD_DLSYM`
- **Rate Limits**: None
- **Pricing**: None
- **Implementation Relevance**: MangoHud should be modeled as a wrapper, not as a generic string field.

#### GameMode

- **Documentation**: [FeralInteractive/gamemode](https://github.com/FeralInteractive/gamemode)
- **Authentication**: None
- **Key Capabilities**:
  - `gamemoderun %command%`
  - host-side performance profile adjustments
- **Rate Limits**: None
- **Pricing**: None
- **Implementation Relevance**: This is the most portable “performance mode” wrapper CrossHook can expose.

#### CachyOS `game-performance`

- **Documentation**: [Gaming with CachyOS Guide](https://wiki.cachyos.org/configuration/gaming/) and [CachyOS `game-performance` script](https://github.com/CachyOS/CachyOS-Settings/blob/master/usr/bin/game-performance)
- **Authentication**: None
- **Key Capabilities**:
  - `game-performance %command%`
  - temporary `power-profiles-daemon` switch to `performance`
- **Rate Limits**: None
- **Pricing**: None
- **Implementation Relevance**: Useful for CachyOS users, but distro-specific and should remain advanced or conditional.

#### Community Proton Fork Documentation

- **Documentation**: [GE-Proton](https://github.com/GloriousEggroll/proton-ge-custom) and [CachyOS gaming guide](https://wiki.cachyos.org/configuration/gaming/)
- **Authentication**: None
- **Key Capabilities**:
  - fork-specific variables such as `PROTON_FSR4_UPGRADE`, `PROTON_DLSS_UPGRADE`, `PROTON_XESS_UPGRADE`, `PROTON_NVIDIA_LIBS`, and `SteamDeck=1`
- **Rate Limits**: None
- **Pricing**: None
- **Implementation Relevance**: These are valuable for power users, but they are not Valve-official Proton surface area and must be labeled accordingly.

### Libraries and SDKs

| Library / Tool     | Current Usage      | Purpose                                                    | Notes                                                         |
| ------------------ | ------------------ | ---------------------------------------------------------- | ------------------------------------------------------------- |
| `@tauri-apps/api`  | already in repo    | invoke typed backend save and launch commands              | Existing IPC surface can carry typed optimization data        |
| React 18           | already in repo    | render grouped checkboxes, status, and advanced disclosure | No new UI framework needed                                    |
| Rust `serde`       | already in repo    | persist optimization IDs in TOML and launch requests       | Existing profile and request types already use it             |
| MangoHud           | optional host tool | overlay wrapper                                            | detect presence before launch or surface missing-tool warning |
| GameMode           | optional host tool | portable performance wrapper                               | preferable over distro-specific wrappers in v1                |
| `game-performance` | optional host tool | CachyOS-specific performance wrapper                       | advanced/conditional only                                     |

### External Documentation

- [ValveSoftware/Proton](https://github.com/ValveSoftware/Proton): authoritative Proton runtime model.
- [GE-Proton](https://github.com/GloriousEggroll/proton-ge-custom): documents additional community/fork options including `SteamDeck=1`, FSR4, DLSS, and Wayland aliases.
- [Gaming with CachyOS Guide](https://wiki.cachyos.org/configuration/gaming/): practical documentation for current Linux gaming launch patterns and caveats.
- [MangoHud README](https://github.com/flightlessmango/MangoHud): wrapper usage, OpenGL notes, and config behavior.
- [GameMode README](https://github.com/FeralInteractive/gamemode): wrapper usage and runtime behavior.

## Business Requirements

### User Stories

**Primary User: Linux or Steam Deck player using Proton-backed profiles**

- As a user, I want to enable common Proton and game-launch fixes with readable labels so I do not need to memorize environment variable names.
- As a user, I want each profile to remember its own launch optimizations so I do not have to re-enter them before every session.
- As a user, I want common options such as controller fixes, overlays, and compatibility toggles to live near the Launch panel because they change launch behavior, not game identity.
- As a user, I want changes in this section to save automatically once the profile already exists so small checkbox edits do not require a full manual Save flow.

**Secondary User: Power user troubleshooting difficult titles**

- As a user, I want advanced options such as HDR, Wayland, NTSync, upscaler upgrades, and Steam Deck compatibility mode to be available behind warnings and grouping instead of buried in external docs.
- As a user, I want CrossHook to distinguish common options from community-only or hardware-specific options so I do not mistake them for universal fixes.
- As a user, I want wrapper-style options to stay structured and previewable rather than becoming a free-form shell editor.

### Business Rules

1. **Profile-scoped persistence**: Launch optimizations belong to the selected profile and must persist with that profile’s TOML document.
2. **Human-first presentation**: Primary labels must describe user-visible effect, not raw env var names.
3. **Per-option explanation**: Every option must include an info (`i`) affordance that explains what the option does, when it helps, and whether it is advanced, experimental, or community-documented.
4. **Curated catalog**: CrossHook should expose a fixed, reviewed set of options rather than arbitrary strings or arbitrary env key/value editing.
5. **Autosave boundary**: Automatic persistence is allowed only for profiles that already exist on disk. New unsaved profiles stage optimization selections in memory until the user performs the first regular Save.
6. **Section-only autosave**: Toggling launch optimizations must not silently persist unrelated dirty edits from the rest of the profile form.
7. **Runner applicability**: `native` profiles must not present Proton-only options as actionable. `proton_run` is the required supported path for this feature. `steam_applaunch` support is optional future work and must not complicate Phase 1.
8. **Advanced labeling**: Community-only, vendor-specific, compositor-specific, or experimental options must be labeled clearly and grouped behind progressive disclosure.
9. **Reset isolation**: Resetting launch session state must not erase saved optimization preferences.
10. **Install-review consistency**: In install review, optimization edits stay draft-only until the reviewed profile is explicitly saved.

### Edge Cases

| Scenario                                    | Expected Behavior                                                                                                   | Notes                                             |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| User is editing a brand-new unsaved profile | Optimization toggles work locally but show `Save profile first to enable autosave`                                  | avoids surprise profile creation                  |
| User switches from `proton_run` to `native` | Proton options become inactive in UI but remain stored unless explicitly cleared                                    | prevents destructive toggling when switching back |
| Wrapper binary is missing                   | Toggle is disabled or launch validation fails with a clear message                                                  | no silent ignore                                  |
| Autosave fails                              | Show inline `Failed to save` status and preserve in-memory selection                                                | user can retry                                    |
| User enters install-review modal            | Optimization edits are allowed only if explicitly scoped as draft-only; otherwise hide the section until final save | preserves current review-first contract           |
| User enables HDR without proper stack       | Show prerequisite warning about compositor/runtime requirements                                                     | prevents false confidence                         |
| User enables multiple performance wrappers  | Enforce mutual exclusion or show a conflict warning                                                                 | avoid confusing wrapper stacking                  |

### Success Criteria

- [ ] Users can enable a curated set of launch optimizations without typing raw Proton or Steam launch strings.
- [ ] Optimization selections persist per profile and reload with the profile.
- [ ] Existing saved profiles apply optimization changes automatically without requiring the main Save button.
- [ ] New unsaved profiles do not get silently created just because an optimization toggle changed.
- [ ] Native launch profiles do not present Proton-only options as available.
- [ ] Every visible option has an info tooltip that explains effect, typical use case, and caveats.
- [ ] `proton_run` launches can translate selected options into deterministic env vars and wrappers.
- [ ] Advanced or community-only options are visually distinct from recommended common options.
- [ ] The UI communicates autosave status, unsupported states, and prerequisite warnings clearly.

## Technical Specifications

### Architecture Overview

```text
[ProfileEditorView] ------------------------------+
        |                                         |
        v                                         v
[GameProfile.launch.optimizations]       [LaunchOptimizationsPanel]
        |                                         |
        | autosave existing profiles only         | option catalog + status + preview
        v                                         |
[profile_save_launch_optimizations]               |
        |                                         |
        v                                         |
[TOML ProfileStore merge/save]                    |
        |                                         |
        +-----------------------------+-----------+
                                      |
                                      v
                             [App.tsx launchRequest]
                                      |
                                      v
                        [crosshook-core::launch::optimizations]
                                      |
                                      v
                               [proton_run command]
                         full env + wrapper support in Phase 1
```

Recommended technical approach:

- Add a typed optimization subsection under `launch`.
- Persist stable option IDs only, not raw env var strings or raw shell fragments.
- Introduce a lightweight backend save path for only the optimization subtree.
- Resolve env vars and wrappers in Rust during launch construction.
- Treat `proton_run` as the only required execution path for this feature.

### Data Models

#### `LaunchOptimizationId`

Stable IDs should be backend-owned and frontend-consumed. Suggested initial IDs:

| ID                          | User-facing label                 | Execution type | Applies to                     |
| --------------------------- | --------------------------------- | -------------- | ------------------------------ |
| `disable_steam_input`       | Disable Steam Input               | env            | `proton_run`                   |
| `prefer_sdl_input`          | Prefer SDL controller handling    | env            | `proton_run`                   |
| `hide_window_decorations`   | Hide window decorations           | env            | `proton_run`                   |
| `show_mangohud_overlay`     | Show MangoHud overlay             | wrapper        | `proton_run`                   |
| `use_gamemode`              | Use GameMode                      | wrapper        | `proton_run`                   |
| `use_game_performance`      | Use CachyOS performance wrapper   | wrapper        | `proton_run`                   |
| `enable_hdr`                | Enable HDR                        | env            | advanced                       |
| `enable_wayland_driver`     | Use native Wayland support        | env            | advanced                       |
| `use_ntsync`                | Use NTSync                        | env            | advanced                       |
| `enable_local_shader_cache` | Isolate shader cache per game     | env            | advanced                       |
| `enable_fsr4_upgrade`       | Auto-upgrade FSR4                 | env            | advanced, community            |
| `enable_fsr4_rdna3_upgrade` | Use RDNA3-optimized FSR4          | env            | advanced, AMD                  |
| `enable_xess_upgrade`       | Auto-upgrade XeSS                 | env            | advanced, vendor-specific      |
| `enable_dlss_upgrade`       | Auto-upgrade DLSS                 | env            | advanced, NVIDIA               |
| `show_dlss_indicator`       | Show DLSS indicator               | env            | advanced, NVIDIA               |
| `enable_nvidia_libs`        | Enable NVIDIA game libraries      | env            | advanced, NVIDIA               |
| `steamdeck_compat_mode`     | Use Steam Deck compatibility mode | env            | advanced, community workaround |

#### Profile Persistence Shape

TypeScript:

```ts
export interface LaunchOptimizations {
  enabled_option_ids: string[];
}

export interface GameProfile {
  // existing fields...
  launch: {
    method: LaunchMethod;
    optimizations: LaunchOptimizations;
  };
}
```

Rust:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchOptimizationsSection {
    #[serde(rename = "enabled_option_ids", default, skip_serializing_if = "Vec::is_empty")]
    pub enabled_option_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchSection {
    #[serde(default)]
    pub method: String,
    #[serde(default, skip_serializing_if = "LaunchOptimizationsSection::is_empty")]
    pub optimizations: LaunchOptimizationsSection,
}
```

Example TOML:

```toml
[launch]
method = "proton_run"

[launch.optimizations]
enabled_option_ids = [
  "disable_steam_input",
  "show_mangohud_overlay",
  "use_gamemode",
]
```

#### Autosave Command Input

| Field                | Type       | Constraints                     | Description                      |
| -------------------- | ---------- | ------------------------------- | -------------------------------- |
| `name`               | `string`   | existing profile only           | profile identifier               |
| `enabled_option_ids` | `string[]` | sorted, deduped, known IDs only | persisted optimization selection |

### API Design

#### `profile_save_launch_optimizations`

**Purpose**: Persist only the optimization subtree for an existing profile.

**Authentication**: local Tauri IPC only

**Request:**

```json
{
  "name": "The Witcher 3 (Proton)",
  "data": {
    "enabled_option_ids": ["disable_steam_input", "show_mangohud_overlay"]
  }
}
```

**Response (200):**

```json
{
  "succeeded": true
}
```

**Errors:**

| Status            | Condition                            | Response                                 |
| ----------------- | ------------------------------------ | ---------------------------------------- |
| client validation | profile has not been saved yet       | show deferred autosave state             |
| IPC error         | profile file missing or write failed | inline save error                        |
| validation        | unknown option ID                    | reject save and keep in-memory selection |

Design notes:

- This command should load the existing profile, replace only `launch.optimizations`, and save the merged profile back to disk.
- It avoids persisting unrelated dirty fields from the rest of the profile editor.
- It avoids the heavy `refreshProfiles()` + `loadProfile()` cycle currently used for explicit Save.

#### Launch Request Extension

`LaunchRequest` should include `enabled_option_ids` so launch-time behavior stays fully typed:

```ts
optimizations: {
  enabled_option_ids: string[];
}
```

#### Backend Launch Resolver

Add a new resolver in `crosshook-core/src/launch/optimizations.rs`:

```rust
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}

pub fn resolve_launch_directives(request: &LaunchRequest) -> Result<LaunchDirectives, ValidationError>;
```

Responsibilities:

- validate all IDs
- apply method gating
- map IDs to deterministic env vars and wrappers
- reject conflicting wrapper combinations
- surface warnings or hard errors for unsupported states

### System Integration

#### Files to Create

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`: right-column card with grouped toggles, autosave state, warnings, and launch preview.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/launch-optimizations.ts`: shared option catalog and option metadata.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`: backend mapping from IDs to env vars and wrappers for direct Proton launches.

#### Files to Modify

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx`: include optimization data in `launchRequest` and stack the new card under `LaunchPanel`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/LaunchPanel.tsx`: optionally host a compact summary or compose with the new panel.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts`: normalize optimization state and add lightweight autosave path.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts`: extend `GameProfile.launch`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/launch.ts`: extend `LaunchRequest`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx`: pass autosave and profile state through if the optimization panel needs editor-level coordination.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css`: card spacing, advanced disclosure, preview, warning badges, and stacked mobile layout.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: persist optimization IDs in TOML.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: add round-trip tests.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: extend request types and validation.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: wrapper-aware command construction for `proton_run`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs`: add the section-specific save command.

#### Configuration

- No new frontend dependency is required.
- No new bundled runtime helper is required for v1 because the required scope is `proton_run`.
- Any Steam work is explicitly optional follow-up scope.

## UX Considerations

### User Workflows

#### Primary Workflow: Saved Proton Profile

1. **Open profile**
   - User selects an existing `proton_run` profile.
   - System shows `LaunchPanel`, then `Launch Optimizations`, then `LauncherExport`.

2. **Toggle optimizations**
   - User enables items such as `Disable Steam Input` or `Show MangoHud overlay`.
   - System updates a small summary like `2 optimizations enabled`.

3. **Autosave feedback**
   - User sees `Saving...` followed by `Saved automatically`.
   - System writes only `launch.optimizations` to disk.

4. **Launch preview**
   - User sees a generated preview such as `PROTON_NO_STEAMINPUT=1 mangohud gamemoderun <proton run game.exe>`.
   - System uses the same ordered directive plan during launch.

#### Draft Workflow: New Unsaved Profile

1. User edits a new profile that has never been saved.
2. Optimization card appears once method context is Proton-capable.
3. Toggles update local state, but a notice explains that autosave starts only after the first manual profile save.

#### Error Recovery Workflow

1. User enables an option with unmet prerequisites or missing host tool.
2. System shows a clear unsupported or warning message near the affected control.
3. User can disable the option or correct the dependency without losing unrelated selections.

### UI Patterns

| Component        | Pattern                                          | Notes                                                               |
| ---------------- | ------------------------------------------------ | ------------------------------------------------------------------- |
| Card shell       | stacked secondary card under `LaunchPanel`       | uses the open space shown in the current right column               |
| Groups           | `fieldset` + `legend` style sections             | `Input`, `Performance & Overlay`, `Graphics & HDR`, `Compatibility` |
| Labels           | effect-first checkbox labels with info tooltip   | env var or wrapper shown as secondary detail only                   |
| Advanced section | `<details>`/`<summary>` or equivalent disclosure | hides risky, community-only, or vendor-specific items by default    |
| Preview          | read-only command summary                        | explains wrapper ordering without exposing arbitrary editing        |
| Status row       | small summary + save state                       | `3 enabled`, `Saving...`, `Saved automatically`, `Failed to save`   |

### Accessibility Requirements

- Use native checkboxes and labels, not div-based pseudo-controls.
- Group related options semantically so screen readers announce section context.
- Keep visible label text and accessible name aligned.
- Provide a keyboard-focusable info (`i`) control for each option with a tooltip or popover description; the content must be available to keyboard and screen-reader users, not hover-only.
- Ensure the right-column stack collapses to one column on smaller windows.
- Preserve keyboard and controller traversal order.
- Use `aria-describedby` for warnings such as `Experimental`, `Requires HDR-capable output`, or `Community-documented`.

### Performance UX

- **Loading States**: not required for static catalog load, but wrapper/tool detection may briefly show `Checking availability...`.
- **Optimistic Updates**: yes for checkbox state, paired with debounced autosave status.
- **Error Feedback**: inline and local to the card, not global page banners.
- **Help Feedback**: each tooltip should explain what the option does, when a user would typically enable it, and the main caveat or dependency.

## Recommendations

### Implementation Approach

**Recommended Strategy**: ship a structured, profile-scoped optimization panel with section-only autosave and backend launch translation for `proton_run`.

**Phasing:**

1. **Phase 1 - Core Proton Support**: add typed persistence, right-column panel, autosave for existing profiles, and a conservative v1 option set for `proton_run`.
2. **Phase 2 - Advanced Options**: add HDR, Wayland, NTSync, community/fork-specific upscaler upgrades, vendor-gated options, and richer warnings.
3. **Phase 3 - Optional Future Steam Integration**: only if later required, add a real Steam launch-options management layer for `steam_applaunch`.

### Technology Decisions

| Decision           | Recommendation                        | Rationale                                                           |
| ------------------ | ------------------------------------- | ------------------------------------------------------------------- |
| Saved shape        | store stable option IDs               | avoids raw env/string persistence and keeps labels evolvable        |
| Autosave           | section-specific backend save command | prevents unrelated dirty profile data from being silently persisted |
| Launch translation | Rust backend                          | keeps quoting, ordering, and validation in one place                |
| UI placement       | stacked under `LaunchPanel`           | matches user expectation and current layout space                   |
| Default scope      | `proton_run` only                     | matches the clarified requirement and current architecture          |
| Steam strategy     | future enhancement only               | not required for this feature                                       |

### Quick Wins

- Ship `Disable Steam Input`, `Prefer SDL controller handling`, `Hide window decorations`, `Show MangoHud overlay`, and `Use GameMode` first.
- Add a launch preview and autosave status even in the initial release.
- Keep `SteamDeck=1`, `game-performance`, HDR, Wayland, NTSync, and upscaler upgrades under `Advanced`.

### Future Enhancements

- Hardware-aware gating for NVIDIA, AMD, and Intel-specific options.
- Wrapper availability detection before launch.
- Optional curated presets such as `Input fixes`, `Overlay`, or `Advanced graphics`.
- Optional Steam launch-option restore workflow for `steam_applaunch`.

## Risk Assessment

### Technical Risks

| Risk                                               | Likelihood | Impact | Mitigation                                                          |
| -------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------- |
| Autosave silently persists unrelated profile edits | Medium     | High   | add section-only backend save command                               |
| Wrapper ordering bugs cause broken launches        | Medium     | High   | centralize wrapper resolution in Rust and test command construction |
| Community-only options look official               | High       | Medium | label source/tier clearly and group in advanced disclosure          |
| Missing wrapper binaries cause confusing failure   | Medium     | Medium | preflight validation and disabled states                            |
| Overloaded UI becomes a troubleshooting wiki       | High       | Medium | small v1 set plus advanced disclosure                               |

### Integration Challenges

- `env_clear()` behavior means every supported env-based option must be explicitly reintroduced.
- Existing `useProfile.persistProfileDraft()` is too heavy for checkbox autosave and reloads the full profile.

### Security Considerations

- Do not introduce arbitrary shell strings or arbitrary env editing in v1.
- Keep all wrapper and env mappings in a backend-owned catalog.
- Validate option IDs and reject unknown values before launch.

## Decisions Needed

1. Should `SteamDeck=1` ship in the first visible advanced catalog, or stay hidden until a later phase because it is a community workaround rather than a general optimization?
2. Should `game-performance` appear beside `GameMode` in v1 for CachyOS users, or wait until CrossHook can detect and gate distro-specific wrappers cleanly?
3. Should unsupported-but-saved options be retained when users switch runner method, or should CrossHook clear them automatically on save?

## Task Breakdown Preview

### Phase 1: Typed Persistence And `proton_run` Launch Translation

**Focus**: establish the persisted model, the UI panel, and working backend translation for direct Proton launches.
**Tasks**:

- add `launch.optimizations.enabled_option_ids` to TypeScript and Rust profile models
- add section-only backend save command for existing profiles
- add `LaunchOptimizationsPanel` and integrate it under `LaunchPanel`
- extend `LaunchRequest` and implement Rust launch-directive resolution
- support the conservative v1 option set and autosave status UI

**Parallelization**: frontend panel and backend persistence/model work can proceed in parallel after the catalog shape is fixed.

### Phase 2: Advanced Options And Validation

**Focus**: add gated advanced options, warnings, and wrapper/tool validation.
**Dependencies**: Phase 1 typed persistence and backend resolver.
**Tasks**:

- add advanced catalog entries and warning metadata
- add host-tool detection and conflict handling
- add Rust tests for wrapper ordering, validation, and TOML round-trips
- refine UI grouping, badges, and preview formatting

**Parallelization**: option catalog/warning UX and backend validation/tests can run in parallel.

### Phase 3: Optional Future Steam Launch-Option Management

**Focus**: only if required later, evaluate whether and how CrossHook should apply equivalent behavior to `steam_applaunch`.
**Dependencies**: explicit future product requirement for Steam parity.
**Tasks**:

- research Steam local config ownership and safe restore behavior
- implement just-in-time launch-option write/restore if approved
- add failure recovery for interrupted sessions
- align UI messaging so users understand Steam support level

**Parallelization**: limited, because the restore model and failure handling define the architecture.

## Research References

- [research-external.md](./research-external.md): external runtime, wrapper, and documentation findings.
- [research-business.md](./research-business.md): user stories, business rules, and autosave boundary reasoning.
- [research-technical.md](./research-technical.md): codebase fit, launch architecture, and Steam support constraints.
- [research-ux.md](./research-ux.md): right-column UX placement, grouping, accessibility, and launch preview guidance.
- [research-recommendations.md](./research-recommendations.md): phased recommendation set and option catalog prioritization.
