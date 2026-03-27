# SQLite3 Addition - UX Research

## Executive Summary

SQLite is a backend infrastructure feature, but users will feel it through more stable identity, richer history, faster catalog browsing, and clearer drift/health explanations. The UX risk is invisible authority confusion: if CrossHook starts showing favorites, collections, history, and launcher relationships that survive renames, users need confidence that the app still respects the files they can see and edit. The best UX framing is "CrossHook remembers context locally" rather than "CrossHook moved your data into a database."

This feature is fundamentally a **local-first data layer**: SQLite acts as a read-through cache and stable projection layer over TOML/filesystem truth, not a replacement. Every UX decision should reinforce that framing.

---

## Competitive Analysis

### How Game Launchers Handle Profile Identity and History

**Steam**
Steam anchors every game to a numeric `AppID` that is permanent and never changes, regardless of what the user renames in their library, where the install lives, or which account owns it. Metadata (playtime, screenshots, achievements) is tied to `AppID`, not install path or display name. Steam's library refresh is silent and background; "checking for updates" spinners appear only for network operations, never for local metadata reads. When a game installation is missing or corrupted, Steam shows an inline status chip ("Not installed" / "Needs repair") directly on the library card without blocking the rest of the UI.

**Heroic Games Launcher**
Heroic stores per-game configuration in a `GameConfig/` directory, with a global `config.json` for runner/prefix settings. Game identity is tied to the GOG/Epic store's own game ID, so display name changes are cosmetic. A critical lesson from Heroic's 2.7.0 release: its offline mode showed completely empty libraries because the local cache was not populated on first run—users had no fallback when the network was unavailable. This is a direct case for why a persistent local metadata cache matters. Heroic resolves missing runner configurations by surfacing a settings prompt in-context rather than blocking launch; it does not silently degrade.

**Lutris**
Lutris binds controller profiles to individual games and uses runner slugs as identity anchors. It exposes runner/script configuration directly in the UI (more complexity but more transparency than CrossHook needs). Lutris does not track per-game launch history natively; users rely on external tools for history analysis. The lesson: exposing raw technical identity (runner slugs, script versions) is powerful but creates UX friction for casual users.

**Playnite**
Playnite is the strongest comparator for profile identity: it assigns each game a UUID (`GameId` + `PluginId`) that persists across renames, library updates, and metadata refreshes. Display names are editable and cosmetic; the UUID is the stable anchor. Metadata (description, ratings, tags, cover art, user collections) is associated with the UUID and survives name changes. Playnite's known UX weakness: when a library plugin no longer returns a game, Playnite retains the record as "installed" indefinitely—the library entry becomes a ghost. CrossHook should avoid this by explicitly flagging externally-deleted profiles as "removed from filesystem" rather than silently preserving stale state.

**Key Takeaway for CrossHook**
The industry pattern is: use an internal stable ID as the true identity anchor; treat display names as cosmetic labels; show status inline on the affected record, not in a modal; never block the rest of the UI over a single missing or drifted profile.

---

### Core User Workflows

### Profile Rename Without Losing Context

1. User renames a profile (in CrossHook or directly by editing the TOML filename).
2. System detects the rename via file-watch or next scan, resolves the new filename to the same stable SQLite record.
3. Favorites, collections, launcher history, recents, and usage insights remain associated.
4. The rename triggers best-effort launcher cleanup (existing behavior). If launchers were removed as part of the rename, the UI should surface this: **"Launcher removed during rename — re-export to update it."** This is currently fire-and-forget; SQLite enables surfacing this outcome persistently.
5. UI confirms: **"Renamed profile. History and launcher mappings were preserved."** If launcher cleanup failed silently, append: "Launcher could not be updated."
6. If the rename is ambiguous (multiple candidates match), surface a lightweight disambiguation prompt: "Is this the same profile as _[old name]_?"

**Phase 1 (retroactive detection)**: Rename resolution fires at profile list-open time — the disambiguation prompt appears when the user opens the list, not in real time. The codebase has no background file watching today.

**Phase 2+ (file watcher)**: Real-time detection via a background file watcher is a new capability to be introduced in a later phase. In Phase 1 the UX must make clear that the prompt fires "on refresh," not instantly after the rename.

**Note on `last_used_profile`**: Settings are updated server-side after rename. No user confirmation required for this update, but it must be reflected immediately in the UI (optimistic update).

### Launcher Drift Detection

The `check_launcher_for_profile` command already returns a `LauncherInfo` struct with `is_stale`, `script_exists`, and `desktop_entry_exists` fields. The current limitation is that this state is not persisted — it exists only during the check. SQLite enables recording this state so the profile card can show drift indicators without requiring an active re-scan.

