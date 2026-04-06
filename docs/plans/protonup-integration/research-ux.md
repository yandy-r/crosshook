## Executive Summary

CrossHook should resolve "missing Proton version" failures with a low-friction assist flow that keeps launch momentum: detect the gap, suggest the closest compatible GE-Proton/Wine-GE version, and let users install in-app without leaving context. The UX should prioritize a one-click recommendation path while preserving a clear "continue anyway" route so launch workflows are not blocked.

The interaction model should separate concerns into: (1) immediate launch decision, (2) install task orchestration, and (3) advanced diagnostics/details. This keeps novice users moving while still giving power users visibility into compatibility logic, file paths, and logs.

Trust is critical because installs involve downloading third-party compatibility tools. Copy and UI affordances should explicitly state source, version, integrity checks, and install destination before execution, then provide clear success/failure evidence afterward.

### Core User Workflows

## Workflow A: Happy path (recommended install from launch)

1. User clicks **Launch** for a profile requiring a missing Proton version.
2. Non-blocking modal/sheet appears: "Required compatibility tool not found."
3. Primary action: **Install recommended version** (preselected, e.g., `GE-Proton9-XX`).
4. Secondary actions:
   - **Choose another version**
   - **Launch with fallback** (if a safe fallback exists)
   - **Cancel launch**
5. On confirm, launch flow transitions to background install task with visible progress panel.
6. After install success, CTA becomes **Launch now** and preserves the user's original launch intent.

Design notes:

- Keep the recommendation explicit: "Recommended for this profile."
- Preserve context (profile name, target game, missing version name).
- Do not force users into settings pages to complete the task.

## Workflow B: Error recovery during install prompt

1. Install starts and fails (network, checksum, disk, permission, dependency issue).
2. Task status changes to **Needs attention** with compact error summary.
3. Recovery actions mapped to error class:
   - Network: **Retry download**
   - Checksum/signature mismatch: **Try alternate version** / **View details**
   - Disk full: **Free space and retry**
   - Permission/path: **Open install location instructions**
   - Missing dependency (`protonup`/runtime): **Install dependency**
4. User can still **Launch with fallback** if available, or **Skip for now**.
5. If retried and success, route back to **Launch now** state.

Design notes:

- Always provide one immediate recovery action + one "details" path.
- Avoid dead-end errors; every failure state needs a next step.

## Workflow C: Proactive install from profile editing (non-launch context)

1. User opens profile compatibility settings.
2. System flags missing configured version with warning badge.
3. User clicks **Resolve missing version**.
4. Same recommendation/install flow appears inline (no forced modal if user is already in settings).
5. On success, settings update and show "Installed and selected."

Design notes:

- Reuse components from launch-time flow to reduce UX drift.
- Keep terminology and button labels identical across contexts.

## UI and Interaction Patterns

## Prompt architecture

- **Tier 1 (essential)**: problem statement, recommended version, primary CTA, safety note.
- **Tier 2 (expandable)**: source URL/domain, install path, expected size/time, release date, changelog link.
- **Tier 3 (advanced drawer)**: full compatibility reasoning, raw error/log preview, exact command details.

This progressive disclosure prevents cognitive overload while keeping advanced info accessible.

## Non-blocking launch integration

- Prompt should be interruptive but not coercive: users can defer and return.
- If user closes prompt, show persistent status chip near Launch button: "Compatibility tool missing - Resolve."
- Keep fallback launch option visible when safe; hide only when launch is guaranteed to fail.

## Version picker behavior

- Default list sorted by recommendation score, then recency.
- Label versions with tags: `Recommended`, `Installed`, `Incompatible`, `Community`.
- Inline search for power users (`GE-Proton`, `Wine-GE` filters).
- Prevent accidental downgrades with confirm step when selected version is known weaker fit.

## Pattern reuse targets in CrossHook

- Reuse existing task/progress surfaces for downloads/install operations.
- Reuse standard alert severity tokens (info/warn/error/success) to match app-wide visual language.
- Keep all action verbs consistent with existing launch UX (`Launch`, `Retry`, `Cancel`, `Open details`).

## Accessibility Considerations

- **Keyboard-first flow**: every prompt action and expandable panel must be reachable by Tab/Shift+Tab; Enter triggers primary CTA; Esc closes non-destructive prompts.
- **Screen reader clarity**: announce missing-version prompt as urgent but actionable. Include profile/game name and selected recommendation in aria labels.
- **Progress announcements**: background task status changes (queued/downloading/installing/verifying/complete/failed) should send polite live-region updates.
- **Color-independent status**: do not rely on color alone for task state; pair with icons + text labels.
- **Motion sensitivity**: progress animations should be subtle; respect reduced-motion settings.
- **Error comprehension**: plain-language summaries first, technical error details second in expandable content.

### Feedback and State Design

## State model for install UX

- `missing_detected`: prompt surfaced with recommendation.
- `user_deciding`: awaiting action.
- `task_queued`: install accepted, waiting for worker.
- `downloading`: bytes progress, speed, ETA when available.
- `installing`: extraction/copy/link steps.
- `verifying`: integrity/version checks.
- `ready_to_launch`: success with launch CTA.
- `failed_recoverable`: actionable failure + retry path.
- `failed_blocking`: no safe fallback; explicit guidance required.
- `deferred`: user skipped; persistent reminder shown.

## Background task/progress guidance

- Show concise inline status near launch controls plus richer details in global task center.
- Use deterministic status text:
  - "Downloading GE-Proton9-XX (42%)"
  - "Installing to Steam compatibility tools..."
  - "Verifying download integrity..."
- Keep launch UI responsive; installs should continue in background unless user cancels.
- On completion, trigger passive toast + contextual CTA on the original profile.

## Offline and missing dependency UX

- If offline before start:
  - Banner: "You're offline. Installation requires an internet connection."
  - Actions: **Retry when online**, **Use installed version**, **Open offline help**
- If dependency missing (e.g., protonup not available):
  - Prompt: "Installer component missing."
  - Actions: **Install dependency**, **Manual setup instructions**, **Cancel**
- If offline mid-download:
  - Pause task automatically; status: "Download paused - waiting for connection."
  - Allow manual retry and auto-resume toggle.

## Trust and safety copy guidance (downloads)

Use explicit, confidence-building copy before install:

- "CrossHook will download this compatibility tool from an official/community source."
- "Version: GE-Proton9-XX"
- "Install location: <path>"
- "Integrity checks run before activation."

Avoid vague claims like "safe download." Prefer verifiable statements:

- Good: "Checksum verification failed. The file was not installed."
- Good: "Downloaded from: github.com/GloriousEggroll/proton-ge-custom"
- Avoid: "Something went wrong."

Post-install confirmations should include:

- Installed version name
- Install location
- Verification outcome
- Next action (`Launch now`)

## UX Risks

1. **Decision overload at launch time**  
   Too many version choices can stall users. Mitigation: preselect one recommendation and hide advanced list behind "Choose another version."

2. **Trust erosion from ambiguous download messaging**  
   If source/integrity details are unclear, users may abandon. Mitigation: explicit source, checksum result, and install path in both pre- and post-install states.

3. **Launch-blocking regressions**  
   Hard-gating installs can feel punitive. Mitigation: preserve fallback/defer options where technically safe; always explain when fallback is impossible.

4. **Silent background failures**  
   If installs fail in background without clear surfacing, users loop on launch errors. Mitigation: persistent "Needs attention" state tied to profile launch controls.

5. **Inconsistent behavior between launch and settings contexts**  
   Divergent flows increase support burden. Mitigation: shared component model, shared copy dictionary, and identical state transitions across entry points.
