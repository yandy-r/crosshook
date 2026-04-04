# UX Research: protontricks-integration

## Executive Summary

The protontricks/winetricks integration feature needs to feel like a natural extension of how CrossHook already surfaces health and readiness information, not a raw CLI wrapper bolted onto the UI. Users come to CrossHook to run trainers without touching a terminal; dependency installation must remain inline, supervised, and transparent. The core UX tension is between long-running operations (dotnet48 can take 10-20 minutes) and the user's need for confidence that something is happening. CrossHook already has strong primitives for this: `ReadinessChecklist` for pre-flight checks, `ConsoleDrawer` / `ConsoleView` for real-time log streaming (extensible with a new `prefix-dep-log` event channel), and the `crosshook-status-chip` CSS pattern for status chips. The status chip pattern is reused via a new `DependencyStatusBadge` component with its own `DepStatus` type — `HealthBadge` itself is not extended, as its type is coupled to the Rust `HealthStatus` IPC enum.

The competitive landscape (Bottles, Lutris, Heroic) shows a consistent lesson: launchers that expose winetricks/protontricks as raw subprocess invocations produce confused, anxious users who cannot distinguish "stuck" from "running slow." The winning pattern is a dependency list with per-package status chips, a streaming console for install output, and a clear idle/active/error state machine per installation.

**Confidence**: High — drawn from multiple sources including GitHub issue trackers, official docs, and direct codebase analysis.

---

## User Workflows

### Primary Flow: First-Time Dependency Setup (Profile with Declared Dependencies)

1. User opens or imports a community profile with `required_protontricks` declared.
2. CrossHook background-scans the prefix against the package list (non-blocking; scan runs in background thread).
3. A **Prefix Dependencies** panel in the profile view renders each package as a status chip: `installed`, `missing`, `unknown`, `user_skipped`, or `install_failed`.
4. If any packages are missing, a "Missing dependencies" banner appears with **[Install]** and **[Skip]** buttons.
5. Choosing **Skip** records `user_skipped` in the SQLite metadata DB so the banner does not recur on subsequent profile opens. The health indicator for the profile remains amber (not fully healthy) to signal the skip.
6. Choosing **Install** opens a confirmation dialog listing the verbs to be installed with human-readable labels and any slow-install warnings. User confirms with **[Confirm Install]** or cancels. This dialog is the explicit consent gate required before CrossHook invokes protontricks.
7. On confirmation, CrossHook acquires a per-prefix install lock, then begins streaming protontricks stdout/stderr to the ConsoleDrawer.
8. Each package transitions from `missing` → `installing` → `installed` or `install_failed` as output lines arrive.
9. When all packages complete, a success summary replaces the banner and the ConsoleDrawer can be collapsed.
10. Failed packages remain marked `install_failed` with an inline remediation message and a per-package [Retry] button.

**Decision point for business rules**: The scan in step 2 resolves to `installed / missing / unknown` per package. "Installed" is determined by CrossHook metadata DB record (fast, may drift) with optional filesystem/registry cross-check. See research-business.md for the tradeoff.

### Flow: Community Profile Import Preview

1. User initiates a community profile import from the Community browser.
2. The import preview wizard includes a **"Required prefix dependencies"** section listing all `required_protontricks` packages.
3. If no prefix is configured on the profile-to-be-imported, this section shows: "Configure a prefix path to enable dependency management after import."
4. If a prefix is configured, the section shows current status for each declared package.
5. Import proceeds regardless — the wizard is informational only. Dependency installation is not gated on import.

### Flow: Manual Dependency Management Panel

The **Prefix Dependencies** panel (always present when a profile has a prefix path set) provides full control:

- All declared packages with current status chips
- User-added extras (editable list — add/remove individual packages)
- Last-checked timestamp with a **[Check Now]** button to force rescan
- **[Install Missing]** — installs only packages in `missing` or `install_failed` state
- **[Install All]** — installs all declared packages (including already-installed; useful for repair)
- **[Force Reinstall]** — reinstalls everything, overwriting existing installations
- Live streaming output inline in the ConsoleDrawer during any install operation

### Flow: Pre-Launch Gate (auto-install off)

