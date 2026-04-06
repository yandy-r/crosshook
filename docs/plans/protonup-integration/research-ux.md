# UX Research: ProtonUp Integration

**Feature:** Proton Version Management (ProtonUp Integration)
**Researcher:** UX Patterns Specialist
**Date:** 2026-04-06

---

## Executive Summary

CrossHook currently surfaces a confusing dead end: when a profile references a Proton path that does not exist on disk, the user sees a path-not-found error with no remediation path. This research defines the UX design space for a first-class Proton version manager embedded in CrossHook.

The core user need is **guided resolution**: detect the problem, surface the fix, and execute it without leaving the app. Competitive analysis of ProtonUp-Qt, Heroic Games Launcher, Lutris, and Steam itself reveals a consistent pattern: successful tools combine a browsable, filterable version list; in-app download with real-time progress; per-game/per-profile version association; and graceful degradation when offline or when the ProtonUp binary is absent.

The recommended approach maps directly onto CrossHook's existing component vocabulary (badges, CollapsibleSection, ThemedSelect, status chips) and data infrastructure (SQLite `external_cache_entries` for TTL-cached version lists, TOML for preferred version, runtime filesystem scan for installed versions, `community_profiles.proton_version` for required-version hints). CrossHook uses a built-in library for installs — no user-installed ProtonUp binary is required.

**Top priorities:**

1. Inline install-suggestion banner on profile launch failure
2. Dedicated Proton Manager panel with search/filter and install affordance
3. Real-time download progress via Tauri event streaming
4. Offline fallback with cache-age indicator following the `CommunityBrowser` cached-data banner pattern

---

## User Workflows

### Primary Flow A — Profile Launch with Missing Proton Version

```
User opens profile → Clicks "Launch"
  → Readiness check detects proton_path not found on disk
  → [NEW] Error panel shows:
      "Proton version '<name>' is not installed."
      + "community_profiles.proton_version = GE-Proton9-15" (if known)
      + [Install GE-Proton9-15] button  (one-click to Proton Manager)
      + [Open Proton Manager] fallback link
  → User clicks [Install GE-Proton9-15]
  → Proton Manager opens with that version pre-selected/highlighted
  → Download progress shows inline
  → On completion: success banner; profile launch proceeds
```

**Decision points that map to business rules:**

- Is the required version known from `community_profiles.proton_version`? → Pre-select it
- Is ProtonUp binary present? → Show install guidance if absent
- Is the user offline? → Show cached list with staleness indicator; disable Install button with tooltip

---

### Primary Flow B — Proton Manager Browsing and Installation

```
User opens Settings → "Proton Versions" tab (or sidebar nav item)
  → Version list loads (skeleton rows while fetching)
  → Installed versions shown first, with [Installed] chip
  → Available versions listed below, grouped or interleaved by date
  → User types in search box → list filters live (debounced ~200ms)
  → User selects sort: "Latest first" (default) | "Oldest first" | "A–Z"
  → User clicks [Install] on a version row
  → Row transitions to progress state: progress bar + "Downloading 45% (12.3 MB/s, ~8s)"
  → On completion: row shows [Installed] chip; [Install] replaced with [Delete]
  → Toast notification: "GE-Proton9-20 installed successfully"
```

---

### Primary Flow C — Auto-Suggestion from Community Profile Import

```
User imports community profile with proton_version = "GE-Proton9-15"
  → Import wizard (CommunityImportWizardModal) shows:
      [NEW] "Required Proton version: GE-Proton9-15"
      + Status: [Not Installed] chip  or  [Installed] chip
      + If not installed: "Install now?" with checkbox (default checked)
  → On import confirm: triggers background install if checkbox checked
  → Import wizard closes; background toast shows install progress
```

---

### Alternative Flow — Delete / Manage Installed Versions

```
User in Proton Manager → Installed section
  → Each installed version shows:
      Version name, install date, disk size
      [Delete] button
  → Click [Delete] → Confirmation modal:
      "Delete GE-Proton9-15? This will remove it from ~/steam/compatibilitytools.d/."
      [Cancel] / [Delete]
  → On confirm: row removed; success notification
```

