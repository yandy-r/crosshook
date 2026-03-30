# Trainer Onboarding Implementation Plan

The trainer-onboarding feature adds a first-run modal wizard with system readiness checks, trainer acquisition guidance, and a chained auto-populate → profile → launch workflow. Implementation composes existing primitives (`HealthIssue` type, `discover_steam_root_candidates`/`discover_compat_tools` functions, `useInstallGame.ts` stage-machine hook, `ProfileReviewModal.tsx` portal modal) with zero new dependencies across 8 new files and 6 modified files. Phase 0 ships security hardening for community taps (W-1 branch injection, W-2 URL scheme allowlist) as a prerequisite since onboarding encourages tap usage. The critical path runs: security fixes → core module → settings flag → Tauri commands → `useOnboarding.ts` hook → wizard components → polish.

## Critically Relevant Files and Documentation

- docs/plans/trainer-onboarding/feature-spec.md: Definitive spec — data models, API signatures, business rules BR-1 through BR-9, 4-phase rollout
- docs/plans/trainer-onboarding/research-technical.md: Architecture diagram, IPC command design, startup event flow, data models
- docs/plans/trainer-onboarding/research-business.md: Business rules, user stories, edge cases, domain model
- docs/plans/trainer-onboarding/research-ux.md: Wizard flow, Steam Deck gamepad requirements, progressive disclosure, inline validation
- docs/plans/trainer-onboarding/research-security.md: W-1/W-2 WARNING fixes, advisory items A-1 through A-10
- docs/plans/trainer-onboarding/research-practices.md: Reusable code inventory, KISS assessment, testability patterns
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `AppSettingsData` struct — add `onboarding_completed: bool`
- src/crosshook-native/crates/crosshook-core/src/profile/health.rs: `HealthIssue` + `HealthIssueSeverity` — reused directly by `ReadinessCheckResult`
- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs: `discover_steam_root_candidates()` — backbone of `steam_installed` check
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: `discover_compat_tools()` — backbone of `proton_available` check
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs: `normalize_subscription()` — Phase 0 security fixes
- src/crosshook-native/src-tauri/src/lib.rs: Command registration, startup event emission pattern (lines 59-70)
- src/crosshook-native/src-tauri/src/commands/settings.rs: Canonical sync command pattern with `State<'_, SettingsStore>`
- src/crosshook-native/src-tauri/src/commands/shared.rs: `sanitize_display_path()` — apply to all path strings in readiness messages
- src/crosshook-native/src/hooks/useInstallGame.ts: Canonical stage-machine hook — mirror for `useOnboarding.ts`
- src/crosshook-native/src/components/ProfileReviewModal.tsx: Portal modal pattern — mirror for `OnboardingWizard.tsx`
- src/crosshook-native/src/App.tsx: Root shell — add event listener and conditional wizard render

## Implementation Plan

### Phase 0: Security Hardening

#### Task 0.1: Fix git branch injection and URL scheme allowlist in community taps Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/trainer-onboarding/research-security.md (W-1, W-2 sections)
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/community/taps.rs

Both W-1 and W-2 modify `normalize_subscription()` — implement together to avoid merge conflicts.

**W-1 (branch injection):** Add a private `validate_branch_name()` helper that rejects branch names starting with `-` and allows only `[a-zA-Z0-9/._-]` characters, max 200 chars. Call it from `normalize_subscription()`. Also add `"--"` end-of-options separator in the fetch command at line ~240: `["fetch", "--prune", "origin", "--", branch]`. Note: do NOT insert `"--"` between `--branch` and its value in `clone_tap` — `--branch` expects the branch name as its immediate next argument. The `validate_branch_name()` helper provides the W-1 protection for clone; the `--` separator is only needed for fetch where the branch is a positional argument.

**W-2 (URL scheme):** Add a private `validate_tap_url()` helper that requires URL to start with `https://` or `ssh://git@`. Call from `normalize_subscription()`. Return a clear error string like `"tap URL must use https:// or ssh://git@"`.

