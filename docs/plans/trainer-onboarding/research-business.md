# Trainer Onboarding — Business Research

CrossHook currently offers full trainer-launch capabilities but no guided path for first-time users. This document captures the business logic, domain rules, user journeys, and codebase integration points for a trainer-onboarding and acquisition guidance feature (#37, P0).

---

## Executive Summary

New users face a cold-start problem: they must independently discover how to find a trainer, understand the loading-mode distinction, confirm that Steam/Proton/game prerequisites are satisfied, and wire up a profile before they can launch. The onboarding feature closes that gap by providing (1) in-app readiness checks, (2) contextual guidance for acquiring trainers (FLiNG, WeMod, etc.), (3) an explanation of `SourceDirectory` vs `CopyToPrefix`, and (4) a wizard that chains auto-populate → profile creation → trainer selection → launch.

---

## User Stories

| ID  | As a…                          | I want to…                                                              | So that…                                                     |
| --- | ------------------------------ | ----------------------------------------------------------------------- | ------------------------------------------------------------ |
| U1  | New Linux gamer (first launch) | See a checklist of what I need before I can use a trainer               | I don't waste time debugging an incomplete setup             |
| U2  | FLiNG/WeMod user               | Get in-app guidance on where to download a trainer                      | I don't leave the app to search for how to find trainers     |
| U3  | Steam Deck user                | Auto-populate Steam fields from my game path with one button            | I don't manually dig through filesystem paths                |
| U4  | First-time Proton user         | Understand the difference between SourceDirectory and CopyToPrefix      | I pick the right loading mode without trial-and-error        |
| U5  | Returning user, new game       | Follow a guided workflow that chains auto-populate → profile → launch   | I set up a new game in one continuous flow                   |
| U6  | User whose trainer won't load  | See diagnostic feedback explaining why launch failed or trainer stalled | I can self-diagnose without filing a support request         |
| U7  | User installing a cracked game | Use the Install page to set up a Proton prefix and then add a trainer   | My game is runnable and trainerized from a single setup flow |

---

## Business Rules

### Core Rules

**BR-1: Readiness gate before trainer launch**
The system must surface whether these prerequisites are satisfied. Readiness checks are split into two stages:

_System-level checks_ (`check_readiness` Tauri command, accepts optional `launch_method: Option<String>`):

1. `steam_installed` — Steam root directory discoverable on the host filesystem — **blocking**
2. `proton_available` — at least one Proton installation found under any Steam library — **blocking**
3. `game_launched_once` — behavior depends on `launch_method`:
   - `None` (not yet known at wizard open): returns `NotApplicable { reason: "launch_method_not_set" }` → UI shows `○ — select launch method below to check`; no filesystem scan performed
   - `steam_applaunch`: scans for any `compatdata/*/pfx` directory under Steam libraries → **advisory ⚠ only** (non-blocking)
   - `proton_run`: returns `NotApplicable { reason: "user_managed_prefix" }` → UI shows `✓ Not required — you manage your own Proton prefix`; no scan
   - `native`: returns `NotApplicable { reason: "not_applicable" }` → row hidden entirely
   - Wizard re-invokes (or re-evaluates cached result) after launch method is determined; the advisory-only nature means no wizard flow gating changes
4. `trainer_available` — always `Skipped` at this stage (informational only; cannot verify before game is chosen)

_Per-game validation_ (deferred to profile creation step using existing `auto_populate_steam` command):

- Specific game's compatdata directory exists (`SteamAutoPopulateFieldState::Found`)
- Proton version used for the game is available
- Trainer file exists and has `.exe` extension

**BR-2: TrainerLoadingMode selection and path validation**

- `SourceDirectory`: the trainer runs directly from its Linux host path; no copying occurs. The trainer path field must point to a **directory** (not a file) — the directory must exist and contain the trainer executable.
- `CopyToPrefix`: the trainer (plus sibling support files with extensions `dll`, `json`, `config`, `ini`, `pak`, `dat`, `bin`) and known subdirectories (`assets`, `data`, `lib`, `bin`, `runtimes`, `plugins`, `locales`, `cef`, `resources`) are staged into `C:\CrossHook\StagedTrainers\<trainer-stem>\` inside the Wine prefix before launch. The trainer path field must point to a **`.exe` file** that exists on the host filesystem.
- Validation logic differs per mode — the onboarding wizard and profile form must validate the trainer path against the selected mode, not a single shared rule.
- **Conflicting recommendations — team-lead decision required**: recommendations-agent analysis suggests `CopyToPrefix` as the default for FLiNG trainers (because they commonly bundle support DLLs that need to be inside the Wine filesystem); UX research (ux-researcher-3) recommends `SourceDirectory` as the safe default for FLiNG. Both analyses are reasonable. The practical difference: `SourceDirectory` fails silently when the trainer needs Wine-internal paths; `CopyToPrefix` is slower but more broadly compatible. Recommendation: default to `SourceDirectory` in the profile model (preserves existing Rust default), but have the wizard UI nudge toward `CopyToPrefix` for FLiNG trainers specifically via a contextual hint. `CopyToPrefix` remains required for WeMod.
- The user must be guided to understand which mode their trainer needs via contextual inline explanation at the mode selector.

**BR-3: Compatdata must exist before trainer launch**
Steam's per-game compatdata directory is only created after the game has been launched at least once through Steam. The onboarding checklist must detect this condition and prompt the user to launch the game through Steam before proceeding.

**BR-4: Launch is a two-step sequence for non-native methods**
For `steam_applaunch` and `proton_run`:

1. Launch game (wait 30 s for startup, timeout 90 s)
2. Wait for user confirmation at main menu
3. Launch trainer (timeout 10 s)

For `native`: single step, no trainer runner flow.

**BR-5: Trainer path must be a Windows `.exe`**
The trainer executable must have an `.exe` extension. The system enforces this at install-request validation and must also enforce it at the onboarding stage.

**BR-6: Profile name must be valid**
Profile names must not be empty and must pass `validate_name` (no invalid characters). The profile-name is slugified to derive the default prefix path (`~/.local/share/crosshook/prefixes/<slug>`).

**BR-7: Auto-populate fields are per-field optional**
Each discovered field (App ID, compatdata path, Proton path) may independently be `Found`, `NotFound`, or `Ambiguous`. The user must individually approve/apply each proposed value. Ambiguous matches require manual resolution.

**BR-8: Onboarding completion state must persist across sessions**

- `onboarding_dismissed: bool` (with `#[serde(default)]`) in `AppSettingsData` (settings.toml) controls wizard auto-open. The name `onboarding_dismissed` is preferred over `first_run_complete` — it correctly captures both the "completed" and "skipped" paths.
- Written via a dedicated `dismiss_onboarding` Tauri command (keeps the flag-write logic in one place, callable from both the final wizard step and the Skip button).
- Wizard auto-opens on first app launch (flag is `false`). Once set to `true` — whether by completing the wizard or by clicking Skip — subsequent launches show only an empty-state banner on Profiles/Launch pages.
- **Skip dismiss timing** (open question resolved by UX research): write `onboarding_dismissed = true` **immediately** on Skip, not only after a successful launch. Users who chose manual setup should not be re-pestered.
- The empty-state banner must link back to the wizard for re-entry on user request. No auto-trigger after the first launch.

**BR-9: Partial profile save is prohibited**
The wizard must not save a profile until all required fields are set. "Save" is only available when the profile is minimally complete (game exe path, trainer path, launch method, and mode-appropriate trainer path validation pass). This prevents broken profiles from entering the health dashboard with missing required fields.

**BR-10: Stale trainer repair flow updates existing profile**
When the Health Dashboard or Launch page detects a stale trainer path, a 2-step mini-wizard (path picker → mode confirmation) repairs the profile. The repair flow must update `trainer.path` and optionally `trainer.loading_mode` on the _existing_ profile — not create a new one. Profile identity (UUID in SQLite `profiles` table) must be preserved.

### Edge Cases

**EC-1: Game not yet installed (fresh copy)**
The user has a game installer (`.exe`) but the game is not yet installed in any prefix. The onboarding should detect the absence of a game executable and offer to route them through the Install page flow (Proton prefix creation → installer execution → executable discovery).

**EC-2: Flatpak Steam**
Steam installed as a Flatpak (`~/.var/app/com.valvesoftware.Steam`) has a different filesystem root. The auto-populate logic already handles this candidate, but the onboarding guidance text must acknowledge it.

**EC-3: Multiple Proton versions**
Steam may have multiple Proton versions installed. The `config.vdf` CompatToolMapping dictates which is used per-game. When the auto-populate discovers an Ambiguous state, onboarding must explain how to find the correct version.

**EC-4: Trainer is a directory, not a single executable**
Some trainer distributions include a launcher `.exe` alongside many support files. `CopyToPrefix` handles this via the support-file staging logic. Onboarding must explain that pointing at the `.exe` is correct even when the trainer is in a directory with other files.

**EC-5: User skips trainer during profile setup**
The trainer path is optional in the install flow (`validate_optional_trainer_path`). Onboarding must permit skipping the trainer step and allow adding it later, without blocking profile creation.

**EC-6: Compatdata path found but game not launched once through Proton run**
For `proton_run` mode (standalone Proton prefix), compatdata is the prefix path itself. This is distinct from Steam's `steamapps/compatdata/<appid>` path. Onboarding must distinguish between the two modes.

---

## Workflows

### Primary Workflow: Guided Onboarding Wizard

```
1. Readiness Check (shown on first launch or triggered from a banner)
   ├── Steam detected? [Yes / No + hint]
   ├── Proton version available? [Yes / No + hint]
   ├── Game launched once (compatdata exists)? [Yes / No + action: launch via Steam]
   └── Trainer downloaded? [Yes / No + acquisition guidance]

2. Profile Setup
   ├── Enter game name + profile name
   └── Browse to game executable

3. Auto-Populate (existing AutoPopulate component)
   ├── Scan → App ID, compatdata path, Proton path
   ├── Apply discovered values individually
   └── Manual hints for unresolved fields

4. Trainer Selection
   ├── Browse to trainer .exe
   ├── Explain SourceDirectory vs CopyToPrefix (contextual hint)
   └── User selects loading mode

5. Profile Save
   └── Profile written to ~/.config/crosshook/<name>.toml

6. Launch
   ├── Validate profile (pre-flight check)
   ├── Launch game → wait for user confirmation at main menu
   └── Launch trainer
```

### Secondary Workflow: Trainer Acquisition Guidance

```
Triggered from onboarding step 1 ("Trainer downloaded?" = informational nudge)
  ├── Show trainer source context (hardcoded text — NOT from community taps):
  │   ├── FLiNG Trainer — free, standalone .exe bundles, no account required
  │   │     → Loading mode: CopyToPrefix (bundles DLLs/assets)
  │   ├── WeMod — free tier available but requires a WeMod account
  │   │     and WeMod desktop app installed under Wine (WINE sub-flow)
  │   │     → Loading mode: CopyToPrefix
  │   └── (CheatHappens/MrAntiFun not recommended — paid subscription or
  │         WeMod-exclusive; mention only if user asks)
  ├── Explain file format (.exe expected, single file or directory)
  ├── Explain loading mode recommendation per source type
  └── Note: CrossHook cannot download trainers on user's behalf —
            user must obtain the .exe themselves
```

### Error Recovery Workflow

```
Launch failure (validation or runtime error)
  ├── ValidationError → highlight offending profile field
  ├── Compatdata not found → prompt to launch game via Steam first
  ├── Proton not found → redirect to auto-populate or manual path entry
  ├── Trainer load timeout → suggest switching loading mode
  └── Installer exited with failure → show log path, retry option
```

### Install Game Sub-Flow (EC-1: game not yet installed)

```
InstallPage (existing)
  ├── Profile name, display name
  ├── Installer .exe path (Windows installer)
  ├── Trainer path (optional)
  ├── Proton path
  ├── Prefix path (auto-derived or manual)
  ├── Run installer through Proton
  ├── Discover installed game executable candidates
  ├── User confirms executable
  └── Profile auto-created → hand off to onboarding step 6
```

---

## Domain Model

### Key Entities

| Entity                    | Description                                                                             | Source                   |
| ------------------------- | --------------------------------------------------------------------------------------- | ------------------------ |
| `GameProfile`             | Core profile: game exe, trainer path, loading mode, launch method, Steam/runtime config | `profile/models.rs`      |
| `TrainerSection`          | Trainer path, type label, loading mode                                                  | `profile/models.rs`      |
| `TrainerLoadingMode`      | `SourceDirectory` or `CopyToPrefix`                                                     | `profile/models.rs`      |
| `LaunchMethod`            | `steam_applaunch`, `proton_run`, `native`                                               | `launch/request.rs`      |
| `SteamSection`            | App ID, compatdata path, Proton path, launcher icon                                     | `profile/models.rs`      |
| `RuntimeSection`          | Standalone prefix path, Proton path, working directory (for `proton_run`)               | `profile/models.rs`      |
| `InstallGameRequest`      | Inputs for the guided install flow (installer, trainer, Proton, prefix, exe)            | `install/models.rs`      |
| `SteamAutoPopulateResult` | Per-field discovery result (App ID, compatdata, Proton) with diagnostics/hints          | `steam/auto_populate.rs` |
| Onboarding State          | Persisted completion/dismissal of onboarding wizard (to be stored in SQLite metadata)   | new — `metadata/` layer  |

### State Transitions

**Onboarding Wizard States**

```
not_started → in_progress → completed
                         ↘ dismissed
```

**Readiness Check States (per prerequisite)**

```
unknown → checking → satisfied
                  ↘ unsatisfied (+ hint/action)
```

**Install Stage States** (existing, reused)

```
idle → preparing → running_installer → review_required → ready_to_save
                                    ↘ failed
```

**Launch Phase States** (existing, reused)

```
Idle → GameLaunching → WaitingForTrainer → TrainerLaunching → SessionActive
     ↘ (native) ────────────────────────────────────────────→ SessionActive
```

---

## Existing Codebase Integration

### Directly Reusable Components

| Component                                   | Path                                | Role in onboarding                                                                         |
| ------------------------------------------- | ----------------------------------- | ------------------------------------------------------------------------------------------ |
| `AutoPopulate` component                    | `src/components/AutoPopulate.tsx`   | Step 3 of the wizard — already scans Steam, reports per-field state and hints              |
| `attempt_auto_populate`                     | `steam/auto_populate.rs`            | Backend for step 3; handles Flatpak Steam, multiple libraries, VDF parsing                 |
| `DiagnosticCollector`                       | `steam/diagnostics.rs`              | Pattern for collecting user-visible diagnostics + manual hints; reuse for readiness checks |
| Install flow (`useInstallGame`)             | `hooks/useInstallGame.ts`           | EC-1 sub-flow; already handles stages, validation routing, candidate discovery             |
| `install_game` / `validate_install_request` | `install/service.rs`                | Backend validation with explicit per-field error variants                                  |
| `useLaunchState`                            | `hooks/useLaunchState.ts`           | Two-step launch sequence; reuse for final onboarding launch step                           |
| `profile/models.rs`                         | `TrainerLoadingMode`, `GameProfile` | Domain types; onboarding must guide selection of `loading_mode`                            |
| `stage_trainer_into_prefix`                 | `launch/script_runner.rs`           | `CopyToPrefix` staging; no changes needed, but onboarding guidance must explain it         |
| SQLite metadata layer                       | `metadata/`                         | Persist onboarding completion state; `cache_store` or a new `onboarding_store` table       |
| Health snapshot store                       | `metadata/health_store.rs`          | Pattern for persisting per-profile status; onboarding completion could follow same schema  |

### Integration Constraints

- The onboarding wizard UI should live as a new page (`OnboardingPage`) or modal flow, consistent with the existing page-based navigation (`App.tsx` sidebar pattern).
- Onboarding state persistence must use the SQLite metadata layer (`metadata/db.rs`), not TOML files, because it is machine-specific and not profile-portable.
- New Tauri commands needed: `check_readiness` (new), `get_onboarding_state` / `set_onboarding_state` (new), `auto_populate_steam` (exists), `validate_install_request` (exists), `install_game` (exists), `launch_game` / `launch_trainer` (exist).
- The DiagnosticCollector pattern (`diagnostics`, `manual_hints` paired lists) is the established idiom for surfacing discovery feedback; any new readiness checks should emit through the same structure.

---

## Success Criteria

1. A first-time user can complete a full profile-setup-to-first-launch flow from within the app without external documentation.
2. The readiness checklist correctly detects the absence of Steam, Proton, compatdata, or a trainer, and provides a specific corrective action for each.
3. Users understand the difference between `SourceDirectory` and `CopyToPrefix` without reading external docs.
4. Onboarding completion state persists across app restarts (no re-showing to users who dismissed/completed it).
5. The guided workflow does not duplicate logic that already exists in `AutoPopulate`, `InstallPage`, or `LaunchPage`; it orchestrates existing components.

---

## Implementation Constraints (from technical review)

### SQLite Schema

- Current schema is at migration version 10. New onboarding tables must be migration 11+.
- Store runs WAL mode, `foreign_keys ON`, `secure_delete ON`.
- `MetadataStore::disabled()` is returned when SQLite is unavailable; `is_available()` returns false. All onboarding features using SQLite must degrade gracefully.
- For a **global** first-run flag, `AppSettingsData` (TOML) is simpler than a SQLite table — add `onboarding_completed: bool` with `#[serde(default)]`. For per-profile readiness state, SQLite is appropriate.

### IPC Boundary

- Tauri commands use `Result<T, String>` — validation errors are stringified before crossing the IPC boundary. Onboarding frontend must do string-based error routing (following the existing `mapValidationErrorToField` pattern in `useInstallGame.ts`).
- Any new store must be `.manage()`'d in `lib.rs` at startup.

### Trainer Path Validation Gap

- `validate_optional_trainer_path` checks file existence and `is_file()` but does **not** check the `.exe` extension. Onboarding must add an explicit `.exe` extension check (same heuristic as `is_windows_executable` in `install/service.rs`).

### Loading Mode Default Recommendation

- `SourceDirectory` is the Rust default, but FLiNG trainers frequently bundle support DLLs and assets that require `CopyToPrefix` to function correctly inside the Wine filesystem. The onboarding UI should recommend `CopyToPrefix` for FLiNG trainers and `SourceDirectory` only for single-file executables with no dependencies.

### Launch Method Must Be Explicit

- `resolve_launch_method` auto-detects from heuristics (`steam.enabled`, `.exe` extension). The onboarding wizard must force an explicit method selection — never rely on the fallback chain — because most trainer users need `proton_run` or `steam_applaunch`.

### First-Run Detection Strategy

Three signals can be combined to detect first run (all must be false to avoid false triggers):

1. No profiles exist yet (`profiles` table is empty or profile files absent)
2. No `last_used_profile` in settings TOML
3. No launch history in SQLite `launch_history` table

**Storage decision — open conflict to resolve**: Business analysis prefers `onboarding_completed: bool` on `AppSettingsData` (TOML) for simplicity. Security review prefers SQLite metadata DB for the completion flag (not a separate file that could be trivially reset). For a single-user desktop app the threat difference is negligible, but SQLite is consistent with other persistent metadata. Tech-designer/team-lead should make the final call.

### Steam Discovery Caching

- `attempt_auto_populate` re-scans the filesystem on every call — no caching. Readiness checks that call Steam discovery should cache results within the session (e.g., React state) to avoid repeated scans.

### External API Constraints (from API research)

- **Steam Store API** (`store.steampowered.com/api/appdetails`) — free, no key, no registration. Could surface game metadata (title, thumbnail) during onboarding. Best-effort, no SLA.
- **ProtonDB API** — free, no auth, community-maintained. Could show compatibility tier (`Platinum/Gold/Silver/Borked`) for the selected game. Undocumented but stable.
- **Trainer sources — cannot be automated**: CrossHook can only guide users to download trainers; it cannot fetch or install them. This is a hard business constraint.
- **WeMod account requirement**: WeMod requires a user account (free tier: limited trainers per day; premium ~$2.99/mo). Onboarding guidance must clearly communicate this friction — do not imply WeMod is frictionless.
- **CheatHappens and MrAntiFun**: CheatHappens is paid (~$6.99/mo); MrAntiFun is now WeMod-exclusive. Neither should be presented as primary recommendations.
- **Version mismatch detection opportunity**: Trainer-to-game version mismatches can be detected locally at zero cost via PE header parsing (no external API needed). This is a future enhancement worth noting but out of scope for initial onboarding.

### Security Constraints (from security review)

- **Trainer source text must be hardcoded**: Any trainer acquisition guidance (FLiNG, WeMod references) shown in onboarding UI must be compile-time constants in the binary — never sourced from community tap index data at runtime. A compromised tap could otherwise inject phishing URLs into onboarding.
- **Readiness check state storage**: Persist only boolean/enum values in SQLite (e.g., `steam_found: bool`, `proton_found: bool`, `compatdata_exists: bool`). Do NOT store file paths in the readiness state row — those already live in profiles and would duplicate sensitive path data.
- **No external trainer API**: If any business rule requires a remote trainer database lookup (e.g., to suggest trainers by game name), a full security review is required before implementation. This feature as scoped has no network calls.
- **`CopyToPrefix` staging is path-traversal safe by construction**: `file_stem()` strips all directory components from the trainer host path, so a malicious path like `/trainers/../../etc/passwd.exe` produces only `"passwd"` as the staging directory name — the `../../` is fully discarded. No explicit containment guard is needed for correctness, though a `debug_assert!(staged_directory.starts_with(&staged_root))` is recommended by the security review for future maintainer clarity. No business rule changes required here.

---

## Open Questions

1. ~~**Trigger condition**~~ — **Resolved**: First-run modal auto-opens on first launch (`first_run_complete = false`); subsequent launches show empty-state banner with re-entry link only (BR-8).
2. **Trainer acquisition guidance depth**: Guidance text only, no deep-linking. Hardcoded compile-time constants. Resolved per security constraints.
3. **WeMod-specific guidance**: WeMod requires its own desktop app installed under WINE. Does onboarding need a dedicated WeMod setup sub-flow, or is a clear callout in the guidance text sufficient for MVP?
4. ~~**Readiness check frequency**~~ — **Resolved**: Run at wizard open, re-run on user request only. Results cached in React state for the session.
5. **Multi-game onboarding**: One-time first-run wizard; subsequent games use the existing Profiles/Install/Launch pages. The empty-state banner on those pages is the re-entry point.
6. ~~**Skip dismiss timing**~~ — **Resolved**: write `onboarding_dismissed = true` immediately on Skip, not gated on first successful launch (BR-8).
7. ~~**FLiNG loading-mode default**~~ — **Partially resolved**: default to `SourceDirectory` in the profile model; wizard UI nudges toward `CopyToPrefix` via contextual hint for FLiNG trainers. Full resolution requires team-lead input (see BR-2 conflict note).
8. ~~**Compatdata detection for `proton_run` mode**~~ — **Resolved**: `check_readiness` accepts optional `launch_method`. When `None`, `game_launched_once` returns `NotApplicable { reason: "launch_method_not_set" }` and the UI shows a pending `○` state. After launch method is determined, the wizard re-evaluates: `proton_run` → `NotApplicable` (no scan, show "not required"); `native` → hidden; `steam_applaunch` → advisory scan runs (BR-1).
9. ~~**`onboarding_completed` location**~~ — **Resolved**: `first_run_complete: bool` in `AppSettingsData` TOML (UX research + simplicity wins over SQLite for this global flag).