---

### Alternative Flow — ProtonUp Binary Not Found

```
User opens Proton Manager
  → Banner at top:
      "ProtonUp-rs is not installed. CrossHook uses it to download Proton versions."
      [How to install ProtonUp-rs] link → opens external documentation
  → Version list still shows installed versions (filesystem scan)
  → All [Install] buttons disabled with tooltip: "Requires ProtonUp-rs"
```

---

### Alternative Flow — GitHub API Rate Limited

```
User opens Proton Manager (unauthenticated, many requests today)
  → Fetch fails with HTTP 403 / rate-limit message
  → Banner: "GitHub API rate limit reached. Showing cached version list."
           "Cache is from [timestamp]. Refresh will retry in ~X minutes."
  → Cached list shown; [Install] buttons remain active on available versions
  → If no cache: "No cached data available. Connect to the internet or configure a GitHub token."
```

---

## UI/UX Best Practices

### Version List Design

**Installed vs. Available status signals:**

- Installed versions: row-level [Installed] chip using `crosshook-status-chip` class family (green, consistent with HealthBadge / OfflineStatusBadge patterns already in codebase)
- Available: no chip (absence is meaningful) or muted "Available" indicator
- In-progress download: animated progress bar row replaces install button
- Row ordering: installed versions float to top within their type group; within each group sort by release date descending (latest first as default)

**Search and filter:**

- Single text input, live filtering on version name and release date (debounced 200ms) — mirrors CommunityBrowser `matchesQuery` pattern
- Sort dropdown: "Latest first" (default) | "Oldest first" | "A–Z" — use `ThemedSelect` component
- Optional type filter chip row: [All] [GE-Proton] [Wine-GE] — if multiple version families are supported
- Show result count: "X of Y versions" in section header — matches CollapsibleSection `meta` prop pattern

**Empty states:**

- No versions installed: "No Proton versions installed. Install a version to launch Windows games."
- No search matches: "No versions matched '[query]'. Try a different search term."
- List not yet loaded: 5–8 skeleton rows (height matches real rows) — prevents layout shift

---

### Download Progress UI

**Progress bar requirements:**

- Determinate progress bar (0–100%) once content-length is known
- Show: filename being downloaded, percentage, download speed (e.g. "12.3 MB/s"), estimated time remaining
- Cancel button labeled "Cancel" (returns to previous state, no side effects)
- If content-length unknown: indeterminate bar + "Downloading…" label
- On completion: brief "Installed" success state (1.5s animation), then transition to installed-row state
- On failure: error state inline with "Retry" button — do not dismiss automatically

**Implementation note for tech designer:** Progress events should stream from the Rust backend via `tauri::Window::emit` (or `AppHandle::emit_to`) on a named event channel (e.g., `proton_download_progress`). The React hook listens with `listen()` from `@tauri-apps/api/event`. This avoids polling and gives sub-second update latency.

**Background download support:**

- Once download starts, user can navigate away from Proton Manager
- A persistent progress indicator (bottom status bar, mini badge, or sidebar dot) shows active download count
- Clicking it returns to Proton Manager and the relevant row

---

### Layout and Navigation

**Placement options (for tech designer consideration):**

| Option                               | Pros                                 | Cons                              |
| ------------------------------------ | ------------------------------------ | --------------------------------- |
| Settings sub-tab ("Proton Versions") | Logical grouping, low nav complexity | Hidden, not discoverable on error |
| Dedicated sidebar item               | High visibility, direct access       | Adds nav weight if rarely used    |
| Inline in profile launch error       | Contextual, zero friction            | Only surfaces on failure          |
| Settings + inline banner             | Both contextual and direct           | Requires two integration points   |

**Recommendation:** Settings sub-tab as the home for the manager + inline banner in the launch-error flow. The banner is high-leverage for discoverability without adding sidebar clutter.

**Within the Proton Manager panel:**

- Use `CollapsibleSection` for "Installed Versions" and "Available Versions" sections — consistent with existing panel pattern (LaunchPanel, CommunityBrowser)
- Toolbar: search input + sort select + optional refresh button (right-aligned)
- Cache-age banner when serving stale data — matches `crosshook-community-browser__cache-banner` pattern