**Drift detection is a single reconciliation pass**, not a per-launcher reactive check. It is triggered explicitly: on startup, when the profile list is opened, or manually by the user. The UI must not imply drift detection is continuous or real-time.

1. User changes a launcher artifact outside CrossHook (moves `.sh` file, renames `.desktop` entry).
2. System detects a missing or hash-mismatched artifact during the next explicit reconciliation pass, reading `is_stale`, `script_exists`, and `desktop_entry_exists` from `LauncherInfo`.
3. Staleness covers two sub-cases: (a) `Name=` line in `.desktop` mismatches display name, (b) full script content differs from expected. The UI chip label should distinguish: "Launcher name mismatch" vs. "Launcher content changed."
4. UI shows a non-destructive inline state on the affected profile card: **"Launcher moved or renamed outside CrossHook."**
5. User is offered repair choices: **Re-link** (pick new path), **Rebuild** (regenerate from profile), **Dismiss** (acknowledge, keep as orphan).
6. Auto-repair should never run silently without surfacing what it did. If confidence is high (single match by stem name), show a soft confirmation banner: "Launcher re-linked automatically—was this correct?"
7. After repair, clear the drift indicator immediately without requiring a full UI refresh (optimistic update).

### History and Intelligence View

Today, launch outcomes are emitted to `ConsoleView` via `launch-log` events and then lost when the view closes. SQLite enables persistent launch history for the first time.

1. User opens a profile detail page or expands a profile card.
2. System shows (from SQLite projections, no filesystem re-scan):
   - Last launch result and timestamp
   - Last successful launch
   - Launch method used (steam_applaunch / proton_run / native)
   - Favorite status and collection memberships
   - Recent diagnostic events (from `DiagnosticReport` results, if stored as JSON)
   - Stale cache warnings for external metadata
3. History is scoped to the current view—no history panel shown until the user expands it, preventing noise on the main profile list.
4. New users (no launch history yet): hide the history panel entirely; show a placeholder only once at least one launch has been recorded.

**Display decision point**: Three viable options for how launch history surfaces in the UI:
- **Option A — Inline on profile card**: Show "Last run: 2 days ago · ✓ Success" as a single-line summary. Most compact; best for Steam Deck.
- **Option B — Dedicated history panel**: Full timeline in an expandable panel on the profile detail page. More information density; better for desktop.
- **Option C — Tooltip on timestamp**: Hover/focus on the "last run" timestamp reveals a small history popover. Works for desktop; problematic for gamepad.

Recommendation: implement Option A for the card summary and Option B for the expanded view. Avoid Option C—it is not gamepad-compatible.

### Community Browsing

1. User searches community profiles by game, trainer type, compatibility, platform tags, or tap.
2. System responds immediately from local indexed metadata (SQLite). No network call required for filter/search within already-synced taps.
3. Tap refresh indicators are shown in a non-blocking status chip ("Last synced 2 hours ago") rather than a gating spinner.
4. When the tap HEAD commit is unchanged after a refresh check, skip re-indexing entirely — this is a silent operation. Optionally show an "Up to date" badge briefly (auto-dismiss 2s), but this is not required.
5. If a tap is stale (e.g., no refresh in 7+ days), surface a passive nudge: **"Tap data may be outdated. Refresh?"** — not a blocking warning.
6. Search results appear optimistically; if a background tap refresh changes the result set, update the list in place without resetting scroll position.

### Recovery / Error Flow

1. SQLite metadata is stale or missing while TOML/filesystem state is valid.
   - Silent rebuild: projection can be reconstructed from TOML files without user action. Surface a quiet status indicator ("Rebuilding metadata cache…") in the status bar or settings page, not a modal.
   - Rebuilt state: once complete, update UI in place.
2. Profile file deleted externally while SQLite record remains.
   - UI shows an inline "Removed from filesystem" badge on the profile card.
   - Do NOT silently erase history—preserve it as a tombstone record with clear labeling.
   - Offer: **Delete record** (clear history entirely), **Restore** (if file can be recovered), or **Archive** (keep history, hide from main list).
3. Corrupt or unreadable SQLite database.
   - Fail gracefully: fall back to TOML-only mode for core operations (launch, edit). The "SQLite disabled" flag lives in Tauri managed app state (in-memory) — not in SQLite itself, which may be unreadable.
   - Surface a recovery prompt in Settings: "Metadata database is unreadable. Rebuilding will restore history from available files."
   - Do not block launch functionality while the database is being rebuilt.
