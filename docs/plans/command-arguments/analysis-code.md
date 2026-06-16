# Command Arguments Code Analysis

## Executive Summary

Command arguments should be implemented as profile-scoped launch preferences stored in `GameProfile.launch`, converted into `LaunchRequest`, resolved in `crosshook-core`, and appended only to the game command path. The critical backend work is to add a dedicated command-argument catalog/resolver instead of extending launch optimizations, because these entries produce argv tokens rather than env vars or wrappers. The critical UI work is to add a single launch-subtab panel for curated argument toggles plus custom tokens, with autosave behavior aligned to the existing launch-section save queue.

Persisted data:

- TOML settings: selected curated command-argument IDs and custom argument tokens under `launch`.
- SQLite metadata: none required for the first implementation.
- Runtime-only: resolved argv list, Steam launch-options line, preview command text, validation issues.

## Existing Code Structure

- `docs/plans/command-arguments/shared.md`: Feature context and required file map. It establishes profile TOML persistence, method-gated curated entries, and no SQLite storage.
- `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`: `LaunchSection` owns profile launch preferences. Existing nested section pattern is `LaunchOptimizationsSection { enabled_option_ids }` with `serde(default)` and `skip_serializing_if`.
- `src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs`: `GameProfile::effective_profile_with()` merges collection defaults into launch fields. New command-argument fields need an explicit decision here: profile-only for first pass, or added to `CollectionDefaultsSection` only if collection-level defaults are in scope.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs`: Profile load/save normalizes launch presets and has `save_launch_optimizations()` as the narrow persistence pattern for catalog-backed launch fields.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`: `LaunchRequest` is the runtime/IPC DTO. It currently carries `optimizations`, `custom_env_vars`, `gamescope`, `mangohud`, hooks, and method flags. Add command-argument request data here so launch, preview, validation, and IPC stay in sync.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`: Central validation has strict `validate()` and aggregate `validate_all()` paths. Command-argument validation must be added to both.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs` and `error_text.rs`: Stable validation codes/messages live here. New variants need code, severity, message, and help text.
- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: Optimization catalog loader pattern. Mirror this with a dedicated command-argument catalog, or split catalog concerns into sibling modules if `catalog.rs` would grow too large.
- `src/crosshook-native/assets/default_optimization_catalog.toml`: Embedded curated catalog precedent. Add a separate `default_command_argument_catalog.toml` rather than mixing argv-producing entries into optimization TOML.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs`: Deterministic resolver pattern for known IDs, duplicates, conflicts, method applicability, and dependency checks. Reuse the shape, not the type, for argument resolution.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`: Builds Steam launch options as `env wrappers %command%`. This must append resolved command arguments after `%command%`.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`: Real direct Proton/umu game command builder. It currently appends only `normalized_game_path`; append resolved game arguments immediately after that `.arg()`.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs`: Trainer command builder. Do not append game command arguments here.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs`: `proton_run` can become `umu-run` depending on settings/PATH. Argument support should be transparent because `proton_game.rs` appends to the target executable invocation after choosing `program_path`.
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs`: Host gateway command construction. Do not bypass this; command arguments are target argv, not new host-tool invocations.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`: Preview orchestration resolves directives, env, wrappers, effective command, and Steam options. Add argument resolution here so preview and execution report the same tokens.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`: Human-readable effective command string. Append resolved arguments to Proton/umu game previews and Steam `%command%` output.
- `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`: Thin IPC commands for preview, validation, and Steam launch-options. Extend signatures/DTOs only; keep business logic in core.
- `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs`: Narrow launch-section save command precedent. Add a sibling save command for command arguments or a more general launch-section save only if it avoids clobber risk.
- `src/crosshook-native/src-tauri/src/commands/catalog.rs`: Existing `get_optimization_catalog()` pattern. Add `get_command_argument_catalog()` returning backend catalog entries.
- `src/crosshook-native/src-tauri/src/lib.rs`: Register any new Tauri commands with snake_case names.
- `src/crosshook-native/src/types/profile.ts`: Frontend `GameProfile.launch` type and normalized defaults. Add command-argument launch fields here.
- `src/crosshook-native/src/types/launch.ts`: Frontend `LaunchRequest` type. Add command-argument request fields matching Rust serde names.
- `src/crosshook-native/src/types/launch-optimizations.ts`: Catalog/ID/type pattern for frontend toggles. Add a separate `launch-command-arguments.ts`.
- `src/crosshook-native/src/utils/launch.ts`: `buildProfileLaunchRequest()` is the main profile-to-request bridge. Thread selected curated IDs and custom tokens here.
- `src/crosshook-native/src/utils/optimization-catalog.ts` and `src/crosshook-native/src/hooks/useLaunchOptimizationCatalog.ts`: Cached catalog fetch pattern to mirror.
- `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts`: Prop assembly for shared LaunchSubTabs surfaces. Add command-argument props and update handlers here.
- `src/crosshook-native/src/hooks/profile/profileNormalize.ts`: Backward-compatible normalization for loaded profiles. Add default empty command-argument section and sanitize arrays.
- `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts` and `useProfileLaunchAutosaveEffects.ts`: Serialized launch-section autosave queue. Add command-argument autosave into this existing queue to avoid write races.
- `src/crosshook-native/src/components/LaunchSubTabs.tsx`: One-page launch configuration host. Add the new panel as a tab or in-page section here.
- `src/crosshook-native/src/components/launch-subtabs/types.ts` and `useTabVisibility.ts`: Add tab ID/label and method visibility. Proton/Steam should show curated arguments; native can show custom-only only if implemented.
- `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`: Curated grouped toggle UX to reuse for predefined argument entries.
- `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`: Derived Steam copy/paste preview. It must pass command arguments into the backend builder.
- `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: Validated row-editor pattern for custom user input. Reuse the approach for custom argument tokens, but use an ordered array, not a key/value map.
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts`: Browser-dev handlers for preview and Steam options. Add mock command arguments to generated strings.
- `src/crosshook-native/src/lib/mocks/handlers/profile-mutations.ts`: Add mock handler for the new profile save command.
- `src/crosshook-native/src/lib/mocks/handlers/system.ts`: Add a mock command-argument catalog alongside the optimization catalog.

