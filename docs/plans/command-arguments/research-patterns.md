# Pattern Research: command-arguments

## Architectural Patterns

**Core-owned launch behavior**: Launch behavior is modeled and validated in `crosshook-core`; Tauri only exposes thin IPC wrappers and frontend hooks consume those commands.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`
- Example: `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs`

**Profile-scoped launch configuration in TOML**: User launch choices that should travel with a profile live under `LaunchSection` and serialize with serde defaults/empty skipping. Command arguments fit this layer better than SQLite because they are user-editable per-profile launch preferences, similar to `launch.optimizations` and `launch.custom_env_vars`.

- Example: `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/optimizations.rs`
- Example: `src/crosshook-native/src/types/profile.ts`

**Data-driven curated catalogs**: Curated launch options are represented as embedded TOML catalogs with Rust parsing/validation, then exposed to TS through an IPC DTO and cached frontend utility. A command-argument catalog should mirror this pattern with entries containing id, method applicability, label/description/help text, category, conflicts, and argument tokens.

- Example: `src/crosshook-native/assets/default_optimization_catalog.toml`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`
- Example: `src/crosshook-native/src/utils/optimization-catalog.ts`
- Example: `src/crosshook-native/src/hooks/useLaunchOptimizationCatalog.ts`

**Deterministic directive resolution**: Optimization IDs resolve in catalog order, reject unknown/duplicate/conflicting IDs, check required binaries, and produce normalized directives. Command arguments should resolve selected curated IDs into a deterministic token list before merging custom tokens.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`

**Preview and execution share semantics but use different builders**: The preview builder creates user-visible strings, while script runner builders create real `Command` values. Command arguments must be added in both surfaces at the same semantic point: after `%command%` for Steam launch options and after the game executable path for direct Proton/umu.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`

**Flatpak host-tool gateway**: Direct Proton/umu/gamescope process creation routes through runtime helper builders that call `platform::host_command_with_env_and_directory` or related helpers. Argument support should append args to the existing command after gateway construction, not introduce direct `Command::new("proton")`, `Command::new("umu-run")`, or wrapper calls.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/platform/gateway.rs`
- Example: `docs/architecture/adr-0001-platform-host-gateway.md`

**One-page launch UX via subtabs**: Launch settings are assembled in `LaunchSubTabs`, mounted in Hero Detail, and divided into panels. The feature scope says one page, not separated like current Steam launch options, so the likely fit is a single command-arguments panel placed alongside or inside the existing launch configuration page instead of creating a separate route.

- Example: `src/crosshook-native/src/components/LaunchSubTabs.tsx`
- Example: `src/crosshook-native/src/components/library/launch/HeroLaunchSubTabsHost.tsx`
- Example: `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts`

## Code Conventions

Rust uses `snake_case` fields and serde renames for persisted/IPC names such as `enabled_option_ids` and `custom_env_vars`. New profile data should use defaults and `skip_serializing_if` so empty command arguments do not churn TOML. Use small structs with `is_empty()` helpers, as in `LaunchOptimizationsSection`.

Frontend TS uses `PascalCase` React components, `camelCase` props/functions, strict interface types in `src/crosshook-native/src/types`, and IPC through `callCommand()` wrappers/hooks rather than raw Tauri `invoke()`. Backend command names are `snake_case` and frontend payload field names match serde JSON names.

CSS classes use BEM-like `crosshook-*` names. Launch panels use `crosshook-subtab-content`, `crosshook-panel`, `DashboardPanelSection`, and feature-specific blocks such as `crosshook-launch-optimizations__...`. A command-arguments UI should follow this naming shape, use existing button/select/input styles, and avoid nested card layouts.

Organization pattern:

- Rust profile model: `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`
- Rust launch request DTO: `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`
- Rust validation/errors: `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`, `src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs`
- Rust command generation: `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`, `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`, `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`
- Tauri IPC: `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs` style, then register in command module/lib if new commands are needed
- Frontend hook/types/components: `src/crosshook-native/src/types/profile.ts`, `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts`, `src/crosshook-native/src/components/LaunchSubTabs.tsx`
- Browser mock handlers: `src/crosshook-native/src/lib/mocks/handlers/profile-presets.ts` style when adding IPC used in browser dev mode

## Error Handling

Validation errors are enum variants with stable snake_case `code()` values, human-readable messages, help text, and fatal/warning severity. New command-argument validation should add variants such as unknown argument id, duplicate argument id, unsupported method, invalid custom argument, or NUL byte if applicable.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`

