# Task Structure Analysis: command-arguments

## Executive Summary

Implement command arguments as a profile-scoped launch feature that starts with shared Rust/TS data contracts, then fans out into catalog/resolver work, launch command generation, thin IPC, one-page frontend controls, mocks, and documentation. The critical architectural constraint is that the same resolved argument tokens must feed Steam launch options, preview strings, and real Proton/umu execution while remaining stored as user-editable profile TOML, not SQLite.

The safest task structure is contract-first: define `launch.command_arguments` and `LaunchRequest.command_arguments`, then build a core resolver that every downstream surface consumes. After that, Steam `%command%` output, direct Proton/umu `Command::arg` appends, preview strings, browser mocks, and UI autosave can be implemented in mostly parallel slices.

## Recommended Phase Structure

### Phase 1: Data Contract and Persistence Boundary

- Add a profile TOML model such as `LaunchCommandArgumentsSection` in `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`.
- Add the matching request DTO in `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`.
- Add TypeScript parity in `src/crosshook-native/src/types/profile.ts` and `src/crosshook-native/src/types/launch.ts`.
- Update frontend normalization in `src/crosshook-native/src/hooks/profile/profileNormalize.ts`.
- Add TOML/model/request round-trip tests before command generation work depends on the shape.

Dependency: this phase must land before all IPC, preview, command, UI, and mock tasks.

### Phase 2: Core Catalog, Resolver, and Validation

- Add a dedicated command-argument catalog asset, likely `src/crosshook-native/assets/default_command_argument_catalog.toml`.
- Add a sibling core module to the optimization catalog/resolver pattern for loading entries, validating IDs, conflict detection, method applicability, and deterministic token output.
- Extend `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs` and `error.rs` for unknown IDs, duplicate IDs, conflicts, unsupported methods, invalid custom tokens, and size limits.
- Keep curated token resolution separate from launch optimizations because arguments produce argv tokens rather than env or wrapper directives.

Dependency: depends on Phase 1 contract. Command generation and UI catalog rendering depend on this.

### Phase 3: Backend Launch Surface Integration

- Update `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs` to append escaped argument tokens after `%command%`.
- Update `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs` to append resolved tokens after `normalized_game_path` with `Command::arg`.
- Update `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs` and `preview/builder.rs` so preview and Steam copy text match real launch behavior.
- Verify trainer builders in `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs` do not inherit game arguments.
- Decide explicitly whether native receives custom-only arguments; if included, update `native.rs` with method-gated curated support.

Dependency: depends on Phase 2 resolver. Steam, Proton/umu, preview, and trainer-isolation tests can be split across parallel tasks after the resolver API is stable.

### Phase 4: Thin IPC and Browser Dev Mocks

- Extend `src/crosshook-native/src-tauri/src/commands/launch/queries.rs` signatures only as needed, especially `build_steam_launch_options_command`.
- Add a thin catalog command in `src/crosshook-native/src-tauri/src/commands/catalog.rs` if the frontend needs catalog data directly.
- Add a narrow profile save command only if autosave cannot use an existing profile/launch save path cleanly.
- Register new snake_case commands in `src/crosshook-native/src-tauri/src/lib.rs`.
- Update `src/crosshook-native/src/lib/mocks/handlers/launch.ts` and relevant profile/catalog mock handlers so browser dev mode remains usable.

Dependency: backend contract and resolver must be stable. Mock updates can run in parallel with frontend UI once IPC names are decided.

### Phase 5: One-Page Frontend UI and Autosave

- Add command argument types/utilities, likely mirroring `src/crosshook-native/src/types/launch-optimizations.ts`, `src/crosshook-native/src/utils/optimization-catalog.ts`, and `src/crosshook-native/src/hooks/useLaunchOptimizationCatalog.ts`.
- Wire profile state through `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts` and `src/crosshook-native/src/components/launch-subtabs/types.ts`.
- Add a panel inside the existing launch configuration page, likely alongside `OptimizationsTabContent` or as a new in-page section in `LaunchSubTabs`, not a new route.
- Reuse the curated toggle pattern from `LaunchOptimizationsPanel.tsx` and the row editor pattern from `CustomEnvironmentVariablesSection.tsx` for custom argv tokens.
- Reuse the serialized launch-write queue in `useProfileLaunchAutosave.ts`; avoid an independent save path that can race with optimization, Gamescope, or MangoHud writes.
- Update `SteamLaunchOptionsPanel.tsx` and `SteamOptionsTabContent.tsx` so derived Steam copy text includes arguments after `%command%`.

Dependency: TS contract and catalog IPC must be stable. Component work and autosave wiring are separable but should integrate through one prop contract.