## Implementation Patterns With Examples

- **Serde-defaulted nested launch section**: Add a section similar to:

  ```rust
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
  pub struct CommandArgumentsSection {
      #[serde(rename = "enabled_argument_ids", default, skip_serializing_if = "Vec::is_empty")]
      pub enabled_argument_ids: Vec<String>,
      #[serde(rename = "custom_tokens", default, skip_serializing_if = "Vec::is_empty")]
      pub custom_tokens: Vec<String>,
  }
  ```

  Then add `pub command_arguments: CommandArgumentsSection` to `LaunchSection` with `serde(default, skip_serializing_if = "CommandArgumentsSection::is_empty")`.

- **Dedicated resolver**: Mirror `resolve_launch_directives_for_method()` with a resolver that returns ordered argv tokens:

  ```rust
  pub struct ResolvedCommandArguments {
      pub tokens: Vec<String>,
  }
  ```

  Resolve in catalog order for curated IDs, then append profile custom tokens in user order. That gives stable output while preserving explicit user ordering for custom arguments.

- **Method applicability**: Existing optimization entries have both `applies_to_method` and `applicable_methods`, but resolver enforcement uses `applies_to_method`. Avoid this ambiguity in the new catalog: use one field, preferably `applicable_methods: Vec<String>`, and have resolver check membership against `request.resolved_method()`. For Steam, resolve against `steam_applaunch` but remember output placement differs.

- **Strict plus aggregate validation**: Follow `validate_custom_env()` and `collect_custom_env_issues()`. Add custom token validation to strict launch validation and issue collection so the launch button and preview show the same failures.

- **Steam output placement**: Current Steam options builder ends with `%command%`. The new form should be:

  ```text
  KEY=value wrapper %command% --arg-from-catalog custom-token
  ```

  Use the existing `escape_steam_token()` for each argument token too. It already quotes whitespace and shell-sensitive characters.

- **Direct Proton/umu output placement**: In `proton_game.rs`, append arguments after the executable:

  ```rust
  command.arg(normalized_game_path.trim());
  for token in resolved_args.tokens {
      command.arg(token);
  }
  ```

  This preserves structured argv and avoids shell splitting.

- **Preview parity**: `preview/command.rs` currently builds strings independently from real command construction. Add the same resolved token list there, and make `preview/builder.rs` resolve once so `effective_command` and `steam_launch_options` cannot drift.

- **Frontend normalization**: Add defaults in `DEFAULT_LAUNCH_SECTION`, normalize loaded arrays in `normalizeSerializedGameProfile()`, and filter invalid/unknown curated IDs in `normalizeProfileForEdit()` once the catalog is loaded.

- **Autosave queue**: Use the existing `enqueueLaunchProfileWrite` chain in `useProfileLaunchAutosave.ts`; do not add an independent autosave path that writes the same profile file concurrently.

