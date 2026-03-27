# UX Research: Dry Run / Preview Launch Mode

## Executive Summary

This document presents UX research findings for CrossHook's "Preview Launch" feature (GitHub issue #40) — a dry-run mode that shows exactly what CrossHook will do before launching a game. The research covers user workflows, UI patterns for displaying dense technical information, competitive analysis of similar preview/plan outputs, Steam Deck gamepad-friendly design, and error handling UX.

**Key recommendation**: Use a **modal dialog** (building on the existing `ProfileReviewModal` pattern) with **collapsible accordion sections** (using the existing `CollapsibleSection` component) as the primary preview container. This approach leverages CrossHook's established UI primitives, provides focused attention on the preview results, supports full-screen gamepad navigation, and enables a clear "Preview → Decide → Launch or Edit" workflow.

**Confidence**: High — grounded in NNGroup research, competitive analysis of Terraform/Docker/IDE patterns, and alignment with CrossHook's existing component library.

---

## User Workflows

### 1.1 Primary Flow: Preview Before Launch

```
User configures profile → clicks "Preview Launch" button
    → System computes launch plan (env vars, wrappers, command, validation)
    → Modal opens showing structured preview
    → User reviews each section
    → Decision point:
        ├── "Launch" → closes modal, initiates actual launch
        ├── "Close" → returns to profile editor to adjust config
        └── "Copy" → copies preview text for sharing/debugging
```

**Key UX principles:**

- The preview button should sit **alongside** the primary launch button, visually subordinate (secondary/ghost style) to avoid confusion about which button actually launches
- The modal should clearly communicate: "No game will be launched. This is a preview."
- The transition from preview to actual launch should require a deliberate action (button click), not auto-launch

**Confidence**: High — aligns with the "Dry Run Button" UX pattern documented by Praxen and NNGroup confirmation dialog research.

### 1.2 Debug Flow: Post-Failure Investigation

```
User launches game → launch fails or behaves unexpectedly
    → User clicks "Preview Launch" to understand what happened
    → Reviews resolved environment variables, command chain, validation
    → Identifies misconfiguration (wrong Proton version, missing wrapper, bad path)
    → Adjusts profile → re-previews → launches
```

**Key UX principles:**

- Preview output should persist so users can reference it while editing — but since a modal blocks interaction, provide a "Copy to Clipboard" button so the user can capture the state, close the modal, and reference it while editing
- Validation results should surface potential issues prominently, even if they wouldn't block the launch (warnings)
- Failed validations should clearly explain what went wrong and what the user can do about it

**Confidence**: High — validated by how Terraform plan and IDE run configurations serve debugging use cases.

### 1.3 Sharing Flow: Copy for Troubleshooting

```
User encounters issue → opens preview
    → Clicks "Copy Preview" button
    → Pastes into Discord / GitHub issue / forum post
    → Community or developer can see exact launch configuration
```

**Key UX principles:**

- The copied text should be **plain-text formatted**, not HTML — ready for pasting into any text context
- Include a structured header (CrossHook version, profile name, timestamp) for context
- Sensitive information (full file paths with usernames) should be present but the user should be aware they're sharing it
- Format should be easily parseable by humans reading it in a monospace context (Discord code blocks, GitHub issues)

**Confidence**: Medium — common pattern in developer tools but specific clipboard format needs user testing.

---

## 2. UI/UX Best Practices

### 2.1 Container Pattern: Modal Dialog (Recommended)

**Decision: Modal dialog over inline panel or expandable section.**