4. External rename creates an ambiguous identity (multiple candidate matches).
   - Present a lightweight disambiguation flow in context, not a full-page wizard.
   - Show old name → new name candidates with file modification timestamps.

### RecentFiles with Stale Paths

The current `RecentFilesStore` silently drops file paths that no longer exist on disk. With SQLite, remembered paths survive moves but may point to a location that has changed.

- If a remembered recent path no longer exists at its known location, show a stale indicator inline in the file picker: **"File not found at last known location."**
- Offer: **Locate** (browse for new path), **Remove** (clear from recents).
- Do not auto-remove stale entries silently — the user may have temporarily unmounted a drive or moved files and want to re-establish the link.

### Profile Duplicate Lineage

The duplicate action creates a copy with a `(Copy)` suffix. With SQLite lineage tracking:
- The duplicated profile can optionally display a provenance label: **"Derived from: _[source name]_"** in the profile detail view.
- Auto-navigate to the new copy after duplication (current intent is unclear from the code).
- Lineage is informational only — the copy is fully independent for all operational purposes.

When multiple profiles have drifted launchers simultaneously (e.g., the user reorganized their scripts directory):
- Group drift notifications into a single summary chip: **"3 launchers need attention."**
- Provide a batch review view (expandable) with per-item repair choices.
- Never show 10 individual toast notifications for 10 drifted launchers.

---

## UI and Interaction Patterns

### Identity and Labeling

- Use stable SQLite IDs behind the scenes; keep filenames/profile names as the primary user-facing labels.
- Never expose raw UUIDs or internal record IDs in the UI. Identity is the profile name.
- Show derived data with explicit provenance when needed:
  - "From last successful launch"
  - "Observed during launcher scan"
  - "Cached from ProtonDB 3 days ago"

### State Separation

Separate current state from history in the UI:

| Current | Historical |
|---|---|
| Current name / current launcher path | Rename history / previous launcher slugs |
| Latest health status | Launch timeline and failure log |
| Active collection memberships | Removed collection history |

### Provenance Components

Provenance labels should be:
- Short, scannable, and consistent (max ~4 words)
- Accompanied by a timestamp when the age matters (e.g., "Cached · 3 days ago")
- Visually subordinate to the primary content (secondary text color or chip)
- Keyboard accessible and screen-reader readable as complete phrases ("Cached from ProtonDB, 3 days ago")

Examples:
- `● Cached · 3 days ago`
- `⚠ Launcher drifted`
- `✓ From last launch`
- `↺ Rebuilding…`

### Status Chips and Inline Badges

- Prefer compact inline status chips on profile cards over modal dialogs.
- Use icon + color + text—never color alone (accessibility).
- Status chip taxonomy:
  - **Neutral / info** (blue): "Last synced 2h ago"
  - **Warning** (amber): "Launcher drifted"
  - **Error** (red): "Profile file missing"
  - **Success** (green): "Launcher re-linked"
  - **In-progress** (muted spinner): "Rebuilding metadata…"
- Chips should be dismissible when the state is non-blocking.

### Collections, Favorites, and Undo

Collections are a new feature with no current code equivalent. Key design decisions:

**Where collections live in the UI**:
- Option A: A dedicated "Collections" tab alongside Main, Settings, Community.
- Option B: A sidebar filter in the profile list view (more discoverable for Steam Deck).
- Option C: A collection chip below the profile name on each card (compact, always visible).
Recommendation: Option B for the primary interaction surface; Option C for the visual indicator on cards.

**Naming and organization**:
- User-named collections (e.g., "RPGs", "Modded", "Steam Deck Ready") are the most flexible.
- System-defined collections (auto-grouped by game, platform tag, or launch method) reduce friction for new users.
- Recommended: user-named collections as the core primitive; system suggestions as optional auto-population.

**Operationally**:
- Collections and favorites should feel instantaneous: write to SQLite locally, confirm in background.
- If a favorite/collection write fails, roll back visually and show an inline error: "Couldn't save—try again."
- For destructive metadata actions (clearing history, removing from collection), provide undo within the same view session (30-second window). This does not need to be persisted across sessions. **Note**: no undo window exists anywhere in the current codebase — this must be built new as part of collections support.
- Export/import of collections is a Phase 2 concern; do not design Phase 1 UI around it.

### History Panels

- Default collapsed; expandable per profile card.
- Show at most 5 entries inline; link to a "Full history" view.
- Each entry: timestamp, event type, outcome, expandable detail.
- Keyboard navigable with arrow keys; timestamps and outcomes in accessible text (not icon-only).

