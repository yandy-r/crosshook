# Game Details Modal — Research Recommendations (Workstream 5)

**Feature:** Game Details Modal opened from the library card, aggregating existing data, with quick actions and **no initial persistence or schema changes** (issue #143).

**Repo anchors:** `LibraryPage` / `LibraryGrid` / `LibraryCard` (`src/crosshook-native/src/components/library/`), `useLibrarySummaries` + `profile_list_summaries`, `useProfile` / `ProfileContext` (`profile_load`), `LaunchPage` + `LaunchPanel` patterns, modal stack (`ProfileReviewModal`, `ProfilePreviewModal`, `crosshook-modal__*`), `useScrollEnhance` (`.crosshook-modal__body` already listed for scroll enhancement).

---

## Executive Summary

The lowest-risk path is a **frontend-first modal** that reuses the established `crosshook-modal` shell (portal, focus management, backdrop, keyboard dismissal) and **existing Tauri commands**—primarily `profile_load` for the focused profile plus whatever the UI already uses elsewhere (e.g. health context, launch helpers). Quick actions should **delegate** to current behaviors (`selectProfile` + `onNavigate('launch'|'profiles')`, `toggleFavorite`) rather than duplicating launch or save logic.

The main design tension is **selection semantics**: the app’s global “selected profile” drives `LaunchPage` and `ProfilesPage`. The modal must either **sync selection** when opened (predictable for downstream actions) or **isolate** modal state (fewer side effects while browsing the library) at the cost of more careful orchestration for Launch/Edit from the modal.

---

### Recommended Implementation Strategy

### 1. Modal shell and layout

- **Reuse** the same structural pattern as `ProfileReviewModal` / `ProfilePreviewModal`: `createPortal`, `crosshook-modal`, `crosshook-modal__surface`, `crosshook-panel`, `crosshook-focus-scope`, `role="dialog"`, `aria-modal="true"`, Escape handling, and optional backdrop dismiss (decide explicitly—see Decision Checklist).
- Put scrollable content in **`crosshook-modal__body`** so WebKitGTK scroll compensation from `useScrollEnhance` applies without adding new selectors.
- For long content, use **`overscroll-behavior: contain`** on inner scroll regions if nested scroll is introduced, per project layout rules.

### 2. Data aggregation (no new persistence)

- **Phase-A (sufficient for MVP):** On open, call `profile_load` for the chosen profile name (same payload `ProfilesPage` / editor already consumes). Map fields into a read-only “details” view: display name, Steam App ID, key paths visible elsewhere, trainer/game hints already on `GameProfile`, etc.
- **Reuse components** where they are already parameterized by `profile` or `appId` (e.g. patterns around `LaunchPage` / `ProtonDbLookupCard`) **only** if they do not assume global `ProfileContext` is already switched—otherwise extract **presentational** subcomponents or pass explicit props.
- **Health / metadata:** If the modal should show health or last-launch hints, prefer **existing hooks/contexts** (`ProfileHealthContext`, etc.) keyed by profile name. If gaps exist, return structured “not available” UI rather than adding SQLite columns in the first iteration.

### 3. Entry point from the library card

- Today the card’s primary click selects the profile; footer buttons run Launch, Favorite, Edit (`LibraryPage.tsx`). Add a **dedicated affordance** for “details” (icon button or overflow menu) so it does not fight with selection-on-click, **unless** the product decision is to open the modal on card click and move selection to a secondary interaction (see decisions).
- **Stop propagation** on the details control so it does not double-trigger grid selection if selection remains on card click.

### 4. Quick actions

- **Launch:** `await selectProfile(name)` then `onNavigate('launch')`—same as `handleLaunch` today.
- **Edit profile:** `await selectProfile(name)` then `onNavigate('profiles')`—same as `handleEdit`.
- **Favorite:** reuse `toggleFavorite` with the same optimistic pattern as the card (or centralize in one handler to avoid drift).
- Avoid embedding full **editable** forms in v1; keep the modal **inspect + shortcut** to existing routes where persistence already lives.

### Tradeoffs (summary)

| Approach                                           | Pros                                       | Cons                                                                                   |
| -------------------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------------------- |
| `profile_load` on each open                        | Full fidelity, no backend changes          | Disk/parse cost per open; needs loading/error states                                   |
| Widen `profile_list_summaries` / new read-only DTO | Cheaper list + modal peek                  | Requires IPC and core changes (still no DB migration, but expands scope)               |
| Sync selection when modal opens                    | Launch/Edit always consistent with context | Mutates global state while “browsing” library                                          |
| Modal-local profile snapshot only                  | Cleaner library browsing                   | Must explicitly sync before Launch/Edit or pass data into launch builder (error-prone) |

---

## Phased Rollout Suggestion

**Phase 1 — Shell + identity + actions**

- Modal open/close, title/hero (cover via existing `useGameCoverArt` or shared helper), Steam App ID, profile name.
- Quick actions: Launch, Edit, Favorite (and Close).
- Loading and error states for `profile_load`.

**Phase 2 — Read-only aggregation**

- Surface a concise subset of launch-oriented readouts already meaningful in-app (e.g. compatibility notes, health badge, trainer version display) using existing types and hooks.
- Deep links or “Open in Launch” style copy where full interactive tooling would duplicate `LaunchPage`.

**Phase 3 — UX polish and performance**

- Debounce or cancel in-flight loads when switching profiles quickly.
- Optional: cache last-loaded `GameProfile` by name in React state for the session (memory-only, not SQLite).
- Keyboard shortcuts from library grid when modal is open (focus trap already required).

---

## Quick Wins

- **Details** icon on `LibraryCard` wired to local `useState` in `LibraryPage` (or a tiny `useGameDetailsModal` hook) without touching persistence.
- Reuse **`profile_load`** only—zero schema migration.
- **Copy Steam App ID** / **Open folder**-style actions if paths are already exposed on `GameProfile` (read-only, high user value).

---

## Future Enhancements

- **Enriched summary IPC:** Extend `ProfileSummary` or add `profile_peek` with a stable, versioned DTO for modal-specific fields (still TOML/SQLite-backed reads only).
- **Persisted UI state:** Remember last-opened tab or modal size in `settings.toml` (requires explicit storage classification in a follow-up issue).
- **Inline edits** that today only exist on `LaunchPage` / `ProfilesPage`—only after clear persistence and validation boundaries.
- **Community / tap** snippets if they can be read from existing metadata APIs without new tables.

---

### Risk Mitigations

| Risk                                                 | Mitigation                                                                                                                               |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Focus trap / Escape / backdrop fights with WebKitGTK | Reuse proven `ProfileReviewModal` patterns; test with modal open from library while another route is underneath.                         |
| Scroll jank inside modal                             | Keep primary scroll on `.crosshook-modal__body`; avoid new `overflow-y: auto` without updating `SCROLLABLE` in `useScrollEnhance`.       |
| Global `selectedProfile` drift                       | Document and implement one rule: either always `selectProfile` on modal open, or never until user taps Launch/Edit—do not mix.           |
| Stale data after external file edits                 | Close modal on successful save elsewhere, or provide explicit “Refresh” calling `profile_load` again.                                    |
| Performance: loading every profile                   | Show skeleton UI; cancel previous request by profile name or `AbortController` if wiring allows.                                         |
| Accessibility                                        | Label dialog, wire `aria-labelledby`, ensure first focus lands on close control or heading; don’t rely on color alone for health states. |

---

## Decision Checklist

Decide **before** parallel implementation planning:

1. **Open gesture:** Dedicated “details” control vs double-click vs primary card click opens modal (and what happens to selection).
2. **Selection policy:** **Resolved** - `selectProfile` runs on modal open.
3. **Backdrop dismiss:** **Resolved** - enabled with Esc + outside click + close button.
4. **Scope of v1 content:** Which fields are mandatory in the first shippable slice vs explicitly “later phase”?
5. **Component reuse vs duplication:** Which `LaunchPage` sub-areas are **read-only excerpts** vs deep navigation only?
6. **List view parity:** If library `viewMode === 'list'`, same modal entry point and layout constraints?
7. **Offline / load failure:** Copy and fallback when `profile_load` fails (broken profile, missing file).
8. **Analytics / telemetry:** If added later, classify as runtime-only vs settings (out of scope for v1 but avoids rework).

**V1 quick actions scope decision:** minimal.

---

_End of Workstream 5 synthesis._
