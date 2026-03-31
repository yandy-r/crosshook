# UX Research: Offline Trainers

## Executive Summary

Offline-first trainer management for CrossHook's Steam Deck users requires deliberate UX choices across four concerns: communicating offline readiness at a glance, guiding users through multi-step offline key activation (Aurora/WeMod on desktop Linux only), surfacing pre-flight dependency checks before launch, and degrading network-only features gracefully without blocking offline-capable workflows.

The primary reference models are: Heroic Games Launcher (illustrates what _not_ to do — its ambiguous offline banner and incorrect "game not installed" errors are recurring user complaints), Aurora's standalone trainer app (models the offline key download workflow but buries it behind a Lifetime PLUS paywall, and does not support offline keys on Steam Deck at all), and FLiNG trainers (fully offline, no UX burden). Industry standards from Nielsen Norman Group, Google's offline UX guidelines, and the Carbon Design System inform the status indicator, error message, and loading state patterns.

CrossHook has strong existing foundations: the `HealthBadge` component with color + icon + text three-factor status signaling, the `CollapsibleSection` progressive disclosure pattern, the `useGamepadNav` two-zone focus model (which exposes `isSteamDeck` for platform-aware UI), and a well-structured dark theme with semantic CSS custom properties (`--crosshook-color-success`, `--crosshook-color-warning`, `--crosshook-color-danger`).

The highest-value additions are: an `OfflineReadinessBadge` on the profile selector and Launch page, a trainer type badge (`FLING` vs `AURORA` vs `WEMOD`) on the profile form, a pre-flight validation panel integrated into the existing Launch page validation flow, a platform-aware Aurora offline key instructional modal (desktop Linux only — Steam Deck shows an "online only" notice instead), and a graceful degraded state for network-blocked Community and ProtonDB features.

**Important platform constraint (confirmed by API research):** Aurora offline keys are hardware-fingerprinted and require running Aurora on Windows. Steam Deck users cannot use Aurora offline — this is not a setup issue but a hard platform limitation. All Aurora-related UX must be platform-aware, using the `isSteamDeck` flag from `useGamepadNav`.

---

## User Workflows

### Primary Workflow: Fully Offline Launch (FLiNG)

**Context**: User has set up a profile with a FLiNG trainer while online, is now on a plane with no Wi-Fi.

**Ideal flow**:

1. Open CrossHook — app launches normally from local state, no network required.
2. Navigate to **Launch** page — profile selector shows current profile with an `OFFLINE READY` badge (green, pill shape).
3. User taps the profile name or uses D-pad to navigate to it.
4. Profile card shows: trainer path exists (green checkmark), hash verified (green checkmark), no network dependency (FLiNG badge).
5. User presses **Launch** — optional pre-flight checklist panel expands showing all checks passing.
6. Launch proceeds. No network calls blocked the path.

**Critical UX requirements**:

- Offline readiness is visible _before_ the user tries to launch.
- The FLiNG trainer badge communicates "this needs no key" without requiring user knowledge of the term "FLiNG".
- Pre-flight check is non-blocking; it shows results instantly (or with a brief spinner) without halting launch.

### Primary Workflow: Aurora Offline Key Setup (Online → Offline Preparation)

**Context**: User wants to take their Aurora-trainer profile on a trip. They are currently online. They need to set up offline mode before leaving.

**Ideal flow**:

1. User opens profile in **Profiles** page.
2. Trainer section shows a trainer type badge: `AURORA` (amber/warning color) with a tooltip "Requires offline key for offline use".
3. A visible call-to-action button: **"Set up offline key"** appears adjacent to the trainer path field.
4. User clicks/selects **"Set up offline key"** — an instructional modal opens.
5. Modal walks through the Aurora offline key process step by step (see modal design in Error Handling section).
6. Modal has a "Mark as configured" toggle that persists in the profile TOML as `trainer.offline_key_configured = true`.
7. Profile now shows `AURORA (Offline Ready)` badge instead of the plain `AURORA` warning badge.
8. User launches game online to trigger Aurora's automatic offline trainer download (the modal explains this).
9. When offline, profile shows full offline-ready status.

**Critical UX requirements**:

