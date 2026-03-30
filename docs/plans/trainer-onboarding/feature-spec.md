# Feature Spec: Trainer Onboarding

## Executive Summary

CrossHook's trainer-onboarding feature (issue #37, P0) adds a first-run modal wizard with system-level readiness checks, trainer acquisition guidance, and a chained auto-populate -> profile -> launch workflow to eliminate the cold-start problem for new users. The implementation composes existing primitives (`DiagnosticCollector`, `useInstallGame` stage-machine, `AutoPopulate`, `HealthIssue`) with zero new dependencies. Two community-taps security fixes (git branch injection W-1, URL scheme allowlist W-2) must ship alongside since onboarding encourages tap usage.

## External Dependencies

### APIs and Services

No external APIs are required for v1. All readiness checks are local filesystem operations.

#### Steam Store API (Future — Not V1)

- **Documentation**: [partner.steamgames.com/doc/webapi_overview](https://partner.steamgames.com/doc/webapi_overview)
- **Authentication**: None (public endpoint)
- **Key Endpoint**: `GET https://store.steampowered.com/api/appdetails?appids={appid}` — game name, header image, platform flags
- **Rate Limits**: ~200 requests / 5 minutes (unofficial)
- **Pricing**: Free
- **Use Case**: Enrich onboarding UI with game artwork and metadata. Best-effort, non-blocking.

#### ProtonDB API (Future — Not V1)

- **Documentation**: Undocumented but widely used
- **Authentication**: None
- **Key Endpoint**: `GET https://www.protondb.com/api/v1/reports/summaries/{appid}.json` — tier (`borked`/`bronze`/`silver`/`gold`/`platinum`/`native`), confidence, score
- **Rate Limits**: Not documented; treat as ~1 req/s courtesy
- **Pricing**: Free
- **Use Case**: Show compatibility tier badge during readiness checks. Cache in SQLite with TTL.

### Libraries and SDKs

| Library           | Version  | Purpose                                                      | Status                                           |
| ----------------- | -------- | ------------------------------------------------------------ | ------------------------------------------------ |
| None (v1)         | —        | All readiness checks use existing `crosshook-core` functions | Zero new deps                                    |
| `reqwest`         | 0.12     | Async HTTP for Steam/ProtonDB enrichment                     | **Deferred** — future feature                    |
| `zip`             | 2.x      | FLiNG trainer archive extraction                             | **Deferred** — requires security review (W-3)    |
| `goblin`/`pelite` | 0.9/0.10 | Full PE parsing (arch detection, version strings)            | **Deferred** — 2-byte MZ check sufficient for v1 |

### Trainer Distribution Sources

No trainer site has a public API. All trainer acquisition is user-assisted:

- **FLiNG** ([flingtrainer.com](https://flingtrainer.com/)): Free, standalone `.exe` bundles in `.zip` archives. Primary recommendation for new users. No account required.
- **WeMod** ([wemod.com](https://www.wemod.com/)): Free tier available but requires WeMod account and desktop app installed under WINE via [wemod-launcher](https://github.com/DeckCheatz/wemod-launcher). Not frictionless — must not be implied as equivalent to FLiNG.
- **CheatHappens** ([cheathappens.com](https://www.cheathappens.com/)): Subscription-based (~$6.99/mo). Not a primary recommendation.
- **MrAntiFun**: Now WeMod-exclusive (no standalone distributions since ~2020).

## Business Requirements

### User Stories

**Primary User: New Linux gamer (first launch)**

- As a new Linux gamer, I want to see a checklist of what I need before I can use a trainer so that I don't waste time debugging an incomplete setup (U1)
- As a FLiNG/WeMod user, I want in-app guidance on where to download a trainer so that I don't leave the app to search (U2)
- As a Steam Deck user, I want to auto-populate Steam fields from my game path with one button so that I don't manually dig through filesystem paths (U3)
- As a first-time Proton user, I want to understand the difference between SourceDirectory and CopyToPrefix so that I pick the right loading mode without trial-and-error (U4)
- As a returning user with a new game, I want a guided workflow that chains auto-populate -> profile -> launch so that I set up a new game in one continuous flow (U5)

**Secondary User: Troubleshooting user**

- As a user whose trainer won't load, I want diagnostic feedback explaining why launch failed so that I can self-diagnose (U6)

### Business Rules

1. **BR-1: Readiness gate before trainer launch** — System-level checks (`check_readiness` Tauri command): Steam installed (blocking), Proton available (blocking), game launched once (advisory only for `steam_applaunch`; `NotApplicable` for `proton_run`/`native`), trainer available (informational — always `Skipped` at system stage). Per-game validation deferred to profile creation via existing `auto_populate_steam`.

2. **BR-2: TrainerLoadingMode selection** — `SourceDirectory` (trainer runs from host path, trainer field = directory) vs `CopyToPrefix` (trainer + support files staged into Wine prefix, trainer field = `.exe` file). Default to `SourceDirectory` in the profile model (preserves existing Rust default); wizard UI nudges toward `CopyToPrefix` for FLiNG trainers via contextual hint (FLiNG trainers commonly bundle support DLLs). Validation logic differs per mode.

3. **BR-3: Compatdata must exist before trainer launch** — Steam's per-game `compatdata/` directory is only created after first Steam launch. Onboarding checklist detects this and prompts the user.

4. **BR-4: Launch is a two-step sequence** — For `steam_applaunch`/`proton_run`: launch game (wait 30s, timeout 90s) -> user confirms at main menu -> launch trainer (timeout 10s). For `native`: single step.

5. **BR-5: Trainer path must be a Windows `.exe`** — Enforced at install-request validation and onboarding stage. Add 2-byte `MZ` PE magic check as advisory validation.

6. **BR-7: Auto-populate fields are per-field optional** — Each discovered field (App ID, compatdata, Proton path) may independently be `Found`/`NotFound`/`Ambiguous`. User approves each individually.

7. **BR-8: Onboarding completion state persists** — `onboarding_completed: bool` (with `#[serde(default)]`) in `AppSettingsData` (`settings.toml`). Written via `dismiss_onboarding` Tauri command. Wizard auto-opens on first launch. Once set to `true` (completion or skip), subsequent launches show only empty-state banners.

8. **BR-9: Partial profile save is prohibited** — Wizard saves profile only when all required fields are set (game exe, trainer path, launch method, mode-appropriate trainer path validation). Prevents broken profiles in the health dashboard.

### Edge Cases

| Scenario                                     | Expected Behavior                                                                                             | Notes                             |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| Game not yet installed (fresh copy)          | Route user through Install page flow (prefix creation -> installer -> exe discovery)                          | EC-1                              |
| Flatpak Steam                                | Auto-populate already handles `~/.var/app/com.valvesoftware.Steam`                                            | EC-2                              |
| Multiple Proton versions                     | Auto-populate surfaces `Ambiguous` state; user manually resolves                                              | EC-3                              |
| Trainer is directory with support files      | `CopyToPrefix` handles via support-file staging; guidance explains pointing at `.exe` is correct              | EC-4                              |
| User skips trainer during profile setup      | Trainer path is optional (`validate_optional_trainer_path`); add later                                        | EC-5                              |
| Standalone Proton prefix (`proton_run`)      | Compatdata is the prefix path itself; distinct from Steam's `steamapps/compatdata/<appid>`                    | EC-6                              |
| Existing users upgrade to onboarding version | `onboarding_completed` defaults to `false`; readiness checks likely all-pass; wizard dismissible in one click | Settings TOML `#[serde(default)]` |

### Success Criteria

- [ ] First-time user completes full profile-setup-to-first-launch flow without external documentation
- [ ] Readiness checklist correctly detects absence of Steam, Proton, compatdata, or trainer with specific corrective actions
- [ ] Users understand SourceDirectory vs CopyToPrefix without reading external docs
- [ ] Onboarding completion state persists across app restarts
- [ ] Guided workflow orchestrates existing components (AutoPopulate, InstallPage, LaunchPage) without logic duplication
- [ ] Wizard is fully usable with gamepad on Steam Deck (56px touch targets, focus trap, B=back)

## Technical Specifications

### Architecture Overview

```text
Frontend (React + TypeScript)
  OnboardingWizard.tsx ─── ReadinessChecklist.tsx ─── TrainerGuidance.tsx
           │
  useOnboarding.ts (stage-machine: readiness -> guidance -> profile -> complete)
           │ invoke()
  ─────────┼────────────────────────────────────────────────
  Tauri IPC │ commands/onboarding.rs
           │
  check_readiness │ dismiss_onboarding │ get_trainer_guidance
           │
  ─────────┼────────────────────────────────────────────────
  crosshook-core
           │
  onboarding/readiness.rs ──▶ steam/discovery, steam/proton,
                               install/service, profile/health (HealthIssue)
           │
  settings/mod.rs ──▶ settings.toml (onboarding_completed: bool)
```

### Data Models

#### Settings Extension (`settings/mod.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,  // NEW
}
```

#### Readiness Check Result (`onboarding/mod.rs`)

Reuses `HealthIssue` from `profile/health.rs` — no parallel type:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheckResult {
    pub checks: Vec<HealthIssue>,  // reuse existing type
    pub all_passed: bool,
    pub critical_failures: usize,
    pub warnings: usize,
}
```

#### Trainer Guidance Content (`onboarding/mod.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerGuidanceEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub when_to_use: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerGuidanceContent {
    pub loading_modes: Vec<TrainerGuidanceEntry>,
    pub trainer_sources: Vec<TrainerGuidanceEntry>,
    pub verification_steps: Vec<String>,
}
```

#### TypeScript Types (`types/onboarding.ts`)

```typescript
import type { HealthIssue } from './health';

export interface ReadinessCheckResult {
  checks: HealthIssue[];
  all_passed: boolean;
  critical_failures: number;
  warnings: number;
}

export type OnboardingWizardStage = 'readiness_check' | 'trainer_guidance' | 'profile_creation' | 'completed';
```

### API Design

Three new Tauri IPC commands (reduced from 5 — `get_onboarding_status` unnecessary since frontend reads `settings_load`; `complete_onboarding_step` unnecessary since wizard progress is ephemeral frontend state):

#### `check_readiness`

**Purpose**: Run all system-level first-run readiness checks. No parameters — checks system readiness, not per-game configuration.

```rust
#[tauri::command]
pub async fn check_readiness() -> Result<ReadinessCheckResult, String>
```

**Response (200):**

```json
{
  "checks": [
    {
      "field": "steam_installed",
      "path": "~/.local/share/Steam",
      "message": "Steam found",
      "remediation": "",
      "severity": "Info"
    },
    {
      "field": "proton_available",
      "path": null,
      "message": "Found 3 Proton versions",
      "remediation": "",
      "severity": "Info"
    },
    {
      "field": "game_launched_once",
      "path": null,
      "message": "No compatdata detected",
      "remediation": "Launch your game once through Steam first",
      "severity": "Warning"
    },
    {
      "field": "trainer_available",
      "path": null,
      "message": "Download a trainer before creating a profile",
      "remediation": "...",
      "severity": "Info"
    }
  ],
  "all_passed": false,
  "critical_failures": 0,
  "warnings": 1
}
```

| Check                | Existing Function Reused                                   | Pass Criteria                                         |
| -------------------- | ---------------------------------------------------------- | ----------------------------------------------------- |
| `steam_installed`    | `steam/discovery.rs` -> `discover_steam_root_candidates()` | At least one Steam root found                         |
| `proton_available`   | `install/service.rs:298` -> Proton discovery               | Non-empty result                                      |
| `game_launched_once` | Filesystem scan of `steamapps/compatdata/*/pfx`            | Any compatdata dir exists                             |
| `trainer_available`  | `install/service.rs:176` -> path existence pattern         | Always `Info` (no trainer path at system check stage) |

#### `dismiss_onboarding`

**Purpose**: Permanently dismiss wizard by setting `onboarding_completed = true`.

```rust
#[tauri::command]
pub fn dismiss_onboarding(settings_store: State<'_, SettingsStore>) -> Result<(), String>
```

#### `get_trainer_guidance`

**Purpose**: Return static compiled guidance content about trainer types and loading modes. Guidance strings are `&'static str` constants — never loaded from community taps or external sources.

```rust
#[tauri::command]
pub fn get_trainer_guidance() -> TrainerGuidanceContent
```

### System Integration

#### Files to Create (8 files)

| File                                                | Layer    | Purpose                                                          |
| --------------------------------------------------- | -------- | ---------------------------------------------------------------- |
| `crates/crosshook-core/src/onboarding/mod.rs`       | Core     | Module root, re-exports, types                                   |
| `crates/crosshook-core/src/onboarding/readiness.rs` | Core     | `check_system_readiness()` free function + inline hint constants |
| `src-tauri/src/commands/onboarding.rs`              | Tauri    | 3 IPC commands wrapping core functions                           |
| `src/hooks/useOnboarding.ts`                        | Frontend | Wizard stage-machine (follows `useInstallGame.ts` pattern)       |
| `src/types/onboarding.ts`                           | Frontend | TypeScript type definitions                                      |
| `src/components/OnboardingWizard.tsx`               | Frontend | Modal wizard overlay                                             |
| `src/components/ReadinessChecklist.tsx`             | Frontend | Readiness check display component                                |
| `src/components/TrainerGuidance.tsx`                | Frontend | Loading mode guidance component                                  |

#### Files to Modify (6 files)

| File                                        | Change                                                           |
| ------------------------------------------- | ---------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs`          | Add `pub mod onboarding;`                                        |
| `crates/crosshook-core/src/settings/mod.rs` | Add `onboarding_completed: bool` to `AppSettingsData`            |
| `src-tauri/src/commands/mod.rs`             | Add `pub mod onboarding;`                                        |
| `src-tauri/src/lib.rs`                      | Register 3 new commands, emit `onboarding-check` at startup      |
| `src/App.tsx`                               | Listen for `onboarding-check` event, conditionally render wizard |
| `src/types/index.ts`                        | Re-export onboarding types                                       |

#### Configuration

- `capabilities/default.json`: Register 3 new commands with minimal permissions (no FS shell access for guidance/status commands)
- No new SQLite migration needed — persistence is `settings.toml` only

## UX Considerations

### User Workflows

#### Primary Workflow: First-Run Guided Wizard

1. **Readiness Check** (auto-runs on wizard open)
   - User: Sees per-check status icons (pass/fail/warning/pending)
   - System: Scans Steam, Proton, compatdata, trainer status
   - Blocking: Steam + Proton must pass. Game-launched-once is advisory. Trainer is informational.

2. **Trainer Guidance** (acquisition + mode selection)
   - User: Reviews trainer source recommendations (FLiNG = primary, WeMod = with caveats)
   - User: Selects loading mode with progressive disclosure explanation
   - System: Shows 2-option card select with collapsed inline details per mode

3. **Profile Creation** (composes existing AutoPopulate + ProfileForm)
   - User: Browses to game executable, triggers auto-populate
   - System: Discovers App ID, compatdata, Proton path per-field
   - User: Confirms discovered values, sets profile name, saves

4. **Completion**
   - System: Sets `onboarding_completed = true`, navigates to Launch page
   - User: Ready to launch first game + trainer combo

#### Skip/Dismiss Flow

- "Skip setup" link at Step 1 header dismisses wizard immediately (`onboarding_completed = true`)
- Empty-state banners on Profiles/Launch pages provide re-entry ("Start Guided Setup" button)
- Settings page provides "Setup Assistant" re-entry for returning users

#### Stale Trainer Repair Flow

- Health badge flags profile with invalid trainer path
- Launch page shows inline warning with "Fix trainer path" CTA
- 2-step mini-wizard: path picker -> mode confirmation -> updates existing profile (preserves UUID)

### UI Patterns

| Component             | Pattern                                                | Notes                                                             |
| --------------------- | ------------------------------------------------------ | ----------------------------------------------------------------- |
| Wizard steps          | Numbered step indicator (1 of N)                       | Not percentage bar — too abstract for non-linear states           |
| Loading mode selector | Two-option card select                                 | Progressive disclosure: 1-line summary default, expand on click   |
| Readiness checks      | Per-item status icons (checkmark/cross/warning/circle) | Green/red/yellow/muted using existing CSS variables               |
| Empty-state banners   | CTA card with illustration                             | `crosshook-panel` with `crosshook-card-padding`, not `PageBanner` |
| File path fields      | `InstallField` component                               | Already has label + input + browse + helpText + error             |
| Collapsible guidance  | `CollapsibleSection` component                         | For "What is a trainer?", "Which mode?" sections                  |
| Controller prompts    | Extended `ControllerPrompts`                           | Accept `confirmLabel`, `backLabel`, `showBumpers` override props  |

### Accessibility Requirements

- **Focus trap**: Tab/Shift+Tab cycles only within wizard modal. `data-crosshook-focus-root="modal"` on wizard root.
- **Touch targets**: 56px minimum in controller mode (`--crosshook-touch-target-min`)
- **ARIA**: `role="dialog"`, `aria-modal="true"`, `aria-labelledby` pointing to step title
- **Reduced motion**: `@media (prefers-reduced-motion: reduce) { .crosshook-skeleton { animation: none; } }`
- **Contrast**: Skeleton blocks need 3:1 ratio against `--crosshook-color-bg` (WCAG 1.4.11)
- **Gamepad**: A=confirm, B=previous step (not close), D-pad=navigate, RB=disabled in wizard

### Performance UX

- **Loading States**: Single brief spinner for `check_readiness` (<200ms total). Per-item spinners not warranted at this latency.
- **Inline Validation**: "Reward Early, Punish Late" — show success immediately on valid path; show errors only on blur.
- **Disabled Continue**: "Next" button disabled until current step validates. Prevents submit-time errors.
- **Steam Deck File Picker**: OS-native dialogs don't support gamepad. Provide typed path input alongside browse button. Use `ShowFloatingGamepadTextInput` for text entry in controller mode.

## Recommendations

### Implementation Approach

**Recommended Strategy**: Hybrid first-run modal wizard + persistent contextual banners (Option D from research). Modal provides focused first-run experience; banners provide ongoing discovery for returning users. Each part is independently valuable and can phase independently.

**Phasing:**

1. **Phase 0 - Security Hardening** (~1 day, prerequisite): W-1 branch injection fix, W-2 URL scheme allowlist, A-1 symlink skip, A-4 desktop `%` escaping. All in `community/taps.rs` and `launch/script_runner.rs`.
2. **Phase 1 - Backend Readiness Checks** (~3-5 days): Core module, 4 readiness checks, settings flag, Tauri commands, empty-state banner on ProfilesPage (quick win).
3. **Phase 2 - Guided Workflow UI** (~5-8 days): `useOnboarding.ts` stage-machine hook, wizard modal, step components, gamepad navigation, controller-mode file selection fallback.
4. **Phase 3 - Trainer Guidance Content** (~3-5 days, parallelizable with Phase 2): Loading mode guidance cards, trainer type detection from filename patterns, WeMod disclaimer, hardcoded guidance text.
5. **Phase 4 - Polish and Integration** (~2-3 days): Post-onboarding health check, version snapshot baseline, interrupt recovery, Steam Deck end-to-end testing.

### Technology Decisions

| Decision                     | Recommendation                                           | Rationale                                                                                      |
| ---------------------------- | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| State persistence            | `onboarding_completed: bool` in `AppSettingsData` (TOML) | Single boolean; no SQLite migration. Wizard step progress is ephemeral frontend state.         |
| First-run detection          | Check TOML flag (~0ms), fallback to empty profile list   | Fast primary check with reliable fallback                                                      |
| Readiness check architecture | Single monolithic `check_readiness()` command            | All checks <200ms total. No benefit from per-check IPC granularity.                            |
| Guidance content delivery    | Static compiled `&'static str` constants in Rust         | Zero latency, version-locked, no external deps. Security: prevents tap-injected phishing URLs. |
| Wizard state management      | Page-level `useOnboarding.ts` hook (no React Context)    | Single page consumes the state; Context adds indirection for no benefit.                       |
| Frontend patterns            | Follow `useInstallGame.ts` stage-machine exactly         | Proven pattern; no wizard library needed for 5 steps.                                          |
| Compatdata detection         | Filesystem check (`is_dir()`)                            | Simpler than SQLite `launch_operations` query; `MetadataStore` not always available.           |

### Quick Wins

- **Empty-state banner on ProfilesPage**: Zero backend work. When `profile_store.list()` is empty, show "Get Started" banner linking to wizard.
- **`onboarding_completed` flag**: 10-line change in `settings/mod.rs`. Enables conditional routing.
- **Trainer file `.exe` extension check**: Extend `validate_optional_trainer_path`. Prevents common user mistake.
- **Expose existing readiness data**: `default_steam_client_install_path` and `list_proton_installs` already exist. Thin readiness command is trivial.

### Future Enhancements

- **Trainer auto-detection**: Scan `~/Downloads` and `~/.local/share/crosshook/trainers/` for `.exe` files, suggest matches by game name.
- **ProtonDB compatibility signal**: Show tier badge using undocumented endpoint. Cache in SQLite. Requires `reqwest`.
- **Batch onboarding**: Set up multiple game+trainer profiles in sequence without restarting wizard.
- **Community profile discovery**: Surface "getting started" profiles from community taps for popular games.
- **Trainer ZIP extraction**: `zip` crate with mandatory path traversal validation (W-3). Separate security review milestone.

## Risk Assessment

### Technical Risks

| Risk                                          | Likelihood | Impact | Mitigation                                                                                                                                 |
| --------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| OS file dialog broken on Steam Deck Game Mode | High       | High   | Typed path input alongside browse button; Steam's `ShowFloatingGamepadTextInput` for keyboard. Never make browse the only path method.     |
| Trainer loading mode confusion                | High       | Medium | Default to `SourceDirectory`; wizard nudges `CopyToPrefix` for FLiNG trainers via contextual hint. Force explicit launch method selection. |
| Game not launched once (no compatdata)        | High       | Medium | Detect missing compatdata; clear one-step fix: "Launch via Steam, then return"                                                             |
| Steam not installed or non-standard path      | Medium     | High   | Graceful fallback with manual path entry; existing `discover_steam_root_candidates` handles Flatpak                                        |
| WeMod treated like FLiNG (wrong workflow)     | Medium     | Medium | Detect WeMod in filename; show dedicated disclaimer about WINE app installation requirement                                                |
| Wizard abandoned mid-way                      | Medium     | Medium | No auto-save; state held in hook only. Profile persisted on explicit save. No cleanup needed.                                              |
| Existing users see wizard on upgrade          | Medium     | Low    | `onboarding_completed` defaults `false`; readiness all-pass likely; dismissible in one click                                               |

### Integration Challenges

- **Sidebar/modal choice**: First-run modal avoids sidebar clutter. "Setup Assistant" in Settings for re-entry. No permanent sidebar entry.
- **Auto-load conflict**: Startup auto-loads last profile. On first run with no profiles, this is a no-op. Wizard doesn't interfere.
- **Profile persistence timing**: Deferred save pattern (review -> explicit save) matches existing Install workflow.

### Security Considerations

#### Critical -- Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings -- Must Address

| Finding                                                            | Risk                                                                                                  | Mitigation                                                                                                | Alternatives                                        |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- | --------------------------------------------------- |
| W-1: Git branch argument injection (`taps.rs:209,240`)             | Branch name starting with `--` parsed as git flag (e.g., `--upload-pack=/evil/script`)                | Validate branch names reject `-` prefix in `normalize_subscription()`; add `--` separator in git commands | Just the `--` separator if validation is too strict |
| W-2: `file://` URL scheme in tap subscriptions (`taps.rs:362-387`) | Cloning arbitrary local directories                                                                   | Allowlist `https://` and `ssh://git@` only in `normalize_subscription()`                                  | Warn-but-allow with explicit user confirmation      |
| W-3: `shell:open` capability must use URL allowlist                | When `opener:open-url` is added for "Install Steam" buttons, URL injection via concatenated scan data | All URLs must be hardcoded frontend constants; scope `opener:allow-open-url` with URL pattern allowlist   | N/A — configure correctly before first use          |

#### Advisories -- Best Practices

- A-1: Symlink skip in `copy_dir_all()` during CopyToPrefix staging (deferral: low risk, FLiNG zips don't contain symlinks)
- A-2: 2-byte `MZ` PE magic check at trainer file selection — `ValidationSeverity::Warning`, no new deps
- A-3: Trainer source URLs must be compile-time constants (architecture decision, not code change)
- A-4: `%` -> `%%` escaping in `escape_desktop_exec_argument()` (1-line fix)
- A-6: Apply `sanitize_display_path()` to all error/remediation strings in `commands/onboarding.rs`
- A-6b: AV false positive warning in onboarding guidance text
- A-9: "Game launched once" check derives path from `discover_steam_libraries()` + app_id, not `profile.steam.compatdata_path`

## Task Breakdown Preview

### Phase 0: Security Hardening

**Focus**: Fix pre-existing security issues that become higher-risk when onboarding encourages tap usage.
**Tasks**:

- W-1: Validate branch names + `--` separator in git commands (`community/taps.rs`)
- W-2: URL scheme allowlist in `normalize_subscription()`
- A-1: `is_symlink()` skip in `copy_dir_all()` (`launch/script_runner.rs`)
- A-4: `%` -> `%%` in `escape_desktop_exec_argument()` (`export/launcher.rs`)
- Unit tests for each fix
  **Parallelization**: All fixes are independent; can run concurrently.

### Phase 1: Backend Readiness Checks

**Focus**: Build the readiness check system and settings flag.
**Tasks**:

- Create `crosshook-core/src/onboarding/` module (mod.rs + readiness.rs)
- Implement 4 readiness check functions composing existing discovery functions
- Add `onboarding_completed: bool` to `AppSettingsData`
- Create `commands/onboarding.rs` with 3 Tauri commands
- Register commands in `lib.rs` and `capabilities/default.json`
- Frontend: `types/onboarding.ts` type definitions
- Frontend: Empty-state "Get Started" banner on ProfilesPage (quick win)
- Unit tests for readiness check functions
  **Parallelization**: Backend and frontend type definitions can run concurrently. Banner depends on types.

### Phase 2: Guided Workflow UI

**Focus**: Build the step-based wizard experience.
**Dependencies**: Phase 1 (readiness checks must exist for wizard to consume)
**Tasks**:

- `hooks/useOnboarding.ts` stage-machine hook (mirror `useInstallGame.ts` pattern)
- `OnboardingWizard.tsx` modal with step navigation
- `ReadinessChecklist.tsx` per-check status cards
- Game setup step composing `AutoPopulate.tsx` + game path picker
- Trainer setup step composing `InstallField` + loading mode selector
- Profile review step composing `ProfileFormSections.tsx`
- Gamepad navigation: focus trap, B=back, controller-mode touch targets
- **Controller-mode file selection**: Typed path input alongside browse button (P0 for Steam Deck)
- Settings page "Setup Assistant" re-entry link
  **Parallelization**: Component implementations can run concurrently after hook is established.

### Phase 3: Trainer Guidance Content

**Focus**: Add contextual help and trainer-specific recommendations.
**Dependencies**: None (parallelizable with Phase 2)
**Tasks**:

- `TrainerGuidance.tsx` loading mode explanation cards with progressive disclosure
- Trainer type detection from filename patterns (FLiNG, WeMod)
- WeMod-specific disclaimer about WINE app installation
- `get_trainer_guidance` Tauri command with static compiled content
- AV false positive warning in guidance text (A-6b)

### Phase 4: Polish and Integration

**Focus**: End-to-end testing and integration.
**Dependencies**: Phase 2 completion
**Tasks**:

- Post-onboarding health check trigger for newly created profile
- Record initial trainer hash in `version_snapshots` on profile creation
- Interrupt recovery validation (wizard dismissal produces no stale state)
- End-to-end testing on Steam Deck (Desktop Mode + Game Mode)

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **Trainer Loading Mode Default**
   - Options: (A) `SourceDirectory` as profile default with wizard nudge toward `CopyToPrefix` for FLiNG, (B) `CopyToPrefix` as universal default
   - Impact: Determines wizard guidance text and auto-detection logic
   - Recommendation: Option A — preserves existing Rust default; wizard provides contextual recommendation

2. **Onboarding Trigger for Existing Users on Upgrade**
   - Options: (A) Auto-show wizard if `onboarding_completed` defaults false, (B) Show banner only if profiles already exist
   - Impact: UX for upgrade path
   - Recommendation: Option B — existing users with profiles see banner only, not modal

3. **WeMod Guidance Depth**
   - Options: (A) Clear callout in guidance text only, (B) Dedicated WeMod setup sub-flow in wizard
   - Impact: Scope and complexity of Phase 3
   - Recommendation: Option A for v1 — WeMod setup is complex enough to warrant a separate feature

4. **Community Profile Integration**
   - Options: (A) Wizard offers to import community profiles, (B) Defer to separate feature
   - Impact: Wizard scope
   - Recommendation: Option B — keep initial scope focused

5. **Native Launch Method in Wizard**
   - Options: (A) Support all three methods, (B) Focus on `steam_applaunch` and `proton_run` only
   - Impact: Wizard complexity
   - Recommendation: Option B — `native` doesn't involve trainers in the typical sense

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): External API details, trainer distribution sources, dependency evaluation
- [research-business.md](./research-business.md): Business logic analysis, user stories, domain model, edge cases
- [research-technical.md](./research-technical.md): Architecture design, data models, API design, codebase changes
- [research-ux.md](./research-ux.md): UX research, competitive analysis, accessibility, gamepad patterns
- [research-security.md](./research-security.md): Security analysis with severity-leveled findings (0 critical, 3 warning, 10 advisory)
- [research-practices.md](./research-practices.md): Engineering practices, reusable code, KISS assessment, build-vs-depend
- [research-recommendations.md](./research-recommendations.md): Full recommendations, phasing, risk assessment, alternative approaches
