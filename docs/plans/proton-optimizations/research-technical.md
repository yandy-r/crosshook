# Proton Optimizations Technical Research

## Executive Summary

CrossHook can support user-friendly Proton optimization toggles by adding a typed optimization section to profile data, surfacing those toggles in a dedicated UI panel, and resolving the saved option IDs to environment variables and wrapper commands in Rust at launch time. The cleanest shape is to persist stable, human-owned option identifiers such as `steamdeck_mode` or `mangohud_overlay`, not raw shell fragments or raw environment variable names, then let the backend translate those IDs into launch directives for `proton_run` and, where feasible, `steam_applaunch`.

The current codebase already has the right structural seams: `GameProfile` and `LaunchRequest` are typed in both TypeScript and Rust, profile persistence is TOML-backed through `profile_save`, and all actual process spawning passes through `crosshook-core/src/launch/`. The main technical work is not storage but execution semantics and autosave behavior. `useProfile.persistProfileDraft()` currently performs a full save-refresh-reload loop, which is too heavy for per-toggle autosave, and the Steam launch path currently shells out to `steam -applaunch` without any mechanism equivalent to Steam’s `%command%` launch-option field. That Steam limitation needs to be treated as a first-class product and architecture decision rather than hidden behind a generic “launch options” abstraction.

### Architecture Approach

- Add a new persisted profile subsection under `launch`, for example:

```toml
[launch]
method = "proton_run"

[launch.optimizations]
enabled_option_ids = ["disable_steam_input", "prefer_sdl_input", "steamdeck_mode", "mangohud_overlay"]
```

- Keep the persisted shape minimal and stable:
  - Persist only stable option IDs.
  - Do not persist human labels, descriptions, categories, env var names, or wrapper commands.
  - Do not persist arbitrary freeform commands or arbitrary key/value env overrides in v1.

- Add a static option catalog in the frontend for presentation:
  - Each entry should include `id`, `label`, `description`, `category`, `supportedMethods`, and optional dependency/conflict metadata.
  - Example categories: `Input`, `Upscaling`, `Graphics`, `Compatibility`, `Overlay and Tools`.
  - The UI should never expose `PROTON_*` names as primary labels, but may reveal them in secondary help text.

- Add a matching backend resolver in Rust:
  - Introduce a `launch/optimizations.rs` module that maps stable IDs to typed launch directives.
  - Recommended internal model:

```rust
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}
```

- The resolver should accept the selected launch method and enabled option IDs, and return either directives or a validation error.
- Backend validation should reject unknown IDs and method-incompatible IDs instead of silently ignoring them.

- Extend the launch request path so the backend owns final translation:
  - `App.tsx` currently builds `launchRequest` directly from the selected profile.
  - Extend `LaunchRequest` in TypeScript and Rust with an `optimizations` subsection:

```ts
optimizations: {
  enabled_option_ids: string[];
}
```

- Avoid sending resolved env vars from React. The frontend should send only the saved IDs, and Rust should compute actual env/wrapper behavior.

- Direct Proton path (`proton_run`) is straightforward:
  - `build_proton_game_command()` and `build_proton_trainer_command()` already construct a `tokio::process::Command`.
  - Insert wrapper resolution before the Proton executable when options require host-side wrappers such as `mangohud` or `game-performance`.
  - Apply resolved environment variables after `env_clear()` and after host/runtime Proton env is populated so the option set is explicit and deterministic.

- Steam path (`steam_applaunch`) is the hard constraint:
  - Current `build_helper_command()` launches `runtime-helpers/steam-launch-helper.sh`, which then invokes `steam -applaunch "$appid"`.
  - That helper does not currently transport any optimization data.
  - More importantly, Steam launch-option features like `ENV=1 mangohud %command%` are defined in Steam’s per-game launch options, not in CrossHook’s helper process. CrossHook invoking `steam -applaunch` from a separate process is not equivalent to editing the game’s launch-options field.
  - Therefore, the architecture should explicitly model two support levels:
    - `proton_run`: full support for env options and wrapper commands.
    - `steam_applaunch`: either experimental support behind a separate implementation path, or explicitly reduced support until CrossHook can manage Steam launch options reliably.

- If Steam support is required for v1, the implementation likely needs a new Steam launch-option manager:
  - Read the per-user Steam local config that stores launch options.
  - Save the previous launch-option string for the target app.
  - Write a generated `%command%` string for the selected profile immediately before launch.
  - Launch the app.
  - Restore the previous user launch options after CrossHook finishes or on next startup if the previous run crashed.
  - This is materially more invasive than the current `steam_applaunch` flow and should be scoped as separate work, not buried inside the generic optimization toggle feature.

- Autosave should be section-specific, not a reuse of the current full save flow:
  - `useProfile.persistProfileDraft()` currently performs `profile_save`, metadata sync, `refreshProfiles()`, and `loadProfile()`.
  - That behavior is appropriate for explicit Save, but not for debounced toggle autosave.
  - A new lightweight persistence path is needed, for example `persistProfileSilently(name, profile)` or `persistProfileSection(name, profile)`, which:
    - writes the profile via `profile_save`
    - updates local “last persisted” state
    - does not reload the profile from disk
    - does not refresh the profile list
    - does not rewrite recent-files metadata on every toggle

