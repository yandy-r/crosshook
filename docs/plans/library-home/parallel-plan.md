# Library Home Implementation Plan

The library-home feature adds a Steam-like poster art grid as CrossHook's default landing page, replacing the profiles route as startup. Implementation spans Rust backend (new `GameImageType::Portrait` variant with 3-URL fallback chain, new `profile_list_summaries` batch IPC), frontend route wiring (4-file `AppRoute` extension), and 4 new React components (`LibraryPage`, `LibraryCard`, `LibraryGrid`, `LibraryToolbar`). The plan is organized into 10 tasks across 4 phases with 5-way parallelism in Phase 1 — the critical path runs through Portrait Rust variant → IPC match + hook param → LibraryCard → LibraryPage integration. Zero new npm dependencies are needed.

## Critically Relevant Files and Documentation

- docs/plans/library-home/feature-spec.md: Complete feature specification with all resolved decisions, data models, CSS layout, and phasing
- docs/plans/library-home/shared.md: Architecture overview, relevant files, patterns, and documentation references
- docs/plans/library-home/research-technical.md: Exact Rust code samples, line numbers, and API design for Portrait variant and profile_list_summaries
- docs/plans/library-home/research-patterns.md: React page patterns, CSS conventions, hook architecture, skeleton loading, and component decomposition rules
- docs/plans/library-home/research-ux.md: Card design spec, gradient scrim WCAG values, hover-reveal pattern, empty-state design, competitive analysis
- docs/plans/library-home/research-integration.md: IPC signatures, SQLite schema (v14), cover art pipeline data flow, portrait URL fallback chain
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx: Best page pattern reference — onNavigate prop with await selectProfile before navigation
- src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx: Skeleton/fallback rendering pattern using useGameCoverArt
- src/crosshook-native/src/components/PinnedProfilesStrip.tsx: Optimistic heart toggle pattern reference
- src/crosshook-native/src/styles/theme.css: crosshook-skeleton class + crosshook-skeleton-shimmer keyframe (~line 4738); crosshook-community-browser\_\_profile-grid auto-fill grid (~line 997)

## Implementation Plan

### Phase 1: Foundation (all tasks independent — maximum parallelism)

#### Task 1.1: Add GameImageType::Portrait Rust variant with fallback chain — Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/library-home/research-technical.md (§ Required backend change — new GameImageType::Portrait variant)
- docs/plans/library-home/research-integration.md (§ Cover Art Pipeline, § GameImageType Enum)
- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/game_images/models.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/client.rs
- src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs

Add `Portrait` variant to `GameImageType` enum in `models.rs`. Update `Display` impl: `Self::Portrait => write!(f, "portrait")`. The `#[serde(rename_all = "snake_case")]` derive means it serializes as `"portrait"` automatically.

In `client.rs`, `build_download_url()` returns a single URL — but Portrait needs a 3-URL fallback chain. Add a helper function `portrait_candidate_urls(app_id: &str) -> Vec<String>` returning `[library_600x900_2x.jpg, library_600x900.jpg, header.jpg]`. Modify `download_and_cache_image` to detect `GameImageType::Portrait` and iterate candidate URLs, skipping 404s, stopping at first success. Other image types continue using the single-URL path unchanged. Add `Portrait` arm to `filename_for()`: `GameImageType::Portrait => "portrait"`.

In `steamgriddb.rs`, add `Portrait` to `build_endpoint()`: `GameImageType::Portrait => ("grids", Some("342x482,600x900"))` — same portrait-oriented dimensions.

**Gotcha 1**: `build_download_url` is not async — the candidate URL iteration must happen in the async `download_and_cache_image` function. Do not change the signature of `build_download_url` — other types must not regress.

**Gotcha 2**: `download_and_cache_image` calls `build_download_url()` at **two** points — line ~213 (SteamGridDB fallback path) and line ~230 (no API key path). The Portrait fallback chain must replace both call sites. Patching only one silently breaks cover art when SteamGridDB fails or when no API key is configured.

After changes, run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

#### Task 1.2: Add profile_list_summaries Rust IPC command — Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/library-home/research-technical.md (§ New command — Option A)
- docs/plans/library-home/research-integration.md (§ New IPC Command Required)
- src/crosshook-native/src-tauri/src/commands/profile.rs (profile_list pattern at line 222)
- src/crosshook-native/src-tauri/src/lib.rs (invoke_handler registration at line 189)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/src-tauri/src/lib.rs

Add `ProfileSummary` struct to `profile.rs` (derive `Debug, Clone, Serialize, Deserialize` with `#[serde(rename_all = "camelCase")]` so JSON keys match TypeScript camelCase conventions):