- The setup path must be discoverable without the user reading documentation.
- The modal must not require internet — it is purely instructional.
- The system distinguishes between "offline key configured" (user-reported) and "offline trainer verified" (hash confirmed locally).

### Alternative Workflow: Pre-Flight Validation Failure

**Context**: User is offline, tries to launch a profile where the trainer .exe file is missing (moved to external drive).

**Ideal flow**:

1. User selects profile on **Launch** page.
2. Profile badge shows `OFFLINE NOT READY` (amber/danger depending on severity).
3. User presses Launch — pre-flight panel expands automatically showing:
   - Trainer file: MISSING (red X, path shown)
   - Game executable: OK (green check)
   - Network: Offline (blue info icon, not an error)
4. Launch button is disabled with tooltip "Fix issues above to enable launch".
5. Missing trainer row has an inline action: **"Locate file"** (opens file picker to repoint the path).
6. After user relocates trainer, pre-flight re-runs automatically (debounced), shows all green.
7. Launch button re-enables.

**Critical UX requirements**:

- The failed pre-flight must identify _which_ dependency is missing with actionable recovery.
- Inline recovery actions (file picker) must be accessible via gamepad.
- The user must not need to cancel out of a modal to fix the issue.

### Alternative Workflow: Community Tap Sync Offline

**Context**: User opens CrossHook offline. Community page normally fetches latest tap indexes.

**Ideal flow**:

1. User navigates to **Community** page.
2. Page banner area shows an inline notice: `Network unavailable — showing cached profiles from last sync [date]`.
3. Tap list renders from local SQLite cache (already populated from previous sync).
4. Sync button is present but shows a "Sync disabled — offline" state (grayed, tooltip explains).
5. Previously installed profiles are fully accessible.
6. When user goes back online, a passive toast notification offers to sync.

**Critical UX requirements**:

- Community page must not be blank/broken offline — it must show cached data.
- The offline notice is informational, not an error — it uses a blue/info tone, not red.
- Network state changes (comes back online) should be detected and surfaced without requiring page reload.

### Alternative Workflow: Hash Verification During Setup

**Context**: User has just pointed to a trainer .exe file in the profile form.

**Ideal flow**:

1. User fills in trainer path field.
2. Brief spinner (100-200ms debounce then inline spinner next to the field) while hash is computed.
3. Hash computed — field shows a small chip: `SHA-256 verified` or `File not found`.
4. Hash is stored in profile for offline pre-flight checks.
5. No blocking modal — verification happens inline.

---

## UI/UX Best Practices

### Industry Standards for Offline-First Desktop Apps

**Core principle (Google Web.dev offline UX guidelines)**: Inform users of _both_ the application state and the available actions during offline conditions. Never leave users uncertain about what they can and cannot do.

**Never rely on a single design element for status communication.** The Carbon Design System and WCAG 2.1 AA both require at minimum two of: color, shape/icon, and text label. CrossHook's existing `HealthBadge` already does this correctly — it combines icon (✓/⚠/✗), a color class, and a text label. Offline-readiness indicators must follow the same three-factor pattern.

**Color semantics to use** (aligns with existing CSS custom properties):

- Green (`--crosshook-color-success`): Fully offline-ready, trainer verified.
- Amber (`--crosshook-color-warning`): Conditional — needs action before going offline (Aurora key not configured, hash not computed).
- Red (`--crosshook-color-danger`): Cannot be used offline — missing file or expired key.
- Blue (`--crosshook-color-accent`): Informational — network unavailable but this feature uses cached data.

**Status indicators must be glanceable.** On the Steam Deck 1280x800 display, users cannot read fine print in the profile list. Status badges should be visible in the collapsed profile row, not buried in an expanded section.

**Graceful degradation, not blockage.** Heroic Games Launcher's core mistake is showing empty libraries and incorrect "not installed" errors when offline. CrossHook must never degrade to a state where locally-stored data appears missing. All reads from the SQLite metadata layer and local TOML files must remain fully functional offline.

### Accessibility

**Gamepad and keyboard navigation**: Every new interactive element (badges with onClick, modal steps, pre-flight checklist rows) must be reachable via the `FOCUSABLE_SELECTOR` list in `useGamepadNav.ts`:

- Modals must use `data-crosshook-focus-root="modal"` to override two-zone focus.
- Interactive badges (e.g., clicking an AURORA badge to open the setup modal) need `role="button"`, `tabIndex={0}`, and `onKeyDown` for Enter/Space — exactly the pattern used in `HealthBadge`.
- Inline recovery actions in pre-flight rows need ≥48px touch target (`--crosshook-touch-target-min`), expanding to 56px in controller mode (`data-crosshook-controller-mode="true"`).

**Screen reader requirements**:

- `aria-label` on badge chips must include full status context: `aria-label="Trainer offline status: Not ready — Aurora offline key not configured"`.
- Pre-flight checklist rows should use `role="status"` or `aria-live="polite"` when auto-updating after file re-selection.
- Modals need `role="dialog"`, `aria-modal="true"`, `aria-labelledby` pointing at the modal heading.

**Color contrast**: WCAG 2.1 AA requires 3:1 contrast ratio for status indicators against background and against each other. The existing `--crosshook-color-bg-elevated: #20243d` background with `--crosshook-color-success: #28c76f` achieves approximately 5.1:1 contrast — sufficient.

### Progressive Disclosure

The `CollapsibleSection` (`<details>/<summary>`) pattern is the right vehicle for the pre-flight checklist on the Launch page. Default behavior:

- **Collapsed by default** if all checks pass (no noise for the happy path).
- **Expanded by default** if any check fails (the problem is surfaced).
- Summary line shows an aggregate status: "Pre-flight: 3/3 OK" or "Pre-flight: 1 issue".

The Aurora offline key setup modal follows progressive disclosure: a short intro paragraph → expandable "Step-by-step guide" → a "Mark configured" toggle. Users who know what they're doing can skip to the toggle; novices get the walkthrough.

### Responsive Layout

CrossHook targets 1280x800 (Steam Deck). Key constraints:

- Trainer type badge + offline readiness badge must both fit inline with the profile name in the profile selector row without wrapping.
- Pre-flight checklist rows should stack vertically with sufficient row height (≥`--crosshook-touch-target-compact: 36px`).
- The Aurora offline key modal content must fit within a reasonable modal height without scroll, or use a two-column layout that collapses to single-column in controller mode.

---

## Error Handling

### Error States Reference Table

| Scenario                          | Severity      | Message                                                                          | Recovery Action                        |
| --------------------------------- | ------------- | -------------------------------------------------------------------------------- | -------------------------------------- |
| Trainer file missing              | Blocking      | "Trainer not found at [path]"                                                    | "Locate file" (file picker)            |
| Game executable missing           | Blocking      | "Game executable not found at [path]"                                            | "Locate file" (file picker)            |
| Hash mismatch (file changed)      | Warning       | "Trainer file changed since last verification — re-verify before offline use"    | "Re-verify" button                     |
| Aurora offline key not configured | Warning       | "Aurora offline key not set up — trainer will not work offline"                  | "Set up offline key" (opens modal)     |
| Aurora offline trainer expired    | Warning       | "Offline trainer expires in [N] days — launch while online to refresh"           | "Launch online now" shortcut           |
| Network unavailable (tap sync)    | Informational | "Showing cached profiles from [date] — sync when online"                         | Passive, no action required            |
| Network unavailable (ProtonDB)    | Informational | "ProtonDB data unavailable offline — showing last known ratings"                 | Passive                                |
| Hash computation failed           | Error         | "Could not read trainer file — check file permissions"                           | "Retry" button                         |
| Unknown trainer type              | Warning       | "Trainer type not recognized — offline capability unknown"                       | Link to documentation                  |
| Aurora on Steam Deck (offline)    | Blocking      | "Aurora trainers require internet on Steam Deck — connect to Wi-Fi to launch"    | Informational only, no recovery action |
| WeMod not launched recently       | Warning       | "WeMod may require re-authentication — connect to internet before going offline" | "Launch WeMod now" shortcut            |

### Error Message Design Principles (Nielsen Norman Group)

All error messages in CrossHook's offline feature must follow these validated patterns:

1. **Plain language**: "Trainer not found" not "File I/O error: ENOENT". Never expose internal error codes to users unless paired with a plain description.
2. **Precise identification**: Name the specific file or step that failed. "The trainer file at `/home/user/trainers/game.exe` was not found" is better than "A required file is missing".
3. **Actionable recovery**: Every blocking error must have at least one inline CTA button that addresses the root cause. "Locate file" launches a native file picker; "Set up offline key" opens the instructional modal.
4. **Preserve context**: Do not clear the user's profile data on error. The form retains all entered values; only the failing field is marked.
5. **Avoid hostility**: Heroic Games Launcher's "Game is not installed" when offline is a hostile false-negative — it implies the user's work is gone. CrossHook must never imply data loss due to a network state.

### Validation Pattern for Pre-Flight Checks

Pre-flight validation runs in two phases to minimize perceived latency:

**Phase 1 — Instant (synchronous, no I/O):** Check whether fields are non-empty and paths look structurally valid (string not empty, no obviously invalid characters). Result: "path is set / not set".

**Phase 2 — Background (async I/O):** Stat each file path to confirm existence; compare stored hash against recomputed hash (if offline hash verification is enabled). Result: "file exists / not found / hash match / hash mismatch".

The UI shows Phase 1 results immediately when the Launch page renders. Phase 2 results stream in per-check with a row-level spinner → green check / red X. The Launch button is enabled as soon as Phase 1 passes, with a note "Verifying files..." — this optimistic approach avoids blocking launch while async checks run, but the Launch button shows a brief inhibitor state while Phase 2 runs for safety-critical checks (missing trainer).

---

## Performance UX

### Loading States

**Hash computation**: SHA-256 of a typical trainer .exe (5–50 MB) takes 50–500ms on modern hardware. The UX approach:

- Inline spinner adjacent to the trainer path field in ProfileFormSections, not a full-page overlay.
- Text: "Computing hash..." with an animated spinner (CSS `animation: spin`).
- On completion: replace spinner with a small chip showing first 8 hex chars of hash + "verified" label, or a "Mismatch" warning chip.
- Debounce: wait 300ms after the user finishes typing the path before triggering hash computation (avoids repeated I/O during typing).

**Pre-flight validation**: All checks run concurrently (Promise.all equivalent on the Rust side). The Launch page shows a pre-flight section that transitions:

1. Initial state: "Checking..." with a subtle pulse animation on the section header.
2. Per-check result: each row updates as its check completes.
3. Final state: aggregate summary chip ("3/3 OK" or "1 issue") replaces the spinner.

Target: all checks complete within 200ms for local file stat checks. If hash re-verification is needed (large file), it runs as a non-blocking background task.

### Optimistic UI for Offline Readiness

When CrossHook loads, the offline readiness state for all profiles is computed from the last cached health check stored in SQLite — no live file I/O is needed on startup. This means:

- Profile list renders immediately with offline readiness badges from cached state.
- Fresh validation runs in the background and updates badges if state changed.
- Users on a Steam Deck in game mode see instant feedback — no loading spinner before profile list appears.

This matches the "local-first, sync-later" pattern used by offline-first PWAs: local data is always the source of truth for display; network/disk I/O only updates that source.

### Background Validation

A background validator service (Tauri side) should:

1. Run at app startup: stat all trainer/executable paths in all profiles.
2. Run when a profile is selected on the Launch page (targeted, single-profile check).
3. Run on a timer if the app stays open (suggest: every 10 minutes, or on focus-regained event).
4. Persist results to SQLite health store — same infrastructure already used by `HealthDashboardPage`.

This reuses the existing `health_store.rs` and `ProfileHealthContext.tsx` infrastructure rather than introducing a new check system.

---

## Competitive Analysis

### Heroic Games Launcher — What Not to Do

Heroic is the most direct comparator for a Linux game launcher with offline mode. Its offline UX has well-documented failure modes across its GitHub issue tracker:

**Issue: Misleading "not installed" errors offline.** When Heroic is launched without internet, it displays games as "Game is not installed" even for locally-installed games, because it tries to verify installation via the Epic/GOG API rather than local state. Users reported lost work and alarm at apparent data deletion. CrossHook's local-only TOML + SQLite approach sidesteps this, but must ensure no Tauri command returns an offline-equivalent of "not installed".