- Recommended autosave lifecycle:
  - Panel changes update in-memory profile state immediately.
  - A debounce timer of roughly 500–800 ms batches rapid checkbox toggles.
  - Autosave runs only when:
    - `profileName.trim()` is non-empty
    - `profile.game.executable_path.trim()` is non-empty
    - the current profile is already persisted or clearly identified as saveable
  - On pending profile switch or app unmount, flush or cancel the debounce explicitly.

- Recommended behavior for unsaved/new profiles:
  - Do not silently create a brand-new profile file merely because a user toggled an optimization.
  - Keep optimization edits in local state for unsaved profiles.
  - Show a small “Saved with the profile after first Save” or “Save the profile first to enable autosave” status.
  - This preserves the user’s expectation that the profile editor owns profile creation, while still allowing optimization choices to be staged.

### Data Model Implications

- TypeScript profile model changes in `src/types/profile.ts`:
  - Extend `GameProfile.launch` with a nested optimization section.
  - Suggested interfaces:

```ts
export interface LaunchOptimizations {
  enabled_option_ids: string[];
}

export interface GameProfile {
  // existing fields ...
  launch: {
    method: LaunchMethod;
    optimizations: LaunchOptimizations;
  };
}
```

- Rust profile model changes in `crates/crosshook-core/src/profile/models.rs`:
  - Extend `LaunchSection` with:

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

- Add `is_empty()` on the optimization subsection so old profiles remain compact and backward-compatible in TOML.

- Update frontend profile normalization:
  - `createEmptyProfile()` should initialize `launch.optimizations.enabled_option_ids` to `[]`.
  - `normalizeProfileForEdit()` should populate missing optimization data from defaults so older profiles load cleanly.
  - `normalizeProfileForSave()` should sort and deduplicate IDs to keep TOML stable.

- Extend `LaunchRequest` in TypeScript (`src/types/launch.ts`) and Rust (`launch/request.rs`) with the same optimization section.

- Keep the request model ID-based rather than directive-based:
  - `LaunchRequest` should carry `enabled_option_ids`.
  - The process-spawning layer should resolve those IDs on demand.
  - This prevents the UI from becoming the authority on shell execution semantics.

- Consider explicit applicability metadata in the catalog but not in the saved profile:
  - Example: `supportedMethods: ["proton_run"]` or `supportedMethods: ["proton_run", "steam_applaunch"]`.
  - The saved profile can retain IDs even when the current method changes, but the UI should clearly mark inactive or unsupported options.
  - An alternative is to clear unsupported IDs automatically on launch-method change, but that is a product behavior decision and should be explicit.

### API Design Considerations

- Existing Tauri command surface can remain mostly intact:
  - `profile_save` already accepts the full `GameProfile`.
  - `launch_game` and `launch_trainer` already accept `LaunchRequest`.
  - No new top-level Tauri commands are strictly required just to persist and launch optimization data.

- New frontend persistence API is still recommended:
  - Add a dedicated hook-level method in `useProfile`, separate from explicit Save.
  - Suggested contract:

```ts
type PersistProfileAutosave = (
  name: string,
  profile: GameProfile
) => Promise<{ ok: true } | { ok: false; error: string }>;
```

- This method should call `invoke("profile_save", ...)` without the current expensive refresh/reload cycle.

- Launch validation should expand:
  - `validate_launch()` in Rust should validate selected option IDs.
  - It should also validate wrapper availability when a wrapper option is enabled, for example by confirming `mangohud` or `game-performance` is discoverable in `PATH`.
  - Errors should be returned before spawning the launcher process so the panel can show actionable failures.

- Process builder changes:
  - Add a shared function to resolve directives from the request:

```rust
pub fn resolve_launch_directives(request: &LaunchRequest) -> Result<LaunchDirectives, ValidationError>
```

- Add a helper that builds wrapped commands:
  - If no wrapper is selected, spawn Proton or the native executable directly.
  - If wrappers are selected, spawn the first wrapper executable and append the remaining wrapper chain and final command as arguments.
- Example target process shape for `proton_run`:

```text
mangohud game-performance /path/to/proton run /path/to/game.exe
```

- Steam helper API changes if Steam support is pursued:
  - `helper_arguments()` in `script_runner.rs` would need to pass serialized optimization IDs or resolved directives into `steam-launch-helper.sh`.
  - The shell scripts would need new CLI flags and validation logic.
  - If CrossHook adopts just-in-time Steam launch-option rewriting, that logic likely belongs in Rust rather than shell so file parsing, restore logic, and error handling stay testable.