## Integration Points (Files To Create Or Modify)

Backend core:

- Create `src/crosshook-native/assets/default_command_argument_catalog.toml`.
- Create `src/crosshook-native/crates/crosshook-core/src/launch/command_arguments/` with:
  - `mod.rs`
  - `catalog.rs` or `entries.rs`
  - `resolver.rs`
  - focused tests for parsing, duplicates, conflicts, method gating, and ordering.
- Modify `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs` to export catalog load/init/get and resolver APIs.
- Modify `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs` to add `CommandArgumentsSection`.
- Modify `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs` to add a narrow `save_command_arguments()` helper if autosave should avoid full-profile writes.
- Modify `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/error.rs` for invalid command-argument IDs if validating on save.
- Modify `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs` to add `CommandArgumentsRequest`.
- Modify `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`, `error.rs`, and `error_text.rs` for new validation.
- Modify `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs` or move the builder to a more general module so it accepts resolved command-argument tokens.
- Modify `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs` to append tokens after the game executable.
- Do not modify `proton_trainer.rs` to append game arguments; add regression coverage that trainer-only does not inherit them.
- Modify `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs` and `preview/command.rs`.

Tauri:

- Modify `src/crosshook-native/src-tauri/src/commands/catalog.rs` to expose `get_command_argument_catalog`.
- Modify `src/crosshook-native/src-tauri/src/commands/launch/queries.rs` so `build_steam_launch_options_command` accepts command-argument data, or replace the signature with a request-shaped DTO if that reduces parameter drift.
- Modify `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs` or add a sibling command module for `profile_save_command_arguments`.
- Modify `src/crosshook-native/src-tauri/src/commands/profile/mod.rs` to re-export the new command.
- Modify `src/crosshook-native/src-tauri/src/lib.rs` to initialize the new catalog and register new commands.

Frontend:

- Create `src/crosshook-native/src/types/launch-command-arguments.ts`.
- Create `src/crosshook-native/src/utils/command-argument-catalog.ts`.
- Create `src/crosshook-native/src/hooks/useCommandArgumentCatalog.ts`.
- Create `src/crosshook-native/src/components/CommandArgumentsPanel.tsx`.
- Create `src/crosshook-native/src/components/launch-subtabs/CommandArgumentsTabContent.tsx` if adding a distinct subtab.
- Modify `src/crosshook-native/src/types/profile.ts`, `types/launch.ts`, and `utils/launch.ts`.
- Modify `src/crosshook-native/src/hooks/profile/profileNormalize.ts`.
- Modify `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts` and `useProfileLaunchAutosaveEffects.ts`.
- Modify `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts`.
- Modify `src/crosshook-native/src/components/launch-subtabs/types.ts`, `useTabVisibility.ts`, and `LaunchSubTabs.tsx`.
- Modify `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx` and `SteamOptionsTabContent.tsx`.
- Modify mocks in `src/crosshook-native/src/lib/mocks/handlers/launch.ts`, `profile-mutations.ts`, and `system.ts`.
- Update test fixtures in `src/crosshook-native/src/test/fixtures.ts`.

## Code Conventions

- Keep launch behavior in `crosshook-core`; `src-tauri` should only marshal DTOs and map errors.
- Use `snake_case` for Tauri commands and serde fields crossing IPC.
- Use profile TOML defaults and `skip_serializing_if` so older profiles load without migration.
- Use ordered `Vec<String>` for argument IDs and custom tokens. Do not use maps for argv tokens because order matters.
- Treat custom arguments as argv tokens, not a shell command line. The UI should collect individual tokens; the backend should append each token via `Command::arg`.
- Keep custom argument validation explicit: reject empty/whitespace-only tokens after trim, reject NUL bytes, and consider a reasonable max token length/count.
- For Steam launch options only, quote tokens with the existing Steam token escaping function or an equivalent in the same module.
- Follow BEM-like `crosshook-*` classes and existing `DashboardPanelSection`/subtab structure for UI.
- Do not introduce a new frontend state system. Thread props through `useLaunchSubTabsProps` and the existing profile context/update callbacks.
- If a new scroll container is added, update `src/crosshook-native/src/hooks/useScrollEnhance.ts`.

## Dependencies And Services

