# UX Research: Profile Health Dashboard

**Date**: 2026-03-27
**Feature**: Profile Health Dashboard with Staleness Detection
**Researcher**: UX Research Specialist

---

## Executive Summary

The profile health dashboard must surface per-profile health status (healthy / stale / broken) at a glance, allow targeted drill-down into broken profiles, and present actionable remediation steps — all within CrossHook's existing dark-theme, gamepad-first, 1280×800 constraint. The most critical design requirement is that **no essential information is hover-only**, every status indicator must be reachable via D-pad, and fix actions must be a single confirm-button press away.

Three patterns dominate the research: (1) **inline health badges** appended to each profile row (reusing `crosshook-compatibility-badge` semantics), (2) a **startup notification banner** summarising broken/stale count with a single CTA, and (3) a **progressive-disclosure detail panel** (reusing `CollapsibleSection`) that reveals per-field remediation without loading a new screen.

**Confidence**: High — corroborated by Carbon Design System status-indicator guidance, established game-launcher patterns (Steam, Lutris, Heroic), and existing CrossHook component precedents in `CompatibilityViewer`, `LaunchPanel`, and `useGamepadNav`.

---

## Teammate Input Synthesis (2026-03-27)

_Findings from tech-designer, business-analyzer, and security-researcher incorporated below._

### Health State Model: 3-State Roll-Up + `HealthIssueKind` at Issue Level

**business-analyzer** has defined the `HealthIssueKind` enum in business rules. All six workflow decisions are now resolved.

**Status definitions (business rules, not time-based):**

- `Healthy` — all configured paths exist and are accessible.
- `Stale` — a path that previously existed (or was configured) is now unreachable due to an external change (game uninstalled, Proton version removed, SD card not mounted). The `stale` label reflects "was valid, now broken by external change" — not age.
- `Broken` — required field empty or path is wrong type. Misconfiguration, not external change.

> **Important correction**: There is no time-based staleness threshold. Health status is NOT persisted to disk (security-researcher constraint). "Last checked X days ago" is not a valid UX pattern for this feature — every check is fresh. Remove all references to 7-day threshold.

**Issue-level distinction (business-defined `HealthIssueKind`):**

- `Missing` — ENOENT: path does not exist. Remedy: re-browse to file.
- `Inaccessible` — EACCES: path exists but cannot be read. Remedy: check file permissions.

| Roll-up status (badge) | Issue kind                | Display label | Remediation copy                                               |
| ---------------------- | ------------------------- | ------------- | -------------------------------------------------------------- |
| `healthy`              | —                         | Healthy       | (none)                                                         |
| `stale`                | `missing` (external)      | Stale         | "Path no longer found — game or runtime may have been removed" |
| `broken`               | `missing` (misconfigured) | Missing       | "Path not found — re-browse to the file"                       |
| `broken`               | `inaccessible`            | Inaccessible  | "Path exists but cannot be read — check file permissions"      |
| —                      | `not_configured`          | Not set       | Neutral, muted — optional field                                |

> Note for tech-designer: the `HealthIssueKind` enum (`Missing`/`Inaccessible`) is now a defined business rule. This is the semantic the Phase 2 `code` field should express — the enum values are already named.

**Notification rules (business rules final):**

- Broken → startup banner always
- Stale → badge only (no banner — stale is expected lifecycle noise)
- Unconfigured → badge only (soft tone)
- Notification dismiss: per-session; re-shows next launch if issues persist

**api-researcher** (final answer from codebase inspection): `LaunchValidationIssue` has exactly three fields — `message: String`, `help: String`, `severity: ValidationSeverity`. No machine-readable state, no action IDs. The distinction between `missing` / `inaccessible` / `not_configured` is encoded in string content only. The typed `ValidationError` variant is flattened to strings before IPC.

**UX recommendation — Phase 1: prose-only remediation, single "Open Profile" CTA**

Ship v1 with the existing type shape. Do not add action-id CTAs in Phase 1:

```typescript
interface ProfileHealthResult {
  profile_name: string;
  status: 'healthy' | 'stale' | 'broken';
  issues: LaunchValidationIssue[]; // existing type, reused as-is
  checked_at: string; // ISO 8601
}
```

Each broken issue shows: `message` (what is wrong) + `help` (how to fix it, as prose). At the **profile level** (not per-issue), a single "Open Profile" button navigates to the ProfileEditor for that profile. This is gamepad-accessible in one confirm press and aligns with the business-analyzer's confirmed flow ("CTA is 'Open Profile' to navigate to ProfileEditor").

