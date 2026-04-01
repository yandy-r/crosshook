# Library Home — Codebase Patterns Research

## Overview

CrossHook's frontend is a Tauri v2 app with a Radix UI tab-based shell, React contexts for global state, and a BEM-like `crosshook-*` CSS class system driven entirely by CSS variables in `variables.css`. All backend interaction uses Tauri `invoke()`, wrapped in custom hooks. Pages are thin orchestrators that pull from contexts and compose sub-components; business logic lives in hooks and `crosshook-core` (Rust). The library-home feature can follow the `HealthDashboardPage` pattern (page with `onNavigate` prop) and reuse `useGameCoverArt`, `useImageDominantColor`, `ProfileContext.toggleFavorite`, and the `crosshook-skeleton` shimmer class directly.

---

## Relevant Files

| File | Description |
|------|-------------|
| `src/crosshook-native/src/components/layout/Sidebar.tsx` | Defines `AppRoute` union type and `SIDEBAR_SECTIONS` array; owns `SidebarSectionItem` interface |
| `src/crosshook-native/src/App.tsx` | `VALID_APP_ROUTES` record; default route `useState<AppRoute>('profiles')`; provider tree |
| `src/crosshook-native/src/components/layout/ContentArea.tsx` | `renderPage()` switch; `forceMount`; `Tabs.Content key={route}` scroll-reset pattern |
| `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx` | Best pattern match: page with `onNavigate?: (route: AppRoute) => void` prop, navigates via `await selectProfile → onNavigate?.()` |
| `src/crosshook-native/src/components/pages/InstallPage.tsx` | Second example of `onNavigate` prop; calls `onNavigate?.('profiles')` after save |
| `src/crosshook-native/src/components/pages/LaunchPage.tsx` | Page that consumes `useProfileContext()` directly; no `onNavigate` needed |
| `src/crosshook-native/src/context/ProfileContext.tsx` | `ProfileProvider` → `useProfileContext()`; exposes `profiles`, `favoriteProfiles`, `selectProfile`, `toggleFavorite`, `refreshProfiles` |
| `src/crosshook-native/src/hooks/useProfile.ts` | The canonical hook: `UseProfileResult` interface, `refreshProfiles`, `toggleFavorite`, `favoriteProfiles` |
| `src/crosshook-native/src/hooks/useGameCoverArt.ts` | `useGameCoverArt(steamAppId, customCoverArtPath)` → `{ coverArtUrl: string|null, loading: boolean }` |
| `src/crosshook-native/src/hooks/useImageDominantColor.ts` | `useImageDominantColor(imageUrl)` → `[r,g,b] | null`; canvas-based; lightweight |
| `src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx` | Existing consumer of `useGameCoverArt`; shows skeleton/fallback pattern |
| `src/crosshook-native/src/components/PinnedProfilesStrip.tsx` | Reusable pattern: favorite toggle with explicit `(name, boolean)` signature; keyboard-accessible |
| `src/crosshook-native/src/components/layout/PageBanner.tsx` | All per-route decorative SVG art components (`ProfilesArt`, `LaunchArt`, etc.); add `LibraryArt` here |
| `src/crosshook-native/src/components/layout/PanelRouteDecor.tsx` | `<PanelRouteDecor illustration={<SomeArt />} />` — standard panel backdrop chrome |
| `src/crosshook-native/src/styles/variables.css` | All CSS custom properties; source of truth for colors, spacing, grid gaps, skeleton timing |
| `src/crosshook-native/src/styles/theme.css` | All component-level CSS; contains `crosshook-skeleton`, `crosshook-skeleton-shimmer`, `crosshook-panel`, `crosshook-error-banner`, `crosshook-community-browser__profile-grid` |
| `src/crosshook-native/src/main.tsx` | CSS import list; new `library.css` must be imported here |
| `src/crosshook-native/src-tauri/src/commands/profile.rs` | `profile_list`, `profile_load`, `profile_set_favorite`, `profile_list_favorites`; add `profile_list_summaries` here |
| `src/crosshook-native/src-tauri/src/lib.rs` | `invoke_handler(tauri::generate_handler![...])` at line 189; register new commands here |