### Phase 6: Documentation and Final Validation

- Update `docs/features/steam-proton-trainer-launch.doc.md` and `docs/getting-started/quickstart.md` with the post-`%command%` behavior, token model, and method gating.
- Run targeted Rust tests during backend work, then full core tests:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- Run frontend checks from `src/crosshook-native`:
  - `npm test`
  - `npm run typecheck`
- Run host gateway verification if command construction changed:
  - `./scripts/check-host-gateway.sh`

Dependency: after implementation behavior and UI copy are finalized.

## Task Granularity Recommendations

- Keep contract tasks small but accept that shared model/type changes necessarily touch more than 1-3 files: Rust profile model, Rust launch request, TS profile type, TS launch type, normalization, and focused tests should be one coordinated task.
- Keep the catalog/resolver as one backend task because catalog parsing, conflict validation, method gating, and token resolution form one unit of correctness.
- Split command generation by surface:
  - Steam launch-options builder and escaping tests.
  - Proton/umu real command builder and argv-order tests.
  - Preview string/builder parity tests.
  - Trainer non-inheritance regression tests.
- Keep Tauri IPC and mock updates together by command surface so browser dev mode stays aligned with every new or changed command.
- Split frontend into three tasks once DTOs are stable:
  - Catalog hook/types/utilities.
  - LaunchSubTabs prop/autosave wiring.
  - Panel component and Steam derived preview integration.
- Keep docs as a late task because tokenization and native-method decisions affect user-facing wording.

## Dependency Analysis

- `LaunchCommandArgumentsSection` is the root dependency for all profile persistence, autosave, and request-building work.
- `LaunchRequest.command_arguments` is the root dependency for validation, launch preview, execution, mocks, and frontend `buildProfileLaunchRequest`.
- The core resolver should become the single semantic dependency for Steam options, Proton/umu execution, preview, and validation. Avoid each surface re-parsing or re-validating custom strings differently.
- Steam options depend on escaping policy. Real Proton/umu execution depends on structured `Command::arg` token boundaries. This is why custom args should be stored as token arrays, not a raw shell string.
- Frontend UI depends on catalog DTO shape and method applicability metadata. The UI should hide or disable unsupported curated entries rather than allow invalid selections that only fail at launch.
- Autosave depends on the existing serialized launch write chain in `useProfileLaunchAutosave.ts`; command arguments should enqueue through that chain to avoid clobbering simultaneous launch-section saves.
- Browser dev mocks depend on final IPC names and payload shape; they should be updated before relying on browser-only manual testing.
- Documentation depends on the final product decision for native support and the exact custom token UX.

## File-to-Task Mapping

### Contract and Persistence

- `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`: add persisted launch command-argument section.
- `src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs`: add collection-default merge only if arguments are allowed as collection defaults.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs`: add narrow save support only if needed for autosave.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/error.rs`: add save-time validation errors for unknown or invalid argument IDs if persistence validates catalog entries.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`: add request-side argument payload.
- `src/crosshook-native/src/types/profile.ts`: add frontend profile launch field.
- `src/crosshook-native/src/types/launch.ts`: add frontend launch request field.
- `src/crosshook-native/src/hooks/profile/profileNormalize.ts`: default older profiles to empty argument selections.

### Catalog and Validation

- `src/crosshook-native/assets/default_command_argument_catalog.toml`: new curated argument catalog.
- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs` or a new sibling module: load and expose command-argument catalog.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs`: pattern reference only; do not overload it unless extracting shared helpers is clearly worthwhile.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`: validate selected IDs, method support, conflicts, and custom tokens.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/error.rs`: add stable issue codes/messages.

### Backend Command Generation

- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`: append escaped tokens after `%command%`.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`: append tokens after the normalized game path.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs`: verify no separate change is needed because umu is selected inside the Proton-run path.
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs`: preserve gateway-aware construction; change only if helper signatures need resolved args.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/native.rs`: update only if native support is included.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs`: add regression coverage or explicit guard to prevent game args on trainer commands.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`: feed resolved args into preview.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`: render argument tokens in effective command strings.

### IPC and Mocks

- `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`: update Steam options command signature and preview request handling if needed.
- `src/crosshook-native/src-tauri/src/commands/catalog.rs`: expose command argument catalog if the UI fetches it through IPC.
- `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs`: pattern for any narrow command-argument save command.
- `src/crosshook-native/src-tauri/src/lib.rs`: register new commands.
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts`: update preview and Steam options mocks.
- `src/crosshook-native/src/lib/mocks/handlers/profile-mutations.ts`: update if adding a narrow profile save command.
- `src/crosshook-native/src/lib/mocks/README.md`: use as contract reference; update only if new mock conventions are introduced.

