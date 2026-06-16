# Documentation Research: command-arguments

## Architecture Docs

- **Required** `CLAUDE.md`: Canonical repo policy. Confirms CrossHook is a native Linux Tauri v2 app, business logic belongs in `crosshook-core`, IPC stays thin, host-tool execution must use the `platform.rs` gateway, and feature plans must classify persisted data.
- **Required** `AGENTS.md`: Stack overview, directory map, SQLite metadata inventory, Browser Dev Mode notes, route layout/scroll-container guidance, and the same launch/Flatpak constraints preserved for agent runtimes.
- **Required** `docs/architecture/adr-0001-platform-host-gateway.md`: Host command boundary. Any argument implementation that touches Proton, `umu-run`, gamescope, MangoHud, or wrapper command construction must preserve `host_command_with_env*` routing and Flatpak env-threading behavior.
- **Required** `docs/features/steam-proton-trainer-launch.doc.md`: Best current user-facing explanation of launch methods, launch optimizations, custom env precedence, Steam launch options copy/paste behavior, and preview semantics. This is the closest existing doc to command-argument behavior.
- **Required** `docs/prps/prds/umu-launcher-migration.prd.md`: Explains why umu only applies to the non-Steam `proton_run` path, why Steam profiles stay Steam-owned, and how command builders branch between direct Proton and `umu-run`.
- **Required** `docs/prps/plans/completed/github-issue-233-umu-gameid-http-resolver.plan.md`: Recent plan for launch request enrichment and umu preview diagnostics. Useful because it explicitly classifies profile TOML fields, SQLite cache rows, and runtime-only launch state.
- **Nice-to-have** `docs/architecture/adr-0004-flatpak-per-app-isolation.md`: Storage layout and migration context for Flatpak profiles/settings. Relevant when deciding where profile command arguments live.
- **Nice-to-have** `docs/architecture/adr-0002-flatpak-portal-contracts.md`: Useful only if argument work intersects GameMode/gamescope lifecycle behavior.

## API / IPC Docs