Rationale: prose help text already tells the user exactly what to do. "Re-browse to the current executable" is actionable even without a wired "Browse…" button — it just adds one navigation step (go to ProfileEditor → find the field → browse). For v1 this is acceptable.

**UX recommendation — Phase 2: add `code` field, enable per-field deep links**

When direct "Browse…" / "Auto-detect" CTAs are in scope (meaningful for Steam Deck), add a `code` field to `LaunchValidationIssue`:

```rust
pub struct LaunchValidationIssue {
    pub message: String,
    pub help: String,
    pub severity: ValidationSeverity,
    pub code: Option<String>,  // e.g. "game_path_missing", "proton_missing"
}
```

The frontend switches on `code` to determine which action button to render. This is the minimal non-breaking extension (api-researcher Option 2). Do not create a `HealthIssue` wrapper type unless health issues and launch validation issues diverge significantly in Phase 2.

**"Open Profile" CTA mapping** (Phase 1 implementation target):

The profile list health detail shows one button per broken/stale profile:

```
[Broken]  My Cyberpunk Trainer
          ✗ Game executable not found.
            Re-browse to the current executable or use Auto-Populate.
          ✗ Trainer path not found.
            Set a trainer path or remove it.
          [Open Profile]   ← single navigation CTA
```

On gamepad: D-pad to the broken profile → Confirm to expand detail → D-pad to "Open Profile" → Confirm → ProfileEditor opens with that profile selected.

### Loading Pattern: Batch Complete (not per-profile streaming)

**api-researcher** clarifies: validation is fast (<50ms typical), fires as a batch via `profile-health-batch-complete` event, not per-profile. This simplifies the loading UX:

1. Frontend calls `invoke('validate_all_profiles')` on component mount (not Rust startup — avoids race condition where events fire before listener registration).
2. Show "Scanning profiles…" placeholder state across all profile rows (spinner badge = `unchecked`).
3. When `profile-health-batch-complete` fires, update all badges atomically.
4. No progressive-per-profile spinner needed.

The startup notification banner (if any) should fire after the batch-complete event settles, not during the scan.

### Startup Validation Approach (business-analyzer)

Startup health check is **passive** — badges appear when the user navigates to the profile list. No modal or blocking flow. Top-level summary banner ("2 profiles broken") recommended for broken profiles; advisory-only for stale.

### Path Display (security-researcher)

`sanitize_display_path()` is in `src-tauri/src/commands/launch.rs:301`. Apply server-side before IPC crossing — frontend then never sees the raw home prefix. Rule: `~/…` notation in **all** UI display and all IPC responses.

Developer logs should use raw paths at `debug` level only, never `info`.

**Copy Report**: The existing `LaunchPanel` Copy Report pattern is acceptable to carry forward, **with one fix**: exported JSON must also use `~/` notation. If the current implementation exports raw absolute paths, that is an existing deficiency to fix in the same pass. Apply `sanitize_display_path()` to all path strings before including them in the report struct.

### Community-Imported Profiles (security-researcher)

When a community-imported profile shows many `missing` states, prepend the issue list with: _"This profile was imported — paths may need to be updated for your system."_ Prevents user confusion about whether CrossHook is broken.

### Architecture Decision: Inline in ProfilesPage

Business-analyzer and UX both agree — health dashboard augments the existing profile list inline, not a separate tab. Lowest gamepad navigation cost; additive to existing `ProfilesPage` surface.

### Component Reuse Corrections (practices-researcher)

- **`severityIcon()` extraction**: Do it. Move the 5-line lookup to `src/utils/severity.ts` alongside `clipboard.ts`. Not an abstraction — pure deduplication.
- **`ValidationIssueItem` extraction**: Wait. Two call sites (LaunchPanel + health dashboard) is below the rule-of-three threshold. Build inline first; extract if a third site emerges.
- **`isStale()` clarification**: Two distinct things exist in the codebase:
  - `isStale(generatedAt: string)` in `LaunchPanel.tsx:119` — checks preview staleness at 60s. **Do not reuse or extract** — wrong threshold for health dashboard.
  - `is_stale: boolean` on `LauncherInfo` (`types/launcher.ts:8`) — backend-computed launcher script staleness. Unrelated.
  - The `checked_at` timestamp is display-only ("Checked just now"). No time-based staleness threshold — health status is not persisted, so age-based stale logic is undefined. Do not add `STALE_THRESHOLD_MS`.
