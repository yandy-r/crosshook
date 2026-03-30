# UX Research: Trainer-Version Correlation

**Feature**: Trainer and game version correlation with mismatch detection
**Researcher**: UX Researcher
**Date**: 2026-03-29

---

## Executive Summary

Version mismatch between a game and its trainer is a known pain point in the Linux/Steam Deck gaming ecosystem. WeMod's version history removal in 2024 triggered major community backlash, proving users strongly depend on access to version tracking when trainers break. The optimal UX for CrossHook is a **non-blocking, layered warning system**: a persistent inline badge on the profile card signals staleness, an actionable banner on the Launch page provides context and recovery options before launch, and a soft confirmation intercepts "Launch" only when a version change is detected—never blocking launch outright.

Gamepad navigation (critical for Steam Deck) must be first-class: all warning states must be fully keyboard/gamepad navigable with correct focus management, and the A/B button conventions must be honored in any dialogs.

**Confidence**: High (based on competitive analysis, primary sources, and existing codebase review)

---

## User Workflows

### 1.1 Primary Flow: Profile with Detected Version Mismatch

```
User selects profile → Opens Launch page
  └─ Background: Manifest scan runs, detects game version changed since last verified
       └─ Inline mismatch badge appears on profile card
       └─ Actionable warning banner appears on Launch page
            ├─ "Launch Anyway" (primary)  ← focuses by default
            ├─ "Check Compatibility"      ← navigates to Compatibility page
            └─ "Mark as Verified"         ← saves current version as baseline
```

**Decision points:**

- Has the trainer actually broken, or is the version diff minor? → User needs to decide
- Does the user have an alternative trainer version? → Recovery action
- Does the user want to suppress future warnings for this version range? → "I've confirmed it works"

### 1.2 Alternative Flow: First Launch After Game Install/Update

```
User opens CrossHook after game updated in Steam
  └─ Profile health scan (background) detects version delta
       └─ Health Dashboard badge for affected profile → "Version Changed"
       └─ PinnedProfilesStrip badge updated
            └─ User navigates to Launch page
                 └─ Warning banner shown with version diff
```

### 1.3 Alternative Flow: All-Clear

```
User verifies trainer still works → Launches game successfully
  └─ After successful launch, update stored "last verified game version"
       └─ Mismatch warning clears
       └─ Health status records "verified at vX.Y.Z"
```

### 1.4 Recovery Flow: Trainer is Broken

```
User sees mismatch warning → Launches → Game runs but trainer doesn't work
  └─ User returns to CrossHook
       └─ User manually marks profile as "Needs trainer update"
       └─ Profile status: broken / trainer path health issue
       └─ User may browse Compatibility page for newer trainer data
       └─ User updates trainer path in Profiles page
```

### 1.5 Edge Flow: User Ignores Warnings

```
User repeatedly launches despite mismatch warning
  └─ Warning continues to appear (not suppressed automatically)
  └─ "Launch count since mismatch detected" counter shown
  └─ After N launches, suggest "Mark as Verified (trainer still works)" to clear noise
```

**Key insight from WeMod's experience**: Users hate when compatibility tools force them to wait for updated trainers. The recovery path must include "I know it may not work, let me launch anyway" as a first-class option—not buried or disabled.

### 1.6 Community Profile Install Flow (First Launch)

When a community profile is imported but never locally launched, the version state is `version_untracked` with a `NULL` stored buildid:

```
User imports community profile (game_version="1.2.3", trainer_version="v4.5")
  └─ Profile version state: "untracked" (NULL buildid)
       └─ Profile card shows: "Version: 1.2.3 (not yet locally verified)"
       └─ Launch page note: "Community profile — launch to verify compatibility on your system"
            └─ User launches successfully
                 └─ Snapshot written: buildid=XXXXX, game_version="1.2.3"
                      └─ State: version_match
```

This is distinct from a version mismatch — no warning should be shown, just a neutral "not yet verified" note. The community-provided `game_version` field is informational, not a constraint to enforce.

---

## UI/UX Best Practices