---

### Inline Error Remediation (Profile Launch)

When a profile's Proton path is not found:

```
[!] Proton path not found
    The path "/home/user/.steam/steam/steamapps/common/Proton - GE-Proton9-15/proton" does not exist.

    [Suggested fix] Install GE-Proton9-15       [Open Proton Manager]

    Note: Profile launch is not blocked. Fix this path to enable launch.
```

- Use existing `crosshook-launch-panel__feedback-*` class family for consistent error presentation
- "Note: Profile launch is not blocked" — matches the requirement to never block profile launch
- `remediation` field pattern already exists in `OfflineReadinessPanel` — reuse it

---

### Accessibility

- All list rows keyboard-navigable (Tab to focus row; Enter to trigger primary action)
- [Install] and [Delete] buttons must have descriptive `aria-label` including version name: `aria-label="Install GE-Proton9-20"`
- Progress bar: `role="progressbar"` with `aria-valuenow`, `aria-valuemin`, `aria-valuemax`, and `aria-label`
- Status chips follow existing `aria-label` pattern from `OfflineStatusBadge`
- Respect `prefers-reduced-motion` — disable skeleton shimmer and progress bar animation when set
- Offline / cached-data banners use `role="status"` and `aria-live="polite"` — consistent with CommunityBrowser cache banner

---

## Error Handling

### Error States Table

| Error Condition                    | User-Visible Message                                                           | Actions Available             | Severity |
| ---------------------------------- | ------------------------------------------------------------------------------ | ----------------------------- | -------- |
| Proton path not found on launch    | "Proton path not found. [version] is not installed."                           | Install version, Open Manager | Warning  |
| ProtonUp binary not found          | "ProtonUp-rs is not installed. [How to install]"                               | External link                 | Warning  |
| Network failure during download    | "Download failed: [error]. Check your connection."                             | Retry, Cancel                 | Error    |
| Disk space insufficient            | "Not enough disk space. Need ~X GB, have Y GB free."                           | Cancel                        | Error    |
| Extraction/integrity failure       | "Installation failed: file may be corrupt. Try again."                         | Retry (re-download), Cancel   | Error    |
| GitHub API rate limited            | "Rate limit reached. Showing cached list from [date]."                         | Wait / retry later            | Info     |
| GitHub API rate limited (no cache) | "Rate limit reached. No cached data. Retry later or configure a GitHub token." | Open settings for token       | Warning  |
| Offline, cached data available     | "Offline. Showing cached version list from [date]."                            | None (informational)          | Info     |
| Offline, no cached data            | "Offline and no cached data available."                                        | None                          | Warning  |
| Version list fetch timeout         | "Could not load version list. [Retry]"                                         | Retry button                  | Warning  |
| Download cancelled by user         | Silent row-reset to pre-download state                                         | Install button restored       | None     |

---

### Validation Patterns

- **Disk space check** before initiating download: fetch available bytes from system, compare to version's known size, warn if margin < 20%
- **Path validation** after installation: verify the expected binary path exists before marking "Installed"
- **GitHub token format**: if user-supplied, validate format client-side with clear inline error before saving to settings

---

## Performance UX

### Loading States

| State                     | UI Pattern                                        | Duration Target                  |
| ------------------------- | ------------------------------------------------- | -------------------------------- |
| Initial version list load | 5–8 skeleton rows matching real row height        | < 500ms with cache; < 2s network |
| Search filter             | Debounced 200ms, no skeleton (instant DOM filter) | Immediate                        |
| Sort change               | Instant re-sort in-memory, no skeleton            | Immediate                        |
| Refresh / re-fetch        | Existing rows remain visible; spinner in toolbar  | < 2s                             |
| Download progress         | Determinate progress bar, speed, ETA              | Real-time streaming              |
| Post-install verification | Short indeterminate spinner on the row            | < 1s                             |

**Implementation guidance:**