1. User clicks Launch on a profile that has missing required dependencies.
2. If `auto_install_prefix_deps` is off (default), launch is blocked.
3. A modal appears: "This profile requires prefix dependencies that are not installed."
   - Lists each missing verb with its human-readable label (e.g., "vcrun2019 — Visual C++ 2015-2019 Runtime")
   - Includes a slow-install warning inline if any listed verb is flagged `is_slow`
   - **[Install + Launch]** — this button IS the confirmation; clicking it starts the install, then launches on success
   - **[Skip and Launch]** — records `user_skipped` in DB for each skipped verb and proceeds with launch
   - **[Cancel]** — closes modal, returns to profile view
4. If `auto_install_prefix_deps` is on, CrossHook silently installs missing dependencies before launch without prompting. The `auto_install_prefix_deps` setting represents standing user consent and bypasses the per-install confirmation dialog.

The pre-launch gate modal IS the confirmation dialog for this path — a separate confirmation step is not needed because the modal already lists what will be installed before the user clicks [Install + Launch].

### Alternative Flow: Manual One-Off Package Install

1. User is in the Prefix Dependencies panel and wants to install a package not declared in the profile schema.
2. A collapsible "Add package" row at the bottom of the panel shows a text input and an **[Add]** button.
3. Input validates against a curated package allowlist (vcrun2019, dotnet48, d3dx9, corefonts, xact, etc.) with inline suggestions/autocomplete.
4. On submit, the package is added to the user-extras list and the same streaming console and status chip flow applies.

**Decision point**: Whether to allow arbitrary free-form package names is a security decision. See research-security.md — allowlist validation in Rust before exec is the recommended approach.

### Alternative Flow: Profile Does Not Declare Dependencies

1. User opens a profile with no `required_protontricks` field.
2. The Prefix Dependencies panel renders an empty-state message: "No dependency requirements declared for this profile."
3. The "Add package" row is still available for manual installs.

### Alternative Flow: Prefix Not Initialized

1. User has a prefix path configured but the prefix has not been initialized (first Wine run has not happened).
2. The Dependencies panel shows: "Run a launch first to initialize the prefix, then retry dependency installation."
3. No install actions are surfaced.

### Alternative Flow: Prefix Not Found / Not Configured

1. User has no prefix path set, or the configured path does not exist.
2. The Dependencies panel shows an `error` state: "Wine prefix not found. Configure the prefix path in Profile settings before installing dependencies."
3. No install actions are surfaced.

### Alternative Flow: protontricks Not Found / Not Configured

1. CrossHook checks for the protontricks binary (auto-detected or from `protontricks_binary_path` setting) on panel load.
2. If not found, the Dependencies panel shows a `not-configured` state: "protontricks is not installed or not configured. Install it via your package manager, then set the path in Settings."
3. A link opens the Settings panel to the `protontricks_binary_path` field.
4. All install actions are disabled. The panel remains visible and package status chips are shown (read-only).
5. Flatpak installs of protontricks require manual path configuration — auto-detection is not available for sandboxed binaries.

### Alternative Flow: DISPLAY Not Available

1. CrossHook detects `DISPLAY` / `WAYLAND_DISPLAY` is not set (e.g., headless environment).
2. The Dependencies panel shows: "Cannot install dependencies — no display environment available. protontricks requires a display to run."
3. All install actions are disabled.

### Alternative Flow: Package Status Inconclusive (Unknown)

1. CrossHook cannot definitively determine whether a package is installed (filesystem probe inconclusive, no DB record).
2. The package chip shows `unknown` state (amber).
3. An optional **[Force Reinstall]** action is available per package in this state.
4. The overall panel health indicator treats `unknown` as non-blocking (does not show the "Missing dependencies" banner for `unknown` packages alone).

---

## UI/UX Best Practices

### Dependency Status Chips

**Do not extend `HealthBadge.tsx`.** `HealthBadge` is typed against `HealthStatus = 'healthy' | 'stale' | 'broken'`, which is serialized from the Rust `HealthStatus` enum. Adding dependency states to it would mutate the health IPC contract.

Instead, create a new **`DependencyStatusBadge`** component that owns a parallel `DepStatus` type and reuses only the CSS classes (`crosshook-status-chip crosshook-compatibility-badge`) and the `STATUS_ICON` / `STATUS_LABEL` map pattern — no shared TypeScript types with `HealthBadge`.

```ts
type DepStatus =
  | 'unknown'
  | 'installed'
  | 'missing'
  | 'install_failed'
  | 'check_failed'
  | 'installing' // UI-only transient state
  | 'user_skipped'; // DB-only, not in backend DependencyState enum
```