### Frontend UI

- `src/crosshook-native/src/utils/launch.ts`: copy profile arguments into `LaunchRequest`.
- `src/crosshook-native/src/utils/optimization-catalog.ts`: pattern for a command-argument catalog utility.
- `src/crosshook-native/src/hooks/useLaunchOptimizationCatalog.ts`: pattern for a command-argument catalog hook.
- `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts`: assemble argument state and handlers.
- `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts`: enqueue command-argument autosave through the existing write chain.
- `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts`: add debounced effect if using narrow autosave.
- `src/crosshook-native/src/components/LaunchSubTabs.tsx`: mount the one-page argument panel.
- `src/crosshook-native/src/components/launch-subtabs/types.ts`: add props and tab/section contract.
- `src/crosshook-native/src/components/launch-subtabs/useTabVisibility.ts`: method-gate the panel if it is exposed only for Steam/Proton methods.
- `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`: curated toggle UX reference.
- `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: custom token row editor reference.
- `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`: include arguments in derived copy/paste output.

### Tests

- `src/crosshook-native/crates/crosshook-core/src/profile/models/tests/launch_section.rs`: empty omission and populated TOML serialization.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/optimizations.rs` or a new command-arguments test file: narrow save and validation behavior.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/tests/serde_roundtrip.rs`: request DTO round-trip.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/tests/method_validation.rs`: unsupported method and validation errors.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`: `%command% <args>` escaping and ordering.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_game.rs`: direct Proton argv order.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_game_umu.rs`: umu argv order without inserting `run`.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/tests/proton_trainer.rs`: trainer non-inheritance.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/tests/command_string.rs`: preview parity.
- Existing frontend tests under `src/crosshook-native/src/components/**/__tests__` and `src/crosshook-native/src/utils/**/__tests__`: add focused UI and request-building coverage where local patterns exist.

## Optimization Opportunities

- Reuse optimization catalog concepts without coupling to optimization directives. Shared helper extraction is reasonable only for generic ID/conflict/method validation, not for env/wrapper-specific behavior.
- Store custom arguments as `Vec<String>`/`string[]` tokens to avoid writing a shell parser and to make real launches safer through `Command::arg`.
- Use one resolver output type for all consumers, such as resolved curated IDs, custom tokens, combined tokens, and warnings. That reduces drift between Steam, preview, and runtime builders.
- Keep the Steam builder signature close to its current shape by adding one argument payload parameter rather than replacing it with a full `LaunchRequest`, unless preview and Tauri call sites become simpler with full request input.
- Avoid SQLite migration work in the first implementation. A static embedded TOML catalog plus IPC exposure is sufficient unless later catalog indexing/offline diagnostics require metadata storage.
- Preserve frontend momentum by placing the first UI inside the existing optimizations/launch page surface instead of creating a route, router state, or new page layout.
- Gate native support deliberately. Custom-only native arguments are cheap architecturally, but curated Proton/Steam presets should remain method-gated to avoid misleading users.
- Add backend tests before UI polish because the highest-risk bugs are invisible ordering bugs: args before `%command%`, args before the game executable, or trainer paths inheriting game args.

## Implementation Strategy Recommendations

- Start with a short contract checkpoint: choose exact field names, likely `launch.command_arguments.enabled_argument_ids` and `launch.command_arguments.custom_args`, plus the same nested shape on `LaunchRequest`.
- Implement validation in core before command builders. The UI should improve ergonomics, but launch safety should not depend on frontend filtering.
- Use deterministic order everywhere: curated catalog tokens in catalog order, then custom user tokens in user order.
- Treat Steam output as display/string generation only; treat Proton/umu/native execution as structured argv construction only.
- Keep `src-tauri` thin. New commands should be catalog/list/save pass-throughs and should not parse, resolve, or construct argument strings.
- Update preview, Steam copy/paste, and real execution in the same phase or same PR slice. Shipping any one without the others creates user-visible drift.
- Preserve Flatpak host gateway rules by appending arguments to commands returned by existing gateway-aware builders and running `./scripts/check-host-gateway.sh` after touching command construction.
- Make autosave use the existing `launchProfileWriteChainRef` queue. This feature edits the same `launch` TOML section as optimizations, Gamescope, and MangoHud.
- Validate with focused tests first, then full commands:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
  - `cd src/crosshook-native && npm test`
  - `cd src/crosshook-native && npm run typecheck`
  - `./scripts/check-host-gateway.sh`
