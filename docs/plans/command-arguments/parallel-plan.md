# Command Arguments Implementation Plan

Implement command arguments as a profile-scoped launch feature that stores selected curated argument IDs and ordered custom argument tokens in profile TOML, then threads the same data into launch validation, preview, Steam launch-options generation, and real Proton/umu execution. Add a dedicated command-argument catalog because these entries produce game argv tokens, not optimization env vars or wrappers. The supported first-class methods are `steam_applaunch` and `proton_run`; `umu-run` is covered through the existing `proton_run` builder. The UI must stay on the existing launch configuration page by adding the argument controls to the current optimization/launch-options surface rather than creating a new route or separate subtab.

## Critically Relevant Files and Documentation

- docs/plans/command-arguments/shared.md: Validated shared context and persistence boundary for this plan.
- docs/plans/command-arguments/analysis-context.md: Condensed architecture and cross-cutting constraints.
- docs/plans/command-arguments/analysis-code.md: Code-level integration map and gotchas.
- docs/plans/command-arguments/analysis-tasks.md: Dependency structure and task split guidance.
- CLAUDE.md: Source-of-truth repo rules, persistence classification, and host-tool constraints.
- AGENTS.md: Stack overview, browser dev mode, SQLite inventory, route layout, and scroll guidance.
- docs/architecture/adr-0001-platform-host-gateway.md: Host command boundary that command construction must preserve.
- docs/features/steam-proton-trainer-launch.doc.md: User-facing Steam/Proton launch behavior to update after implementation.
- docs/getting-started/quickstart.md: User-facing launch setup and preview guide to update after implementation.
- docs/internal-docs/design-tokens.md: UI design token guidance for the new in-page controls.
- src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs: Profile TOML launch schema extension point.
- src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs: Rust `LaunchRequest` DTO extension point.
- src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs: Central strict and aggregate validation path.
- src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs: Validation issue variants and stable codes.
- src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs: Embedded catalog loader pattern.
- src/crosshook-native/assets/default_optimization_catalog.toml: Catalog TOML shape to mirror.
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs: ID resolution, duplicate, conflict, and method-gating pattern.
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs: Steam `%command%` builder and escaping tests.
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs: Real direct Proton/umu game command builder.
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs: Trainer launch path that must not inherit game arguments.
- src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs: Preview orchestration for effective command and Steam options.
- src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs: Human-readable command string generation.
- src/crosshook-native/src-tauri/src/commands/launch/queries.rs: Thin IPC for preview and Steam launch-options command generation.
- src/crosshook-native/src-tauri/src/commands/catalog.rs: Catalog IPC pattern.
- src/crosshook-native/src-tauri/src/lib.rs: Tauri startup/catalog initialization and command registration.
- src/crosshook-native/src/types/profile.ts: Frontend `GameProfile.launch` contract.
- src/crosshook-native/src/types/launch.ts: Frontend `LaunchRequest` contract.
- src/crosshook-native/src/utils/launch.ts: Profile-to-launch-request bridge.
- src/crosshook-native/src/hooks/profile/profileNormalize.ts: Backward-compatible profile normalization.
- src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts: Serialized launch-section autosave queue.
- src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts: Debounced launch-section autosave effect pattern.
- src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts: Launch page state and prop wiring.
- src/crosshook-native/src/components/LaunchSubTabs.tsx: Existing one-page launch configuration host.
- src/crosshook-native/src/components/launch-subtabs/OptimizationsTabContent.tsx: Existing launch optimization tab content to extend in-place.
- src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx: Curated grouped toggle UI pattern.
- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: Ordered custom input/editor pattern reference.
- src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx: Derived Steam copy/paste preview.
- src/crosshook-native/src/lib/mocks/handlers/launch.ts: Browser dev mock for launch preview and Steam options.
- src/crosshook-native/src/lib/mocks/handlers/profile-mutations.ts: Browser dev profile mutation mock pattern.

## Implementation Plan

### Phase 1: Contract and Profile Persistence Foundation

#### Task 1.1: Add Rust Profile Command Arguments Section Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/command-arguments/shared.md
- src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs
- src/crosshook-native/crates/crosshook-core/src/profile/models/tests/launch_section.rs

**Instructions**

Files to Create

- None

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs
- src/crosshook-native/crates/crosshook-core/src/profile/models/tests/launch_section.rs

