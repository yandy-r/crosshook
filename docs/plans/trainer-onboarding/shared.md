# Trainer Onboarding

CrossHook's trainer-onboarding feature adds a first-run modal wizard with system-level readiness checks, trainer acquisition guidance, and a guided profile creation workflow. The backend adds a minimal `crosshook-core/src/onboarding/` module (2 files) with free functions composing existing Steam/Proton discovery, three sync Tauri IPC commands in `commands/onboarding.rs`, and an `onboarding_completed: bool` flag in `AppSettingsData` (settings.toml). The frontend adds a stage-machine hook (`useOnboarding.ts`) mirroring the `useInstallGame.ts` pattern, a portal-based modal wizard following `ProfileReviewModal.tsx`'s focus-trap/ARIA conventions, and per-check readiness status cards reusing the existing `HealthIssue` type.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/lib.rs: Module registry — add `pub mod onboarding;`
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `AppSettingsData` struct with `#[serde(default)]` — add `onboarding_completed: bool`; `SettingsStore` load/save pattern
- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs: `discover_steam_root_candidates()` — returns `Vec<PathBuf>` of Steam roots; reuse for `steam_installed` check
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: `discover_compat_tools()` — returns `Vec<ProtonInstall>`; reuse for `proton_available` check
- src/crosshook-native/crates/crosshook-core/src/profile/health.rs: `HealthIssue` and `HealthIssueSeverity` types — reused directly by `ReadinessCheckResult`
- src/crosshook-native/crates/crosshook-core/src/install/service.rs: `validate_optional_trainer_path()` at ~line 32 and `is_windows_executable()` at line 292 — trainer validation patterns
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `TrainerLoadingMode` enum (`SourceDirectory`/`CopyToPrefix`) — used as-is in guidance content
- src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs: `attempt_auto_populate()` — called during wizard profile creation step via existing `auto_populate_steam` command
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs: `normalize_subscription()` — needs W-1 branch validation + W-2 URL scheme allowlist fixes
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: `copy_dir_all()` — needs A-1 symlink skip fix; `stage_trainer_into_prefix()` staging logic
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs: `escape_desktop_exec_argument()` — needs A-4 `%` escaping fix
- src/crosshook-native/src-tauri/src/lib.rs: App setup — store registration, startup event emission pattern (auto-load-profile at 350ms delay), command registration in `invoke_handler!`
- src/crosshook-native/src-tauri/src/startup.rs: `resolve_auto_load_profile_name()` — startup settings reading pattern
- src/crosshook-native/src-tauri/src/commands/mod.rs: Command module registry — add `pub mod onboarding;`
- src/crosshook-native/src-tauri/src/commands/settings.rs: Canonical sync command pattern with `State<'_, SettingsStore>` injection
- src/crosshook-native/src-tauri/src/commands/steam.rs: `default_steam_client_install_path()` and `list_proton_installs()` — discovery command patterns
- src/crosshook-native/src-tauri/src/commands/shared.rs: `sanitize_display_path()` — must apply to all path strings in readiness check messages
- src/crosshook-native/src-tauri/capabilities/default.json: `core:default` + `dialog:default` — no new permissions needed for v1
- src/crosshook-native/src/App.tsx: Root shell — `AppShell` holds route state; add `onboarding-check` event listener and conditional wizard render
- src/crosshook-native/src/hooks/useInstallGame.ts: **Canonical stage-machine hook** — mirror this pattern for `useOnboarding.ts`
- src/crosshook-native/src/hooks/useGamepadNav.ts: Gamepad navigation — wizard uses `data-crosshook-focus-root="modal"` to trap focus
- src/crosshook-native/src/components/ProfileReviewModal.tsx: Portal-based modal reference — `createPortal`, `inert`/`aria-hidden` siblings, focus trap, `data-crosshook-modal-close`
- src/crosshook-native/src/components/AutoPopulate.tsx: Steam auto-discovery component — compose in wizard profile creation step
- src/crosshook-native/src/components/ProfileFormSections.tsx: Profile editor form — compose in wizard review step
- src/crosshook-native/src/components/ui/InstallField.tsx: File path input with browse + validation — reuse for trainer path selection
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: Collapsible content — for loading mode progressive disclosure
- src/crosshook-native/src/components/layout/ControllerPrompts.tsx: Gamepad button hints — extend with `confirmLabel`/`backLabel` override props
- src/crosshook-native/src/types/health.ts: TypeScript `HealthIssue` interface — import directly for readiness check types
- src/crosshook-native/src/types/index.ts: Type re-exports — add `export * from './onboarding';`
- src/crosshook-native/src/styles/variables.css: CSS variables including `--crosshook-touch-target-min` (56px in controller mode)