Add `#[cfg(test)] mod tests` with test cases: valid branch names, branch starting with `--`, branch with special chars, valid HTTPS URL, `file://` URL, `git://` URL.

#### Task 0.2: Add symlink skip in CopyToPrefix staging Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/trainer-onboarding/research-security.md (A-1 section)
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs

In `copy_dir_all()` (lines ~310-326), add a symlink check before the `is_dir()`/`fs::copy()` branch:

```rust
if source_path.is_symlink() {
    tracing::debug!(path = %source_path.display(), "skipping symlink during trainer staging");
    continue;
}
```

Add a test case in the existing test module that creates a symlink in a temp directory and verifies it is skipped during staging.

#### Task 0.3: Fix desktop Exec `%` escaping in launcher export Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/trainer-onboarding/research-security.md (A-4 section)
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs

In `escape_desktop_exec_argument()` (lines ~593-598), add `%` → `%%` replacement alongside the existing `\`, space, and `"` replacements. The `%` character has special meaning in `.desktop` Exec lines (`%f`, `%u` are URI substitution tokens). Add a test case verifying `%` is doubled.

### Phase 1: Backend Readiness Checks and Settings

#### Task 1.1: Create onboarding core module with readiness checks Depends on [0.1, 0.2, 0.3]

**READ THESE BEFORE TASK**

- docs/plans/trainer-onboarding/feature-spec.md (Technical Specifications section)
- src/crosshook-native/crates/crosshook-core/src/profile/health.rs
- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs
- src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/lib.rs

Create `onboarding/mod.rs` with types: `ReadinessCheckResult { checks: Vec<HealthIssue>, all_passed: bool, critical_failures: usize, warnings: usize }`, `TrainerGuidanceEntry { id, title, description, when_to_use, examples }`, `TrainerGuidanceContent { loading_modes, trainer_sources, verification_steps }`. Re-export `pub use readiness::check_system_readiness;`.

Create `onboarding/readiness.rs` with a sync free function `pub fn check_system_readiness() -> ReadinessCheckResult` that:

1. Calls `discover_steam_root_candidates("", &mut diagnostics)` — empty vec = `Error` severity `HealthIssue` with field `"steam_installed"`
2. Calls `discover_compat_tools(&steam_roots, &mut diagnostics)` — empty vec = `Error` severity with field `"proton_available"`
3. Scans all `steam_root/steamapps/compatdata/*/pfx` directories with `fs::read_dir().any()` — no dirs found = `Warning` severity with field `"game_launched_once"`
4. Returns `Info` severity for `"trainer_available"` (always informational at system check stage)

Use `HealthIssue` from `crate::profile::health`. For path strings in `HealthIssue.path`, implement a simple inline home-dir replacement (replace `$HOME` prefix with `~`) directly in `readiness.rs` — do NOT import `sanitize_display_path()` from `commands/shared.rs` as that lives in the Tauri layer and would create a forbidden crate dependency inversion. The Tauri command layer (Task 1.3) will also apply `sanitize_display_path()` as a second pass, but the core function should produce clean paths independently. Log diagnostics with `tracing::debug!`.

Add `pub mod onboarding;` to `lib.rs`.

Write inline `#[cfg(test)] mod tests` using `tempfile::tempdir()` — create fake Steam directory structures and verify check results. Test: all-pass case, no-Steam case, no-Proton case, no-compatdata case.

#### Task 1.2: Add onboarding_completed flag to settings Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs

Add `pub onboarding_completed: bool` to `AppSettingsData` struct. The struct already has `#[serde(default)]` — no per-field annotation needed. `bool` defaults to `false`, which is correct for first-run detection.

Update the existing `save_and_load_round_trip` test to include `onboarding_completed: true` in the test struct and verify it round-trips. Add a test verifying that deserializing TOML without the field yields `false` (the `#[serde(default)]` behavior).

