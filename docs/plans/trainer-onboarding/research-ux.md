# UX Research: Trainer Onboarding — CrossHook

**Date:** 2026-03-30
**Author:** research-specialist (UX Research Agent)
**Issue:** #37 (P0)
**Output file:** `docs/plans/trainer-onboarding/research-ux.md`

---

## Executive Summary

CrossHook's trainer onboarding feature must guide users — many of whom are new to Linux gaming, Proton, and game trainer concepts — through a setup that involves multiple prerequisites, two distinct loading modes, and a multi-step workflow. The research confirms the team's proposed approach (first-run modal wizard + contextual empty-state banners) aligns with industry best practice. However, several critical patterns are missing from the current design discussion:

1. **Prerequisite readiness gates with per-check actionable errors** — not just a pass/fail overall.
2. **Inline explanation of trainer modes** at the decision point, not in separate docs.
3. **Controller-first focus management** for Steam Deck — the existing `data-crosshook-controller-mode='true'` CSS system is a strong foundation but the wizard must trap focus and use larger touch targets.
4. **Skippability** — forcing users through every step increases abandonment; the wizard must be dismissible after the readiness check step.
5. **Persistent empty-state banners** on the Profiles and Launch pages serve as a re-entry path after dismissal.

---

## User Workflows

### 1.1 Primary Flow: First-Run Guided Wizard

The recommended flow follows a linear "staged disclosure" wizard that appears automatically on first launch.

```
[App Launch]
     │
     ▼
[Step 1: Readiness Check] ─── Auto-scan: Steam installed? Proton available?
     │                         Game launched at least once? Trainer downloaded?
     │  all pass               │ one or more fail
     ▼                         ▼
[Step 2: Find Your Trainer] [Per-check error + fix action button]
     │
     ▼
[Step 3: Trainer Mode] ─── "SourceDirectory" vs "CopyToPrefix"
     │                      Inline explanation with visual metaphor
     ▼
[Step 4: Auto-Populate] ─── Scan Steam library → select game
     │
     ▼
[Step 5: Profile Created] ─── "You're ready to launch!"
     │                         CTA → navigate to Launch page
     ▼
[Wizard Dismissed] ─── Persisted "first-run complete" flag in settings
```

**Key decision points that feed business rules:**

- Readiness gate: all four prerequisites must pass before Step 2 (blocking gate).
- Trainer mode selection: determines `trainer_path` field behavior in the profile (SourceDirectory uses a folder path; CopyToPrefix copies the exe into the Proton prefix).
- Game selection during auto-populate: pre-fills game path, Proton version, prefix path.

### 1.2 Alternative Flow: Dismiss and Return Later

Users who dismiss the wizard should see:

- **Profiles page**: empty-state banner ("No profiles yet — start with the guided setup") with a "Start Setup" button that re-opens the wizard.
- **Launch page**: empty-state banner if no profile is selected, with a "Create Your First Profile" link.
- These banners disappear once a profile exists.

### 1.3 Alternative Flow: Expert Bypass

Power users can bypass the wizard entirely:

- A "Skip guided setup" link in the wizard header (Step 1) navigates directly to the Profiles page.
- Manual profile creation workflow (already exists) remains accessible.
- The readiness check results are surfaced on the Health Dashboard for reference.

### 1.4 Re-onboarding (Trainer not found)

When a trainer path becomes invalid (stale after OS update, file moved):

- Health badge flags the profile.
- The Launch page shows an inline warning with a "Fix trainer path" CTA.
- Clicking the CTA opens a simplified 2-step mini-wizard: path picker → mode confirmation.

---

## 2. UI/UX Best Practices

### 2.1 Industry Standards for First-Run Wizards