**Issue: Ambiguous offline banner.** Heroic shows a purple "Offline (ignore)" banner at the top when it detects offline state. The CTA label "ignore" is confusing — users aren't sure if they're ignoring a problem or acknowledging it. A better pattern: "Network unavailable — offline features active" with no "ignore" verb.

**Issue: Empty library in offline mode (v2.7.0).** Heroic showed an empty library in some offline conditions, which users interpreted as data loss. Root cause: cached data was not rendered when API calls failed. CrossHook must ensure the profile list is always rendered from SQLite cache, not from in-flight API response.

**Positive pattern**: Heroic does mark whether individual games support offline mode vs. online-required. This per-game offline flag is the pattern to adapt for CrossHook's per-profile trainer type badge.

**Confidence**: Medium (based on GitHub issues, not official design documentation; reflects real user-reported behavior through 2024)

### Aurora (Cheat Happens) — Offline Key Workflow Reference

Aurora is the upstream model for the Aurora offline key flow CrossHook must guide desktop Linux users through. **Steam Deck users cannot use Aurora offline at all** — this is a hard platform limitation, not a configuration issue.

**Platform constraint (confirmed by API research):** Aurora offline keys are hardware-fingerprinted and require running Aurora natively on Windows. The HWID binding means a Steam Deck running Aurora via Proton cannot generate or use a valid offline key. CheatHappens offers a dedicated native Steam Deck Tool, but it also does not support offline keys.

**Offline key workflow for desktop Linux users** (researched via Cheat Happens support articles and community discussions):

1. User must be logged into Aurora (online).
2. User opens avatar menu → "Offline Key" tab.
3. User clicks to generate an offline key tied to their current hardware fingerprint.
4. Key is downloaded as a file placed in Aurora's installation directory.
5. Each trainer must also be individually "downloaded for offline use" — Aurora auto-downloads when the trainer is opened from favorites, showing a top-right toast "offline trainer downloaded".
6. Offline trainers expire after 14 days from creation date.
7. If hardware changes (GPU swap, etc.), offline key may be invalidated.
8. Only Lifetime PLUS subscribers can generate offline keys.

**Implications for CrossHook's Aurora UX**:

- On **Steam Deck**: show a persistent `ONLINE ONLY` badge (danger/red). The "Set up offline key" CTA must not appear. Instead: "Aurora does not support offline mode on Steam Deck. An internet connection is required to use Aurora trainers." This is informational, not an error — the profile is still fully usable online.
- On **desktop Linux**: the instructional modal explains steps 3-6. The expiry date (14 days) is a critical detail — CrossHook should track `trainer.offline_key_expiry` in the profile TOML if the user provides it, and warn when approaching expiry.
- The Lifetime PLUS requirement is a business blocker for free/standard Aurora subscribers — the modal should acknowledge this constraint.
- The `isSteamDeck` boolean from `useGamepadNav`'s `GamepadNavState` drives which variant to show.

**Confidence**: High (API research confirmed Steam Deck limitation; multiple official support articles and community confirmation cross-reference offline key workflow for desktop)

### WeMod — Online-Required by Default

WeMod requires initial online authentication before offline use. Community reports confirm: users log in once (hotspot if needed), then can operate offline for the session. No explicit offline key — the session persists locally. Offline trainer cache is implicit, not user-managed.

**Implication for CrossHook**: WeMod's offline model is simpler to communicate than Aurora's — a single login step. The trainer type badge for WeMod should link to a simpler instructional modal: "Launch once with internet to authenticate, then go offline."

**Confidence**: Medium (based on WeMod community forum discussions, not official documentation)

### FLiNG Trainers — Fully Offline, No UX Burden

FLiNG trainers are standalone `.exe` files with no network dependency at runtime. The trainer is extracted from an archive, placed in a folder, and run before the game. No keys, no activation, no expiry.

**Implication for CrossHook**: FLiNG profiles are the "happy path" for offline use. The `FLING` trainer type badge is effectively an "offline guaranteed" indicator. No instructional modal is needed — just the badge, path, and hash verification.