## Relevant Tables

- profiles: Profile identity (profile_id, current_filename) — used for empty-state banner check (`profile_store.list()` preferred over SQLite query)
- health_snapshots: Per-profile health status — post-onboarding health check writes here for newly created profile
- version_snapshots: Trainer file hash tracking — Phase 4 records initial `trainer_file_hash` on profile creation

## Relevant Patterns

**Tauri IPC Command Pattern**: Sync commands use `fn(State<'_, StoreType>) -> Result<T, String>` with `.map_err(|e| e.to_string())`. Async uses `spawn_blocking`. All registered in `invoke_handler![]` in `lib.rs`. See [src/crosshook-native/src-tauri/src/commands/settings.rs](src/crosshook-native/src-tauri/src/commands/settings.rs) for the minimal example.

**Stage-Machine Hook Pattern**: Stage is a string union type driving all UI state. Pure functions derive `statusText`/`hintText`/`actionLabel` from stage. Factory functions create initial state. `useCallback`-wrapped async driver transitions stages sequentially. See [src/crosshook-native/src/hooks/useInstallGame.ts](src/crosshook-native/src/hooks/useInstallGame.ts) — the `useOnboarding.ts` hook mirrors this exactly.

**Portal Modal Pattern**: Modals use `createPortal` to a `document.body`-appended div. Set `inert`/`aria-hidden` on siblings when open. Focus trap via `data-crosshook-focus-root="modal"`. Close button carries `data-crosshook-modal-close` for gamepad B-button handler. See [src/crosshook-native/src/components/ProfileReviewModal.tsx](src/crosshook-native/src/components/ProfileReviewModal.tsx).

**Startup Event Pattern**: Backend emits events via `tauri::async_runtime::spawn` with `sleep(Duration::from_millis(350))` to ensure React has mounted. Frontend listens with `listen<T>(event_name)` in `useEffect`. See [src/crosshook-native/src-tauri/src/lib.rs](src/crosshook-native/src-tauri/src/lib.rs) lines 59-70.

**Settings Load-Mutate-Save Pattern**: `SettingsStore::save()` is full-struct overwrite. Always `load() -> mutate field -> save()`. Never construct a fresh default. See [src/crosshook-native/src-tauri/src/commands/settings.rs](src/crosshook-native/src-tauri/src/commands/settings.rs).

**HealthIssue Reuse Pattern**: Readiness checks return `Vec<HealthIssue>` reusing the existing type from `profile/health.rs` — same fields, same IPC serialization, same frontend type. No parallel `ReadinessCheck` type needed.

## Relevant Docs

**docs/plans/trainer-onboarding/feature-spec.md**: You _must_ read this — definitive spec with data models, API signatures, 8 files to create, 6 to modify, 4-phase rollout, and all decisions.

**CLAUDE.md**: You _must_ read this when working on any CrossHook component — architecture overview, module paths, build commands, code conventions.

**docs/plans/trainer-onboarding/research-technical.md**: You _must_ read this when implementing backend architecture — component diagram, IPC command signatures, startup event flow, data models.

**docs/plans/trainer-onboarding/research-business.md**: You _must_ read this when implementing business logic — BR-1 through BR-10 are requirements, not guidance. BR-2 (loading mode validation per mode) and BR-8 (persistence flag) are critical.

**docs/plans/trainer-onboarding/research-ux.md**: You _must_ read this when implementing frontend UI — wizard step flow, Steam Deck gamepad requirements (56px targets, B=back, focus trap), progressive disclosure, inline validation patterns.

**docs/plans/trainer-onboarding/research-security.md**: You _must_ read this when implementing Phase 0 security fixes — W-1 (git branch injection), W-2 (URL scheme allowlist), advisory items A-1 through A-10.

**docs/plans/trainer-onboarding/research-practices.md**: Reference for reusable code inventory, KISS assessment, and build-vs-depend decisions.