_Sources: [Nielsen/Norman — Wizards](https://www.nngroup.com/articles/wizards/), [Eleken — Wizard UI Pattern](https://www.eleken.co/blog-posts/wizard-ui-pattern-explained), [LogRocket — Setup Wizard](https://blog.logrocket.com/ux-design/creating-setup-wizard-when-you-shouldnt/)_

**Confidence: High** (multiple authoritative sources, 2022–2025)

| Principle                     | Application to CrossHook                                                                                    |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Limit to 3–5 core steps       | Proposed 5 steps is at the upper bound; consider merging Steps 4+5 (auto-populate + confirmation)           |
| Each step is self-sufficient  | All necessary context for trainer mode choice must be on Step 3 — no links to docs                          |
| Progress indicator            | Numbered step indicator (1 of 5) plus step title; not a percentage bar (too abstract for non-linear states) |
| Disabled Continue until valid | "Next" button disabled until readiness gate passes; file path validated before Step 3 can advance           |
| Allow backward navigation     | "Back" retains all inputs — no data loss                                                                    |
| Specific button labels        | "Check Prerequisites" not "Next"; "Choose Trainer Mode" not "Continue"                                      |
| Contextual help inline        | Tooltips/popovers anchored to the current step, not links to external docs                                  |
| Smart defaults                | If trainer mode was previously set in any profile, default to that mode                                     |
| Skippability                  | "Skip setup" link at Step 1 header; wizard must not be mandatory for re-launches                            |
| Save & resume                 | If user dismisses mid-wizard, partially completed steps persist in a `first_run_state` settings key         |

### 2.2 Progressive Disclosure for Trainer Mode Explanation

_Source: [NN/G — Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/), [UXPin — Progressive Disclosure](https://www.uxpin.com/studio/blog/what-is-progressive-disclosure/)_

**Confidence: High**

The trainer mode step is the most conceptually difficult. Apply staged progressive disclosure:

**Level 1 (default — shown to all users):**

```
Which loading mode does your trainer use?

  ◉  Source Directory (recommended)
     The trainer runs from its own folder.
     [Learn more ▾]

  ○  Copy to Prefix
     The trainer is copied into the game's Wine prefix.
     [Learn more ▾]
```

**Level 2 (expanded — on "Learn more"):**

```
Source Directory: Use this when your FLiNG or standalone trainer is a .exe
that lives in its own folder (e.g. ~/Trainers/GameName/). CrossHook will
point Proton to that folder. The trainer stays where you put it.

Copy to Prefix: Use this when your trainer must run inside the game's Wine
environment. CrossHook copies the .exe into the Proton prefix before launch.
Choose this for trainers that require .NET or DirectX from the game's prefix.
```

Never show Level 2 content unprompted — cognitive overload prevents decision-making.

### 2.3 Accessible, Gamepad-Friendly Modal Design

_Sources: [A11Y Collective — Modal Accessibility](https://www.a11y-collective.com/blog/modal-accessibility/), [UXPin — Focus Traps](https://www.uxpin.com/studio/blog/how-to-build-accessible-modals-with-focus-traps/), [Valve Steamworks — Steam Deck Recommendations](https://partner.steamgames.com/doc/steamdeck/recommendations)_

**Confidence: High**

Critical requirements for the wizard modal in controller mode:

1. **Focus trap**: Tab/Shift+Tab cycles only within the modal. No background elements receive focus while wizard is open.
2. **Auto-focus first interactive element** on modal open (the readiness "Check" button).
3. **Return focus** to the element that triggered the modal on close.
4. **Role attributes**: `role="dialog"`, `aria-modal="true"`, `aria-labelledby` pointing to the step title.
5. **Touch targets**: Use `--crosshook-touch-target-min` (56px in controller mode) for all buttons. The existing CSS variable system already handles this.
6. **Escape key**: Dismisses modal at Step 1; at Steps 2–5 prompts "Exit setup?" confirmation.
7. **On-screen keyboard**: Any text input (file path entry) must use Tauri's `ShowFloatingGamepadTextInput` API when in controller mode.
8. **Font size**: Minimum 18px for body text in modal; step titles at 20px+ to be readable from couch distance.
9. **Hidden cursor**: When `data-crosshook-controller-mode='true'` is set, cursor should not be visible. Focus ring must be highly visible (use `--crosshook-color-accent-strong: #2da3ff` at 2px+ width).

**Controller navigation within a step:**

```
[D-pad / Left stick]   → Navigate between interactive elements
[A / Cross button]     → Activate focused element (equivalent to click)
[B / Circle button]    → Back (previous step) or dismiss at Step 1
[Right bumper / R1]    → Next step (when current step is complete)
```

### 2.4 Empty State Design

_Sources: [NN/G — Empty States](https://www.nngroup.com/articles/empty-state-interface-design/), [Eleken — Empty State UX](https://www.eleken.co/blog-posts/empty-state-ux), [Carbon Design System](https://carbondesignsystem.com/patterns/empty-states-pattern/)_

**Confidence: High**

Empty states on Profiles and Launch pages serve as persistent re-entry points. Recommended structure:

```
[Illustrated icon — e.g., gamepad + trainer icon]
"No profiles yet"
"Set up your first game + trainer combo in minutes."
[Button: "Start Guided Setup"]  [Text link: "Create manually"]
```

Rules:

- Never leave an empty list with no explanation.
- The CTA button ("Start Guided Setup") reopens the first-run wizard.
- Secondary action ("Create manually") goes directly to the profile form.
- Once any profile exists, these banners are permanently hidden.
- On the Launch page empty state, the secondary action can be "Open Profiles" to select an existing profile.

### 2.5 Contextual Help: Info Icons vs. Question Marks

_Sources: [UX Movement — Question Mark vs Info Icon](https://uxmovement.com/forms/question-mark-vs-info-icon-when-to-use-which/), [Formsort — Effective Tooltips](https://formsort.com/article/tooltips-design-signup-flows/)_

**Confidence: Medium** (clear consensus on distinction, limited dark-theme specific guidance)

- Use `ⓘ` (info icon) **inline** next to field labels for "more detail on this specific field" — triggers a popover on click/focus.
- Use `?` (question mark) to link to **external documentation** — should open the help section, not a popover.
- For gamepad: both icons must be keyboard/gamepad focusable. Use `button` element, not `span` or `div`.
- Popovers must include a close button and be dismissible with Escape or B button.
- Minimum touch target: 44px × 44px (visual size may be 16px icon, but padding extends the tap zone).

---

## 3. Error Handling UX

### 3.1 Readiness Check Error States

_Sources: [Smashing Magazine — Inline Validation](https://www.smashingmagazine.com/2022/09/inline-validation-web-forms-ux/), [NN/G — Error Form Guidelines](https://www.nngroup.com/articles/errors-forms-design-guidelines/)_

**Confidence: High**

Each prerequisite check should display its own status independently:

| Check              | Pass State                                | Fail State + Action                                   |
| ------------------ | ----------------------------------------- | ----------------------------------------------------- |
| Steam installed    | `✓ Steam detected at /usr/share/steam`    | `✗ Steam not found — [Install Steam ↗]`               |
| Proton available   | `✓ Proton 9.0-4 available`                | `✗ No Proton found — [Open Steam Settings ↗]`         |
| Game launched once | `✓ Compatdata prefix exists`              | `⚠ Game may not have run — [Why does this matter? ▾]` |
| Trainer downloaded | `✓ .exe detected at path` (if pre-filled) | `○ Not yet set — add path below`                      |

Design rules:

- `✓` green (`--crosshook-color-success: #28c76f`), `✗` red (`--crosshook-color-danger: #ff758f`), `⚠` yellow (`--crosshook-color-warning: #f5c542`), `○` muted (unset, not an error).
- "Install Steam" and "Open Steam Settings" must open in the system browser or launch the target app — never navigate away from CrossHook.
- The "game launched once" check is advisory (⚠), not blocking — advance is allowed. The "trainer downloaded" check blocks advance until a valid path is entered.
- Show error messages immediately after scan completes, not on submit.

### 3.2 Inline Validation for File/Folder Paths

_Source: [Baymard — Inline Form Validation](https://baymard.com/blog/inline-form-validation), [LogRocket — Form Validation](https://blog.logrocket.com/ux-design/ux-form-validation-inline-after-submission/)_

**Confidence: High**

Validation timing — "Reward Early, Punish Late":

- **On blur** (when user leaves the field): validate the path. If invalid, show error immediately.
- **While typing**: do not show errors mid-type; show inline success once a valid path is detected.
- **On "Next" button attempt**: re-validate all fields in the step and scroll to the first error.

Error message examples (specific, actionable, not vague):

| Situation                   | Bad Message       | Good Message                                                              |
| --------------------------- | ----------------- | ------------------------------------------------------------------------- |
| Path doesn't exist          | "Invalid path"    | "Path not found. Check the folder exists: `/home/user/Trainers/`"         |
| Not an executable           | "Wrong file type" | "Expected a .exe file. Selected: `readme.txt`"                            |
| Path has no read permission | "Access denied"   | "Can't read this path. Check file permissions."                           |
| Proton not found            | "Proton missing"  | "Proton not found. Open Steam → Settings → Compatibility to download it." |

### 3.3 Recovery Flows

- Always preserve user inputs when an error occurs (no clearing the form).
- Provide a "Try again" button for scan-based checks (e.g., re-run readiness scan after user installs Steam).
- For critical failures (e.g., Steam not installed), gray out later steps with an overlay label: "Complete the steps above to continue."
- After fixing an error, the checkmark should update in real-time without requiring a manual re-scan of all checks.

---

## Performance UX

_Sources: [LogRocket — Skeleton Screens](https://blog.logrocket.com/ux-design/skeleton-loading-screen-design/), [Pencil & Paper — Loading Patterns](https://www.pencilandpaper.io/articles/ux-pattern-analysis-loading-feedback), [Adrian Roselli — Accessible Skeletons](https://adrianroselli.com/2020/11/more-accessible-skeletons.html)_

**Confidence: High**

### 4.1 Readiness Check Scanning (Step 1)

The scan runs asynchronously on wizard open. Display phases:

```
Phase 1 (immediate): Show 4 skeleton rows with shimmer animation
Phase 2 (per-result): Replace each skeleton with real status as it resolves
Phase 3 (all done): Enable "Continue" button if all blocking checks pass
```

Individual check timing varies: Steam detection is near-instant; Proton scan may take 500ms–2s if scanning multiple library paths. Show per-item spinners, not a single global spinner — this communicates progress granularity.

**Accessibility note:** Skeleton animations must stop after 5 seconds (WCAG 2.2.2). Use `prefers-reduced-motion` media query to disable shimmer for users with vestibular disorders:

```css
@media (prefers-reduced-motion: reduce) {
  .crosshook-skeleton {
    animation: none;
  }
}
```

### 4.2 Auto-Populate Steam Scan (Step 4)

Steam library scanning can involve reading many manifests. Display:

```
[Spinning icon] "Scanning Steam libraries…"
    → results appear progressively as found
    → each discovered game appears in the list as it resolves
```

Avoid making the user wait for the full scan to complete before seeing results. A "games are still loading" indicator at the bottom of the list is preferable to a blocking full-page spinner.

### 4.3 WCAG Contrast for Skeleton Screens

The existing dark theme uses `--crosshook-color-bg: #1a1a2e` and `--crosshook-color-bg-elevated: #20243d`. Skeleton placeholder blocks need a 3:1 contrast ratio against the background (WCAG 1.4.11). Use `--crosshook-color-surface-strong: #0c1120` as the skeleton base and `--crosshook-color-border: rgba(224,224,224,0.12)` for the shimmer highlight — verify contrast programmatically before ship.

---

## 5. Competitive Analysis

### 5.1 Heroic Games Launcher

_Sources: [Heroic Issue #2050](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2050), [GamingOnLinux Interview](https://www.gamingonlinux.com/2023/01/an-interview-with-the-creator-of-the-heroic-games-launcher/)_

**Confidence: High** (direct issue tracker evidence)

- **Current state**: No onboarding wizard exists as of 2022–2025. Issue #2050 was filed in Nov 2022 as a high-priority, needs-design feature request — never shipped.
- **Pain point identified**: Hidden features (Add Game to Steam, Cloud Saves, Wine manager) go undiscovered because no guided tour exists.
- **Proposed design**: Screenshot-based feature modal + links to Discord/GitHub. Rejected the guided overlay approach as "too complex."
- **Lesson for CrossHook**: Heroic's inaction is an opportunity. A functional wizard will differentiate CrossHook from its closest Linux-gaming-launcher competitors.

### 5.2 Bottles (Wine Prefix Manager)

_Sources: [Bottles Docs](https://docs.usebottles.com/), [Linux Uprising — Bottles](https://www.linuxuprising.com/2021/12/bottles-wine-prefix-manager-released.html)_

**Confidence: Medium** (limited UX documentation, mostly feature descriptions)

- **Pattern used**: First-run download wizard (download Wine runners, detect existing Wine installations).
- **Strength**: Environment-based approach pre-configures dependency bundles (Gaming, Application, Custom) so users aren't presented with raw Wine configuration.
- **Lesson for CrossHook**: Abstract away the technical complexity (SourceDirectory vs CopyToPrefix) behind user-goal-oriented framing: "Does your trainer run standalone or does it need to share the game's environment?" — not technical terminology upfront.

### 5.3 Lutris

_Sources: [Lutris FAQ](https://lutris.net/faq), [MakeUseOf — Lutris Guide](https://www.makeuseof.com/how-to-play-pc-games-on-linux-with-lutris/)_

**Confidence: Medium** (no detailed UX documentation in public sources)

- **Pattern used**: No dedicated wizard. Onboarding is implicit — user logs in, links accounts, searches game library.
- **Strength**: Leverages an online game catalog so "install" is one click.
- **Weakness**: Offers no contextual guidance for troubleshooting compatibility or runner selection; users must consult forums.
- **Lesson for CrossHook**: The catalog approach doesn't apply to CrossHook (trainer files are user-supplied), but the "search and one-click install" metaphor for trainers from a community tap is worth exploring for future iterations.

### 5.4 Steam (Big Picture / Steam Deck UI)

_Sources: [Valve Steamworks Deck Recs](https://partner.steamgames.com/doc/steamdeck/recommendations), [Tom's Hardware — Steam Deck UI on Desktop](https://www.tomshardware.com/news/steam-deck-ui-comes-to-desktop)_

**Confidence: High** (official Valve documentation)

- **Pattern used**: Steam Deck UI redesigned Big Picture mode with controller-first navigation: hidden cursor in gamepad mode, large touch targets, on-screen keyboard auto-triggered for text input, universal search.
- **Key design decision**: Valve explicitly recommends avoiding separate launcher windows that don't support controller navigation — CrossHook must not pop up OS-native file dialogs that break controller flow.
- **Lesson for CrossHook**: CrossHook's `data-crosshook-controller-mode='true'` CSS system mirrors Valve's approach. The wizard must respect this mode. Use Tauri's `ShowFloatingGamepadTextInput` API for any text entry within the wizard when controller mode is active.

### 5.5 WeMod (Windows trainer platform, now rebranded Wand)

_Sources: [WeMod.com](https://www.wemod.com/), [DeckCheatz WeMod Launcher](https://github.com/DeckCheatz/wemod-launcher)_

**Confidence: Medium** (limited public UX documentation)

- **Pattern used**: Desktop app with auto-detect when a matching game process launches. Trainers are downloaded and activated in-app. Linux community built a wrapper launcher (DeckCheatz) that attempts to run WeMod via Proton.
- **Strength**: Zero configuration for Windows users — WeMod auto-matches the running game.
- **Weakness on Linux**: The manual Proton wrapper approach is complex and fragile. Community DeckCheatz launcher requires a multi-page wiki guide — a sign that without proper onboarding, Linux users struggle significantly.
- **Lesson for CrossHook**: The "automatic matching" UX model (game auto-detected → matching trainer shown) is the aspirational end state. The guided wizard is the bridge while that capability is developed. CrossHook's community taps feature is the structural equivalent.

### 5.6 Competitive Summary Matrix

| Product             | Has First-Run Wizard     | Controller Navigation | Prerequisite Checks | Trainer Mode Explanation | Empty-State Guidance |
| ------------------- | ------------------------ | --------------------- | ------------------- | ------------------------ | -------------------- |
| CrossHook (current) | ✗ None                   | ✓ Full gamepad nav    | ✗ None              | ✗ None                   | ✗ None               |
| Heroic              | ✗ Requested, not shipped | Partial               | ✗                   | ✗                        | Partial              |
| Bottles             | ✓ Download wizard        | ✗                     | ✓ Dependencies      | N/A                      | ✓                    |
| Lutris              | ✗ Implicit               | ✗                     | ✗                   | ✗                        | Partial              |
| Steam               | ✓ Game-specific          | ✓ Full (Big Picture)  | N/A                 | N/A                      | ✓                    |
| WeMod (Windows)     | ✓ Account setup          | ✗                     | Partial             | N/A                      | ✓                    |

---

## 6. Recommendations

### 6.1 Must Have (P0 — Shipping with #37)

1. **First-run modal wizard** (5 steps) with:
   - Numbered step indicator
   - Focus trap + gamepad navigation
   - Disabled "Next" until current step is valid
   - "Skip setup" link at Step 1
   - Smart defaults (last-used trainer mode)

2. **Readiness check step** with per-check status icons (✓/✗/⚠/○):
   - Steam installed
   - Proton available
   - Game launched at least once (advisory)
   - Trainer path (unset is fine; blocking only if user tries to advance with an invalid path)

3. **Trainer mode step** with:
   - Two-option card select (SourceDirectory / CopyToPrefix)
   - Collapsed inline explanation per mode (progressive disclosure)
   - Default: SourceDirectory (most common for FLiNG)

4. **Empty-state banners** on Profiles page and Launch page:
   - CTA opens wizard
   - Dismisses once any profile exists

5. **Inline path validation** with specific error messages (not generic "invalid")

6. **Gamepad compatibility**:
   - Full Tab/Shift+Tab focus cycle within wizard
   - Respect `data-crosshook-controller-mode='true'` for touch target sizes
   - Use `ShowFloatingGamepadTextInput` for any text input when in controller mode

### 6.2 Should Have (P1 — Follow-up iteration)

1. **Persistent re-entry**: "Add another trainer setup" link in wizard completion state to immediately chain into a second profile.
2. **Re-onboarding flow**: 2-step mini-wizard for stale trainer paths (triggered from Health Dashboard or Launch page warning).
3. **Skeleton screens** for readiness check scan phase with `prefers-reduced-motion` support.
4. **Contextual info icons** on complex fields in the profile form (ⓘ popover, not external link).
5. **Completion animation**: On wizard finish, animate the newly created profile card appearing in the sidebar/profiles list.

### 6.3 Nice to Have (P2 — Future)

1. **Auto-detect trainer** from running game (equivalent to WeMod's auto-match UX model).
2. **Trainer mode auto-inference**: Detect whether trainer requires game prefix by scanning its imports or manifest.
3. **Community trainer suggestions** during wizard Step 2: "We found 3 community-verified trainers for this game. Want to download one?"
4. **Wizard analytics**: Track step abandonment rates to identify friction points post-ship.

---

## 7. Open Questions

1. **File picker UX in controller mode**: Tauri's native file dialog is not gamepad-navigable. Does the wizard use a typed path input (with Tauri's on-screen keyboard) or a custom in-app file browser component? This is a build vs. configure decision with significant implementation scope.

2. **First-run detection**: Where is the `first_run_complete` flag persisted — in `settings.toml` or SQLite? If settings.toml, what key name? This should be agreed with the tech-designer before implementation.

3. **Wizard trigger on subsequent launches**: If a user dismisses at Step 1, does the wizard re-appear on the next launch? Or does the empty-state banner become the only re-entry? Recommendation: show the wizard once per install, then switch to empty-state banner only.

4. **"Game launched once" check scope**: Does CrossHook check all profiles' game paths, or only the game being configured in the current wizard session? Recommendation: only check the game being configured.

5. **Skip partial profiles**: If a user creates a profile via the wizard but doesn't complete the trainer selection, is a partial profile saved? Recommendation: do not save partial profiles; the wizard should be all-or-nothing, with a "save and exit" option that only saves if all required fields are set.

6. **Tooltip / popover accessibility on Steam Deck**: Hover-based tooltips don't work in controller mode. All contextual help must be click/focus activated. This applies retroactively to any ⓘ icons added to the existing profile form.

---

---

## 8. Codebase UX Grounding

This section grounds the recommendations above in specific CrossHook codebase patterns discovered during analysis. Implementors should reference these files directly rather than building net-new primitives.

### 8.1 Focus Management — `useGamepadNav` Specifics

File: `src/crosshook-native/src/hooks/useGamepadNav.ts`

The two-zone model (sidebar / content) is the normal navigation context. The wizard modal must override this by adding `data-crosshook-focus-root="modal"` to the modal's root element. The `getNavigationRoot()` function already returns the last `[data-crosshook-focus-root="modal"]` element in the DOM as the focus root, overriding the two-zone model for all D-pad and keyboard navigation.

```
Modal uses:   data-crosshook-focus-root="modal"
Existing:     data-crosshook-focus-zone="sidebar|content"
```

The existing `ProfileReviewModal` uses this pattern and includes its own focus-trap implementation (mirroring `LaunchPanel.tsx`). The wizard should adopt the same implementation — not create a third copy.

Gamepad button mapping already wired in the hook:

- `GAMEPAD_CONFIRM_BUTTON = 0` → A / Cross → `confirm()` → `.click()`
- `GAMEPAD_BACK_BUTTON = 1` → B / Circle → `back()`
- `GAMEPAD_LEFT_BUMPER = 4`, `GAMEPAD_RIGHT_BUMPER = 5` → cycle sidebar views (these fire even when a modal is open — wizard must prevent this via the `isModalNavigationRoot` check already in the hook)
- D-pad Up/Down → `focusPrevious()` / `focusNext()` within current zone
- D-pad Left/Right → `switchZone('sidebar')` / `switchZone('content')` — irrelevant inside a modal where `isModalNavigationRoot` is true

Steam Deck detection (`isSteamDeckRuntime()`) combines:

1. `window.SteamDeck` / `VITE_STEAM_DECK` env flag
2. `(pointer: coarse)` media query
3. `(max-width: 1280px) and (max-height: 800px)` media query
4. `userAgent.includes('steam')`

### 8.2 Controller Mode CSS Variables

File: `src/crosshook-native/src/styles/variables.css`

When `data-crosshook-controller-mode='true'` is set on `<html>`, CSS custom properties automatically adapt:

| Property                            | Default                         | Controller Mode       |
| ----------------------------------- | ------------------------------- | --------------------- |
| `--crosshook-touch-target-min`      | 48px                            | 56px                  |
| `--crosshook-touch-target-compact`  | 36px                            | 44px                  |
| `--crosshook-card-padding`          | 28px                            | 32px                  |
| `--crosshook-settings-grid-columns` | `minmax(0,1fr) minmax(0,1.1fr)` | `1fr` (single column) |
| `--crosshook-panel-padding`         | 20px                            | 24px                  |

The wizard wizard steps should use the settings grid column pattern (`--crosshook-settings-grid-columns`) so they automatically collapse to single-column on Steam Deck without any wizard-specific breakpoints.

There is also a `max-height: 820px` media query that reduces padding further — important for the Steam Deck's 800px screen height.

### 8.3 AutoPopulate State Machine — Reuse Directly

File: `src/crosshook-native/src/components/AutoPopulate.tsx`

The existing `AutoPopulate` component implements a FieldCard state machine (`Idle | Saved | Found | Ambiguous | NotFound`) for three fields: App ID, Compatdata path, Proton path. This is precisely the per-item readiness check pattern needed for Step 1 and the game setup step.

The readiness checklist items for Step 1 should use this same visual state taxonomy:

- `Idle` → "Not checked yet" (initial state, skeleton-like)
- `Found` (green) → check passed
- `NotFound` (red) → check failed + action button
- `Ambiguous` (yellow) → advisory warning (e.g., "game launched once" is uncertain)
- `Saved` → already confirmed from a prior session

The existing CSS classes (`crosshook-auto-populate-field--found`, `--not-found`, `--ambiguous`, `--idle`) should be reused, not new classes added.

### 8.4 Modal + Confirmation Guard Pattern — `InstallPage`

File: `src/crosshook-native/src/components/pages/InstallPage.tsx`

`InstallPage` implements the most complete wizard-like pattern in the codebase: a primary modal (`ProfileReviewModal`) with nested confirmation sub-dialogs for destructive actions. Key patterns to adopt:

1. **Dirty state guard**: before closing the wizard mid-setup, check if the user has unsaved edits (`isProfileReviewSessionDirty`). If dirty, show a confirmation sub-dialog before dismissing.
2. **Promise-based confirmation**: `requestReviewConfirmation()` returns a `Promise<boolean>` resolved by the sub-dialog's confirm/cancel buttons. Clean pattern for async gate logic.
3. **Session state**: `ProfileReviewSession` is the model for wizard step state. An `OnboardingWizardState` type should follow the same shape: step index, draft values per step, error strings per step, loading flags.
4. **Navigate on save**: `onNavigate?.('profiles')` is the pattern for navigating after wizard completion — the `InstallPage` passes `onNavigate` from the parent router. The wizard should accept the same prop.

### 8.5 Error + Warning Banner Classes

Existing classes confirmed in `InstallPage.tsx` and `ProfileReviewModal`:

- `crosshook-error-banner` — red, for blocking errors (save failed, launch failed)
- `crosshook-warning-banner` — yellow, for advisory warnings (incomplete fields, stale paths)

These are the correct components for inline feedback inside wizard steps. Do not introduce a new toast/notification component for errors that can be shown inline.

### 8.6 ControllerPrompts — Step-Contextual Variant Needed

File: `src/crosshook-native/src/components/layout/ControllerPrompts.tsx`

The existing `ControllerPrompts` renders static hints: A=Select, B=Back, LB/RB=Switch View. For the wizard, the step context changes what the buttons do:

- Normal: `A=Select, B=Back, LB/RB=Switch View`
- Wizard Step 1: `A=Run Checks, B=Dismiss wizard`
- Wizard Steps 2–4: `A=Next / Confirm, B=Previous step, LB/RB=(disabled)`
- Wizard Step 5 (launch): `A=Launch, B=Back to Step 4`

The `ControllerPrompts` component should accept optional override props (`confirmLabel`, `backLabel`, `showBumpers`) to render contextual hints rather than creating a parallel component.

### 8.7 PageBanner Pattern — No Wizard-Specific Banner Needed

File: `src/crosshook-native/src/components/layout/PageBanner.tsx`

The `PageBanner` component (`eyebrow + title + copy + SVG illustration`) is used on all pages. Empty-state banners within the page body should NOT use `PageBanner` — that is reserved for the top-of-page header. Instead, use a `div.crosshook-panel` with `crosshook-card-padding` and the `crosshook-color-accent-soft` background token for the empty-state CTA card style.

A new `OnboardingArt` SVG component (following the `ProfilesArt`, `LaunchArt` pattern) should be added to `PageBanner.tsx` for use in the wizard modal header and the empty-state banners.

---

## Sources

| Source                                                          | URL                                                                                                        | Date          |
| --------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ------------- |
| Nielsen/Norman — Wizards: Definition and Design Recommendations | <https://www.nngroup.com/articles/wizards/>                                                                | 2023          |
| Nielsen/Norman — Progressive Disclosure                         | <https://www.nngroup.com/articles/progressive-disclosure/>                                                 | 2013 (stable) |
| Nielsen/Norman — Designing Empty States                         | <https://www.nngroup.com/articles/empty-state-interface-design/>                                           | 2021          |
| Nielsen/Norman — Error Form Guidelines                          | <https://www.nngroup.com/articles/errors-forms-design-guidelines/>                                         | 2022          |
| Eleken — Wizard UI Pattern Explained                            | <https://www.eleken.co/blog-posts/wizard-ui-pattern-explained>                                             | 2024          |
| LogRocket — Creating a Setup Wizard                             | <https://blog.logrocket.com/ux-design/creating-setup-wizard-when-you-shouldnt/>                            | 2023          |
| A11Y Collective — Modal Accessibility                           | <https://www.a11y-collective.com/blog/modal-accessibility/>                                                | 2024          |
| UXPin — Focus Traps in Accessible Modals                        | <https://www.uxpin.com/studio/blog/how-to-build-accessible-modals-with-focus-traps/>                       | 2023          |
| Valve Steamworks — Steam Deck Recommendations                   | <https://partner.steamgames.com/doc/steamdeck/recommendations>                                             | 2024          |
| Valve Steamworks — Steam Input Best Practices                   | <https://partner.steamgames.com/doc/features/steam_controller/steam_input_gamepad_emulation_bestpractices> | 2024          |
| Heroic Games Launcher — Issue #2050 (Onboarding Modal)          | <https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2050>                                 | Nov 2022      |
| Smashing Magazine — Complete Guide to Live Validation UX        | <https://www.smashingmagazine.com/2022/09/inline-validation-web-forms-ux/>                                 | 2022          |
| Baymard — Usability Testing of Inline Form Validation           | <https://baymard.com/blog/inline-form-validation>                                                          | 2020          |
| LogRocket — Skeleton Loading Screen Design                      | <https://blog.logrocket.com/ux-design/skeleton-loading-screen-design/>                                     | 2023          |
| Adrian Roselli — More Accessible Skeletons                      | <https://adrianroselli.com/2020/11/more-accessible-skeletons.html>                                         | 2020          |
| UX Design Institute — Onboarding Best Practices 2025            | <https://www.uxdesigninstitute.com/blog/ux-onboarding-best-practices-guide/>                               | 2025          |
| UX Movement — Question Mark vs Info Icon                        | <https://uxmovement.com/forms/question-mark-vs-info-icon-when-to-use-which/>                               | 2021          |
| Bottles Documentation — Why Bottles?                            | <https://docs.usebottles.com/faq/why-bottles>                                                              | 2023          |
| DeckCheatz — WeMod Launcher (Linux)                             | <https://github.com/DeckCheatz/wemod-launcher>                                                             | 2024          |
| GamingOnLinux — Interview with Heroic Creator                   | <https://www.gamingonlinux.com/2023/01/an-interview-with-the-creator-of-the-heroic-games-launcher/>        | 2023          |