#### Task 1.3: Create Tauri onboarding commands and startup event Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/settings.rs
- src/crosshook-native/src-tauri/src/commands/steam.rs
- src/crosshook-native/src-tauri/src/lib.rs (lines 59-70 for startup event pattern, lines 123-193 for command registration)
- src/crosshook-native/src-tauri/src/commands/shared.rs

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/onboarding.rs

Files to Modify

- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

Create `commands/onboarding.rs` with three sync commands:

1. `check_readiness() -> Result<ReadinessCheckResult, String>` — calls `crosshook_core::onboarding::check_system_readiness()`. No `State` parameter needed (pure filesystem). Apply `sanitize_display_path()` to all `HealthIssue.path` fields before returning.

2. `dismiss_onboarding(settings_store: State<'_, SettingsStore>) -> Result<(), String>` — load-mutate-save: `let mut s = store.load()?; s.onboarding_completed = true; store.save(&s)?`. NEVER construct a fresh default — this would erase all other settings.

3. `get_trainer_guidance() -> TrainerGuidanceContent` — returns static compiled content with `&'static str` constants for loading modes (SourceDirectory, CopyToPrefix) and trainer sources (FLiNG, WeMod). No `State`, no `Result`.

Add `pub mod onboarding;` to `commands/mod.rs`.

In `lib.rs`: register all three commands in `invoke_handler![]`. Add startup event emission for `onboarding-check` following the `auto-load-profile` pattern at 350ms delay. The event payload should include both `show: bool` (from `!settings.onboarding_completed`) and `has_profiles: bool` (from `profile_store.list()` length) — lets frontend decide modal vs banner without a second IPC call.

Add compile-time command signature tests following `commands/settings.rs:38-51` pattern.

#### Task 1.4: Create TypeScript onboarding types Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/health.ts
- src/crosshook-native/src/types/install.ts (for pattern reference)
- docs/plans/trainer-onboarding/feature-spec.md (TypeScript Types section)

**Instructions**

Files to Create

- src/crosshook-native/src/types/onboarding.ts

Files to Modify

- src/crosshook-native/src/types/index.ts

Create `types/onboarding.ts` with:

- `import type { HealthIssue } from './health';`
- `ReadinessCheckResult { checks: HealthIssue[], all_passed: boolean, critical_failures: number, warnings: number }`
- `OnboardingWizardStage = 'readiness_check' | 'trainer_guidance' | 'profile_creation' | 'completed'`
- `TrainerGuidanceEntry { id: string, title: string, description: string, when_to_use: string, examples: string[] }`
- `TrainerGuidanceContent { loading_modes: TrainerGuidanceEntry[], trainer_sources: TrainerGuidanceEntry[], verification_steps: string[] }`
- `OnboardingCheckPayload { show: boolean, has_profiles: boolean }` (startup event payload type)

Add `export * from './onboarding';` to `types/index.ts`.

#### Task 1.5: Add empty-state banner on ProfilesPage Depends on [1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- docs/plans/trainer-onboarding/research-ux.md (Empty State Design section)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

When the profile list is empty (`profiles.length === 0`), render an empty-state CTA card with:

- Heading: "No profiles yet"
- Copy: "Set up your first game + trainer combo in minutes."
- Primary button: "Start Guided Setup" (will open wizard — wire in Phase 2)
- Secondary text link: "Create manually" (navigates to profile creation)

Use `div.crosshook-panel` with `crosshook-card-padding`. Verify the exact accent background token name exists in `src/crosshook-native/src/styles/variables.css` before use — look for `--crosshook-color-accent-soft` or the closest equivalent. Do NOT use `PageBanner` — that's reserved for page headers. The banner disappears once any profile exists.

### Phase 2: Guided Workflow UI

#### Task 2.1: Create useOnboarding stage-machine hook Depends on [1.3, 1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useInstallGame.ts (canonical pattern — study in detail)
- src/crosshook-native/src/types/onboarding.ts

**Instructions**

Files to Create

- src/crosshook-native/src/hooks/useOnboarding.ts

Mirror the `useInstallGame.ts` pattern exactly:

1. Pure module-scope derive functions: `deriveStatusText(stage)`, `deriveHintText(stage)`, `deriveActionLabel(stage)`
2. Factory function: `createInitialOnboardingState()`
3. Hook function `useOnboarding()` returning `UseOnboardingResult`:
   - `stage: OnboardingWizardStage` state
   - Derived booleans: `isReadinessCheck`, `isTrainerGuidance`, `isProfileCreation`, `isCompleted`
   - `readinessResult: ReadinessCheckResult | null`
   - `statusText`, `hintText`, `actionLabel` (derived from stage)
   - `useCallback`-wrapped async driver: `runChecks()` — invokes `check_readiness`, sets result
   - `advance()` — transitions to next stage
   - `dismiss()` — invokes `dismiss_onboarding`, sets stage to completed
   - `reset()` — returns all state to initial

No React Context — single consumer (`OnboardingWizard.tsx`). No `useReducer` — `useState` is sufficient for linear stages.

#### Task 2.2: Extend ControllerPrompts with override props Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/layout/ControllerPrompts.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/layout/ControllerPrompts.tsx

Add optional override props: `confirmLabel?: string`, `backLabel?: string`, `showBumpers?: boolean`. When provided, these override the default button hint labels. Default behavior is preserved when props are omitted. This enables the wizard to show step-contextual prompts (e.g., "A=Run Checks" on Step 1, "A=Next" on Steps 2-4, "B=Previous Step" instead of "B=Back").

#### Task 2.3: Create OnboardingWizard modal component Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileReviewModal.tsx (portal modal pattern)
- src/crosshook-native/src/hooks/useGamepadNav.ts (focus-root attribute)
- docs/plans/trainer-onboarding/research-ux.md (wizard steps, accessibility, gamepad)

**Instructions**

Files to Create

- src/crosshook-native/src/components/OnboardingWizard.tsx

Follow `ProfileReviewModal.tsx` portal pattern:

- Create portal host div on mount (not conditional on open state)
- Set `inert`/`aria-hidden` on body siblings when open
- Apply `data-crosshook-focus-root="modal"` on wizard surface
- Apply `data-crosshook-modal-close` on back/close buttons
- `role="dialog"`, `aria-modal="true"`, `aria-labelledby` with `useId()`
- Restore focus on close

Wizard has 3 visible steps (readiness, guidance, profile creation) plus completion state, driven by `useOnboarding()` hook. Each step is a section within the modal, conditionally rendered based on `stage`.

**Composition model**: The wizard renders step content inline — NOT by importing `ReadinessChecklist.tsx` (Task 2.4) or `TrainerGuidance.tsx` (Task 3.2) directly. Instead, use placeholder content for each step initially (e.g., a simple list for readiness checks, basic radio buttons for loading mode). Tasks 2.4 and 3.2 will later replace these placeholders with the real components. This avoids serializing all component work behind the wizard shell. Include:

- Numbered step indicator (1 of 3)
- Per-step content area
- Navigation buttons: "Next"/"Back"/"Skip Setup"/"Complete"
- `ControllerPrompts` with step-contextual override props
- Touch targets: 56px minimum via `--crosshook-touch-target-min`

Props: `open: boolean`, `onComplete: () => void`, `onDismiss: () => void`.

#### Task 2.4: Create ReadinessChecklist component Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/AutoPopulate.tsx (FieldCard state pattern reference)
- src/crosshook-native/src/types/health.ts

**Instructions**

Files to Create

- src/crosshook-native/src/components/ReadinessChecklist.tsx

Renders per-check status cards from `ReadinessCheckResult.checks`. Each `HealthIssue` maps to a card:

- `severity: 'info'` → green checkmark with `--crosshook-color-success`
- `severity: 'warning'` → yellow warning with `--crosshook-color-warning`
- `severity: 'error'` → red cross with `--crosshook-color-danger`
- Display `message` as primary text, `remediation` as secondary hint text

Follow AutoPopulate.tsx's FieldCard visual state taxonomy. Reuse existing CSS classes: `crosshook-auto-populate-field--found`, `--not-found`, `--ambiguous`.