Add a serde-defaulted nested profile section under `LaunchSection`, named `command_arguments`, with `enabled_argument_ids: Vec<String>` and `custom_args: Vec<String>`. Store both arrays in user order, omit the entire section when empty, and add an `is_empty()` helper like `LaunchOptimizationsSection`. Do not add collection-default merge behavior in this task; keep arguments profile-specific for the first implementation. Add TOML tests proving empty command arguments are omitted, populated command arguments round-trip, and older profiles without the section deserialize to empty defaults.

#### Task 1.2: Add Rust LaunchRequest Command Arguments DTO Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request/tests/serde_roundtrip.rs
- src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs

**Instructions**

Files to Create

- None

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request/tests/serde_roundtrip.rs

Add a request-side command-arguments DTO matching the profile shape and attach it to `LaunchRequest` with serde defaults and empty omission. Keep the field name aligned with profile TOML and frontend JSON, preferably `command_arguments`. Add request serde round-trip coverage for missing, empty, and populated command arguments. The request DTO should not carry resolved/escaped strings; it carries selected IDs and custom tokens only.

#### Task 1.3: Add Frontend Profile, Launch, and Normalization Types Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/types/launch.ts
- src/crosshook-native/src/hooks/profile/profileNormalize.ts
- src/crosshook-native/src/test/fixtures.ts
- src/crosshook-native/src/hooks/profile/**tests**/profileNormalize.test.ts

**Instructions**

Files to Create

- src/crosshook-native/src/types/launch-command-arguments.ts

Files to Modify

- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/types/launch.ts
- src/crosshook-native/src/hooks/profile/profileNormalize.ts
- src/crosshook-native/src/test/fixtures.ts
- src/crosshook-native/src/hooks/profile/**tests**/profileNormalize.test.ts

Define frontend command-argument types separately from launch optimizations, then add `launch.command_arguments` to `GameProfile` and `command_arguments` to `LaunchRequest`. Normalize missing sections to `{ enabled_argument_ids: [], custom_args: [] }`, trim curated IDs, and drop blank custom rows only as draft hygiene before save; do not silently drop non-blank invalid tokens with control characters or NUL bytes. Invalid non-blank tokens should remain visible to the UI and be surfaced by frontend or backend validation. Update test fixtures so existing tests receive stable defaults. Add focused normalization tests for older profiles, blank-row cleanup, and populated command-argument arrays.

### Phase 2: Core Catalog, Resolver, Validation, and Save Semantics

#### Task 2.1: Create Command Argument Catalog and Resolver Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs
- src/crosshook-native/assets/default_optimization_catalog.toml
- docs/plans/command-arguments/analysis-code.md

**Instructions**

Files to Create

- src/crosshook-native/assets/default_command_argument_catalog.toml
- src/crosshook-native/crates/crosshook-core/src/launch/command_arguments/mod.rs
- src/crosshook-native/crates/crosshook-core/src/launch/command_arguments/catalog.rs
- src/crosshook-native/crates/crosshook-core/src/launch/command_arguments/resolver.rs
- src/crosshook-native/crates/crosshook-core/src/launch/command_arguments/tests.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs

Create a dedicated argument catalog with entries containing `id`, `tokens`, `label`, `description`, `help_text`, `category`, `advanced`, `community`, `applicable_methods`, and `conflicts_with`. Seed a conservative catalog with clearly caveated common entries such as Vulkan/DX11/DX12 selectors and launcher-skip style arguments, but do not imply the flags work for every game. Implement a resolver that validates known IDs, rejects duplicates/conflicts, checks method applicability, emits curated tokens in catalog order, and appends custom tokens in user order. Add unit tests for parsing, bad entries, unknown IDs, duplicates, conflicts, method gating, and deterministic ordering.

#### Task 2.2: Add Launch Validation Errors for Command Arguments Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request/error_text.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request/tests/method_validation.rs

**Instructions**

Files to Create

- None

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request/error_text.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request/tests/method_validation.rs

Wire command-argument validation into both strict `validate()` and aggregate `validate_all()`. Reject unknown curated IDs, duplicate curated IDs, conflicts, unsupported method selections, empty or whitespace-only custom args, NUL/control characters, and excessive token count/length. First-class command arguments are supported only for `steam_applaunch` and `proton_run`; reject non-empty command arguments for `native` in this implementation rather than adding native custom-only behavior. Allow normal CLI punctuation such as `--flag=value`, `+set`, `-dx11`, and paths with spaces because custom args are stored as structured tokens. Add stable issue codes/messages and tests that verify fatal validation behavior without depending on frontend filtering.

