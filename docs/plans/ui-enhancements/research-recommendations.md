# UI Enhancements: Recommendations Report (Second Pass)

## Executive Summary

**Second pass update**: This revision integrates issue #52 (game metadata and cover art via Steam API / SteamGridDB) into the Profiles page restructuring plan. The original first-pass phasing (Phases 0-3) was a UI-only restructuring with LOW risk. This second pass weaves #52's backend infrastructure, API integration, and visual enhancements into a unified phasing strategy so that cover art is built into the card layout from the start -- not bolted on after the fact.

The core recommendation remains: **Hybrid Promote + Cards (D1) followed by Sub-Tabs (D4)**. What changes is that the card layout is designed from Phase 1 with cover art slots, and backend image infrastructure is built in Phase 0 alongside the component cleanup. SteamGridDB is deferred to Phase 3 to limit initial external API surface. Overall risk shifts from LOW to **LOW-MEDIUM** due to new external API dependencies and filesystem cache management.

The Figma concept specifically targets a **library grid system with game cover art cards** where favorite, edit, and launch actions are accessible directly from the card. This is not a theme redesign -- the existing CrossHook dark glassmorphism theme, BEM `crosshook-*` classes, CSS variables, and controller mode all remain unchanged. The grid/card pattern integrates into the existing Profiles page as an alternative browse mode alongside the current dropdown selector. The pattern draws from the existing `crosshook-community-browser__profile-grid` (auto-fit responsive grid) and `crosshook-community-browser__profile-card` (glassmorphism article cards) infrastructure already used in CommunityBrowser. Elements buildable with existing CSS are planned for Phase 2; the full library grid view is Phase 4.

## Dependency Analysis

Before evaluating approaches, the API/library research established what's available without new dependencies:

| Library                     | Status                        | Verdict                                                                                                                                                                          |
| --------------------------- | ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@radix-ui/react-tabs`      | Already installed (`^1.1.13`) | Use for sub-tabs. WAI-ARIA compliant, keyboard nav built-in.                                                                                                                     |
| `@radix-ui/react-select`    | Already installed (`^2.2.6`)  | Already used by `ThemedSelect`.                                                                                                                                                  |
| `@radix-ui/react-accordion` | Not installed                 | Could replace native `<details>` in `CollapsibleSection` for animation and `type="multiple"` support. Low-risk single dependency add from same vendor. Not required for Phase 1. |
| shadcn/ui                   | Not installed                 | **Requires Tailwind CSS** -- incompatible with CrossHook's plain CSS variable system. Not recommended.                                                                           |
| Headless UI                 | Not installed                 | Duplicates Radix Tabs capability. Adds ~22kB+ unnecessarily. Not recommended.                                                                                                    |
| MUI / Ant Design / etc.     | Not installed                 | ~500kB+ bundle increase, opinionated theming conflicts with `crosshook-*` CSS. Not recommended.                                                                                  |
| `reqwest`                   | Already in Cargo.toml         | Used by ProtonDB client. Reuse for Steam Store API and SteamGridDB fetches. Zero new Rust dependency.                                                                            |

**Conclusion**: Zero new frontend dependencies needed for any recommended phase. Backend reuses existing `reqwest` + `MetadataStore` + `external_cache_entries` infrastructure. The only new backend artifact is a `game_image_cache` SQLite table and filesystem cache directory.

## Design Intent Evidence for Sub-Tabs

The `variables.css` design tokens contain explicit sub-tab infrastructure that has never been used in production:

```css
/* Default (variables.css:45-46) */
--crosshook-subtab-min-height: 40px;
--crosshook-subtab-padding-inline: 16px;

