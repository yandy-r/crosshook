# UX Research: Profiles Page UI Enhancement

**Date**: 2026-04-01 (updated from 2026-03-31)
**Feature**: Advanced section decluttering — Profiles page in CrossHook + Game metadata and cover art (issue #52)

---

## Executive Summary

The CrossHook Profiles page currently collapses most of its configuration into a single "Advanced" `<details>` section. This one-section-hides-everything pattern is a known UX anti-pattern: it sacrifices discoverability for simplicity without offering any meaningful structural guidance about what settings are available or which ones matter for a typical workflow.

Research across game launchers (Heroic Games Launcher, Lutris, Bottles, Playnite), IDEs (VS Code, JetBrains), and system settings (macOS Ventura, KDE Plasma) consistently points to the same solution space: **task-oriented grouping with clear visual containers and at most two levels of disclosure**. The specific mechanism (sub-tabs, cards, or promoted sections) is secondary to correct grouping.

**Strongest recommendation**: Split the current "Advanced" section into 3–4 named, visually-bounded containers that are always visible (not collapsed), organized around the mental task each group serves ("Who am I?", "How do I run?", "What trainer?", "How do I appear?"). Promote health/diagnostic content to a persistent inline badge strip rather than hiding it inside the Advanced collapse.

**Updated (post-api-researcher findings)**: The codebase already contains a fully styled, production-ready sub-tab system (`crosshook-subtab-row` / `crosshook-subtab` / `crosshook-subtab--active` classes in `theme.css`) with controller-mode responsive overrides, and `@radix-ui/react-tabs` (`^1.1.13`) is already installed. This materially changes the implementation cost of the sub-tab option — it is now as low-cost as cards. The recommendation shifts to: **use the existing sub-tab infrastructure for the runner-method-dependent fields** (the section whose content changes most dramatically between steam/proton/native), while using always-visible cards for stable sections (Trainer, Environment). This hybrid gives tab-switching benefits exactly where the content diverges, without hiding stable fields.

**Second pass additions (issue #52)**: Game cover art display introduces new UX surface areas — Games page with portrait art cards, grid/list view toggle, skeleton loading states, and the Launch page split-pane layout. The Figma concept's visual direction (dark Steam-inspired theme, portrait 3:4 cards with gradient overlays, pill sub-tabs) maps closely to the existing CrossHook design system with only a few intentional adaptations required.

**Confidence**: High — supported by multiple authoritative sources, direct code inspection, confirmed library availability, and competitive analysis of Steam, Heroic, Lutris, and Playnite.

---

## 1. Current State Inventory

From reading `ProfileFormSections.tsx` and `ProfilesPage.tsx`, the Profiles page structure is:

```
[Page Banner]
[Panel]
  ├── Guided Setup (always visible, accent background)
  ├── Active Profile selector (always visible, only when profiles > 0)
  └── [CollapsibleSection: "Advanced"] (collapsed by default)
        ├── Health/stale summary row
        ├── ProfileFormSections
        │     ├── Section: Profile Identity (profile name + selector)
        │     ├── Section: Game (name, path)
        │     ├── Section: Runner Method (select)
        │     ├── CustomEnvironmentVariablesSection
        │     ├── Section: Trainer (path, type, loading mode, version) — OptionalSection
        │     ├── Section: Steam/Proton/Native Runtime
        │     │     ├── steam_applaunch: App ID, Prefix Path, Launcher Name/Icon,
        │     │     │   Proton Path, AutoPopulate, ProtonDbLookupCard
        │     │     ├── proton_run: Prefix Path, App ID, Launcher Name/Icon,
        │     │     │   Proton Path, Working Dir (OptionalSection), ProtonDbLookupCard
        │     │     └── native: Working Dir (OptionalSection)
        │     └── [nested CollapsibleSection: "Health Issues"] (when profile is broken/stale)
        └── ProfileActions (save, delete, duplicate, rename, preview, export, history)
[Panel: Launcher Export] (CollapsibleSection, when supportsLauncherExport)
```

**Identified problems:**

1. Everything except wizard + profile selector is hidden in a single "Advanced" collapse (`ProfilesPage.tsx:622`, `defaultOpen=false`).
2. Health badges and status chips are in the collapse header `meta` — they exist but users must notice them to understand something is wrong.
3. Profile identity fields (name, selector) appear inside "Advanced" even though they are the primary entry point.
4. Trainer section (`ProfileFormSections.tsx:778`) and working directory (`ProfileFormSections.tsx:1055, 1111`) are `OptionalSection` (`<details>`) — but `trainerCollapsed`/`workingDirectoryCollapsed` are `false` in the main editor when fields are non-empty. These collapses are NOT a primary problem in normal editing mode; they are a nesting violation only in `reviewMode` (InstallPage). The real nesting issue in the main editor is the Health Issues section.
5. Health Issues is a nested `CollapsibleSection` inside Advanced (`ProfilesPage.tsx:709`) — this is the concrete two-level collapse in the main editor: Advanced (level 1) → Health Issues (level 2).
6. ProtonDB lookup, environment variables, and launcher export all live at the same visual priority level with no hierarchy.

---

## 1b. Existing Infrastructure (Post-Research Update)

Confirmed via direct inspection after receiving findings from api-researcher:

**`@radix-ui/react-tabs` is already installed** (`package.json`: `^1.1.13`). Full WAI-ARIA Tabs pattern compliance out of the box, including:

- `orientation="vertical"` for sidebar-style sub-nav
- `activationMode="manual"` option (require Enter/Space instead of auto-activate on arrow — useful for complex panels)
- Data attributes for styling: `data-state="active"`, `data-disabled`, `data-orientation`

**Sub-tab CSS is already defined** in `src/styles/theme.css:104–135` and `src/styles/variables.css:45–47, 86–87`:

- `.crosshook-subtab-row`: pill-shaped container (`border-radius: 999px`, subtle border + background tint)
- `.crosshook-subtab`: individual tab button with hover/active transitions
- `.crosshook-subtab--active`: blue gradient fill (`--crosshook-color-accent` to `--crosshook-color-accent-strong`)
- Controller/responsive override (`theme.css:3199–3205`): full-width row, tabs expand to fill (`flex: 1 1 0`)
- CSS variables: `--crosshook-subtab-min-height: 40px` (standard) / `48px` (controller mode), `--crosshook-subtab-padding-inline: 16px` / `20px` (controller mode)

**Implication**: The sub-tab option is not a new design — it is an already-designed component waiting to be applied to the Profiles page. Implementation cost is equivalent to using named section cards. The runner-method selector (Steam app launch / Proton runtime / Native) is a natural match for sub-tabs because it already changes which fields are visible — a tab strip makes that branching explicit and navigable rather than implicit and collapsed.

---

## User Workflows

### 2.1 Primary Flow: Edit an Existing Profile

1. User opens Profiles page.
2. User selects a profile from the dropdown (Active Profile — currently always visible, good).
3. User wants to change a setting — must expand "Advanced" to reach any field.
4. User edits fields, saves.
5. User may then go to Launcher Export (already a separate panel, good).

**Pain point**: Step 3 is a mandatory detour through a collapsed section that has no visible preview of what's inside.

### 2.2 Primary Flow: Create a New Profile

1. User sees the wizard (always visible), clicks "New Profile".
2. Wizard guides through fields step by step.
3. After wizard completes, user lands back on the manual form inside "Advanced" to make tweaks.

**Pain point**: After wizard exit, the detailed form is collapsed. The user must remember to expand "Advanced" to verify fields.

### 2.3 Advanced User Flow: Tune Environment Variables + ProtonDB

1. User selects profile.
2. User opens "Advanced".
3. User scrolls past Profile Identity, Game, Runner Method sections to find env vars.
4. User opens ProtonDB card inside the Steam Runtime section — may not find it easily.

**Pain point**: Multiple levels of nesting before reaching the relevant feature.

### 2.4 Health Monitoring Flow

1. Health badges appear in the "Advanced" collapse header — visible without expanding.
2. User must expand "Advanced" then scroll to see Health Issues panel.
3. Health issues trigger a top-of-page banner for broken profiles — good, but separate from the inline context.

**Pain point**: Health information is split between the banner (top), the collapse header (meta badges), and the nested CollapsibleSection ("Health Issues") inside the form.

### 2.5 Cover Art Discovery Flow (New — Issue #52)

1. User opens Games page — expects to browse/select a game.
2. User sees game cards with cover art (portrait 3:4 from Steam CDN or SteamGridDB).
3. User clicks a game card — enters the profile editor or launch page for that game.
4. In launch page split-pane: game art preview on the left sidebar, trainer config on the right.

**Design goal**: Cover art is primarily decorative-plus-identifier (helps recognition; not the only source of game information). Title, playtime, and launch actions must be legible regardless of art quality.

---

## 3. UI/UX Best Practices

### 3.1 Progressive Disclosure — What the Research Says

Source: [Nielsen Norman Group — Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/), [LogRocket — Progressive Disclosure Types](https://blog.logrocket.com/ux-design/progressive-disclosure-ux-types-use-cases/)

**Key findings:**

- Progressive disclosure improves learnability, expert efficiency, and error reduction — but only when the split is done correctly.
- The most important requirement: **everything users frequently need must appear upfront**. If the split is wrong, progressive disclosure becomes a hindrance.
- **Never exceed two disclosure levels.** Designs with three or more levels of nesting show poor usability as users lose context.
- Use task analysis or frequency-of-use data to determine which features belong at each level. Intuition alone is unreliable.
- "Staged disclosure" (wizards) is a separate pattern — it works for sequential onboarding, not for ongoing configuration.

**Implication for CrossHook**: The current design violates the "two levels" rule (Advanced > OptionalSection > nested CollapsibleSection). Profile Identity fields are not advanced — they should always be visible or at minimum in the first-visible layer.

### 3.2 Tabs — When and How

Source: [Eleken — Tabs UX](https://www.eleken.co/blog-posts/tabs-ux), [LogRocket — Tabs UX Best Practices](https://blog.logrocket.com/ux-design/tabs-ux-best-practices/)

**When tabs are appropriate:**

- Grouping genuinely categorical (not sequential) content.
- Users do not need to compare content across tabs simultaneously.
- 3–5 tabs maximum; more indicates poor information architecture.
- Content is self-contained within each tab.

**Tab design specifics:**

- Labels: short, descriptive nouns. Avoid "Info", "More", "Advanced".
- Active state: bold font + underline or color change, never color alone (accessibility).
- Keyboard: Tab moves focus to tablist; Arrow keys navigate between tabs; Space/Enter activates.
- ARIA: `role="tablist"`, `role="tab"`, `role="tabpanel"`, `aria-selected="true"` on active tab.
- Fixed tabs (all visible) for 4 or fewer options; scrollable only when necessary.

**Sub-tabs (horizontal within a main page):**

- Acceptable for a single additional depth level; do not nest sub-tabs within sub-tabs.
- Alternative to sub-tabs: card-based grouping with section headers (often cleaner for settings pages).

**When to avoid tabs and use alternatives:**

- Accordions: better for hierarchical content where headings provide structure clues.
- Sidebars/vertical tabs: better when there are 6+ categories.
- Segmented controls: better for minimalist 2–3 state switches where categories are closely related.

### 3.3 Card-Based Layouts for Settings Grouping

Source: [Design Shack — Card Layouts Modern UX](https://designshack.net/articles/ux-design/card-layouts-modern-ux/), [Mockplus — Card UI Design Best Practices](https://www.mockplus.com/blog/post/card-ui-design)

Cards create visual containment that:

- Communicates that fields inside are related.
- Allows users to quickly scan the page structure.
- Provides clear whitespace separations between sections.

**In the context of settings pages**, cards (visual panels with a section heading, optional border, background tint) are often more appropriate than tabs because:

- Users can see multiple sections simultaneously.
- Scrolling preserves spatial memory better than tab switching.
- No "what tab was that field on?" disorientation.

This is exactly the existing pattern CrossHook uses for the Profile/Launcher Export split — two distinct panels on the page. The recommendation is to bring this same pattern inside the current monolithic "Advanced" section.

### 3.4 Basic vs. Advanced — Categorization Heuristics

Source: [NN/G — Mental Models](https://www.nngroup.com/articles/mental-models/), [LogRocket — Information Architecture](https://blog.logrocket.com/ux-design/organizing-categorizing-content-information-architecture/)

The decision of what is "basic" versus "advanced" should be driven by:

1. **Frequency of use**: Fields changed on most profiles should always be visible.
2. **Consequence of wrong values**: High-consequence fields (game path, runner method) should be visible and clearly labeled.
3. **Task-dependency**: Fields only relevant after a certain condition (e.g., Launcher Name only relevant when exporting a launcher) can be conditionally disclosed.

**The existing code already implements conditional disclosure** via `showLauncherMetadata`, `supportsTrainerLaunch`, `showProtonDbLookup` — these fields only render when the runner method warrants them. This is correct.

**What is wrong**: Wrapping all of this in a single "Advanced" section label with no structure signals to users that they never need to look inside unless they are "advanced".

---

## 4. Competitive Analysis

### 4.1 Heroic Games Launcher

Source: [Heroic Games Launcher Settings Interface](https://deepwiki.com/Heroic-Games-Launcher/HeroicGamesLauncher/4.4-settings-interface), [Heroic v2.4.0 Beta Discussion](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/discussions/1478)

**Structure:**

- Global Settings vs. per-game settings (two levels of hierarchy, clearly labelled).
- Per-game settings categories: Wine/Proton, Performance, Launch Options, Advanced.
- Fallback mechanism: only changed values are stored per-game; defaults inherit from global.
- Advanced section contains experimental features and environment variable tables.

**Cover Art / Library Display:**

- Game cards show cover art prominently; title is **always visible** (not only on hover) since v2.4.0.
- Cards include persistent action buttons (play, settings, update).
- Epic Store games use tall `art_square` images for grid; wide `art_cover` for featured/first position.
- Controller/gamepad mode: cards show no overlay buttons — navigation emulates SteamOS model.
- Hover effect: scale transform on `GameCard` for visual feedback.
- SteamGridDB integration for custom cover art resolution.
- Cover art aspect ratio issues were acknowledged (GitHub issue #603); resolution priority ordering was patched.

**What CrossHook can learn:**

- Always show game title on the card; do not rely on hover-reveal for the most basic identifying information.
- The category split (Wine/Proton + Performance + Launch Options + Advanced) maps well onto CrossHook's fields.
- "Advanced" in Heroic is genuinely a small residual category for experimental/power-user flags — not the entire feature set.
- Per-game vs. global inheritance is a useful mental model; CrossHook profiles are already per-game.

**Confidence**: High — based on official documentation and GitHub discussions.

### 4.2 Lutris

- Per-game configuration accessible via right-click > Configure.
- Settings are organized into tabs: Game, Runner, System, and sometimes Wine.
- Advanced runner settings exist as a separate tab within Runner — not a collapse of the main screen.
- Library shows game cover art in grid view; list view shows small thumbnails.

**What CrossHook can learn:**

- Runner-specific settings (equivalent to CrossHook's Steam Runtime / Proton Runtime section) should be a dedicated visual area, not a conditional block inside a larger form.

### 4.3 Playnite

Source: [Playnite Desktop Mode](https://api.playnite.link/docs/manual/gettingStarted/playniteDesktopMode.html), [GridViewCards Theme](https://github.com/JG00SE/GridViewCards)

**Three distinct view modes:**

1. **Grid View**: Cover art primary; hover reveals detail panel. Supports portrait (Steam-style), square, and landscape covers.
2. **Details View**: Compact list with cover image on left, metadata on right. Adjustable cover height.
3. **List/Table View**: Column-based, sortable; no cover art, maximum information density.

**Cover art handling:**

- Mixed aspect ratios are a known design challenge: Steam uses portrait 2:3 (600x900), console covers are square-ish, itch.io uses landscape.
- Playnite handles this with configurable cover display settings rather than enforcing a single ratio.
- Community themes (GridViewCards) enforce uniform portrait ratio for visual consistency.

**What CrossHook can learn:**

- When mixing art sources (Steam CDN + SteamGridDB + user-provided), enforce a single display ratio (3:4 portrait) at the container level; let the image fill/cover inside that container.
- Three view modes are the standard for game library apps: grid (visual), details (mixed), list (dense).

**Confidence**: High — based on official Playnite docs and community theme inspection.

### 4.4 Steam Library (Big Picture / Desktop)

Source: [Steam Library Assets — Steamworks](https://partner.steamgames.com/doc/store/assets/libraryassets), [Steam Community — Library Poster Dimensions](https://steamcommunity.com/discussions/forum/0/1661194916737323026/)

**Asset types and dimensions:**

| Asset Type                 | Dimensions                                    | Use Case                   |
| -------------------------- | --------------------------------------------- | -------------------------- |
| Library Portrait (poster)  | 600×900 px (updated Aug 2024: doubled assets) | Grid view primary card     |
| Library Header             | 920×430 px (safe area: 860×380)               | Library detail page banner |
| Store Capsule (horizontal) | 460×215 px → preferred 920×430                | Store/list view thumbnail  |
| Hero (background)          | 1920×620 px (3840×1240 for high DPI)          | Game detail page hero      |
| Logo                       | Transparent PNG                               | Overlaid on hero           |

**Grid layout:**

- Big Picture / Steam Deck: full-screen grid, portrait art dominant.
- Desktop library: configurable grid column count.
- Aspect ratio enforced at container level; art is never stretched.

**What CrossHook can learn:**

- Use the 600×900 portrait (3:4 aspect ratio) as the canonical grid card format.
- For list/horizontal view, use the 460×215 capsule format (or display a 132×80 crop of the portrait).
- The Steam CDN URL patterns for game art are stable and directly usable: `https://cdn.akamai.steamstatic.com/steam/apps/{appid}/library_600x900.jpg` and `https://cdn.akamai.steamstatic.com/steam/apps/{appid}/header.jpg`.

**Confidence**: High — based on official Steamworks documentation.

### 4.5 VS Code Settings

Source: [VS Code UX Guidelines](https://code.visualstudio.com/api/ux-guidelines/overview)

- Settings use a sidebar tree (categories) with a search bar and full-text search as the primary navigation.
- Two views: GUI form and JSON editor — users can use either.
- Settings are tagged `(modified)` when changed from default.
- No "Advanced" collapse — every setting is visible by scrolling, with categories providing structure.
- Settings have scope indicators (User, Workspace, Folder).

**What CrossHook can learn:**

- A search-in-form capability would allow removing the "Advanced" hide entirely without creating cognitive overload — users can find what they need.
- Marking modified fields visually helps users understand what is changed from defaults.

### 4.6 JetBrains IDEs

- Settings use a deep sidebar tree with many categories.
- Each settings pane shows fields in a standard form layout with no additional collapses.
- Keyboard shortcut `⌘,` opens settings directly; keyboard navigation within the tree is complete.

**What CrossHook can learn:**

- Deep hierarchies work in settings precisely because users are searching for something specific; there is no need to hide a settings pane from within its own pane.

### 4.7 macOS Ventura System Settings

Source: [9to5Mac — Ventura System Settings](https://9to5mac.com/2022/06/06/macos-13-ventura-system-settings-first-look/), [Macworld — Ventura System Settings Problems](https://www.macworld.com/article/836295/macos-ventura-system-settings-preferences-problems.html)

- Moved from icon grid (System Preferences) to sidebar list (System Settings) to mirror iOS.
- 31 preference sections consolidated and reorganized.
- Mixed reception: the sidebar approach works well for navigation, but some items were moved to unintuitive locations.

**Lesson**: Sidebar + content area is a proven pattern for complex settings, but the label taxonomy must match user mental models. "Advanced" fails this because it's not a task-category, it's a skill-level judgment.

### 4.8 KDE Plasma System Settings

Source: [KDE UserBase — System Settings](<https://userbase.kde.org/System_Settings/GNOME_Application_Style_(GTK)>)

- Sidebar tree + main panel; categories map to user tasks (e.g., "Workspace Behavior", "Input Devices").
- "Simple by default, powerful when needed" — initial view shows common settings; "More options" or dedicated subcategories reveal advanced settings.
- Complexity management challenge explicitly acknowledged: support common cases well while avoiding complexity explosion.

**Lesson**: KDE's philosophy — simple defaults, power accessible via clear secondary navigation rather than hidden collapses — is the right model.

### 4.9 Browser Profile Settings (Firefox / Chrome)

- Browser settings pages are long-scrolling forms with section headers, not tabs.
- Section headers (e.g., "General", "Home", "Privacy") serve as visual landmarks.
- Search functionality within settings is a primary navigation path.
- This layout works because browsers have stable, well-known setting categories.

**What CrossHook can learn:**

- CrossHook's field count is small enough that a long-scroll card layout (no tabs) may be more appropriate than sub-tabs — less interaction cost, better spatial memory.

---

## 5. Game Art Card Patterns (New — Issue #52)

### 5.1 Portrait 3:4 Card Layout

**Confidence**: High — confirmed by Steam documentation, SteamGridDB specs, and competitive analysis.

The industry-standard aspect ratio for game library cards is **3:4 portrait** (e.g., 600×900 px). This is established by:

- Steam library poster format: 600×900 px canonical, doubled to higher resolution in Aug 2024.
- SteamGridDB grid format: 600×900 px primary; 342×482 and 660×930 as alternatives.
- Playnite and Heroic both default to portrait display for Steam-sourced games.

**CSS implementation:**

```css
.crosshook-game-card__art {
  aspect-ratio: 3 / 4;
  width: 100%;
  object-fit: cover; /* fills the container, clips overflow */
  object-position: center top; /* keep top of art visible (title areas) */
  border-radius: var(--crosshook-radius-md);
}
```

Using `aspect-ratio: 3/4` on the container prevents layout shifts before the image loads — a critical perceived-performance practice.

**CSS variable addition needed:**

```css
--crosshook-game-art-aspect: 3 / 4;
--crosshook-game-art-thumbnail-width: 132px;
--crosshook-game-art-thumbnail-height: 80px;
--crosshook-game-grid-min-card: 160px; /* min column width in grid */
--crosshook-game-grid-columns: repeat(auto-fill, minmax(var(--crosshook-game-grid-min-card), 1fr));
```

### 5.2 Gradient Overlay for Text Readability

Source: [Epic Web Dev — Gradient Overlay Tutorial](https://www.epicweb.dev/tutorials/fluid-hover-cards-with-tailwind-css/implementation/enhance-text-readability-with-a-gradient), [Medium — Why Your Card Design Needs a Gradient Overlay](https://medium.com/@lilskyjuicebytes/why-your-card-design-needs-a-gradient-overlay-and-how-to-do-it-b142393572e1)

**Confidence**: High — validated across Steam, Netflix, and major game launcher designs.

The Figma concept specifies: `from-black via-black/60 to-transparent` applied from bottom to top. This is the bottom-gradient overlay pattern.

**Key implementation detail from research:**

```css
.crosshook-game-card__overlay {
  position: absolute;
  inset-x: 0;
  bottom: 0;
  /* gradient from opaque black at bottom, transparent ~70% up */
  background: linear-gradient(to top, rgba(0, 0, 0, 0.88) 0%, rgba(0, 0, 0, 0.6) 30%, transparent 70%);
  border-radius: 0 0 var(--crosshook-radius-md) var(--crosshook-radius-md);
  padding: 12px;
}
```

**Design rules:**

- Gradient must extend from 100% opacity at the bottom to transparent at ~60–70% height.
- The `from-30%` technique (fully opaque up to 30%, then fades) ensures reliable readability on bright art.
- Text (title, playtime) is anchored to `bottom: 0` within the overlay div.
- Action buttons (Launch/Heart/Edit) sit at the bottom edge, above text if space allows, or in a separate bottom bar.

**Hover state:** On hover, lighten the overlay from 88% to 60% opacity so the art "breathes" — this is the pattern Steam Big Picture uses.

### 5.3 Card Action Buttons

The Figma concept places Launch/Heart/Edit buttons at the bottom of each card. Industry patterns for this:

1. **Always visible at card bottom**: Used by Heroic v2.4.0+ — ensures actions are discoverable without hover.
2. **Hover-reveal action bar**: Common in older launchers; not recommended for controller/keyboard users.
3. **Context menu only**: Accessible but slow — right-click/menu key required.

**Recommendation**: Always-visible compact action bar at card bottom (`min-height: var(--crosshook-touch-target-compact)` = 36px standard, 44px controller mode), using icon-only buttons with `aria-label`. This aligns with Heroic v2.4.0+ behavior and CrossHook's controller mode requirements.

---

## 6. Image Loading UX (New — Issue #52)

### 6.1 Loading State Hierarchy

Source: [LogRocket — Skeleton Loading Screen Design](https://blog.logrocket.com/ux-design/skeleton-loading-screen-design/), [LogRocket — Progressive Image Loading in React](https://blog.logrocket.com/progressive-image-loading-react-tutorial/)

**Confidence**: High — industry standard, confirmed across Netflix, YouTube, and LinkedIn implementations.

The recommended three-state loading pattern for game cover art:

```
State 1: SKELETON  → show shaped placeholder matching 3:4 card dimensions
State 2: LOADED    → fade in real art (cross-fade, no layout shift)
State 3: ERROR     → show icon-based placeholder (controller/gamepad icon, or text initials)
```

**State 1 — Skeleton implementation:**

```css
.crosshook-game-card__skeleton {
  aspect-ratio: 3 / 4;
  background: linear-gradient(
    90deg,
    var(--crosshook-color-surface) 0%,
    var(--crosshook-color-bg-elevated) 50%,
    var(--crosshook-color-surface) 100%
  );
  background-size: 200% 100%;
  animation: crosshook-shimmer 1.8s ease-in-out infinite;
  border-radius: var(--crosshook-radius-md);
}

@keyframes crosshook-shimmer {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}
```

Key rules from research:

- Show skeleton within **300ms** of any user action — fast feedback confirms tap was registered.
- Use **1.5–2 second shimmer cycle** to signal active loading without feeling frantic.
- Skeleton shape must match the final content shape (3:4 for grid cards, 132×80 for list thumbnails).
- Do NOT use a spinner inside a skeleton — combine the two approaches: skeleton for layout, no extra spinners.

**State 2 — Loaded transition:**

```css
.crosshook-game-card__art {
  opacity: 0;
  transition: opacity var(--crosshook-transition-standard) ease;
}
.crosshook-game-card__art--loaded {
  opacity: 1;
}
```

Cross-fade prevents jarring layout shifts. The image element is mounted with `opacity: 0`, then the `--loaded` class is added once `onLoad` fires.

**State 3 — Error/placeholder:**

```css
.crosshook-game-card__art--error {
  background: var(--crosshook-color-surface-strong);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--crosshook-color-text-subtle);
  /* Show game title initials or a controller icon */
}
```

For offline-cached art: serve from SQLite blob or Tauri filesystem cache. The crosshook metadata DB (schema v13) already has cache infrastructure — art URLs can be stored and blobs cached in the existing `cache` tables. When offline, serve from cache; when cache miss, show error placeholder.

### 6.2 Lazy Loading with Intersection Observer

Source: [LogRocket — Lazy Loading with Intersection Observer API](https://blog.logrocket.com/lazy-loading-using-the-intersection-observer-api/), [Lazy Loading Images in React](https://miletadulovic.me/blog/lazy-loading-images-with-intersection-observer-in-react)

For a game grid with 20–100+ cards, lazy loading is essential. Pattern:

```tsx
// Reusable hook
function useInViewRef(rootMargin = '200px') {
  const ref = useRef<HTMLDivElement>(null);
  const [inView, setInView] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setInView(true);
          observer.unobserve(el);
        }
      },
      { rootMargin } // preload 200px before entering viewport
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [rootMargin]);
  return { ref, inView };
}
```

Key rules:

- Use `rootMargin: "200px"` to preload art slightly before the card scrolls into view.
- Unobserve after first intersection (art never needs reloading once fetched).
- `<img loading="lazy">` HTML attribute provides a free browser-native fallback.

### 6.3 Offline-Cached Art Display

When CrossHook is offline:

- Check local cache (SQLite metadata DB, art blob or URL hash table) first.
- If cached: serve from Tauri `fs` API or inline as data URL.
- If not cached: render error placeholder (controller icon + game title text).

This matches the existing offline-readiness pattern in the CrossHook metadata DB (schema v13 has `--crosshook-offline-ready/partial/not-ready` CSS variables and offline status badges). Art cache status should be reflected in the offline readiness indicator.

---

## 7. Grid/List View Patterns (New — Issue #52)

### 7.1 When to Offer View Modes

Source: [UX Movement — List vs Grid View](https://uxmovement.medium.com/list-vs-grid-view-when-to-use-which-on-mobile-da9a1ae62211), [Nick Babich — Mobile UX: List and Grid View](https://babich.biz/blog/mobile-ux-design-list-view-and-grid-view/), [UX Patterns Dev — Table vs List vs Cards](https://uxpatterns.dev/pattern-guide/table-vs-list-vs-cards)

**Confidence**: High — consistent findings across multiple authoritative UX sources.

**Grid view is best when:**

- Items have strong visual identity (cover art, photographs).
- Users browse to discover (not to find a specific known item).
- Visual differentiation between items is the primary navigation cue.
- Information per item is minimal (title + playtime is enough).

**List view is best when:**

- Users know what they're looking for (search + scan).
- Metadata density matters (title + runner method + last played + health status).
- Mobile/small display — vertical scrolling is more natural.
- Items have no meaningful visual differentiation (e.g., system profiles without cover art).

**CrossHook recommendation**: Grid as default (cover art is available for most Steam games; visual browsing is primary use case). List view as opt-in for power users with many profiles or who prefer metadata density.

### 7.2 Responsive Grid Column Strategy

The Figma concept specifies 2–6 column responsive grid. Implementation:

```css
/* Fluid grid: auto-fills columns, min 160px per card */
.crosshook-game-grid {
  display: grid;
  grid-template-columns: var(--crosshook-game-grid-columns);
  /* = repeat(auto-fill, minmax(160px, 1fr)) */
  gap: var(--crosshook-grid-gap); /* 20px standard, 16px < 1360px */
}
```

**Column count math** (based on `--crosshook-content-width: 1440px` and `--crosshook-page-padding: 32px`):

- Available width ≈ 1376px → 1376 / (160 + 20) ≈ 7.6 → 7 columns at maximum.
- At 900px: (900 - 64) / 180 ≈ 4.6 → 4–5 columns.
- At 600px (compact Tauri window): (600 - 32) / 180 ≈ 3 columns.

This naturally yields 2–6 columns matching the Figma concept without explicit breakpoints — `auto-fill` handles it.

**Controller mode:** Override `--crosshook-game-grid-min-card` to `200px` in controller mode for larger, easier-to-select targets.

### 7.3 List View Information Density

The Figma concept's list view row: `132×80px thumbnail | title + metadata | action buttons`.

```css
.crosshook-game-list-item {
  display: grid;
  grid-template-columns: 132px 1fr auto;
  gap: 16px;
  align-items: center;
  padding: 12px;
  border-radius: var(--crosshook-radius-sm);
  border: 1px solid var(--crosshook-color-border);
  background: var(--crosshook-color-surface);
}

.crosshook-game-list-item__thumbnail {
  width: 132px;
  height: 80px; /* 16:9-ish crop of the portrait art */
  object-fit: cover;
  border-radius: var(--crosshook-radius-sm);
}
```

Note: 132×80 is a landscape crop (~5:3) of the portrait 3:4 art. `object-fit: cover; object-position: center top` ensures the top portion (game logo area) is visible.

### 7.4 View Mode Toggle

**Toggle placement**: Search bar row, right side — consistent with Steam, Playnite, and Heroic patterns. The Figma concept shows `[Search bar][Filter button][Grid toggle][List toggle]` in a single toolbar row.

**Persistence**: Store in `localStorage` via a `useStickyState` hook. Key: `crosshook.gameView` (values: `'grid' | 'list'`). Read on mount, write on change.

```tsx
function useStickyState<T>(defaultValue: T, key: string) {
  const [value, setValue] = useState<T>(() => {
    const stored = localStorage.getItem(key);
    return stored !== null ? (JSON.parse(stored) as T) : defaultValue;
  });
  useEffect(() => {
    localStorage.setItem(key, JSON.stringify(value));
  }, [key, value]);
  return [value, setValue] as const;
}
```

**Toggle button accessibility:**

```tsx
<button
  aria-label="Grid view"
  aria-pressed={viewMode === 'grid'}
  className={`crosshook-view-toggle ${viewMode === 'grid' ? 'crosshook-view-toggle--active' : ''}`}
  onClick={() => setViewMode('grid')}
/>
```

`aria-pressed` is the correct ARIA attribute for toggle buttons (not `aria-selected` which is for tabs).

---

## 8. Figma Concept Analysis

### 8.1 Scope Clarification

The Figma concept is **scoped to the cover art card grid system** — it describes how game cards look and behave when cover art is the primary visual, and how launch/favorite/edit actions are accessible directly from those cards. It is **not** a redesign of the CrossHook theme, settings panels, or Profiles page form layout. The existing dark glassmorphism theme (`crosshook-panel`, `crosshook-card`, CSS variables) does not change.

**What the Figma concept covers:**

- Portrait 3:4 game cards with cover art as the primary visual surface
- Gradient overlay on art for title/playtime text readability
- Launch, Favorite, and Edit action buttons attached to the card (always visible)
- Grid view (2–6 responsive columns) and list view (thumbnail + metadata row)
- Search + filter + grid/list view toggle toolbar
- Launch page: split layout with game art preview sidebar + trainer config panel

**What the Figma concept does NOT prescribe:**

- Theme overhaul — the CrossHook dark palette, glassmorphism panels, and CSS variables stay as-is.
- Tailwind migration — implementation uses existing `crosshook-*` BEM classes and CSS variables.
- Profiles page form restructuring — that is covered independently in sections 3 and 13.

### 8.2 Concept Visual Elements (Card Grid Scope)

The Figma concept's relevant visual elements, scoped to the card grid:

- **Game card**: Portrait 3:4, cover art fills the card face, `border-radius` matching `--crosshook-radius-md`.
- **Gradient overlay**: Bottom-to-top gradient on the art for text readability (always present, not just on hover).
- **Title + playtime**: Always visible at card bottom inside the gradient layer.
- **Action buttons**: Launch / Favorite (heart) / Edit — placed at card bottom, always visible (not hover-only, per Heroic v2.4.0+ lesson).
- **Active/selected card state**: Accent-colored border using `--crosshook-color-accent`.
- **Filter sub-tabs above grid**: Pill-shaped tabs (e.g., "All", "Recent", "Favorites") — map directly to the existing `.crosshook-subtab-row` component already in theme.css.
- **Toolbar**: `[Search input][Filter button][Grid toggle][List toggle]` in a single row above the grid.
- **List row**: 132×80 thumbnail + game title + metadata + action buttons.

### 8.3 New Components Needed (Card Grid Only)

The Figma concept adds only the cover art card grid surface. The CrossHook theme does not change; these are purely additive.

| New Item                                               | Description                                                                                           | Uses Existing System Via                                                         |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `.crosshook-game-card`                                 | Portrait 3:4 card container                                                                           | `--crosshook-radius-md`, `--crosshook-color-border`, `--crosshook-color-accent`  |
| `.crosshook-game-card__art`                            | `<img>` with `aspect-ratio: 3/4`, `object-fit: cover`                                                 | `--crosshook-radius-md`                                                          |
| `.crosshook-game-card__overlay`                        | Bottom gradient for text readability                                                                  | `rgba(0,0,0,...)` — no new variable needed                                       |
| `.crosshook-game-card__actions`                        | Always-visible Launch/Favorite/Edit button bar                                                        | `--crosshook-touch-target-compact`, `--crosshook-color-accent`                   |
| `.crosshook-game-list-item`                            | List row: 132×80 thumbnail + metadata + actions                                                       | `--crosshook-color-surface`, `--crosshook-color-border`, `--crosshook-radius-sm` |
| `.crosshook-game-grid`                                 | `auto-fill` responsive grid container                                                                 | `--crosshook-grid-gap`                                                           |
| `.crosshook-view-toggle`                               | Grid/list toggle button with `aria-pressed`                                                           | `--crosshook-touch-target-compact`, `--crosshook-color-accent-soft`              |
| `@keyframes crosshook-shimmer` + `.crosshook-skeleton` | Shimmer loading placeholder                                                                           | `--crosshook-color-surface`, `--crosshook-color-bg-elevated`                     |
| New CSS variables in `variables.css`                   | `--crosshook-game-grid-min-card`, `--crosshook-game-grid-columns`, `--crosshook-game-art-thumbnail-*` | Additive only — no existing variables change                                     |

### 8.4 Controller Mode Implications

The Figma concept does not account for controller mode, but CrossHook's design system already does. Required adaptations to the card grid:

- **Card action buttons**: `min-height: var(--crosshook-touch-target-min)` (56px in controller mode vs 36px standard).
- **Grid column min width**: Override `--crosshook-game-grid-min-card` to `200px` in `[data-crosshook-controller-mode='true']` for larger, easier-to-select targets.
- **View toggle**: Hide in controller mode — grid-only view is appropriate; list-view scrolling is awkward with a controller.
- **Card hover effects**: Suppress `transform: scale(...)` on `:hover` in controller mode (hover does not apply to controller navigation).

### 8.5 Cover Art in Profile Context

Cover art from the card grid surfaces in three contexts:

1. **Games page grid/list**: Primary display — the Figma concept's core scope. 3:4 portrait card, full art with gradient overlay and action buttons.
2. **Launch page sidebar**: Art preview (~240–300px wide × 360–400px tall) alongside trainer config — confirms the user is launching the right game.
3. **Active profile selector / profile card**: Optional small thumbnail — low priority, not part of the initial implementation. Do not load art for every profile on dropdown open.

**Design rule**: Cover art is **decorative-plus-identifier** — it aids recognition but is never the sole source of game information. Title and key metadata must be legible regardless of art quality or availability.

---

## 9. Error Handling UX

### 9.1 Validation Across Tabs / Sections

Source: [Cloudscape Design System — Unsaved Changes](https://cloudscape.design/patterns/general/unsaved-changes/), [Smashing Magazine — Inline Validation](https://www.smashingmagazine.com/2022/09/inline-validation-web-forms-ux/)

- If moving to a tab/section pattern, validation errors in non-visible sections must be surfaced at the section level, not just field level.
- Common pattern: **tab indicator mutation** — append `•` or `*` to tab label when that section has unsaved changes or validation errors.
- Field-level inline validation: show error state immediately after focus-out, not only on save attempt.
- Cross-section save: a confirmation modal is appropriate when changes span multiple sections. "Are you sure? You have changes in: Game, Trainer, Runtime."

### 9.2 Unsaved Changes Warning

Source: [Cloudscape — Unsaved Changes](https://cloudscape.design/patterns/general/unsaved-changes/), [GitHub — Unsaved Changes Alerts in React](https://medium.com/@ignatovich.dm/how-to-create-a-custom-hook-for-unsaved-changes-alerts-in-react-b1441f0ae712)

- Warn when user tries to navigate away (profile switch, page navigation) with unsaved changes.
- Use in-page modal for in-app navigation; use `beforeunload` for window close.
- Track `hasChanges` boolean state; the existing `dirty` state in `ProfileContext` already does this.
- Confirmation message: "You have unsaved changes. Leave anyway?" with Cancel and Leave buttons.
- Do not warn if no changes were made (even if user opened a section).

**Scope clarification (from security-researcher)**: Sub-tab switching within the same profile (Steam → Proton → Native runner tabs) does NOT require a dirty-check prompt — form state persists across tab switches in React state. The `dirty` guard applies only to: (1) selecting a different profile from the dropdown, (2) page-level navigation away from Profiles. This simplifies tab implementation — no inter-tab confirmation dialogs needed.

**Discard action constraint**: If a "Discard changes" or "Reset" button is added anywhere, it must reload from the backend (`invoke('load_profile', ...)`) and clear `dirty`. Resetting to an in-memory snapshot is not acceptable — backend is the source of truth.

**Note**: CrossHook already has a `dirty` state from `useProfileContext()`. The mechanism for warning exists; it is a matter of hooking it to profile-switch and navigation events only.

### 9.3 Tab/Section State Persistence

- When a user switches sections (sub-tabs or collapses), their changes within that section must persist in React state until explicit save or discard.
- CrossHook already manages this via the profile form state in context — switching sections or expanding/collapsing should not lose field values.
- For tab-based layout: mount all tab content eagerly (or use CSS `display: none` to hide, not conditional rendering) to avoid losing form state on tab switch.

### 9.4 Image Loading Error Handling

- All image load errors must be caught via the `onError` event on `<img>` elements.
- On error: replace art with a deterministic placeholder (game title initials or a generic controller icon).
- Do not show a broken-image icon (browser default) — always provide a meaningful fallback.
- Log failed art URLs to the app console/Tauri log for debugging; do not surface to user.

---

## Performance UX

Source: [MDN — Lazy Loading](https://developer.mozilla.org/en-US/docs/Web/Performance/Guides/Lazy_loading), [618media — Lazy Loading in JS 2024](https://618media.com/en/blog/user-experience-with-lazy-loading-in-js/)

### 10.1 For CrossHook's Scale

The Profiles page has a bounded, predictable field count. Performance concerns around lazy-loading tab content are not significant at this scale. The ProtonDB lookup card is the heaviest component (network requests to the ProtonDB API) and already deferred by the app_id dependency.

For the Games page, a grid of 20–100+ cover art images is the primary performance concern. Lazy loading via Intersection Observer is required here.

### 10.2 Recommendations

- Mount all section/tab content eagerly and control visibility via CSS `display: none` or `visibility: hidden`, not conditional rendering (`{condition && <Component />`). This prevents form state loss on tab switch.
- Skeleton screens are appropriate for cover art cards (lazy-loaded via Intersection Observer).
- Skeleton screens are NOT appropriate for the profile form fields (all render from local state instantly).
- Tab switching should be instantaneous — no artificial delays or lazy mounting.

### 10.3 Perceived Performance

- When expanding a section, animate the height transition with CSS `transition: height` for perceived smoothness (already used in `CollapsibleSection`).
- Keep initial visible content minimal but complete — show all section headers immediately so users understand what's available.
- For the Games page: render the first viewport of cards immediately (above-fold images should NOT be lazy-loaded). Only cards below the fold should use Intersection Observer.

---

## 11. Accessibility Requirements

Source: [W3C WAI ARIA Authoring Practices — Tab Pattern](https://www.w3.org/WAI/ARIA/apg/), [Deque — ARIA Tab Panel Accessibility](https://www.deque.com/blog/a11y-support-series-part-1-aria-tab-panel-accessibility/), [Level Access — Accessible Navigation Menus](https://www.levelaccess.com/blog/accessible-navigation-menus-pitfalls-and-best-practices/), [W3C WAI — Decorative Images](https://www.w3.org/WAI/tutorials/images/decorative/)

### 11.1 Tab Pattern (if adopted)

Required ARIA roles:

- Container: `role="tablist"`
- Individual tabs: `role="tab"`, `aria-selected="true|false"`, `aria-controls="panel-id"`
- Content panels: `role="tabpanel"`, `aria-labelledby="tab-id"`

Keyboard navigation:

- `Tab` key: moves focus INTO the tablist (not between tabs)
- Arrow keys `←/→` (horizontal tabs) or `↑/↓` (vertical): navigate BETWEEN tabs and activate them
- `Home`/`End`: jump to first/last tab
- `Space`/`Enter`: activates focused tab (should also work if not using automatic activation)

### 11.2 Card/Accordion Pattern (if adopted instead of tabs)

- Section headings use proper heading level hierarchy (`h2` for section titles within the page).
- Collapsible accordions: use `<button>` for trigger, `aria-expanded="true|false"`, `aria-controls` pointing to content panel.
- Never use `<details>/<summary>` for interactive disclosure that modifies application state — use button+panel pattern for full ARIA support. **Note**: `OptionalSection` in `ProfileFormSections.tsx` uses `<details>/<summary>` — this is acceptable for static optional content but not for stateful or frequently-accessed sections.

### 11.3 Visual Accessibility

- Active section indicator: do not rely on color alone; use underline, bold, or border change.
- Focus indicators: visible on all interactive elements; maintain 3:1 contrast ratio minimum.
- Minimum touch targets: 44×44 CSS pixels for tab/section buttons (48×48 in controller mode per `--crosshook-touch-target-min`).
- High contrast mode: ensure section borders use `currentColor` or CSS variables that adapt.

### 11.4 Cover Art Accessibility (New — Issue #52)

**WCAG 1.1.1 Non-text Content (Level A)** requires text alternatives for all images.

**Game cover art classification**: Cover art is **decorative-plus-identifier** — it supplements the game title but does not convey unique information unavailable elsewhere (the title is always displayed below/over the art).

**Alt text rules:**

| Context                                | Alt text                                               | Reasoning                                             |
| -------------------------------------- | ------------------------------------------------------ | ----------------------------------------------------- |
| Grid card with visible title overlay   | `alt=""` (empty)                                       | Art is decorative; title text is the accessible label |
| List row thumbnail with adjacent title | `alt=""` (empty)                                       | Same — title is visible adjacent text                 |
| Launch page art sidebar                | `alt="{game title} cover art"`                         | Art is the primary visual; title may not be adjacent  |
| Error/placeholder                      | `alt="{game title}"` or `alt="No cover art available"` | Informs user why no image is visible                  |

**Additional rules:**

- All art `<img>` elements must have an explicit `alt` attribute (even if empty) — never omit `alt`.
- Do not use game title as alt text when that title is already rendered as visible text immediately adjacent — this creates duplicate announcements for screen reader users.
- The skeleton placeholder div is `role="status"` + `aria-label="Loading cover art"` while loading.

### 11.5 View Toggle Accessibility

```tsx
<div role="group" aria-label="View mode">
  <button aria-pressed={viewMode === 'grid'} aria-label="Grid view">
    <GridIcon aria-hidden="true" />
  </button>
  <button aria-pressed={viewMode === 'list'} aria-label="List view">
    <ListIcon aria-hidden="true" />
  </button>
</div>
```

Using `aria-pressed` on toggle buttons, not `aria-selected` (which is for tabs/options). Icons are `aria-hidden` since the button has an `aria-label`.

---

## 12. Component Inventory (Post-Practices-Researcher Update)

Grounded assessment of what exists vs. what needs to be built:

| Component                      | Status                                                                                                                                                                                                                                           | Action                                                                                                                                                                               |
| ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `FieldWithBrowse` / `FieldRow` | **Duplicated** — `InstallField` at `src/components/ui/InstallField.tsx` (exported, has `browseMode`/`browseFilters`/`browseTitle`/`className`) and private `FieldRow` inside `ProfileFormSections.tsx` (thinner, no browse filters, has `useId`) | Consolidate: add `id` prop to `InstallField`, delete private `FieldRow`. `InstallField` becomes canonical.                                                                           |
| `SectionCard`                  | **Not needed as new component** — `crosshook-panel` CSS class already provides the visual surface (border, background gradient, radius, shadow in `theme.css:137–151`). `CollapsibleSection` already accepts `className="crosshook-panel"`.      | A thin React wrapper (`title`, `meta?: ReactNode`, `children` props) is a one-day addition only if always-visible titled headers are needed in 2+ places. Do not build preemptively. |
| `SectionErrorIndicator`        | **Not needed** — `CollapsibleSection`'s existing `meta` slot accepts arbitrary `ReactNode`. An error dot is a two-line addition at the call site.                                                                                                | Extract only if the same indicator appears in 3+ call sites.                                                                                                                         |
| `UnsavedChangesGuard`          | **Valid but out of scope for initial cleanup.** `dirty` state exists in `ProfileContext`; tab switching within the page does not cause data loss (all tabs share the same profile state). Inter-page navigation guard is additive scope.         | Track as a follow-on issue, not a prerequisite for the layout change.                                                                                                                |
| `@radix-ui/react-tabs`         | **Already installed** (`^1.1.13`). Use primitives directly in `ProfilesPage.tsx`; no wrapper component until the pattern appears on a second page.                                                                                               | Use directly.                                                                                                                                                                        |
| `GameCard` (grid)              | **Does not exist.** New component needed: `crosshook-game-card`, `crosshook-game-card__art`, `crosshook-game-card__overlay`, `crosshook-game-card__actions`.                                                                                     | Build as new component in issue #52 implementation.                                                                                                                                  |
| `GameListItem` (list row)      | **Does not exist.** New component: `crosshook-game-list-item` with 132×80 thumbnail, title/meta, action buttons.                                                                                                                                 | Build as new component in issue #52 implementation.                                                                                                                                  |
| `ImageWithFallback`            | **Does not exist.** Reusable wrapper: skeleton state → loaded state → error/placeholder state. Accepts `src`, `alt`, `aspectRatio`, `fallbackIcon`.                                                                                              | Build once; used by both `GameCard` and `GameListItem` and potentially `LaunchPage` art sidebar.                                                                                     |
| Skeleton shimmer CSS           | **Does not exist.** `@keyframes crosshook-shimmer` + `.crosshook-skeleton` class.                                                                                                                                                                | Add to `theme.css`.                                                                                                                                                                  |
| View mode toggle               | **Does not exist.** `crosshook-view-toggle` CSS class + `useStickyState` hook for `localStorage` persistence.                                                                                                                                    | Build as simple component; reusable for any future grid/list view.                                                                                                                   |
| Game grid CSS                  | **Does not exist.** `crosshook-game-grid` class + `--crosshook-game-grid-*` CSS variables.                                                                                                                                                       | Add to `theme.css` / `variables.css`.                                                                                                                                                |

---

## 13. Recommendations

### 13.1 Must Have

**M1 — Eliminate the single "Advanced" collapse around the entire form.**
Replace it with named, always-visible section cards and/or sub-tabs. The sections should reflect user task mental models:

Hybrid recommended layout (post-infrastructure discovery):

| Section                          | Pattern                                                                     | Contents                                                                           |
| -------------------------------- | --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Profile Identity                 | Always-visible strip (already partial)                                      | Profile name, selector dropdown                                                    |
| Health Status                    | Always-visible badge strip                                                  | Health badge, offline status, version badge                                        |
| Runner (Steam / Proton / Native) | **Sub-tabs** using existing `crosshook-subtab-row` + `@radix-ui/react-tabs` | Runner-specific fields: App ID, prefix, proton path, AutoPopulate, ProtonDB lookup |
| Trainer                          | Always-visible card                                                         | Trainer path, type, loading mode, version                                          |
| Environment & Launch             | Always-visible card                                                         | Custom env vars, working directory override                                        |
| Launcher Export                  | Separate panel (already exists — keep)                                      | Launcher name/icon, export action                                                  |

Using sub-tabs for the runner section is now the preferred approach because:

1. The existing `crosshook-subtab-*` CSS is purpose-built for exactly this use case.
2. The three runner methods (Steam / Proton / Native) have distinct field sets — sub-tabs make that branching explicit.
3. `@radix-ui/react-tabs` provides ARIA compliance without additional library cost.
4. The runner method dropdown (current `ThemedSelect`) becomes the tab selector — removing a dropdown and replacing it with visible, labeled tabs reduces cognitive load.

**M2 — Promote health status to always-visible position.**
The health badge and offline status badge belong in a persistent strip below the profile selector, not inside the collapsed section. Health information is critical-path — users should see it without having to expand anything.

**M3 — Keep launcher export as a separate panel.**
The existing split between "Profile" and "Launcher Export" panels is correct and should be preserved. It represents a task boundary: configure → export.

**M4 — Cap disclosure at one level.**
Within any card, a single optional sub-section (working directory override, launcher metadata) is acceptable using the existing `OptionalSection` pattern. But no card should contain a collapse that contains another collapse.

**M5 — Games page: portrait 3:4 grid with gradient overlay text, always-visible title (issue #52).**
Use `aspect-ratio: 3/4` container, `object-fit: cover` art, bottom gradient overlay, game title always visible (not hover-only). Add `ImageWithFallback` component with skeleton → loaded → error states.

**M6 — Provide grid/list view toggle with localStorage persistence (issue #52).**
Default to grid view. Store preference in `localStorage` via `useStickyState`. Toggle buttons use `aria-pressed`.

**M7 — Lazy load off-viewport game art (issue #52).**
Use Intersection Observer with `rootMargin: "200px"`. First-viewport cards should NOT be lazy-loaded (above-the-fold images load immediately).

### 13.2 Should Have

**S1 — Dirty state indicator per section.**
When using section cards, append a visual indicator (e.g., a colored left border or `•` in the section title) when that section has been modified since last save. This prevents lost changes when users scroll past edited fields.

**S2 — ProtonDB lookup card in a prominent but contextual position.**
Currently embedded deep inside the Steam Runtime section. Should be immediately after the runner-specific required fields (App ID + Proton path), clearly labeled as "ProtonDB Compatibility" with a short description. Not hidden in a collapse.

**S3 — Unsaved changes dialog on profile switch.**
When `dirty === true` and the user selects a different profile in the top dropdown, show a confirmation dialog: "You have unsaved changes to [current profile]. Switch anyway?" This uses the existing `dirty` state from context. Note: tab switching within the same profile does NOT need this guard — all runner tabs share the same `profile` state in `ProfileContext`. This guard applies only to profile dropdown selection and sidebar page navigation.

**S4 — AutoPopulate result as a contextual hint, not a separate section.**
The `AutoPopulate` component currently renders as a separate sub-block inside the Steam Runtime section. Consider integrating its results as inline suggestions beneath the fields they populate (App ID, Prefix Path, Proton Path), similar to the ProtonDB suggestion pattern.

**S5 — Launch page split-pane with art sidebar (issue #52).**
Split the Launch page into: left sidebar (game art 240–300px wide, game title, playtime) and right main area (trainer config, launch button, status). The art sidebar uses `ImageWithFallback` with the same skeleton/error states as the Games page grid.

**S6 — Art cache in offline readiness indicator (issue #52).**
Include art cache status in the offline readiness badge. "Offline ready" should consider whether art is cached, not only whether trainer/profile data is available.

### 13.3 Nice to Have

**N1 — Inline field search.**
A search box within the profile form that scrolls to and highlights matching fields. Eliminates the discoverability problem entirely for power users. VS Code and JetBrains settings use this to great effect.

**N2 — Contextual help tooltips on hover.**
Short (1–2 sentence) explanations for non-obvious fields (Trainer Loading Mode, Prefix Path, STEAM_COMPAT_DATA_PATH derivation). Currently this information is in `crosshook-help-text` paragraphs; converting to tooltip-on-demand reduces vertical space while preserving the information.

**N3 — Section collapse as a user preference.**
Allow users to personally collapse sections they never use (e.g., a native Linux user who never uses trainer fields). Persist this preference in localStorage or TOML user settings. This is a "power user" feature and should not be the primary decluttering mechanism.

**N4 — "Compact / Expanded" view toggle.**
A single toggle that collapses all optional/infrequently-needed fields (working directory override, launcher metadata) while keeping essential fields visible. This is a simpler version of N3 that requires no per-section persistence.

**N5 — Blur-up progressive image loading (issue #52).**
Serve a tiny (20×27 px) blurred LQIP (Low Quality Image Placeholder) from cache, then crossfade to the full-res art. This requires the API layer to provide or cache LQIPs — evaluate against implementation cost; simpler skeleton shimmer is sufficient for initial implementation.

---

## 14. Creative Ideas

### 14.1 Task-Oriented Wizard as the Default Path; Manual Form as Power-User Path

The existing wizard is excellent for first-time setup. Consider extending this model: the default view shows a compact "summary card" of the current profile (name, runner method, trainer path, health badge — read-only) with an "Edit" button that expands into the full form. This makes the page clean for users who just want to verify their setup without editing.

### 14.2 Split-Pane: Profile List Left, Editor Right

Replace the page-banner layout with a two-column layout: a profile list on the left (small, ~250px), and the editor on the right (large). This is the pattern used by VS Code's multi-file editor, JetBrains settings, and most terminal emulator profile editors (Alacritty, WezTerm). Benefits:

- Profile list always visible — no dropdown needed.
- Editor area larger and easier to scroll.
- Direct visual feedback when switching profiles.

This is a significant layout change but would address root-cause layout problems better than any single section reorganization.

### 14.3 Inline Health Issue Annotations

Rather than a separate "Health Issues" collapse, annotate specific fields with the health issue inline. If `game.executable_path` is broken, show a red border and inline error on that specific field. This eliminates the need for a separate health section entirely.

### 14.4 "Required for Launch" vs. "Optional" Visual Treatment

A subtle label system:

- Red asterisk or "Required" chip: fields that will prevent launch if empty.
- Gray "Optional" label: fields that are safe to leave empty.
- Blue "Recommended" chip: ProtonDB suggestions.

This replaces the "Advanced" concept with a task-relevance concept — users know which fields they must fill, not which ones require "advanced" knowledge.

### 14.5 Games Page Hero Mode (Issue #52)

When a game is selected in the grid, expand it to a "featured" card that shows the hero/header art (920×430) full-width across the top of the grid, with a "Launch" CTA and key metadata. Other cards shrink to standard size. This is similar to the Apple App Store "Today" featured card pattern and creates visual hierarchy without a separate detail page.

---

## 15. Open Questions

1. **Target user profile**: Is the primary user a first-time configurator (wizard path) or a returning power user making small tweaks? This determines whether compact-by-default or expanded-by-default is more appropriate.

2. **Profile count at scale**: What happens to the profile selector UX when a user has 20+ profiles? The dropdown already supports pinning — is that sufficient, or does a profile list panel (split-pane idea in §14.2) become necessary at that scale?

3. **Runner-method-driven field conditionality**: Fields change significantly based on runner method. Would sub-tabs per runner method (Steam | Proton | Native) be cleaner than showing/hiding fields within a single form? This would eliminate the `showLauncherMetadata`, `supportsTrainerLaunch`, `showProtonDbLookup` conditional logic in favor of separate tab content.

4. **Wizard-vs-form parity**: The onboarding wizard and the manual form cover the same fields. If the form is reorganized into named cards, the wizard should step through the same card groups (Profile Identity → Game & Runtime → Trainer → Finalize) to create consistent mental models.

5. **When to introduce tabs**: Originally deferred pending cost analysis. **Updated**: The existing `crosshook-subtab-*` CSS and `@radix-ui/react-tabs` library make tabs zero additional design or dependency cost. Use tabs for the runner-method section (where content diverges most). Use always-visible cards for Trainer and Environment sections (whose content is stable regardless of runner method).

6. **Art source priority**: When both Steam CDN and SteamGridDB provide art for a game, which source takes priority? Recommendation: Steam CDN first (official, always correct for Steam App ID), SteamGridDB as fallback (covers non-Steam games, higher quality alternatives). User-uploaded custom art overrides both.

7. **Art caching scope**: Should art be cached in the SQLite metadata DB (persistent, survives app restart) or in a temp directory (simpler, cleared on restart)? Given the existing SQLite cache infrastructure in schema v13, SQLite blob or URL cache is preferred for offline readiness.

8. **Grid column count upper bound**: The Figma concept shows 2–6 columns. Should there be an explicit maximum (e.g., cap at 6 regardless of window width) to prevent overly small cards on ultra-wide displays?

---

## Sources

- [NN/G — Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/)
- [LogRocket — Progressive Disclosure UX Types](https://blog.logrocket.com/ux-design/progressive-disclosure-ux-types-use-cases/)
- [Eleken — Tabs UX: Best Practices](https://www.eleken.co/blog-posts/tabs-ux)
- [LogRocket — Tabbed Navigation UX Best Practices](https://blog.logrocket.com/ux-design/tabs-ux-best-practices/)
- [Design Shack — Card Layouts Modern UX](https://designshack.net/articles/ux-design/card-layouts-modern-ux/)
- [Cloudscape Design System — Unsaved Changes](https://cloudscape.design/patterns/general/unsaved-changes/)
- [W3C WAI ARIA Authoring Practices](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/)
- [Deque — ARIA Tab Panel Accessibility](https://www.deque.com/blog/a11y-support-series-part-1-aria-tab-panel-accessibility/)
- [W3C WAI — Decorative Images Tutorial](https://www.w3.org/WAI/tutorials/images/decorative/)
- [Heroic Games Launcher — Settings Interface](https://deepwiki.com/Heroic-Games-Launcher/HeroicGamesLauncher/4.4-settings-interface)
- [Heroic Games Launcher v2.4.0 Beta Discussion](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/discussions/1478)
- [9to5Mac — macOS Ventura System Settings](https://9to5mac.com/2022/06/06/macos-13-ventura-system-settings-first-look/)
- [Macworld — Ventura System Settings Problems](https://www.macworld.com/article/836295/macos-ventura-system-settings-preferences-problems.html)
- [VS Code UX Guidelines](https://code.visualstudio.com/api/ux-guidelines/overview)
- [Smashing Magazine — Inline Validation UX](https://www.smashingmagazine.com/2022/09/inline-validation-web-forms-ux/)
- [MDN — Lazy Loading](https://developer.mozilla.org/en-US/docs/Web/Performance/Guides/Lazy_loading)
- [618media — Lazy Loading in JS 2024](https://618media.com/en/blog/user-experience-with-lazy-loading-in-js/)
- [Level Access — Accessible Navigation Menus](https://www.levelaccess.com/blog/accessible-navigation-menus-pitfalls-and-best-practices/)
- [NN/G — Mental Models](https://www.nngroup.com/articles/mental-models/)
- [Steam Library Assets — Steamworks Documentation](https://partner.steamgames.com/doc/store/assets/libraryassets)
- [SteamGridDB — FAQ and Image Specs](https://www.steamgriddb.com/faq)
- [Epic Web Dev — Gradient Overlay for Text Readability](https://www.epicweb.dev/tutorials/fluid-hover-cards-with-tailwind-css/implementation/enhance-text-readability-with-a-gradient)
- [Medium — Why Your Card Design Needs a Gradient Overlay](https://medium.com/@lilskyjuicebytes/why-your-card-design-needs-a-gradient-overlay-and-how-to-do-it-b142393572e1)
- [LogRocket — Skeleton Loading Screen Design](https://blog.logrocket.com/ux-design/skeleton-loading-screen-design/)
- [LogRocket — Progressive Image Loading in React](https://blog.logrocket.com/progressive-image-loading-react-tutorial/)
- [LogRocket — Lazy Loading with Intersection Observer API](https://blog.logrocket.com/lazy-loading-using-the-intersection-observer-api/)
- [UX Movement — List vs Grid View](https://uxmovement.medium.com/list-vs-grid-view-when-to-use-which-on-mobile-da9a1ae62211)
- [Nick Babich — Mobile UX: List and Grid View](https://babich.biz/blog/mobile-ux-design-list-view-and-grid-view/)
- [UX Patterns Dev — Table vs List vs Cards](https://uxpatterns.dev/pattern-guide/table-vs-list-vs-cards)
- [Josh W. Comeau — Persisting React State in localStorage](https://www.joshwcomeau.com/react/persisting-react-state-in-localstorage/)
- [Playnite Desktop Mode Documentation](https://api.playnite.link/docs/manual/gettingStarted/playniteDesktopMode.html)
- [Smashing Magazine — Responsive Image Effects with CSS Gradients and aspect-ratio](https://www.smashingmagazine.com/2021/02/responsive-image-effects-css-gradients-aspect-ratio/)