Profile persistence errors use `ProfileStoreError` and are mapped to strings at the Tauri boundary. Optimizations reject unknown IDs during save; command arguments should similarly reject unknown curated IDs at save time rather than silently persisting bad selections.

- Example: `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/error.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/optimizations.rs`
- Example: `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs`

Preview errors are non-panicking: directive failures are stored in `directives_error` and can leave `effective_command` or `steam_launch_options` unset. Command-argument builder failures should follow the same path so the UI can show why a preview cannot be generated.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/preview/tests/command_string.rs`

Steam launch option escaping is explicit and tested. Custom command arguments need their own escaping/tokenization policy: for Steam, append escaped argument tokens after `%command%`; for direct Proton/umu, append each token as a separate `Command::arg` after the normalized game path.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`

## Testing Approach

Add Rust model/TOML tests for round-tripping empty and populated command-argument sections, including omission when empty and preservation during launch-section-only saves.

- Example: `src/crosshook-native/crates/crosshook-core/src/profile/models/tests/launch_section.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/optimizations.rs`

Add Rust directive/catalog tests for curated argument parsing, invalid catalog entries, duplicate IDs, unknown IDs, conflict handling, method filtering, and deterministic output order.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs`

Add Rust command generation tests for all affected launch surfaces:

- Steam launch options: `%command% --vulkan --launcher-skip` and correct quoting for spaces/metacharacters.
- Proton direct: command args appear after `<proton> run <game_path>`.
- umu direct: command args appear after `umu-run <game_path>`.
- Gamescope/wrapper cases: args stay after the game path, not before `--`, wrappers, or Proton.
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_game.rs`
- Example: `src/crosshook-native/crates/crosshook-core/src/launch/preview/tests/command_string.rs`

Add frontend tests for UI state and command preview presentation if UI is implemented: toggling curated args, adding/removing custom args, autosave status, and highlighting flags in command previews.

- Example: `src/crosshook-native/src/components/library/__tests__/HeroLaunchSubTabsHost.test.tsx`
- Example: `src/crosshook-native/src/components/library/__tests__/HighlightedCommandBlock.test.tsx`
- Example: `src/crosshook-native/src/utils/__tests__/launchPreviewPresentation.test.ts`

Run validation commands matched to touched surfaces: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` for Rust launch behavior, `npm test` and `npm run typecheck` under `src/crosshook-native` for frontend/TS changes, and `./scripts/check-host-gateway.sh` if command construction or host-tool paths are touched.

## Patterns to Follow

- Store selected curated argument IDs and custom argument tokens in profile TOML under `launch`, not SQLite. Persistence classification: user-editable preferences = TOML settings/profile data; no SQLite metadata is needed unless later adding bundled preset history/analytics; preview/build state remains runtime-only.
- Model curated command arguments like optimizations: embedded TOML catalog, Rust parser/validator, global fallback catalog, IPC DTO, frontend cached fetch hook, and grouped UI.
- Keep command generation in `crosshook-core`. `src-tauri` should only expose save/list commands and map errors.
- Apply arguments to both Steam and direct Proton/umu paths. The existing code treats `steam_applaunch` and `proton_run` as parallel launch surfaces for optimizations, and direct `proton_run` can become umu internally based on preference. Arguments should follow the actual runtime command path.
- Append command/game arguments only at the target executable boundary: after `%command%` in Steam launch options, after `game_path` in direct Proton/umu, and after trainer path only if the feature explicitly supports trainer arguments later.
- Validate custom argument tokens for NUL and decide whether custom input is tokenized as one arg per row. Prefer structured rows/tokens over free-form shell strings to avoid shell parsing ambiguity.
- Reuse launch autosave serialization from `useProfileLaunchAutosave`; concurrent launch-section writes are queued to avoid clobbering.
- Update browser dev mocks for any new IPC command; mock errors must start with `[dev-mock]`.
- Preserve the one-page UX requirement by integrating controls into the existing launch configuration surface rather than adding a separate route or Steam-only tab.