---

## Architectural Patterns

### File Location Convention

- Page components: `src/components/pages/LibraryPage.tsx`
- Feature sub-components: `src/components/library/LibraryCard.tsx`, `LibraryGrid.tsx`, `LibraryToolbar.tsx`  
  (follows existing precedent: `src/components/profile-sections/`, `src/components/ui/`)
- Feature CSS: `src/styles/library.css` (imported in `main.tsx`)
- Types: `src/types/library.ts`
- Hook: `src/hooks/useLibraryProfiles.ts`

### Page Component Pattern

- Pages live in `src/components/pages/` and are pure orchestrators.
- They destructure from `useProfileContext()` (never re-instantiate the hook themselves).
- The `onNavigate` prop type is `(route: AppRoute) => void` (optional `?`); called after `await selectProfile(name)` to avoid a race.
- Root class follows `crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--{routename}`.
- Example: `HealthDashboardPage` at line 826 uses `onNavigate?.('profiles')` after `await selectProfile(profileName)`.

```tsx
// Pattern: page with onNavigate
export function LibraryPage({ onNavigate }: { onNavigate?: (route: AppRoute) => void }) {
  const { profiles, favoriteProfiles, selectProfile, toggleFavorite, refreshProfiles } = useProfileContext();
  // ...
  async function handleLaunch(name: string) {
    await selectProfile(name);
    onNavigate?.('launch');
  }
  return <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--library">...</div>;
}
```

### Route Registration (Three-File Change)

1. **`Sidebar.tsx:13`** — Extend `AppRoute` union: `| 'library'`; add to `SIDEBAR_SECTIONS` with an icon + label.
2. **`App.tsx:19`** — Add `library: true` to `VALID_APP_ROUTES`; change default `useState<AppRoute>` from `'profiles'` to `'library'`.
3. **`ContentArea.tsx:34`** — Add `case 'library': return <LibraryPage onNavigate={onNavigate} />;` in `renderPage()`. The exhaustive `never` check at the default case enforces that all routes are handled.

### Context Consumption Pattern

- `useProfileContext()` throws if called outside `ProfileProvider` (fast-fail pattern).
- `LaunchStateContext` and `PreferencesContext` are nested inside `ProfileProvider` in `App.tsx`; `LibraryPage` only needs `ProfileContext`.
- Destructure only what you need: `const { profiles, favoriteProfiles, selectProfile, toggleFavorite, refreshProfiles } = useProfileContext();`

### Hook Architecture

- Hooks encapsulate all `invoke()` calls and async state.
- Return interfaces are named `Use{Name}Result` and exported.
- Race condition guard: `useGameCoverArt` uses a `requestIdRef` incremented on each new request; stale results are dropped by comparing `requestId !== requestIdRef.current`.
- Error handling: hooks set local `error: string | null` state (from `err instanceof Error ? err.message : String(err)`); they do **not** throw from effects.
- `refreshProfiles` re-fetches `profile_list` via `invoke<string[]>('profile_list')` — same IPC call as on mount.
- `toggleFavorite(name, boolean)` calls `invoke('profile_set_favorite', { name, favorite })` then re-runs `loadFavorites()`.

### CSS Grid Pattern (Community Browser as Precedent)

The closest existing grid pattern is `.crosshook-community-browser__profile-grid`:

```css
/* theme.css:997 */
.crosshook-community-browser__profile-grid {
  grid-template-columns: repeat(auto-fit, minmax(var(--crosshook-community-profile-grid-min), 1fr));
}
/* variables.css: --crosshook-community-profile-grid-min: 280px (default), 340px (controller mode) */
```

For the library grid, use `auto-fill` (not `auto-fit`) to keep card widths fixed at 190px:

```css
.crosshook-library-grid {
  display: grid;
  gap: var(--crosshook-library-grid-gap);
  grid-template-columns: repeat(auto-fill, minmax(var(--crosshook-library-card-width), 1fr));
}
```

New variables to add to `variables.css`:

```css
--crosshook-library-card-width: 190px;
--crosshook-library-card-aspect: 3 / 4;
--crosshook-library-grid-gap: var(--crosshook-grid-gap);
```

### Skeleton Loading Pattern

```css
/* Existing in theme.css — use these classes directly */
.crosshook-skeleton { /* shimmer gradient */ }
@keyframes crosshook-skeleton-shimmer { ... }
/* variables.css controls timing: --crosshook-skeleton-duration: 1.8s */
```

```tsx
// GameCoverArt.tsx:23 — exact pattern to replicate in LibraryCard:
if (loading) return <div className="crosshook-library-card__art crosshook-skeleton" />;
```

### Component Decomposition Rule

`LibraryCard` must be **pure-props-driven** (no direct context access). This enables isolation and future testing. `LibraryGrid` must be **stateless layout-only** (no hooks). `LibraryPage` owns state and passes down through props.

Do **not** create a composite `useCoverArtWithDominantColor` hook — calling `useGameCoverArt` + `useImageDominantColor` in `LibraryCard` is not a DRY violation that warrants a new abstraction.

---

## Code Conventions

### TypeScript Naming

- Interfaces: `PascalCase`, e.g. `LibraryCardData`, `UseLibraryProfilesResult`
- Hooks: `use` prefix, return type named `Use{Name}Result`
- Components: `PascalCase` function, default + named export both
- CSS classes: `crosshook-{component}__element--modifier` (BEM-like)
- IPC command names: `snake_case` string literal in `invoke()`

### IPC Invocation Pattern

```typescript
// Direct invoke — for one-off side effects
await invoke('profile_set_favorite', { name, favorite });

// Typed return — for data fetches
const names = await invoke<string[]>('profile_list');
const path = await invoke<string | null>('fetch_game_cover_art', { appId, imageType: 'cover' });
```

- Always `camelCase` the parameter object keys when calling `invoke()` — Tauri's Serde deserializes them from camelCase on the Rust side.
- Wrap errors: `catch (err) { setError(err instanceof Error ? err.message : String(err)); }`.

### Rust Command Pattern

```rust
// profile.rs pattern — all public commands follow this shape:
#[tauri::command]
pub fn profile_list_summaries(store: State<'_, ProfileStore>) -> Result<Vec<ProfileSummary>, String> {
    // ...
    .map_err(|e| e.to_string())
}
```

- Return type is always `Result<T, String>` for commands (Tauri serializes `Err(String)` to the frontend reject).
- Use `State<'_, T>` for managed dependencies.
- Register in `lib.rs` inside `tauri::generate_handler![...]` at line 189.
- Emit events via `app.emit("profiles-changed", reason)` when mutations should refresh other listeners.
- **`profile_list_summaries` should be synchronous** (`pub fn`, not `pub async fn`) — it reads local TOML files only. `async` is only needed for commands that make network calls.
- **Never write raw SQL in commands** — delegate to `MetadataStore` or `ProfileStore` methods. Store methods use internal `with_conn()` / `with_sqlite_conn()` wrappers.

### CSS File Organization

- Each feature domain gets its own CSS file: `sidebar.css`, `console-drawer.css`, etc.
- Feature CSS files are imported in `main.tsx` alongside the existing list.
- The file `theme.css` is for cross-cutting component classes; new library-specific classes go in `library.css`.
- All CSS custom properties live in `variables.css`; never hardcode values that should vary by breakpoint or controller mode.
- Controller mode overrides use `:root[data-crosshook-controller-mode='true']`.
- Responsive overrides use `@media (max-width: …)` and `@media (max-height: …)` blocks at the bottom of `variables.css`.

---

## Gotchas