---

## Performance UX

### Loading State Strategy

CrossHook's SQLite layer enables a two-tier loading strategy:

1. **Immediate (local)**: All data available in SQLite renders instantly with no spinner. Profile list, favorites, collections, last-launch results, and community browse all qualify.
2. **Background refresh**: Filesystem scan, tap network refresh, and external metadata fetch run asynchronously. Show a subtle status indicator (status bar chip or settings panel) rather than blocking the main view.

This matches the pattern used by mature local-first apps: write to local state immediately, treat the network/filesystem as an async side effect.

### Avoiding Perceived Latency

- **Profile list**: Load from SQLite projection; no filesystem stat calls on initial render.
- **Community browse**: Show cached results immediately; refresh tap data in background.
- **Launcher status**: Show last-known state from SQLite; queue a background verification pass.
- **Launch initiation**: Launch can begin using cached profile data; do not gate on a fresh filesystem check unless explicitly required.

### Background Sync Indicators

Following the principle from Carbon Design System: do not notify users about background technical operations that do not require user involvement. SQLite rebuild and tap refresh should use:
- A small, non-intrusive indicator in the status bar or a dedicated "Sync" section in Settings.
- No modal dialogs or toast notifications for routine background operations.
- A toast only if a background operation **fails** and requires user action.

### Debouncing and Scan Timing

- Filesystem scan results should be debounced (e.g., 500ms) before updating UI state to avoid flicker during bulk external file moves.
- Progressive disclosure: show available profiles immediately; overlay drift/health badges as the scan completes rather than waiting for the full scan before rendering.

---

## Accessibility Considerations

### General (Keyboard and Screen Reader)

- Drift, stale cache, and failed launch states must not rely on color alone; include icons and plain-language labels.
- History timelines and diagnostic summaries must be keyboard navigable; timestamps and outcomes in text (not icon-only).
- Provenance labels ("Cached", "Out of sync") should be short, consistent, and readable as full phrases by screen readers.
- Modal dialogs and expandable panels must trap focus correctly and return focus on close.
- Status chips must have appropriate ARIA roles (`role="status"` for non-urgent, `role="alert"` for errors requiring action).

### Steam Deck (Gamepad Navigation)

The existing `useGamepadNav` hook provides the foundation. The SQLite-backed features introduce new interactive elements that must be wired into gamepad focus management:

- **Profile cards with status chips**: The drift/health chip on a profile card must be focusable and activatable (confirm button = "show repair options"). It should not steal focus automatically.
- **Inline disambiguation prompts**: When a rename ambiguity prompt appears, it must be inserted into the spatial navigation graph at the correct position (adjacent to the affected profile card), not appended at the end of the focus order.
- **History panels**: Expanding a history panel should move controller focus into the panel. Closing returns focus to the expand button.
- **Batch drift review**: The grouped summary chip should be focusable; activating it opens a review list that is fully navigable with D-pad.
- **Undo toast**: If undo toasts are added, they must be reachable via controller. Consider a dedicated "Undo" button binding (e.g., hold Y) rather than requiring navigation to a floating toast.
- **Touch targets**: All new interactive elements (status chips, expand buttons, repair actions) must meet the minimum 44×44px touch/tap target size for Steam Deck touchscreen use.

### Color and Contrast

- Status chip colors must meet WCAG AA contrast ratios (4.5:1 minimum for text).
- Drift/health indicator icons should be legible at the small chip size used on Steam Deck's 800p display.

---

## Feedback and State Design

### Notification Hierarchy

| Priority | Use Case | Component |
|---|---|---|
| **Passive / info** | Background sync running, tap last-refreshed date | Status bar chip or Settings page row |
| **Warning (non-blocking)** | Launcher drifted, metadata stale | Inline chip on profile card; no toast |
| **Warning (action needed)** | Rename ambiguity, multiple drift candidates | Inline prompt in context |
| **Error (blocking)** | Profile file missing, corrupt DB | Inline error badge + action button; toast only if not currently visible |
| **Success** | Rename preserved history, re-link confirmed | Brief confirmation banner (auto-dismiss 3s) |

Key rule (from Carbon Design System): **do not surface notifications for background operations that do not require user involvement.** SQLite rebuild, routine tap refresh, and background scan completion are all silent unless they fail.

### Loading

- Use optimistic local reads from SQLite projections; quietly refresh from filesystem/tap scans.
- No full-page loading state for any SQLite-backed view after initial app load.
- Skeleton loaders or placeholder chips for data that requires a network/scan operation to populate initially.

### Empty States