Props: `checks: HealthIssue[]`, `isLoading: boolean`.

#### Task 2.5: Wire OnboardingWizard into App.tsx Depends on [2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/App.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/App.tsx

Inside the `AppShell` function (NOT top-level `App`), add:

1. State: `const [showOnboarding, setShowOnboarding] = useState(false);`
2. Event listener in `useEffect`: `listen<OnboardingCheckPayload>('onboarding-check', (event) => { if (event.payload.show && !event.payload.has_profiles) setShowOnboarding(true); })` — this implements the upgrade-path guard: existing users with profiles see banner only, not modal
3. Conditional render: `{showOnboarding && <OnboardingWizard open={showOnboarding} onComplete={() => setShowOnboarding(false)} onDismiss={() => setShowOnboarding(false)} />}`

Clean up the listener with `return () => { p.then(f => f()); }` pattern (Tauri `listen()` returns `Promise<UnlistenFn>`).

### Phase 3: Trainer Guidance Content

#### Task 3.1: Add trainer guidance content to backend command Depends on [1.3]

**READ THESE BEFORE TASK**

- docs/plans/trainer-onboarding/research-external.md (trainer sources)
- docs/plans/trainer-onboarding/feature-spec.md (TrainerGuidanceContent model)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/onboarding.rs

Flesh out the `get_trainer_guidance()` command with static `&'static str` content:

Loading modes:

- `source_directory`: "Proton reads the trainer directly from its downloaded location. The trainer stays in place."
- `copy_to_prefix`: "CrossHook copies the trainer and support files into the WINE prefix's C:\\ drive before launch."

Trainer sources:

- `fling`: FLiNG standalone .exe trainers — free, no account, primary recommendation
- `wemod`: WeMod extracted trainers — requires WeMod account and desktop app installed under WINE. Include clear disclaimer about friction.

Verification steps: file existence check, game version match, companion files for CopyToPrefix mode, game launched once.

All strings are compile-time constants. NEVER load from community taps or external sources (security: prevents tap-injected phishing URLs per A-3).

#### Task 3.2: Create TrainerGuidance component Depends on [2.1, 3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ui/CollapsibleSection.tsx
- docs/plans/trainer-onboarding/research-ux.md (progressive disclosure section)

**Instructions**

Files to Create

- src/crosshook-native/src/components/TrainerGuidance.tsx

Loading mode selection with progressive disclosure:

- Two-option card select: SourceDirectory (default recommendation) / CopyToPrefix
- Each card has a 1-line summary visible by default
- "Learn more" expands a `CollapsibleSection` with detailed explanation
- Contextual hint for FLiNG trainers: "FLiNG trainers that bundle DLLs work best with Copy to Prefix"
- WeMod card includes disclaimer: "WeMod requires its own desktop app installed under WINE — see wemod-launcher"

Props: `selectedMode: TrainerLoadingMode`, `onModeChange: (mode: TrainerLoadingMode) => void`, `guidanceContent: TrainerGuidanceContent`.

Include AV false positive warning text (A-6b): "Some antivirus tools may flag trainer executables — this is a known false positive with game trainers."

### Phase 4: Polish and Integration

#### Task 4.1: Trigger post-onboarding health check Depends on [2.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/health.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useOnboarding.ts

After wizard completion (stage transitions to `'completed'` and profile is saved), invoke the existing `batch_validate_profiles` command to trigger a health check on the newly created profile. This writes a `health_snapshots` row for the new profile, integrating it into the Health Dashboard from day one.

This is a best-effort call — if MetadataStore is disabled, the invoke will return a default empty result. Do not block wizard completion on this.

#### Task 4.2: Record initial trainer version snapshot on first launch Depends on [2.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/version.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useOnboarding.ts

**Note**: No existing Tauri command writes a trainer file hash on demand — hash recording happens automatically during game launch (in `launch.rs`). The version snapshot for a newly onboarded profile will be created on the user's first launch, not during wizard completion.