The backend `DependencyState` enum defines five values (`unknown`, `installed`, `missing`, `install_failed`, `check_failed`). The UI layer adds two display-only states on top:

| State            | Source   | Visual Treatment                         | Accessible Label                             |
| ---------------- | -------- | ---------------------------------------- | -------------------------------------------- |
| `unknown`        | IPC / DB | Amber chip, question icon, "Unknown"     | "Package corefonts: Status unknown"          |
| `installed`      | IPC / DB | Green chip, checkmark icon, "Installed"  | "Package vcrun2019: Installed"               |
| `missing`        | IPC / DB | Red/error chip, dash icon, "Missing"     | "Package dotnet48: Missing"                  |
| `install_failed` | IPC / DB | Red/error chip, X icon, "Failed"         | "Package d3dx9: Installation failed"         |
| `check_failed`   | IPC      | Amber chip, warning icon, "Check failed" | "Package xact: Status check failed"          |
| `installing`     | UI only  | Muted chip with spinner, "Installing..." | "Package dotnet48: Installing, please wait"  |
| `user_skipped`   | DB only  | Gray/muted chip, dash icon, "Skipped"    | "Package xact: Installation skipped by user" |

`check_failed` (returned when a `check_prefix_dependencies` probe errors on a specific package) is distinct from `unknown` (no prior DB record). `check_failed` means a probe was attempted and errored; `unknown` means the panel loaded from `get_dependency_status` cache with no prior check. Both offer **[Force Reinstall]**.

`installing` is a transient frontend-only state, set when an `install_prefix_dependency` command is in-flight for that package and cleared on receiving the `prefix-dep-complete` event for that package.

`user_skipped` lives only in the CrossHook SQLite DB; it is not part of the backend `DependencyState` enum. It must remain visually distinct from both `installed` (user chose to skip — not a successful install) and `missing` (banner suppression is intentional for skipped packages).

Do not rely on color alone. Every chip includes both an icon and a text label, following the same `STATUS_ICON` + `STATUS_LABEL` map pattern as `HealthBadge.tsx` — just on its own type.

### Dependency List Layout

Render packages as a vertical list of rows, each containing:

- Package name (e.g., `vcrun2019`)
- A human-readable label (e.g., "Visual C++ 2019 Redistributable")
- The status chip
- An optional per-package **[Install]** or **[Retry]** button visible only when the package is in `missing` or `install_failed` state and no install is in progress
- An optional **[Force Reinstall]** action for `unknown` state packages

Group packages: profile-required packages first (labeled "Required by profile"), user-added extras below (labeled "Additional packages" with an add/remove control).

### Install Button States

| Condition                                   | Button state                                                                        |
| ------------------------------------------- | ----------------------------------------------------------------------------------- |
| All required packages installed             | No primary CTA; [Install All] / [Force Reinstall] in secondary style                |
| Some missing, no install running            | "Install Missing" — primary style                                                   |
| All missing but user previously skipped all | No banner; amber health indicator; [Install Missing] still in panel                 |
| Install in progress (any prefix)            | Disabled globally, spinner, "Installing..." label — lock is per-app not per-profile |
| protontricks not found / not configured     | Disabled, tooltip: "protontricks not configured — see Settings"                     |
| Prefix not configured                       | Disabled, tooltip: "Prefix path not set"                                            |
| Prefix not initialized                      | Disabled, tooltip: "Launch once to initialize the prefix"                           |
| DISPLAY not set                             | Disabled, tooltip: "No display environment available"                               |

**Concurrent install lock is global**: `install_prefix_dependency` returns `PrefixDepsError::AlreadyInstalling` immediately if any install is active. The install button must be disabled across all profiles and prefixes when any install is running — not just the current profile's panel. The frontend should track a global `isInstallingDeps` boolean (lifted to app-level context or a Tauri state subscription) to enforce this.

**Accessibility**: `aria-busy="true"` on the list section during active installs. The install button uses `aria-disabled` (not HTML `disabled` attribute) when blocked by the lock, so screen readers can still announce its label and tooltip.

### Settings Panel Requirements

Two fields must be added to the Settings panel for this feature:

1. **`protontricks_binary_path`**: Text input with a **[Browse...]** file picker. Below the field, a live validation indicator driven by `detect_protontricks_binary` shows one of:
   - "Binary found (protontricks)" — `source: "path"` or `"settings"`
   - "Binary found via Flatpak — manual path required" — `source: "flatpak"`
   - "Binary not found" — `source: "not_found"`
     Flatpak users must enter a full invocation path or wrapper script here — auto-detection is not reliable for sandboxed binaries.

