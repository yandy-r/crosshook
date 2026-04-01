# Phase 1 Task Analysis: Library Home

## Executive Summary

Phase 1 decomposes cleanly into **10 tasks across 4 execution waves**. The first wave is 5 fully parallel tasks; nothing blocks simultaneously. The critical path runs through: `GameImageType::Portrait` (Rust) → Portrait IPC match + `useGameCoverArt` param → `LibraryCard` → `LibraryPage`. All other work can be front-loaded in Wave 1 to minimize critical-path blocking.

One non-obvious architectural concern: `build_download_url()` returns a single `String`, but `Portrait` requires a 3-URL fallback chain (`library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg`). The RUST-1 task must handle this — either by returning `Vec<String>` from a new helper or by adding inline fallback retry logic in `download_and_cache_image`. This is the most complex single change in Phase 1.

---

## Recommended Phase Structure

### Wave 1 — Foundation (5 tasks, fully parallel)

| ID     | Name                                 | Files Touched                                                             | Scope  |
| ------ | ------------------------------------ | ------------------------------------------------------------------------- | ------ |
| RUST-1 | Portrait Rust variant + IPC match    | `models.rs`, `client.rs`, `steamgriddb.rs`, `commands/game_metadata.rs`   | Medium |
| RUST-2 | `profile_list_summaries` IPC         | `profile/models.rs` (or `toml_store.rs`), `commands/profile.rs`, `lib.rs` | Small  |
| TS-1   | Library types + `useLibraryProfiles` | `types/library.ts`, `hooks/useLibraryProfiles.ts`                         | Small  |
| TS-2   | Route wiring                         | `Sidebar.tsx`, `App.tsx`, `icons/SidebarIcons.tsx`                        | Small  |
| CSS-1  | CSS setup                            | `styles/variables.css`, `styles/library.css`, `main.tsx`                  | Small  |

### Wave 2 — Bridge layer (3 tasks, parallel after Wave 1)

| ID     | Name                                | Files Touched                           | Unblocked By |
| ------ | ----------------------------------- | --------------------------------------- | ------------ |
| TS-3   | `useGameCoverArt` `imageType` param | `hooks/useGameCoverArt.ts`              | RUST-1       |
| COMP-2 | `LibraryToolbar`                    | `components/library/LibraryToolbar.tsx` | TS-1, CSS-1  |
| COMP-3 | `PageBanner` library art            | `components/layout/PageBanner.tsx`      | TS-2         |

### Wave 3 — Core components (1 task)

| ID     | Name          | Files Touched                        | Unblocked By      |
| ------ | ------------- | ------------------------------------ | ----------------- |
| COMP-1 | `LibraryCard` | `components/library/LibraryCard.tsx` | TS-1, CSS-1, TS-3 |

### Wave 4 — Integration (1 task)

| ID     | Name                                                 | Files Touched                                           | Unblocked By                              |
| ------ | ---------------------------------------------------- | ------------------------------------------------------- | ----------------------------------------- |
| COMP-4 | `LibraryGrid` + `LibraryPage` + `ContentArea` wiring | `LibraryGrid.tsx`, `LibraryPage.tsx`, `ContentArea.tsx` | TS-1, TS-2, CSS-1, COMP-1, COMP-2, RUST-2 |

---

## Task Granularity Recommendations

### RUST-1: `GameImageType::Portrait` + fallback chain + IPC match arm

**Files:**

- `src/crosshook-native/crates/crosshook-core/src/game_images/models.rs`
- `src/crosshook-native/crates/crosshook-core/src/game_images/client.rs`
- `src/crosshook-native/crates/crosshook-core/src/game_images/steamgriddb.rs`
- `src/crosshook-native/src-tauri/src/commands/game_metadata.rs`

**Dependencies:** None