- Version list data is cached in SQLite `external_cache_entries` with TTL. On open: serve cache immediately (zero perceived load time), then revalidate in background if TTL expired. On revalidation complete: update list without disrupting user's scroll position or active operations.
- Stale-while-revalidate: show "Refreshing…" label in toolbar during background fetch, not a full skeleton reload.

---

### Optimistic UI

- When a version is being installed: immediately show it as "Installing" in both Installed and Available sections
- Do not optimistically show "Installed" until the backend confirms path verification — failures should not leave ghost entries
- Install button disables immediately on click to prevent double-submission

---

### Offline Behavior

- Offline detection: rely on network request failure, not OS-level connectivity check (more reliable in VPN/partial connectivity scenarios)
- Cache fallback: serve `external_cache_entries` data when fetch fails
- Cache age indicator: show "Last updated: [human-relative time, e.g. '3 days ago']" in the version list header
- Install buttons: remain active when versions are cached (user may have the binary locally and just needs the path resolved)
- Disable install for versions whose download URL is not cached (no URL = no way to install offline)

---

## Competitive Analysis

### ProtonUp-Qt

**Source:** [GitHub](https://github.com/DavidoTek/ProtonUp-Qt) | [Homepage](https://davidotek.github.io/protonup-qt/)

**What it does well:**

- Simple, focused UI: one list of versions, one install button per row
- Supports multiple launchers (Steam, Lutris) with launcher selector at top
- Optimized for Steam Deck with gamepad navigation
- Keyboard shortcuts documented; i18n via Weblate
- GitHub/GitLab token configuration for API rate limit mitigation

**What it lacks / what CrossHook can improve on:**

- No inline contextual suggestion when a game fails to launch
- No integration with per-profile version tracking
- No auto-suggest based on community profile requirements
- No offline indicator or cache-age display

**Confidence:** High — directly inspected GitHub repository and wiki

---

### Heroic Games Launcher

**Source:** [Wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/How-To:-Wine-and-Proton) | [GamingOnLinux v2.18 review](https://www.gamingonlinux.com/2025/07/heroic-games-launcher-2-18-adds-ge-proton-prioritisation-improved-ui-navigation-and-new-analytics/)

**What it does well:**

- Wine Manager with per-game version selection — directly analogous to CrossHook's per-profile Proton path
- GE-Proton versions prioritized by default (v2.18+, configurable)
- In-app download of Wine/Proton versions via `heroic-wine-downloader`
- Clear custom path support (user can point to any binary)
- Controversy lesson: hiding non-GE versions caused confusion → **revert was necessary**. CrossHook should show all version types by default, with filter controls rather than hiding.

**What CrossHook can improve on:**

- Heroic's Wine Manager is a separate screen with no contextual trigger from game failures
- No cache-age display on the version list
- No auto-suggest from community-profile metadata

**Confidence:** High

---

### Lutris

**Source:** [GitHub](https://github.com/lutris/lutris) | [Linux Magazine guide](https://www.linux-magazine.com/Issues/2019/228/Lutris)

**What it does well:**

- Runner management via File > Manage Runners — clear mental model ("runners run games")
- CLI support: `--list-runners`, `--install-runner`, `--list-wine-versions` — power user path
- Per-game runner selection with override at individual game level
- Hierarchical config: system → runner → game level overrides

**What it lacks:**

- UX for Manage Runners is buried in menus (not discoverable on failure)
- No install-from-error-state flow

**What to adopt:** The concept of clear version hierarchy (default preferred version in settings + per-profile override) maps well to CrossHook's TOML preferred version + per-profile `proton_path` architecture.

**Confidence:** High

---

### Steam

**Source:** [Steam Play Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=1974055703) | [ProtonPlus coverage](https://www.gamingonlinux.com/2025/07/protonplus-makes-managing-proton-versions-on-linux-steamos-and-steam-deck-simple/)

**What it does well:**

- Per-game "Force compatibility tool" dropdown directly in game properties
- Versions auto-appear in dropdown after installation (no restart required in modern Steam)
- ProtonPlus third-party tool now shows all Steam games with their active Proton version — a feature CrossHook's health dashboard could approximate

**What it lacks:**

- No in-app download of community Proton versions (requires third-party tools)
- Dropdown has no search; becomes unwieldy with many versions installed

**What to adopt:**

- Show currently active Proton version on each profile card/row (equivalent of Steam's per-game compatibility tool display)
- Version dropdown in profile should update immediately when a new version is installed

**Confidence:** High

---

## Recommendations

### Must Have

1. **Inline install-suggestion on Proton path error** — contextual `remediation` field in launch panel error row with "Install [version]" action button (leverages existing `OfflineReadinessPanel` remediation pattern)

2. **Proton Manager panel** with:
   - Installed versions section (floating to top, with [Installed] chips and [Delete] action)
   - Available versions section with search input + sort dropdown
   - Skeleton loading rows on initial fetch
   - Cache-age banner when serving stale data (mirrors `crosshook-community-browser__cache-banner`)

3. **Real-time download progress** — Tauri event streaming from Rust backend, not polling; determinate progress bar with speed + ETA; Cancel button

4. **ProtonUp binary not-found guidance** — top-of-panel banner with external documentation link; installed-versions list still operational

5. **Offline fallback with cache-age indicator** — serve SQLite cache on network failure; show "Offline — cached list from [human timestamp]"; install buttons remain active for versions with cached download URLs

---

### Should Have

6. **Auto-suggest in community profile import wizard** — detect `proton_version` requirement in import preview; show [Not Installed] / [Installed] chip; optional "Install on import" checkbox

7. **Background download persistence** — allow navigating away from Proton Manager during download; mini badge or status bar indicator with active download count

8. **GitHub API token setting** — in Settings panel, optional GitHub token field to raise rate limit from 60 to 5000 req/hr (follows ProtonUp-Qt precedent; store in TOML settings, never in DB or logs)

9. **Default preferred Proton version** in Settings — TOML-persisted, used as fallback for new profiles without an explicit proton_path; shown as "(default)" in profile dropdown

10. **Per-profile Proton version quick-change** — in profile form, ProtonPathField should list installed versions from filesystem scan first; newly installed versions appear without form reload (reactive hook)

---

### Nice to Have

11. **Disk space pre-check** before download — warn if < 20% margin; surface the warning but do not block the install

12. **Install from import** — "Install all required Proton versions" action in the community import batch flow

13. **Version notes / changelog preview** — on hover or expand, show GitHub release notes excerpt for the selected version (already cached in `external_cache_entries`)

14. **Keyboard shortcut** — Ctrl+P or similar to open Proton Manager from anywhere in the app

---

## API-to-UX Binding

Technical constraints from the architecture that directly shape UI decisions (sourced from `research-tech.md`):

---

### Version List: Wrapper Struct Recommendation

**Answer to tech-designer's open question:** Yes, include `fetched_at` and `is_stale` in a wrapper struct. The UI needs both to render the cache-age banner correctly without a second IPC call.

Required wrapper shape:

```
{
  versions: AvailableProtonVersion[],
  fetched_at: string | null,   // ISO 8601 — null when no cache exists yet
  is_stale: boolean,           // true when TTL has expired
  is_offline: boolean          // true when network fetch failed, serving cache
}
```

UI behavior mapping:

- `is_offline: true, fetched_at: non-null` → show cache-age banner ("Offline — cached list from [relative time]")
- `is_offline: false, is_stale: true` → show background refresh spinner in toolbar; banner only if refresh subsequently fails
- `is_offline: true, fetched_at: null` → show empty state with "Offline and no cached data" message
- `is_offline: false, is_stale: false` → show list normally, no banner

---

### Install Progress Event Mapping

`proton-install-progress` event phases map to UI states:

| Phase         | Progress bar           | Label                           | Cancel button               |
| ------------- | ---------------------- | ------------------------------- | --------------------------- |
| `downloading` | Determinate (percent)  | "Downloading… X% (Y MB/s, ~Zs)" | Visible — "Cancel"          |
| `verifying`   | Indeterminate pulse    | "Verifying checksum…"           | Hidden (non-cancellable)    |
| `extracting`  | Indeterminate pulse    | "Extracting…"                   | Hidden (non-cancellable)    |
| `complete`    | Full (100%), fades out | "Installed" chip replaces bar   | Hidden                      |
| `error`       | Red/error state        | Error message + [Retry]         | Hidden; [Retry] replaces it |

Throttling to ~1 event per 0.5% means at most 200 events for a typical install — smooth enough for a progress bar without client-side throttling.

No resume support: if the user cancels, the row returns fully to pre-install state. [Install] button restores immediately on cancel.

---

### Installed Versions: No Skeleton Needed

`get_installed_proton_versions` is a synchronous filesystem scan with no network. Call on mount — no loading skeleton required for the "Installed Versions" section. Only the "Available Versions" section needs skeleton rows while the async IPC call resolves.

---

### ProtonUp Binary: Flow Does Not Apply

The architecture uses the built-in library (not a user-installed ProtonUp binary). The "ProtonUp binary not found" error flow from the initial research section should be replaced with:

- If filesystem scan returns empty and no path is configured: show an informational empty state at the top of the panel: "No Proton installations found. Install a version below to get started."
- This is a neutral empty state, not an error banner. The install flow is always available.

---

### Tool Type Defaulting

Two families: `GEProton` (Steam) and `WineGE` (Lutris). CrossHook's workflow is Steam-focused.

UI default: type filter dropdown defaults to "GE-Proton" on first open. User can switch to "All" or "Wine-GE". Persist last-used filter in component state only (not TOML — not worth a settings write for a display preference).

The Heroic anti-pattern lesson still applies: do not remove Wine-GE from the dataset. Show it behind the filter, always available.

---

### Size Display Before Install

At 300–600 MB per version, disk size is material. Display `size_bytes` formatted (e.g. "412 MB") on each available version row so the user sees it before clicking [Install]. No confirmation modal — the size on the row is the disclosure. Only surface a warning if a pre-install disk space check shows insufficient margin.

---

### `suggest_proton_version_for_profile` — UX Surfaces

This command returns whether the community profile's required version is installed and the closest available version if not.

UX implications:

- **`CommunityBrowser` profile cards**: when rendering `proton_version`, call this command. If not installed, show [Not Installed] chip + closest available as tooltip/subtext: "GE-Proton9-27 not installed — closest available: GE-Proton9-26"
- **`CommunityImportWizardModal`**: use the result to populate the "Install after import" callout with the exact version name and its formatted size
- **`ProtonPathField` remediation**: when the saved path no longer exists, the suggestion from this command pre-populates the "Suggested fix" text

---

## Decision Point Resolutions

Answers to the specific UX decision points raised by business analysis (cross-referenced with `research-business.md`):

---

### Where does the Proton Manager live?

**Recommendation: Settings panel section + contextual inline entry points.**

`SettingsPanel.tsx` already uses `CollapsibleSection` for logically grouped feature areas (launchers, prefix storage, diagnostics). A "Proton Versions" `CollapsibleSection` in Settings is the natural home — consistent with the app's existing settings architecture and consistent with how `default_proton_path` already lives there.

Do not make it a dedicated sidebar page. The sidebar is for primary workflows (Profiles, Library, Community, Launch). Version management is a support task, not a primary task.

Contextual entry points (non-exclusive, all open the Settings Proton Versions section):

- Inline button in the `ProtonPathField` dropdown when no installs are detected: "No installs found — Manage Proton Versions"
- Remediation link in `HealthDashboardPage` and `OfflineReadinessPanel` error rows: "Open Proton Manager"
- Banner in `CommunityBrowser` profile cards when `proton_version` is not installed

---

### How are installed vs. available versions differentiated visually?

**Recommendation: Two `CollapsibleSection` sub-sections within the Proton Versions panel.**

- "Installed" section: always rendered first, even if empty; each row has a green `crosshook-status-chip` labeled "Installed" and a [Delete] action button.
- "Available" section: versions from cache/network not present on disk; each row has an [Install] button.

This two-section approach is clearer than a single list with status columns or a tag approach for a small-count list (typically < 30 versions visible). It matches how `CommunityBrowser` separates logical groups using `CollapsibleSection`.

Do not use a single interleaved list with status tags — this pattern works for large registries (npm, VS Code Extensions) but adds cognitive load for a short, action-oriented list where the user's primary decision is "do I need to install something."

---

### Progress display: inline progress bar, toast, or modal?

**Recommendation: Inline progress bar on the version row + optional background indicator.**

- When [Install] is clicked, the version row transitions in-place: button disappears, replaced by a determinate `<progress>` bar with percentage, speed, and ETA text below it. Cancel button appears to the right.
- This is the ProtonUp-Qt and Heroic pattern — the user sees exactly which version is downloading without a modal stealing focus.
- Toast on completion only (auto-dismiss after 4s): "GE-Proton9-20 installed successfully." Use `role="status"` `aria-live="polite"`.
- No modal for progress — modals are for decisions, not for observing background work.
- If user navigates away: a mini badge or count indicator in the Settings sidebar link or a bottom status strip shows "1 download in progress." Clicking returns to the Proton Versions section. Download state must live in a React context (not local component state) for this to work.

---

### Confirmation when installing a version already installed on disk

**Recommendation: No confirmation modal — show an informational inline note instead.**

Show a muted note under the row's install button: "Already found at [path]. Reinstall?" with the [Install] button relabeled "Reinstall." This avoids a modal interrupt for an edge case that is not destructive. The user explicitly chose to reinstall; no confirmation gate is needed.

If the filesystem scan shows an install that is not in the "available" API list (i.e., a manually installed custom version), it appears in the Installed section only with a "Custom" chip and no [Delete] button (CrossHook should not delete versions it did not install).

---

### One-at-a-time install constraint: how to communicate it

**Recommendation: Disable all other [Install] buttons with a tooltip while one download is active.**

- When a download is in progress: all other [Install] buttons get `disabled` attribute + `title="Another install is in progress. Wait for it to complete."`.
- The active download row shows the progress bar and a [Cancel] button.
- Do not queue installs silently — the user should explicitly choose what to install next.
- This is the ProtonUp-Qt pattern and simplest to implement correctly.

---

### The community profile suggestion badge: prominence and blocking

**Recommendation: Non-blocking annotation chip; visible but not alarming.**

In `CommunityBrowser` profile cards, where `proton_version` is currently rendered as muted text ("Proton: GE-Proton9-27"), augment this with a `crosshook-status-chip` in warning color when that version is not installed:

```
Proton: GE-Proton9-27  [Not Installed]
```

The chip uses the existing chip vocabulary (consistent with HealthBadge, OfflineStatusBadge). Clicking the chip navigates to the Proton Manager with that version pre-highlighted. The [Import] button on the card is never disabled because of a missing Proton version — that would violate the "never block profile launch/import" requirement.

In the `CommunityImportWizardModal`, show a more prominent inline callout (not a chip) when the required version is not installed:

```
Required Proton version: GE-Proton9-27
[!] Not installed on this system.
    [Install GE-Proton9-27 after import]  (checkbox, default: checked)
```

This is discoverable, non-blocking, and gives the user explicit agency.

---

### Integration with existing components

| Existing component                       | Integration approach                                                                                                                                                                                               |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `ProtonPathField.tsx`                    | Add "No installs detected? Manage Proton Versions →" link below dropdown when `installs` array is empty. After install completes, re-trigger the installs scan hook so the dropdown populates without form reload. |
| `CommunityBrowser.tsx`                   | Add [Not Installed] chip next to `proton_version` text when version not in filesystem scan. Chip navigates to Proton Manager.                                                                                      |
| `HealthDashboardPage.tsx`                | Existing `remediation` text on broken Proton path issues gets a clickable "Open Proton Manager" action link appended.                                                                                              |
| `SettingsPage.tsx` / `SettingsPanel.tsx` | Add "Proton Versions" `CollapsibleSection` after the existing Proton path field. Renders the full `ProtonManagerPanel` component.                                                                                  |

---

## Open Questions

1. **Navigation placement:** Should the Proton Manager live as a Settings sub-tab or as a dedicated sidebar navigation item? This depends on expected usage frequency; if many users will manage Proton versions regularly, sidebar elevation is warranted.

2. **Version type scope:** Initial implementation covers GE-Proton. Should Wine-GE and other families (e.g., Proton-CachyOS, Proton-Sarek) be in scope for v1? The Heroic controversy around hiding non-GE versions suggests defaulting to show all families with a filter, not a curated subset.

3. **Proton install location:** Does CrossHook always install to `~/.steam/root/compatibilitytools.d/` or allow user-configurable install path? This affects the filesystem scan and the path inserted into TOML settings.

4. **Rate limit token storage:** GitHub PATs must never be stored in SQLite or logs. TOML settings with OS keychain integration is the right pattern — clarify with security researcher before implementation.

5. **Install verification:** After extraction, how should CrossHook verify a successful install? Check for the `proton` binary at expected path? Hash check? This is both a UX question (what does "success" mean to the user) and a reliability question.

6. **Concurrent install limit:** Should the UI allow multiple parallel downloads? ProtonUp-Qt allows one at a time. Given disk I/O and bandwidth constraints, serializing is simpler and less error-prone for v1.

7. **Deletion of in-use version:** If a profile currently uses a Proton version, should deletion be blocked or warned? A profile health check after deletion could surface this, but a pre-delete warning is friendlier.

---

## Sources

- [ProtonUp-Qt GitHub](https://github.com/DavidoTek/ProtonUp-Qt)
- [ProtonUp-Qt Homepage](https://davidotek.github.io/protonup-qt/)
- [ProtonUp-Qt v2.9.1 Release Notes — GamingOnLinux](https://www.gamingonlinux.com/2024/01/protonup-qt-v291-released-for-easy-compatibility-tool-installs-on-linux-steam-deck/)
- [Heroic Games Launcher — How To: Wine and Proton](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/How-To:-Wine-and-Proton/e9d35369a9a9376c109f09f36827e854e1870e9e)
- [Heroic v2.18 GE-Proton Prioritisation — GamingOnLinux](https://www.gamingonlinux.com/2025/07/heroic-games-launcher-2-18-adds-ge-proton-prioritisation-improved-ui-navigation-and-new-analytics/)
- [Heroic reverts non-GE hiding — gHacks](https://www.ghacks.net/2025/08/05/heroic-games-launcher-reverts-a-change-that-hid-non-proton-ge-versions-by-default/)
- [heroic-wine-downloader GitHub](https://github.com/Heroic-Games-Launcher/heroic-wine-downloader)
- [Lutris GitHub](https://github.com/lutris/lutris)
- [Lutris FAQ](https://lutris.net/faq)
- [ProtonPlus — GamingOnLinux](https://www.gamingonlinux.com/2025/07/protonplus-makes-managing-proton-versions-on-linux-steamos-and-steam-deck-simple/)
- [Select Compatibility Tools Per Game in Steam](https://pulsegeek.com/articles/select-compatibility-tools-per-game-in-steam/)
- [Desktop UX: Software Installer Best Practices — Medium](https://medium.com/@renfei1992/desktop-ux-software-installer-best-practices-6d6d7383dc98)
- [UX best practices for on demand delivery — Android Developers](https://developer.android.com/guide/playcore/feature-delivery/ux-guidelines)
- [Offline UX Design Guidelines — web.dev](https://web.dev/articles/offline-ux-design-guidelines)
- [Offline-First Architecture — Medium](https://medium.com/@jusuftopic/offline-first-architecture-designing-for-reality-not-just-the-cloud-e5fd18e50a79)
- [Real-Time UI Updates with SSE — CodingWithMuhib](https://www.codingwithmuhib.com/blogs/real-time-ui-updates-with-sse-simpler-than-websockets)
- [Handling React Loading States — LogRocket](https://blog.logrocket.com/handling-react-loading-states-react-loading-skeleton/)
- [WCAG 2.2 — W3C](https://www.w3.org/TR/WCAG22/)
- [Keyboard Accessibility — WebAIM](https://webaim.org/techniques/keyboard/)
- [WAI-ARIA 1.3 — W3C](https://w3c.github.io/aria/)