#### Task 2.3: Add Narrow Profile Save Support for Command Arguments Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/error.rs
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/optimizations.rs
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/mod.rs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/command_arguments.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/error.rs
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/mod.rs

Add a `save_command_arguments` helper that loads the profile, trims curated IDs and custom-token outer whitespace, validates curated IDs/custom args through the core command-argument validation path, replaces only `launch.command_arguments`, and saves the profile. Keep this profile TOML only; do not add SQLite schema or metadata tables. Backend save must reject empty or whitespace-only custom tokens instead of silently removing them; frontend draft cleanup may avoid submitting blank rows, but persisted/request validation stays strict. Add tests proving the helper preserves unrelated profile fields, rejects missing profiles, rejects unknown IDs, rejects blank and invalid custom tokens, and writes only the intended launch subsection.

### Phase 3: Backend Launch, Steam Options, and Preview Integration

#### Task 3.1: Append Arguments After `%command%` in Steam Launch Options Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs
- src/crosshook-native/crates/crosshook-core/src/launch/command_arguments/resolver.rs
- docs/features/steam-proton-trainer-launch.doc.md

**Instructions**

Files to Create

- None

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs

Extend `build_steam_launch_options_command` so it accepts command-argument request data or an already-resolved token slice, resolves/escapes tokens through core code, and appends them after `%command%`. Preserve existing env and wrapper ordering before `%command%`. Use the existing Steam token escaping behavior or an argument-specific wrapper around it for spaces and shell-sensitive characters. Add tests for empty args, curated args, custom args, combined curated/custom ordering, quoting of spaces/metacharacters, and gamescope/mangohud wrapper ordering with args still after `%command%`.

#### Task 3.2: Append Arguments After Game Executable for Proton and umu Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_game.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_game_umu.rs

**Instructions**

Files to Create

- None

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_game.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_game_umu.rs

Resolve command arguments once in the game launch builder and append each token with `Command::arg` immediately after `command.arg(normalized_game_path.trim())`. Do not shell-concatenate, do not touch host-tool construction, and do not change `runtime_helpers` unless tests prove a helper signature is necessary. Add tests showing direct Proton args appear after `<proton> run <game.exe>`, umu args appear after `umu-run <game.exe>` without inserting `run`, and gamescope/wrapper chains keep args after the game executable rather than before wrappers or `--`.

#### Task 3.3: Update Launch Preview and Guard Trainer Non-Inheritance Depends on [3.1, 3.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs
- src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs
- src/crosshook-native/crates/crosshook-core/src/launch/preview/tests/command_string.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_trainer.rs

**Instructions**

Files to Create

- None

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs
- src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs
- src/crosshook-native/crates/crosshook-core/src/launch/preview/tests/command_string.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_trainer.rs

Make preview resolve command arguments through the same core resolver and pass the resulting tokens to both the human-readable effective command and Steam launch-options generation. Preserve current behavior where preview can surface directive/validation errors without panicking. Add tests showing Proton/umu previews include args after the game executable, Steam previews show `%command% <args>`, and trainer-only launches do not inherit game args. Do not append game args in `proton_trainer.rs`; tests should protect the existing separation.

### Phase 4: IPC, Startup Registration, and Browser Dev Mocks

#### Task 4.1: Add Thin Tauri Commands and Catalog Startup Depends on [2.1, 2.3, 3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/catalog.rs
- src/crosshook-native/src-tauri/src/commands/launch/queries.rs
- src/crosshook-native/src-tauri/src/commands/profile/mod.rs
- src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs
- src/crosshook-native/src-tauri/src/lib.rs

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/profile/command_arguments.rs

Files to Modify

- src/crosshook-native/src-tauri/src/commands/catalog.rs
- src/crosshook-native/src-tauri/src/commands/launch/queries.rs
- src/crosshook-native/src-tauri/src/commands/profile/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

Expose `get_command_argument_catalog`, update `build_steam_launch_options_command` IPC to accept command-argument data, and add `profile_save_command_arguments` as a thin wrapper over the profile store helper. Initialize the command-argument catalog during startup alongside the optimization catalog, using embedded defaults and optional user override only if the core catalog loader supports it. The first implementation must not add command-argument SQLite tables or schema migrations for either user selections or catalog persistence; the catalog is embedded/runtime unless an existing non-migrating generic path is reused. Keep all command names snake_case and all parsing/resolution in `crosshook-core`.