/* Controller mode override (variables.css:86-87) */
--crosshook-subtab-min-height: 48px;
--crosshook-subtab-padding-inline: 20px;
```

Additionally, `theme.css:104-135` defines complete `crosshook-subtab-row` and `crosshook-subtab` / `crosshook-subtab--active` CSS classes with pill-shaped styling, gradient active state, and transition animations. This infrastructure was built but never connected to any page component, indicating sub-tabs were always part of the design plan.

## Approach Evaluation

### Approach A: Card-Based Visual Container Separation

**Description**: Break the monolithic Advanced collapsible into distinct `crosshook-panel` / `crosshook-card` containers, one per logical group (Profile Identity, Game, Runner Method, Trainer, Runtime, Environment Variables, ProtonDB, Health).

**Pros**:

- Lowest implementation effort -- existing CSS classes (`crosshook-panel`, `crosshook-card`) already provide glassmorphism styling, borders, and shadows (`theme.css:137-152`)
- No new dependencies or architectural changes
- Preserves the linear form layout that `ProfileFormSections` expects
- No impact on any `ProfileFormSections` consumer (`ProfilesPage` full editor, `InstallPage` review modal, or `OnboardingWizard` type imports)
- Consistent with existing patterns: `LaunchPage` uses multiple `CollapsibleSection` + `crosshook-panel` containers for Gamescope, MangoHud, Launch Optimizations, etc.
- Each card can independently collapse via `CollapsibleSection`
- Practices assessment: "Low complexity, medium value -- do it as a baseline layer under any other option"
- Security: no component unmount risk -- all sections remain in DOM
- **#52 integration**: Cards provide natural slots for cover art imagery adjacent to game metadata

**Cons**:

- Does not reduce vertical scrolling -- the page becomes visually clearer but physically longer
- No information architecture change -- all sections are still visible on one scroll
- Conditional sections (Trainer, ProtonDB) still create empty gaps depending on launch method

**Effort**: Low (1-2 days). Primarily CSS and JSX restructuring in `ProfilesPage.tsx`.

**Risk**: Low. This is purely additive visual separation.

### Approach B: Sub-Tab Navigation Within Page

**Description**: Replace the collapsed Advanced section with a sub-tab bar (e.g., "Profile | Runtime | Trainer | Tools") where each tab shows only its relevant form sections.

**Pros**:

- Dramatically reduces visible clutter -- only one section group visible at a time
- Design system already fully supports this: `crosshook-subtab-row` and `crosshook-subtab` classes exist in `theme.css:104-135` with active state styling, CSS variables in `variables.css` with controller mode overrides (`48px` min-height, `20px` padding for gamepad)
- Radix UI Tabs is already a project dependency (used in `Sidebar.tsx` and `ContentArea.tsx` via `@radix-ui/react-tabs`) -- do not build a custom tab component, do not add Headless UI or any alternative
- Zero new dependencies required
- Enables future scalability as more features are added per section
- Practices assessment: "Low-medium complexity, highest value -- best first step"
- API research assessment: "RECOMMENDED -- zero cost" given all infrastructure exists

**Cons**:

- **Conditional tab visibility is complex**: The Trainer tab only exists when `launchMethod !== 'native'`. The Runtime tab content varies dramatically by launch method (Steam shows 5+ fields; native shows 1). Empty or sparse tabs feel broken.
- **Breaks cross-section workflows**: ProtonDB recommendations (in Runtime/Tools) apply environment variables (in Runtime/Env Vars). If these are on different tabs, users must switch tabs mid-workflow. Similarly, AutoPopulate fills Steam fields that span the Runtime section.
- **ProfileFormSections reuse conflict**: This component is used in two rendering contexts beyond `ProfilesPage`: `InstallPage.tsx:441` renders it with `reviewMode` inside a `ProfileReviewModal` for compact review after game installation. Embedding tabs inside `ProfileFormSections` would force a tab-based layout into that compact review modal -- wrong UX for a confirmation step. The `OnboardingWizard` only imports the `ProtonInstallOption` type, not the component itself, so it is not directly affected. The correct approach: tabs must live at the `ProfilesPage` level, wrapping `ProfileFormSections` output -- not inside the component.
- **Action bar placement ambiguity**: Save/Delete/Duplicate must remain accessible regardless of active tab. Requires either a sticky footer (new pattern) or duplicating the bar on each tab.
- **State preservation**: Unsaved changes in one tab should be preserved when switching to another. The current single-form approach handles this naturally; tabs could accidentally lose input focus context.
- **Security (W1): Component unmount data loss**: `CustomEnvironmentVariablesSection` buffers env var row edits in local React state. If sub-tab navigation unmounts the component mid-edit, in-progress rows are silently discarded. Must use CSS show/hide (`display: none`) instead of conditional rendering for tab content, or add a `useEffect` cleanup to flush local rows to `ProfileContext` on unmount.

**Effort**: Medium-High (3-5 days). Requires refactoring `ProfileFormSections` into composable section components, adding tab state management, handling conditional tab visibility, and ensuring the action bar remains accessible.

**Risk**: Medium. The conditional visibility and cross-section workflow issues are real usability regressions if not handled carefully.

**Note**: Both practices and API research classify the tab mechanism itself as near-zero complexity since all infrastructure exists. The real effort is in the section extraction from `ProfileFormSections` and the conditional visibility logic -- not in the tab implementation.

### Approach C: Promote Key Sections from Advanced

**Description**: Remove the collapsed "Advanced" `CollapsibleSection` wrapper entirely. Promote the most important sections (Profile Identity, Game, Runner Method) to be always visible at the top level. Keep less-frequently-edited sections (Trainer details, ProtonDB, Environment Variables) in individually collapsible containers below.

**Pros**:

- Directly addresses the core problem: critical fields are hidden behind a click
- Game and Runner Method are mandatory fields that must not live behind a collapsed section (practices assessment)
- Matches how `LaunchPage` already works -- top-level sections with individual collapsibles
- Minimal code change: remove the outer `CollapsibleSection` in `ProfilesPage.tsx:622-751`, restructure the inner content into separate `CollapsibleSection` or `crosshook-panel` blocks
- The wizard, profile selector, and action bar are already outside the Advanced section -- this approach extends that pattern
- Health issues can become a persistent banner or their own card rather than being buried
- Practices assessment: "Should be done regardless"
- Security: preserves existing delete confirmation two-step flow (`confirmDelete` -> `executeDelete`) without modification

**Cons**:

- Page becomes longer (all sections rendered, even if some are collapsed)
- Doesn't introduce new navigational patterns -- power users who want quick access to a specific section still scroll
- "Where does Advanced end?" -- without clear visual grouping, the page may feel like an undifferentiated list

**Effort**: Low (1-2 days). Primarily restructuring JSX in `ProfilesPage.tsx`.

**Risk**: Low. This is the most conservative change with the highest immediate impact.

### Approach D: Creative Alternatives

#### D1: Hybrid Promote + Cards (Recommended for Phase 1)

Combine Approach C (promote from Advanced) with Approach A (card containers):

- Remove the Advanced collapsible wrapper
- Group sections into 3-4 named cards: **Core** (Profile Identity + Game + Runner Method), **Runtime** (Steam/Proton fields + Env Vars + ProtonDB + AutoPopulate), **Trainer** (path + type + version + loading mode), **Diagnostics** (Health Issues + Version Status)
- Each card is a `CollapsibleSection` with `crosshook-panel` styling
- Action bar sits outside all cards in a sticky footer or dedicated bottom card
- Cards that are empty for the current launch method simply don't render (already handled by conditional logic in `ProfileFormSections`)
- **#52 integration**: Core card includes a cover art slot adjacent to profile identity, designed with proper aspect-ratio CSS that collapses gracefully when no art is available

**Effort**: Low-Medium (2-3 days)
**Risk**: Low

#### D2: Quick Settings Summary Bar

Add a compact summary strip above the form sections showing key profile metadata at a glance (launch method badge, game name, trainer status, health status, ProtonDB rating). Clicking any badge scrolls to or expands the relevant section. This provides discoverability without restructuring the form.

**#52 enhancement**: The summary bar can include a small game cover art thumbnail (32x32 or 48x48) as the first element, providing instant visual identification.

**Effort**: Low (1-2 days)
**Risk**: Low -- additive, no restructuring needed

#### D3: Section Anchor Navigation

Add a sticky section navigation bar (similar to a table-of-contents sidebar or horizontal pill strip) that shows all visible sections and scrolls to the clicked one. Uses `scrollIntoView` -- no tab content switching, just navigation aid.

**Effort**: Low-Medium (2-3 days)
**Risk**: Low -- the existing `healthIssuesRef.current?.scrollIntoView()` pattern in `ProfilesPage.tsx:517` proves this works in the codebase

#### D4: Sub-Tabs as Phase 3

After implementing the hybrid promote + cards approach, add sub-tabs as a follow-up phase. By then, sections will already be cleanly separated into composable card components, making the tab extraction straightforward. The existing Radix Tabs + `crosshook-subtab` CSS makes this zero-dependency. Both practices and API research strongly recommend this as the eventual target state.

**Not recommended**: Full page split into separate routes. High complexity, marginal gain over sub-tabs. Also not recommended: shadcn/ui (requires Tailwind), Headless UI (duplicates Radix), or any full design system (bundle bloat, style conflicts).

## Implementation Recommendations

### Recommended Approach

### Primary: Hybrid Promote + Cards (D1) followed by Sub-Tabs (D4)

**Rationale**:

1. **Addresses the root cause immediately**: The real problem is that everything is behind a single collapsed "Advanced" toggle. Promoting sections and giving them visual boundaries directly solves this.
2. **Lowest risk first**: No new navigation patterns, no new dependencies, no component reuse conflicts in Phase 1.
3. **Consistent with existing patterns**: `LaunchPage` already uses this exact pattern (multiple `CollapsibleSection` + `crosshook-panel` blocks at the page level).
4. **Preserves `ProfileFormSections` reuse**: Both the `ProfilesPage` full editor and the `InstallPage` review modal continue to work unchanged. Tab-based navigation is layered at the `ProfilesPage` level only in Phase 3, wrapping the linear form output rather than modifying the component itself.
5. **Paves the way for sub-tabs**: Cards become natural tab content containers. The design system's existing sub-tab CSS tokens and controller mode overrides confirm this was always the intended trajectory.
6. **Zero frontend dependency cost**: Both phases use only what's already installed and styled.
7. **Security-safe**: Phase 1 has no component unmount risks. Phase 3 (sub-tabs) must use CSS show/hide for tab content to avoid data loss in buffered components (see Security Constraints below).
8. **#52 integration**: Cards provide the natural layout slot for cover art from day one. Building the image cache infrastructure in Phase 0 means Phase 2 only needs to wire the display -- no structural rework.

### With Additions: Quick Settings Bar (D2) + Sticky Action Footer

- Add a summary bar below the profile selector showing key badges/chips, with optional game art thumbnail
- Move `ProfileActions` to a sticky bottom bar that's always visible regardless of scroll position

### Section Grouping (Recommended)

Based on the data model (`GameProfile` type in `types/profile.ts`), conditional rendering logic, and #52 cover art requirements:

| Card            | Contents                                                                       | Collapsible?                         | Condition                                |
| --------------- | ------------------------------------------------------------------------------ | ------------------------------------ | ---------------------------------------- |
| **Core**        | Profile Identity, Game Name, Game Path, Runner Method, **Cover Art slot**      | No (always open)                     | Always                                   |
| **Runtime**     | Steam/Proton fields, Prefix Path, Proton Path, AutoPopulate, Working Directory | Yes (default open)                   | Always (content varies by launch method) |
| **Environment** | Custom Env Vars, ProtonDB Lookup + Apply                                       | Yes (default open)                   | Always                                   |
| **Trainer**     | Trainer Path, Type, Loading Mode, Version                                      | Yes (default open)                   | `launchMethod !== 'native'`              |
| **Launcher**    | Launcher Name, Launcher Icon                                                   | Yes (default closed)                 | `supportsTrainerLaunch && !reviewMode`   |
| **Diagnostics** | Health Issues, Version Status, Stale Info                                      | Yes (default open when issues exist) | When `selectedReport` has issues         |

**Key grouping decisions**:

- Environment Variables and ProtonDB stay together in one card because ProtonDB apply-env-vars flows directly into the env vars table
- AutoPopulate stays with Runtime because it fills Steam/Proton fields
- Launcher metadata gets its own small card because it's only relevant when exporting (low-frequency action)
- Diagnostics is promoted to a visible card rather than buried inside the form
- **Cover art slot in Core card**: Adjacent to game name / game path. When `steam.app_id` is set and art is cached, display the image. When no art is available, the slot collapses to zero height via `display: none` or conditional rendering (no empty placeholder boxes). The cover art slot does NOT use an empty placeholder in production -- it either shows art or is absent.

## Unified Phasing Strategy (Second Pass)

This is the key deliverable: a phasing plan that interleaves the original UI restructuring (first-pass Phases 0-3) with #52 game metadata/cover art so there is no rework.

### Phase 0: Component Cleanup + Image Cache Infrastructure -- Estimated 2 days

**Scope**: First-pass Phase 0 cleanup PLUS #52 backend infrastructure.

**UI Cleanup (from first pass)**:

1. **Deduplicate FieldRow / InstallField**: Replace `FieldRow` usages in `ProfileFormSections.tsx` with the existing `ui/InstallField.tsx` component (or unify the API if minor prop differences exist).
2. **Consolidate ProtonPathField**: Make `ui/ProtonPathField.tsx` the single canonical implementation. Extract `formatProtonInstallLabel` to a shared utility to break the circular import.
3. **Replace OptionalSection**: Swap `OptionalSection` for `CollapsibleSection defaultOpen={false}` to eliminate inconsistent inline styles.
4. **Verify all consumers**: Test `ProfilesPage` (full editor), `InstallPage` (review modal with `reviewMode`), and `OnboardingWizard` (type imports + independent component imports). Confirm all three code paths still work after deduplication.

**#52 Backend Infrastructure (new)**:

5. **SQLite migration (v14)**: Add `game_image_cache` table with columns: `cache_id TEXT PRIMARY KEY`, `steam_app_id TEXT NOT NULL`, `image_type TEXT NOT NULL` (header, capsule, hero, library, grid), `file_path TEXT NOT NULL`, `source_url TEXT NOT NULL`, `source TEXT NOT NULL` (steam, steamgriddb), `checksum TEXT`, `width INTEGER`, `height INTEGER`, `file_size_bytes INTEGER`, `expires_at TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`. Unique constraint on `(steam_app_id, image_type, source)`.
6. **Filesystem cache directory**: Create `~/.local/share/crosshook/cache/images/` with `0o700` permissions on startup (following `db.rs` pattern for metadata.db directory creation).
7. **Rust `GameImageStore` module**: New module in `crosshook-core/src/metadata/` with `put_image_cache_entry()`, `get_image_cache_entry()`, `evict_expired_image_entries()`. Pattern follows `cache_store.rs` exactly.
8. **Steam Store metadata cache key**: Use `steam:appdetails:v1:{app_id}` in `external_cache_entries`. Payload is the Steam Store API JSON (3-15 KiB, well within 512 KiB cap). TTL: 24 hours (game metadata changes less frequently than ProtonDB reports).
9. **`AppSettingsData` extension**: Add `steamgriddb_api_key: Option<String>` field with `#[serde(default)]`. This is a user-editable preference in `settings.toml`.

