# Research: External APIs, Libraries, and Integration Patterns for library-home

**Date**: 2026-04-01
**Researcher**: External APIs agent

---

## Executive Summary

CrossHook already has a mature, production-ready image pipeline in `crosshook-core` that:

- Fetches from **Steam CDN** (`cdn.cloudflare.steamstatic.com`) as the primary source
- Falls back to **SteamGridDB API** when an API key is configured
- Validates all bytes by magic-number (JPEG/PNG/WebP allow-list, 5 MB cap)
- Caches files on disk with a 24-hour TTL and a SQLite metadata row per image
- Returns the cached file path as a `file://` URL via Tauri's `convertFileSrc`

The hook for the library-home grid (`useGameCoverArt`) and the Tauri command (`fetch_game_cover_art`) already exist and work. The **primary gap** is not in the image pipeline — it is in the **grid rendering layer**: no virtualised grid component exists for displaying many game cards simultaneously.

**Recommendation**: Reuse the existing hook for card-level cover art; add `@tanstack/react-virtual` (v3) for grid virtualisation.

**Confidence**: High — conclusions are based on direct codebase inspection and cross-referenced external documentation.

---

## Primary APIs

### 1.1 Steam CDN (Cloudflare edge)

| Property                         | Detail                                                                                                                                                                 |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Base host                        | `https://cdn.cloudflare.steamstatic.com/steam/apps/{APP_ID}/`                                                                                                          |
| Legacy Akamai host (still works) | `https://steamcdn-a.akamaihd.net/steam/apps/{APP_ID}/`                                                                                                                 |
| Auth                             | None — unauthenticated CDN                                                                                                                                             |
| Rate limits                      | Undocumented (CDN caching means repeated fetches are cheap); the JSON API at `store.steampowered.com/api/appdetails` is rate-limited at **200 requests per 5 minutes** |
| Pricing                          | Free (no account required)                                                                                                                                             |

**Confirmed URL formats used by CrossHook today** (see `client.rs:334–352`):

```
Cover   →  https://cdn.cloudflare.steamstatic.com/steam/apps/{APP_ID}/header.jpg
           (460×215 landscape; adequate as fallback but not the 600×900 portrait format)

Hero    →  https://cdn.cloudflare.steamstatic.com/steam/apps/{APP_ID}/library_hero.jpg

Capsule →  https://cdn.cloudflare.steamstatic.com/steam/apps/{APP_ID}/capsule_616x353.jpg
```

**Portrait 600×900 format** (desired by the Figma design — not yet in use):

```
https://cdn.cloudflare.steamstatic.com/steam/apps/{APP_ID}/library_600x900_2x.jpg
    ↑ 2x = true 600×900 px
https://cdn.cloudflare.steamstatic.com/steam/apps/{APP_ID}/library_600x900.jpg
    ↑ 1x = 300×450 px (half resolution, stored locally in Steam cache)
```

Not all games have the 600×900 format. Returning HTTP 404 is the expected failure mode; the existing fallback chain in `download_and_cache_image` handles this gracefully.

**Action item for implementation**: Add `GameImageType::Portrait600x900` (or repurpose `Cover`) to use the `library_600x900_2x.jpg` URL as the primary attempt, with `header.jpg` as the fallback.

**Confidence**: High (confirmed via community forum + direct URL structure examination)

Sources:

- [Steam library 600×900 assets community thread](https://steamcommunity.com/discussions/forum/1/4202490864582293420/)
- [Steamworks standard assets documentation](https://partner.steamgames.com/doc/store/assets/standard)

### 1.2 Steam Store `appdetails` API (unofficial, rate-limited)

| Property              | Detail                                                                                              |
| --------------------- | --------------------------------------------------------------------------------------------------- |
| Endpoint              | `https://store.steampowered.com/api/appdetails?appids={APP_ID}`                                     |
| Auth                  | None                                                                                                |
| Rate limit            | **200 requests / 5 minutes**                                                                        |
| Image fields returned | `header_image`, `capsule_image`, `capsule_imagev5`, `background`, `background_raw`, `screenshots[]` |
| Note                  | `header_image` resolves to the `header.jpg` CDN URL — same file already used by CrossHook           |

This API is **not needed** for the library-home feature since URLs can be constructed directly. It would only add value if game metadata (name, description) must be fetched alongside the image.

**Confidence**: High (rate limit confirmed by community; endpoint confirmed by multiple third-party documentation sources)

Sources:

- [Revadike InternalSteamWebAPI wiki](https://github.com/Revadike/InternalSteamWebAPI/wiki/Get-App-Details)
- [appdetails R documentation with fields](https://jslth.github.io/steamr/reference/appdetails.html)

### 1.3 SteamGridDB API (already integrated)

| Property                   | Detail                                                                                            |
| -------------------------- | ------------------------------------------------------------------------------------------------- |
| API reference              | <https://www.steamgriddb.com/api/v1> (v1 deprecated) / v2 in use                                  |
| npm package                | `steamgriddb` v2.2.0 (May 2024, 1 active maintainer)                                              |
| Tauri-specific wrapper     | `@tormak/tauri-steamgriddb` (exists on npm; maintenance status unclear)                           |
| Auth                       | Free API key; 90-day rotation required; 2FA enforced                                              |
| Rate limits                | Unspecified in public docs; practical reports indicate throttling under heavy batch use           |
| Grid image style           | Supports portrait `600×900` "p" style grids explicitly                                            |
| Fallback role in CrossHook | Primary source when `steamgriddb_api_key` is set in settings; Steam CDN is the secondary fallback |

The existing Rust implementation in `steamgriddb.rs` already handles SteamGridDB queries. No new library integration is needed.

**Confidence**: Medium (official docs are sparse; behavioural details from community sources)

Sources:

- [SteamGridDB npm package](https://www.npmjs.com/package/steamgriddb)
- [SteamGridDB node wrapper GitHub](https://github.com/SteamGridDB/node-steamgriddb)
- [Libraries.io maintenance status](https://libraries.io/npm/steamgriddb)

---

## 2. Libraries and SDKs

### 2.1 Virtual/Windowed Grid Rendering

Three established options exist:

| Library                   | Version     | Bundle   | Grid support                         | Maintenance                | Verdict                          |
| ------------------------- | ----------- | -------- | ------------------------------------ | -------------------------- | -------------------------------- |
| `@tanstack/react-virtual` | v3 (latest) | 10–15 KB | Yes (row + column virtualizers)      | Active (TanStack team)     | **Recommended**                  |
| `react-window`            | 1.8.x       | < 2 KB   | `FixedSizeGrid` / `VariableSizeGrid` | Maintenance mode (bvaughn) | Acceptable for fixed-width cards |
| `react-virtualized`       | 9.x         | ~33.5 KB | Yes                                  | Slow maintenance           | Not recommended                  |

**TanStack Virtual v3** is the clear choice for a new feature:

- Single hook (`useVirtualizer`) for rows and columns
- Variable-size support out of the box — needed if the grid changes column count on resize
- Composable (bring your own scroll container) — fits the existing `crosshook-*` CSS structure
- Tree-shakable, 10–15 KB overhead
- No required fixed `height`/`width` props on the scroll element

**react-window `FixedSizeGrid`** is a simpler alternative if column count and card width are fixed (e.g., always 190 px cards with a fixed container width). It is 2 KB but does not handle dynamic column count changes or variable card sizes well.

**Installation**:

```bash
pnpm add @tanstack/react-virtual
```

```typescript
import { useVirtualizer } from '@tanstack/react-virtual';
```

**Grid virtualizer pattern for a responsive image grid**:

```typescript
// Two virtualizers: one for rows, one for columns
const rowVirtualizer = useVirtualizer({
  count: Math.ceil(gameProfiles.length / columnCount),
  getScrollElement: () => scrollRef.current,
  estimateSize: () => CARD_HEIGHT, // e.g. 253 px for 190 px width at 3:4
  overscan: 3,
});

const columnVirtualizer = useVirtualizer({
  count: columnCount,
  getScrollElement: () => scrollRef.current,
  estimateSize: () => CARD_WIDTH, // 190 px
  horizontal: true,
  overscan: 2,
});
```

**Confidence**: High (npm trends, official docs, direct benchmark comparison)

Sources:

- [TanStack Virtual official docs](https://tanstack.com/virtual/latest)
- [TanStack v3 React fixed example](https://tanstack.com/virtual/v3/docs/framework/react/examples/fixed)
- [npm trends comparison](https://npmtrends.com/@tanstack/react-virtual-vs-react-virtualized-vs-react-window)
- [Borstch TanStack vs react-window comparison](https://borstch.com/blog/development/comparing-tanstack-virtual-with-react-window-which-one-should-you-choose)

### 2.2 Image Lazy Loading

**Recommendation**: Use the browser-native `loading="lazy"` attribute on `<img>` tags.

- No additional dependency required
- Supported in WebKitGTK 2.36+ (which maps to WebKit 614.x / Safari TP 140 equivalent)
- Tauri on Linux requires webkit2gtk 4.1; `loading="lazy"` is fully supported at that version

For cards rendered inside a TanStack Virtual scroll container, images outside the visible viewport are not rendered at all (the entire cell is virtualised), making an additional IntersectionObserver unnecessary.

The `react-intersection-observer` package is useful only if non-virtualised scrolling is chosen, or for analytics/animation triggers.

**Confidence**: High

Sources:

- [Tauri webview versions reference](https://v2.tauri.app/reference/webview-versions/)
- [LogRocket: lazy loading with Intersection Observer](https://blog.logrocket.com/lazy-loading-using-the-intersection-observer-api/)

### 2.3 Game Metadata (already handled internally)

**`useGameMetadata(steamAppId)`** — `src/hooks/useGameMetadata.ts`

- Returns: game name, genres, description from the SQLite metadata DB
- States: `idle | loading | ready | stale | unavailable`
- **400ms debounce built in** — prevents IPC storms during rapid virtualised-grid scrolling where cards mount/unmount quickly
- IPC command: `fetch_game_metadata(appId, forceRefresh)` → `SteamMetadataLookupResult`
- Component wrapper: `GameMetadataBar` at `src/components/profile-sections/GameMetadataBar.tsx`

No new metadata API integration is needed. All Steam data flows Rust backend → SQLite cache → IPC → frontend hook; zero direct external calls happen in the frontend.

**Confidence**: High (confirmed by direct codebase inspection)

### 2.4 Image Caching (already handled at Rust layer)

The Tauri/Rust pipeline (`download_and_cache_image`) writes images to `$XDG_DATA_HOME/crosshook/cache/images/{APP_ID}/` and returns the absolute path. The front end receives a `convertFileSrc`-converted `asset://` URL. **No additional JS-layer caching library is needed.**

The Medium article on Tauri image caching (base64 Data URI approach) is an alternative but is less efficient than the existing file-based approach since base64 encoding increases payload size by ~33%.

---

## Integration Patterns

### 3.1 Heroic Games Launcher

- Built with Electron + React + TypeScript
- Uses portrait `art_square` images for library grid cards (same concept as Steam's `library_600x900`)
- Cover art grid is a CSS grid (not virtualised), implying the total game count in most users' libraries is manageable without virtualisation — however CrossHook's architecture may accumulate more profiles than typical Heroic use cases
- Fetches from Epic CDN or GOG CDN directly; no Steam CDN usage by default

Source: [Heroic GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher)

### 3.2 Bazzite Sunshine Manager (reference implementation)

- Fetches cover art from Steam CDN or SteamGridDB in priority order — exactly the same pattern CrossHook uses
- Caches to local filesystem, stores path in metadata

Source: [bazzite-sunshine-manager GitHub](https://github.com/wadiebs/bazzite-sunshine-manager)

### 3.3 General pattern

All major Linux game launchers that display game grids use:

1. **Portrait art** from CDN when available
2. **SteamGridDB** as community fallback
3. **Local filesystem cache** to avoid repeated network round-trips
4. **CSS grid** or a flex wrapping layout (not virtualised) for small libraries

CrossHook's approach is already ahead of this baseline by having a Rust-backed, TTL-aware, magic-byte-validated cache with a SQLite metadata layer.

---

## 4. Constraints and Gotchas

### 4.1 Steam CDN: no `library_600x900_2x.jpg` for all games

- Games without the portrait format return HTTP 404
- CrossHook must implement a fallback: `library_600x900_2x.jpg` → `library_600x900.jpg` → `header.jpg`
- The existing `download_and_cache_image` fallback chain only goes CDN → stale cache; a within-CDN URL fallback sequence is not yet implemented

**Confidence**: High

### 4.2 Steam CDN rate limits on image requests

- CDN image requests (not the JSON API) are served by Akamai/Cloudflare edge caches and are practically unlimited for read-through access
- The `appdetails` JSON API (separate from CDN images) is limited to 200 requests / 5 minutes — not relevant if only constructing image URLs directly

**Confidence**: Medium (no official documentation; derived from community and research paper)

Source: [IIJ Lab Steam CDN research paper (2024)](https://www.iijlab.net/en/members/romain/pdf/chris_pam2024.pdf)

### 4.3 WebP and AVIF support in WebKitGTK on Linux

- CrossHook targets Linux; Tauri v2 requires webkit2gtk 4.1
- **WebP**: Fully supported in WebKit (support added in WebKit bug 47512; enabled in WebKitGTK 2.x for years)
- **AVIF**: Supported in WebKitGTK but requires libavif at compile time; not guaranteed on all distributions
- Steam CDN serves JPEG images; SteamGridDB can serve WebP or PNG
- The Rust validation layer (`ALLOWED_IMAGE_MIMES`) already accepts `image/webp` alongside JPEG/PNG
- **Safe choice**: Prefer JPEG/PNG; accept WebP; do not rely on AVIF

**Confidence**: Medium (compile-time flag dependency for AVIF is platform-specific)

Sources:

- [webkit2gtk AVIF support note (LFS)](https://www.linuxfromscratch.org/blfs/view/svn/x/webkitgtk.html)
- [WebKit WebP bug history](https://bugs.webkit.org/show_bug.cgi?id=47512)

### 4.4 SteamGridDB API key requirement

- SteamGridDB requires an account + API key; tokens now expire every 90 days and require 2FA
- This is already modelled in `SettingsStore` (`steamgriddb_api_key` field); the feature works without a key (Steam CDN only)
- Library-home should degrade gracefully when no key is configured: show `header.jpg` landscape art instead of portrait art, or show a styled placeholder

**Confidence**: High

### 4.5 Concurrent image fetches at grid load time

- A library-home page with 50–200 game cards will issue 50–200 concurrent `fetch_game_cover_art` invocations on mount if each card independently calls `useGameCoverArt`
- The Tauri async runtime and reqwest HTTP client handle concurrency, but this could spike network usage on cold start
- **Mitigation (cover art)**: use the virtualised grid — only visible cards invoke the hook at any given time. The `overscan` setting (1–2 extra rows) prevents constant mount/unmount at the scroll boundary
- **Mitigation (metadata)**: `useGameMetadata` has a built-in 400ms debounce, so rapid scroll-through does not hammer the `fetch_game_metadata` IPC command
- **Note**: `useGameCoverArt` has no debounce — only the request-id race guard. Fast scrolling can still fire many cover art IPC calls; the 24-hour disk cache means all repeat visits are zero-network, and on-cache-hit the Rust command returns immediately from SQLite without network I/O

**Confidence**: High

---

## 5. Code Examples

### 5.1 Portrait 600×900 URL construction (Rust change needed)

Current `build_download_url` for `GameImageType::Cover` returns `header.jpg` (landscape). A new type or URL sequence is needed:

```rust
// Proposed: try portrait first, fall back to header
fn portrait_url_candidates(app_id: &str) -> Vec<String> {
    vec![
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900.jpg"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/header.jpg"),
    ]
}
```

### 5.2 TanStack Virtual responsive grid (React/TS)

```tsx
import { useVirtualizer } from '@tanstack/react-virtual';
import { useRef, useMemo } from 'react';

const CARD_WIDTH = 190;
const CARD_HEIGHT = 253; // 190 * (4/3) ≈ 253 for 3:4 aspect
const GAP = 12;

function LibraryGrid({ profiles }: { profiles: GameProfile[] }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const containerWidth = useContainerWidth(containerRef); // custom hook / ResizeObserver

  const columnCount = Math.max(1, Math.floor((containerWidth + GAP) / (CARD_WIDTH + GAP)));
  const rowCount = Math.ceil(profiles.length / columnCount);

  const rowVirtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => containerRef.current,
    estimateSize: () => CARD_HEIGHT + GAP,
    overscan: 2,
  });

  const colVirtualizer = useVirtualizer({
    count: columnCount,
    getScrollElement: () => containerRef.current,
    estimateSize: () => CARD_WIDTH + GAP,
    horizontal: true,
    overscan: 1,
  });

  return (
    <div ref={containerRef} style={{ overflow: 'auto', height: '100%' }}>
      <div
        style={{
          height: rowVirtualizer.getTotalSize(),
          width: colVirtualizer.getTotalSize(),
          position: 'relative',
        }}
      >
        {rowVirtualizer.getVirtualItems().map((row) =>
          colVirtualizer.getVirtualItems().map((col) => {
            const index = row.index * columnCount + col.index;
            if (index >= profiles.length) return null;
            return (
              <LibraryCard
                key={profiles[index].id}
                profile={profiles[index]}
                style={{
                  position: 'absolute',
                  top: row.start,
                  left: col.start,
                  width: CARD_WIDTH,
                  height: CARD_HEIGHT,
                }}
              />
            );
          })
        )}
      </div>
    </div>
  );
}
```

### 5.3 Card cover art reuse pattern

```tsx
// GameCoverArt is already available at:
// src/crosshook-native/src/components/profile-sections/GameCoverArt.tsx
// useGameCoverArt is at:
// src/crosshook-native/src/hooks/useGameCoverArt.ts

import { GameCoverArt } from '../profile-sections/GameCoverArt';

function LibraryCard({ profile, style }: { profile: GameProfile; style: CSSProperties }) {
  return (
    <div className="crosshook-library-card" style={style}>
      <GameCoverArt steamAppId={profile.steam_app_id} customCoverArtPath={profile.custom_cover_art_path} />
      {/* gradient overlay + action buttons */}
    </div>
  );
}
```

---

## 6. Open Questions

1. **Portrait image type in `GameImageType`**: Should `library_600x900_2x.jpg` become the default `Cover` URL, or should a new `Portrait` variant be added? The existing code returns `header.jpg` for `Cover`. A breaking change to the CDN URL for `Cover` could affect the existing profile editor which uses `GameCoverArt`.

2. **Column count and responsiveness**: Should the grid adapt column count via a `ResizeObserver` hook, or is a fixed-column responsive CSS grid (non-virtualised) sufficient for typical library sizes (< 200 profiles)?

3. **Placeholder / skeleton state**: The existing `GameCoverArt` renders `crosshook-skeleton` during loading. Is a single skeleton per card sufficient for the library grid, or should a shimmer animation be designed?

4. **List view toggle**: The feature description mentions a grid/list toggle. The list view layout (horizontal card? text + thumbnail row?) is not specified; different image dimensions may be needed.

5. **Search**: Is full-text search over profile names implemented at the Rust/SQLite layer already, or does the library-home need to filter a client-side array?

---

## Sources

- [TanStack Virtual latest docs](https://tanstack.com/virtual/latest)
- [TanStack Virtual v3 React fixed example](https://tanstack.com/virtual/v3/docs/framework/react/examples/fixed)
- [react-window official docs](https://react-window.vercel.app/)
- [npm trends: TanStack vs react-window vs react-virtualized](https://npmtrends.com/@tanstack/react-virtual-vs-react-virtualized-vs-react-window)
- [Borstch: TanStack Virtual vs react-window](https://borstch.com/blog/development/comparing-tanstack-virtual-with-react-window-which-one-should-you-choose)
- [TanStack Virtual vs react-virtualized analysis](https://borstch.com/blog/development/tanstack-virtual-vs-react-virtualized-differences-similarities-and-performance-analysis)
- [Steam library 600×900 format community thread](https://steamcommunity.com/discussions/forum/1/4202490864582293420/)
- [Steamworks standard store assets](https://partner.steamgames.com/doc/store/assets/standard)
- [IIJ Lab: Steam CDN research (2024)](https://www.iijlab.net/en/members/romain/pdf/chris_pam2024.pdf)
- [Tauri v2 webview versions](https://v2.tauri.app/reference/webview-versions/)
- [Tauri image caching implementation guide](https://losefor.medium.com/implementing-image-caching-with-tauri-enhancing-performance-and-offline-access-6a55c2dbc802)
- [SteamGridDB node wrapper](https://github.com/SteamGridDB/node-steamgriddb)
- [SteamGridDB npm package](https://www.npmjs.com/package/steamgriddb)
- [Heroic Games Launcher GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher)
- [bazzite-sunshine-manager (CDN + SGDB cover art)](https://github.com/wadiebs/bazzite-sunshine-manager)
- [Steam appdetails R documentation](https://jslth.github.io/steamr/reference/appdetails.html)
- [Tauri calling Rust from frontend](https://v2.tauri.app/develop/calling-rust/)
- [webkitgtk AVIF support (LFS)](https://www.linuxfromscratch.org/blfs/view/svn/x/webkitgtk.html)
- [Intersection Observer lazy loading (LogRocket)](https://blog.logrocket.com/lazy-loading-using-the-intersection-observer-api/)
