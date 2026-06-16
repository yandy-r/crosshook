# Context Analysis: command-arguments

## Executive Summary

Command arguments should be profile-scoped launch preferences stored in profile TOML under `GameProfile.launch`, then copied into `LaunchRequest` for validation, preview, Steam launch-options generation, and real Proton/umu execution. Treat curated entries as a dedicated argument catalog, not launch optimizations, because they produce game argv tokens appended after `%command%` for Steam and after the game executable for direct Proton/umu. Keep parsing, validation, catalog resolution, and command construction in `crosshook-core`; `src-tauri` should only expose thin IPC and persistence commands.

## Architecture Context

- **System Structure**: CrossHook launch behavior is centered in `src/crosshook-native/crates/crosshook-core`, with `src-tauri` as a thin IPC layer and React/TypeScript editing profile-backed launch state. Add command arguments as a sibling to `launch.optimizations` and `launch.custom_env_vars`, with a core-owned catalog/resolver and frontend one-page controls inside the existing launch configuration surface.
- **Data Flow**: Profile TOML `launch.command_arguments` should hold selected curated IDs plus custom argv tokens. Frontend normalization loads empty defaults, `src/crosshook-native/src/utils/launch.ts` copies effective values into `LaunchRequest`, core validation resolves curated IDs and custom tokens into ordered argv, then preview, Steam copy/paste, and real launch builders consume the same resolved data.
- **Integration Points**: Steam launch options append escaped tokens after `%command%`; direct Proton and `umu-run` append `Command::arg` tokens after the normalized game path in `proton_game.rs`; trainer builders must not inherit game args. Browser dev mocks need DTO/IPC parity for any new catalog, save, preview, or Steam-options command surface.

## Critical Files Reference