### 2.1 Version Status Indicators

**Recommendation**: Use a dedicated version mismatch status that extends the existing `HealthStatus` taxonomy.

#### Existing CrossHook status vocabulary (from codebase)

- `HealthStatus`: `healthy` (✓) | `stale` (⚠) | `broken` (✗)
- `CompatibilityRating`: `platinum` | `working` | `partial` | `broken` | `unknown`
- CSS class pattern: `crosshook-compatibility-badge--{rating}` (already used by `HealthBadge`)

#### Five version states (refined with team input)

| State                   | Badge tier | Icon   | Display text             | When                                                       |
| ----------------------- | ---------- | ------ | ------------------------ | ---------------------------------------------------------- |
| `version_match`         | healthy    | ✓      | `v1.2.3 (verified)`      | current buildid == stored buildid                          |
| `version_mismatch`      | stale      | ⚠      | `Game updated`           | current buildid != stored buildid                          |
| `version_untracked`     | —          | (none) | (no badge shown)         | profile never successfully launched                        |
| `community_unspecified` | unknown    | —      | `No version requirement` | community profile has no version field — **not a warning** |
| `local_unknown`         | unknown    | ?      | `Version unknown`        | cannot read local Steam manifest                           |

**Critical distinction**: `community_unspecified` is NOT a warning state. Many community profiles don't specify a version requirement, meaning the trainer works across versions or the author didn't constrain it. Treat this as neutral/informational only.

**`version_untracked`** shows no badge at all — there is no baseline to compare against. On first successful launch, a snapshot is written and state transitions to `version_match`.

**`local_unknown`** surfaces as "Version unknown" without alarm language — the user may not have the Steam manifest accessible (non-Steam game path, external drive, etc.).

This maps cleanly onto the existing badge component without new CSS colors—`stale` maps to warning (⚠), which is already `var(--crosshook-color-warning)` orange/yellow.

#### Badge content pattern

- **Compact** (profile card, pinned strip): `⚠ v1.3.0` (icon + new version)
- **Verbose** (Launch page warning banner): `Game updated 1.2.3 → 1.3.0 · Trainer compatibility unverified`
- **Tooltip on hover/focus**: "Game version changed since last verified launch. Trainer may still work."

**Confidence**: High — aligns with PatternFly/Carbon status indicator guidance and existing CrossHook patterns.

### 2.2 Mismatch Alert Pattern: The Three-Layer Approach

Based on NN/G's severity taxonomy and notification best practices:

| Layer                | Component                              | Placement                  | Urgency     | Blocks launch? |
| -------------------- | -------------------------------------- | -------------------------- | ----------- | -------------- |
| 1. Indicator         | `VersionMismatchBadge` on profile card | ProfilesPage card          | Low         | No             |
| 2. Actionable banner | Persistent warning strip               | Top of LaunchPage          | Medium      | No             |
| 3. Soft confirmation | Non-modal confirmation callout         | Pre-launch, in LaunchPanel | Situational | No             |

**Layer 1 – Indicator** (always visible, zero friction):

- Small `⚠` badge on the profile card in `ProfilesPage`
- On `PinnedProfilesStrip`: dot badge indicator only (space-constrained)
- On `HealthDashboard`: new "Version Changed" issue category

**Layer 2 – Actionable Banner** (visible when on Launch page with affected profile):

```
┌─────────────────────────────────────────────────────────────────────┐
│ ⚠  Game updated: 1.2.3 → 1.3.0   Trainer compatibility unverified  │
│     [Launch Anyway]  [Check Compatibility]  [Mark as Verified]  [✕] │
└─────────────────────────────────────────────────────────────────────┘
```

- Persistent (does not auto-dismiss)
- Dismissible with ✕ (persists until next version change or manual clear)
- `crosshook-banner--warning` CSS class pattern (consistent with theme)

**Layer 3 – Soft Confirmation** (only on first launch after mismatch detection):