- **Required** `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`: Thin Tauri command pattern for `preview_launch`, `validate_launch`, and `build_steam_launch_options_command`. New command-argument preview/copy behavior should keep core logic in `crosshook-core`.
- **Required** `src/crosshook-native/src-tauri/src/lib.rs`: Command registration list. Any new IPC command must be registered here and use snake_case.
- **Required** `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`: Existing Steam `%command%` line builder, token escaping, custom env merge order, gamescope wrapper placement, and tests. This is the strongest local precedent for appending game arguments to Steam launch options.
- **Required** `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`: Builds launch preview from validation, directives, environment, effective command, Steam launch options, and umu decision. Command arguments must appear consistently in preview and real launch paths.
- **Required** `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`: Human-readable effective command string generation for `proton_run`, `steam_applaunch`, and `native`.
- **Required** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`: Real direct Proton/umu game command construction. This is where game executable arguments would likely be appended for `proton_run`.
- **Required** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs`: Trainer command construction. Read to avoid applying game arguments to trainer subprocesses accidentally.
- **Required** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs`: umu decision helpers and Steam-profile opt-out. Important for deciding whether arguments should apply to `umu-run` and direct Proton identically under `METHOD_PROTON_RUN`.
- **Required** `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs`: Low-level command builder and gamescope extra-args precedent. Shows that structured args are appended as separate `Command` args, not shell-concatenated strings.
- **Required** `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs` and `src/crosshook-native/src/types/launch.ts`: Rust/TS launch request DTOs. Additive launch argument fields need serde defaults and TS parity.
- **Required** `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs` and `src/crosshook-native/src/types/profile.ts`: Profile TOML launch schema and frontend profile type. Command arguments should be profile TOML if user-authored per-profile settings.
- **Nice-to-have** `src/crosshook-native/src/lib/mocks/README.md` and `src/crosshook-native/src/lib/mocks/handlers/launch.ts`: Browser dev mock contract and launch handler fixtures. Needed for frontend manual testing without Rust.

## Development Guides

- **Required** `docs/getting-started/quickstart.md`: User-facing custom env, preview, and launch-mode descriptions. It currently states Steam launch options include env vars after optimizations and before wrappers; command arguments will need a parallel wording update.
- **Required** `docs/TESTING.md`: Validation entrypoint for Rust/frontend/manual checks once implementation starts.
- **Required** `src/crosshook-native/src/lib/mocks/README.md`: Browser Dev Mode mock expectations. Any new IPC surface or preview field used in the one-page UX needs mock coverage.
- **Required** `docs/internal-docs/design-tokens.md`: UI token and palette rules. Relevant for adding command-argument controls inside the existing launch page without style drift.
- **Nice-to-have** `docs/research/tauri-webkitgtk-e2e-spike/README.md`: Use only if the UX needs WebKitGTK-specific smoke coverage.

## README and Rule Files

- **Required** `README.md`: Public architecture/product overview. Confirms Flatpak-only distribution, native Linux scope, launch modes, preview launch mode, and curated Launch Optimizations.
- **Required** `.cursor/rules/project.mdc` and `.ai/rules/project.md`: Rule mirrors for other runtimes. Useful for confirming no rule drift when planning across agent surfaces.
- **Required** `.github/pull_request_template.md`: Required later for implementation PR shape.
- **Nice-to-have** `.github/copilot-instructions.md`: Mirrors PR/issue expectations for coding agents.

## Must-Read Documents

1. **Core behavior and scope**
   - `docs/features/steam-proton-trainer-launch.doc.md`
   - `docs/getting-started/quickstart.md`
   - `docs/prps/prds/umu-launcher-migration.prd.md`

2. **Architecture and persistence**
   - `CLAUDE.md`
   - `AGENTS.md`
   - `docs/architecture/adr-0001-platform-host-gateway.md`
   - `docs/prps/plans/completed/github-issue-233-umu-gameid-http-resolver.plan.md`

3. **Backend command generation**
   - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`
   - `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`
   - `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`
   - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`
   - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs`
   - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs`
   - `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs`

4. **Schema, IPC, and frontend one-page UX**
   - `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`
   - `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`
   - `src/crosshook-native/src/types/profile.ts`
   - `src/crosshook-native/src/types/launch.ts`
   - `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`
   - `src/crosshook-native/src/components/LaunchSubTabs.tsx`
   - `src/crosshook-native/src/components/launch-subtabs/types.ts`
   - `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts`
   - `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`
   - `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`
   - `src/crosshook-native/src/components/launch-subtabs/SteamOptionsTabContent.tsx`
   - `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx`
   - `src/crosshook-native/src/components/library/launch/HeroLaunchSubTabsHost.tsx`

5. **Catalog precedent**
   - `src/crosshook-native/assets/default_optimization_catalog.toml`
   - `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`
   - `src/crosshook-native/src-tauri/src/commands/catalog.rs`
   - `src/crosshook-native/src/utils/optimization-catalog.ts`

## Documentation Gaps

- No dedicated document defines a command-arguments data model, catalog schema, conflict model, or quoting rules. Implementers must currently infer from `LaunchOptimizationsSection`, `custom_env_vars`, `GamescopeConfig.extra_args`, and `build_steam_launch_options_command`.
- Existing user docs describe env vars and wrappers before `%command%`, but not user-authored tokens after `%command%`. The feature should update `docs/features/steam-proton-trainer-launch.doc.md` and `docs/getting-started/quickstart.md`.
- `docs/getting-started/quickstart.md` still says the Launch Optimizations panel is limited to `proton_run`, while `docs/features/steam-proton-trainer-launch.doc.md` and current UI/code include `steam_applaunch`. Resolve this before using quickstart wording as a UX contract.
- No local docs specify Steam launch-option escaping for post-`%command%` arguments. `escape_steam_token` covers env values and gamescope args, but command/game args may need a clearer "single token vs user-entered string" decision.
- No doc states whether command arguments should apply to trainers. Current architecture suggests they are game executable arguments only: `steam_applaunch` places them after `%command%`, while `proton_run` should append to the game command in `proton_game.rs`; trainer builders should stay separate unless a separate trainer-args feature is planned.
- No doc states whether arguments should apply to `native`. User scope mentions Steam-style game arguments such as `--vulkan`; planning should explicitly decide whether native game paths get the same field or whether scope is limited to Windows game launch methods.
- No command-argument catalog asset exists. If modeled like optimizations, likely candidates are a new TOML asset plus core catalog loader and `get_*_catalog` IPC, but this is not documented today.
- External references named locally and relevant if implementation needs confirmation:
  - Tauri v2: linked from `README.md`.
  - Steam launch options workflow: represented locally by `docs/images/steam-properties-shortcut.png` and the Steam/Proton feature guide.
  - umu database / Open Wine Components: listed in `docs/prps/plans/completed/github-issue-233-umu-gameid-http-resolver.plan.md`.
  - Flatpak host command behavior: documented locally in ADR-0001; use upstream Flatpak docs only if ADR behavior is questioned.
