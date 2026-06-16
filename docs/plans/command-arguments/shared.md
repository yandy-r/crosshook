# Command Arguments

Command arguments should be modeled as profile-scoped launch configuration that flows from `GameProfile.launch` into `LaunchRequest`, then into the existing Steam launch-options string builder, launch preview, and real Proton/umu command construction. The feature should add a dedicated command-argument catalog because argument entries produce argv tokens, not optimization env/wrapper directives, while still reusing the catalog-driven UX pattern from Launch Optimizations. Per-profile selected curated IDs and custom argument tokens are user-editable profile preferences stored in profile TOML; resolved command strings and preview output are runtime-only derived state. The first implementation should support `steam_applaunch` and `proton_run`, with `umu-run` covered automatically through the `proton_run` command builder; native support can be limited to custom arguments only or deferred explicitly, but Proton/Steam-specific curated entries must be method-gated.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs: Profile TOML launch schema and collection-default launch subset.
- src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs: Effective profile merge logic for collection launch defaults.
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs: Profile load/save helpers and narrow launch-section persistence pattern.
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store/error.rs: Profile persistence error variants for invalid saved launch data.
- src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs: Rust `LaunchRequest` DTO that must carry effective command arguments.
- src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs: Central launch validation path for custom env and method-specific rules.
- src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs: Stable validation error messages and issue codes.
- src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs: Embedded optimization catalog loader pattern to mirror for argument catalog loading.
- src/crosshook-native/assets/default_optimization_catalog.toml: Curated catalog TOML precedent with labels, help text, categories, conflicts, methods.
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs: Deterministic ID resolution, duplicate/conflict/method validation pattern.
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs: Steam `%command%` launch-options builder and escaping tests.
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs: Real direct Proton/umu game command builder.
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs: Trainer command builder that must not inherit game arguments.
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs: `proton_run` umu selection and GAMEID/STORE environment helpers.
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs: Flatpak-safe host command construction and structured `Command::arg` patterns.
- src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs: Preview assembly for environment, wrappers, effective command, Steam options.
- src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs: Human-readable command string generation for all launch methods.
- src/crosshook-native/src-tauri/src/commands/launch/queries.rs: Thin IPC commands for preview and Steam launch-options generation.
- src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs: Narrow launch-section save command pattern.
- src/crosshook-native/src-tauri/src/commands/catalog.rs: Catalog IPC pattern for exposing backend catalog payloads.
- src/crosshook-native/src-tauri/src/lib.rs: Tauri command registration for new snake_case IPC handlers.
- src/crosshook-native/src/types/profile.ts: Frontend `GameProfile.launch` type and serialized profile schema.
- src/crosshook-native/src/types/launch.ts: Frontend `LaunchRequest` type parity with Rust DTO.
- src/crosshook-native/src/types/launch-optimizations.ts: Optimization catalog type pattern for argument catalog TS types.
- src/crosshook-native/src/utils/launch.ts: Builds `LaunchRequest` from a profile before launch/preview IPC.
- src/crosshook-native/src/utils/optimization-catalog.ts: Cached catalog fetch and lookup helpers to mirror.
- src/crosshook-native/src/hooks/useLaunchOptimizationCatalog.ts: Frontend catalog hook pattern.
- src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts: Launch page prop assembly and profile update handler wiring.
- src/crosshook-native/src/hooks/profile/profileNormalize.ts: Backward-compatible frontend profile normalization.
- src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts: Serialized launch-section autosave queue and state aggregation.
- src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts: Debounced narrow autosave effect pattern.
- src/crosshook-native/src/components/LaunchSubTabs.tsx: One-page launch configuration host shared by launch surfaces.
- src/crosshook-native/src/components/launch-subtabs/types.ts: Launch subtab IDs, labels, and prop contract.
- src/crosshook-native/src/components/launch-subtabs/useTabVisibility.ts: Method-gated launch tab visibility.
- src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx: Curated grouped toggle UX to reuse for predefined arguments.
- src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx: Derived Steam copy/paste preview that must append args after `%command%`.
- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: Validated user row editor pattern for custom argument tokens.
- src/crosshook-native/src/lib/mocks/handlers/launch.ts: Browser dev mock for preview and Steam options commands.
- src/crosshook-native/src/lib/mocks/handlers/profile-mutations.ts: Browser dev profile mutation mock pattern.

## Relevant Tables

- None: selected curated command-argument IDs and custom argument tokens are profile TOML preferences; no SQLite metadata table is required for the first implementation.

## Relevant Patterns

**Profile TOML With Empty Defaults**: User-editable launch preferences live in `GameProfile.launch` with serde defaults and `skip_serializing_if` helpers. See [/src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs](/src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs).

**Dedicated Catalog Resolver**: Curated IDs are validated and resolved in core from embedded TOML, with deterministic ordering, conflicts, and method applicability. See [/src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs](/src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs) and [/src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs](/src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs).

**Thin Tauri IPC**: `src-tauri` handlers pass DTOs through and map errors; parsing, validation, and command generation stay in `crosshook-core`. See [/src/crosshook-native/src-tauri/src/commands/launch/queries.rs](/src/crosshook-native/src-tauri/src/commands/launch/queries.rs).

**Preview And Execution Parity**: Preview strings and real `Command` builders are separate implementations that must be updated together. See [/src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs](/src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs) and [/src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs](/src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs).

**Flatpak Host Gateway Preservation**: Host-tool process creation must keep using platform gateway helpers; argument support only appends target argv to commands already built through those helpers. See [/docs/architecture/adr-0001-platform-host-gateway.md](/docs/architecture/adr-0001-platform-host-gateway.md).

**One-Page Launch UX**: Launch controls are mounted in `LaunchSubTabs` and shared by the hero detail launch tab; command arguments should be a single in-page section/panel, not a separate route. See [/src/crosshook-native/src/components/LaunchSubTabs.tsx](/src/crosshook-native/src/components/LaunchSubTabs.tsx).

**Serialized Launch Autosaves**: Launch-section autosaves are queued to avoid concurrent narrow writes clobbering each other. See [/src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts](/src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts).

## Relevant Docs

**/CLAUDE.md**: You _must_ read this when working on CrossHook architecture, persistence classification, PR conventions, and host-tool restrictions.

**/AGENTS.md**: You _must_ read this when working on agent-runtime rules, stack overview, SQLite inventory, browser dev mode, route layout, and scroll-container guidance.

**/docs/architecture/adr-0001-platform-host-gateway.md**: You _must_ read this when touching Proton, umu, gamescope, MangoHud, or host command construction.

**/docs/features/steam-proton-trainer-launch.doc.md**: You _must_ read this when changing user-facing Steam/Proton launch behavior, launch optimizations, custom env precedence, preview semantics, or limitations.

**/docs/getting-started/quickstart.md**: You _must_ read this when updating user-facing launch setup, custom env, preview, and Steam launch-options copy/paste guidance.

**/docs/prps/prds/umu-launcher-migration.prd.md**: You _must_ read this when deciding how `proton_run` maps to direct Proton versus `umu-run`, and why Steam-owned launches stay distinct.

**/docs/prps/plans/completed/github-issue-233-umu-gameid-http-resolver.plan.md**: You _must_ read this for the recent pattern of profile TOML fields, SQLite cache classification, and runtime-only launch enrichment.

**/docs/TESTING.md**: You _must_ read this when selecting validation commands for Rust launch behavior and frontend changes.

**/docs/internal-docs/design-tokens.md**: You _must_ read this when adding or styling the command-argument controls in the launch UI.