- **`useGameCoverArt` hardcodes `imageType: 'cover'`** at line 42. The library-home feature spec calls for a new `GameImageType::Portrait` Rust variant (`library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg`). To use portrait art from `LibraryCard`, either (a) add an optional `imageType?` third parameter to `useGameCoverArt`, or (b) call `invoke('fetch_game_cover_art', { appId, imageType: 'portrait' })` directly in a new hook. Option (a) is simpler; the existing `GameCoverArt` component continues to work unchanged since it passes no third arg.
- **`forceMount` means LibraryPage stays mounted when inactive.** A `refreshProfiles()` `useEffect` will fire once on first mount (correct), but any polling or event-driven re-fetches will continue in the background. Gate effects using `if (route !== 'library') return;` if needed, or rely on the existing `profiles-changed` Tauri event listener in `useProfile` which already handles re-syncing globally.
- **Favorites data flows from `ProfileContext`, not local state.** `favoriteProfiles` is a `string[]` of profile names. `isFavorite` for a card is `favoriteProfiles.includes(card.name)`. Do not persist favorite state inside `LibraryCard`.

---

## Error Handling

### Frontend

- IPC errors surface via `error: string | null` state on hooks; components render `<div className="crosshook-error-banner">{error}</div>`.
- No error boundaries currently — errors stay local to the hook/component that owns the state.
- Optimistic UI pattern (from `PinnedProfilesStrip` / feature-spec R5): update state immediately, revert on IPC error; use a local `optimisticFavorite` state alongside the real `isFavorite` from context.
- `void` keyword is used to explicitly discard async return values for fire-and-forget event handlers: `onClick={() => void someAsyncFn()}`.

### Rust / IPC

- All commands return `Result<T, String>`; error is `.map_err(|e| e.to_string())`.
- Tauri maps `Err(String)` to a rejected Promise on the frontend.
- Non-fatal side effects (metadata sync, event emit) use `tracing::warn!` but do not abort the command.

---

## Testing Approach

- No frontend test framework is configured (`CLAUDE.md`: "There is **no** configured frontend test framework").
- Backend: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` for Rust unit tests.
- UI behavior validation uses dev/build scripts: `./scripts/dev-native.sh`.

---

## Patterns to Follow for Library Home

1. **`LibraryPage` root class**: `crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--library`
2. **Navigation**: always `await selectProfile(name)` before `onNavigate?.('launch'|'profiles')`
3. **Cover art**: use `useGameCoverArt(steamAppId, customCoverArtPath)` exactly as `GameCoverArt.tsx` does; render `crosshook-skeleton` div while `loading`
4. **Dominant color glow (Phase 2)**: `useImageDominantColor(coverArtUrl)` returns `[r,g,b]|null`. The **established pattern** (used in `ProfileSubTabs.tsx:114`, `LaunchSubTabs.tsx:152`, `UpdateGamePanel.tsx:74`, `InstallGamePanel.tsx:141`) is to set CSS custom properties as inline style:
   ```tsx
   const gameColorStyle: CSSProperties | undefined = dominantColor
     ? ({
         '--crosshook-game-color-r': String(dominantColor[0]),
         '--crosshook-game-color-g': String(dominantColor[1]),
         '--crosshook-game-color-b': String(dominantColor[2]),
       } as CSSProperties)
     : undefined;
   ```
   Then CSS reads them via `rgba(var(--crosshook-game-color-r), var(--crosshook-game-color-g), var(--crosshook-game-color-b), 0.45)`. Reuse this pattern in `LibraryCard` — do **not** compute an inline `box-shadow` directly.
5. **Favorites**: call `profileContext.toggleFavorite(name, !isFavorite)` (already wired to `profile_set_favorite` IPC); read `favoriteProfiles` from context for initial state
6. **Grid CSS**: `repeat(auto-fill, minmax(var(--crosshook-library-card-width), 1fr))` — use `auto-fill` not `auto-fit` so empty tracks maintain card width
7. **Route decor**: add `LibraryArt` SVG component to `PageBanner.tsx`; use `<PanelRouteDecor illustration={<LibraryArt />} />` in the page
8. **Empty state**: render inline CTA with `<button className="crosshook-button" onClick={() => onNavigate?.('profiles')}>Create your first profile</button>` — matches `HealthDashboardPage:1163`
9. **New Rust command**: add `profile_list_summaries` to `profile.rs`, register in `lib.rs`; return `Vec<ProfileSummary>` with `name`, `game_name`, `steam_app_id`, `custom_cover_art_path`
10. **CSS import**: add `import './styles/library.css';` to `main.tsx` after the existing imports