**Confidence**: High (FLiNG is well-documented across the game trainer community; confirmed via FLiNG's own site and community guides)

### Steam Deck Game Mode — Gamepad UX Requirements

Valve's official Steamworks documentation for Steam Deck recommends:

- All launcher UI must be fully navigable by controller — no mouse-only interactions.
- The `SetGameLauncherMode` API translates controller input to keyboard/mouse events for launchers in game mode.
- Touch targets should be ≥48px (CrossHook already enforces this via `--crosshook-touch-target-min`).
- Expanding to 56px in controller mode (CrossHook's `data-crosshook-controller-mode="true"` variant already handles this).

CrossHook's `useGamepadNav` hook implements D-pad navigation, bumper page switching, and A/B confirm/back. Any new modal (Aurora offline key, pre-flight checklist) must use `data-crosshook-focus-root="modal"` to correctly intercept the focus zone and prevent the sidebar from receiving D-pad events while the modal is open.

**Confidence**: High (Valve Steamworks documentation is primary source)

---

## Recommendations

### Must Have

**M1 — Offline Readiness Badge on Profile Selector Rows**
Display a compact status chip inline with the profile name in the Launch page profile selector and the Profiles page profile list. States driven by `readiness_score` from `OfflineReadinessReport`:

| Score                  | Badge label     | Color | CSS rating class  |
| ---------------------- | --------------- | ----- | ----------------- |
| 80–100                 | `OFFLINE READY` | Green | `working`         |
| 50–79                  | `NEEDS SETUP`   | Amber | `partial`         |
| 0–49                   | `NOT READY`     | Red   | `broken`          |
| Aurora on Steam Deck   | `ONLINE ONLY`   | Red   | `broken`          |
| Score not yet computed | `UNKNOWN`       | Muted | (no rating class) |

Reuse the `crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--{rating}` CSS pattern from `HealthBadge`. Aria-label must include full status description including the reason (e.g., "Offline status: Online only — Aurora does not support offline mode on Steam Deck").

**M2 — Trainer Type Badge on Profile Form**
A small pill badge in the Trainer section of `ProfileFormSections.tsx` showing `FLING`, `AURORA`, `WEMOD`, or `UNKNOWN`. Color: green for FLING (offline-capable), amber for AURORA/WEMOD (conditional), muted for UNKNOWN. Clicking the badge opens the trainer-type-specific instructional modal. Badge must be keyboard/gamepad accessible (role="button" + tabIndex).

**M3 — Pre-Flight Validation Section on Launch Page**
A `CollapsibleSection` on the LaunchPage between the profile selector and the Launch button. Default: collapsed if all checks pass ("Pre-flight: all good"), expanded if any check fails. Rows: Trainer file (exists/missing), Game executable (exists/missing), Offline key (configured/not configured, for Aurora/WeMod), Network status (informational). Each failing row has an inline recovery CTA. Uses existing `CollapsibleSection` component. `meta` prop shows aggregate status chip.

**M4 — Aurora Platform-Aware Modal**
The Aurora trainer type badge opens a modal whose content is determined by platform:

**On Steam Deck** (`isSteamDeck === true`):

- Title: "Aurora — Internet connection required"
- Body: "Aurora trainers do not support offline mode on Steam Deck. An internet connection is required each time you use an Aurora trainer. This is a limitation of Aurora's hardware-bound offline key system, not a CrossHook issue."
- No setup steps, no "Mark as configured" toggle.
- A single dismiss button ("Got it").

**On desktop Linux** (`isSteamDeck === false`):

- Title: "Set up Aurora for offline use"
- Step 1: "Open Aurora on Windows (in your Proton prefix)"
- Step 2: "Click your avatar → Offline Key tab → generate key" (requires Lifetime PLUS subscription)
- Step 3: "Open each trainer from your Favorites to download an offline copy" (Aurora auto-downloads when opened from favorites)
- Step 4: "Note: offline trainers expire after 14 days from creation, not from first use. Launch online before your trip to refresh."
- Step 5: A date field: "Offline key expiry date" (persisted to profile TOML as `trainer.offline_key_expiry`)
- Toggle: "I have completed setup" (persisted as `trainer.offline_key_configured = true`) — label must be explicit consent
- Required modal attributes: `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, `data-crosshook-focus-root="modal"`.

**M5 — Community Page Offline Degradation**
When network is unavailable, Community page renders from SQLite cache with an informational banner: "Network unavailable — showing profiles cached on [date]. Sync will resume when online." The banner uses accent/info color (`--crosshook-color-accent`), not danger. The Sync button is visually disabled with tooltip "Network unavailable". No data appears lost.

### Should Have

**S1 — Inline Hash Verification in Profile Form Trainer Field**
After trainer path is set (300ms debounce), trigger Tauri command to stat the file and compute SHA-256. Show inline spinner → "✓ SHA-256: abc12345..." chip or "File not found" warning next to the path field. Hash stored in profile TOML for offline pre-flight comparison.

**S2 — Aurora Offline Key Expiry Warning (Desktop Linux only)**
If `trainer.offline_key_expiry` is set and within 3 days of expiry, show the offline readiness badge as `EXPIRING SOON` (amber) rather than `OFFLINE READY`. Pre-flight checklist row for offline key shows: "Offline trainer expires in [N] days — launch while online to refresh." Not applicable to Steam Deck Aurora profiles (those are always `ONLINE ONLY`).

**S2b — WeMod Re-authentication Timing Warning**
Use `launch_history.rs` to check the last launch date for WeMod profiles. If the profile has not been launched in more than 7 days, show an amber warning badge `REAUTH MAY BE NEEDED` with tooltip: "WeMod may require re-authentication — connect to internet and launch WeMod before going offline." Threshold of 7 days is conservative; can be made user-configurable in Settings as a later iteration.

**S3 — Network State Detection and Passive Toast**
Use Tauri's network detection to observe when the device transitions from offline to online. Show a passive toast (bottom of screen, 4-second auto-dismiss): "Network available — tap sync will resume shortly." This allows Community page to auto-refresh without user intervention.

**S4 — "Offline Only" Profile Filter**
In the Launch page profile selector and Profiles page list, a filter option "Offline ready" that shows only profiles with `OFFLINE READY` status. Useful on Steam Deck in game mode before a trip. Implemented as a filter chip above the profile list, consistent with existing filter patterns in `HealthDashboardPage`.

### Nice to Have

**N1 — Visual Trainer Type Icon in Profile Card**
In the pinned profiles strip (`PinnedProfilesStrip.tsx`), add a small icon to indicate trainer type. FLiNG: a network-crossed icon. Aurora: a lock icon with a dot. This reinforces offline status at the quickest-access point. Icon must be paired with text label for accessibility.

**N2 — Pre-Flight History**
The existing `launch_history.rs` records launch events. Extend to record pre-flight validation results per launch. Surface in the `HealthDashboardPage` as "Last pre-flight: all OK / 1 issue" in the profile health row. Reuses existing health snapshot infrastructure.

**N3 — Aurora Key Expiry Countdown Banner on Launch Page**
If the selected profile has an Aurora trainer with expiry within 24 hours, show a dismissable banner at the top of the Launch page: "Aurora offline trainer expires today — launch online to refresh before going offline." Uses `--crosshook-color-warning` background, close button, session-persisted dismissal (sessionStorage key, same pattern as `HEALTH_BANNER_DISMISSED_SESSION_KEY` in `ProfilesPage.tsx`).

---

## Open Questions

**Q1: Should hash verification be opt-in or opt-out?**
Computing SHA-256 of trainer files adds I/O on every profile load. Options: (a) compute once on path change and cache in profile TOML, skipping re-verification unless the user requests it; (b) re-verify on each Launch page visit with a background async check; (c) user preference in Settings. Recommendation: option (a) is the simplest with lowest performance cost. File modification time (mtime) can serve as a cheap staleness signal before re-hashing.

**Q2: How does CrossHook distinguish "offline key configured" from "offline key valid"?**
CrossHook cannot call Aurora's API to verify key validity — it's a closed Windows app in a Proton prefix. The only signal available is the user's self-report (`trainer.offline_key_configured`) and the optional expiry date (`trainer.offline_key_expiry`). This means the `OFFLINE READY` badge for Aurora profiles is always partially trust-based. The UI should be transparent about this: tooltip on AURORA OFFLINE READY badge should note "Based on your reported key configuration — actual validity is confirmed by Aurora."

**Q3: What happens when the Proton prefix is on an external drive that isn't mounted?**
Pre-flight path checks will show the trainer path as missing if the external drive is not mounted. The error message must distinguish "file missing" from "drive not mounted" where possible (stat errno can differentiate these). This may require the Rust backend to provide richer `PreflightCheckResult` variants.

**Q4: WeMod Linux compatibility?**
WeMod's Linux support via Proton/WINE is community-maintained (see `DeckCheatz/wemod-launcher` GitHub project). CrossHook's offline trainer guidance for WeMod should note that WeMod on Linux has additional setup complexity that the offline key modal cannot fully address. Consider linking to the wemod-launcher wiki from the modal.

**Q5: Should the pre-flight check block launch or only warn?**
For trainer file missing: blocking is appropriate (launch without the trainer would be pointless). For hash mismatch: warn but allow — the user may have intentionally updated the trainer. For Aurora key expiry: warn but allow — the trainer may still work even after expiry in some cases. For network unavailable: never block — offline is a valid mode.

---

## Sources

- [Offline UX Design Guidelines — Google Web.dev](https://web.dev/articles/offline-ux-design-guidelines)
- [Designing Offline-First Web Apps — A List Apart](https://alistapart.com/article/offline-first/)
- [Offline-First Architecture — Jusuf Topic, Medium](https://medium.com/@jusuftopic/offline-first-architecture-designing-for-reality-not-just-the-cloud-e5fd18e50a79)
- [Status Indicator Pattern — Carbon Design System](https://carbondesignsystem.com/patterns/status-indicator-pattern/)
- [Badges vs. Pills vs. Chips vs. Tags — Smart Interface Design Patterns](https://smart-interface-design-patterns.com/articles/badges-chips-tags-pills/)
- [Error Message Guidelines — Nielsen Norman Group](https://www.nngroup.com/articles/error-message-guidelines/)
- [An Error Messages Scoring Rubric — NN/G](https://www.nngroup.com/articles/error-messages-scoring-rubric/)
- [Hostile Patterns in Error Messages — NN/G](https://www.nngroup.com/articles/hostile-error-messages/)
- [Heroic Games Launcher — Offline Banner Issue #3603](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/3603)
- [Heroic — "Game is not installed" offline Issue #2606](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2606)
- [Heroic — Empty library offline Issue #2645](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2645)
- [Aurora Offline Key Support (Cheat Happens Zendesk)](https://cheathappens.zendesk.com/hc/en-us/articles/4451585703315-How-do-i-use-my-Offline-Key-in-Aurora)
- [Obtaining Aurora Offline Key (Cheat Happens)](https://cheathappens.zendesk.com/hc/en-us/articles/4408862962835-How-do-i-obtain-an-offline-key-for-my-trainers)
- [Aurora Helpful Q&A (Cheat Happens)](https://cheathappens.zendesk.com/hc/en-us/articles/15617043599123-Aurora-helpful-Q-A)
- [WeMod Offline Mode Community Discussion](https://community.wemod.com/t/offline-mode/94241)
- [WeMod Offline Trainers Discussion](https://community.wemod.com/t/load-trainer-when-offline/180984)
- [FLiNG Trainer — Main Site](https://flingtrainer.com/)
- [Steamworks — Getting Your Game Ready for Steam Deck](https://partner.steamgames.com/doc/steamdeck/recommendations)
- [Optimistic UI Pattern — RxDB](https://rxdb.info/articles/optimistic-ui.html)
- [Progressive Disclosure — UX Patterns](https://ui-patterns.com/patterns/ProgressiveDisclosure)
- [Badge Accessibility — Material Design 3](https://m3.material.io/components/badges/accessibility)
- [User Experiences with Online Status Indicators — ACM DL](https://dl.acm.org/doi/fullHtml/10.1145/3313831.3376240)
- [Design Guidelines for Offline and Sync — Google Open Health Stack](https://developers.google.com/open-health-stack/design/offline-sync-guideline)