#### Task 4.2: Add Frontend Catalog Utilities and Request Bridge Depends on [1.3, 4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/launch-command-arguments.ts
- src/crosshook-native/src/utils/optimization-catalog.ts
- src/crosshook-native/src/hooks/useLaunchOptimizationCatalog.ts
- src/crosshook-native/src/utils/launch.ts

**Instructions**

Files to Create

- src/crosshook-native/src/utils/command-argument-catalog.ts
- src/crosshook-native/src/hooks/useCommandArgumentCatalog.ts

Files to Modify

- src/crosshook-native/src/types/launch-command-arguments.ts
- src/crosshook-native/src/utils/launch.ts

Add TypeScript catalog payload types, cached fetch helpers, and a hook for command-argument catalog data. Update `buildProfileLaunchRequest` so every launch, preview, and dry-run request includes `profile.launch.command_arguments`. Keep the frontend payload shape aligned with Serde names. Add focused utility/request-building tests if existing test layout provides a suitable location; otherwise rely on downstream LaunchSubTabs tests in Phase 5.

#### Task 4.3: Update Browser Dev Mocks for New IPC and DTO Fields Depends on [3.3, 4.1, 4.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/README.md
- src/crosshook-native/src/lib/mocks/handlers/launch.ts
- src/crosshook-native/src/lib/mocks/handlers/profile-mutations.ts
- src/crosshook-native/src/lib/mocks/handlers/system.ts
- src/crosshook-native/src/test/fixtures.ts

**Instructions**

Files to Create

- None

Files to Modify

- src/crosshook-native/src/lib/mocks/handlers/launch.ts
- src/crosshook-native/src/lib/mocks/handlers/profile-mutations.ts
- src/crosshook-native/src/lib/mocks/handlers/system.ts
- src/crosshook-native/src/test/fixtures.ts

Add mock catalog data, mock `profile_save_command_arguments`, and update `build_steam_launch_options_command` / `preview_launch` mock behavior so command arguments appear after `%command%` or after the game executable in browser dev mode. This task depends on preview parity because mock preview strings should mirror the real `preview/command.rs` behavior after Task 3.3. Preserve existing fixture modes: `error` must throw `[dev-mock] forced error for <command>`, `loading` must never resolve, and `empty` should produce safe empty catalog or `%command%` output. Keep mock IDs internally consistent with frontend tests.

### Phase 5: One-Page Frontend UI, Autosave, and Derived Steam Output

#### Task 5.1: Wire Launch State and Autosave for Command Arguments Depends on [4.1, 4.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts
- src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts
- src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts
- src/crosshook-native/src/components/launch-subtabs/types.ts
- src/crosshook-native/src/components/launch-subtabs/useAutoSaveChip.ts

**Instructions**

Files to Create

- None

Files to Modify

- src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts
- src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts
- src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts
- src/crosshook-native/src/components/launch-subtabs/types.ts
- src/crosshook-native/src/components/launch-subtabs/useAutoSaveChip.ts

Add command-argument state, toggle/update handlers, and debounced autosave status using the existing serialized launch write queue. Saved profiles should call `profile_save_command_arguments`; unsaved profiles should update local draft state and surface a save-first warning like optimizations. Include command-argument autosave status in the existing chip priority aggregation so users see saving/success/error feedback in the same in-page surface.

#### Task 5.2: Add Command Argument Controls Inside Existing Optimizations Surface Depends on [5.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LaunchSubTabs.tsx
- src/crosshook-native/src/components/launch-subtabs/OptimizationsTabContent.tsx
- src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx
- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx
- docs/internal-docs/design-tokens.md

**Instructions**

Files to Create

- src/crosshook-native/src/components/CommandArgumentsPanel.tsx

Files to Modify

- src/crosshook-native/src/components/LaunchSubTabs.tsx
- src/crosshook-native/src/components/launch-subtabs/OptimizationsTabContent.tsx
- src/crosshook-native/src/components/launch-subtabs/types.ts
- src/crosshook-native/src/styles/launch-pipeline.css