2. **`auto_install_prefix_deps`** toggle: Default off. When on, CrossHook silently installs missing dependencies before launch. When off (default), the pre-launch gate modal is shown instead.

### Real-Time Console Output

The existing `ConsoleDrawer` / `ConsoleView` architecture is reused. The ConsoleDrawer listens on `prefix-dep-log` events in addition to `launch-log` and `update-log`:

- Auto-expands when the first `prefix-dep-log` line arrives
- Shows a line count badge in the collapsed tab
- ANSI stripping happens in Rust before emission — no frontend stripping needed
- Auto-scrolls to bottom while the user has not manually scrolled up (existing `shouldFollowRef` behavior)

### Indeterminate Progress Indication

For the overall install operation, use an indeterminate progress bar above the ConsoleDrawer when an install is running. Do not attempt to calculate exact percentage — protontricks does not emit structured progress events. The bar conveys "work is happening" not "N% complete."

```
[Indeterminate progress bar] — visible only during active install
```

A text status line below the bar: "Installing dotnet48 (this may take 10-20 minutes)..." for packages known to be slow.

### Accessibility

- Wrap the dependencies section in `<section aria-label="Prefix dependencies">`.
- Use `role="status"` on the overall install status text so it is announced as a live region without interrupting focus.
- Use `aria-live="polite"` on the package list so chip state changes are announced.
- The indeterminate progress bar: `<progress aria-label="Dependency installation in progress" />` (no value attribute means indeterminate).
- Confirm dialog (if showing before a destructive re-install): standard modal with focus trap.

---

## Error Handling

### Error States Table

All user-facing messages below are **templated strings** — CrossHook constructs them in Rust before emitting to the frontend. Raw subprocess output is never passed through to the UI layer.

| Error Condition                 | User-Facing Message (exact or close)                                                       | Raw Detail Destination               | Recovery Action                             |
| ------------------------------- | ------------------------------------------------------------------------------------------ | ------------------------------------ | ------------------------------------------- |
| protontricks binary missing     | "protontricks is not installed. Install via Flatpak or your package manager."              | CrossHook log only                   | Link to Settings `protontricks_binary_path` |
| SHA256 / checksum mismatch      | "Dependency download verification failed. Please update winetricks to the latest version." | CrossHook log only                   | None — user must update winetricks          |
| Network timeout                 | "Dependency download timed out. Check your internet connection and try again."             | CrossHook log only                   | Retry button                                |
| Network failure (generic)       | "Dependency download timed out. Check your internet connection and try again."             | wget/curl stderr to CrossHook log    | Retry button                                |
| Unknown verb (profile-declared) | "Profile specifies an unknown dependency. The profile may need to be updated."             | winetricks stderr to CrossHook log   | None — contact profile author               |
| Unknown verb (user-entered)     | "Unknown package name. Check spelling and try again."                                      | None — blocked by allowlist pre-exec | Inline validation on input                  |
| Install failed (generic)        | "Dependency installation failed. See CrossHook logs for details."                          | Full stderr to CrossHook log         | Console link, per-package [Retry]           |
| Install timeout (300s)          | "Dependency installation timed out after 5 minutes. The download may be stalled."          | CrossHook log only                   | Retry button                                |
| Running as root / elevated      | "protontricks cannot run with elevated privileges. CrossHook will use your user account."  | CrossHook log only                   | None — informational                        |
| Prefix path not found           | "Wine prefix not found. Set the prefix path in Profile settings."                          | CrossHook log only (no path shown)   | Navigate to Profile settings                |
| Prefix not initialized          | "Run a launch first to initialize the prefix, then retry."                                 | None                                 | None — informational                        |
| DISPLAY not set                 | "Cannot install dependencies — no display environment available."                          | Env check result to CrossHook log    | None                                        |
| Concurrent install attempt      | "An installation is already in progress for this prefix."                                  | None                                 | None (button disabled)                      |
| Prefix permission denied        | "Cannot write to prefix path. Check folder permissions."                                   | Errno to CrossHook log               | None                                        |
| Flatpak protontricks            | "Flatpak protontricks requires manual path configuration. See Settings."                   | None                                 | Link to Settings                            |

### Sensitive Information Policy