- Not a modal — an inline confirmation within `LaunchPanel`
- "This profile's game version has changed. Launch anyway?" with two buttons
- After confirming once, clear the soft confirmation until the next version change
- **Never use a modal dialog for this** — it's too disruptive for a non-blocking warning

**Confidence**: High — based on NN/G confirmation dialog guidelines (avoid overuse / cry wolf) and Carbon's non-blocking notification pattern.

### 2.3 Compatibility Display in Compatibility Page

The existing `CompatibilityViewer` already displays `game_version`, `trainer_version`, and `compatibility_rating` per entry. For version correlation, augment the display:

- **Version chip state**: When the user's current game version matches a community entry's `game_version`, highlight that card ("This version")
- **Freshness indicator**: Show "Reported X days ago" on each compatibility card
- **Mismatch callout**: If the user's profile has a version mismatch, show a prominent callout at the top of the Compatibility page: "Looking for compatibility data for v1.3.0?"

---

## Error Handling UX

### 3.1 Presenting "Your Trainer May Not Work"

**Do not frame as an error** — it is a warning about an uncertain future state, not a confirmed failure.

Language guidelines:

- ❌ "Trainer is broken" (too definitive, causes alarm)
- ❌ "Version mismatch detected" (too technical)
- ✓ "Game was updated — trainer compatibility unverified"
- ✓ "Game version changed since last verified launch"
- ✓ "Trainer may still work — launch to verify"

This framing borrows from WeMod's approach and Heroic's disclaimer pattern: inform without alarming, and preserve user agency.

### 3.2 Button Labeling

Follow NN/G's guidance: replace vague Yes/No with action-specific labels.

| Scenario               | Primary action  | Secondary action      | Tertiary        |
| ---------------------- | --------------- | --------------------- | --------------- |
| Version changed banner | "Launch Anyway" | "Check Compatibility" | "Mark Verified" |
| Soft confirmation      | "Launch Anyway" | "Cancel"              | —               |
| Health Dashboard issue | "Open Profile"  | "View History"        | —               |

"Launch Anyway" signals: "I've read the warning, I understand the risk, proceed." This is the Microsoft UX guideline pattern of adding "anyway" to positive commit labels for risk-bearing actions.

### 3.3 Recovery Flows

**Flow A: User confirms trainer still works**

- After a successful launch with a mismatch badge, prompt: "Did the trainer work?" (Y/N) in the console/log area
- On "Yes" → auto-save current game version as `last_verified_version`, clear mismatch badge
- On "No" → mark profile as `trainer_broken`, surface in Health Dashboard

**Flow B: User needs a different trainer version**

- Link from warning banner to Compatibility page pre-filtered for this game
- If community data exists for the new version, show a "Compatible trainer found" callout

**Flow C: User wants to suppress warnings**

- "Mark as Verified" — updates the stored baseline version, clears warning
- Explicitly does NOT mean "ignore all future warnings" — next update will warn again
- **No permanent "don't warn again" option** — every new buildid change re-triggers the warning. This is intentional: each game update is a new unknown, and silencing all future warnings would undermine the feature's value. (Business rule confirmed by business-analyzer.)

### 3.4 File Path Security in Error Messages

**Important**: Do not display raw file paths in user-facing warning messages. The health system already surfaces path issues via `IssueCategory` field names (e.g., `missing_trainer`). Version mismatch warnings should follow the same pattern — describe the issue in terms of versions and status, not file system paths.

Example:

- ❌ "Trainer at `/home/user/.config/crosshook/profiles/game.toml` may be incompatible"
- ✓ "Trainer compatibility unverified for game version 1.3.0"

---

## Performance UX

### 4.1 Background Version Checking

Version checking (reading game manifest, comparing with stored version) is a fast local operation — no network required for the core detection. The UX should treat it accordingly:

- **On profile load**: Check version synchronously inline (< 10ms) — no loading state needed
- **On community index sync**: Compare community compatibility data asynchronously — show "Syncing…" in the background, update compatibility badges when done
- **On health scan**: Include version check in the existing health scan loop — no separate trigger needed

### 4.2 Loading States

