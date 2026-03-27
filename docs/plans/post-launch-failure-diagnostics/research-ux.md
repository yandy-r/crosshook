# Post-Launch Failure Diagnostics — UX Research

**Issue**: #36
**Generated**: 2026-03-27

---

## Executive Summary

All major competing Linux game launchers (Lutris, Bottles, Heroic) share the same anti-pattern: silent "stuck launching" states where failures produce no in-app feedback and logs require multi-step navigation to access. CrossHook has an opportunity to deliver genuinely novel UX by surfacing structured failure diagnostics inline in the existing `LaunchPanel` / `ConsoleView` area immediately after process exit. The existing `ValidationSeverity` badge + message + help rendering pattern from #39 is the right foundation — extend it rather than invent new patterns. The three key UX pillars are: **proactive** (show without user navigation), **progressive** (summary first, raw logs on demand), and **actionable** (suggestions link to fixes, not just describe problems).

---

## User Workflows

### Primary Flow: Game / Trainer Launch Failure (Proton)

1. User clicks **Launch Game** or **Launch Trainer** in `LaunchPanel`
2. `ConsoleView` streams log lines as usual
3. Process exits with non-zero code
4. **[NEW]** `LaunchPanel` feedback area immediately renders a structured diagnostic panel below the launch controls — no navigation required
5. Panel shows: severity badge + one-line summary + actionable suggestion
6. User can expand "Details" to see matched log pattern and category context
7. User can expand "Raw Log" section (already in `ConsoleView`) without leaving the panel
8. User clicks "Copy Diagnostic Report" → clipboard receives Markdown-formatted summary → transient "Copied!" toast confirms
9. User adjusts profile settings or follows suggestion → re-launches

### Alternative Flow: Power User Bug Report

1. Same steps 1–7 as above
2. User clicks "Copy Diagnostic Report" with full detail level
3. Report includes: exit code/signal, Proton version, pattern matches, crash report metadata, sanitized environment context
4. User pastes directly into GitHub issue or ProtonDB report

### Alternative Flow: Multiple Simultaneous Failures

1. Several patterns match (e.g., SIGABRT exit + ntdll.dll load failure)
2. Panel shows **highest-severity entry** as the headline with a count badge ("2 issues found")
3. User expands to see ordered list — fatal first, then warnings
4. The most-likely root cause is listed first (by priority order from pattern definitions)

### Alternative Flow: Exit Code 0 with Suspicious Patterns (Phase 1.1, deferred)

1. Game exits cleanly but WINE logs show fixme/error lines
2. Panel shows `info`-severity advisory — visually distinct (blue) from fatal errors
3. Non-intrusive: no auto-focus, no modal; appended below `ConsoleView`

### No-Failure Flow (Happy Path)

1. Process exits with code 0 and no suspicious patterns
2. No diagnostic panel appears — `LaunchPanel` returns to idle state normally
3. `ConsoleView` remains visible and scrollable for reference

---

## UI/UX Best Practices

### Placement: Extend the Existing Feedback Panel

The `crosshook-launch-panel__feedback` element (lines 730–757 in `LaunchPanel.tsx`) already renders severity badges, titles, and help text for pre-launch validation errors from #39. Diagnostics should use this same area with the same component structure rather than a separate panel or modal. This means:

- **No new layout slots** — reuse the feedback container that slides in when `feedback` is set
- **Consistent visual language** — same `data-severity` attribute drives CSS color tokens
- The only addition is an expand/collapse toggle for "Show Details" and a "Copy Report" button

```
┌─────────────────────────────────────────────────────────┐
│  [FATAL] Game exited: SIGSEGV (signal 11)               │
│  The process crashed with a segmentation fault.         │
│                                                         │
│  Suggestion: Check that the Proton version in your      │
│  profile is compatible with this game. Try switching    │
│  to Proton Experimental or GE-Proton.                   │
│                                                         │
│  [▼ Show Details]  [Copy Report]  [2 issues found ▸]   │
└─────────────────────────────────────────────────────────┘
```