**Why `game_metadata.rs` ships here:** Rust's exhaustive match enforcement ensures `models.rs` + `client.rs` + `steamgriddb.rs` compile together. `game_metadata.rs` matches on the _string_ `image_type.as_deref()` (not on `GameImageType` directly), so it is not subject to Rust's exhaustive check — it would compile fine without the `"portrait"` arm. However, omitting it causes a **silent behavioral regression**: `fetch_game_cover_art` called with `imageType: 'portrait'` falls through to `Cover`, returning landscape art. Shipping all four files atomically prevents this class of bug.

**Scope:** Medium — non-trivial due to fallback chain

**What to do:**

1. Add `Portrait` variant to `GameImageType` enum (after `Capsule`). The `#[serde(rename_all = "snake_case")]` derive means it serializes as `"portrait"` automatically.
2. Add `Portrait` arm to `fmt::Display` impl: `Self::Portrait => write!(f, "portrait")`.
3. In `client.rs`, `build_download_url()` returns a single URL — but Portrait needs three candidates. **Recommended approach:** add a `build_portrait_fallback_urls(app_id: &str) -> Vec<String>` helper that returns `[library_600x900_2x.jpg, library_600x900.jpg, header.jpg]` URLs. Modify `download_and_cache_image` (or add a wrapper) to iterate candidates, skipping 404s, stopping at first success. The other types continue to use the single-URL path.
4. Add `Portrait` to `filename_for()`: `GameImageType::Portrait => "portrait"` prefix.
5. In `steamgriddb.rs`, add `Portrait` to `build_endpoint()` with `dimensions=342x482,600x900` (portrait-oriented dimensions).
6. In `game_metadata.rs:28`, add `"portrait" => GameImageType::Portrait` **before** the `_ => GameImageType::Cover` arm:

   ```rust
   "portrait" => GameImageType::Portrait,
   _ => GameImageType::Cover,
   ```

**Gotcha — do not change `build_download_url`'s return type.** It is called at 3 sites in `client.rs` (lines ~213, ~230, ~289) for Cover/Hero/Capsule paths. Changing its signature breaks all three callers. The correct Portrait implementation:

1. Add `Portrait` arm to `filename_for()` (simple match arm — `"portrait"` prefix).
2. Add `fn portrait_cdn_candidates(app_id: &str) -> Vec<String>` returning the three candidate URLs in priority order: `library_600x900_2x.jpg`, `library_600x900.jpg`, `header.jpg`.
3. Add `async fn try_download_portrait_from_cdn(app_id: &str) -> Result<(Vec<u8>, String), GameImageError>` — loops candidates, returns `(bytes, used_url)` on first HTTP 200, continues on error/404.
4. In `download_and_cache_image`, branch early on `image_type == GameImageType::Portrait` to call `try_download_portrait_from_cdn`. The returned `used_url` is stored as `download_url` in the DB upsert at step (g). All non-Portrait types hit the existing single-URL flow unchanged.

This keeps `build_download_url` untouched and RUST-1 + RUST-2 remain parallel (they touch different files).

---

### RUST-2: `ProfileSummary` DTO + `profile_list_summaries` IPC

**Files:**

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — add `ProfileSummary` struct
- `src/crosshook-native/src-tauri/src/commands/profile.rs` — add `profile_list_summaries` command
- `src/crosshook-native/src-tauri/src/lib.rs` — register in `invoke_handler!` macro

**Dependencies:** None

**Scope:** Small (~25 lines of new Rust)

**What to do:**

1. In `profile/models.rs`, add:

   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ProfileSummary {
       pub name: String,
       pub game_name: String,
       pub steam_app_id: String,
       pub custom_cover_art_path: String,
   }
   ```

2. In `commands/profile.rs`, add a `#[tauri::command]` function that:
   - Takes `State<'_, ProfileStore>`
   - Calls `store.list()` to get profile names
   - For each name calls `store.load(&name)` to get `GameProfile`
   - Maps to `ProfileSummary { name, game_name: profile.game.name, steam_app_id: profile.steam.app_id, custom_cover_art_path: profile.game.custom_cover_art_path }`
   - Returns `Result<Vec<ProfileSummary>, String>`