- **Focus trap duplication**: Out of scope — do not touch during this feature.
- **Toast/notification**: Only add if the feature spec includes startup notification. On-demand-only health check requires no toast infrastructure.

### Startup Validation Approach (business-analyzer)

Business-analyzer confirmed: startup health check is **passive** — badges appear when the user navigates to the profile list. No modal or blocking flow. Top-level summary banner ("2 profiles broken") recommended for broken profiles; advisory-only for stale.

### Path Display (security-researcher)

Backend `sanitize_display_path()` converts `/home/yandy/...` to `~/...`. Frontend must display what the backend sends — do NOT reconstruct absolute paths in JavaScript.

### Community-Imported Profiles (security-researcher)

When a community-imported profile shows many `missing` states, the UI must add a contextual note: _"This profile was imported — paths may need to be updated for your system."_ This prevents user confusion ("did CrossHook break?") and avoids blaming the app for a configuration mismatch.

### Architecture Decision Pending

Business-analyzer raises: where does the health dashboard live?

- Option A: Inline in profile list (augment `ProfilesPage`) — lowest navigation cost for gamepad users
- Option B: New dedicated tab — more space, but requires an extra navigation step
- **UX recommendation**: Option A for v1. The profile list already exists; adding badge + collapsible detail panel is additive. A new tab implies more content than a badge + issue list provides.

---

---

## User Workflows

### Primary Flow 1: Startup Health Notification

```
App launches
  └─ Background validation runs for all profiles (on Profiles page mount)
       ├─ [all healthy] → No notification; badges render green
       ├─ [≥1 broken] → Startup banner: "N profiles have broken paths" [Review]
       └─ [stale/unconfigured only] → Badges render, no banner (expected lifecycle noise)
```

**Resolved (business-analyzer)**: Banner only for broken. Stale and unconfigured = badge only. Dismiss is per-session; re-shows next launch if issues persist.

### Primary Flow 2: Profile List Health At-A-Glance

```
User opens Profiles page
  └─ Profile list renders with health badge per entry
       [Broken]  My Cyberpunk Trainer   [Edit] [Recheck]
       [Healthy] Elden Ring + FLiNG     [Edit]
       [Stale]   Witcher 3 Trainer      [Edit] [Recheck]
         ↓
       User selects a broken profile (D-pad Down + Confirm)
         └─ Health detail section expands inline (CollapsibleSection)
              Showing: which path is broken + remediation CTA
```

### Primary Flow 3: Drill-Down to Broken Profile

```
User selects broken profile card
  └─ Detail panel opens (modal OR inline CollapsibleSection)
       Header:   [Broken] profile-name — 2 issues
       Issue 1:  ✗ Game executable not found
                 Path: /home/user/games/game.exe
                 → [Browse…]  [Auto-detect from Steam]
       Issue 2:  ! Trainer path not set
                 → [Set Trainer Path]
       Footer:   [Re-check]  [Close]
```

### Primary Flow 4: Manual Re-check

```
User triggers re-check:
  ├─ [Recheck All] button in dashboard header
  │    → Batch progress indicator (N / total profiles validated)
  │    → Results appear progressively per profile
  └─ [Recheck] on individual profile
       → Spinner on that profile row
       → Badge updates in-place: Broken → Healthy / still Broken
```

### Alternative Flow: Startup Notification with No Interaction

```
App launches → notification banner appears
User ignores it → continues to Launch page → tries to launch broken profile
  └─ Launch validation catches same broken paths (existing LaunchPanel behaviour)
       → User sees familiar "fatal" validation feedback
       → Remediation hints already present (existing `help` field on issues)
```

This means the health dashboard is additive, not the only path to discovering broken profiles.

---

## UI/UX Best Practices

### Health Dashboard Patterns