```rust
#[serde(rename_all = "camelCase")]
pub struct ProfileSummary {
    pub name: String,
    pub game_name: String,
    pub steam_app_id: String,
    pub custom_cover_art_path: Option<String>,
}
```

Add `profile_list_summaries` command — sync (`pub fn`, not async), takes `State<'_, ProfileStore>`, calls `store.list()` then `store.load(&name)` per entry, maps to `ProfileSummary`, returns `Result<Vec<ProfileSummary>, String>`. Register in `lib.rs` `invoke_handler` macro near the other `profile_*` commands (~line 252).

After changes, run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

#### Task 1.3: Create library types and useLibraryProfiles hook — Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/library-home/feature-spec.md (§ Data Models — LibraryCardData, LibraryViewMode)
- src/crosshook-native/src/types/profile.ts (GameProfile interface for field reference)

**Instructions**

Files to Create

- src/crosshook-native/src/types/library.ts
- src/crosshook-native/src/hooks/useLibraryProfiles.ts

Create `types/library.ts` with:

```typescript
export type LibraryViewMode = 'grid' | 'list';

export interface LibraryCardData {
  name: string;
  gameName: string;
  steamAppId: string;
  customCoverArtPath?: string;
  isFavorite: boolean;
}
```

Create `hooks/useLibraryProfiles.ts` — a pure transform hook (no IPC, no context). Takes `summaries: LibraryCardData[]` and `searchQuery: string`, returns filtered `LibraryCardData[]`. Filter is case-insensitive substring match on both `name` and `gameName`. No favorites sorting for Phase 1 (favorites mixed alphabetically per decision #5). Source order from IPC is already alphabetical.

#### Task 1.4: Wire library route into AppRoute system — Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/library-home/research-patterns.md (§ Route Registration Pattern)
- src/crosshook-native/src/components/layout/Sidebar.tsx (AppRoute union at line 13, SIDEBAR_SECTIONS at line 33, ROUTE_LABELS at line 58)
- src/crosshook-native/src/App.tsx (VALID_APP_ROUTES at line 19, default route at line 43)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/layout/Sidebar.tsx
- src/crosshook-native/src/components/icons/SidebarIcons.tsx
- src/crosshook-native/src/App.tsx

Add a `LibraryIcon` SVG component to `src/crosshook-native/src/components/icons/SidebarIcons.tsx` (simple grid/shelf icon matching existing icon conventions — functional component with `SVGProps<SVGSVGElement>`).

In `Sidebar.tsx:13`, extend `AppRoute` union with `| 'library'`. Add `library: 'Library'` to `ROUTE_LABELS` record. Import `LibraryIcon` from `SidebarIcons` and add library item to `SIDEBAR_SECTIONS` in the Game section, before Profiles: `{ route: 'library', label: 'Library', icon: LibraryIcon }`.

In `App.tsx:19`, add `library: true` to `VALID_APP_ROUTES`. At line 43, change `useState<AppRoute>('profiles')` to `useState<AppRoute>('library')`.

**Gotcha**: The TypeScript `never` exhaustive guard in `ContentArea.tsx` will produce a compile error after this change until Task 2.4 adds the `case 'library'` arm. This is expected — TypeScript enforces completeness.

#### Task 1.5: Create CSS variables and library.css stylesheet — Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/library-home/research-ux.md (§ Card Design, § Glass Morphism Buttons, § Gradient Scrim)
- src/crosshook-native/src/styles/variables.css (existing CSS custom properties)
- src/crosshook-native/src/styles/theme.css (~line 997 for community grid pattern, ~line 4738 for skeleton)

**Instructions**

Files to Create

- src/crosshook-native/src/styles/library.css

Files to Modify

- src/crosshook-native/src/styles/variables.css
- src/crosshook-native/src/main.tsx

Add three CSS variables to `variables.css` (after existing grid/layout tokens):

```css
--crosshook-library-card-width: 190px;
--crosshook-library-card-aspect: 3 / 4;
--crosshook-library-grid-gap: var(--crosshook-grid-gap);
```

Create `library.css` with BEM `crosshook-library-*` classes:

- `.crosshook-library-page` — page container
- `.crosshook-library-toolbar` — toolbar row (search + view toggle), flex layout
- `.crosshook-library-toolbar__search` — search input styling
- `.crosshook-library-grid` — CSS Grid: `display: grid; grid-template-columns: repeat(auto-fill, minmax(var(--crosshook-library-card-width), 1fr)); gap: var(--crosshook-library-grid-gap); align-content: start`
- `.crosshook-library-card` — `position: relative; overflow: hidden; border-radius: var(--crosshook-radius-md); background: var(--crosshook-color-surface); cursor: pointer`
- `.crosshook-library-card__image` — `aspect-ratio: var(--crosshook-library-card-aspect); width: 100%; object-fit: cover; display: block`
- `.crosshook-library-card__scrim` — `position: absolute; inset: 0; background: linear-gradient(to top, rgba(0,0,0,0.85) 0%, transparent 50%); pointer-events: none`
- `.crosshook-library-card__footer` — `position: absolute; bottom: 0; left: 0; right: 0; padding: 10px 10px 8px`
- `.crosshook-library-card__title` — `font-weight: 600; font-size: 0.85rem; line-height: 1.3; text-overflow: ellipsis; white-space: nowrap; overflow: hidden; color: var(--crosshook-color-text)`
- `.crosshook-library-card__actions` — `display: flex; gap: 6px; opacity: 0; transition: opacity var(--crosshook-transition-fast) ease`
- `.crosshook-library-card:hover .crosshook-library-card__actions, .crosshook-library-card:focus-within .crosshook-library-card__actions` — `opacity: 1`
- `.crosshook-library-card__btn--launch` — filled blue button (`background: var(--crosshook-color-accent); flex: 1`)
- `.crosshook-library-card__btn--glass` — `background: rgba(255,255,255,0.12); backdrop-filter: blur(8px); border: 1px solid rgba(255,255,255,0.18); border-radius: 6px`
- `.crosshook-library-card__favorite-badge` — top-right corner heart indicator (persistent when favorited)
- `.crosshook-library-card__fallback` — dark gradient placeholder with centered game initials
- `.crosshook-library-empty` — empty state container with centered CTA

In `main.tsx`, add `import './styles/library.css';` after existing CSS imports.

### Phase 2: Bridge Layer (parallel after Phase 1)

#### Task 2.1: Add portrait IPC match arm and useGameCoverArt imageType parameter — Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/game_metadata.rs (current match at line 25-28)
- src/crosshook-native/src/hooks/useGameCoverArt.ts (hardcoded imageType at line 42)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/game_metadata.rs
- src/crosshook-native/src/hooks/useGameCoverArt.ts

In `game_metadata.rs`, add `"portrait" => GameImageType::Portrait,` **before** the `_ => GameImageType::Cover` catch-all arm. If placed after the `_`, portrait requests silently fall through to Cover.

In `useGameCoverArt.ts`, add an optional third parameter `imageType?: string` to the function signature. Change line ~44 from `imageType: 'cover'` to `imageType: imageType ?? 'cover'`. Also add `imageType` to the `useCallback` dependency array for `fetchCoverArt` (currently `[normalizedAppId]` — update to `[normalizedAppId, imageType]`) so changing imageType at runtime retriggers the fetch. All existing callers omit the third param and continue defaulting to `'cover'`.

After Rust change, run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

#### Task 2.2: Create LibraryToolbar component — Depends on [1.3, 1.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/library.ts (LibraryViewMode type from Task 1.3)
- src/crosshook-native/src/styles/library.css (toolbar classes from Task 1.5)

**Instructions**

Files to Create

- src/crosshook-native/src/components/library/LibraryToolbar.tsx

Create a pure controlled component with props: `searchQuery: string`, `onSearchChange: (q: string) => void`, `viewMode: LibraryViewMode`, `onViewModeChange: (mode: LibraryViewMode) => void`. No IPC, no context — purely presentational.

Renders: search `<input>` with `type="search"`, `placeholder="Search games..."`, `aria-label="Search games"`. Grid/list toggle with two `<button>` elements using grid/list icons (SVG inline or Lucide-style), with `aria-pressed` for active state. Apply `crosshook-library-toolbar` and sub-element BEM classes.

#### Task 2.3: Add LibraryArt SVG to PageBanner — Depends on [1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/layout/PageBanner.tsx (existing per-route SVG art pattern)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/layout/PageBanner.tsx

Read `PageBanner.tsx` to understand the existing pattern (inline SVG in exported named functions using shared `SVG_DEFAULTS`). Add a `LibraryArt` SVG component — a simple grid/shelf illustration matching the visual style of existing banner art components. Wire it to the `'library'` route case in the page banner rendering logic.

### Phase 3: Core Card Component

#### Task 3.1: Create LibraryCard component — Depends on [1.3, 1.5, 2.1]

**READ THESE BEFORE TASK**

- docs/plans/library-home/research-ux.md (§ Card Design, § Gradient Scrim, § Glass Morphism Buttons, § Skeleton Loading)
- docs/plans/library-home/research-patterns.md (§ Component Decomposition Rule)
- src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx (skeleton/fallback render pattern)
- src/crosshook-native/src/components/PinnedProfilesStrip.tsx (optimistic heart toggle pattern)
- src/crosshook-native/src/hooks/useGameCoverArt.ts (hook API with imageType param from Task 2.1)

**Instructions**

Files to Create

- src/crosshook-native/src/components/library/LibraryCard.tsx

Create a **pure-props-driven component** (no context access). Props interface:

```typescript
interface LibraryCardProps {
  profile: LibraryCardData;
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  isLaunching?: boolean;
}
```

Implementation details:

1. Call `useGameCoverArt(profile.steamAppId, profile.customCoverArtPath, 'portrait')` for cover art
2. While `loading` is true, render a `<div className="crosshook-library-card__image crosshook-skeleton" />` shimmer placeholder
3. When `coverArtUrl` is null and not loading, render fallback: dark gradient div with 2-char game initials (`gameName` first 2 chars or `name` first 2 chars) centered in brand accent color
4. When `coverArtUrl` is available, render `<img>` with `loading="lazy"`, `className="crosshook-library-card__image"`, `alt={profile.gameName}`
5. Gradient scrim overlay: `<div className="crosshook-library-card__scrim" />`
6. Footer with title: `<span className="crosshook-library-card__title">{profile.gameName || profile.name}</span>`
7. Favorite badge in top-right (persistent when `profile.isFavorite`): filled heart icon with `aria-label="Favorited"`
8. Action row (hover-reveal via CSS): Launch button (filled blue, `crosshook-library-card__btn--launch`), Heart toggle (glass morphism, `crosshook-library-card__btn--glass`), Edit (glass morphism)
9. ARIA: `role="listitem"` on card root; `aria-label="Launch {gameName}"`, `aria-pressed={profile.isFavorite}` on heart, `aria-label="Edit {gameName}"` on edit button

Optional but recommended for Phase 1: Add `IntersectionObserver` to gate `useGameCoverArt` invocation — only start fetching when the card enters the viewport. Pattern:

```typescript
const [visible, setVisible] = useState(false);
const ref = useRef<HTMLDivElement>(null);
useEffect(() => {
  const obs = new IntersectionObserver(([e]) => {
    if (e.isIntersecting) setVisible(true);
  });
  if (ref.current) obs.observe(ref.current);
  return () => obs.disconnect();
}, []);
const { coverArtUrl, loading } = useGameCoverArt(
  visible ? profile.steamAppId : undefined,
  profile.customCoverArtPath,
  'portrait'
);
```

### Phase 4: Integration

#### Task 4.1: Create LibraryGrid, LibraryPage, and wire ContentArea — Depends on [1.2, 1.3, 1.4, 1.5, 2.2, 3.1]

**READ THESE BEFORE TASK**

- docs/plans/library-home/research-patterns.md (§ Page Component Pattern, § Context Consumption Pattern)
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx (page with onNavigate pattern)
- src/crosshook-native/src/context/ProfileContext.tsx (provides profiles, favoriteProfiles, selectProfile, toggleFavorite)
- src/crosshook-native/src/components/layout/ContentArea.tsx (renderPage switch, never exhaustive guard)

**Instructions**

Files to Create

- src/crosshook-native/src/components/library/LibraryGrid.tsx
- src/crosshook-native/src/components/pages/LibraryPage.tsx

Files to Modify

- src/crosshook-native/src/components/layout/ContentArea.tsx

**LibraryGrid** — stateless layout wrapper:

```typescript
interface LibraryGridProps {
  profiles: LibraryCardData[];
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  launchingName?: string;
}
```

Maps `profiles` to `LibraryCard` instances. Apply `crosshook-library-grid` CSS class. `role="list"` on container. When `profiles.length === 0`, render empty state: centered illustration + "No game profiles yet" heading + "Create your first profile" CTA button that calls `onNavigate?.('profiles')`.

**LibraryPage** — state owner, follows `HealthDashboardPage` pattern:

```typescript
interface LibraryPageProps {
  onNavigate?: (route: AppRoute) => void;
}
```

- Destructure from `useProfileContext()`: `profiles`, `favoriteProfiles`, `selectProfile`, `toggleFavorite`, `refreshProfiles`
- State: `summaries: LibraryCardData[]` (from `profile_list_summaries` IPC), `searchQuery: string`, `viewMode: LibraryViewMode` (persisted to localStorage key `'crosshook.library.viewMode'`), `launchingName: string | undefined`
- Define TypeScript interface for IPC response (matches Rust `ProfileSummary` with `#[serde(rename_all = "camelCase")]`):

  ```typescript
  interface ProfileSummary {
    name: string;
    gameName: string;
    steamAppId: string;
    customCoverArtPath?: string;
  }
  ```

- On mount: call `invoke<ProfileSummary[]>('profile_list_summaries')`, merge with `favoriteProfiles` to build `LibraryCardData[]`:

  ```typescript
  const favoriteSet = new Set(favoriteProfiles);
  const cards: LibraryCardData[] = summaries.map((s) => ({
    name: s.name,
    gameName: s.gameName,
    steamAppId: s.steamAppId,
    customCoverArtPath: s.customCoverArtPath,
    isFavorite: favoriteSet.has(s.name),
  }));
  ```

  Also call `refreshProfiles()` to ensure the list is current.

- Filter: `const filtered = useLibraryProfiles(summaries, searchQuery)`
- Launch handler: `async (name) => { setLaunchingName(name); await selectProfile(name); onNavigate?.('launch'); setLaunchingName(undefined); }`
- Edit handler: `async (name) => { await selectProfile(name); onNavigate?.('profiles'); }`
- Favorite handler: `(name, current) => void toggleFavorite(name, !current)` — optimistic UI: immediately update local `summaries` state, revert on error
- Root class: `crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--library`
- Render: `<LibraryToolbar ... />` then `<LibraryGrid ... />`
- Re-sync summaries when `profiles` array changes (listen to `profiles` from context — it updates on `profiles-changed` Tauri event)

**ContentArea.tsx** — add import and case:

```typescript
import LibraryPage from '../pages/LibraryPage';
// In renderPage():
case 'library':
  return <LibraryPage onNavigate={onNavigate} />;
```

This resolves the TypeScript `never` exhaustive guard compile error from Task 1.4.

After all changes, run `npx tsc --noEmit --pretty` to verify no TypeScript errors.

## Advice

- **Critical path is RUST-1 → Task 2.1 → Task 3.1 → Task 4.1.** Prioritize the Portrait Rust variant above all else — it unblocks the entire component chain. RUST-2, TS-1, TS-2, and CSS-1 can all run in parallel with RUST-1.

- **`build_download_url()` returns a single URL but Portrait needs 3 candidates.** Do NOT change `build_download_url`'s signature — that would affect Cover/Hero/Capsule. Instead, add a separate `portrait_candidate_urls()` helper and branch on `image_type == Portrait` inside `download_and_cache_image` to use the fallback loop. This is the most architecturally significant change in the entire plan.

- **The `never` exhaustive guard in ContentArea.tsx will produce a compile error between Task 1.4 and Task 4.1.** This is intentional TypeScript safety — do NOT suppress with `@ts-ignore`. The error resolves when Task 4.1 adds the `case 'library'` arm. Same applies to `VALID_APP_ROUTES` and `ROUTE_LABELS` records.

- **`useGameCoverArt` hook change (Task 2.1) is surgical: one new optional parameter, one string literal change.** All 3+ existing callers (`GameCoverArt.tsx`, `PinnedProfilesStrip.tsx`, ProfileSubTabs) continue working unchanged since they omit the third arg.

- **Favorites are NOT pinned to top of grid in Phase 1** (decision #5: mixed alphabetical). The `useLibraryProfiles` hook does client-side search filtering only — no sort manipulation. A separate favorites filter/tab comes later.

- **`await selectProfile(name)` before EVERY `onNavigate` call — non-negotiable.** Without the await, the target page renders with stale/null profile data. See `HealthDashboardPage.tsx:826` for the exact pattern. Both Launch and Edit handlers in LibraryPage must follow this.

- **Card actions use hover-reveal (decision #3).** Buttons are invisible by default and appear on `:hover` and `:focus-within`. The gradient scrim intensifies on hover. This keeps poster art clean while remaining keyboard/gamepad accessible via `:focus-within`.

- **Schema version is v14, not v13** (integration-researcher confirmed: migration 13→14 creates `game_image_cache`). No migration needed for Phase 1 — `game_image_cache.image_type` is free-form TEXT, so `"portrait"` works as-is.

- **`profile_list_summaries` is sync (`pub fn`), not async.** It only reads local TOML files — matching the pattern of `profile_list` and `profile_load`. Do not use `pub async fn` unless wrapping with `spawn_blocking`.

- **IntersectionObserver in LibraryCard is optional for Phase 1 but strongly recommended.** Without it, a 50-profile library fires 50 concurrent `fetch_game_cover_art` IPC calls on mount. The observer defers cover art fetching to visible cards only, with zero new dependencies.