**Parallelization**: UI cleanup (tasks 1-4) and backend infrastructure (tasks 5-9) can run in parallel since they touch different parts of the codebase.

### Phase 1: Promote + Cards with Cover Art Slots -- Estimated 3-4 days

**Scope**: First-pass Phase 1 (remove Advanced, create cards) PLUS cover art layout slots designed from day one.

10. **Remove Advanced collapsible wrapper**: Unwrap the `CollapsibleSection` at `ProfilesPage.tsx:622-751`. Move its children to the page level.
11. **Create section cards**: Wrap each logical group in its own `CollapsibleSection` + `crosshook-panel`. Follow the grouping table above.
12. **Core card cover art slot**: Add a `<div className="crosshook-profile-cover-art">` element inside the Core card, positioned adjacent to the game name. CSS: `aspect-ratio: 460 / 215` for Steam header art (or `2 / 3` for portrait mode). **Critical**: This element is conditionally rendered -- it only appears when cover art is available. No empty placeholder boxes in production. During Phase 1, this element never renders (no backend wiring yet), but its CSS class is defined and tested with a mock image in development to validate the layout.
13. **Extract action bar**: Move `ProfileActions` from inside the former Advanced section to a dedicated bottom area. Consider making it sticky. **Security note**: ensure the Delete button and its two-step confirmation flow remain easily accessible.
14. **Promote Health Issues**: Move the health issues IIFE block (`ProfilesPage.tsx:689-750`) to its own top-level card that renders conditionally.
15. **Test all ProfileFormSections consumers**: Verify `InstallPage` review modal's `reviewMode` rendering and `OnboardingWizard` type/component imports are unaffected.
16. **Verify keyboard/controller navigation**: Ensure F2 rename, tab order, and focus zones work with the new layout.

### Phase 2: Steam API Integration + Art Display + Polish -- Estimated 3-4 days

**Scope**: First-pass Phase 2 polish PLUS #52 Steam Store API integration and art display.

