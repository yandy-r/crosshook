# Game Details Modal — UX Research (Workstream 4)

**Feature:** Library card body opens a modal overlay with rich game/trainer details (issue #143).  
**Scope:** Desktop Linux (Tauri/WebKitGTK), including Steam Deck–class handhelds (touch, constrained viewport, occasional gamepad-style focus).

---

## Executive Summary

A **non-navigational overlay** is the right primary pattern: users stay in the Library mental model while inspecting richer metadata than fits on a card. The modal should **dim and de-emphasize** the library without hiding it entirely, support **pointer, keyboard, and touch** equally, and **never steal or duplicate** the card’s existing explicit actions (launch, overflow menus, etc.)—those remain on the card or are clearly echoed inside the dialog only when it improves clarity.

**Form factors:** On large desktops, a **centered, width-capped dialog** with an internal scroll region preserves context and matches existing CrossHook modal styling (`crosshook-modal` family). On **narrow or short viewports** (720p handheld, tiled windows), prefer **edge-to-edge or near full-viewport** surfaces with safe-area padding, larger hit targets, and a single primary scroll container to avoid WebKitGTK dual-scroll issues.

**Resilience:** Treat **offline and partial cache** as normal: show stable identifiers (title, profile name) immediately when known, skeleton or “unavailable” placeholders for deferred fields, and **non-blocking** retry or “last updated” cues rather than hard failures.

**Accessibility:** Use a real **`role="dialog"`** with **`aria-modal="true"`**, labelled title, optional description, **focus trap**, **Escape** and **consistent dismiss controls**, and restore focus to the invoking card control on close—aligned with patterns already used in `ProfileReviewModal` and `OnboardingWizard`.

---

### Core User Workflows

### 1. Discover and open

- User **clicks or activates the card body** (not small chrome buttons) to open details.
- **Keyboard:** Enter/Space on a focused card body affordance opens the modal; avoid relying solely on click handlers without a focusable wrapper or explicit “View details” control for screen-reader users.
- **Touch (Deck):** The card body is a **single large target**; avoid requiring hover-only hints for core entry.

### 2. Orient and scan

- User reads **hero identity** (game name, artwork if any, profile/Steam context) and **secondary blocks** (paths, versions, health, community tap state, last launch, etc.—as spec’d by product).
- **Scrolling** happens **inside the dialog**; the library behind does not scroll while the modal is open (prevents disorientation).

### 3. Act without leaving Library context

- **Primary actions stay on the card** per issue requirements; the modal is primarily **informational**.
- If the modal surfaces **secondary actions** (e.g. “Copy path”, “Open folder”), they must be **labeled**, **non-destructive by default**, and not duplicate destructive work without confirmation.

### 4. Dismiss and return

- User closes via **Escape**, **visible Close** control, or **backdrop click** (if enabled and consistent with other modals).
- Focus returns to the **element that opened** the dialog; the library selection state (if any) remains unchanged unless the user explicitly changes it.

### 5. Degraded connectivity

- User opens details while **offline** or with **stale metadata**: sees **partial content** + clear **source labels** (“Cached”, “Unavailable offline”) and optional **retry** when back online—without blocking dismissal or other Library tasks.

---

## UI and Interaction Patterns

### Layout and hierarchy

- **Header:** Title + optional subtitle (e.g. profile name); **Close** in the header action cluster (matches existing modal headers).
- **Body:** Group related facts in **cards or definition lists** with consistent spacing; put **longest content** (logs, file lists) in **collapsible** sections if needed to reduce initial noise.
- **Footer (optional):** Secondary actions or status chips; avoid crowding—Deck users need thumb reach and clear separation from scrollable content.
- **Max width:** ~`min(720px, 100% - 2 * gutter)` on desktop; full width minus safe margins on small screens.
- **Max height:** Cap dialog height to viewport; **internal scroll** with `overscroll-behavior: contain` on the scroll container to reduce scroll chaining (per project scroll guidance).

### Backdrop and stacking

- **Semi-opaque backdrop** with light blur (consistent with `crosshook-modal__backdrop`) signals modality without fully obscuring the Library silhouette.
- **mousedown on backdrop** dismisses only when the event target is the backdrop itself (not children), matching existing `ProfileReviewModal` behavior—reduces accidental close when selecting text near the edge.

### Input modalities

| Modality                               | Recommendation                                                                                                                                                                                           |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mouse                                  | Click body to open; clear hover states on buttons; draggable regions should not steal card clicks.                                                                                                       |
| Touch                                  | **44×44 px minimum** interactive targets; avoid hover-only tooltips for essential info; support swipe-from-edge OS gestures by not placing critical controls flush against screen edges without padding. |
| Keyboard                               | Tab cycles within dialog; **Shift+Tab** reverses; **Escape** closes; no tab loop into the obscured page.                                                                                                 |
| Gamepad / Deck (when OS maps to focus) | Predictable **focus order** (header → body top → actions); visible focus ring using existing focus styles.                                                                                               |

### Relationship to card actions

- **Card buttons** (launch, menu, etc.) remain **separate hit targets** with **stopPropagation** where needed so body click does not fire when pressing a button.
- **Visual:** Subtle hint on card (“View details” chevron or text) optional but improves learnability without requiring tutorial copy.

### Motion

- Respect **`prefers-reduced-motion`**: instant or minimal open/close transitions (project already reduces animations globally in `theme.css`).

---

## Accessibility Considerations

### Semantics

- Container: **`role="dialog"`** and **`aria-modal="true"`** on the focusable surface.
- **`aria-labelledby`** pointing to the visible **`<h2>` / title** id; **`aria-describedby`** when a short summary or error banner is needed for context.
- Backdrop: **`aria-hidden="true"`** so screen readers stay in the dialog tree.

### Focus management

- On open: **move focus** into the dialog—typically the **Close** button or the **heading** with `tabIndex={-1}` for programmatic focus (see `ProfileReviewModal` / onboarding patterns).
- On close: **restore focus** to the previously focused element (card or “details” control).
- **Focus trap:** While open, Tab must not escape to the Library; use existing **`data-crosshook-focus-root`** / focus-scope utilities where applicable.

### Keyboard

- **Escape:** closes unless a nested confirmation `alertdialog` is open—then Escape cancels the inner layer first (stacked modal pattern already used in `ProfileReviewModal`).
- **No keyboard dead-ends** inside scroll regions; ensure focusable controls remain reachable.

### Action affordances

- Every control has a **visible label**; icon-only buttons need **`aria-label`**.
- **Destructive actions** (if any) are not primary-styled; require confirmation in a separate **`role="alertdialog"`** step when data loss is possible.

### Color and contrast

- Error/offline banners meet **contrast** requirements; do not rely on color alone—use **icon + text**.

---

### Feedback and State Design

### Loading (skeleton)

- Show the **dialog shell immediately** (title area + close) with **skeleton blocks** shaped like final sections—not a blank canvas.
- Skeleton **duration** is indeterminate; avoid fake progress bars unless tied to real steps.
- If some fields resolve faster, **progressive reveal** (hydrate sections as data arrives) beats waiting for one mega-request.

### Empty / partial data

- **Unknown field:** em dash or “Not set” with muted styling; optional “Why?” tooltip only if it adds real value.
- **Offline:** banner or inline badge **“Offline — showing cached data”** with **timestamp** when available.
- **Missing artwork:** neutral **placeholder** (initials or generic game glyph), not a broken image icon.

### Error

- **Inline error region** with **`role="status"`** or **`role="alert"`** for critical blockers; include **Retry** when the operation is idempotent.
- **Non-fatal errors** (e.g. one section failed): show **section-level** error with retry; keep other sections usable.
- Do not trap the user—**Close** always works.

### Success / steady state

- Once loaded, **remove skeletons** entirely; avoid flicker by **keeping stable layout** (min-heights or reserved rows for known variable content).
- After user-triggered refresh, prefer **subtle “Updated just now”** text or toast pattern consistent with app conventions—avoid loud success animations for routine loads.

---

## UX Risks

| Risk                                       | Mitigation                                                                                                                                       |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Accidental opens** from mis-taps on Deck | Separate button zones; optional press-and-hold or settings toggle if telemetry shows false opens.                                                |
| **Dual-scroll / scroll jank** (WebKitGTK)  | Single designated scroll container; register it in `useScrollEnhance` **SCROLLABLE** selector per project rules; `overscroll-behavior: contain`. |
| **Focus loss** on close                    | Always restore focus; test with keyboard-only navigation from card to modal and back.                                                            |
| **Backdrop dismiss during text selection** | Use backdrop **mousedown** targeting checks; document whether click-outside is enabled when future nested modals exist.                          |
| **Misleading offline data**                | Label stale data; show **last successful sync** where possible; avoid implying live Steam/API state when offline.                                |
| **Action duplication confusion**           | Modal stays informational; if echoing card actions, use **identical labels** and outcomes, or omit.                                              |
| **Tall content / small viewport**          | Ensure **Close** and critical actions remain reachable (sticky header/footer or always-visible dismiss).                                         |
| **Performance on low-power devices**       | Defer heavy sections; lazy-render tabs; keep initial payload minimal so shell + skeleton feels instant.                                          |

---

_This document is UX research input for implementation of the game details modal; it does not prescribe backend contracts or exact field lists._