| Pattern            | Pros                                                                                                        | Cons                                                                                                       | Fit for CrossHook                                                                                                                      |
| ------------------ | ----------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| **Modal dialog**   | Focused attention; full-screen gamepad support; clear enter/exit; existing `ProfileReviewModal` to build on | Blocks background interaction; can't reference profile while modal is open                                 | **Best fit** — preview is a discrete inspection action, not a persistent state                                                         |
| Inline panel       | Non-blocking; can reference profile simultaneously                                                          | Competes for space on 1280x800 screen; information density challenges; complex layout interactions         | Poor fit — too much data for inline display                                                                                            |
| Expandable section | Lightweight; progressive disclosure                                                                         | Insufficient real estate for command chains + env vars + validation; would push launch controls off-screen | Poor fit — CrossHook already uses `CollapsibleSection` heavily; adding another large section to LaunchPage would create scroll fatigue |
| Side drawer        | Non-blocking; dedicated space                                                                               | 1280x800 screen too narrow for side-by-side; gamepad navigation complexity                                 | Poor fit — screen constraints                                                                                                          |

**Rationale (NNGroup research):**

- Modal dialogs are appropriate when users need to "make a relevant and distinct choice" (launch vs. adjust) and the content demands focused attention
- The preview is a **discrete inspection action** — the user explicitly requested to see the plan. This is not an interruption; it's a response to user intent
- Since users need to review dense technical information (env vars, command chains, validation), a full-viewport modal provides the most reading space on the 1280x800 Steam Deck display
- CrossHook already has a well-built modal system (`ProfileReviewModal`) with focus trapping, keyboard navigation, portal rendering, and gamepad support — reuse is both efficient and consistent

**Confidence**: High — NNGroup explicitly recommends modals for user-initiated inspection actions with complex content.

### 2.2 Internal Structure: Accordion Sections

Within the modal, organize preview data into collapsible accordion sections using CrossHook's existing `CollapsibleSection` component. This applies the **progressive disclosure** principle — users see all section headers at a glance and expand only what they care about.

**Recommended sections (in display order):**

1. **Summary** (always expanded, not collapsible)
   - Profile name, launch method, game executable, Proton version
   - Overall validation status badge (Pass / Warning / Fail)
   - One-line description of what will happen

2. **Validation Results** (expanded by default)
   - List of all checks with pass/warning/error status
   - Most important section for debugging — users need to see issues first

3. **Command Chain** (expanded by default)
   - The effective `%command%` string or full wrapper chain
   - Shows: gamemoderun → mangohud → proton → game.exe (visual pipeline)

4. **Environment Variables** (collapsed by default)
   - Key-value list of all resolved environment variables
   - Most users won't need this unless debugging

5. **Proton / Runtime Setup** (collapsed by default)
   - Proton prefix path, compatibility tool, WINE version
   - Steam library paths, app manifest details

**Default state rationale**: NNGroup research recommends allowing multiple sections open simultaneously and not auto-collapsing when one opens. The first three sections (Summary, Validation, Command Chain) cover 80% of use cases and should default to expanded. Env vars and Proton details are for advanced debugging only.

**Confidence**: High — matches NNGroup accordion guidelines and CrossHook's existing `CollapsibleSection` pattern.

### 2.3 Data Presentation Patterns

#### Environment Variables: Key-Value Table

Display environment variables as a **two-column layout** with monospace values:

```
WINEPREFIX          /home/user/.local/share/Steam/steamapps/compatdata/12345/pfx
PROTON_USE_WINED3D  1
DXVK_HUD            fps,frametimes
MANGOHUD            1
SteamAppId          12345
```

**Best practices:**

- Use `var(--crosshook-font-mono)` for both keys and values
- Left-align keys, left-align values
- Use `var(--crosshook-color-text-muted)` for keys, `var(--crosshook-color-text)` for values to create visual hierarchy
- Add subtle row separators using `var(--crosshook-color-border)` for scanability
- Keep rows single-line where possible; for long values, allow horizontal scroll or text wrapping with `word-break: break-all`

**Confidence**: High — standard pattern across Docker inspect, IDE settings, and terminal tool configurations.

#### Command Chain: Visual Pipeline

Display the full command chain as a **syntax-highlighted, monospace block** with visual structure:

```
gamemoderun \
  mangohud \
  /path/to/proton run \
  /path/to/game.exe \
  --launch-args
```

**Best practices:**