**Inline status on list rows (recommended primary surface)**
Research confirms ([Carbon Design System](https://carbondesignsystem.com/patterns/status-indicator-pattern/), [Pencil & Paper](https://www.pencilandpaper.io/articles/ux-pattern-analysis-data-dashboards)): status must be visible at-a-glance in the list, not just on drill-down. Append a health badge directly to each profile row, left of the edit button. Use the `crosshook-compatibility-badge--{rating}` class semantic, adapted to health vocabulary.

**Summary banner at page top (recommended secondary surface)**
A short count banner — "3 of 8 profiles have issues" — sets context before the user reads individual rows. Place it above the profile selector, dismissible but persistent until all issues resolved.

**Two-tier disclosure**

- Tier 1: badge on profile row (always visible)
- Tier 2: issue list in `CollapsibleSection` below the selected profile (on selection/expand)
  Do not require a separate page or modal for basic health details; save the modal for complex fix flows (e.g. file picker).

### Status Indicator Design

**Map existing compatibility-badge vocabulary to health states (4-state model per security-researcher):**

| Backend State    | Badge Class Suffix | Color Token                           | Icon | Display Label |
| ---------------- | ------------------ | ------------------------------------- | ---- | ------------- |
| `healthy`        | `working`          | `--crosshook-color-success` (#28c76f) | ✓    | Healthy       |
| `stale`          | `partial`          | `--crosshook-color-warning` (#f5c542) | !    | Stale         |
| `missing`        | `broken`           | `--crosshook-color-danger` (#ff758f)  | ✗    | Missing       |
| `inaccessible`   | `broken`           | `--crosshook-color-danger` (#ff758f)  | ✗    | Inaccessible  |
| `not_configured` | `unknown`          | muted text                            | –    | Not set       |
| `unchecked`      | `unknown`          | muted text (spinner while pending)    | …    | Checking…     |

Note: `missing` and `inaccessible` share the `broken` visual treatment but must carry distinct help text in the issue detail. The badge label distinguishes them for screen readers and controller users who cannot rely on color.

**Accessibility rule (Carbon DS)**: Never rely on color alone. Include icon shape + color + text label. This codebase already follows this pattern in `CompatibilityBadge` and `severityIcon()` in LaunchPanel.

**Traffic light vs. badge vs. icon — recommendation**: Use the **badge pattern** (pill with text + icon), not standalone colored dots. Dots are problematic: hard to hit with gamepad, convey no information to colorblind users, and disappear at small sizes. Badges are already a proven component in this codebase.

### Progressive Disclosure of Health Details

The existing `CollapsibleSection` (`<details>/<summary>`) is the correct primitive. Use it in two modes:

1. **Per-profile in the list**: collapsed by default; expand on profile selection or explicit user action.
2. **Batch summary**: `CollapsibleSection` with title "Health Summary" and meta `3 issues` — collapsed by default, auto-opens if any broken profiles exist on page load.

From [UX Patterns for Devs](https://uxpatterns.dev/glossary/progressive-loading): load the simplest view first (badge summary), then reveal details on request. This reduces cognitive load on the Steam Deck's 1280×800 viewport where vertical space is limited.

### Gamepad Navigation Considerations

The existing `useGamepadNav` hook already handles:

- **D-pad Up/Down**: moves through focusable elements sequentially
- **D-pad Left/Right**: switches between sidebar and content zones
- **L1/R1**: cycles sidebar tabs
- **A (button 0)**: confirms (clicks) the focused element
- **B (button 1)**: back / switches to sidebar zone

For the health dashboard these constraints translate directly to design requirements:

1. **Each profile card must be a single focusable unit** (a `<button>` or `<a>`), not just a visual card. The current profile list uses a `<select>` dropdown — the health dashboard should present profiles as a focusable list of cards where D-pad down moves to the next profile.

2. **The health detail/fix section must open inline** (expand below the card) or use the existing modal pattern. Do NOT use a separate route — the gamepad `back` button is already wired to close modals and return to the sidebar zone.

3. **"Recheck" and "Fix" must be buttons** with `tabindex >= 0` and `min-height: var(--crosshook-touch-target-min)` (48px). Never put these in a hover-only tooltip or context menu.

4. **Controller prompt bar**: The existing `ControllerPrompts` overlay should contextually show the correct hints when a broken profile is focused:
   - `A  Open` / `B  Back` / `Y  Recheck` / `X  Fix`
     (map to gamepad buttons 0/1/3/2 respectively)

5. **Scrolling**: Use `element.scrollIntoView({ block: 'nearest' })` (already in `focusElement()` in `useGamepadNav`) — health badges and fix actions must remain visible when a profile card is focused.

---

## Error Handling UX

### Error States Table

| Backend State    | Field              | Display label  | User-facing message                                                                     | Remediation action                         |
| ---------------- | ------------------ | -------------- | --------------------------------------------------------------------------------------- | ------------------------------------------ |
| `missing`        | game executable    | Missing        | "Game executable not found — file may have moved"                                       | [Browse…] [Auto-detect from Steam]         |
| `inaccessible`   | game executable    | Inaccessible   | "Game executable exists but cannot be read — check file permissions"                    | [Open in File Manager]                     |
| `not_configured` | trainer            | Not set        | "No trainer configured — profile will launch game only"                                 | [Set Trainer Path] _(advisory, not error)_ |
| `missing`        | trainer            | Missing        | "Trainer executable not found — file may have moved"                                    | [Browse…] [Remove Trainer]                 |
| `missing`        | proton version     | Missing        | "Proton runtime not found — version may have been uninstalled"                          | [Change Proton Version] [Auto-detect]      |
| `stale`          | steam prefix       | Stale          | "Steam compat data prefix not initialised — run game once via Steam"                    | [Open Profile]                             |
| `broken`         | profile TOML       | Broken         | "Profile data could not be loaded — file may be corrupt"                                | [Delete Profile]                           |
| `stale`          | any path           | Stale          | "Path no longer found — game or runtime may have been removed or moved"                 | [Open Profile] [Recheck]                   |
| `missing`        | (community import) | Missing + note | "Paths not found — this profile was imported and may need path updates for your system" | [Open Profile]                             |

**Sanitization rule** (from security-researcher): All paths in messages must be in `~/...` notation, sourced from backend `sanitize_display_path()`. Frontend must display what the backend sends — do NOT construct path strings in JavaScript.

**Message design principles** ([Pencil & Paper error UX](https://www.pencilandpaper.io/articles/ux-pattern-analysis-error-feedback)):

- State what happened, not just that an error occurred.
- Tell the user what to do next — include a specific action.
- Show path in a secondary/monospace element, never as the primary message.
- Distinguish `missing` ("file may have moved") from `inaccessible` ("check permissions") — they need different fixes.
- Avoid blame language — prefer passive or system-oriented framing.
- For community-imported profiles with many missing paths, show a contextual note before the issue list.

### Remediation Suggestion UI

The existing `LaunchPanel` `validationFeedback` / `diagnosticFeedback` UI already implements this correctly:

```
[Badge: Fatal]   Game executable not found
                 Path: /home/user/games/Cyberpunk2077.exe
                 [Browse…]  [Auto-detect from Steam]
```

The health dashboard should reuse this layout:

- `crosshook-launch-panel__feedback-header` + badge
- `crosshook-launch-panel__feedback-title` for the message
- `crosshook-launch-panel__feedback-help` for the path/detail
- `crosshook-launch-panel__feedback-actions` for CTAs

Prefer **one primary CTA per issue** (most likely fix) + **one secondary** (less common alternative). More than two CTAs per issue creates decision paralysis.

### Batch Error Summary vs Individual Detail

Use the **two-tier approach**:

- **Batch summary**: "X of N profiles have issues" with breakdown (e.g. "2 broken, 1 stale"). This uses a `crosshook-status-chip` counter pattern already seen in `CompatibilityViewer`.
- **Individual detail**: revealed on profile selection — do not show all broken profiles expanded simultaneously (too much noise, especially on 1280×800).

**Never show all errors at once in a modal.** Heroic Games Launcher's experience shows users get overwhelmed by long validation error lists. Progressive reveal wins.

---

## Performance UX

### Loading Indicators During Batch Validation

**Pattern**: Show a spinner/pulse on each profile badge while it is being validated. Do not block the entire UI.

Implementation path for CrossHook:

- Individual profile cards show `[Checking…]` spinner badge while their validation is pending.
- Profiles that have already been validated show their final badge.
- A global progress indicator (e.g. subtle progress bar in the section header meta slot of `CollapsibleSection`) shows "Checking 4 of 8…".

From [Carbon Loading Pattern](https://carbondesignsystem.com/patterns/loading-pattern/): the inline spinner is preferred for items in a list; a skeleton placeholder works for initial load; a full-screen overlay is reserved for blocking operations only.

### Batch Results Display

**api-researcher confirms**: validation is fast (<50ms typical), so per-profile streaming is not needed. The backend fires a single `profile-health-batch-complete` event after all profiles are checked. The UI should:

1. Show all profiles immediately with a spinner/`unchecked` badge (no empty list).
2. Render a "Scanning profiles…" caption in the section header meta slot while the `invoke` is pending.
3. When `profile-health-batch-complete` fires, update all badges atomically — the transition from spinner to final badge is the perceived "result".

For >50 profiles where batch time is measurable, a progress counter (`health-check-progress` events: `{ current, total, profile_name }`) can be shown in the header — but individual badge updates still happen atomically at the end, not per-profile. This avoids partial-state visual noise.

### Background Validation with Notification

**Recommended startup sequence**:

1. App starts → profiles list loads immediately from disk (no blocking).
2. Background validation fires as a Tauri command (non-blocking, results streamed back).
3. If zero profiles fail: no UI change (silent success).
4. If ≥1 profile fails: a dismissible banner appears (similar to the existing `crosshook-rename-toast` pattern with `role="status"` and `aria-live="polite"`).

**`checked_at` display**: Show "Checked just now" or relative time after a manual recheck, using the `checked_at` ISO timestamp from `ProfileHealthResult`. This is cosmetic feedback only — health status is not persisted, so there is no cross-session "last checked" display. Do not adapt `isStale()` or `generatedTimeLabel` from LaunchPanel for this purpose.

### Optimistic UI Patterns

When a user manually triggers "Recheck" on a single profile:

1. Immediately set badge to spinner.
2. On result: animate badge to new state (healthy → add check animation, broken → no animation).
3. Do NOT reset badge to "unchecked" between checks — users trust "was healthy 3 minutes ago" more than "unknown".

---

## Competitive Analysis

### Steam Library: Verify Integrity of Game Files

- **Pattern**: Per-game, found via right-click → Properties → Installed Files → "Verify integrity of game files"
- **Strengths**: Simple progress dialog, clear "N files failed to validate and will be reacquired" result
- **Weaknesses**: No batch validation across library; no inline status badges on game library cards; hover/right-click only (not gamepad-accessible from main library view)
- **Relevance**: CrossHook should learn from the "just fix it automatically" ethos — where possible, auto-repair (e.g. re-detect Proton version) rather than surfacing an error for the user to manually resolve

### Lutris: Runner/Wine Prefix Checks

- **Pattern**: Games list shows an icon/tag for broken runners. Side panel shows error on selection.
- **Strengths**: Inline status visible in the list; side panel shows specific error message
- **Weaknesses**: Error messages are often technical (`wine: error accessing...`); no remediation suggestions; no batch health check
- **Relevance**: CrossHook must go further — user-readable messages + actionable fix buttons, not just "this failed"

### Heroic Games Launcher

- **Pattern**: Game cards show "Not Installed" or "Update Available" status badges. Repair option in context menu.
- **Strengths**: Status badges are always visible; clear visual hierarchy
- **Weaknesses**: Context menus are not gamepad-navigable without specific controller support; no proactive health dashboard
- **Relevance**: The always-visible badge-on-card approach directly validates CrossHook's inline badge recommendation

### Grafana / Datadog (Monitoring Dashboards)

- **Pattern**: Traffic light icons (red/amber/green), metric cards, sparklines, drilldown on click
- **Strengths**: High information density; clear severity hierarchy; expandable rows for details
- **Weaknesses**: Designed for mouse/keyboard, not gamepad; often too dense for 1280×800
- **Relevance**: The two-tier summary → detail pattern translates well. The "all green dashboard" concept (all profiles healthy) is a satisfying end state CrossHook should optimise for

### Summary Matrix

| Tool                 | Inline list badges | Batch health check | Gamepad-friendly | Remediation CTAs  | Progressive results |
| -------------------- | ------------------ | ------------------ | ---------------- | ----------------- | ------------------- |
| Steam                | No                 | No                 | No               | No (auto-repairs) | No (blocks UI)      |
| Lutris               | Partial            | No                 | No               | No                | N/A                 |
| Heroic               | Yes                | No                 | Partial          | Partial           | N/A                 |
| Grafana              | Yes                | Yes                | No               | No                | Yes                 |
| **CrossHook target** | **Yes**            | **Yes**            | **Yes**          | **Yes**           | **Yes**             |

---

## Recommendations

### Must Have

0. **4-state health model** — expose `missing`, `inaccessible`, `not_configured`, and `stale` as distinct states with distinct remediation copy. Do not collapse `missing` and `inaccessible` into a single "broken" badge — they require different user actions.

1. **Inline health badge on every profile row** — reuse `crosshook-compatibility-badge` semantic with health-mapped CSS suffixes. Badge must be visible without expanding or hovering, and must be focusable/readable when profile is selected via gamepad. Integrate into `ProfilesPage` inline (Option A), not a new tab.

2. **Startup background validation** — run on app launch, stream results; no blocking. Show notification banner only if ≥1 profile is broken.

3. **Per-issue remediation CTA** — every broken-path issue must have at least one actionable button (Browse / Auto-detect / Remove). Message must state what is wrong + what to do, never just the path string alone.

4. **Manual "Recheck" button** per profile and a "Recheck All" in the section header — always accessible without hover, minimum 48px height (`--crosshook-touch-target-min`). Progress reported via `health-check-progress` Tauri events (confirmed by tech-designer).

4a. **Community-import context note** — when a profile has a `community_tap_url` and shows ≥2 `missing` issues, prepend the issue list with: _"This profile was imported — paths may need to be updated for your system."_

5. **Gamepad-accessible health detail** — health detail section opens inline (CollapsibleSection) or via existing modal pattern. Fix buttons must be reachable with D-pad + A confirm, no hover required.

### Should Have

6. ~~**Staleness display**~~ — **Removed.** Health status is not persisted to disk (security-researcher constraint). Time-based staleness ("last checked X days ago") is undefined without persistence. `checked_at` is display-only post-recheck feedback. Stale status is binary (path exists or not), not age-based.

7. **Batch summary** — `CollapsibleSection` header meta shows "N issues" count; auto-opens if broken profiles exist.

8. **Progressive badge updates** — as batch validation streams results, update individual badges in-place (spinner → healthy/broken), not all at once at the end.

9. **Controller hint overlay updates** — `ControllerPrompts` should contextually show "Y Re-check" / "X Fix" when a broken profile is focused.

10. **Silent success** — do not notify the user when all profiles are healthy. Only surface the banner when action is needed.

### Nice to Have

11. **"Fix All" quick action** — for issues that CrossHook can auto-resolve (re-detecting Steam paths, updating Proton version to currently installed), offer a single "Auto-fix N issues" button that runs repair logic in batch.

12. **Health history** — store the last N validation timestamps per profile; surface a simple "validated 3 times, healthy" or "first broken at 2026-03-20" in the detail view.

13. **Optimistic "last known healthy" badge** — instead of reverting to "unchecked" between rechecks, keep the previous status visible with a "checking..." overlay until new result arrives.

---

## Open Questions

0. ~~**Auto-revalidate on profile save?**~~ — **Closed.** Confirmed by business-analyzer: auto-revalidate on save is the business rule. After `invoke('save_profile')` resolves → call `invoke('validate_profile', { name })` (single-profile) → update that profile's badge in-place. Navigate-away edge case: hold last-known state, update silently when result arrives. "Recheck All" remains for post-external-change scenarios. Requires new Tauri command: `validate_profile(name: String) -> ProfileHealthResult`.

1. **Validation depth**: Resolved by business-analyzer — shallow only for Phase 1: existence + type + permissions (`Missing` ENOENT / `Inaccessible` EACCES). No Proton prefix structure check, no Steam AppID network resolution.

2. ~~**Stale threshold configurability**~~ — **Closed.** Stale is binary (path exists or not), not time-based. No threshold to configure.

3. **Auto-repair scope**: Which issues should CrossHook attempt to auto-fix without user confirmation? Suggesting this scope to the business-analyzer and tech-designer for business rule definition.

4. **Path privacy in error messages**: Broken paths may reveal personal home directory structure. Should the UI truncate paths (e.g. `~/games/...`) or always show full absolute paths for debugging clarity? (Note: flagged separately to security-researcher.)

5. **Notification persistence**: Should the startup notification persist across app restarts until the user resolves the issues, or only appear once per session?

6. **Profile card vs. selector UX**: The current `ProfilesPage` uses a `<select>` dropdown for profile selection. A health dashboard implies a **list/card view** where badges are visible. Should the health dashboard introduce a new card-list presentation of profiles, or augment the existing selector with badge indicators? (Flagged for tech-designer on API implications.)

---

## Sources

- [Carbon Design System — Status Indicator Pattern](https://carbondesignsystem.com/patterns/status-indicator-pattern/)
- [Carbon Design System — Loading Pattern](https://carbondesignsystem.com/patterns/loading-pattern/)
- [Pencil & Paper — Dashboard UX Patterns](https://www.pencilandpaper.io/articles/ux-pattern-analysis-data-dashboards)
- [Pencil & Paper — Error UX Patterns](https://www.pencilandpaper.io/articles/ux-pattern-analysis-error-feedback)
- [Pencil & Paper — Loading UX Patterns](https://www.pencilandpaper.io/articles/ux-pattern-analysis-loading-feedback)
- [UX Patterns for Devs — Progressive Loading](https://uxpatterns.dev/glossary/progressive-loading)
- [Heroic Games Launcher](https://heroicgameslauncher.com/)
- [Heroic Games Launcher — Troubleshooting Wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Troubleshooting)
- [Steam Support — Verify Integrity of Game Files](https://help.steampowered.com/en/faqs/view/0C48-FCBD-DA71-93EB)
- [MDN — :focus-visible](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Selectors/:focus-visible)
- [Sara Soueidan — Designing Accessible Focus Indicators](https://www.sarasoueidan.com/blog/focus-indicators/)
- [Medium — Error Handling UX Design Patterns](https://medium.com/design-bootcamp/error-handling-ux-design-patterns-c2a5bbae5f8d)
- [DesignRush — Dashboard Design Principles 2026](https://www.designrush.com/agency/ui-ux-design/dashboard/trends/dashboard-design-principles)
- [Unreal Engine — Gamepad UI Navigation](https://medium.com/@Jamesroha/dev-guide-gamepad-ui-navigation-in-unreal-engine-5-with-enhanced-input-3ab5403f8ab5)

---

## Codebase Observations

The following existing CrossHook patterns are directly reusable or directly applicable:

| Existing pattern                                                 | File                             | Health dashboard application                                                                                                                                                                                                                                                       |
| ---------------------------------------------------------------- | -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CompatibilityBadge` + `crosshook-compatibility-badge--{rating}` | `CompatibilityViewer.tsx`        | Reuse for health badge (healthy/stale/broken/unchecked mapping to working/partial/broken/unknown)                                                                                                                                                                                  |
| `severityIcon()` + `data-severity` attributes                    | `LaunchPanel.tsx`                | Reuse severity icon and data-attribute pattern for health issues                                                                                                                                                                                                                   |
| `isStale()` timestamp check                                      | `LaunchPanel.tsx:119`            | **Do not reuse** — checks preview staleness at 60s threshold, wrong for profile health (needs ~7d). Write a new inline staleness check in the health hook with a named constant. The `is_stale: boolean` on `LauncherInfo` (`types/launcher.ts:8`) is also distinct and unrelated. |
| `CollapsibleSection`                                             | `ui/CollapsibleSection.tsx`      | Per-profile health detail + batch summary panels                                                                                                                                                                                                                                   |
| `crosshook-launch-panel__feedback-*` classes                     | `LaunchPanel.tsx`                | Remediation message + CTA layout pattern. Render inline in health detail — do not prematurely extract `ValidationIssueItem` until 3+ call sites.                                                                                                                                   |
| `severityIcon()`                                                 | `LaunchPanel.tsx`                | Extract to `src/utils/severity.ts` — pure deduplication (~5 lines), justified when health dashboard also needs it.                                                                                                                                                                 |
| `crosshook-rename-toast` + `role="status"` + `aria-live`         | `ProfilesPage.tsx`               | Startup notification banner pattern                                                                                                                                                                                                                                                |
| Modal focus-trap pattern                                         | `LaunchPanel.tsx` (PreviewModal) | If fix flow requires file-picker or complex multi-step repair                                                                                                                                                                                                                      |
| `useGamepadNav` two-zone model                                   | `hooks/useGamepadNav.ts`         | Health dashboard content zone; profile cards as focusable units                                                                                                                                                                                                                    |
| `data-crosshook-focus-root="modal"` attribute                    | `useGamepadNav.ts`               | For any health repair modal                                                                                                                                                                                                                                                        |
| `--crosshook-touch-target-min: 48px`                             | `variables.css`                  | All Recheck and Fix buttons must meet this minimum                                                                                                                                                                                                                                 |
| `--crosshook-color-success/warning/danger`                       | `variables.css`                  | Badge and icon color tokens already defined                                                                                                                                                                                                                                        |