For the Launch page warning banner:

- If version check is in-flight (very fast), show a skeleton/ghost state for the banner slot only
- Never delay the launch button while version check runs
- Banner appears/updates without page re-render (update in place)

### 4.3 Non-Blocking Notification Strategy

Use **passive, persistent banner** (not toast) for version mismatch:

- Toast is wrong here: the warning would auto-dismiss before the user reads it or takes action
- Modal is wrong: blocks launch and trains users to ignore/click through warnings
- Persistent inline banner is correct: stays visible until acknowledged, actionable, non-blocking

Per LogRocket's toast guidance: toasts are for "low-priority confirmations" that can be dismissed harmlessly — version mismatch requires user attention, so it must persist.

### 4.4 Version Scan on App Startup

- Trigger health scan (including version check) 2-3 seconds after app startup, in background
- Update `PinnedProfilesStrip` badges when complete
- Do not show a loading indicator at app startup for version checks — users don't need to wait
- Store scan timestamp → "Last checked: 5 minutes ago" tooltip on version badge

### 4.5 IPC / Event System Integration

The existing Tauri event pattern should drive UI updates for version status (confirmed by tech-designer):

- **Polling pattern**: `invoke('get_version_status', { profileId })` on profile selection — synchronous, fast, suitable for badge updates on profile load
- **Async push pattern**: Use `listen('version-check-complete', ...)` after the background startup scan — same pattern as `listen('launch-complete', ...)` already in use
- **State management**: Version status data should follow the `EnrichedProfileHealthReport` enrichment pattern — either extend `metadata` in that type or add a parallel `VersionCorrelationReport` enriched in the same batch prefetch
- **No polling**: Do not continuously poll for version changes — check on profile load and app startup only

---

## Competitive Analysis

### 5.1 Steam (Valve)

**Pattern**: Deck Verified tier system — "Verified" / "Playable" / "Unsupported" / "Unknown"

- Color-coded badges: green (Verified) → yellow (Playable) → orange (Unsupported) → gray (Unknown)
- Badge is always visible on game tiles — never hidden behind a click
- Clicking badge opens a detailed breakdown: categories tested, specific issues
- No "mismatch warning" per se — compatibility is checked per-version by Valve, not per-user

**Lesson for CrossHook**: The tiered, always-visible badge system works well. CrossHook's existing `CompatibilityBadge` is a good analogue. Version mismatch should extend this vocabulary, not replace it.

### 5.2 WeMod

**Pattern**: Version Guard + trainer version history selector

- Version Guard stores game snapshots to enable running older game versions with proven trainers
- "History" tab (later "Mod Settings" → version selector) lets users pick a specific trainer version
- **Key UX incident (May 2024)**: WeMod 9.0 removed the History tab → community backlash → restored in 9.0.2
- Users expected to see previous trainer versions and select them manually

**Lesson for CrossHook**:

1. Always expose version history to users — don't hide it
2. Provide a clear "use this trainer version" selection when mismatches occur
3. Users will not accept being locked to the latest version only when compatibility is uncertain

### 5.3 Heroic Games Launcher

**Pattern**: Compatibility data sourced from ProtonDB/Steam with explicit accuracy disclaimer

- Shows ProtonDB tier and Steam Deck compatibility tier inline on game pages
- Issue #2887 added disclaimer text: data "may not be accurate" for non-Steam stores
- Controversial: hid non-GE Proton versions behind a settings toggle; added inline warning text near the selector

**Lesson for CrossHook**: When showing community-sourced compatibility data, always include a caveat that it may not be accurate for the specific store/version combination. The Compatibility page should note data freshness/source.

### 5.4 Vortex (Nexus Mods)

**Pattern**: Incompatibility detection with status notifications

- Reports incompatible BSA/BA2 archives via notifications
- **User complaint**: Notifications are insufficient — don't help identify _which_ mod is causing issues
- **User complaint**: No "launch anyway" override — forces users to resolve before launching

**Lesson for CrossHook**:

1. Warnings must be specific — tell users exactly what changed, not just "something is wrong"
2. Always provide a "launch anyway" escape hatch — never force users to resolve issues to launch

### 5.5 ProtonDB (Community)

**Pattern**: Community-reported compatibility tiers with version-specific reports

- Tiers: Borked → Bronze → Silver → Gold → Platinum → Native
- Each report includes game version, Proton version, hardware specs
- Decky Loader plugin adds badge to Steam Deck library view — tappable to open full report

**Lesson for CrossHook**: Version-specific compatibility data is highly valuable to Linux gamers. CrossHook's Compatibility page should surface version-specific matches prominently ("community data available for your version").

### 5.6 Summary Table

| Tool     | Mismatch detection            | Warning style               | Launch blocked? | Version history     |
| -------- | ----------------------------- | --------------------------- | --------------- | ------------------- |
| Steam    | No (version-level checks)     | Tier badge                  | No              | N/A                 |
| WeMod    | Yes (game version tracking)   | UI badge + history selector | No              | Yes (in settings)   |
| Heroic   | Partial (accuracy disclaimer) | Inline text warning         | No              | Via Proton settings |
| Vortex   | Yes (archive incompatibility) | Notification                | Sometimes yes   | No                  |
| ProtonDB | Community reports             | Tiered badge                | N/A             | Per-version reports |

---

## Gamepad Navigation

### 6.1 Core Requirements for Steam Deck

Based on the Electron/Tauri Steam Deck compatibility guide:

**Button mapping (must follow Xbox/Steam Deck conventions)**:

- A (South button) = Confirm / Click primary action
- B (East button) = Cancel / Back / Dismiss
- X (West button) = Secondary action
- D-pad / Left stick = Navigate between focusable elements

**Focus management rules**:

- When a warning banner appears, it must NOT steal focus from the current element
- When a soft confirmation appears, it MUST capture focus to its first button ("Launch Anyway")
- When dismissed (B or ✕), focus returns to the previously focused element
- Tab order: warning banner → primary action → secondary action → tertiary action → ✕ dismiss

**Layer stack pattern** (from the Electron/Steam Deck guide):

```
State: { layers: ['launch-page', 'version-warning-banner'] }
When banner active: focus restricted to banner buttons
When B pressed: POP_LAYER → focus returns to launch-page context
```

### 6.2 Version Warning Banner — Gamepad Navigation

```
Banner focusable elements (in tab order):
  1. "Launch Anyway" button    ← default focus when banner appears
  2. "Check Compatibility"     ← navigates to Compatibility page
  3. "Mark as Verified"        ← clears mismatch
  4. ✕ Dismiss                 ← B button also dismisses

D-pad Left/Right: moves between buttons
A: activates focused button
B: dismisses banner (same as ✕)
```

### 6.3 Health Dashboard — Gamepad Navigation

Version mismatch issues shown in Health Dashboard should be navigable via D-pad:

- D-pad Up/Down: moves through profile cards
- A: expands issue detail / opens profile
- X: quick action (e.g., "Mark Verified" without leaving dashboard)

### 6.4 Compatibility Page — Gamepad Navigation

The existing `CompatibilityViewer` filter inputs need gamepad consideration:

- Filter inputs should be navigable but not require text input to browse
- Default state: show all entries, scrollable via D-pad
- Filter focus: only when user explicitly navigates to filter area (don't trap in filters)

### 6.5 Focus Trap in Soft Confirmation

If a soft confirmation is used (within `LaunchPanel`):

- Must trap focus within the confirmation block
- B button = Cancel (dismiss confirmation, don't launch)
- A button on "Launch Anyway" = proceed
- Pressing launch button again while confirmation is visible = same as "Launch Anyway"

---

## Recommendations

### 7.1 Must Have (P0)

1. **Version mismatch badge on profile cards** — Extend `HealthBadge` or add `VersionMismatchBadge` using `stale` styling (⚠, `crosshook-compatibility-badge--partial`). Appears in `ProfilesPage`, `PinnedProfilesStrip`, and Health Dashboard.

2. **Actionable warning banner on Launch page** — Persistent `crosshook-banner--warning` strip at top of `LaunchPage` when the active profile has a version mismatch. Buttons: "Launch Anyway", "Check Compatibility", "Mark as Verified", ✕.

3. **Store `last_verified_game_version` in profile metadata** — Required data foundation. On successful launch with mismatch, prompt user to confirm trainer worked and auto-update stored version.

4. **`launch_anyway_count` tracking** — Track how many times user has launched despite mismatch. Used to offer "Mark as Verified" suggestion after repeated launches.

5. **Gamepad-navigable warning banner** — Full A/B/D-pad navigation. B dismisses. Focus goes to "Launch Anyway" by default.

### 7.2 Should Have (P1)

6. **Version history in Compatibility page** — Show "Last verified at vX.Y.Z on [date]" per profile entry. Let users manually set/reset the baseline version.

7. **Health Dashboard: Version Changed issue category** — Add `version_changed` as an `IssueCategory` alongside `missing_trainer`, `missing_executable`, etc. Feeds into existing health score.

8. **Community version match highlight** — On Compatibility page, highlight entries where community `game_version` matches the user's current game version. "Data available for your version" callout.

9. **Post-launch "Did trainer work?" prompt** — Non-blocking inline prompt in `ConsoleView` after launch. Y/N auto-clears or escalates the mismatch status.

10. **Language calibration** — Audit all version mismatch strings to use non-alarming language: "unverified" not "broken", "game was updated" not "mismatch detected".

### 7.3 Nice to Have (P2)

11. **Version change timeline on profile** — Collapsible section in Profiles page showing version history: "v1.2.3 → v1.3.0 on Mar 28 (trainer unverified)".

12. **Batch "Mark as Verified" in Health Dashboard** — Allow selecting multiple profiles with version changes and verifying them in one action.

13. **Proton version correlation** — In addition to game version, track Proton version used at last verified launch. Warn if Proton version also changed.

14. **Community contribution flow** — After marking a trainer as "works at v1.3.0", offer a one-click export to share this as a community compatibility report.

---

## Open Questions

1. **Where is game version read from?** Steam manifest (`appmanifest_*.acf`) contains `LastUpdated` and `buildid`. Does CrossHook's existing Steam discovery module already parse these fields? The `steam/` crate and VDF parser should be checked. **Impact**: determines if version detection is free or requires new parsing work. _(Tech-designer confirmed `CommunityProfileRow` already stores `game_version`/`trainer_version`; local manifest parsing status TBD from `steam/` crate review.)_

2. **What constitutes a version "change"?** **Answered**: Use `steam_build_id` as the comparison key (confirmed by business-analyzer). It changes on every Steam update; trigger warning on any buildid change. Minor-patch false positives are resolved by the user with "Mark as Verified".

3. **How granular is trainer version tracking?** Trainers often have internal version numbers in their filename or metadata. Can CrossHook read trainer file metadata (e.g., PE version resource) or only compare file modification timestamps? _(Unanswered — tech-designer to clarify.)_

4. **How does this interact with `proton_run` vs `steam_applaunch`?** For `steam_applaunch` profiles, Steam auto-updates the game. For `proton_run` profiles, the game path is user-managed. The mismatch detection strategy may differ for non-Steam-managed game paths. _(Unanswered.)_

5. **Version confirmation UX after launch**: **Recommendation settled**: use a passive inline prompt in `ConsoleView` after `listen('launch-complete')` fires — non-blocking. "Did trainer work?" Y/N appears below the launch log output, not as a blocking dialog.

6. **Steam Deck banner text density**: On 1280×800, "Game updated: 1.2.3 → 1.3.0 · Trainer compatibility unverified" may exceed banner width. Short form "Game updated · verify trainer" with full detail visible on focus/expand is safer. _(Needs visual validation.)_

7. **`CommunityProfileRow` version data for matching**: `CommunityProfileRow` already has `game_version`, `trainer_version`, `proton_version`, `compatibility_rating` (confirmed by tech-designer). The Compatibility page version-match highlight (Section 7.2 item 8) is directly achievable with existing data — no new backend work required.

---

## Sources

- [WeMod Version Guard (Medium, 2018)](https://medium.com/wemod/version-guard-781d5e152a13) — snapshot-based version management concept
- [WeMod History Tab Removal Community Thread (2024)](https://community.wemod.com/t/what-happened-to-version-history/311814) — user impact of removing version history
- [WeMod Version History Support Thread](https://community.wemod.com/t/old-trainer-version/25506) — trainer version selection UX
- [Heroic Games Launcher Issue #2887 — Proton Compatibility Accuracy Warning](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2887) — inline disclaimer pattern
- [Heroic Games Launcher Proton Version Visibility Issue](https://www.ghacks.net/2025/08/05/heroic-games-launcher-reverts-a-change-that-hid-non-proton-ge-versions-by-default/) — UI change controversy and rollback
- [ProtonDB Badges Steam Deck Plugin](https://steamdecklife.com/2022/10/18/protondb-badges-steam-deck-plugin/) — tiered badge pattern on Steam Deck
- [NN/G — Confirmation Dialogs Can Prevent User Errors](https://www.nngroup.com/articles/confirmation-dialog/) — "Launch Anyway" button pattern, cry-wolf effect
- [NN/G — Indicators, Validations, and Notifications](https://www.nngroup.com/articles/indicators-validations-notifications/) — taxonomy: indicator vs. notification, severity scale
- [LogRocket — Toast Notifications Best Practices](https://blog.logrocket.com/ux-design/toast-notifications/) — when NOT to use toasts (persistent warnings)
- [PatternFly — Status and Severity](https://www.patternfly.org/patterns/status-and-severity/) — warning/danger/info status icon guidance
- [Making Electron Apps Steam Deck Compatible (Brainhub)](https://brainhub.eu/library/making-electron-apps-steam-deck-compatible) — gamepad focus layer stack, A/B button mapping
- [Carbon Design System — Notification Pattern](https://carbondesignsystem.com/patterns/notification-pattern/) — toast vs inline vs modal decision framework
- [Smashing Magazine — Design Guidelines for Better Notifications UX (2025)](https://www.smashingmagazine.com/2025/07/design-guidelines-better-notifications-ux/) — non-blocking notification best practices
- [Vortex Incompatible Mods Thread](https://forums.nexusmods.com/topic/12785703-vortex-incompatible-mods/) — user pain points with mod compatibility warnings

---

## Search Queries Executed

1. `game launcher version compatibility warning UX patterns desktop app 2024`
2. `WeMod trainer version compatibility warning user experience UI`
3. `Lutris Heroic game launcher compatibility detection UX update warnings`
4. `WeMod Version Guard` (fetched article)
5. `Steam Deck notification UX gamepad controller-friendly warning dialog overlay patterns`
6. `"launch anyway" warning dialog UX pattern desktop app compatibility mismatch`
7. `background version checking non-blocking notification UX pattern desktop application best practices 2023 2024`
8. `Steam ProtonDB compatibility badge status indicator UX Linux game launcher version warning`
9. `NN/G confirmation dialog` (fetched article)
10. `Making Electron apps Steam Deck compatible` (fetched article)
11. `Heroic Games Launcher version mismatch game update compatibility notification proton version warning UI 2023`
12. `Heroic issue #2887` (fetched)
13. `WeMod community trainer version history rollback UX flow game update broke trainer workflow`
14. `Nexus Mods Vortex mod manager version compatibility warning UX mismatch detection outdated mod`
15. `version mismatch status badge inline warning indicator UI design system components 2024`
16. `software update broke compatibility "last known good version" UX recovery workflow pattern game modding`
17. `Steam game update notification Linux Steam Deck trainer mod compatibility 2024`
18. `toast notification vs inline alert vs modal dialog UX decision tree when to use software application 2024`
19. `PatternFly status and severity` (fetched)
20. `LogRocket toast notifications best practices` (fetched)
