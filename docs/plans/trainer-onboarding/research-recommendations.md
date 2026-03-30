# Trainer Onboarding: Research Recommendations

## Executive Summary

Trainer onboarding (GitHub issue #37, P0) introduces guided acquisition, readiness checking, and a chained workflow for new CrossHook users. The codebase already contains 80%+ of the building blocks needed: Steam discovery, auto-populate, profile creation, install workflows with stage-based state machines, health checks, and a SQLite metadata layer at migration v10. The primary engineering work is **composing existing primitives into a guided experience** rather than building new infrastructure.

This document synthesizes findings from codebase analysis, technical architecture research, business domain analysis, UX research, security evaluation, external API research, and engineering practices review. Zero new npm/cargo dependencies are required for the core onboarding feature. Two security hardening items in the community taps module (git branch argument injection, URL scheme allowlist) should ship before or alongside onboarding since the feature actively encourages tap usage.

The recommended approach is a phased rollout: (1) backend readiness checks reusing existing validation/diagnostic patterns, (2) a first-run modal wizard composing existing components with contextual banners for returning users, and (3) trainer-specific guidance content with loading mode recommendations. The guided workflow must be completable in under 2 minutes for a Steam game — cold-start friction is the core risk.

**Competitive differentiator**: Heroic Games Launcher filed issue #2050 for an onboarding modal (Nov 2022, never shipped). Bottles has a download wizard but no trainer-mode explanation. Lutris has no wizard. CrossHook shipping a functional trainer onboarding wizard would make it the most guided Linux gaming tool for the trainer use case.

---

## Implementation Recommendations

### Approach: Compose Existing Primitives

The codebase already provides the critical pieces. The onboarding feature should compose them rather than introduce new infrastructure.

| Existing Primitive              | Location                                                                | Reuse Strategy                                                                                                         |
| ------------------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Steam discovery + auto-populate | `steam/auto_populate.rs`, `AutoPopulate.tsx`                            | Embed auto-populate step in wizard                                                                                     |
| Diagnostic collector            | `steam/diagnostics.rs:6`                                                | Mirror pattern for readiness check result collection                                                                   |
| Install workflow state machine  | `useInstallGame.ts:244` (stage: idle->preparing->running->review->save) | Clone pattern for onboarding stages                                                                                    |
| Profile validation chain        | `install/service.rs` (`validate_install_request`)                       | Reuse validators for readiness checks                                                                                  |
| Profile form sections           | `ProfileFormSections.tsx`                                               | Compose into profile review step                                                                                       |
| Health issue representation     | `profile/health.rs:31` (`HealthIssue`)                                  | Reuse for readiness issue representation (field/path/message/remediation/severity) instead of defining a parallel type |
| Proton discovery                | `commands/steam.rs` (`list_proton_installs`)                            | Call during readiness checks                                                                                           |
| PageBanner + CollapsibleSection | `layout/PageBanner.tsx`, `ui/CollapsibleSection.tsx`                    | Standard page chrome and guidance text sections                                                                        |
| InstallField component          | `components/ui/InstallField.tsx`                                        | Trainer path file selection with browse + validation error display                                                     |
| Settings persistence            | `settings/mod.rs` (`AppSettingsData`)                                   | Add `onboarding_completed` flag                                                                                        |

### Technology Choices

- **Backend**: New command file `src-tauri/src/commands/onboarding.rs`, optionally back-ended by `crosshook-core/src/onboarding/readiness.rs` if logic warrants extraction from the command handler. Free functions with `&Path`/`&str` arguments — no service struct.
- **Frontend**: New `hooks/useOnboardingFlow.ts` hook + `types/onboarding.ts` + `components/pages/OnboardingPage.tsx` (or modal overlay). No new React Context provider — page-level hook is sufficient.
- **Storage**: Extend `AppSettingsData` with `onboarding_completed: bool` for global first-run detection. No new SQLite migration needed — readiness checks are transient filesystem operations, not persisted state.
- **IPC**: 2-3 new Tauri commands: `check_onboarding_readiness`, `mark_onboarding_complete`, optionally `suggest_trainer_loading_mode`.
- **Guidance content**: Static compiled Rust strings — guidance content changes infrequently and should be version-locked to the app. No bundled JSON files or network dependencies.
- **Dependencies**: Zero new npm/cargo dependencies. All needs are buildable from existing patterns.

### Architectural Decisions (Synthesized from Team Research)

These decisions were evaluated by the tech-designer and validated against practices research:

| Decision                         | Recommendation                                                               | Rationale                                                                                                                                                 |
| -------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **State persistence**            | `onboarding_completed: bool` in `AppSettingsData` (TOML)                     | Simplest option; no SQLite migration. Readiness checks are transient, not worth persisting. Settings TOML is the established pattern for app-level flags. |
| **First-run detection**          | Check settings TOML flag (~0ms), confirm with empty profile list as fallback | Fast primary check with reliable fallback. Avoids DB query on every startup.                                                                              |
| **Readiness check architecture** | Single monolithic `check_readiness()` command                                | All checks are fast filesystem operations (~50ms total). No benefit from parallelization or per-check IPC granularity for MVP.                            |
| **Guidance content delivery**    | Static compiled Rust content                                                 | Zero latency, no external deps, version-locked to app. Content updates ship with app releases.                                                            |
| **Compatdata existence check**   | Filesystem check (`compatdata_path.is_dir()`)                                | Simpler and more reliable than querying `launch_operations` table. The `compatdata_path_for_match` function already derives the path from manifest data.  |

### Phasing

#### Phase 1: Readiness Checks (Backend-First, Low Risk)

Create the readiness check system that validates prerequisites before a user can meaningfully use CrossHook.

**Backend (`crosshook-core/src/onboarding/readiness.rs`):**

- `check_steam_installed()` — reuses `discover_steam_root_candidates` from `steam/discovery.rs`
- `check_proton_available()` — reuses `discover_compat_tools` from `steam/proton.rs`
- `check_game_compatdata(app_id)` — checks compatdata directory existence via filesystem
- `check_trainer_file(trainer_path)` — extends `validate_optional_trainer_path` with `.exe` extension check (reuse `is_windows_executable` from `install/service.rs`) and optional 2-byte `MZ` PE magic header check (no new deps, just `std::fs::read`)
- `aggregate_readiness()` — returns a `ReadinessResult` using `HealthIssue`-compatible structs from existing health module

**Tauri command (`commands/onboarding.rs`):**

- `check_onboarding_readiness` — calls `aggregate_readiness()` and returns results to frontend
- `mark_onboarding_complete` — sets `onboarding_completed = true` in `AppSettingsData`

**Frontend (`hooks/useOnboardingReadiness.ts`):**

- Hook that calls `check_onboarding_readiness` on mount and exposes per-check status

#### Phase 2: Guided Workflow UI (UX-Heavy)

Build the step-based onboarding experience that chains: readiness -> auto-populate -> profile creation -> trainer selection -> launch.

**First-run modal wizard + persistent contextual banners:**

- Modal wizard for first-run experience (focused, can't be missed)
- Contextual "Get Started" banners on ProfilesPage for returning users who dismissed or haven't completed the wizard
- Wizard accessible from Settings as "Setup Assistant" for returning users

**Wizard steps:**

- Step 1: Welcome + readiness dashboard (pass/fail cards per check)
- Step 2: Game selection via file picker or Steam library scan (compose `AutoPopulate.tsx`)
- Step 3: Trainer selection with loading mode guidance (compose `InstallField`)
- Step 4: Profile review (compose `ProfileFormSections.tsx`)
- Step 5: Test launch

**State management (`hooks/useOnboardingFlow.ts`):**

- Stage-based state machine mirroring `useInstallGame.ts`:
  - `welcome` -> `readiness_check` -> `game_setup` -> `trainer_setup` -> `profile_review` -> `test_launch` -> `complete`
- Each stage has its own validation before advancing
- Pure `deriveStatusText`/`deriveHintText` functions (same pattern as install hook)
- Partial completion handling: wizard state held in hook, not persisted until explicit save

**Key UX constraints:**

- (Business analysis) The 5-step flow must be completable in under 2 minutes for a Steam game. Cold-start friction is the #1 risk.
- (UX research) Per-item readiness check status (pass/fail/warning icons) — not a single pass/fail gate. Standard prerequisite-check pattern used by IDEs and installers.
- (UX research) Progressive disclosure for trainer loading mode explanation — two-line summary by default, expand on click. Prevents cognitive overload on the most complex decision.
- (UX research) "Reward Early, Punish Late" inline validation — show success immediately on valid path entry; only show errors after the user leaves the field.
- (UX research) Disabled Continue button until step is valid — prevents users from hitting errors on submit.
- (UX research) Specific inline error messages — "Path not found: `/home/user/Trainers/`" not "Invalid path". Baymard research confirms specific errors reduce abandonment ~30%.
- (UX research) **B button = previous step** inside wizard (not close/dismiss). Users expect the back gesture to step backward. Only close on explicit "Cancel" button. Critical for Steam Deck controller UX.

**P0 Steam Deck risk (from UX research):** OS-native file dialogs do not support gamepad input in controller mode. If the wizard uses the Tauri file picker for trainer/game path selection without a controller-mode fallback, the wizard is broken on Steam Deck. Mitigation: provide a typed path input field alongside the file browse button. The typed input works with Steam's `ShowFloatingGamepadTextInput`. Touch targets must be 56px minimum in controller mode (already in CSS variables as `--crosshook-touch-target-min`).

#### Phase 3: Trainer-Specific Guidance Content

Add contextual help, loading mode recommendations, and trainer type detection.

- Trainer type hints based on filename patterns (e.g., "FLiNG" in name -> recommend CopyToPrefix)
- In-app guidance cards explaining SourceDirectory vs CopyToPrefix trade-offs
- **WeMod disclaimer**: WeMod requires its own desktop app installation under WINE, not just a single `.exe`. The wizard must detect WeMod-like trainers and show a dedicated disclaimer rather than treating them identically to FLiNG trainers.
- Hardcoded guidance text in Rust command responses (no external resource files)

### Security Prerequisites (Must Ship Before or With Onboarding)

The security evaluation identified two WARNING-level issues in the community taps module that become higher-risk when onboarding actively encourages users to add taps. These should be addressed before onboarding ships:

| ID      | Issue                                                                                            | Location                                        | Fix                                                                                     | Effort            |
| ------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------- | --------------------------------------------------------------------------------------- | ----------------- |
| **W-1** | Git branch argument injection — branch names starting with `--` parsed as git CLI flags          | `community/taps.rs`, `normalize_subscription()` | Validate branch names reject `-` prefix; add `--` separator in git fetch/clone commands | ~15 lines + tests |
| **W-2** | Community tap URL scheme allowlist — `file://` and other schemes allow cloning local directories | `community/taps.rs`, `normalize_subscription()` | Allowlist `https://` and `ssh://git@` only; reject other schemes with clear error       | ~5 lines + tests  |

Additionally, these advisory items should be addressed during implementation:

| ID   | Issue                                                             | Fix                                                                                                                             | Effort |
| ---- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | ------ |
| A-1  | Symlink following in `copy_dir_all()` during CopyToPrefix staging | Add `is_symlink()` skip in `script_runner.rs`                                                                                   | Low    |
| A-2  | PE header check at trainer file selection                         | 2-byte `MZ` (`0x4D 0x5A`) magic check via `std::fs::File` — zero new deps, zero new attack surface                              | Low    |
| A-2b | Full PE parser is overspec                                        | Defer goblin/pelite unless concrete arch/version use case defined                                                               | N/A    |
| A-3  | Trainer source URLs must be compile-time constants                | Architecture decision — already recommended (static Rust content)                                                               | N/A    |
| A-4  | Desktop Exec `%` escaping in launcher export                      | Add `%`->`%%` in `escape_desktop_exec_argument()`                                                                               | 1 line |
| A-6b | AV false positive warning                                         | Add guidance text in onboarding UI warning that trainers (memory modification tools) commonly trigger antivirus false positives | Low    |

**Conditional warning (out of v1 scope):**

| ID  | Issue                                 | Trigger                                          | Fix                                                                                      |
| --- | ------------------------------------- | ------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| W-3 | Zip archive path traversal (zip-slip) | Only if `zip` crate extraction is added to scope | Validate archive entry paths reject `..` components and absolute paths before extraction |

### Quick Wins

1. **Add `onboarding_completed` to `AppSettingsData`** — 10-line change in `settings/mod.rs`, enables conditional routing
2. **Expose existing readiness data** — `default_steam_client_install_path` already detects Steam; `list_proton_installs` already finds Proton. A thin readiness command that calls both is trivial.
3. **Empty-state banner on Profiles page** — When `profile_store.list()` returns empty, show a "Get Started" banner linking to the onboarding wizard. Zero backend work.
4. **Trainer file extension check** — Extend `validate_optional_trainer_path` with `.exe` extension validation. Prevents common user mistake of selecting wrong files.

---

## Improvement Ideas

### Related Features

- **Trainer version tracking integration**: The existing `version_snapshots` table (migration v9) already tracks trainer file hashes. The onboarding flow could record the initial trainer hash at profile creation time, enabling version correlation from day one.
- **Community profile discovery**: The existing community taps system (`community/`, `CommunityBrowser.tsx`) could surface "getting started" profiles for popular games, giving new users pre-configured profiles to import rather than building from scratch. (Defer to Phase 5 to keep initial scope focused.)
- **Health dashboard integration**: The existing health check system (`health_store.rs`, `HealthDashboardPage.tsx`) could provide a "post-onboarding health check" that validates the newly created profile before first launch.

### Future Enhancements

- **Trainer auto-detection**: Scan common trainer download directories (`~/Downloads`, `~/.local/share/crosshook/trainers`) for .exe files and suggest matches by game name.
- **ProtonDB compatibility signal**: The undocumented endpoint `protondb.com/api/v1/reports/summaries/{appid}.json` provides free per-game compatibility ratings. Could be used to warn users about poorly rated games or suggest Proton versions. **Risk**: endpoint is undocumented and could disappear. Mitigation: cache responses in SQLite per app_id with TTL, degrade gracefully offline. Requires `reqwest` crate (~2MB binary increase). Defer to post-onboarding phase.
- **Proton version recommendation**: Based on community profile data and/or ProtonDB ratings, suggest the most compatible Proton version for a given game/trainer combination.
- **Onboarding analytics (local only)**: Track which readiness checks fail most often to prioritize UX improvements. Store in SQLite, never phone home.
- **Batch onboarding**: Allow users to set up multiple game+trainer profiles in sequence without restarting the wizard.
- **Trainer ZIP extraction**: Some trainers are distributed as ZIP archives. If needed in future, use the `zip` crate (standard, synchronous, well-maintained). Do NOT shell out to `unzip` binary (breaks in minimal AppImage environments). Defer to post-onboarding — manual extraction is fine for v1.

### Optimization Opportunities

- **Lazy Steam discovery**: The current `discover_steam_root_candidates` scans on every call. Cache the result in memory during the onboarding session (it won't change mid-wizard).
- **Readiness check performance**: All checks are fast filesystem operations (~50ms total). No parallelization needed for MVP, but `tokio::join!` is available if checks become more expensive.
- **Progressive disclosure**: Don't show all readiness checks at once. Show Steam check first; only show Proton check after Steam passes. Reduces cognitive load.

---

## Risk Assessment

### Technical Risks

| Risk                                                             | Severity | Likelihood | Mitigation                                                                                                                                                                                                                                                                      |
| ---------------------------------------------------------------- | -------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| OS-native file dialog broken on Steam Deck Game Mode             | **High** | **High**   | OS file dialogs don't support gamepad input. Provide typed path input alongside browse button; Steam's `ShowFloatingGamepadTextInput` handles keyboard input. Never make file browse the only path selection method.                                                            |
| Steam not installed or in non-standard location                  | High     | Medium     | Graceful fallback with manual path entry; extend `discover_steam_root_candidates` with env var override                                                                                                                                                                         |
| Proton not available (fresh Steam install)                       | High     | Medium     | Clear instruction to install Proton via Steam; link to Steam settings                                                                                                                                                                                                           |
| Trainer loading mode confusion (SourceDirectory vs CopyToPrefix) | Medium   | High       | Default to CopyToPrefix for FLiNG trainers (which bundle support DLLs that need to be inside the WINE filesystem); SourceDirectory only for single-file no-dependency executables. Force explicit method selection — never rely on `resolve_launch_method` fallback heuristics. |
| Game not launched once through Steam (no compatdata)             | Medium   | High       | Detect missing compatdata via filesystem check; provide clear one-step fix: "Launch the game in Steam, then return here"                                                                                                                                                        |
| Flatpak Steam vs native Steam path differences                   | Medium   | Medium     | Already handled by `discover_steam_root_candidates` checking 3 paths including Flatpak                                                                                                                                                                                          |
| WeMod treated like FLiNG (wrong workflow)                        | Medium   | Medium     | Detect WeMod in trainer filename; show dedicated disclaimer about WINE app installation requirement                                                                                                                                                                             |
| Wizard abandoned mid-way (partial state)                         | Medium   | Medium     | No auto-save; wizard state held in hook only, profile persisted on explicit save. No cleanup needed.                                                                                                                                                                            |
| CopyToPrefix staging performance for large trainers              | Low      | Low        | Already mitigated by selective file copying in `stage_trainer_support_files`. No caching of staged files (directory wiped each launch).                                                                                                                                         |
| Onboarding state staleness                                       | Low      | Medium     | Readiness checks are re-runnable on demand; `onboarding_completed` flag can be reset from Settings                                                                                                                                                                              |

### Integration Challenges

1. **Sidebar navigation / modal choice**: A first-run modal wizard avoids sidebar clutter. For returning users, a "Setup Assistant" entry in Settings provides re-entry. Empty-state banners on ProfilesPage provide passive discovery. No new permanent sidebar entry needed.
2. **Auto-load conflict**: The startup flow (`startup.rs`) auto-loads the last used profile. On first run with no profiles, this is a no-op, which is correct. The wizard doesn't interfere with this path.
3. **Profile persistence timing**: The Install workflow uses a deferred save pattern (review modal -> explicit save). The onboarding wizard follows the same pattern — no auto-save until the user confirms in the review step.
4. **Existing modal state management**: The `ProfileReviewModal` pattern uses `reviewConfirmation` state with a promise-based resolver. A first-run modal wizard should NOT reuse this pattern (it's designed for confirmation dialogs, not multi-step wizards). Instead, the wizard manages its own step state independently.

### Performance Considerations

- Readiness checks involve filesystem I/O (checking Steam/Proton paths). On Steam Deck's SD card, this can be slow. Use async Tauri commands with loading indicators.
- The `discover_compat_tools` function scans all Steam libraries for Proton installations. On systems with many Steam libraries, this may take 1-2 seconds. Cache the result for the onboarding session.
- Total readiness check time is estimated at ~50ms for typical systems. No parallelization needed for MVP.

### Security Considerations

**Net assessment (from security evaluation):** The existing codebase has strong injection mitigations (proper shell quoting, argv-based process launch, path sanitization). Two WARNING-level issues in community taps (W-1, W-2) must ship before onboarding because onboarding actively lowers the user's guard ("follow these steps to add a community tap"). See "Security Prerequisites" section above for details.

- **Trainer file trust**: Trainers are inherently risky (memory modification tools). The onboarding guidance should include a disclaimer about trusting trainer sources AND a warning that trainers commonly trigger antivirus false positives (A-6b). Add a 2-byte `MZ` (`0x4D 0x5A`) PE magic header check at file selection as a basic sanity gate — zero new deps, just `std::fs::File`. Do NOT add full PE parsing crates (goblin, pelite) for onboarding — the magic check is sufficient (A-2b).
- **CopyToPrefix staging path safety**: Confirmed safe by security review — `file_stem()` strips all directory components, so `trainer_base_name` can never contain `..` or path separators. Staging destination cannot escape the prefix root.
- **No external downloads**: The onboarding feature should guide users to find trainers but NEVER download them automatically. All file selection must go through the Tauri file picker dialog.
- **Path validation**: All user-provided paths must go through existing validation functions (`validate_optional_trainer_path`, `validate_proton_path`, etc.) to prevent path traversal. Use `PathBuf` construction consistently (already the project standard). Add `is_symlink()` skip in `copy_dir_all()` for CopyToPrefix staging.
- **Curated external links**: Trainer source URLs must be compile-time constants in Rust, not user-configurable, to prevent phishing/malware redirection. Prefer guidance text over deep-links — URLs to external sites change frequently, creating maintenance burden and possible legal ambiguity.
- **Community tap hardening**: Branch name validation (reject `-` prefix) and URL scheme allowlist (`https://`, `ssh://git@` only) must ship before onboarding. See W-1 and W-2 in Security Prerequisites.

---

## Alternative Approaches

### Option A: Dedicated Onboarding Page in Sidebar

A dedicated `OnboardingPage.tsx` in the sidebar, visible only when `onboarding_completed` is false or when manually accessed from Settings.

| Aspect     | Detail                                                                                                              |
| ---------- | ------------------------------------------------------------------------------------------------------------------- |
| **Pros**   | Clean separation, follows existing page pattern, can be hidden post-completion, composable from existing components |
| **Cons**   | New page adds sidebar complexity, needs routing logic, can be skipped accidentally                                  |
| **Effort** | Medium (2-3 weeks)                                                                                                  |
| **Risk**   | Low — follows proven patterns                                                                                       |

### Option B: First-Run Modal Wizard

A multi-step modal that appears on first launch, overlaying the existing UI.

| Aspect     | Detail                                                                                                                                     |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| **Pros**   | No new page/route needed, forces user attention, focused experience, familiar wizard UX                                                    |
| **Cons**   | Modal fatigue, can't easily return to it later, fights with existing confirmation modal patterns, must handle gamepad navigation carefully |
| **Effort** | Medium (2-3 weeks)                                                                                                                         |
| **Risk**   | Medium — needs careful dismiss/skip/resume behavior                                                                                        |

### Option C: Inline Contextual Guidance

Instead of a dedicated wizard, add contextual help banners/tooltips to existing pages (Profiles, Launch, Install) that appear when fields are empty.

| Aspect     | Detail                                                                        |
| ---------- | ----------------------------------------------------------------------------- |
| **Pros**   | No new pages/routes, teaches in context, lower implementation cost            |
| **Cons**   | Scattered UX, no guided flow, user may miss steps, harder to track completion |
| **Effort** | Low-Medium (1-2 weeks)                                                        |
| **Risk**   | Medium — may not solve the core problem of users not knowing where to start   |

### Option D: Hybrid — First-Run Modal + Contextual Banners (Recommended)

Combine Option B (modal wizard for first-run) with Option C (contextual banners on existing pages). The modal provides the guided first-run flow; banners persist afterward for returning users and ongoing discovery.

| Aspect     | Detail                                                                                                                                                                                                                                                     |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**   | Focused first-run experience that can't be missed + ongoing discovery for returning users. Modal wizard manages its own step state independently of existing modal patterns. Can phase independently — wizard in Phase 2, banners as quick win in Phase 1. |
| **Cons**   | Slightly more total work. Must handle "re-open wizard" path (Settings > Setup Assistant).                                                                                                                                                                  |
| **Effort** | Medium-High (3-4 weeks across both phases)                                                                                                                                                                                                                 |
| **Risk**   | Low — each part is independently valuable                                                                                                                                                                                                                  |

**Recommendation**: Option D (Hybrid). The modal wizard ships in Phase 2. Empty-state banners on ProfilesPage are a quick win that can ship in Phase 1 with zero backend work.

---

## Task Breakdown Preview

### Phase 0: Security Hardening (~1 day, prerequisite for onboarding)

| Task Group                 | Tasks                                                                                                 | Complexity |
| -------------------------- | ----------------------------------------------------------------------------------------------------- | ---------- |
| W-1: Branch injection      | Validate branch names reject `-` prefix; add `--` separator in git fetch/clone in `community/taps.rs` | Low        |
| W-2: URL scheme allowlist  | Allowlist `https://` and `ssh://git@` only in `community/taps.rs` `normalize_subscription()`          | Low        |
| A-1: Symlink skip          | Add `is_symlink()` check in `script_runner.rs` `copy_dir_all()`                                       | Low        |
| A-4: Desktop Exec escaping | Add `%`->`%%` in `escape_desktop_exec_argument()`                                                     | Low        |
| Tests                      | Unit tests for each fix                                                                               | Low-Medium |

### Phase 1: Backend Readiness Checks (~3-5 days)

| Task Group       | Tasks                                                                                                                                             | Complexity |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- |
| Core module      | Create `commands/onboarding.rs` (optionally backed by `crosshook-core/src/onboarding/readiness.rs`)                                               | Low        |
| Readiness checks | `check_steam_installed`, `check_proton_available`, `check_game_compatdata`, `check_trainer_file` (with `is_windows_executable` + 2-byte MZ check) | Low-Medium |
| Aggregation      | Single `check_onboarding_readiness` command returning `ReadinessResult` using `HealthIssue`-compatible structs                                    | Low        |
| Settings flag    | Add `onboarding_completed: bool` to `AppSettingsData`                                                                                             | Low        |
| Tauri commands   | `check_onboarding_readiness`, `mark_onboarding_complete`                                                                                          | Low        |
| Quick win        | Empty-state "Get Started" banner on ProfilesPage                                                                                                  | Low        |
| Tests            | Unit tests for each readiness check function                                                                                                      | Medium     |

### Phase 2: Guided Workflow UI (~5-8 days)

| Task Group                         | Tasks                                                                                                                                          | Complexity                     |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| Types                              | `types/onboarding.ts` — stage enum, readiness result, wizard state                                                                             | Low                            |
| Hook                               | `hooks/useOnboardingFlow.ts` with stage-based state machine (mirror `useInstallGame` pattern)                                                  | Medium                         |
| Wizard component                   | First-run modal wizard with step navigation                                                                                                    | Medium                         |
| Readiness step                     | Readiness dashboard with pass/fail cards per check                                                                                             | Low-Medium                     |
| Game setup step                    | Compose `AutoPopulate.tsx` + game path picker                                                                                                  | Medium                         |
| Trainer setup step                 | Compose `InstallField` + loading mode selector with guidance                                                                                   | Medium                         |
| Profile review step                | Compose `ProfileFormSections.tsx` for review                                                                                                   | Low                            |
| Settings re-entry                  | "Setup Assistant" entry in SettingsPage to re-open wizard                                                                                      | Low                            |
| **Controller-mode file selection** | **Typed path input alongside browse button for all file pickers — OS file dialogs don't support gamepad. 56px min touch targets.**             | **Medium (P0 for Steam Deck)** |
| Gamepad navigation                 | Ensure `useGamepadNav` works with wizard step navigation; **B button = previous step (not close)**; all interactive elements gamepad-reachable | Medium                         |

### Phase 3: Trainer Guidance Content (~3-5 days, parallelizable with Phase 2)

| Task Group                      | Tasks                                                                           | Complexity |
| ------------------------------- | ------------------------------------------------------------------------------- | ---------- |
| Loading mode guidance           | Contextual cards explaining SourceDirectory vs CopyToPrefix                     | Low        |
| Trainer type detection          | Filename pattern matching for FLiNG, WeMod, etc.                                | Low-Medium |
| WeMod disclaimer                | Detect WeMod; show dedicated sub-flow or disclaimer about WINE app installation | Medium     |
| Trainer loading mode suggestion | `suggest_trainer_loading_mode` Tauri command with static Rust content           | Low        |
| External resource guidance      | Hardcoded guidance text (not deep-links) about trainer sources                  | Low        |

### Phase 4: Polish and Integration (~2-3 days)

| Task Group                   | Tasks                                                                     | Complexity |
| ---------------------------- | ------------------------------------------------------------------------- | ---------- |
| Post-onboarding health check | Trigger health check on newly created profile                             | Low        |
| Version snapshot baseline    | Record initial trainer hash in `version_snapshots` on profile creation    | Low        |
| Interrupt recovery           | Ensure wizard dismissal is clean (no persisted partial state to clean up) | Low        |
| Testing                      | End-to-end flow testing on Steam Deck (Desktop + Game Mode)               | Medium     |

**Dependencies:**

- Phase 0 should ship before or alongside Phase 1 (security hardening is a prerequisite for the feature that encourages community tap usage)
- Phase 1 has no dependency on Phase 0 for readiness checks, but both should land before Phase 2
- Phase 2 depends on Phase 1 (readiness checks must exist before UI consumes them)
- Phase 3 can run in parallel with Phase 2
- Phase 4 depends on Phase 2 completion

---

## Key Decisions Needed

1. **Onboarding trigger**: Should the wizard appear automatically on first run, or only when the user has zero profiles? (Recommendation: zero profiles + `onboarding_completed` is false — check settings TOML flag first, confirm with empty profile list as fallback)
2. **Trainer loading mode default**: Should the default be `SourceDirectory` (simpler, less copying) or `CopyToPrefix` (more compatible with FLiNG trainers)? (Recommendation: `CopyToPrefix` for FLiNG trainers that bundle support DLLs; `SourceDirectory` only for single-file no-dependency executables. Auto-detect based on filename pattern + sibling file scan. **Note**: some single-exe FLiNG trainers may work with SourceDirectory — UX researcher should validate with actual distributions.)
3. **Wizard presentation**: First-run modal wizard vs. dedicated sidebar page? (Recommendation: modal wizard for first-run, "Setup Assistant" re-entry from Settings, no permanent sidebar entry)
4. **Community profile integration**: Should the wizard offer to import community profiles as an alternative to manual setup? (Recommendation: defer to Phase 5, keep initial scope focused)
5. **Readiness check strictness**: Should all readiness checks pass before allowing the user to proceed? (Recommendation: Steam and Proton are blocking; trainer and compatdata checks are warnings with clear remediation steps)
6. **Onboarding scope**: Per-app-install (global wizard, one-time) or per-profile (shown when creating any new profile)? (Recommendation: per-app-install for Phase 1, with the wizard available for re-use via Settings)
7. **Guided flow chaining**: Does the wizard chain directly into auto-populate, or are they separate entry points? (Recommendation: chain them — wizard Step 2 embeds auto-populate as a sub-step)

---

## Open Questions

1. Should the onboarding wizard support the `native` launch method, or focus exclusively on `steam_applaunch` and `proton_run`? (Native doesn't involve trainers in the typical sense.)
2. ~~How should the onboarding flow handle Steam Deck Game Mode vs Desktop Mode?~~ **Resolved**: All file pickers must have a typed path input alongside the browse button. OS-native file dialogs don't support gamepad input. Steam's `ShowFloatingGamepadTextInput` handles text entry in controller mode. Touch targets must be 56px minimum (already in CSS variables). Valve explicitly recommends against separate launcher windows without controller nav support.
3. Is there a preferred trainer directory convention (`~/.local/share/crosshook/trainers/`) that should be suggested during onboarding?
4. Is trainer `type` field (FLiNG/WeMod) needed for readiness validation, or just for display/guidance?
5. Should the wizard be re-runnable for a second game, or should returning users go through the normal Profiles + Install pages? (Recommendation: wizard is re-runnable via Settings > "Setup Assistant", but normal pages are the primary path for experienced users.)
6. What is the acceptable latency for the complete readiness check? (Estimated ~50ms on typical systems, potentially 1-2s on Steam Deck SD card.)

---

## External Dependency Evaluation (from API Research)

The core onboarding feature requires **zero new dependencies**. The following were evaluated for future phases:

| Dependency               | Use Case                        | Recommendation                    | Notes                                                                                                                                                                                                                         |
| ------------------------ | ------------------------------- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `reqwest`                | ProtonDB API calls (future)     | Defer — not needed for onboarding | Async, tokio-native, ~2MB binary increase. Only needed if ProtonDB compatibility signal is added post-onboarding.                                                                                                             |
| `goblin`                 | PE file parsing                 | Defer — not needed for v1         | Multi-format parser. Only review if arch/version extraction becomes a concrete requirement post-onboarding.                                                                                                                   |
| `pelite`                 | PE version info extraction      | Defer — not needed for v1         | PE-only, richer version APIs. Same deferral rationale as goblin.                                                                                                                                                              |
| `std::fs::File` MZ check | Trainer file validation         | **Recommended for v1**            | 2-byte `MZ` (`0x4D 0x5A`) magic check — zero new deps, zero new attack surface, sufficient for "is this a PE?" validation.                                                                                                    |
| `zip`                    | Trainer ZIP extraction (future) | Defer — not needed for v1         | Standard, synchronous, well-maintained. **Security note (W-3)**: if added later, archive entry path traversal must be explicitly validated to prevent zip-slip attacks. Keep out of v1 scope to avoid this mitigation burden. |
| `ureq`                   | Alternative HTTP client         | Not recommended                   | Sync-only, doesn't fit the async tokio stack. Would require `spawn_blocking`.                                                                                                                                                 |

---

## Cross-References

| Research Area          | Output File              | Key Findings Incorporated                                                                                                                                                                                                                                                                     |
| ---------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Technical Architecture | `research-tech-specs.md` | State persistence decisions, readiness check architecture, first-run detection strategy, modal vs page trade-offs                                                                                                                                                                             |
| Business Analysis      | `research-business.md`   | Domain complexity (HIGH), compatdata #1 pain point, WeMod distinction, 2-minute completion target, CopyToPrefix default for FLiNG, explicit launch method selection                                                                                                                           |
| Engineering Practices  | `research-practices.md`  | Reuse targets, KISS findings (no new context/wizard library/SQLite table/service struct), module boundaries                                                                                                                                                                                   |
| Security Evaluation    | `research-security.md`   | 2 WARNING + 1 conditional WARNING (W-3 zip-slip, out of v1 scope) + 8 advisory items. Staging path confirmed safe. Architecture review of tech-designer proposal: sound. No new deps needed. Community tap hardening before onboarding ships.                                                 |
| External APIs          | `research-external.md`   | ProtonDB endpoint (undocumented, defer), PE parsing (MZ 2-byte check via std::fs sufficient — goblin/pelite deferred), HTTP client (reqwest if needed later), ZIP extraction (defer + W-3 mitigation). Most readiness checks already exist via existing primitives — true new work is narrow. |
| UX Research            | `research-ux.md`         | Competitive gap (Heroic #2050 never shipped), per-item readiness status pattern, progressive disclosure for loading mode, inline validation, Steam Deck P0 file dialog risk, 56px touch targets, controller-mode path input fallback                                                          |