3. In `lib.rs`, add `commands::profile::profile_list_summaries` to the `generate_handler!` macro (pattern: look at line ~217 for existing `commands::profile::*` entries).

**Gotcha:** `store.load()` reads TOML from disk per profile — acceptable for the typical use case (< 50 profiles). Not async-gated; this is synchronous local I/O matching the pattern of other profile commands.

---

### TS-1: Library types + `useLibraryProfiles` hook

**Files:**

- `src/crosshook-native/src/types/library.ts` — new file
- `src/crosshook-native/src/hooks/useLibraryProfiles.ts` — new file

**Dependencies:** None

**Scope:** Small

**What to do:**

1. Create `types/library.ts` with `LibraryViewMode` and `LibraryCardData` exactly as specified in `feature-spec.md` § Data Models.
2. Create `hooks/useLibraryProfiles.ts` — a pure transform hook (no IPC). Signature:

   ```ts
   function useLibraryProfiles(summaries: LibraryCardData[], searchQuery: string): LibraryCardData[];
   ```

   Filters case-insensitively on `gameName` and `name`. No sort for MVP (alphabetical is preserved from source order). Returns filtered array — no side effects, no context access.

**Gotcha:** Do NOT add `isFavorite` filtering in this hook for Phase 1 (favorites remain mixed-in per decision #5). The hook is filter-only.

---

### TS-2: Route wiring (Sidebar + App + LibraryIcon)

**Files:**

- `src/crosshook-native/src/components/layout/Sidebar.tsx`
- `src/crosshook-native/src/App.tsx`
- `src/crosshook-native/src/components/icons/SidebarIcons.tsx` (or wherever sidebar icons live)

**Dependencies:** None (TypeScript type changes only — LibraryPage is not imported here)

**Internal ordering within this task:** `SidebarIcons.tsx` (create `LibraryIcon`) must be done **before** `Sidebar.tsx` changes, since `Sidebar.tsx` imports `LibraryIcon`. These are in the same task but the icon comes first.

**Why ContentArea is NOT in this task:** `ContentArea.tsx` imports `LibraryPage`, which doesn't exist yet. Adding `'library'` to `AppRoute` without adding the ContentArea case is intentional — the `never` guard will fail TypeScript compilation until COMP-4 resolves it. Do not add the ContentArea case here; that belongs in COMP-4.

**Scope:** Small

**What to do:** 0. `SidebarIcons.tsx` — add `LibraryIcon` SVG component first (prerequisite for step 4).

1. `Sidebar.tsx:13` — extend `AppRoute` union:

   ```ts
   export type AppRoute =
     | 'library'
     | 'profiles'
     | 'launch'
     | 'install'
     | 'community'
     | 'compatibility'
     | 'settings'
     | 'health';
   ```

2. Add `ROUTE_LABELS['library'] = 'Library'` to the `ROUTE_LABELS` record.
3. Add a `LibraryIcon` SVG component to `SidebarIcons`. Use a simple grid/shelf icon SVG. Match the existing icon conventions (functional component with `SVGProps<SVGSVGElement>`).
4. Import `LibraryIcon` and add to `SIDEBAR_SECTIONS` under the `'Game'` section:

   ```ts
   { route: 'library', label: 'Library', icon: LibraryIcon }
   ```

   Place it first in the 'Game' section (before 'Profiles').

5. `App.tsx:19` — add `library: true` to `VALID_APP_ROUTES`.
6. `App.tsx:43` — change `useState<AppRoute>('profiles')` → `useState<AppRoute>('library')`.

**Gotcha:** TypeScript's exhaustive `never` guard in `ContentArea.tsx` will produce a compile error until COMP-4 adds the `'library'` case. This is expected and correct — it enforces completion. Run `cargo build` only after COMP-4 is also done.

---

### CSS-1: CSS variables + `library.css` + `main.tsx` import

**Files:**

- `src/crosshook-native/src/styles/variables.css`
- `src/crosshook-native/src/styles/library.css` — new file
- `src/crosshook-native/src/main.tsx`

**Dependencies:** None

**Scope:** Small

**What to do:**

1. `variables.css` — add three tokens (find the existing grid/layout tokens section for placement):

   ```css
   --crosshook-library-card-width: 190px;
   --crosshook-library-card-aspect: 3 / 4;
   --crosshook-library-grid-gap: var(--crosshook-grid-gap);
   ```

2. `library.css` — create BEM-prefixed stylesheet with:
   - `.crosshook-library-page` — page container
   - `.crosshook-library-toolbar` — toolbar row (search + view toggle)
   - `.crosshook-library-grid` — CSS Grid `repeat(auto-fill, minmax(var(--crosshook-library-card-width), 1fr))` with `gap: var(--crosshook-library-grid-gap)`
   - `.crosshook-library-card` — card with `aspect-ratio: var(--crosshook-library-card-aspect)`; `position: relative; overflow: hidden; border-radius: var(--crosshook-radius-md)`
   - `.crosshook-library-card__image` — `width: 100%; height: 100%; object-fit: cover`
   - `.crosshook-library-card__scrim` — `linear-gradient(to top, rgba(0,0,0,0.85) 0%, transparent 50%)`; positioned absolute bottom
   - `.crosshook-library-card__title` — truncate with `text-overflow: ellipsis; white-space: nowrap; overflow: hidden`
   - `.crosshook-library-card__actions` — hover-reveal row; visible on `:hover` and `:focus-within`
   - `.crosshook-library-card__fallback` — dark gradient placeholder with initials
   - `.crosshook-library-empty` — empty state container
3. `main.tsx` — add `import './styles/library.css';` after existing CSS imports.

**Gotcha:** Check `theme.css` line ~997 for the `crosshook-community-browser__profile-grid` pattern to match grid conventions. Check `theme.css` line ~4738 for `crosshook-skeleton` to reuse shimmer animation by class name (do not duplicate the keyframe).

---

### TS-3: `useGameCoverArt` `imageType` parameter

**Files:**

- `src/crosshook-native/src/hooks/useGameCoverArt.ts`

**Dependencies:** RUST-1 (Portrait variant + `game_metadata.rs` match arm must ship first)

**Scope:** Trivial (~5 lines)

**Note:** `game_metadata.rs` was moved into RUST-1 (ships atomically with the Portrait Rust variant to prevent silent Cover fallback). This task is now purely the TypeScript hook side.

**What to do:**

1. `useGameCoverArt.ts` — add optional `imageType?` third parameter. New signature:

   ```ts
   export function useGameCoverArt(
     steamAppId: string | undefined,
     customCoverArtPath?: string,
     imageType?: 'cover' | 'portrait' | 'hero' | 'capsule'
   ): UseGameCoverArtResult;
   ```

2. At line 44, replace hardcoded `imageType: 'cover'` with `imageType: imageType ?? 'cover'`.

**Gotcha:** Existing callers of `useGameCoverArt` omit `imageType` entirely — they will continue to default to `'cover'` with no change needed. Verify no existing call sites break by checking that the parameter is strictly optional (`?`).

---

### COMP-1: `LibraryCard` component

**Files:**

- `src/crosshook-native/src/components/library/LibraryCard.tsx` — new file

**Dependencies:** TS-1 (`LibraryCardData`), CSS-1 (BEM classes + variables), TS-3 (`useGameCoverArt` `imageType` param)

**Scope:** Medium

**What to do:**
Implement a pure-props-driven component. No context access — all data and callbacks come from props.

```ts
interface LibraryCardProps {
  profile: LibraryCardData;
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  isLaunching?: boolean; // brief spinner while selectProfile resolves
}
```

Card structure:

- Renders cover art via `useGameCoverArt(profile.steamAppId, profile.customCoverArtPath, 'portrait')`
- Shows `crosshook-skeleton` shimmer div while `loading` is true
- Shows fallback div with game initials (2 chars from `gameName`) when `coverArtUrl` is null and not loading
- Gradient scrim overlay at bottom
- Title label (`profile.gameName || profile.name`) with `text-overflow: ellipsis`
- Favorite badge (heart icon) in top-right corner — always visible when `profile.isFavorite`
- Action row (Launch, Heart toggle, Edit) — appears on `:hover`/`:focus-within`
- `aria-label` on each button: `"Launch {gameName}"`, `"Toggle favorite for {gameName}"`, `"Edit {gameName}"`
- Heart button: `aria-pressed={profile.isFavorite}`
- `role="listitem"` on card root

Reference: `src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx` for skeleton/fallback pattern. `src/crosshook-native/src/components/PinnedProfilesStrip.tsx` for optimistic heart toggle pattern.

**Quick wins to bundle here:**

- Add `loading="lazy"` to the `<img>` element — prevents eager decode of off-screen images.
- Add `IntersectionObserver` to gate `useGameCoverArt` invocation: only start fetching cover art when the card enters the viewport. This prevents N parallel CDN requests on mount for large libraries (identified as a High-likelihood risk in `feature-spec.md` Risk table). Pattern: `const [visible, setVisible] = useState(false); const ref = useRef<HTMLDivElement>(null); useEffect(() => { const obs = new IntersectionObserver(([e]) => { if (e.isIntersecting) setVisible(true); }); obs.observe(ref.current!); return () => obs.disconnect(); }, []); const { coverArtUrl, loading } = useGameCoverArt(visible ? profile.steamAppId : undefined, ...)`

---

### COMP-2: `LibraryToolbar` component

**Files:**

- `src/crosshook-native/src/components/library/LibraryToolbar.tsx` — new file

**Dependencies:** TS-1 (`LibraryViewMode`), CSS-1 (BEM classes)

**Scope:** Small

**What to do:**

```ts
interface LibraryToolbarProps {
  searchQuery: string;
  onSearchChange: (q: string) => void;
  viewMode: LibraryViewMode;
  onViewModeChange: (mode: LibraryViewMode) => void;
}
```

- Search `<input>` with `type="search"`, `placeholder="Search games…"`, `aria-label="Search games"`
- Grid/list toggle: two `<button>` elements (grid icon, list icon) with `aria-pressed` state
- No IPC, no context — purely controlled component

---

### COMP-3: `PageBanner` library art

**Files:**

- `src/crosshook-native/src/components/layout/PageBanner.tsx`

**Dependencies:** TS-2 (`'library'` in `AppRoute`)

**Scope:** Small

**What to do:**
Read `PageBanner.tsx` first to understand the existing per-route SVG art pattern. Add a `LibraryArt` SVG component and wire it to the `'library'` route case. Match the visual style of existing banner art components.

---

### COMP-4: `LibraryGrid` + `LibraryPage` + `ContentArea` wiring

**Files:**

- `src/crosshook-native/src/components/library/LibraryGrid.tsx` — new file
- `src/crosshook-native/src/components/pages/LibraryPage.tsx` — new file
- `src/crosshook-native/src/components/layout/ContentArea.tsx` — add `'library'` case

**Dependencies:** TS-1, TS-2, CSS-1, COMP-1, COMP-2, RUST-2

**Scope:** Medium — integration task

**What to do:**

**LibraryGrid** — stateless layout:

```ts
interface LibraryGridProps {
  profiles: LibraryCardData[];
  onLaunch: (name: string) => void;
  onEdit: (name: string) => void;
  onToggleFavorite: (name: string, current: boolean) => void;
  launchingName?: string;
}
```

Maps `profiles` to `LibraryCard` instances. Applies `crosshook-library-grid` CSS class. `role="list"` on container. Empty state: render `<LibraryEmpty />` (inline empty state component or subcomponent) when `profiles.length === 0`.

**LibraryPage** — state owner, follows `HealthDashboardPage` pattern:

```ts
interface LibraryPageProps {
  onNavigate?: (route: AppRoute) => void;
}
```

- `const { profiles, favoriteProfiles, selectProfile, toggleFavorite, refreshProfiles } = useProfileContext()`
- `const [summaries, setSummaries] = useState<LibraryCardData[]>([])`
- On mount: call `invoke<ProfileSummary[]>('profile_list_summaries')`, merge with `favoriteProfiles` to build `LibraryCardData[]`, then `refreshProfiles()`
- `const [searchQuery, setSearchQuery] = useState('')`
- `const [viewMode, setViewMode] = useLocalStorage<LibraryViewMode>('crosshook-library-view-mode', 'grid')` (use `localStorage` directly or a thin hook)
- `const filtered = useLibraryProfiles(summaries, searchQuery)`
- Launch handler: `async (name) => { setLaunchingName(name); await selectProfile(name); onNavigate?.('launch'); setLaunchingName(undefined); }`
- Edit handler: `async (name) => { await selectProfile(name); onNavigate?.('profiles'); }`
- Render: `<LibraryToolbar ... /> <LibraryGrid ... />`

**ContentArea.tsx** — add one case:

```ts
case 'library':
  return <LibraryPage onNavigate={onNavigate} />;
```

Add import: `import LibraryPage from '../pages/LibraryPage';`
This resolves the TypeScript `never` exhaustive guard error from TS-2.

---

## Dependency DAG

```
RUST-1 ──────────────────────────────────────────────────────────── TS-3 ──► COMP-1 ──►
RUST-2 ──────────────────────────────────────────────────────────────────────────────── COMP-4
TS-1  ────────────────────────────────────────────── COMP-2 ──────────────────────────► COMP-4
                                                 └── COMP-1 (via TS-3) ──────────────► COMP-4
TS-2  ────────────────────────────────────────────── COMP-3 (independent)
                                                 └────────────────────────────────────► COMP-4
CSS-1 ────────────────────────────────────────────── COMP-1, COMP-2 ─────────────────► COMP-4
```

**Critical path:** `RUST-1` (Portrait variant + `game_metadata.rs` match arm) → `TS-3` (`useGameCoverArt` param) → `COMP-1` → `COMP-4`

All other tasks should be started immediately to avoid blocking the critical path.

---

## File-to-Task Mapping

| File                                            | Task                                            |
| ----------------------------------------------- | ----------------------------------------------- |
| `crosshook-core/src/game_images/models.rs`      | RUST-1                                          |
| `crosshook-core/src/game_images/client.rs`      | RUST-1                                          |
| `crosshook-core/src/game_images/steamgriddb.rs` | RUST-1                                          |
| `src-tauri/src/commands/game_metadata.rs`       | RUST-1 (ships atomically with Portrait variant) |
| `crosshook-core/src/profile/models.rs`          | RUST-2                                          |
| `src-tauri/src/commands/profile.rs`             | RUST-2                                          |
| `src-tauri/src/lib.rs`                          | RUST-2                                          |
| `src/hooks/useGameCoverArt.ts`                  | TS-3                                            |
| `src/types/library.ts`                          | TS-1                                            |
| `src/hooks/useLibraryProfiles.ts`               | TS-1                                            |
| `src/components/layout/Sidebar.tsx`             | TS-2                                            |
| `src/App.tsx`                                   | TS-2                                            |
| `src/components/icons/SidebarIcons.tsx`         | TS-2                                            |
| `src/styles/variables.css`                      | CSS-1                                           |
| `src/styles/library.css`                        | CSS-1 (new)                                     |
| `src/main.tsx`                                  | CSS-1                                           |
| `src/components/library/LibraryCard.tsx`        | COMP-1 (new)                                    |
| `src/components/library/LibraryToolbar.tsx`     | COMP-2 (new)                                    |
| `src/components/layout/PageBanner.tsx`          | COMP-3                                          |
| `src/components/library/LibraryGrid.tsx`        | COMP-4 (new)                                    |
| `src/components/pages/LibraryPage.tsx`          | COMP-4 (new)                                    |
| `src/components/layout/ContentArea.tsx`         | COMP-4                                          |

---

## Parallelization Opportunities

**Maximum parallel slots per wave:**

| Wave | Parallel Tasks | Notes                                                                        |
| ---- | -------------- | ---------------------------------------------------------------------------- |
| 1    | 5              | RUST-1, RUST-2, TS-1, TS-2, CSS-1                                            |
| 2    | 3              | TS-3 (waits on RUST-1), COMP-2 (waits on TS-1+CSS-1), COMP-3 (waits on TS-2) |
| 3    | 1              | COMP-1 (waits on TS-3)                                                       |
| 4    | 1              | COMP-4 (final integration)                                                   |

**Wave 2 optimization:** COMP-2 and COMP-3 are both small and can be dispatched as soon as Wave 1 completes (they don't need TS-3). TS-3 is also fast (~10 lines). So Wave 2 may complete before COMP-2/COMP-3 if those are slower — COMP-2 and COMP-3 do not gate COMP-4 independently from COMP-1, which is the real bottleneck.

**Lowest-risk parallelization order for two implementors:**

- Implementor A: RUST-1 → TS-3 → COMP-1 (critical path — keep unblocked)
- Implementor B: RUST-2 + TS-1 + CSS-1 + TS-2 → COMP-2 → COMP-3 → COMP-4

---

## Implementation Strategy

### Pre-Implementation Checklist

Before any implementation begins:

- [ ] Read `docs/plans/library-home/research-technical.md` (Rust changes: exact line numbers for `build_download_url` arms)
- [ ] Read `docs/plans/library-home/research-patterns.md` (React component patterns, hook conventions)
- [ ] Read `docs/plans/library-home/research-integration.md` (IPC signatures, SQLite schema confirmation)
- [ ] Read `docs/plans/library-home/research-ux.md` (card design spec, gradient scrim values, empty state)

### Verification After Rust Changes

After completing RUST-1 and RUST-2 (or any Rust change):

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

### TypeScript Exhaustive Guard

The `never` guard in `ContentArea.tsx` will produce a TypeScript compile error after TS-2 adds `'library'` to `AppRoute` but before COMP-4 adds the `case 'library'` arm. This is intentional — TypeScript enforces that all routes are handled. Do not suppress this error with `@ts-ignore`. It resolves when COMP-4 is complete.

### Key Constraints Summary

| Constraint                                                | Source                              | Impact                                                                     |
| --------------------------------------------------------- | ----------------------------------- | -------------------------------------------------------------------------- |
| `build_download_url` returns single URL                   | `client.rs:334`                     | RUST-1 must add fallback-loop path for Portrait                            |
| `fetch_game_cover_art` `_ =>` catch-all defaults to Cover | `game_metadata.rs:28`               | Portrait arm must be before the `_ =>` fallback                            |
| `useGameCoverArt` hardcodes `imageType: 'cover'`          | `useGameCoverArt.ts:44`             | TS-3 adds optional param; existing callers unaffected                      |
| `LibraryCard` must be pure-props (no context)             | `shared.md` pattern rule            | Enforces testability and composability                                     |
| `await selectProfile(name)` before `onNavigate`           | `shared.md` pattern rule            | Race condition guaranteed otherwise — never skip                           |
| No `localStorage` for search query (ephemeral)            | `feature-spec.md` persistence table | `viewMode` uses localStorage; `searchQuery` is React state only            |
| No SQLite migration in Phase 1                            | `feature-spec.md` persistence       | `game_image_cache.image_type` is free-form text — `'portrait'` works as-is |