- Do not introduce a generic “custom launch string” field in v1:
  - It would undermine type safety.
  - It would bypass method compatibility rules.
  - It would create shell injection and quoting complexity across Rust and bash.
  - It would weaken the whole point of a curated, profile-safe optimization model.

### System Constraints

- Steam execution constraint:
  - Steam’s `%command%` semantics live in the per-title launch-options field inside Steam, not in CrossHook’s current helper process.
  - CrossHook calling `steam -applaunch` from `steam-launch-helper.sh` does not, by itself, provide a place to prepend `mangohud` or per-game env assignments to the actual game executable.
  - This makes `steam_applaunch` support a materially different problem from `proton_run`.

- Environment isolation constraint:
  - CrossHook deliberately uses `env_clear()` and explicit rehydration in both Rust and shell helpers.
  - This is good for deterministic launches, but it means every supported env-based optimization must be explicitly allowed and set.
  - If an option is intended to affect both game and trainer phases, both paths need the same directive handling.

- Script parity constraint:
  - `WINE_ENV_VARS_TO_CLEAR` in `launch/env.rs` is explicitly kept in sync with shell helper `unset` blocks.
  - Any optimization-related environment additions for trainer launching may require updates in both Rust and shell scripts to preserve parity.

- Autosave race constraint:
  - Reusing the current `persistProfileDraft()` for autosave will likely cause UI churn because it reloads the profile after saving.
  - Rapid toggle changes could race with a concurrent manual edit in the left profile editor.
  - Autosave must avoid whole-profile reloads and must scope its “dirty” handling carefully.

- Profile identity constraint:
  - Autosave depends on profile identity.
  - `profileName` alone is not enough if the user is drafting a new profile that has never been saved.
  - The implementation needs an explicit rule for when a profile is considered autosave-eligible.

- Host dependency constraint:
  - Wrapper tools such as MangoHud or `game-performance` are host executables.
  - CrossHook cannot assume they are installed even on Linux gaming systems.
  - The feature should validate availability and present a clear unsupported state rather than silently dropping the wrapper.

- Screen and layout constraint:
  - The optimization panel is intended to sit near or below `LaunchPanel`, which already contains status, actions, and possibly `LauncherExport` below it.
  - The component needs compact grouping and collapsible advanced sections so it remains usable at Steam Deck widths.

- Security and correctness constraint:
  - Because the app is distributed as a native launcher, not a shell front-end, it should avoid evaluating arbitrary user command strings.
  - Curated IDs with explicit backend mapping keep launch behavior inspectable, testable, and recoverable.

### File-Level Impact Preview

- Likely files to modify:
  - `src/crosshook-native/src/types/profile.ts`
    - Add the persisted optimization section to `GameProfile.launch`.
  - `src/crosshook-native/src/types/launch.ts`
    - Add optimization IDs to `LaunchRequest`.
  - `src/crosshook-native/src/hooks/useProfile.ts`
    - Initialize defaults, normalize saved IDs, and add a lightweight autosave persistence path.
  - `src/crosshook-native/src/App.tsx`
    - Include optimization data when constructing `launchRequest`.
  - `src/crosshook-native/src/components/LaunchPanel.tsx`
    - Likely host or compose the new optimization section beside current status and action controls.
  - `src/crosshook-native/src/components/ProfileEditor.tsx`
    - If the optimization panel needs access to autosave status and profile state, this component may need to pass through new handlers.
  - `src/crosshook-native/src/styles/theme.css`
    - Add layout rules for a new right-column panel or stacked section below the launch card.
  - `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
    - Persist optimization IDs in TOML.
  - `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`
    - Extend tests to cover round-trip save/load of optimization data.
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
    - Extend request types and validation rules for selected IDs.
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
    - Resolve directives, apply env vars, and build wrapped commands for direct Proton launch.
  - `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`
    - Potentially add shared helpers for wrapper-aware command construction or explicit env application.
  - `src/crosshook-native/runtime-helpers/steam-launch-helper.sh`
    - Only if Steam support is implemented; otherwise it should remain unchanged and the UI should mark unsupported options.
  - `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`
    - Only if trainer-phase optimization directives need parity with the direct Proton trainer path.

- Likely files to create:
  - `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`
    - Dedicated UI for grouped toggles, help text, applicability state, and autosave feedback.
  - `src/crosshook-native/src/types/launch-optimizations.ts`
    - Frontend catalog types and curated option definitions.
  - `src/crosshook-native/src/hooks/useProfileAutosave.ts`
    - Optional debounce/autosave hook if the team wants to keep `useProfile.ts` smaller.
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
    - Backend option registry and ID-to-directive resolution.

- Test impact:
  - Rust unit tests should cover:
    - TOML round-trip of optimization IDs
    - request validation for unknown IDs
    - wrapper command construction order
    - environment application for direct Proton launch
    - method gating (`native` unsupported, `steam_applaunch` partial/experimental, `proton_run` supported)
  - Frontend tests are not currently configured, so behavior verification will likely need to rely on targeted Rust tests plus manual UI verification unless the repo adds a frontend test harness.