### Progressive Disclosure: Three Levels

**Level 1 — Always-visible summary** (feedback panel):

- Severity badge (Fatal / Warning / Info) matching existing `ValidationSeverity` rendering
- One-line title: what failed (exit code + signal name OR pattern category)
- One-line suggestion: what to do

**Level 2 — Expandable details** (collapsed by default):

- Matched log pattern (the triggering line, sanitized, capped at 512 chars)
- Category label (e.g., "Proton Runtime", "File Permission")
- All diagnostic entries if multiple, ordered by severity then priority
- Crash report metadata if present (count, size, timestamp — never contents)
- Proton version string if available

**Level 3 — Raw log** (existing `ConsoleView` functionality):

- Already available via `ConsoleView` collapse/expand
- "Jump to Error" affordance: a button that scrolls `ConsoleView` to the first error line if the panel contains a `matched_line`

### Severity Color Coding (Using Existing Tokens)

The existing CSS variables already have the correct tokens:

| Severity  | CSS Variable                      | Value     | Usage                    |
| --------- | --------------------------------- | --------- | ------------------------ |
| `fatal`   | `--crosshook-color-danger`        | `#ff758f` | Badge, left border, icon |
| `warning` | `--crosshook-color-warning`       | `#f5c542` | Badge, left border, icon |
| `info`    | `--crosshook-color-accent-strong` | `#2da3ff` | Badge, left border, icon |

No new color tokens needed. The existing `data-severity` CSS attribute pattern used in the LaunchPanel validation feedback applies directly.

### Error Message Structure

Three-part structure for every diagnostic entry:

```
[BADGE: FATAL]  Game exited: SIGABRT (signal 6)
                Process crashed with an assertion failure or abort.
                → Delete the Proton prefix and let it rebuild on next launch.
                  (Profile → Prefix Path → folder icon to open in Files)
```

- **Title**: `"[Process] exited: [signal name or generic description]"` — factual, no blame
- **Detail**: One sentence explaining the technical cause in plain language
- **Suggestion**: Starts with `→` arrow, specific action. Cross-references CrossHook UI elements by name where the fix is achievable in-app (e.g., "Profile → Prefix Path")

**Anti-patterns to avoid** (from competitor audit):