- **New user (no launch history)**: Do not show empty history panels. Hide the history affordance entirely until at least one launch event is recorded.
- **New user (no community taps)**: Show a "Browse community taps" prompt in place of the community browse list.
- **Profile with no launcher exported**: Show a "No launcher exported yet" inline hint, not a warning badge.
- **Collection with no members**: Show an empty state with a "Add profile" action, not an error.

### Success

Emphasize continuity, not technical achievement:
- "Renamed profile. Your history and launcher mappings are preserved." (not "Database record updated.")
- "Launcher re-linked." (not "SQLite row patched.")

### Error

The backend will map database and sync failures to typed error categories. The UI must handle each distinctly — never show raw SQLite error text (which may include SQL fragments, table names, or file paths).

**Typed backend error categories → UI messages:**

| Backend category | User-facing message | Degradation level |
|---|---|---|
| `DatabaseUnavailable` | "Metadata unavailable — some features are limited." | Core launch/profile works; history, favorites, collections degraded. |
| `SyncFailed` | "Last sync incomplete — data may be outdated." | Show stale data with freshness warning; retry available. |
| `NotFound` | "This item is no longer in the local index." | Non-blocking; shown inline on affected item. |
| `PayloadTooLarge` | "External metadata could not be cached." | Non-blocking; community profile shown without cached enrichment. |

**Additional named states** (not from DB layer, but requiring distinct messaging):
- **"Profile file missing"** — TOML file deleted or moved outside CrossHook.
- **"Launcher drifted"** — exported `.sh` / `.desktop` no longer matches the record.
- **"Cache expired"** — external metadata is older than the freshness threshold.
- **"Metadata rebuilding"** — SQLite projection being reconstructed; core features still work.

Never use generic "Something went wrong" messages for these states — each has a distinct cause and a distinct recovery path.

### Path Display in Error Messages

All paths surfaced in error messages, status chips, or history panels must replace `$HOME` with `~`. For example: `~/.local/share/crosshook/profiles/mygame.toml`, not `/home/yandy/.local/share/crosshook/profiles/mygame.toml`. This sanitization should happen on the backend before passing strings to the frontend. The existing launch diagnostic panel already follows this convention — all new SQLite-backed error states must match it.

### Staleness

- Show freshness timestamps for external metadata caches and derived health states.
- Freshness thresholds are configurable defaults stored as new fields in `AppSettingsData` in `settings.toml`: 48h for ProtonDB cache, 7 days for community taps. The Settings panel should expose these values.
- Display as relative time when recent ("2 hours ago"), absolute date when old ("Cached on Mar 20").

---

## UX Risks

1. **Invisible authority confusion**: Users may believe SQLite replaced their TOML profiles if the UI over-emphasizes database-backed features. Mitigation: always frame metadata as "remembered context," never as the primary record.

2. **Ghost profiles (Playnite anti-pattern)**: If externally deleted profiles are retained as normal records indefinitely, users will be confused by profiles that appear in the list but cannot be launched. Mitigation: explicit "Removed from filesystem" tombstone state, not silent retention.

3. **Spooky auto-repair**: Silent automatic matches for externally renamed launchers could feel alarming if confidence/explanation is weak. Mitigation: show what was matched and offer a one-click undo or correction path.

4. **History noise**: Historical failures and diagnostics can overwhelm the main profile editing experience if shown by default. Mitigation: collapse history panels by default; limit inline display to the most recent entry.

5. **Inconsistent sync timing**: Collections, favorites, and usage insights can appear inconsistent if sync timing is unclear after external file edits. Mitigation: show provenance timestamps on derived data; debounce scan results.

6. **Toast floods**: If drift detection finds 10 drifted launchers and emits 10 toasts, the UI becomes unusable. Mitigation: batch all simultaneous drift findings into a single grouped notification.

7. **Blocking recovery**: If the corrupt-DB recovery flow gates launch functionality, users on Steam Deck mid-session are hard-blocked. Mitigation: fall back to TOML-only mode for core operations during database recovery.

8. **Gamepad focus trapping**: New inline prompts (disambiguation, batch drift review) inserted into the DOM after initial render may be unreachable by gamepad navigation if not wired into the `useGamepadNav` spatial graph. Mitigation: test all new interactive SQLite-backed states with controller-only navigation before shipping.

9. **Raw error text leakage**: SQLite errors contain SQL fragments, table names, and absolute file paths. If the Tauri command layer passes raw `rusqlite::Error` strings to the frontend, they will appear in the UI. Mitigation: backend must map all database errors to typed categories before returning to the frontend; the UI must never render an untyped error string.
