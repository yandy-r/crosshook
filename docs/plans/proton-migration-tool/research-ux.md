# UX Research: Proton Migration Tool

**Feature**: Proton version migration tool for profile path updates (GitHub issue #48)
**Date**: 2026-03-29
**Author**: research-specialist (UX Research)
**Revision**: 2 — updated with teammate findings from tech-designer, business-analyzer,
security-researcher, and api-researcher.

---

## Executive Summary

When users upgrade GE-Proton (e.g., from GE-Proton 9-4 to 9-7), profiles referencing the old
Proton path silently break. The current error "The Steam Proton path does not exist" provides no
actionable recovery path. This research documents optimal user workflows, UI/UX patterns, error
handling strategies, performance UX requirements, and competitive analysis to inform a Proton
migration tool.

**Key findings:**

- **Every competing launcher fails silently** (Lutris, Heroic, Bottles, ProtonUp-Qt). Users
  discover broken paths only when a game fails to launch. CrossHook has a genuine first-mover
  opportunity with proactive detection and batch auto-suggest.
- The Health Dashboard's existing `missing_proton` issue category is the natural integration
  point — surface a contextual "Fix Proton Paths" action there rather than adding a new page.
- The backend enforces a `dry_run`/`confirm` two-phase split; the UX must surface this clearly
  with an explicit confirmation before any writes occur.
- Three confidence levels for suggestions (`SameFamilyNewer`, `SameFamilyOlder`,
  `DifferentFamilyFallback`) require distinct visual treatment; `DifferentFamilyFallback` must
  be opt-in and excluded from the default batch selection.
- Cross-major-version suggestions (e.g., GE-Proton 9→10) require an explicit compatibility
  warning (Steam prefix compatibility risk).
- Two Proton path fields exist: `steam.proton_path` and `runtime.proton_path`; the UI must
  identify which field is stale per profile.
- TOCTOU edge case: if a replacement Proton version is uninstalled between dry-run and confirm,
  the apply must fail gracefully with a re-scan prompt.

**Confidence**: High — based on direct codebase inspection, competitive source research,
industry UX standards (NN/G, WCAG), and cross-team findings from tech-designer,
business-analyzer, security-researcher, and api-researcher.

---

## User Workflows

### 1.1 User Contexts

| Context                  | Primary concern                                  | UX implication                                                                               |
| ------------------------ | ------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| **Steam Deck (gamepad)** | Single-button fixes, no keyboard                 | Batch "Fix All" CTA must be reachable via D-pad; no text input for paths                     |
| **Linux desktop**        | Verify before committing, want to see full paths | Show expandable full paths; show before/after comparison                                     |
| **Both**                 | GE-Proton updates frequently (every few weeks)   | Proactive detection; Health Dashboard should show count without waiting for a launch failure |

### 1.2 Three Entry Points

| Entry Point                            | Context                                  | Flow                                                               |
| -------------------------------------- | ---------------------------------------- | ------------------------------------------------------------------ |
| **Health Dashboard — batch toolbar**   | Multiple profiles affected               | "Fix Proton Paths" button in toolbar; opens Migration Review Modal |
| **Health Dashboard — per-profile row** | Single profile; user sees it in the list | "Update Proton" action on the `missing_proton` issue row           |
| **Profile Editor — inline field**      | User is editing a profile; path is stale | Warning + suggestion below the Proton path field                   |

### 1.3 Primary Flow — Passive Discovery via Health Dashboard (Batch)

```
[Open CrossHook]
    │
    ├─ Health Dashboard loads → detects 1+ profiles with `missing_proton` issues
    │   • Summary card "Broken" count reflects affected profiles
    │   • "Missing/invalid Proton path" category row shows count
    │
    ├─ Contextual toolbar button appears:
    │   "Fix Proton Paths (X)" — only shown when missing_proton count > 0
    │
    ├─ User clicks → dry_run scan initiates
    │   • Button disabled immediately (prevent double-invoke)
    │   • Spinner + "Scanning Proton installations…" text inside modal
    │
    ├─ dry_run completes → Migration Review Modal opens
    │   • Table: checkboxes | Profile | Affected Field | Old Path | Suggested Path | Confidence
    │   • SameFamilyNewer rows: pre-checked, green indicator
    │   • SameFamilyOlder rows: pre-checked with ⚠ warning, yellow indicator
    │   • DifferentFamilyFallback rows: UNCHECKED by default, orange indicator
    │   • No-suggestion rows: excluded from checkboxes, shown separately at bottom
    │   • "1 profile needs manual attention" callout for no-suggestion rows
    │
    ├─ User reviews, adjusts selection, clicks "Update N Profiles"
    │   • confirm phase: backend writes TOML for selected profiles only
    │   • Progress: "Updating [3/5]: Dark Souls III…"
    │
    └─ Success: "5 profiles updated. 1 profile needs manual attention."
        └─ Health re-check triggers automatically (programmatic, not user-initiated)
```

**TOCTOU handling**: If a replacement Proton version is deleted between dry_run and confirm
(e.g., a concurrent ProtonUp-Qt uninstall), the backend's pre-flight check fails for that
profile. The UX shows: "GE-Proton 9-7 is no longer available. Please re-scan." with a
"Re-scan" button that restarts the dry_run phase.

### 1.4 Alternative Flow — Per-Profile Row Action (Health Dashboard)

```
Health Dashboard row for "Dark Souls III" → status: broken
    │
    ├─ Expand issues → sees "Missing/invalid Proton path: runtime.proton_path"
    ├─ "Update Proton" action button on that row
    │
    ├─ Inline suggestion appears (no modal):
    │   "Suggested: GE-Proton 9-7  [Use GE-Proton 9-7]  [Choose different…]"
    │
    └─ One click → confirm phase writes the single profile
        └─ Undo toast (5 s): "Proton path updated → Undo"
```

### 1.5 Alternative Flow — Profile Editor Inline Fix

```
[Profiles page] → Select profile with stale Proton path
    │
    ├─ Proton path field shows:
    │   • ❌ icon in field border (--crosshook-color-danger)
    │   • Below field: "GE-Proton 9-4 is no longer installed."
    │   • If suggestion exists: "GE-Proton 9-7 found  [Use GE-Proton 9-7]  [Browse…]"
    │   • If suggestion is a downgrade: ⚠ "GE-Proton 9-2 available (older version)"
    │   • If no suggestion: "[Browse for Proton…]"
    │
    └─ "Use GE-Proton 9-7" → updates the field value (profile marked dirty)
        └─ User saves normally; undo available via standard profile revert
```

### 1.6 Rollback / Undo

- **Single-profile inline fix**: Undo toast (5 s timeout). Profile is marked dirty until saved;
  user can also revert with the standard discard-changes flow.
- **Batch migration**: No in-app undo after confirmation. The backend writes TOML. Full paths
  are shown before confirmation so users can verify. Recovery: re-open each profile, pick
  previous version manually, or use the Profile Editor's path browse.

---

## 2. UI/UX Best Practices

### 2.1 Confidence Level Visual Treatment

From business-analyzer: three confidence levels must be visually distinct.

| Level                                     | Meaning                              | Default state   | Visual                                                               |
| ----------------------------------------- | ------------------------------------ | --------------- | -------------------------------------------------------------------- |
| `SameFamilyNewer` (same major)            | e.g., GE-Proton 9-4 → GE-Proton 9-7  | Pre-checked     | ✓ green badge "Upgrade"                                              |
| `SameFamilyOlder` (same major)            | e.g., GE-Proton 9-4 → GE-Proton 9-2  | Pre-checked + ⚠ | Amber badge "Older version — may cause issues"                       |
| `SameFamilyNewer` (cross-major upgrade)   | e.g., GE-Proton 9-4 → GE-Proton 10-1 | **Unchecked**   | Amber/orange badge "Major version change — prefix may need reset"    |
| `SameFamilyOlder` (cross-major downgrade) | e.g., GE-Proton 10-x → GE-Proton 9-x | **Unchecked**   | Red badge "Major version downgrade — high risk of prefix corruption" |
| `DifferentFamilyFallback`                 | GE-Proton → Proton (official)        | **Unchecked**   | Orange badge "Different family — verify compatibility"               |

**Cross-major and `DifferentFamilyFallback` rows must not appear in the default batch selection.**
They belong in a separate "Needs Manual Review" section of the review modal that the user must
explicitly expand and opt into. Heroic's v2.18.0 lesson: silently substituting a different
Proton family caused immediate backlash and required a hotfix reversal.

### 2.2 Cross-Major Version Warning

When the suggested version crosses a major version boundary (e.g., GE-Proton 9-x → GE-Proton
10-x), show an explicit inline warning **per row** in the review modal (not as a global banner).
Steam itself surfaces "Prefix has an invalid version?!" on major jumps, so users may already
be familiar with this risk.

**Graduated severity by direction:**

| Scenario                              | Warning level  | Message                                                        |
| ------------------------------------- | -------------- | -------------------------------------------------------------- |
| Same-family, older build (same major) | Amber ⚠        | "Older version — may cause issues"                             |
| Cross-major upgrade                   | Amber/orange ⚠ | "Major version change — your game prefix may need to be reset" |
| Cross-major downgrade                 | Red ⛔         | "Major version downgrade — high risk of prefix corruption"     |

**Batch flow implication**: Cross-major candidates (any direction) must not be in the default
checked set. They are grouped in a collapsed "Needs Manual Review" section below the main table.
The user must expand the section and check individual rows to include them.

**"Proton Experimental" edge case**: When the stale path points to a `Proton Experimental`
directory that no longer exists, it cannot be auto-migrated. Show a distinct message:

> "Proton Experimental has been removed or relocated. CrossHook cannot automatically migrate
> this path. Go to Steam → Settings → Compatibility to reinstall Proton Experimental."

This case should be treated as a no-suggestion row, not a migration candidate.

**Release cadence copy guidance**: GE-Proton releases approximately weekly. Frame suggestions
positively — "A newer GE-Proton is available" rather than "your Proton is broken" or "path
invalid." This aligns with business-analyzer's recommendation and reduces user anxiety.

### 2.3 Two Proton Path Fields

From tech-designer: two field names exist — `steam.proton_path` (Steam launch method profiles)
and `runtime.proton_path` (proton_run launch method profiles). The UI should label which field
is stale in the review table and in the per-row health issue expansion:

- Column: **Affected Field** → shows "Steam Proton" or "Runtime Proton"
- Health Dashboard issue row expansion: "Missing runtime Proton path" vs. "Missing Steam Proton
  path"

This prevents confusion when a user has both types of profiles in the same batch.

### 2.4 Migration Review Modal — Full Design

**Columns**: ☐ | Profile Name | Affected Field | Current Path | Suggested Path | Confidence

- **Current Path**: shown in `--crosshook-color-danger` with a ❌ prefix
- **Suggested Path**: shown in `--crosshook-color-success` with a ✓ prefix (for SameFamilyNewer)
- **Path truncation**: truncate long paths at ~60 chars with a "Show full path" expand trigger
  (uses `CollapsibleSection` pattern)
- **Select All / Deselect All**: header row toggle; note that `DifferentFamilyFallback` rows
  are excluded from "Select All" — they require individual opt-in
- **No-suggestion section**: shown below the main table with a muted separator:
  "The following profiles have no Proton installation available. Fix manually."
- **Cross-major / DifferentFamilyFallback section**: separate collapsed `CollapsibleSection`
  below the no-suggestion section — "Needs Manual Review (N)" — user must expand to access.
- **Confirm button**: "Update N Profile(s)" — N updates as checkboxes are toggled
- **Cancel**: clearly separated; ghost-style button

**Modal shell**: Use `LauncherPreviewModal` as the accessibility base — it provides a complete
portal, focus trap, Tab cycling, Escape handler, `inert` background management, `aria-modal`,
and focus restore on close. `ProfileReviewModal` is not suitable (it has a summary-item layout
with no checkboxes or table). The migration modal needs new body content but inherits this
shell verbatim. (Confirmed by practices-researcher.)

### 2.5 Escape Hatch — Choose a Different Candidate

The UX must not force users to accept the auto-suggested path. Provide a "Choose different…"
action per row that opens a dropdown or mini-picker showing all detected Proton installations,
sorted: same-family-newer first, same-family-older next, different-family last. This is the
"escape hatch" identified as important by api-researcher.

### 2.6 Gamepad / Steam Deck Navigation

CrossHook's existing infrastructure handles this. New components must follow:

- `crosshook-focus-ring`, `crosshook-focus-target`, `crosshook-nav-target` CSS classes on all
  interactive elements
- `min-height: var(--crosshook-touch-target-min)` (48px desktop / 56px controller mode) on
  buttons and checkboxes
- Tab order in Migration Review Modal: Select All toggle → row checkboxes (top to bottom) →
  Cancel → Update N Profiles
- Controller prompts bar should show relevant button mappings when modal is open (e.g.,
  "A: Toggle B: Cancel Start: Confirm")
- `useGamepadNav` hook to be applied to the modal's focus scope

**"Fix Proton Paths" button placement**: `TableToolbar` in `HealthDashboardPage.tsx` is a
file-local component (not exported). Add the conditional button directly inside that component
in `HealthDashboardPage.tsx` — do not extract it to a shared component for this use. (Correction
from practices-researcher; the component is at `HealthDashboardPage.tsx:110-183`.)

### 2.7 Security-Mandated Confirmation Requirements

From security-researcher: the UX **must** show full paths (not truncated by default) before
the user clicks the confirm button, because users need full context to make an informed decision.
Truncation is acceptable as a default _display_ mode, but "Show full path" expansion must be
accessible without any extra navigation. The "Update N Profiles" button must be a deliberate
action — no auto-confirm, no timer-based dismiss.

### 2.8 Existing Component Reuse Summary

From practices-researcher verification (all confirmed against source):

| Component / Pattern                                                                             | Status                     | Notes                                                                                                                                                                           |
| ----------------------------------------------------------------------------------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `LauncherPreviewModal`                                                                          | ✓ Use as modal shell       | Complete a11y shell: focus trap, Escape, `aria-modal`, portal, focus restore                                                                                                    |
| `CollapsibleSection`                                                                            | ✓ Directly reusable        | Exported; has `meta` slot; supports controlled `open`/`onToggle`                                                                                                                |
| `HealthBadge`                                                                                   | ✓ Directly reusable        | Exported; supports `onClick` interactive mode for status column                                                                                                                 |
| `TableToolbar` in `HealthDashboardPage.tsx`                                                     | ⚠ File-local only          | Not exported; add "Fix Proton Paths" button directly inside it at line 110–183                                                                                                  |
| Linear progress bar                                                                             | ✗ Does not exist           | Use native `<progress>` element with inline CSS                                                                                                                                 |
| `categorizeIssue()` in `HealthDashboardPage.tsx`                                                | ℹ Potential extraction     | Maps `field === 'steam.proton_path'` → `'missing_proton'` at lines 39–48; could be extracted to a shared util if migration detection needs to reference the same categorization |
| `ProfileReviewModal`                                                                            | ✗ Wrong base for migration | Summary-item layout only; no checkboxes or table                                                                                                                                |
| CSS variables `--crosshook-color-warning`, `--crosshook-color-danger`                           | ✓ Confirmed                | Use for inline field warning and broken path column respectively                                                                                                                |
| Focus CSS classes (`.crosshook-focus-target`, `.crosshook-nav-target`, `.crosshook-focus-ring`) | ✓ Confirmed                | Apply to all interactive elements in migration modal                                                                                                                            |

### 2.9 Language and Tone

From error handling UX research (NN/G):

- Use **neutral, empathetic language** — this is an environmental error, not a user mistake.
- **Recovery-focused structure**: What happened → Why it matters → How to fix.

| Instead of                             | Use                                                                          |
| -------------------------------------- | ---------------------------------------------------------------------------- |
| "Error: Proton path invalid"           | "GE-Proton 9-4 is no longer installed"                                       |
| "The Steam Proton path does not exist" | "This Proton version was removed or renamed. A migration tool is available." |
| "Apply" / "OK"                         | "Update 4 Profiles"                                                          |
| "Warning: path issue"                  | "GE-Proton 9-4 not found — suggested: GE-Proton 9-7"                         |

The current error message ("The Steam Proton path does not exist") must be updated to mention
the migration tool even when the user is not on the Health Dashboard — this was the original
user complaint in issue #48.

---

## 3. Error Handling UX

### 3.1 Error States Table

| State                                            | UI Pattern                                     | Message                                                                                            |
| ------------------------------------------------ | ---------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| Single broken path, same-family-newer suggestion | Inline suggestion below field / per-row action | "GE-Proton 9-4 not found. GE-Proton 9-7 is available. [Use GE-Proton 9-7]"                         |
| Single broken path, older suggestion             | Inline suggestion + ⚠ downgrade warning        | "GE-Proton 9-4 not found. Only GE-Proton 9-2 is available. ⚠ Older version."                       |
| Single broken path, different-family only        | Inline with explicit opt-in                    | "GE-Proton 9-4 not found. Official Proton 9.0 is available (different family). [Use Proton 9.0 ↗]" |
| Single broken path, no suggestion                | Inline with browse + install guidance          | "GE-Proton 9-4 not found. No Proton installations detected. [Browse…] [Install Proton →]"          |
| Batch migration, partial no-suggestion           | Separate section in review table               | "1 profile needs manual attention" callout; these rows have no checkbox                            |
| Batch migration, partial apply failure           | Post-migration result modal                    | "3 profiles updated. 1 failed: Dark Souls III — no writable path. [Retry] [View Profile]"          |
| TOCTOU: suggested version uninstalled mid-flow   | Replace modal with error + re-scan CTA         | "GE-Proton 9-7 is no longer available. Please re-scan. [Re-scan]"                                  |
| Proton discovery scan fails                      | Error banner inside modal                      | "Could not scan Proton installations — check Steam library settings. [Open Settings]"              |
| Scan finds no stale paths                        | Toast                                          | "All Proton paths are valid." (no action needed)                                                   |
| Cross-major-version suggestion                   | Per-row inline warning in review table         | "⚠ Major version change: prefix may need recreation."                                              |

### 3.2 Validation Timing

- Validate Proton path on **profile load** and on **health re-check** — not on keystroke
  (paths are picked via file picker, not typed manually).
- Batch scan: **on-demand only** (toolbar button click). Do not auto-scan on app open —
  avoids surprising the user with a modal at startup.
- After migration: **auto-trigger re-check** programmatically (no user prompt required).

### 3.3 Existing Error Message Update

The current error "The Steam Proton path does not exist" (shown in the Profile Editor validation)
should be updated to include a recovery hint even outside the Health Dashboard context:

> "GE-Proton 9-4 is no longer installed. Go to the Health Dashboard to scan for a replacement,
> or browse for a Proton installation manually."

---

## Performance UX

### 4.1 Loading States During Proton Discovery (dry_run Phase)

From tech-designer: `discover_compat_tools()` is synchronous/fast (directory listing). However,
it blocks the thread. The UX should treat it as potentially slow (HDD / NFS mounts) and show
loading state regardless.

**Recommendations:**

- Modal opens immediately (empty state) with a spinner and "Scanning Proton installations…"
- If scan completes in <500ms: no timeout text needed; modal populates directly
- If scan takes >3s: add "This is taking longer than usual…"
- Trigger button disabled during scan (visual: opacity 0.5, aria-disabled=true)

### 4.2 dry_run / confirm Two-Phase UX Mapping

From security-researcher: backend has explicit `dry_run`/`confirm` parameter split.

| Phase        | UX action                       | Loading state                                     |
| ------------ | ------------------------------- | ------------------------------------------------- |
| dry_run      | User clicks "Fix Proton Paths"  | Spinner in modal: "Scanning…"                     |
| User reviews | Modal displays review table     | No loading; user interacts                        |
| confirm      | User clicks "Update N Profiles" | Progress bar: "Updating [N/total]: Profile name…" |
| Post-confirm | Results available               | Modal shows summary: "N updated, M failed"        |

The confirm phase should never happen automatically — the confirm button click is the deliberate
trigger.

### 4.3 Progress Indicators for Batch Operations

For ≥3 profiles: inline linear progress bar in the confirm phase of the modal.
For 1–2 profiles: spinner only (bar is visual overkill at small counts).

**Implementation note**: No progress bar component exists in the codebase (confirmed by
practices-researcher). Use a native `<progress>` element with inline CSS — do not introduce
a component library for a single use. The `<progress max={total} value={done}>` element
is accessible and renderable with `--crosshook-color-accent` styling.

Progress bar pattern:

```
Updating profiles…  [=========>  ]  4 / 6
Updating: Dark Souls III…
```

After completion: replace bar with ✓ checkmark + summary. Do not auto-close — let users read
the result and dismiss explicitly.

### 4.4 Health Dashboard Re-check After Migration

After successful migration (confirm phase complete), programmatically trigger the existing
`build_enriched_health_summary()` flow or emit an equivalent event. This means:

- `missing_proton` count in the Health Dashboard summary cards drops immediately
- The "Fix Proton Paths" toolbar button disappears if count reaches zero
- No separate "refresh" prompt needed — the feedback loop is invisible to the user

Do NOT use optimistic updates — TOML writes must be confirmed before the UI reflects success.

---

## 5. Competitive Analysis

### 5.1 Key Finding: All Launchers Fail Silently

From api-researcher's research: **every competing launcher leaves users to fix broken Proton
paths manually, and none offer proactive detection or suggestions.**

The typical user journey across all competitors:

1. Launch game → game fails to open or shows cryptic error
2. Check logs (if the user knows how)
3. Google the error
4. Manually navigate to launcher settings
5. Re-select a runner from a dropdown (if the user knows which one to pick)

This is a significant friction point, especially on Steam Deck where users don't have desktop
mode readily available. **CrossHook's proposed feature is genuinely novel in this ecosystem.**

---

### 5.2 Lutris

**Version management**: Per-game Wine runner set in Configure > Runner options. Global runner
management via "Manage versions" side panel.

**Silent failure pattern**: Validates runner path on launch. If runner is missing, shows
`MissingExecutableError`. Has a silent fallback to "default Wine version" — changes the runner
without informing the user why. No "your runner moved, here's the nearest replacement" prompt.

**Batch update**: None. Users change each game manually.

**Notable anti-pattern**: Changing the runner for one game sometimes changes all games (GitHub
issue #4370) — a cascading silent change that confuses users.

**What to adopt**: Runner version list with install/remove buttons.
**What to avoid**: Silent runner substitution; per-game-only config with no batch path; deep
sub-menu navigation to find runner version setting.

**Confidence**: High — based on GitHub issues and forum discussions.

---

### 5.3 Bottles

**Version management**: Runner selection in Bottle Preferences. Can switch Wine → Proton "on
the fly." Stable vs. pre-release toggle.

**Silent failure pattern**: If runner is removed, launch fails with "Invalid Steam Proton path"
or similar generic error. User must open bottle preferences, re-select runner, save. No
before/after comparison, no batch migration.

**What to adopt**: On-the-fly runner switching without recreating a configuration; stable /
pre-release tier label for version names.
**What to avoid**: The Bottles model abstracts away from file paths, which doesn't map to
CrossHook's path-based model.

**Confidence**: High — based on official Bottles documentation and GitHub issues.

---

### 5.4 Heroic Games Launcher

**Version management**: Dropdown per game. Global default in settings.

**Silent failure pattern**: If stored path is invalid, game fails to launch with "could not
find the selected wine version." User manually opens game settings and re-selects. GitHub
issue #2900: after a wine-GE version update, games silently failed to launch; discovery was
only through failed launches. Issue #4026: "Wine not found!" — no suggestion offered.

**v2.18.0 incident (2025)**: Hid non-GE Proton versions by default → users couldn't select
desired versions → immediate hotfix (v2.18.1) reversed it to opt-in.
**Lesson: never restrict or substitute user version choices without opt-in.**

**Steam Deck (Issue #3771)**: Community requested integrated Proton management "easier to
navigate with Steam Deck controls than ProtonUp-Qt." Heroic addressed this through Wine manager
keyboard navigation improvements.

**What to adopt**: Per-profile version dropdown; opt-in (not opt-out) filtering; keyboard
navigation in version manager.
**What to avoid**: Silently hiding version options; changes that require Desktop Mode on Steam
Deck; any default-batch inclusion of different-family fallbacks.

**Confidence**: High — based on GitHub issues, GHacks article, and GamingOnLinux coverage.

---

### 5.5 ProtonUp-Qt

**Version management**: Dedicated manager for GE-Proton/Wine-GE/Luxtorpeda. Simple list:
installed versions + install new. Clicking version shows which games use it (Issue #18).

**Gap**: ProtonUp-Qt **only manages installing new versions**. After installing GE-Proton 10-34,
old profiles in other launchers still point to GE-Proton 10-28 — no notification, no profile
update. Users must manually update each launcher profile.

**Steam Deck theme (v2.12.0)**: Larger buttons, darker scheme. Trade-off: more scrolling.

**What to adopt**: "Show games/profiles using this version" per-row expand (maps to CrossHook's
"show affected profiles" in migration modal). Larger button targets.
**What to avoid**: Requiring users to leave CrossHook for a separate tool; the install-only
scope that leaves profile references stale.

**Confidence**: Medium — UI documentation sparse; based on GamingOnLinux and SteamDeckHQ.

---

### 5.6 Cross-Domain Patterns Worth Adopting

**VS Code npm-outdated extension**:

- Inline diagnostic at the outdated entry; code action for one-click upgrade or batch update all
- **Adopt for**: inline Proton path suggestion on the Profile page

**IntelliJ IDEA dependency manager**:

- Hover tooltip showing newer version; Alt+Enter context action; batch "update all" button
- **Adopt for**: Health Dashboard contextual action + profile inline suggestion

**SOLIDWORKS Auto-Repair** (broken assembly references):

- System finds match by proximity/pattern; folder-level batch repair
- **Adopt for**: automatic best-match Proton suggestion based on version string parsing

**NN/G Confirmation Dialog Research**:

- Reserve for consequential, rare operations — batch path migration qualifies
- Show specific items by name/count; descriptive button labels; avoid dialog fatigue

---

## 6. Recommendations

### Must Have

| #   | Recommendation                                                                                                                                                   | Rationale                                                      |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| M1  | Inline `⚠` warning + auto-suggestion below Proton path field on Profile page                                                                                     | Zero-friction fix for single-profile case; follows IDE pattern |
| M2  | Update existing error message to mention migration tool ("...Go to Health Dashboard to scan for a replacement")                                                  | Addresses the original issue #48 complaint directly            |
| M3  | "Fix Proton Paths (N)" contextual toolbar button on Health Dashboard (only when `missing_proton` > 0)                                                            | Integrates with existing health workflow                       |
| M4  | Migration Review Modal with: before/after table, confidence-level indicators, per-row checkboxes, ≥1 "Show full path" expand, "Update N Profiles" confirm button | Required for batch; NN/G confirmation pattern                  |
| M5  | `DifferentFamilyFallback` suggestions excluded from default batch selection; require per-row opt-in                                                              | Heroic v2.18.0 lesson — never silently change Proton family    |
| M6  | Cross-major-version inline warning per row in review modal                                                                                                       | Steam prefix compatibility risk disclosure                     |
| M7  | TOCTOU graceful failure: if suggested version uninstalled between dry_run and confirm, show "re-scan" error instead of writing a broken path                     | Security requirement from security-researcher                  |
| M8  | Auto-trigger health re-check after successful migration (invisible to user)                                                                                      | Immediate feedback that the fix worked                         |
| M9  | Show which field is stale (`steam.proton_path` vs. `runtime.proton_path`) per row                                                                                | Technical requirement from tech-designer                       |

### Should Have

| #   | Recommendation                                                                                | Rationale                                               |
| --- | --------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| S1  | Progress bar + profile name display during batch confirm phase (≥3 profiles)                  | Transparency for batch operations                       |
| S2  | "Choose different…" per-row escape hatch showing all detected Proton installations            | Users must not be locked to the auto-suggestion         |
| S3  | Undo toast (5 s) for single-profile inline auto-fix                                           | Low-cost safety net; avoids modal for trivial case      |
| S4  | Post-migration result summary: "N updated, M failed" with per-failed-profile detail           | Partial failure transparency                            |
| S5  | "Scan for Stale Proton Paths" button in Settings page                                         | Proactive discovery for power users                     |
| S6  | Group batch table by suggested replacement path (e.g., all profiles → GE-Proton 9-7 together) | Easier to confirm when many profiles share the same fix |

### Nice to Have

| #   | Recommendation                                                                                   | Rationale                                                                            |
| --- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| N1  | Per-profile "Update Proton" action on Health Dashboard issue row expansion                       | Finer-grained single-profile fix without opening Profile Editor                      |
| N2  | "Show profiles using this Proton version" on installed version list (if Proton manager is added) | Mirrors ProtonUp-Qt feature; helps users understand impact of uninstalling a version |
| N3  | Passive background scan on app open (no popup; badge update only)                                | Proactive health without interruption                                                |
| N4  | Migration history log (profile name, old path, new path, date) in SQLite                         | Useful for debugging; existing `launch_history.rs` pattern could support it          |

---

## 7. Open Questions

1. **Version matching algorithm**: Prefer closest minor (GE-Proton 9-5 for GE-Proton 9-4) vs.
   newest available? UX favors "closest compatible" for `SameFamilyNewer`, but this is a
   business/technical decision that affects the confidence label shown.

2. **Proton discovery scope**: Steam-managed Proton only, or also custom user-installed paths
   in arbitrary directories? More scope = more suggestions but more scan time.

3. **Transient vs. persistent stale warning**: Should the `missing_proton` warning on the
   Profile page persist as a banner, or only on validation? Persistent could be noise for users
   who temporarily uninstalled a version.

4. **Migration history persistence**: Should path migrations be logged to SQLite? Adds scope
   but enables "what changed and when" debugging. The existing `launch_history.rs` pattern
   could support it.

5. **`DifferentFamilyFallback` discovery**: Should the UI show official Proton versions as
   a fallback at all by default, or only when the user explicitly requests "show all available"?
   Hiding them by default avoids confusion; showing them ensures the user knows options exist.

6. **Install Proton guidance**: When no Proton version is found at all, should CrossHook link
   to or reference ProtonUp-Qt? Or is this out of scope?

---

## Sources

- [Lutris Forum: How to update Wine versions](https://forums.lutris.net/t/how-to-update-wine-versions/18153)
- [Lutris Issue #4337: Better handling of default Wine version](https://github.com/lutris/lutris/issues/4337)
- [Lutris Issue: Wine version change on one game changes all games](https://forums.lutris.net/t/wine-version-the-runner-change-on-one-game-causes-the-wine-runner-to-change-on-all-games-a-bug-or-a-feature/13567)
- [Bottles Docs: Runners](https://docs.usebottles.com/components/runners)
- [Heroic Games Launcher reverts non-GE Proton hide change](https://www.ghacks.net/2025/08/05/heroic-games-launcher-reverts-a-change-that-hid-non-proton-ge-versions-by-default/)
- [Heroic Issue #3771: Revise the Proton manager GUI](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/3771)
- [ProtonUp-Qt GitHub repository](https://github.com/DavidoTek/ProtonUp-Qt)
- [ProtonUp-Qt Steam Deck theme (v2.12.0)](https://steamdeckhq.com/news/protonup-qt-gets-new-steam-deck-theme/)
- [NN/G: Confirmation Dialogs Can Prevent User Errors](https://www.nngroup.com/articles/confirmation-dialog/)
- [Error Handling UX Design Patterns — Design Bootcamp / Medium](https://medium.com/design-bootcamp/error-handling-ux-design-patterns-c2a5bbae5f8d)
- [VS Code npm-outdated extension](https://github.com/mskelton/vscode-npm-outdated)
- [SOLIDWORKS Auto-Repair for Missing Mate References](https://help.solidworks.com/2024/English/WhatsNew/c_wn2024_assemblies_auto_repair_mates.htm)
- [IntelliJ IDEA — Managing Dependencies](https://foojay.io/today/managing-dependencies-in-intellij-idea/)
- [Accessible touch target sizes — LogRocket](https://blog.logrocket.com/ux-design/all-accessible-touch-target-sizes/)
- CrossHook codebase: `src/crosshook-native/src/styles/variables.css` — controller mode CSS vars
- CrossHook codebase: `src/crosshook-native/src/styles/focus.css` — focus ring and gamepad styles
- CrossHook codebase: `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx` — existing health patterns
- Team findings: tech-designer (Tauri IPC patterns, two path fields, dry_run/confirm split)
- Team findings: business-analyzer (confidence levels, gamepad contexts, entry points)
- Team findings: security-researcher (confirmation flow requirements, TOCTOU edge case)
- Team findings: api-researcher (competitive launcher silent-failure analysis)