- Use a dark, slightly recessed container (`var(--crosshook-color-surface)` background) to visually distinguish from surrounding content — similar to a code block or terminal view
- Highlight wrapper commands (gamemoderun, mangohud) with `var(--crosshook-color-accent)` tint
- Show the game executable path with standard text color
- Show launch arguments with `var(--crosshook-color-text-muted)`
- If the chain is a single-line `%command%` string, show it as-is; if it's a multi-step wrapper chain, show each wrapper on its own line with line-continuation markers
- Include a small "Copy" button in the top-right corner of the code block

**Confidence**: High — mirrors Terraform plan, VS Code terminal, and Docker Compose output patterns.

#### Validation Results: Status List with Severity Icons

Display validation results as a **vertical list** with color-coded severity indicators:

```
[✓] Game executable exists at path        — Pass
[✓] Proton version found                  — Pass
[!] mangohud not found in PATH            — Warning
[✗] Trainer executable missing            — Error
```

**Best practices (based on PatternFly and Carbon Design System research):**

- Use three severity levels matching CrossHook's existing color tokens:
  - **Pass**: `var(--crosshook-color-success)` (#28c76f) — checkmark icon
  - **Warning**: `var(--crosshook-color-warning)` (#f5c542) — triangle/exclamation icon
  - **Error/Fatal**: `var(--crosshook-color-danger)` (#ff758f) — X/circle icon
- Never rely on color alone — always pair with icon and text label (accessibility)
- Group by severity: errors first, then warnings, then passes
- Each result should include: icon, check name, one-line description, and expandable detail text for help/fix suggestions
- Use the existing `data-severity` pattern from `LaunchPanel` feedback display

**Confidence**: High — aligns with PatternFly status patterns, NNGroup validation guidelines, and CrossHook's existing severity pattern.

### 2.4 Summary Banner: Terraform-Inspired

At the top of the preview modal (within the always-visible Summary section), display a **Terraform plan-style summary line**:

```
Preview: 12 env vars set, 3 wrappers active, 5 checks passed, 1 warning
```

Or with issues:

```
Preview: 12 env vars set, 3 wrappers active, 3 checks passed, 1 warning, 1 error — cannot launch
```

**Best practices:**

- Color-code the counts: green for passes, yellow for warnings, red for errors
- This summary gives users an instant "health check" before they dig into sections
- The presence of errors should change the modal footer actions (disable "Launch Now", show only "Close" or "Copy")
- Inspired by Terraform's `Plan: X to add, Y to change, Z to destroy` summary line

**Confidence**: High — Terraform plan's summary line is widely praised in developer UX research.

---

## 3. Error Handling UX

### 3.1 Preview Computation Errors

**Scenario**: The preview itself fails to compute (e.g., profile is incomplete, Tauri command errors).

**Recommended approach:**

- Show the modal with whatever partial data was computed
- Replace uncomputed sections with a clear error state: "Could not compute environment variables: no Proton version selected"
- Use the existing `data-severity="fatal"` styling pattern from `LaunchPanel`
- The summary banner should reflect: "Preview incomplete — some sections could not be computed"
- Still allow "Copy" so users can share the partial result for debugging

**Confidence**: High — graceful degradation is a well-established UX principle for preview/inspection tools.

### 3.2 Invalid Profile State

**Scenario**: User clicks "Preview" but the profile is missing required fields (no game executable, no launch method selected).

**Recommended approach:**

- **Option A (preferred)**: Disable the "Preview Launch" button when the profile is clearly incomplete (same conditions that disable the launch button), with a tooltip explaining why
- **Option B**: Show the modal but with prominent validation errors explaining what's missing
- Both options should mirror the existing `canLaunch` / `feedback` pattern in `LaunchPanel`

**Confidence**: High — mirrors existing CrossHook validation patterns.

### 3.3 Validation Warnings vs. Errors

**Distinction:**

- **Errors** (fatal): Block the launch. The game will not work. The "Launch Now" button in the preview modal should be disabled.
  - Examples: game executable not found, Proton version not installed, missing required environment
- **Warnings** (non-fatal): Something may not work as expected, but the launch can proceed. The "Launch Now" button remains enabled.
  - Examples: mangohud not in PATH (game will launch without overlay), trainer path unverified, DXVK_HUD set but DXVK not detected
- **Info** (informational): Neutral observations. No action required.
  - Examples: using default Proton version, Steam app ID resolved from manifest

This three-tier system matches CrossHook's existing `severity: 'fatal' | 'warning' | 'info'` in the LaunchPanel feedback system.

**Confidence**: High — directly reuses CrossHook's existing severity model.

---

## Performance UX

### 4.1 Loading State

The preview computation should be **near-instant** since it's purely resolving paths, environment variables, and validation checks without I/O-heavy operations. However:

- Show a **brief loading state** (100-200ms minimum display time) to signal that computation occurred — this builds user trust that the preview is fresh, not cached
- Use a subtle skeleton or shimmer effect within the modal body while computing, matching CrossHook's `var(--crosshook-transition-fast)` (140ms) timing
- If computation takes longer than 500ms (e.g., Steam manifest parsing), show a progress message: "Resolving Proton environment..."

**Confidence**: Medium — the 100-200ms minimum display is a UX heuristic (Jakob Nielsen's response time guidelines) but the actual computation time needs profiling.

### 4.2 Transition Animations

- Modal entrance: fade backdrop + scale-up surface (matching existing `ProfileReviewModal` behavior)
- Section expand/collapse: use `var(--crosshook-transition-standard)` (220ms) for smooth accordion transitions
- No animation on close if user clicked "Launch Now" — immediate transition to launch state

**Confidence**: High — consistent with CrossHook's existing animation patterns.

---

## 5. Competitive Analysis

### 5.1 Terraform Plan

**What it does well:**

- **Color-coded symbols**: `+` green (create), `~` yellow (change), `-` red (destroy) provide instant visual parsing
- **Summary line**: `Plan: X to add, Y to change, Z to destroy` gives an immediate overview before details
- **Resource grouping**: Changes are grouped by resource with `# resource_type.resource_name will be [action]` headers
- **Attribute-level diffs**: Shows before/after values for each changed attribute
- **Safety messaging**: "Note: You didn't use the -out option..." reminds users about best practices

**What to adopt for CrossHook:**

- The summary line pattern → apply to the preview banner
- Color-coded severity indicators → apply to validation results
- Clear section headers with action descriptions → apply to accordion section titles (e.g., "Environment Variables — 12 variables set")

**Confidence**: High — Terraform plan is the direct inspiration for this feature.

### 5.2 Docker Desktop — Container Inspect

**What it does well:**

- **Tabbed interface**: Logs, Inspect, Bind mounts, Debug, Files, Stats tabs organize dense data into focused views
- **Key-value display**: Container metadata shown as clean label-value pairs
- **JSON view**: Raw inspect data available for advanced users
- **Quick actions**: Pause, resume, start, stop buttons alongside inspection

**What to adopt for CrossHook:**

- Key-value presentation style for environment variables
- The concept of showing "raw" output alongside formatted output (optional: a "Copy as TOML/JSON" option for the preview data)

**Confidence**: Medium — Docker's tab-per-topic pattern translates to CrossHook's accordion-per-section, but Docker has more screen real estate.

### 5.3 VS Code / JetBrains — Run Configurations

**What they do well:**

- **Structured forms**: Run configurations show settings as labeled fields (executable, arguments, working directory, environment variables)
- **Variable resolution**: VS Code shows `${workspaceFolder}` in config but resolves to actual paths at runtime
- **One-click launch**: "Run" button directly in the configuration view
- **Configuration comparison**: VS Code's launch.json allows multiple configurations with easy switching

**What to adopt for CrossHook:**

- Show both the "configured" value and the "resolved" value where they differ (e.g., configured: `${PROTON_PATH}`, resolved: `/path/to/proton`)
- Place "Launch Now" action directly in the preview modal (like VS Code's Run button in configuration)

**Confidence**: Medium — IDE patterns apply conceptually but their form-based approach differs from CrossHook's read-only preview.

### 5.4 Lutris — Game Configuration

**What it does well:**

- **Settings hierarchy**: System → Runner → Game configuration layers
- **Launch configs**: Named configurations for games with multiple executables
- **Configuration override visibility**: Clear display of which level overrides which

**What to adopt for CrossHook:**

- Consider showing the "source" of each setting in the preview (profile, settings override, default) — helps users understand why a variable has a particular value
- Launch method labeling (Steam / Proton / Native) in the preview header

**Confidence**: Medium — Lutris's configuration view is functional but not highly polished UX to emulate.

### 5.5 Steam — Launch Options Dialog

**What it does:**

- Simple text input field for custom launch options
- No preview of what the full command will look like
- No validation of the options entered

**What to learn (negative example):**

- Steam's lack of preview is precisely the gap CrossHook fills
- Users frequently ask "what does my `%command%` actually resolve to?" — CrossHook's preview answers this
- Steam provides zero feedback about whether launch options are valid

**Confidence**: High — Steam's minimal approach validates the need for CrossHook's preview feature.

---

## 6. Steam Deck Considerations

### 6.1 Screen Constraints (1280x800)

- The preview modal should use **near-full-viewport sizing** (similar to existing `ProfileReviewModal`), with comfortable padding (16-24px margins)
- Available content area: approximately 1232x720 after padding and modal chrome (header + footer)
- Section headers must be **large enough for touch/gamepad targets** — minimum 48px height (matching `var(--crosshook-touch-target-min)`)
- Font sizes should remain at the existing CrossHook body size — avoid shrinking text to fit more content

**Confidence**: High — based on CrossHook's existing responsive breakpoints and touch target standards.

### 6.2 Scrollable Content with Gamepad

- The modal body should be a **single scrollable container** — gamepad D-pad/left stick controls scroll position
- CrossHook already has `scroll-margin-block: 20px` on focusable elements within `.crosshook-modal__body` — this ensures focused elements aren't hidden behind the header
- Accordion expand/collapse should be triggered by the **A button** (confirm) on the focused section header
- When a section expands, scroll the view to show the section header at the top with its content visible below

**Confidence**: High — aligns with CrossHook's existing `useGamepadNav` hook and focus management system.

### 6.3 Focus Management in Modal

- Focus trap within the modal is already implemented in `ProfileReviewModal` — reuse this pattern
- Tab order: Summary → Validation section header → validation items → Command Chain section header → command content → Env Vars section header → env var rows → Proton section header → proton details → footer actions (Copy / Launch / Close)
- The footer "Launch Now" button should be the **last focusable element before focus wraps**, making it easy to reach via Shift+Tab from the top
- Use `data-crosshook-focus-root="modal"` attribute for gamepad navigation root

**Confidence**: High — directly leverages existing CrossHook modal focus management.

### 6.4 Copy to Clipboard Without Keyboard

- Provide a prominent **"Copy Preview"** button in the modal footer
- Use `navigator.clipboard.writeText()` via Tauri's clipboard API
- After copying, show brief **toast/inline feedback**: "Copied to clipboard" with a checkmark — auto-dismiss after 2 seconds
- The button should be a standard `crosshook-button` with gamepad focus support (48px minimum height)
- On Steam Deck, clipboard content can be pasted using the on-screen keyboard's paste button in other apps

**Note on Steam Deck clipboard limitations**: SteamOS clipboard sync between the gaming context and desktop apps has known issues (Valve gamescope issue #916). The copied text will be available within the CrossHook session and in desktop mode apps, but may not transfer to games running under gamescope. This is a platform limitation, not a CrossHook issue.

**Confidence**: Medium — clipboard API is reliable; Steam Deck clipboard sync across contexts has documented limitations.

### 6.5 Controller Button Prompts

- When in controller mode (`data-crosshook-controller-mode='true'`), show contextual button prompts at the bottom of the modal:
  - **A**: Expand/Collapse section | Confirm
  - **B**: Close preview
  - **X**: Copy preview
  - **Y**: Launch Now
- Use the existing `.crosshook-controller-prompts` component for consistent styling

**Confidence**: High — CrossHook already has controller prompt infrastructure.

---

## 7. Recommendations

### Must Have (MVP)

1. **Modal dialog container** — reuse `ProfileReviewModal` structure with adapted header/footer
2. **Summary banner** with Terraform-style counts (env vars set, wrappers active, checks passed/warned/failed)
3. **Validation Results section** — expanded by default, color-coded severity with icons, grouping by severity
4. **Command Chain section** — expanded by default, monospace code block with visual structure
5. **Environment Variables section** — collapsed by default, key-value table with mono font
6. **Copy to Clipboard** button — copies full preview as plain text
7. **"Launch Now" button** — directly launch from preview (disabled if errors exist)
8. **Gamepad navigation** — focus management, accordion expand/collapse, controller prompts

### Should Have (Post-MVP)

1. **Proton/Runtime Details section** — collapsed by default, prefix path, compatibility tool, WINE version
2. **"Configured vs. Resolved" display** — show original profile value and resolved value side-by-side where they differ
3. **Validation detail expansion** — click on a warning/error to see help text and fix suggestions
4. **Copy individual sections** — small copy icon on each section's code block
5. **Toast confirmation** on successful clipboard copy

### Nice to Have (Future)

1. **Diff view** — if user changed profile since last preview, show what changed (before/after)
2. **Export as file** — save preview to a `.txt` or `.md` file for sharing
3. **Quick-fix actions** — from a validation error, button to "Fix this" that navigates to the relevant profile field
4. **Preview history** — keep last N preview results for comparison
5. **Syntax highlighting** — full syntax coloring for command chains and environment variables

---

## 8. Open Questions

1. **Button placement**: Should "Preview Launch" be in the `LaunchPanel` actions bar (alongside "Launch Game" and "Reset") or in the `CollapsibleSection` header for "Launch Controls"? Recommendation: alongside the launch button, as a secondary/ghost button.

2. **Preview freshness**: Should the preview auto-refresh when the modal is open and the profile changes in the background (e.g., via another tab)? Or is a manual "Refresh" button sufficient? Recommendation: manual refresh with a subtle "Profile changed since preview" indicator.

3. **Clipboard format**: What exact plain-text format should the copied preview use? Should it be structured text, TOML, JSON, or a custom human-readable format? Recommendation: structured plain text optimized for pasting into Discord code blocks and GitHub issues.

4. **Sharing sensitivity**: Should CrossHook offer to redact usernames from file paths before copying? (e.g., replace `/home/yandy/` with `/home/<user>/`). This adds complexity but improves privacy for public sharing.

5. **Modal size**: Full-viewport modal (like `ProfileReviewModal`) or slightly smaller centered dialog? On 1280x800, full-viewport is recommended to maximize reading space. On larger desktop monitors, constrain to ~900px width for readability.

---

## Sources

### UX Research & Guidelines

- [Modal & Nonmodal Dialogs: When (& When Not) to Use Them — NNGroup](https://www.nngroup.com/articles/modal-nonmodal-dialog/)
- [Accordions on Desktop: When and How to Use — NNGroup](https://www.nngroup.com/articles/accordions-on-desktop/)
- [Confirmation Dialogs Can Prevent User Errors — NNGroup](https://www.nngroup.com/articles/confirmation-dialog/)
- [Modal vs. Separate Page: UX Decision Tree — Smashing Magazine](https://www.smashingmagazine.com/2026/03/modal-separate-page-ux-decision-tree/)
- [Mastering Modal UX: Best Practices — Eleken](https://www.eleken.co/blog-posts/modal-ux)
- [The Dry Run Button: UX That Saves Your Users Money — Praxen/Medium](https://medium.com/@Praxen/the-dry-run-button-ux-that-saves-your-users-money-a0a9be0b16fe)
- [Designing Better Error Messages UX — Smashing Magazine](https://www.smashingmagazine.com/2022/08/error-messages-ux-design/)

### Design System Patterns

- [Status and Severity — PatternFly](https://www.patternfly.org/patterns/status-and-severity/)
- [Status Indicators — Carbon Design System](https://carbondesignsystem.com/patterns/status-indicator-pattern/)
- [10 Design Guidelines for Reporting Errors in Forms — NNGroup](https://www.nngroup.com/articles/errors-forms-design-guidelines/)
- [Accordion UI Examples: Best Practices — Eleken](https://www.eleken.co/blog-posts/accordion-ui)

### Competitive Analysis

- [Terraform Plan Command: Examples & How It Works — Spacelift](https://spacelift.io/blog/terraform-plan)
- [Terraform Plan Command Reference — HashiCorp Developer](https://developer.hashicorp.com/terraform/cli/commands/plan)
- [Terraform Plan Viewer](https://terraformplan.com/)
- [Docker Desktop Containers — Docker Docs](https://docs.docker.com/desktop/use-desktop/container/)
- [VS Code Debug Configuration — Microsoft](https://code.visualstudio.com/docs/debugtest/debugging-configuration)
- [Lutris launch_configs — GitHub Issue #4648](https://github.com/lutris/lutris/issues/4648)

### Steam Deck UX

- [Steam Deck Controller Guide — Steam Community](https://steamcommunity.com/sharedfiles/filedetails/?id=2804823261)
- [Steam Deck UI 2024 — Figma Community](https://www.figma.com/community/file/1404930986444403252/steam-deck-ui-2024)
- [Clipboard sync issue — gamescope #916](https://github.com/ValveSoftware/gamescope/issues/916)
- [Copy/Paste Steam Deck Feature Requests](https://steamcommunity.com/app/1675200/discussions/2/5154941593372488674/)

### React Components

- [react-terminal-ui — GitHub](https://github.com/jonmbake/react-terminal-ui)
- [Tauri UI — GitHub](https://github.com/agmmnn/tauri-ui)

---

## Search Queries Executed

1. `terraform plan output UX design color coded preview 2025 2026`
2. `modal dialog vs inline panel vs expandable section UX tradeoffs desktop app`
3. `Steam Deck UI UX design patterns gamepad navigation modal scrollable content`
4. `displaying environment variables key value pairs UI design pattern dark theme`
5. `Lutris Linux game launcher configuration view UI preview settings`
6. `IDE run configuration preview JetBrains VS Code launch.json display UX`
7. `Docker Desktop container configuration preview inspect UI design 2025`
8. `validation results display pattern UX success warning error severity icons grouping`
9. `copy to clipboard UX pattern gamepad controller no keyboard Steam Deck`
10. `collapsible section accordion UI pattern technical information dense data display`
11. `"terraform plan" output format symbols color green red yellow additions changes UX`
12. `preview mode dry run UX pattern desktop application "what will happen" confirmation dialog`
13. `command line preview syntax highlighted terminal output dark theme UI component React`
14. `Tauri v2 desktop app drawer panel side sheet UI pattern`

---

## Uncertainties & Gaps

1. **Gamepad scroll performance**: No specific benchmarks for how smoothly large accordion content scrolls with gamepad input in Tauri v2 webview on Steam Deck hardware. Needs hands-on testing.
2. **Clipboard format preference**: No user research on whether CrossHook users prefer structured text, JSON, or TOML for copied preview data. Would benefit from community input (Discord poll or GitHub discussion).
3. **Preview trigger frequency**: Unknown how often users will use preview vs. direct launch. If preview becomes the primary workflow (always preview before launch), consider making it a persistent panel instead of a modal.
4. **Accessibility testing**: The color-coded severity system needs validation with screen readers and color-blind users. CrossHook's existing `aria-label` and `role="alert"` patterns provide a foundation but need specific testing for the preview context.
5. **Multi-step launch preview**: For `steam_applaunch` and `proton_run` methods with two-step launch flows (game + trainer), should the preview show both steps or just the current pending step? Recommendation: show both steps clearly labeled, with the pending step expanded and the completed/future step collapsed.