**Steam API Integration (new for #52)**:

17. **Rust Steam Store client**: New module `crosshook-core/src/steam/store_api.rs` implementing `fetch_steam_app_details(app_id)`. Uses `reqwest` (already a dependency). Fetches `https://store.steampowered.com/api/appdetails?appids={id}`. Caches full JSON response in `external_cache_entries` with key `steam:appdetails:v1:{app_id}` and 24-hour TTL. Pattern follows `protondb/client.rs` exactly: try cache first, fetch live, fall back to stale cache on failure.
18. **Rust image downloader**: Function `download_and_cache_image(app_id, image_url, image_type)` that downloads an image to `~/.local/share/crosshook/cache/images/{app_id}/{image_type}.jpg`, records metadata in `game_image_cache` table, and returns the local filesystem path.
19. **Tauri commands**: New `steam_app_details` and `steam_app_cover_art` commands in `src-tauri/src/commands/steam.rs`. These are the IPC boundary. `steam_app_details` returns parsed metadata (name, description, genres, tags). `steam_app_cover_art` returns a local filesystem path to cached art (or triggers download).
20. **Frontend hook**: `useGameMetadata(appId)` hook that calls `steam_app_details` and `steam_app_cover_art` via `invoke()`. Returns `{ metadata, coverArtPath, loading, error }`. Follows the `useProtonDbLookup` pattern.
21. **Cover art display in Core card**: When `steam.app_id` is set and art is available, show the cached image in the Core card's cover art slot. Use Tauri's `convertFileSrc()` to convert the filesystem path to a `tauri://localhost/` URL loadable by `<img>`. Graceful degradation: if no art, slot is hidden; if art loading, show a subtle shimmer/skeleton; if art failed, log and hide slot.
22. **Steam metadata display**: Genre tags as `crosshook-status-chip` badges below the cover art. Short description as `crosshook-help-text`. These only render when metadata is available.

**Polish (from first pass)**:

23. **Add quick settings summary**: Create a compact metadata strip below the profile selector. Include game art thumbnail (if available), launch method badge, health status, ProtonDB rating badge.
24. **Sticky action footer**: CSS positioning for the action bar.
25. **Card header summaries**: Show collapsed-state summaries in card headers using `CollapsibleSection` `meta` prop.
26. **Launch method badges**: Visual indicator for profile type.

### Phase 3: Sub-Tabs + SteamGridDB -- Estimated 4-5 days

**Scope**: First-pass Phase 3 (ProfileFormSections split + sub-tabs) PLUS SteamGridDB optional integration.

**Sub-Tabs (from first pass)**:

27. **Refactor ProfileFormSections into composable section components**: Split the 1,144-line component into `ProfileIdentitySection`, `GameSection`, `RunnerMethodSection`, `TrainerSection`, `RuntimeSection`, `LauncherMetadataSection`. Use the already-extracted `CustomEnvironmentVariablesSection`, `GamescopeConfigPanel`, `MangoHudConfigPanel`, and `LaunchOptimizationsPanel` as the template for how section components should be structured (pure controlled props components).
28. **Add sub-tab navigation at ProfilesPage level**: Use `@radix-ui/react-tabs` (already installed) + existing `crosshook-subtab-row` / `crosshook-subtab` classes. Persist active tab in sessionStorage using key `crosshook.profilesActiveTab`. **Security constraint (W1)**: use CSS `display: none` for inactive tab panels instead of conditional rendering, to prevent data loss in `CustomEnvironmentVariablesSection`'s buffered local state. **Architectural constraint**: tabs wrap `ProfileFormSections` output at the `ProfilesPage` level -- they must NOT be embedded inside `ProfileFormSections` itself, to avoid breaking the `InstallPage` review modal.
29. **Handle conditional tabs**: Tab visibility based on launch method. Decide: disabled vs. hidden for Trainer/Launcher tabs when `launchMethod === 'native'`.
30. **Preserve all consumer reuse**: Ensure composable sections work in tabbed (`ProfilesPage`), linear review (`InstallPage` review modal), and independent import (`OnboardingWizard`) modes.

**SteamGridDB Integration (new for #52)**:

31. **Rust SteamGridDB client**: New module `crosshook-core/src/steam/steamgriddb.rs`. Requires `steamgriddb_api_key` from settings. Fetches grids/heroes for a given Steam App ID. Falls back to Steam Store art when API key is not configured or request fails.
32. **Image source preference**: Fallback chain: SteamGridDB (if API key configured) -> Steam Store API -> text-only. User can set preferred source per profile or globally in settings.
33. **Settings UI**: Add SteamGridDB API key field to the Settings panel. Input field with "Get API key" link to SteamGridDB website. Key is stored in `settings.toml` as `steamgriddb_api_key`.
34. **Tauri IPC updates**: Extend `steam_app_cover_art` command to accept an optional `preferred_source` parameter. If SteamGridDB is preferred and key is available, try that first.

### Phase 4: Library Grid View (Figma Concept) -- Estimated 2-3 days

**Scope**: The Figma concept's core deliverable -- a library-style grid of game cover art cards with overlaid actions, as an alternative browse mode for the Profiles page. This does NOT change the existing theme; it adds a new view mode alongside the current dropdown selector + form editor.

35. **Profile library grid component** (`ProfileLibraryGrid`): A responsive CSS grid of game cover art cards, using `grid-template-columns: repeat(auto-fill, minmax(var(--crosshook-profile-grid-min, 200px), 1fr))` with controller mode override for larger cards. Each card is a `<article>` element following the `crosshook-community-browser__profile-card` pattern but with cover art as the dominant visual. New CSS variable `--crosshook-profile-grid-min` in `variables.css` (default `200px`, controller mode `280px`).
36. **Game cover art card**: Each card shows: cover art image (landscape Steam header or portrait SteamGridDB grid), game name overlaid at the bottom with a gradient scrim for readability (`crosshook-profile-cover-art--gradient` using `::after` pseudo-element with `linear-gradient(transparent 40%, rgba(0,0,0,0.85))`), and action buttons. The card uses the existing `crosshook-card` glassmorphism styling for the container.
37. **Card action overlay**: Three actions accessible directly from each card -- **Launch** (primary, navigates to LaunchPage with profile pre-selected or triggers quick-launch), **Favorite/Pin** (toggle, reuses existing `onToggleFavorite` from `PinnedProfilesStrip` infrastructure), **Edit** (opens the profile in the form editor, which is the current card-based layout from Phase 1). Actions appear as icon buttons in a bottom bar or as a hover overlay. Controller mode: actions in a visible bottom bar (no hover on gamepad).
38. **Grid/list view toggle**: A toggle button or segmented control above the profile area that switches between **List view** (current dropdown + form editor) and **Grid view** (library grid). Persist selection in sessionStorage (`crosshook.profilesViewMode`). Default: list view (preserves existing behavior).
39. **Grid card metadata**: Below the cover art, show compact metadata: launch method badge, health status dot, ProtonDB tier badge, trainer type. Uses existing `crosshook-status-chip` and `crosshook-community-browser__meta-grid` patterns.
40. **Empty state for grid view**: When no profiles exist, show the same onboarding prompt as list view. When profiles exist but have no cover art, cards fall back to a solid color background with the game name centered (no broken image icons).
41. **Review and integrate community feedback**: By Phase 4, the card layout and sub-tabs have been in use. The grid view should be gated behind a feature flag or preference if there are concerns about readiness.

**Architectural note**: The grid view is a **browse** mode -- selecting a card opens the form editor (Phases 1-3). The grid does NOT replace the form editor. It replaces the dropdown selector as an alternative way to pick which profile to edit/launch. The existing `PinnedProfilesStrip` (horizontal chip strip) remains as a quick-access shortcut above both views.

**Relationship to existing code**: The `CommunityBrowser` already implements this exact pattern (`crosshook-community-browser__profile-grid` with auto-fit columns, `crosshook-community-browser__profile-card` articles with header/meta/chips/actions). The profile library grid can reuse the same CSS grid approach with profile-specific card content. The key difference is that profile cards have cover art as the primary visual and direct launch/edit/favorite actions, whereas community cards have text-only metadata and an import action.

## Figma Concept Integration: Library Grid System

The Figma concept is specifically about a **library grid system with game cover art cards** -- a visual browse mode where users see their profiles as a grid of game covers with launch, favorite, and edit actions accessible directly from each card. This is NOT a theme redesign. The existing CrossHook dark glassmorphism theme, BEM `crosshook-*` class system, CSS variables, and controller mode remain unchanged.

### What the Concept Is

- A responsive grid of game cover art cards (similar to Steam's library grid or Lutris's game list)
- Each card: cover art as primary visual, game name overlaid with gradient scrim, action buttons (launch, favorite, edit)
- Grid/list view toggle: switch between the library grid and the current form-editor view
- Controller-friendly: cards are large enough for gamepad selection, actions visible without hover

### What the Concept Is NOT

- Not a theme overhaul (no Tailwind, no new design system, no color changes)
- Not a replacement for the form editor (the grid is a browse/select mode; editing still uses the card-based form from Phase 1)
- Not a redesign of CommunityBrowser or other pages (only applies to the Profiles page)

### Existing Infrastructure to Reuse

The codebase already has a nearly identical grid card pattern in `CommunityBrowser`:

| Existing Pattern                       | CSS Location                                                                     | Reusable For                                |
| -------------------------------------- | -------------------------------------------------------------------------------- | ------------------------------------------- |
| Auto-fit responsive grid               | `crosshook-community-browser__profile-grid` (`theme.css:592`)                    | Profile library grid layout                 |
| Grid min-width CSS variable            | `--crosshook-community-profile-grid-min` (`variables.css:52`, controller `:93`)  | Profile grid min-width with controller mode |
| Glassmorphism article card             | `crosshook-community-browser__profile-card` (`theme.css:596`)                    | Profile cover art card container            |
| Card header with title + badge         | `crosshook-community-browser__profile-header` (`theme.css:606`)                  | Game name + health/rating badge on card     |
| Metadata grid (small font stats)       | `crosshook-community-browser__meta-grid` (`theme.css:635`)                       | Launch method, trainer type, ProtonDB tier  |
| Chip row for tags                      | `crosshook-community-browser__chip-row` (`theme.css:650`)                        | Genre tags on cover art cards               |
| Button row (actions)                   | `crosshook-community-browser__button-row` (`theme.css:658`)                      | Launch/Edit/Favorite actions per card       |
| Controller mode single-column override | `@media` query forcing `grid-template-columns: 1fr` (`theme.css:3254`)           | Controller mode for profile grid            |
| Status chips                           | `crosshook-status-chip`                                                          | Health, ProtonDB rating, launch method      |
| Pinned/favorite toggle                 | `crosshook-profile-pin-btn` (`theme.css:1439`) + `PinnedProfilesStrip` component | Favorite action on cover art card           |

### New CSS Needed

| Element                    | New CSS Class                           | Description                                                                                               | Phase |
| -------------------------- | --------------------------------------- | --------------------------------------------------------------------------------------------------------- | :---: |
| Cover art image            | `crosshook-profile-cover-art`           | `aspect-ratio: 460/215`, `object-fit: cover`, `border-radius` matching card, conditional rendering        |   1   |
| Gradient scrim on art      | `crosshook-profile-cover-art--gradient` | `::after` with `linear-gradient(transparent 40%, rgba(0,0,0,0.85))` for text over image readability       |   4   |
| Profile grid container     | `crosshook-profile-grid`                | `display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--crosshook-profile-grid-min), 1fr))` |   4   |
| Profile grid min-width var | `--crosshook-profile-grid-min`          | `200px` default, `280px` controller mode (new CSS variable in `variables.css`)                            |   4   |
| Image loading skeleton     | `crosshook-skeleton`                    | Keyframe shimmer animation for image loading state                                                        |   2   |
| View mode toggle           | `crosshook-profile-view-toggle`         | Segmented control for grid/list switch                                                                    |   4   |

### Interaction Model: Grid Card Actions

Each cover art card in the library grid exposes three actions:

1. **Launch** (primary): Navigates to the LaunchPage with this profile pre-selected and ready to launch. Currently, launching requires switching to the Launch tab -- the grid card provides a shortcut. Implementation: set the active profile via `selectProfile()` then navigate to the Launch tab.
2. **Favorite/Pin** (toggle): Reuses the existing `onToggleFavorite` infrastructure from `PinnedProfilesStrip` and `ProfileFormSections`. The star icon and `crosshook-profile-pin-btn` CSS already exist.
3. **Edit** (secondary): Selects the profile and switches to the form editor view (the card-based layout from Phase 1). This is the equivalent of choosing the profile from the dropdown.

**Controller mode behavior**: All three actions are visible in a bottom bar on each card (no hover state -- gamepad users cannot hover). Touch targets are 48px minimum per existing controller mode CSS variables.

### Aspirational (Future, Not Part of Current Plan)

- **Portrait card layout** (`2:3` aspect ratio): Only viable when SteamGridDB grid art (600x900) is available. Deferred beyond Phase 4.
- **Animated card transitions**: Spring animations on grid card selection -- requires `@radix-ui/react-accordion` or CSS `view-transitions`. Low value.
- **Parallax cover art**: Subtle depth effect on hover -- performance concern on Steam Deck, not usable with gamepad.
- **Full-bleed hero backgrounds**: Page-level hero image behind the form editor -- conflicts with the functional card layout.

## Component Callsite Map

`ProfileFormSections` is used in three distinct contexts. Any restructuring must account for all of them:

| Callsite             | File                   | Props                                         | Context                                                                                                                                                                                                     |
| -------------------- | ---------------------- | --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Full editor**      | `ProfilesPage.tsx:675` | No `reviewMode`, no `profileSelector`         | Primary editing UI -- the target of this refactor                                                                                                                                                           |
| **Review modal**     | `InstallPage.tsx:441`  | `reviewMode` set                              | Compact review step inside `ProfileReviewModal` after game installation. Tabs would be wrong UX here.                                                                                                       |
| **Type import only** | `OnboardingWizard.tsx` | N/A (imports `ProtonInstallOption` type only) | Wizard builds its own step-by-step form from individual components (`InstallField`, `ProtonPathField`, `CustomEnvironmentVariablesSection`, `AutoPopulate`). Not affected by `ProfileFormSections` changes. |

**Architectural constraint**: Tabs must live at the `ProfilesPage` level, wrapping `ProfileFormSections` output. They must NOT be embedded inside `ProfileFormSections` itself, because that would force a tabbed layout into the `InstallPage` review modal.

**Addendum from practices research**: Since `OnboardingWizard` already independently imports individual section components (`InstallField`, `ProtonPathField`, `CustomEnvironmentVariablesSection`, `AutoPopulate`), the Phase 3 section extraction still has value -- it gives the wizard a richer set of named, reusable building blocks without coupling it to a tab layout. But this benefit is independent of the tab decision.

## Component Deduplication (Prerequisite Cleanup)

**Critical finding from practices research**: `ProfileFormSections.tsx` contains private helper components that duplicate existing `ui/` components:

| Private component in `ProfileFormSections.tsx` | Existing `ui/` component    | Status                                                                                                                                                                                                                    |
| ---------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `FieldRow` (line 124, 10+ usages)              | `ui/InstallField.tsx`       | Functionally equivalent -- same label + input + browse + helpText + error pattern. `InstallField` has additional `browseMode`/`browseTitle`/`browseFilters` props and integrates `chooseFile`/`chooseDirectory` directly. |
| `ProtonPathField` (line 166)                   | `ui/ProtonPathField.tsx`    | Near-duplicate. The `ui/` version imports `formatProtonInstallLabel` from `ProfileFormSections` (circular-ish dependency). The private version accepts more explicit props (`label`, `onBrowse`).                         |
| `OptionalSection` (line 290)                   | `ui/CollapsibleSection.tsx` | Could be replaced with `CollapsibleSection defaultOpen={false}`. `OptionalSection` uses raw `<details>` with inconsistent inline styles.                                                                                  |

**Recommendation**: Before or during Phase 1, consolidate these:

1. Replace `FieldRow` usages with `InstallField` (or unify into a single shared component)
2. Consolidate the two `ProtonPathField` implementations -- the `ui/` version should be canonical
3. Replace `OptionalSection` with `CollapsibleSection defaultOpen={false}`

This reduces `ProfileFormSections.tsx` by ~100 lines and eliminates the inconsistent inline styles flagged by practices research. It also makes the Phase 3 section extraction cleaner since each section will use shared UI primitives rather than private copies.

## Security Constraints

### Overall Security Risk: LOW-MEDIUM

**Changed from first pass**: The first-pass risk was LOW (UI-only restructuring). Adding #52 introduces new external API calls, filesystem I/O for image caching, and a new SQLite table. This shifts to LOW-MEDIUM. No new Tauri capabilities are needed beyond those already available (the existing `core:default` + `shell:open-url` are sufficient; image downloads happen in the Rust backend, not the webview).

### Must Address (Warnings)

**W1: Component unmount data loss (sub-tabs Phase 3)**

- `CustomEnvironmentVariablesSection` buffers env var row edits in local React state (`useState<CustomEnvVarRow[]>`). If sub-tab navigation unmounts the component mid-edit, in-progress rows are silently discarded.
- **Mitigation (choose one)**:
  - **Preferred**: Use CSS `display: none` instead of conditional rendering for tab panels. All sections remain mounted; only visibility changes. This is the simplest fix with zero state management overhead.
  - **Alternative**: Add a `useEffect` cleanup hook (~5 lines) to flush buffered rows to the parent `onUpdateProfile` callback on unmount.
- **Applies to**: Phase 3 only. Phases 0-2 do not unmount form sections.

**W2: sessionStorage key namespace**

- Any new `sessionStorage` keys (e.g., for sub-tab state persistence) must use the `crosshook.` prefix to avoid collisions across browser contexts.
- Existing keys follow this pattern: `crosshook.healthBannerDismissed`, `crosshook.renameToastDismissed`.
- **Recommended key**: `crosshook.profilesActiveTab`.

**W3: `injection.*` fields must not be surfaced**

- `injection.dll_paths` and `injection.inject_on_launch` are present in `GameProfile` but intentionally absent from all form components. The community export sanitizer explicitly clears them (`exchange.rs:259`). Must not be exposed during any restructuring.

**W4: Steam Store API response validation (NEW for #52)**

- The Steam Store API returns user-generated content (game descriptions, tag names). This content must be text-rendered, never inserted as HTML. The existing codebase has zero `dangerouslySetInnerHTML` usage -- this must remain true.
- **Mitigation**: All metadata strings are rendered via React JSX text content (automatic escaping). Do not use `innerHTML` or `dangerouslySetInnerHTML` for any game description or tag rendering.

**W5: SteamGridDB API key exposure (NEW for #52)**

- The SteamGridDB API key is stored in `settings.toml` (plaintext on disk). This is consistent with how other user preferences are stored. The key is not particularly sensitive (it grants read-only access to a public art database), but it should not be logged, transmitted to analytics, or displayed in full in the UI.
- **Mitigation**: Mask the API key in the settings UI input field (type="password"). Do not include it in diagnostic bundle exports. Do not log it at any tracing level.

**W6: Filesystem image cache path traversal (NEW for #52)**

- The image cache stores files at `~/.local/share/crosshook/cache/images/{app_id}/{image_type}.jpg`. The `app_id` is user-controlled (from the profile's `steam.app_id` field). A malicious `app_id` like `../../../etc/` could cause path traversal.
- **Mitigation**: Validate `app_id` is a numeric string (digits only) before constructing the filesystem path. Reject any non-numeric app_id at the `GameImageStore` boundary. This mirrors the ProtonDB client's `normalize_app_id()` pattern which already validates app IDs.

### Preserved Security Strengths

The following existing security properties must be preserved through all phases:

- **Profile name path traversal prevention**: enforced in `validate_name()` at `toml_store.rs:497-521`. All filesystem ops gated through this. Not affected by UI restructuring.
- **Env var key validation**: client-side checks for `=`, NUL, reserved keys (`WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`) -- mirrors backend constraints. Must remain in `CustomEnvironmentVariablesSection` regardless of where it's rendered.
- **Delete confirmation two-step flow**: `confirmDelete` -> `executeDelete` with modal dialog prevents accidental data loss. Must remain accessible in the restructured layout -- not hidden behind a collapsed section or non-obvious tab.
- **No `dangerouslySetInnerHTML`**: Not used anywhere in the current codebase. Must not be introduced. **Especially critical for #52**: game descriptions from Steam API are user-generated content.
- **ProtonDB env var sanitization**: Backend `aggregation.rs:254-311` sanitizes env var suggestions before IPC. This pattern should be followed for any Steam metadata that could be applied to profile fields.

### Advisory (Non-Blocking Best Practices)

- **A1**: Path inputs (Game Path, Trainer Path) show no client-side validation feedback -- errors only surface on save from backend. Optional: add non-blocking advisory warnings for obviously malformed paths (empty, missing file extension). Backend remains authoritative.
- **A2**: Env var values have no client-side length cap. Optional: soft advisory display for values exceeding ~1000 characters. Not a blocker -- this is user self-inflicted.
- **A3 (NEW)**: Image cache disk usage has no upper bound. A user with 200 profiles could accumulate 1+ GB of cached images. Optional: add a cache size limit setting or automatic eviction of images not accessed in 30+ days. Not a launch blocker but should be addressed before v1.0.

## Creative Ideas

### Immediately Actionable (Phase 1)

1. **Profile type indicator**: Show a visual badge next to the profile name indicating "Steam", "Proton", or "Native" with appropriate coloring. Helps users immediately understand which sections are relevant.

2. **Section completion indicators**: Add small checkmarks or progress dots on each card header showing whether required fields are filled. Similar to the health badge pattern already in use.

3. **Sticky action footer**: Move Save/Delete/Duplicate/Rename buttons to a fixed-position footer bar. This solves the problem of scrolling to find the action buttons. The "unsaved changes" indicator becomes a persistent banner.

4. **Smart defaults on card collapse**: When a card is collapsed, show a one-line summary of its current state in the header (e.g., "Trainer: Aurora v1.2 (copy mode)" or "Runtime: Steam App 1245620, Proton Experimental"). Use the existing `CollapsibleSection` `meta` prop which already supports arbitrary ReactNode content.

### Phase 2 Actionable (#52 Integration)

5. **Cover art as visual anchor**: The cover art in the Core card serves as the primary visual identification for a profile. When users have 10+ profiles, art helps them find the right one faster than text-only profile names.

6. **Game metadata badges**: Genre tags (RPG, Action, etc.) as `crosshook-status-chip` elements below the cover art or in the summary bar. These provide at-a-glance context about the game without opening a browser.

7. **"No art available" graceful degradation**: When no cover art exists (native games, games not on Steam, API failure), the Core card simply omits the art slot. No broken image icons, no placeholder text. The layout adjusts naturally because the art element is conditionally rendered.

### Future Consideration (Phase 3+)

8. **Profile templates**: Pre-fill common configurations (e.g., "Steam + Aurora trainer", "Proton + WeMod", "Native Linux"). The `BundledOptimizationPreset` pattern in `types/profile.ts:76-83` already demonstrates this concept for launch optimizations -- extend it to full profiles.

9. **Comparison view**: Side-by-side diff of two profiles (the `ConfigHistoryPanel` and `fetchConfigDiff` already support TOML diff rendering -- reuse for profile-vs-profile comparison).

10. **Conditional section auto-expand**: When the user changes the Runner Method, automatically expand sections that become relevant and collapse those that become irrelevant. This provides contextual guidance without complex smart-settings infrastructure.

11. **Sub-tab state persistence**: If sub-tabs are added in Phase 3, the active tab should persist across page navigation. The `sessionStorage` pattern already exists for banners/toasts (`HEALTH_BANNER_DISMISSED_SESSION_KEY`, `RENAME_TOAST_DISMISSED_SESSION_KEY` in `ProfilesPage.tsx`). Use key `crosshook.profilesActiveTab` per W2.

12. **Radix Accordion upgrade** (optional): Replace the native `<details>` in `CollapsibleSection` with `@radix-ui/react-accordion` for animation support and `type="multiple"` (all sections open simultaneously). Same vendor as existing Radix dependencies, single-package add. Not required -- only if animated expand/collapse is desired.

13. **SteamGridDB custom art selection**: When SteamGridDB returns multiple art options for a game, show a gallery picker in the profile editor so users can choose their preferred image. This is a Phase 3+ polish item.

## Risk Assessment

### Low Risk

- **Card separation (A/C/D1)**: Pure visual restructuring. No logic changes. Easy to revert. No component unmount risk.
- **Sticky action footer**: CSS-only change with potential z-index edge cases.
- **Quick settings bar (D2)**: Additive component. No existing code modified.
- **Component deduplication**: Replacing private helpers with existing `ui/` components. Low risk if done incrementally.
- **Cover art CSS slot (Phase 1)**: Purely structural -- no backend, no API calls. Conditionally rendered.

### Medium Risk

- **Sub-tabs (B/D4)**: Conditional tab visibility, cross-section workflow breaks, `InstallPage` review modal reuse conflict. All solvable but require careful design. Risk is lower than initially assessed because all tab infrastructure already exists (Radix Tabs, CSS tokens, controller mode overrides). **Security constraint**: must use CSS show/hide for tab panels to prevent data loss in buffered components (W1). **Architectural constraint**: tabs must live at `ProfilesPage` level, not inside `ProfileFormSections`.
- **Refactoring ProfileFormSections into composable sections**: This component is 1,144 lines with shared internal state (ProtonDB overwrite flow, trainer info modal). Splitting it requires careful state hoisting. Practices research recommends 7 named section components: `ProfileIdentitySection`, `GameSection`, `RunnerMethodSection`, `RuntimeSection`, `TrainerSection`, `(EnvVars already done)`, `LauncherMetadataSection`.
- **Steam Store API integration (NEW)**: External dependency with no SLA. Rate limits undocumented. Responses can be slow. Mitigated by aggressive caching (24-hour TTL) and stale fallback pattern (matching ProtonDB client).
- **Image cache filesystem management (NEW)**: Disk usage grows with profile count. No eviction policy in initial implementation. Steam Deck has limited storage. Mitigated by per-image tracking in SQLite and planned cache eviction in Phase 4.

### Higher Risk

- **SteamGridDB integration**: Requires user-managed API key (friction). API could change or become unavailable. Adds second external API dependency. **Mitigated by**: deferring to Phase 3, making it fully optional with Steam Store as fallback, and storing key in settings.toml.
- **Drag-and-drop section reordering**: Requires new persistence layer, DnD library, complex state management. Over-engineering for ~15 form fields.
- **Contextual/smart settings**: Requires game metadata infrastructure that doesn't exist. Scope creep risk. (#52 partially addresses this by providing game genre metadata, but smart-settings should remain a future consideration.)
- **Search/filter for settings**: Inappropriate for a form with fewer than 20 fields. Adds cognitive overhead without proportional benefit.
- **shadcn/ui or Tailwind adoption**: Incompatible with existing CSS variable system. Would require complete restyling.
- **Full design system (MUI, Ant Design)**: ~500kB+ bundle bloat, opinionated theming conflicts with `crosshook-*` CSS.

### Cross-Cutting Risks

- **ProfileFormSections multi-consumer compatibility**: Changes to `ProfileFormSections` props or rendering affect both `ProfilesPage` (full editor) and `InstallPage` (review modal with `reviewMode`). The `OnboardingWizard` only imports the `ProtonInstallOption` type, not the component -- but it independently imports shared UI components (`InstallField`, `ProtonPathField`, `CustomEnvironmentVariablesSection`, `AutoPopulate`) that may be affected by deduplication work. Test all three code paths after changes.
- **Keyboard navigation**: The existing `F2` rename shortcut, tab focus management, and `data-crosshook-focus-root` / `data-crosshook-focus-zone` attributes must be preserved. Sub-tabs add another layer of keyboard navigation complexity -- but Radix Tabs handles this natively with arrow key navigation.
- **Controller mode**: The `ControllerPrompts` component suggests gamepad support. The sub-tab CSS tokens already have controller mode overrides (`variables.css:86-87`), which means D-pad navigation was considered in the design. Implementation should verify actual gamepad interaction works.
- **Circular dependency risk**: `ui/ProtonPathField.tsx` imports `formatProtonInstallLabel` from `ProfileFormSections.tsx`. If `ProfileFormSections` is split into section components, this import path must be updated. Consider extracting `formatProtonInstallLabel` to a utility module.
- **Delete confirmation accessibility**: The two-step delete flow (`confirmDelete` -> `executeDelete`) with modal dialog must remain easily accessible in the restructured layout. If sub-tabs are used, the Delete button must be on a persistent action bar, not hidden inside a tab.
- **External API latency (NEW)**: Steam Store API fetches add latency to profile loading. Must be non-blocking: profile form renders immediately, cover art loads asynchronously. ProtonDB lookup already demonstrates this pattern with loading/stale/unavailable states.
- **Offline behavior (NEW)**: Image cache must work offline. Cached images persist on disk indefinitely until a successful refresh. Metadata JSON has stale fallback in `external_cache_entries`. When offline: cached art shows, uncached art degrades to text-only, no errors surfaced for missing art.

### #52-Specific Risk Table

| Risk                                    | Likelihood | Impact | Mitigation                                                                                                   |
| --------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------ |
| Steam Store API rate limiting           | Medium     | Low    | 24-hour cache TTL; stale fallback; fetches only when `steam.app_id` is set (not on every render)             |
| Steam Store API down/unreachable        | Low        | Low    | Stale cache fallback; text-only degradation; no blocked profile load/launch                                  |
| Image cache disk bloat                  | Medium     | Medium | Per-image SQLite tracking; planned eviction policy (Phase 4); manual deletable cache directory               |
| SteamGridDB API key UX friction         | Medium     | Low    | Deferred to Phase 3; fully optional; Steam Store is default source                                           |
| Path traversal via malicious app_id     | Low        | High   | Numeric-only validation on app_id before filesystem path construction (mirrors `normalize_app_id()` pattern) |
| `dangerouslySetInnerHTML` with API data | Low        | High   | React JSX text content auto-escapes; code review gate; no innerHTML patterns in codebase                     |
| SteamGridDB API deprecation             | Low        | Low    | Optional integration; Steam Store is primary; no feature degradation without SteamGridDB                     |
| Image format incompatibility            | Low        | Low    | Steam serves JPEG/PNG; WebView2/WebKitGTK handle both natively                                               |

## Alternative Approaches

### Alt 1: Settings-Style Two-Column Layout

The `SettingsPanel` uses a two-column grid (`crosshook-settings-grid` with `crosshook-settings-grid-columns: minmax(0, 1fr) minmax(0, 1.1fr)`). The Profiles page could adopt this: left column for Core + Runtime fields, right column for Trainer + Environment + Diagnostics.

**Trade-off**: Better space utilization on wide screens, but responsive design becomes more complex. The existing `--crosshook-layout-main-columns` CSS variable (`minmax(0, 1.3fr) minmax(320px, 0.9fr)`) already defines a two-column layout at the app level -- nesting another two-column grid inside could feel cramped.

### Alt 2: Accordion-Only (No Cards)

Replace the single Advanced collapsible with multiple independent accordions (one per section). No visual card styling -- just expand/collapse toggles. Similar to the current `OptionalSection` pattern in `ProfileFormSections` (using `<details>` elements). Could optionally use `@radix-ui/react-accordion` for animation and `type="multiple"` support.

**Trade-off**: Simplest possible change. But without visual container boundaries, sections blur together when multiple are expanded.

### Alt 3: Modal-Based Editing for Secondary Sections

Keep Core fields always visible. Move Trainer, Environment, and Diagnostics into modal dialogs accessible via buttons. The codebase already has multiple modal patterns (`ProfilePreviewModal`, `ProfileReviewModal`, `OfflineTrainerInfoModal`, `CommunityImportWizardModal`).

**Trade-off**: Reduces page clutter dramatically but introduces modal fatigue. Editing environment variables in a modal (with the ProtonDB apply flow) would be awkward.

### Alt 4: Steam-Only-First (Deferred SteamGridDB)

Build the entire #52 scope with only Steam Store API support. No SteamGridDB at all. Add SteamGridDB as a separate issue/PR later.

**Trade-off**: Significantly reduces scope and external API surface. Steam Store API provides `header_image` (460x215) which is adequate for profile cards. SteamGridDB adds higher-quality art options but at the cost of API key management and a second external dependency. **This is the recommended approach for initial implementation.**

## Quick Wins (Updated for Second Pass)

### Immediate (can ship with Phase 0 or Phase 1)

- **Remove the Advanced wrapper**: Single biggest impact change -- promotes all content to always-visible
- **Move ProfileActions outside any collapsible**: Save/Delete always accessible
- **Promote health badges to profile selector bar**: Health status visible at a glance
- **Define cover art CSS class**: Even before backend wiring, define `crosshook-profile-cover-art` CSS class with proper aspect-ratio and object-fit rules. Costs nothing, prevents layout rework later.
- **Add `steamgriddb_api_key` to `AppSettingsData`**: Additive `Option<String>` field with serde default. Zero migration, zero risk.

### Phase 1 Quick Wins

- **Cover art placeholder styling**: CSS-only, no backend. Ensures the card layout accommodates art from day one.
- **Launch method badges in card headers**: Quick visual identification using existing `crosshook-status-chip`.

### Phase 2 Quick Wins

- **Steam header image in Core card**: The simplest #52 deliverable. Steam Store API's `header_image` URL is a direct JPEG link (no parsing). Download, cache, display.
- **Genre chips**: Trivial to render from `appdetails` JSON using existing chip CSS.

## Key Decisions Needed (Updated for Second Pass)

1. **Sticky action footer vs. inline actions**: Should Save/Delete/Rename be in a fixed footer or at the bottom of a scrollable area? Sticky footers are more discoverable but consume permanent screen real estate. Security note: Delete must remain accessible without extra navigation.

2. **Default collapse state for promoted cards**: Should Runtime, Trainer, and Environment cards default to open or closed? Recommendation: all default open for new profiles (no data yet = quick setup), default closed for existing profiles (user is likely editing one specific thing).

3. **Card ordering**: The recommended order (Core > Runtime > Environment > Trainer > Launcher > Diagnostics) follows the typical setup workflow. Should this be configurable? Recommendation: no, not in Phase 1.

4. **Sub-tabs timeline**: Phase 3 should be planned given the design system's existing sub-tab infrastructure. The question is whether to schedule it immediately after Phase 2 or wait for user feedback on the card-based approach.

5. **Scope of ProfileFormSections refactor**: Phase 0 does the component deduplication cleanup in `ProfileFormSections`. Phase 1 changes only `ProfilesPage.tsx` -- the form component continues to render sections linearly, and the page-level code wraps them in cards. Phase 3 splits `ProfileFormSections` into composable section components but layers tabs at `ProfilesPage` only -- the `InstallPage` review modal and wizard continue using the linear form or individual components.

6. **Launcher tab behavior for native profiles**: Should the Launcher tab show a disabled state, or be hidden entirely? Hiding is simpler; disabled state adds accessibility complexity but is natively handled by Radix Tabs.

7. **Sub-tab state persistence**: Should the active sub-tab persist across page navigation via sessionStorage? Recommended yes, following the existing pattern for banner/toast dismissal state. Use namespaced key `crosshook.profilesActiveTab` per W2.

8. **Tab panel rendering strategy**: CSS show/hide (preferred for data safety per W1) vs. conditional rendering (lighter DOM but risks data loss). Recommendation: CSS show/hide.

9. **(NEW) Steam-only vs. dual-source for initial #52 launch**: Should Phase 2 ship with Steam Store API only, deferring SteamGridDB to Phase 3? **Recommendation: Yes.** Steam-only-first reduces scope, eliminates API key friction, and provides adequate cover art (460x215 header images). SteamGridDB adds value (higher-res art, custom art) but at the cost of a second external API dependency. Building the image cache with a `source` column from Phase 0 ensures SteamGridDB can be added later without migration.

10. **(NEW) Cover art aspect ratio**: Steam header images are 460x215 (landscape, ~2.14:1). SteamGridDB grids are 600x900 (portrait, 2:3). Library capsules are 600x900 (portrait). Should the default card layout use landscape or portrait? **Recommendation: Landscape (Steam header) for Phase 2** -- it fits naturally in a horizontal card layout. Portrait option as Phase 4 polish when SteamGridDB is available.

11. **(NEW) Image cache eviction policy**: Should there be an automatic cache cleanup? **Recommendation: Not in initial implementation.** Track file sizes in `game_image_cache` table. Add a "Clear image cache" button in Settings (Phase 3). Implement automatic eviction (e.g., LRU, 500 MB cap) only if disk usage becomes a reported issue. The `evict_expired_image_entries()` function handles TTL-based expiry.

12. **(NEW) Cover art in InstallPage review modal**: Should the review modal also show cover art? **Recommendation: No.** The review modal is a compact confirmation step. Adding cover art would bloat it. The `reviewMode` flag already gates content; cover art should be gated by `!reviewMode`.

## Open Questions

1. Is the `OnboardingWizard` staying long-term, or will the card-based layout make it redundant for editing? (It still has value for guided creation, and it independently imports individual components rather than using `ProfileFormSections`.)

2. Are there plans for additional profile sections (e.g., DLL injection config is in the data model at `GameProfile.injection` but not rendered in the current form)? If so, the card-based layout accommodates this better than tabs.

3. Should the Launcher Export section (`ProfilesPage.tsx:801-813`) be absorbed into the profile editor cards, or remain a separate top-level section? It currently lives outside the former Advanced area.

4. The `ProtonDbLookupCard` was recently added (untracked files in git status). Should its placement be finalized before or during this UI refactor?

5. Controller mode implications: The sub-tab CSS tokens already have controller mode overrides. Does the gamepad navigation system need explicit updates beyond what Radix Tabs provides for keyboard/focus management?

6. Is `ui/InstallField.tsx` already the intended replacement for the private `FieldRow`? If yes, why wasn't it used in `ProfileFormSections` -- was this an intentional split for the wizard context, or an oversight?

7. Should `@radix-ui/react-accordion` be added for animated section expand/collapse, or is the native `<details>` element in `CollapsibleSection` sufficient?

8. **(NEW)** What is the Steam Store API rate limit? The API documentation does not specify explicit limits. Community reports suggest ~200 requests/minute before throttling. With 24-hour caching, a user would need 200+ profiles loaded simultaneously to hit this. Is this a realistic concern?

9. **(NEW)** Should cover art be shown in the CommunityBrowser profile cards as well? The `CommunityBrowser` already has `crosshook-community-browser__profile-card` styling. Adding cover art there would require fetching metadata for all indexed community profiles, which could be expensive. Recommend deferring to a separate issue.

10. **(NEW)** Should the `game_image_cache` table support multiple images per game (header, capsule, hero, library)? **Recommendation: Yes** -- design the table for multiple image types from the start. Phase 2 only uses `header`, but Phase 3+ (SteamGridDB) will use `grid`, `hero`, and `library` types.

## Persistence & Usability (Second Pass Addition)

### Datum Classification

| Datum                                                       | Layer                                                                                   | Reasoning                                                                                     |
| ----------------------------------------------------------- | --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `steam_app_id`                                              | TOML profile (existing `[steam] app_id`)                                                | Already exists; no change                                                                     |
| Steam Store metadata JSON (name, description, genres, tags) | SQLite `external_cache_entries` (key: `steam:appdetails:v1:{app_id}`)                   | Payload ~3-15 KiB; within 512 KiB cap; TTL 24h                                                |
| Cover art / hero image binaries                             | Filesystem `~/.local/share/crosshook/cache/images/{app_id}/` + `game_image_cache` table | Images 80 KB-2 MB exceed `MAX_CACHE_PAYLOAD_BYTES`; filesystem + DB metadata is correct split |
| SteamGridDB API key                                         | `settings.toml` (`AppSettingsData.steamgriddb_api_key`)                                 | User-editable preference                                                                      |
| Image fetch/display state                                   | Runtime-only (memory)                                                                   | Ephemeral UI state                                                                            |
| Sub-tab active state                                        | Runtime-only (`useState` in ProfilesPage)                                               | Optional sessionStorage persistence per session (key: `crosshook.profilesActiveTab`)          |
| Card collapse state                                         | Runtime-only (no persistence)                                                           | Cards default open on page load; no need to persist collapse state                            |

### Migration/Backward Compatibility

- **Phase 0 migration (v14)**: Adds `game_image_cache` table. Additive -- users without it have no cover art but all existing functionality unaffected. No data loss on upgrade.
- **`AppSettingsData` extension**: New `steamgriddb_api_key` field with `#[serde(default)]`. Existing settings files without it deserialize cleanly. No migration needed.
- **Profile data**: No changes to TOML storage. All existing persistence mechanisms unchanged. `steam.app_id` is already in profiles.

### Offline Behavior

- **Metadata JSON**: Available as stale fallback in `external_cache_entries` (matching ProtonDB pattern). When offline, stale metadata is shown; no error surfaced.
- **Cached images**: Persist on filesystem indefinitely until a successful refresh replaces them. Cards show cached art offline.
- **Without cache**: Cards degrade to text-only. No broken image icons. No blocked profile load/launch.
- **SteamGridDB unavailable or unconfigured**: Fall back to Steam Store API art. No art at all -> hidden art slot.

### Degraded Fallback Chain

```
SteamGridDB art (if API key configured)
  |-- fallback --> Steam Store API header_image
                     |-- fallback --> Stale cached image (from previous successful fetch)
                                       |-- fallback --> Hidden art slot (text-only card)
```

At no point does a missing image block profile functionality.

## Cross-References

- Practices research: `docs/plans/ui-enhancements/research-practices.md`
- External APIs/libraries: `docs/plans/ui-enhancements/research-external.md`
- Security research: `docs/plans/ui-enhancements/research-security.md`
- Technical design: `docs/plans/ui-enhancements/research-technical.md`
- UX research: `docs/plans/ui-enhancements/research-ux.md`
- Business analysis: `docs/plans/ui-enhancements/research-business.md`
- Issue #52: Game metadata and cover art via Steam API / SteamGridDB
- Implementation guide: `docs/research/additional-features/implementation-guide.md` (Issue #52 Storage Boundary Note)
- ProtonDB client pattern: `crates/crosshook-core/src/protondb/client.rs` (reference for cache-first + stale-fallback)
- Cache store: `crates/crosshook-core/src/metadata/cache_store.rs` (reference for `external_cache_entries` operations)