**CrossHook must never surface raw subprocess output in the UI.** This is a firm security requirement (S-11 from security-researcher), not a style preference.

Winetricks and protontricks stderr is particularly hazardous because it routinely contains:

- Full filesystem paths (home directory, Steam library paths, Wine prefix paths)
- Wine debug output including `WINEPREFIX` and `WINEDLLPATH` values
- Environment variables that Wine logs during initialization

The sanitization boundary is in the Rust command handler: the handler maps subprocess exit codes and known stderr patterns to templated error variants, then emits those variants to the frontend. The raw stderr goes to the CrossHook internal log file only, accessible via the ConsoleDrawer — not embedded in any UI error string.

Additional items that must never appear in UI-layer strings:

- System username
- Host OS version or kernel details
- Raw exception stack traces or Rust panic messages

### Explicit Confirmation Before Install (Security Requirement)

Before triggering any protontricks install — whether from the "Install Missing" button, the pre-launch gate, or the manual add flow — CrossHook must show the user exactly what will be installed and require explicit confirmation. This is both a security control (transparency about community profile actions) and a UX best practice.

**Confirmation dialog content**:

- Title: "Install prefix dependencies?"
- Body: list of verbs to be installed, each shown with its human-readable label if a verb→label mapping exists (e.g., "vcrun2019 — Visual C++ 2015-2019 Runtime", "dotnet48 — .NET Framework 4.8")
- If any verb is `unknown` / not in the allowlist, it must be flagged visually and the install must be blocked until resolved
- A slow-install warning inline if any verb is flagged `is_slow` in the manifest
- **[Confirm Install]** / **[Cancel]** buttons

This dialog applies to both the "Install Missing" panel action and the pre-launch gate [Install + Launch] path. The `auto_install_prefix_deps` setting bypasses this confirmation dialog — users who opt in to auto-install have given standing consent.

### Validation Patterns

- **Package name input (S-22 CRITICAL)**: The server-side `validate_protontricks_verbs()` gate in the IPC command handler is the security boundary — it fires regardless of input source (community profile TOML, manual UI entry, or any future call site). Client-side inline feedback (error chip on blur, disabled submit for unknown names) is a **UX affordance only**, not a security control. Autocomplete from the known-verb set is the strongest UX mitigation: a constrained dropdown eliminates free-form text entry from the UI entirely and makes the server-side gate redundant for this surface in practice.
- **Prefix path**: Validate existence and writability before enabling any install action. Show the result as a `ReadinessChecklist`-style check card.
- **Concurrent lock**: Enforced in Rust state; the UI disables buttons while the lock is held. No toast or dialog needed — the button state communicates the lock.

---

## Performance UX

### Loading State Sequence

The panel has two distinct load operations with different latency profiles:

**Initial load (fast — `get_dependency_status`)**: On profile open, call `get_dependency_status` — a synchronous SQLite read, effectively instant. This returns the last-checked `PackageDependencyState[]` with a `checked_at` timestamp. Render these results immediately with a "Checked X minutes ago" label. On first open with no cache, all states are `unknown`.

**Live check (slow — `check_prefix_dependencies`)**: Spawns processes; takes 3-30 seconds per package. Trigger this on explicit [Check Now] press, or automatically in the background after initial load if the cached result is stale (define staleness threshold, e.g. >30 minutes). While running:

- Show a `<div aria-busy="true">` spinner beside the section heading (matches `ReadinessChecklist`'s loading pattern).
- Keep the cached state chips visible (stale but informative — better than a blank panel).
- Update individual chips as each package result arrives if the IPC returns per-package progress; otherwise update all at once on completion.

**Binary detection (instant — `detect_protontricks_binary`)**: Call on panel mount and on Settings save. Returns `{ found, binary_path, binary_name, source }`. Use `source` to tailor the Settings guidance: `"flatpak"` source gets a note about manual path configuration; `"path"` or `"settings"` source shows "Binary found" confirmation.

### Streaming stdout/stderr

Install progress arrives via two Tauri events (not polling):

- **`prefix-dep-log`** — plain `string` — one ANSI-stripped line at a time from protontricks stdout/stderr. Emitted as a plain string (same shape as `launch-log` and `update-log`) so `ConsoleView` / `ConsoleDrawer` can listen with `listen<LogPayload>('prefix-dep-log', handler)` using the existing `normalizeLogMessage` utility unchanged. No new payload type, no adapter.
- **`prefix-dep-complete`** — `{ package: string, succeeded: bool, exit_code: number | null }` — final result per package. Consumed by the `DependencyRow` component (not `ConsoleView`) to transition the chip from `installing` to `installed` or `install_failed`.

**Separation of concerns**: the console stream carries human-readable text only. Structured per-package status data (which package completed, whether it succeeded) travels on `prefix-dep-complete` and is consumed by the dependency panel, not the console. Do not embed package context into the log stream.

**Adding `prefix-dep-log` to ConsoleDrawer is a one-liner** — add one `listen` call and one `unlisten` cleanup alongside the existing `launch-log` / `update-log` listeners. `LogPayload` already handles `string | { line: string } | { message: string } | { text: string }`; a plain-string emission satisfies it with no changes to `log.ts`.

**What the console will show**: winetricks stderr contains wget-style download progress lines such as:

```
0K .......... .......... .......... .......... .......... 0% 823K 40s
```

Do not attempt to parse these into a structured percentage — render them as-is. The indeterminate progress bar above the console handles the "something is happening" signal; the raw output provides detail for users who want it.

**ANSI stripping happens in Rust**: winetricks outputs terminal color codes. Strip before emitting via `prefix-dep-log`, not in the frontend.

Lines accumulate in state; no virtualization needed for typical sessions (typically under 2000 lines).

**Wine UI windows during install**: Some verbs (notably `dotnet48`) trigger Wine's own installer dialogs on the X11/Wayland display. This is expected. The console view shows a static note during active installs: "Wine may display installer windows during installation — this is normal behavior."

### Install Duration Expectations (for UI copy)

Based on api-researcher findings (confirmed against community reports):

| Package     | Typical Duration | Notes                                              |
| ----------- | ---------------- | -------------------------------------------------- |
| `vcrun2019` | 2-5 min          | ~50 MB download from Microsoft CDN                 |
| `dotnet48`  | 10-20 min        | Large download + complex Wine .NET install process |
| `d3dx9`     | 1-3 min          |                                                    |
| `corefonts` | 1-2 min          |                                                    |
| `xact`      | 1-2 min          |                                                    |

**Slow install warning trigger**: Show a prominent warning before starting any install that includes `dotnet48` (or other verbs flagged `is_slow` in the package manifest): "Installing .NET Framework 4.8 may take 10-20 minutes. Do not close CrossHook during installation — leaving partway through may require manual prefix repair."

**"Already installed" handling**: When running without `--force`, winetricks prints a notice and exits with code 0 if a package is already present. CrossHook should treat exit code 0 as success regardless; the console will show the skip notice. This is expected behavior, not an error.

### Cancellation

Mid-flight cancellation is **not safe** for winetricks: Wine processes left mid-install can corrupt the prefix, requiring a full wineprefix deletion and rebuild. This was confirmed by both api-researcher findings and community evidence.

For the initial implementation:

- Do **not** expose a Cancel button during active installs.
- Show the slow-install warning before starting so the user makes an informed decision.
- If the user force-quits CrossHook during install, the ConsoleDrawer line count and install lock will be in an inconsistent state on next launch. The Rust-side lock should be cleared on startup if no protontricks process is running.

Cancellation with safe prefix cleanup can be added in a future phase once the process management architecture is established.

---

## Competitive Analysis

### Bottles

**Approach**: Replaced winetricks entirely with their own dependency manifest system. Dependencies have their own manifests; installation is tracked in the bottle config file for reproducibility.

**What works well**:

- Dependencies are declared in environment templates (Software, Gaming, etc.) — user doesn't need to know package names
- Dependency status is tracked in the bottle config, not inferred from the filesystem at runtime
- Installation UI is integrated; no terminal window pops up

**What fails**:

- No real-time install feedback: users see the UI appear "stuck" with no progress; Bottles GitHub issue #2195 documents users waiting 20+ minutes with no indication of activity
- No progress bar during install — issue #2902 requested this and was unresolved as of research date
- The only way to see what's happening is to launch Bottles from a terminal

**Lesson for CrossHook**: Bottles' architectural decision (own manifest system) is the right long-term direction but requires significant infrastructure. The immediate lesson is simpler: **always stream output to a visible console and never leave the user staring at a static UI during a long operation**.

**Confidence**: High — GitHub issue tracker evidence.

### Lutris

**Approach**: Winetricks is accessible as an external tool from the game's context menu, but it is not integrated into the UI for per-game dependency management. Installation scripts declare dependencies as script steps.

**What works well**:

- Script-based dependency declaration means dependencies are reproducible across installs
- Power users can run winetricks from within Lutris via right-click

**What fails**:

- No UI for dependency status per-game; users must know to look in context menus
- Winetricks launches in a separate window (YAD/Zenity), breaking visual continuity
- GitHub issue #5486 documents users requesting built-in winecfg/winetricks access for months

**Lesson for CrossHook**: The right-click context menu approach fails discoverability. Dependency management should be a first-class section in the profile view, not a hidden context menu item.

**Confidence**: High — GitHub issue tracker evidence.

### Heroic Games Launcher

**Approach**: Winetricks is accessible from the bottom of a game's settings panel. Not deeply integrated; it launches as an external tool.

**What works well**:

- Accessible from per-game settings — closer to the profile context than Lutris
- UX improvement PR #4602 shows attention to clarity in wine settings UI

**What fails**:

- Dependency status is not displayed; users don't know what's installed
- External tool launch creates a separate window, breaking flow
- No profile-declared dependency support

**Lesson for CrossHook**: The "settings panel link to external tool" pattern is the baseline minimum. CrossHook should exceed this by providing inline status and integrated streaming output.

**Confidence**: Medium — GitHub issues and PR analysis; no direct UI screenshots available.

### Steam

**Approach**: Steam automatically downloads Proton and runtime dependencies. For game-specific Windows runtimes, Steam relies on the game's own installer (which runs inside Proton). Steam does not expose winetricks/protontricks.

**What works well**:

- Completely invisible to the user — auto-download with progress bar
- Progress shown in Steam library (downloading, installing)

**What CrossHook can adapt**:

- The model of "declare requirements, auto-detect missing, present a single install action" is the right mental model
- Progress bar during install (even indeterminate) sets appropriate expectations

**Confidence**: Medium — behavioral observation; Steam's internals are not documented.

---

## Recommendations

### Must Have

1. **`DependencyStatusBadge` component** with its own `DepStatus` type (`installed | missing | install_failed | check_failed | unknown | installing | user_skipped`), reusing `crosshook-status-chip` / `crosshook-compatibility-badge` CSS classes and the `STATUS_ICON` / `STATUS_LABEL` map pattern. Do **not** extend `HealthBadge` or the `HealthStatus` type — that is coupled to the Rust health IPC enum. Never show raw paths or subprocess output in UI error messages.
2. **ConsoleDrawer integration** for protontricks output via the canonical `prefix-dep-log` Tauri event channel. Auto-expand on first output line.
3. **Concurrent install lock** enforced in Rust state, communicated in the UI via button disabled state (not a dialog).
4. **"Missing dependencies" banner** on profile open when missing packages exist, with [Install] / [Skip] options. Skip persists `user_skipped` to SQLite.
5. **Explicit install confirmation dialog** before any protontricks invocation (security requirement S-11): lists verbs with human-readable labels, flags slow installs, requires [Confirm Install] / [Cancel]. For the panel [Install Missing] path, this is a dedicated dialog. For the pre-launch gate path, the gate modal itself serves as the confirmation.
6. **Pre-launch gate modal** when missing dependencies exist and `auto_install_prefix_deps` is off: lists what will be installed, then [Install + Launch] / [Skip and Launch] / [Cancel]. The modal IS the confirmation dialog for that path.
7. **`protontricks_binary_path` field in Settings** with browse picker and live validation indicator ("Binary found / not found").
8. **`auto_install_prefix_deps` toggle in Settings** defaulting to off. Enabling it grants standing consent and bypasses per-install confirmation dialogs.
9. **protontricks not found** renders as a degraded-but-visible panel state — not a hard block on opening CrossHook.
10. **Slow install warning** shown inside the confirmation dialog before the user confirms, for any verb flagged `is_slow`.
11. **Error messages are templated strings only** — raw winetricks/protontricks stderr (which contains filesystem paths, Wine debug output, and env vars) is written to the CrossHook log only, never embedded in UI-layer error strings.

### Should Have

11. **Indeterminate progress bar** during active install above the ConsoleDrawer.
12. **Human-readable package labels** alongside package IDs (e.g., "dotnet48 — .NET Framework 4.8").
13. **Per-package [Retry]** for `install_failed` packages without rerunning the full install batch.
14. **[Force Reinstall] action** for packages in `unknown` or `check_failed` state.
15. **Scan result caching** in the CrossHook metadata DB so the panel loads instantly on subsequent visits, with a [Check Now] manual rescan.
16. **Community profile import wizard** includes a "Required prefix dependencies" preview section.
17. **Autocomplete / constrained dropdown for manual package entry** (S-22): presenting only known verbs from the curated set as selectable options eliminates the free-form injection surface at the UI layer entirely, making the server-side `validate_protontricks_verbs()` gate redundant for manual entry in practice. Elevated from Nice to Have due to security finding S-22.

### Nice to Have

18. **ANSI code stripping** in console output — confirmed: happens in Rust before `prefix-dep-log` emission, not in the frontend.
19. **Cancellation support** (future phase — requires Rust-side prefix cleanup on abort).
20. **Dependency manifest templates** per game genre surfaced as quick-install groups.
21. **User-editable extras list** in the Prefix Dependencies panel with add/remove controls for packages not declared in the profile schema (only meaningful once autocomplete/constrained input is in place).

---

## Open Questions

1. **Filesystem scan vs. metadata DB** _(resolved by tech-designer + business-analyzer)_: `get_dependency_status` (fast SQLite read) drives panel display on load; `check_prefix_dependencies` (slow process spawn) is triggered by [Check Now] or background staleness check. The two-tier approach is confirmed. Staleness threshold to define (suggest 30 minutes as default).

2. **Package name allowlist scope**: The five initial packages (vcrun2019, dotnet48, d3dx9, corefonts, xact) are confirmed. Should the allowlist be hardcoded in Rust or loaded from a bundled manifest? A manifest allows adding labels, `is_slow` flags, and new packages without a CrossHook release. This is an architecture decision for the tech-designer but the UX benefit of a manifest (human-readable labels in the confirmation dialog) is significant.

3. **`user_skipped` reset UX**: When a user has skipped a package, how do they undo the skip? Needed: a way to transition `user_skipped` → `missing` without forcing a reinstall attempt. Options: per-package **[Mark as required]** context action, or a **[Reset skip decisions]** button in the panel footer.

4. **`check_prefix_dependencies` per-package streaming**: The tech spec says this call takes 3-30s per package and returns a full `states[]` array on completion. Does it emit any per-package events during the check, or only a final result? If only a final result, chips cannot update individually during a scan — all chips stay in their cached state until the entire scan completes, then update together. This affects whether to show per-chip scan spinners (only possible if per-package events exist) or a single section-level spinner (always safe).

5. **`auto_install_prefix_deps` acknowledgement**: Should enabling this setting require a one-time confirmation warning ("Auto-install will run protontricks commands from community profiles without prompting")? This closes the standing-consent loop. Awaiting security-researcher response.

6. **Pre-launch gate skip persistence**: When the user chooses "Skip and Launch", should this write `user_skipped` to the DB (persistent, same as panel skip) or be session-only (prompts again next launch)? The choice affects whether the amber health indicator clears after a single skip-and-launch.

---

## Sources

- [GitHub: Matoking/protontricks](https://github.com/Matoking/protontricks)
- [Bottles FAQ: Where is Winetricks?](https://docs.usebottles.com/faq/where-is-winetricks)
- [GitHub Bottles issue #2195: Give more feedback on dependency installing](https://github.com/bottlesdevs/Bottles/issues/2195)
- [GitHub Bottles issue #2902: Add progressbar to the dependency manager](https://github.com/bottlesdevs/Bottles/issues/2902)
- [GitHub Lutris issue #5486: Enable launching winecfg and winetricks for a prefix](https://github.com/lutris/lutris/issues/5486)
- [GitHub Heroic PR #4602: Improve "Use Default Wine Settings" tooltip](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/pull/4602)
- [Tauri v2: Calling the Frontend from Rust](https://v2.tauri.app/develop/calling-frontend/)
- [Tauri v2: Long-running backend async tasks](https://sneakycrow.dev/blog/2024-05-12-running-async-tasks-in-tauri-v2)
- [MDN: ARIA live regions](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Guides/Live_regions)
- [CWE-209: Generation of Error Message Containing Sensitive Information](https://cwe.mitre.org/data/definitions/209.html)
- [react-logviewer: React log streaming component](https://github.com/melloware/react-logviewer)
- [Smart Interface Design Patterns: Badges vs Chips vs Tags](https://smart-interface-design-patterns.com/articles/badges-chips-tags-pills/)