- `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`: profile TOML launch schema; natural home for `LaunchCommandArgumentsSection` with serde defaults and empty omission.
- `src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs`: effective profile/collection-default merge behavior if command arguments become collection defaults.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs`: profile load/save and narrow launch-section persistence precedent.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`: Rust `LaunchRequest` DTO that must carry effective selected IDs and custom argument tokens.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`: central validation path for launch method support, unknown IDs, conflicts, duplicates, and custom token safety.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs`: stable issue codes/messages for argument validation failures.
- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: embedded catalog loader pattern to mirror for command-argument catalog data.
- `src/crosshook-native/assets/default_optimization_catalog.toml`: TOML catalog precedent for IDs, labels, descriptions, categories, conflicts, and applicable methods.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs`: deterministic catalog-order resolution and method/conflict validation pattern.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`: Steam `%command%` builder and escaping tests; append game args after `%command%`.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`: real direct Proton/umu game command builder; append args after the game executable path.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs`: trainer path to protect from accidental game-argument inheritance.
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs`: gateway-aware Proton/umu/gamescope command construction; preserve structured `.arg(...)` use.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`: preview assembly; must feed resolved args into effective command and Steam options.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`: human-readable command strings for Proton/umu/native/Steam preview parity.
- `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`: thin IPC for validation, preview, and Steam-options generation.
- `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs`: narrow launch-section save-command precedent.
- `src/crosshook-native/src-tauri/src/commands/catalog.rs`: catalog IPC precedent for exposing backend catalog payloads.
- `src/crosshook-native/src/types/profile.ts`: frontend profile schema; add normalized profile launch fields.
- `src/crosshook-native/src/types/launch.ts`: frontend `LaunchRequest` parity with Rust DTO.
- `src/crosshook-native/src/utils/launch.ts`: frontend handoff that builds launch/preview requests from profiles.
- `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts`: serialized launch autosave queue to reuse for command-argument saves.
- `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts`: launch page prop/state assembly for one-page UI wiring.
- `src/crosshook-native/src/components/LaunchSubTabs.tsx`: shared launch configuration host where the one-page UI should live.
- `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`: grouped curated-toggle UX pattern.
- `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`: derived Steam copy/paste preview that must reflect args after `%command%`.
- `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: editable row/token UI pattern for custom arguments.
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts`: browser dev mock coverage for preview and Steam-options DTO changes.

## Patterns to Follow

- **Profile TOML Preferences**: Store selected curated IDs and custom argument tokens under `GameProfile.launch` with serde defaults and `skip_serializing_if`; see `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`.
- **Dedicated Catalog Resolver**: Use a new command-argument catalog/resolver instead of extending optimization directives; mirror `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs` and `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs`.
- **Deterministic Ordering**: Resolve curated tokens in catalog order, then append custom tokens in user order; follow optimization directive ordering in `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs`.
- **Preview and Execution Parity**: Update preview string builders and real `Command` builders together; examples are `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs` and `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`.
- **Thin IPC**: Keep `src-tauri` commands as pass-throughs that map errors and register snake_case commands; see `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`.
- **Serialized Launch Autosaves**: Reuse the launch-section autosave queue to avoid clobbering simultaneous optimization/gamescope/MangoHud saves; see `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts`.
- **One-Page Launch UX**: Add controls inside the existing launch page/subtab system, using the curated toggle and row editor patterns rather than a new route; see `src/crosshook-native/src/components/LaunchSubTabs.tsx`.
- **Host Gateway Preservation**: Append target args only to commands built through existing platform helpers; do not introduce direct host-tool `Command::new` calls. See `docs/architecture/adr-0001-platform-host-gateway.md`.

## Cross-Cutting Concerns

- Persistence boundary: selected curated IDs and custom tokens are user-editable profile preferences in TOML; resolved strings/previews are runtime-only; no SQLite table is needed for the first implementation.
- Token model: store custom arguments as structured argv tokens, not one raw shell string, to avoid inconsistent shell parsing between Steam copy text and `Command::arg`.
- Validation: reject unknown curated IDs, duplicate IDs, catalog conflicts, unsupported launch methods, NUL/control characters, empty custom tokens, and excessive token count/length; allow common CLI punctuation such as `--flag=value`, `+set`, paths with spaces, and `-dx11`.
- Method gating: first-class support should cover `steam_applaunch` and `proton_run`; `umu-run` is covered through the `proton_run` builder. Native support should be explicit, likely custom-only if included, with Proton/Steam curated entries hidden or warned.
- Trainer isolation: game arguments apply to game execution only; trainer builders remain separate unless a future trainer-arguments feature is scoped.
- Escaping: real launches use structured args; only Steam launch-options output needs string escaping after `%command%`, with focused tests for spaces, quotes, `$`, `;`, `|`, `&`, `<`, and `>`.
- Browser dev mode: any new IPC command or DTO field must be reflected in mock handlers and continue using `callCommand()` rather than raw `invoke()`.
- Documentation: implementation should later update `docs/features/steam-proton-trainer-launch.doc.md` and `docs/getting-started/quickstart.md` because current docs explain env/wrappers before `%command%` but not game args after it.

## Parallelization Opportunities

- Core model/catalog/validation work can proceed independently from frontend UI once the `LaunchCommandArgumentsSection` and `LaunchRequest` shape are agreed.
- Command-generation updates can be split by surface: Steam options, Proton/umu runtime builder, and preview command strings, then reconciled through shared resolver tests.
- Frontend work can split into catalog hook/types, launch autosave wiring, and the one-page panel component after DTO names are stable.
- Browser mock updates can be handled in parallel with frontend wiring because they mirror the same IPC contract.
- Documentation/test updates can run after implementation details settle, especially the tokenization policy and native-method decision.

## Implementation Constraints

- Do not store selected arguments in SQLite; profile TOML is the source of truth for per-profile preferences.
- Do not overload launch optimizations for argv arguments; create a sibling argument catalog because output semantics differ from env/wrapper directives.
- Do not shell-concatenate real launch commands; append each argument with `Command::arg`.
- Do not apply game arguments to trainer launches.
- Do not add a separate route; the feature scope requires a one-page UI inside the existing launch configuration surface.
- Preserve Steam append position exactly: optimization env/wrappers first, `%command%`, then curated/custom game arguments.
- Preserve Proton/umu append position exactly: host/gamescope/wrapper/proton-or-umu setup first, game executable path, then curated/custom game arguments.
- Keep `crosshook-core` authoritative and `src-tauri` thin; IPC command names must remain snake_case and Serde DTOs must match frontend types.
- Keep Flatpak host-tool boundary intact by using existing gateway-aware builders and running `scripts/check-host-gateway.sh` when command construction changes.

## Key Recommendations

- Add `launch.command_arguments` with `selected_argument_ids: Vec<String>` and `custom_args: Vec<String>` in profile TOML, plus TS normalization defaults.
- Add a `default_command_argument_catalog.toml` and core resolver with categories, labels, descriptions, conflicts, method applicability, and one-or-more argv tokens per entry.
- Thread the same argument data through `LaunchRequest`, validation, preview, Steam launch-options command generation, and real `proton_run` execution.
- Use deterministic merge order: curated catalog tokens first, custom user tokens second.
- Add a focused save path only if the autosave UX needs it; otherwise keep arguments on the existing launch/profile save path but still reuse serialized launch writes.
- Gate curated entries by launch method and make native support an explicit product choice before implementation.
- Build tests around TOML round-trip/empty omission, catalog resolution, validation errors, Steam escaping after `%command%`, Proton/umu argv order, gamescope ordering, and trainer non-inheritance.