Create an in-page command arguments panel and render it in the existing optimizations/launch-options tab content, not in a new route or new subtab. The panel should present curated catalog entries with grouped toggles, accessible help text, conflict/unsupported-method states, and an ordered custom-token editor. Use the same visual density and BEM-style `crosshook-*` classes as launch optimizations, but avoid nesting cards inside cards; if existing launch/optimization classes are insufficient, add scoped styles in the existing launch stylesheet rather than creating a new broad theme file. Curated entries should be available for `steam_applaunch` and `proton_run`; hide or disable unsupported entries by method.

#### Task 5.3: Include Arguments in Steam Copy/Paste Preview and Frontend Tests Depends on [3.3, 5.2, 4.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx
- src/crosshook-native/src/components/launch-subtabs/SteamOptionsTabContent.tsx
- src/crosshook-native/src/components/**tests**/LaunchSubTabs.test.tsx
- src/crosshook-native/src/components/library/**tests**/HeroLaunchCommandSection.test.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/**tests**/CommandArgumentsPanel.test.tsx

Files to Modify

- src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx
- src/crosshook-native/src/components/launch-subtabs/SteamOptionsTabContent.tsx
- src/crosshook-native/src/components/**tests**/LaunchSubTabs.test.tsx
- src/crosshook-native/src/components/library/**tests**/HeroLaunchCommandSection.test.tsx

Thread command arguments into the derived Steam launch-options panel so the copyable line shows `%command% <args>`. Keep this panel read-only/derived; editing remains in the one-page launch optimization/arguments surface. Add tests for tab/page visibility, toggling curated arguments, editing ordered custom tokens, autosave status, Steam output including arguments, and hero launch command preview text when command arguments are present. Update existing LaunchSubTabs tests to expect the in-page command arguments section for supported methods without adding a new tab.

### Phase 6: Documentation and Verification

#### Task 6.1: Update User Documentation for Command Arguments Depends on [3.3, 5.3]

**READ THESE BEFORE TASK**

- docs/features/steam-proton-trainer-launch.doc.md
- docs/getting-started/quickstart.md
- docs/plans/command-arguments/shared.md

**Instructions**

Files to Create

- None

Files to Modify

- docs/features/steam-proton-trainer-launch.doc.md
- docs/getting-started/quickstart.md

Document the new command-arguments model: curated argument toggles, custom token rows, Steam placement after `%command%`, direct Proton/umu placement after the game executable, no trainer inheritance, and profile TOML persistence. Clarify that CrossHook does not write into Steam for `steam_applaunch`; users still copy the generated line after editing arguments. Resolve the stale quickstart wording that says Launch Optimizations are proton-only if it conflicts with current Steam support.

#### Task 6.2: Run Final Validation Suite Depends on [6.1]

**READ THESE BEFORE TASK**

- docs/TESTING.md
- scripts/check-host-gateway.sh
- src/crosshook-native/Cargo.toml
- src/crosshook-native/package.json

**Instructions**

Files to Create

- None

Files to Modify

- None

Run the backend and frontend validation appropriate for the touched surfaces: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`, `cd src/crosshook-native && npm run typecheck`, `cd src/crosshook-native && npm test`, and `./scripts/check-host-gateway.sh`. If UI layout changed materially, also run browser dev smoke/manual verification through `./scripts/dev-native.sh --browser` and capture any failures in the implementation report. Do not claim launch parity until Steam options, preview, direct Proton/umu command tests, and trainer non-inheritance tests all pass.

## Advice

- Keep command arguments semantically separate from launch optimizations. Optimizations produce env/wrapper directives; command arguments produce target argv tokens.
- Store custom arguments as an ordered token array, not a raw shell string. This avoids shell parsing drift and lets real launches use `Command::arg` safely.
- Use one core resolver output everywhere. Steam options, preview, validation, and real launch should not independently interpret IDs or custom tokens.
- The Steam placement rule is strict: env/wrappers before `%command%`, game arguments after `%command%`.
- The Proton/umu placement rule is strict: wrapper/proton-or-umu setup, then the game executable path, then game arguments.
- Do not add game arguments to trainer launches. Trainer argument support would be a separate feature with a different UI and persistence boundary.
- Avoid a new frontend route or subtab. The user specifically asked for one page, so the controls belong inside the existing launch configuration surface.
- No SQLite migration is needed for the first implementation. User-selected arguments are profile TOML preferences; generated command lines are runtime-only.
- Run `./scripts/check-host-gateway.sh` after command builder work even though this feature only appends target args; it is the guardrail for accidental host-tool gateway regressions.