After wizard completion, invoke `check_version_status` (read-only) to pre-populate the UI with the current version state. This is informational only — the actual `trainer_file_hash` is written at launch time. Add a comment in the completion handler: `// Version snapshot is recorded on first launch, not here`.

Best-effort — do not block wizard completion if MetadataStore is disabled or if the command returns an empty result.

#### Task 4.3: Validate interrupt recovery Depends on [2.1, 2.5, 3.2]

**READ THESE BEFORE TASK**

- docs/plans/trainer-onboarding/research-business.md (BR-9: partial profile save prohibited)

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useOnboarding.ts

Verify that dismissing the wizard at any step produces no stale state:

- No partial profile is saved to TOML (profile only persists on explicit save at review step)
- `onboarding_completed` is set to `true` on both completion and skip paths (via `dismiss_onboarding`)
- Hook state resets cleanly on re-open (if user navigates to Settings > "Setup Assistant")
- No orphaned event listeners (verify `useEffect` cleanup)

This is primarily a code review and manual verification task, not a new feature. Add a comment in `useOnboarding.ts` documenting the invariant: "No profile is persisted until the user explicitly confirms in the review step."

## Advice

- **W-1 + W-2 must be a single task**: Both modify `normalize_subscription()` in `taps.rs`. Splitting them guarantees a merge conflict with zero parallelism benefit. The recommended `validate_branch_name()` + `validate_tap_url()` helpers are private to the same function scope.

- **`check_readiness` must be sync, not async**: All four underlying checks (`discover_steam_root_candidates`, `discover_compat_tools`, `is_dir()`) are synchronous filesystem operations totaling <200ms. No `spawn_blocking` needed. The feature-spec marks it `async fn` — this is incorrect; use `fn`.

- **Settings load-mutate-save is the single most critical pattern**: `SettingsStore::save()` does a full-struct overwrite. If `dismiss_onboarding` constructs a fresh `AppSettingsData::default()` with only `onboarding_completed = true`, it will silently erase the user's `community_taps`, `auto_load_last_profile`, and `last_used_profile`. Always `load() → mutate → save()`.

- **Wizard belongs inside `AppShell`, not `App`**: The wizard composes `ProfileFormSections.tsx` which requires `ProfileContext`. The conditional render and event listener must be inside `AppShell()` where context providers are available, not in the outer `App()` function.

- **Upgrade path for existing users**: `onboarding_completed` defaults `false` via `#[serde(default)]`. Without the `has_profiles` guard in the startup event payload, existing users upgrading would see the full wizard modal. The startup emit must include `has_profiles: bool` so the frontend can decide modal (new users) vs banner-only (existing users with profiles).

- **`discover_steam_root_candidates` takes `""` not `None`**: The first parameter is `impl AsRef<Path>`, not `Option<&Path>`. Pass `""` to skip the configured path and use home-dir fallbacks. Passing `None` won't compile.

- **Compatdata scan is inline, not an existing utility**: No function in `crosshook-core` scans all `steamapps/compatdata/*/pfx` directories. Implement this as a loop over `steam_roots` with `fs::read_dir().any()` in `readiness.rs` — do not create a shared utility (only one call site, rule of three).

- **Portal host must be unconditional**: Following `ProfileReviewModal.tsx`, the portal host div is created on component mount, not conditional on the `open` prop. Only the rendered content is gated on `open && isMounted`. Misplacing this guard causes portal mount failures.

- **`HealthIssue.path` must be sanitized**: Apply `sanitize_display_path()` from `commands/shared.rs` to all `path` fields before returning over IPC. This strips sensitive home directory paths from error messages that could appear in screenshots.

- **350ms startup event timing**: The `onboarding-check` event fires at the same 350ms delay as `auto-load-profile`. Both are independent. The frontend `useEffect` listener must be registered before both events fire — this is guaranteed by React mount happening before the first Tauri event delivery.

- **Phase 0 can ship as a standalone PR**: The 3 security tasks are self-contained, reviewable in isolation, and unblock Phase 1 with no rework. Consider shipping them before the larger onboarding work begins.