- No SQLite migration or metadata service is needed.
- No new runtime process dependency is needed.
- Avoid adding a shell parser dependency if the UI stores custom arguments as discrete tokens. A command-line textarea would require robust shell-like parsing and escaping; the current codebase does not have a general parser for that.
- `platform.rs` host gateway remains the only route for host-tool execution. Command arguments should be appended to commands already built by `runtime_helpers`, not used to construct new host commands.
- Catalog initialization should mirror optimization catalog startup in `src-tauri/src/lib.rs`: load embedded defaults plus optional user override from settings config dir, initialize a process-global catalog, then expose via IPC.
- Browser dev mode requires mock catalog, preview, Steam options, and profile mutation behavior because frontend code uses `callCommand()` through the mock bridge.

## Gotchas And Warnings

- Do not add game command arguments to trainer launches. `proton_trainer.rs` launches the trainer executable and should stay independent.
- Do not append arguments before `%command%` for Steam. Steam launch options prefix before `%command%` configures env/wrappers; game argv belongs after `%command%`.
- Do not use `split_whitespace()` on a user-entered command-line string for real execution. It breaks quoting and can change argv semantics. Prefer one row per token.
- Do not route custom arguments through environment variables or wrappers. That would blur the existing optimization model and make preview/execution parity harder.
- Do not let custom arguments override Proton runtime structure like `run`, `PROTON_VERB`, `GAMEID`, or wrapper placement. They are only target executable arguments.
- Steam and direct Proton need separate rendering: direct launch uses structured `Command::arg`, while Steam requires a single escaped string.
- `preview/command.rs` is independent of `proton_game.rs`; updating only one will create misleading previews.
- `build_steam_launch_options_command` is called both from preview and from `SteamLaunchOptionsPanel`; update both call sites and the Tauri command signature together.
- Current mock `build_steam_launch_options_command` reads `enabled_option_ids`, but frontend sends `enabledOptionIds` through Tauri camelCase conversion. When extending mocks, verify the mock argument shape matches actual browser-mode `callCommand()` payloads.
- Existing mock optimization catalog IDs (`esync`, `fsync`, `mangohud`) differ from real built-in IDs. Keep new command-argument mocks internally consistent with the new UI tests rather than copying production entries blindly.
- If collection defaults are not part of the feature, leave command arguments profile-only and document that decision. Adding them to `CollectionDefaultsSection` has merge semantics implications because custom tokens are ordered and not naturally additive like env vars.
- If custom native arguments are supported, native preview/execution must also append them after `game_path`; curated Proton/Steam entries should remain gated off for native.

## Task-Specific Guidance

1. Start backend-first with profile/request models:
   - Add `CommandArgumentsSection` to `LaunchSection`.
   - Add matching `CommandArgumentsRequest` to `LaunchRequest`.
   - Thread fields from frontend request builder and test fixtures.

2. Build the catalog/resolver in core:
   - Use a separate embedded TOML catalog.
   - Include `id`, `tokens`, `label`, `description`, `help_text`, `category`, `advanced`, `community`, `applicable_methods`, and `conflicts_with`.
   - Resolve selected IDs in catalog order, detect duplicate selected IDs, reject unknown IDs, reject conflicts, and method-gate entries.
   - Append validated `custom_tokens` after curated tokens.

3. Update execution and preview together:
   - `proton_game.rs`: append resolved tokens after `normalized_game_path`.
   - `steam_options.rs`: append escaped tokens after `%command%`.
   - `preview/builder.rs`: resolve arguments once and pass to effective command and Steam option builders.
   - `preview/command.rs`: show the same token placement as real execution.

4. Add validation coverage:
   - Unknown curated ID.
   - Duplicate curated ID.
   - Method-gated curated ID on native.
   - Conflict between curated IDs.
   - Custom token containing NUL.
   - Direct Proton/umu command appends tokens after game exe.
   - Steam launch options append tokens after `%command%`.
   - Trainer-only launch ignores game command arguments.

5. Add UI as one launch configuration surface:
   - Prefer a dedicated `Command Arguments` subtab if the combined controls are too dense for the existing optimization tab.
   - Reuse the grouped toggle pattern from `LaunchOptimizationsPanel` for curated entries.
   - Use an ordered row/list editor for custom tokens, based on `CustomEnvironmentVariablesSection` patterns but not key/value fields.
   - Add autosave status into the existing chip aggregation if command arguments autosave independently.

6. Keep persistence narrow:
   - Add `profile_save_command_arguments` or an equivalent narrow save command.
   - Use the existing launch write queue so command-argument autosaves cannot race with optimization/gamescope/mangohud saves.
   - Capture metadata config revisions the same way existing launch-section saves do; no SQLite schema change is required.

7. Verification commands for implementation:
   - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
   - `cd src/crosshook-native && npm run typecheck`
   - `cd src/crosshook-native && npm test`
   - `./scripts/check-host-gateway.sh`