- Never say "An error occurred" without a specific code or category
- Never leave the suggestion empty — even "No known fix; check raw log for details" is better than silence
- Never use technical jargon as the headline without the human-readable translation alongside it (e.g., don't say "status c0000135" alone — say "Missing .NET runtime (status c0000135)")

### Multiple Issues Display

When `entries.length > 1`:

- Show count badge: `"2 issues found"` (clickable / expandable)
- Primary display shows the highest-priority `fatal` entry
- Expanded view shows all entries as a list, sorted: fatal → warning → info
- Visually distinguish "likely root cause" from "related issues" if the first fatal entry is significantly higher priority than subsequent ones — simple label: "Primary:" / "Also detected:"

---

## Error Handling UX

### Error States Reference

| State                             | Trigger                           | Visual Treatment                                                               |
| --------------------------------- | --------------------------------- | ------------------------------------------------------------------------------ |
| Single fatal error, known pattern | Pattern match + non-zero exit     | Red feedback panel, specific title + suggestion                                |
| Single fatal error, unknown       | Non-zero exit, no pattern matches | Red panel, generic "Process exited with code N — check raw log"                |
| Signal-based crash                | `signal()` returns Some           | Red panel, signal translated to name (SIGSEGV, SIGABRT, etc.)                  |
| Warning-level pattern, exit 0     | Exit 0 + suspicious pattern       | Blue/amber panel, `info` or `warning` severity, non-blocking                   |
| Multiple issues                   | Multiple patterns matched         | Red panel (highest severity), count badge, expandable list                     |
| Crash reports found               | `crashreports/` files < 5 min old | Info line inside expanded details: "Proton crash dump found (2 files, 1.4 MB)" |
| No diagnostic at all              | Exit 0, no patterns               | No panel; feedback area remains hidden                                         |

### Confidence and Ambiguity Signaling

Not all patterns have equal reliability (HIGH vs MEDIUM vs LOW detectability per BR-2). Display this without overwhelming users:

- **HIGH confidence** patterns: shown as direct assertions — "A required DLL could not be loaded."
- **MEDIUM confidence** patterns: soften language — "A file permission issue may be preventing launch."
- **LOW confidence** (heuristic, e.g., short runtime) — mark as advisory: "The process exited very quickly, which _may_ indicate a startup failure."

Do NOT show a confidence meter or percentage — this adds cognitive load. Language softening alone signals uncertainty without requiring the user to understand confidence scores.

### Priority Ordering Rule

When multiple diagnostics are present, the display order rule is:

1. Fatal entries first (sorted by pattern priority)
2. Warning entries second
3. Info entries last

The pattern priority from `FAILURE_PATTERN_DEFINITIONS` (ntdll.dll missing = highest, timing heuristic = lowest) serves as the tiebreaker within each severity level. This mirrors the existing `sortIssuesBySeverity()` in `LaunchPanel.tsx:69`.

---

## Performance UX

### Analysis Timing

Diagnostic analysis runs **after** process exit, not during streaming (per BR-7). From a UX perspective this means:

- No "analyzing..." spinner during active log streaming — this would be noise
- After the last log line appears, a brief `"Analyzing..."` state (200–400ms) before showing the diagnostic panel is acceptable and provides visual continuity
- If analysis completes in < 200ms (typical for substring matching on 2MB), skip the intermediate state entirely

### Progressive Diagnostic Display

The `launch-diagnostics` event carries the complete `DiagnosticReport`. Render all entries at once — do not stream individual diagnostics. The full analysis is fast enough (substring matching, not ML inference) that progressive display would add unnecessary complexity.

### Large Log Handling

- Pattern matching is capped to the **last 2MB** of log content (per BR-2)
- `ConsoleView` itself may have thousands of lines in memory — use the existing virtualization approach or the `react-window` library if performance becomes an issue in a future iteration
- For v1, the log display is append-only and there is no evidence of performance problems from the current implementation. Do not add virtualization preemptively.

### "Analyzing" State Indicator

A minimal approach: after the log stream ends (signaled by the `launch-complete` event or equivalent), show a single-line status in the `ConsoleView` footer or feedback area:

```
● Analyzing output...
```

This should:

- Use a pulsing dot animation (CSS `animation: pulse 1s ease-in-out infinite`)
- Disappear entirely when `launch-diagnostics` event arrives
- Only show if analysis takes more than 200ms (avoid flash of loading state)

---

## Competitive Analysis

### Lutris

**Failure UX**: Log button in right sidebar; after failure, shows "Launching" spinner indefinitely with no timeout or error state. Explicit user action required to view logs.

**Known issue**: Silent failure when `umu` download fails — "Launching" state never resolves. [GitHub #6402](https://github.com/lutris/lutris/issues/6402)

**What works**: Dedicated "System Info" dialog is useful for generating support reports. The separation of system info from game logs is sensible.

**What to adopt from Lutris**: System context in bug reports (OS version, hardware). **What to avoid**: The stuck-launching anti-pattern and the multi-step log navigation.

### Heroic Games Launcher

**Failure UX**: Error requires 3 clicks to reach (card → Tools → "Latest Log"). Status remains "Launching" after failure. Credentials expiry produces zero feedback.

**Known issue**: Log modal crashes the app when log files exceed hundreds of MB. [GitHub #4699](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/4699)

**What works**: The "Tools" tab concept for advanced diagnostics is reasonable for power users. The wiki-based troubleshooting guide is comprehensive.

**What to adopt from Heroic**: A "Latest Log" quick-access shortcut inside the diagnostic panel (link that opens/scrolls to the log). **What to avoid**: Hiding diagnostics behind game-specific menus; loading entire log into memory.

### Bottles

**Failure UX**: No in-app failure feedback. Failures are either silent or require terminal. The Bottles "Eagle" feature provides pre-launch analysis (executable format, architecture) but nothing post-launch.

**What works**: Eagle's pre-launch architecture check is an interesting complement to post-launch diagnostics. CrossHook already has pre-launch validation (#39) that parallels this.

**What to adopt from Bottles**: Nothing directly. Bottles demonstrates the lower bound — complete absence of diagnostic UX. **What to avoid**: Treating all failures as silent non-events.

### Steam (Big Picture / Deck UI)

**Failure UX**: OS-level notification "Failed to start game (missing executable)" with no detail. Big Picture mode uses large modal dialogs with D-pad navigation — modal closes with B-button, offers Retry.

**What works**: Auto-focus management on modal open (B-button always dismisses), clear retry affordance, large touch targets (~56px min height in Big Picture).

**What to adopt from Steam**: Auto-focus the primary CTA when the diagnostic panel opens; provide a visible "Try Again" button alongside "Copy Report" and "Dismiss". **What to avoid**: The lack of any actionable information in the notification itself.

### VS Code (reference, non-gaming)

**Failure UX**: Problems panel extracts compiler errors from terminal output into a structured list. Errors link to file:line. Quick Fix lightbulb appears inline.

**What to adopt from VS Code**: The concept of extracting structured diagnostics _out of_ the raw stream into a separate display, while keeping the raw stream accessible. The LaunchPanel feedback panel is the CrossHook equivalent of the Problems panel.

---

## Steam Deck Considerations

### Layout Budget (1280×800)

The `--crosshook-console-drawer-height: 280px` variable suggests the existing drawer model. The diagnostic panel should fit within the feedback area of `LaunchPanel` without requiring drawer expansion:

- **Collapsed summary**: ~80–96px (badge + title + suggestion, single line each)
- **Expanded details**: ~240px max before internal scroll kicks in
- Total with expanded state: fits within the 280px console drawer height

### Touch Targets

The existing `--crosshook-touch-target-min: 48px` variable is already set. All interactive elements in the diagnostic panel — "Show Details", "Copy Report", "Try Again", individual expandable entries — must meet this 48px minimum height. This is already the standard used for gamepad navigation throughout CrossHook (`useGamepadNav.ts`).

### Gamepad Navigation Focus Order

When the diagnostic panel renders after a failed launch, focus should auto-advance to the panel's primary action button:

```
Tab order: [Show Details toggle] → [Copy Report] → [Try Again / Re-launch] → [Dismiss]
```

- `A` button (confirm): activates focused element
- `B` button (back): dismisses diagnostic panel / returns to launch state
- `D-pad Up/Down`: navigates between diagnostic entries in expanded view
- Right thumbstick: scrolls expanded details area (consistent with ConsoleView scroll behavior)

No hover-dependent interactions. The "Show Details" toggle must work via keyboard/gamepad activation, not mouse hover.

### Readable on 1280×800

- Font size minimum: 14px for body text, 16px for titles (existing theme standard)
- Avoid >3 visible diagnostic entries without scroll — truncate to "N more issues" affordance
- The matched log line snippet (from `matched_line` field) should be monospace, truncated at ~80 chars visible before horizontal scroll, matching `ConsoleView` styling
- Contrast: `--crosshook-color-danger` (`#ff758f`) on `--crosshook-color-bg` (`#1a1a2e`) meets WCAG AA (ratio ≈ 4.7:1)

### Controller-Accessible Copy

"Copy Diagnostic Report" must be a first-class focusable button — not tucked inside a dropdown menu. Steam Deck users can't easily right-click. Place it in the tab order alongside "Try Again" and "Dismiss."

---

## Recommendations

### Must Have (P0 — Core Diagnostic UX)

1. **Inline failure panel in LaunchPanel feedback area**: Use the existing `feedback` render path — add a `diagnostic` kind to `LaunchFeedback`. Same severity badge + title + help structure as validation errors from #39. No new layout required.

2. **Auto-focus management**: When the diagnostic panel appears after a failed launch, focus the "Show Details" or primary CTA button automatically. Essential for Steam Deck.

3. **"Copy Report" button as first-class control**: Prominently placed (not in dropdown), always visible when panel is shown, shows "Copied!" toast on activation (2s transient, non-blocking).

4. **Severity-appropriate color**: Map `fatal → --crosshook-color-danger`, `warning → --crosshook-color-warning`, `info → --crosshook-color-accent-strong`. Already defined in `variables.css`.

5. **Multiple issues count badge**: If `entries.length > 1`, show a badge "N issues" that toggles the expanded list. Primary display shows only the top-priority entry — avoid overwhelming with a wall of errors on first view.

### Should Have (P1 — Completeness)

6. **Expandable details section**: Toggle showing matched log line (sanitized, 512 char cap), category label, and crash report metadata. Default collapsed. Keyboard/gamepad accessible.

7. **"Analyzing..." interim state**: Pulsing dot indicator in the feedback area after log stream ends, before `launch-diagnostics` event arrives. Only show if analysis exceeds 200ms.

8. **"Try Again" action**: Re-launch button alongside "Copy Report" and "Dismiss" in the diagnostic panel. Users often want to retry after following a suggestion.

9. **Proton version in report**: Include Proton version string in the clipboard export for Proton launches. Surfaces automatically from `DiagnosticReport.proton_version`.

10. **Confidence language softening**: HIGH-confidence patterns use assertive language; MEDIUM-confidence patterns use "may be"; LOW-confidence patterns use "might indicate". No numeric confidence scores visible to users.

### Nice to Have (P2 — Polish)

11. **"Jump to Error" in ConsoleView**: After a pattern match with `matched_line`, offer a button that scrolls `ConsoleView` to the first occurrence of the matched line. Requires adding a search/scroll API to `ConsoleView`.

12. **Distinct game vs trainer labeling**: Diagnostic title prefix: "Game:" vs "Trainer:" when the failure source is known. Already supported by `target_kind` in `DiagnosticReport`.

13. **Dismiss / collapse affordance**: Allow the user to collapse the diagnostic panel without clearing state — they can reference the raw log while keeping the summary visible. A minimize icon in the panel header.

14. **Animated entry**: Subtle slide-in animation for the diagnostic panel (100ms) using `--crosshook-transition-fast`. Avoid jarring pop-in; use `transform: translateY` to slide up from below the launch controls.

---

## Open Questions

1. **Should "Try Again" be part of the diagnostic panel or remain a separate button in the existing launch controls?** The existing LaunchPanel already has launch/stop buttons. Duplicating a "Re-launch" here risks confusion — the current button state may be sufficient.

2. **How should the panel behave when a second launch starts?** The `useLaunchState` reducer clears diagnostics on new launch (per BR-8), but should the panel animate out or snap away? A transition out (100ms fade) would feel cleaner.

3. **Should the "Details" section show all diagnostic entries, or only fatal ones?** Showing all can be overwhelming; hiding warnings behind an extra click may hide useful context. Suggestion: show all entries in expanded view but visually group them by severity.

4. **For the `matched_line` display, should the matched substring be highlighted within the 512-char snippet?** This would help users spot the relevant part quickly in a long log line. Requires knowing the match start position from the backend — worth adding to `DiagnosticEntry` if feasible.

5. **Toast library or inline feedback for "Copied!"?** CrossHook has no existing toast component. A simple CSS-animated inline state change on the Copy button ("Copy Report" → "✓ Copied!" → "Copy Report" after 2s) avoids a new dependency and works well for a single-action confirmation.
