# External Research: UI Libraries, APIs, and Integration Patterns for CrossHook UI Enhancements

**Date**: 2026-04-01 (updated)
**Task**: Research external APIs, libraries, and integration patterns for the Profiles page Advanced section declutter PLUS game metadata and cover art integration (GitHub issue #52).

---

## Executive Summary

This document covers two scopes:

**Pass 1 — Advanced section restructuring**: CrossHook already has `@radix-ui/react-tabs` v1.1.13. The sub-tab CSS classes (`.crosshook-subtab-row`, `.crosshook-subtab`, `.crosshook-subtab--active`) are already defined in `theme.css` and unused. **Zero new frontend dependencies required** for the tab restructuring.

**Pass 2 — Game metadata and cover art (issue #52)**: The Figma concept is a **cover art card grid** — game cover art is the primary visual, with launch/favorite/edit actions overlaid on or attached to each card. This is a new browsing mode for the Profiles page, not a theme overhaul. The existing CrossHook dark glassmorphism theme, BEM `crosshook-*` classes, CSS variables, and controller mode are preserved as-is. Steam Store API is no-auth and free. SteamGridDB requires a user-provided API key. The `external_cache_entries` SQLite table already provides the exact upsert/TTL pattern used by ProtonDB — Steam metadata JSON should reuse it with namespace `steam:appdetails:v1:{app_id}`. Binary image files need separate filesystem caching (`~/.local/share/crosshook/cache/images/{steam_app_id}/`) plus a new `game_image_cache` SQLite table for metadata-only tracking. Tauri's `convertFileSrc` / asset protocol is the correct mechanism for serving cached images to the frontend.

**Confidence**: High for Pass 1 (verified against codebase). High for Steam CDN URL patterns (widely documented). Medium for SteamGridDB rate limits (not officially published).

---

## Current Technology Stack (Verified)

From `src/crosshook-native/package.json`:

| Dependency               | Version | Role                               |
| ------------------------ | ------- | ---------------------------------- |
| `@radix-ui/react-tabs`   | ^1.1.13 | Tab primitives (already installed) |
| `@radix-ui/react-select` | ^2.2.6  | Select primitives                  |
| `react-resizable-panels` | ^4.7.6  | Resizable panel splits             |
| `@tauri-apps/api`        | ^2.0.0  | Tauri IPC                          |
| React                    | ^18.3.1 | Frontend framework                 |
| Vite                     | ^8.0.2  | Bundler                            |
| TypeScript               | ^5.6.3  | Type system                        |

**No Tailwind CSS, no MUI, no design system framework** — the project uses a handcrafted CSS variable system with `crosshook-*` CSS custom properties.

From `src/crosshook-native/crates/crosshook-core/Cargo.toml` (already in use):

| Crate      | Role                                     |
| ---------- | ---------------------------------------- |
| `reqwest`  | HTTP client (used in protondb/client.rs) |
| `rusqlite` | SQLite driver for MetadataStore          |
| `serde`    | Serialization across IPC boundary        |
| `tokio`    | Async runtime                            |
| `chrono`   | Timestamp handling in cache entries      |

---

## PART 1: Advanced Section Restructuring (Pass 1 — Preserved)

### Primary UI Libraries

### 1. Radix UI Primitives (ALREADY INSTALLED)

- **Docs**: <https://www.radix-ui.com/primitives>
- **Tabs docs**: <https://www.radix-ui.com/primitives/docs/components/tabs>
- **Accordion docs**: <https://www.radix-ui.com/primitives/docs/components/accordion>
- **Version already in use**: `@radix-ui/react-tabs` v1.1.13
- **Bundle size**: `@radix-ui/react-tabs` ~90kB minified (individual packages, tree-shaken per component)
- **Tauri compatibility**: Full — headless, no browser-specific APIs, pure DOM manipulation

**Tabs API**:

```typescript
import * as Tabs from '@radix-ui/react-tabs';

<Tabs.Root defaultValue="general" orientation="vertical">
  <Tabs.List aria-label="Profile sections">
    <Tabs.Trigger value="general">General</Tabs.Trigger>
    <Tabs.Trigger value="runtime">Runtime</Tabs.Trigger>
    <Tabs.Trigger value="advanced">Advanced</Tabs.Trigger>
  </Tabs.List>
  <Tabs.Content value="general">...</Tabs.Content>
  <Tabs.Content value="runtime">...</Tabs.Content>
  <Tabs.Content value="advanced">...</Tabs.Content>
</Tabs.Root>
```

**Keyboard navigation** (built-in):

- `Tab` / `Shift+Tab` — move between trigger list and panel
- `ArrowLeft` / `ArrowRight` (horizontal) or `ArrowUp` / `ArrowDown` (vertical) — navigate triggers
- `Home` / `End` — jump to first/last trigger

**Key props**:

- `orientation`: `"horizontal"` | `"vertical"` — vertical is ideal for a sidebar-style sub-nav
- `activationMode`: `"automatic"` (default, activates on arrow key) | `"manual"` (requires Enter/Space)
- `defaultValue` / `value` — uncontrolled or controlled mode

**Confidence**: High — official docs verified, already in `package.json`

---

### 2. Radix UI Accordion (NOT YET INSTALLED — natural add)

- **Install**: `npm install @radix-ui/react-accordion`
- **Docs**: <https://www.radix-ui.com/primitives/docs/components/accordion>
- **Bundle size**: ~90.2kB minified
- **Tauri compatibility**: Full

**Accordion API**:

```typescript
import * as Accordion from '@radix-ui/react-accordion';

<Accordion.Root type="single" defaultValue="general" collapsible>
  <Accordion.Item value="general">
    <Accordion.Header>
      <Accordion.Trigger>General</Accordion.Trigger>
    </Accordion.Header>
    <Accordion.Content>...</Accordion.Content>
  </Accordion.Item>
</Accordion.Root>
```

**Confidence**: High — official docs verified, same vendor pattern as existing deps

---

### 3. shadcn/ui (Copy-paste approach — incompatible)

**Compatibility with CrossHook**: **Problematic**. shadcn/ui components assume Tailwind CSS. CrossHook does not use Tailwind. **Recommendation**: Skip.

**Confidence**: High — Tailwind requirement confirmed as incompatible

---

### Tab/Navigation Libraries Evaluated

| Library                     | Install                           | Size                   | Tailwind Required | Already Installed |
| --------------------------- | --------------------------------- | ---------------------- | ----------------- | ----------------- |
| `@radix-ui/react-tabs`      | Yes                               | ~90kB                  | No                | **YES**           |
| `@radix-ui/react-accordion` | `npm i @radix-ui/react-accordion` | ~90kB                  | No                | No                |
| `@headlessui/react`         | `npm i @headlessui/react`         | ~22kB                  | No                | No                |
| `ark-ui`                    | `npm i @ark-ui/react`             | Large (+ zag)          | No                | No                |
| `shadcn/ui tabs`            | Copy-paste                        | 0 (but needs Tailwind) | **YES**           | No                |

**Winner**: `@radix-ui/react-tabs` — already installed, zero new dependency cost, consistent API with existing code.

---

### Integration Patterns (UI Restructuring)

#### Pattern 1: Vertical Sub-Tabs (Recommended for "sub-tabs within pages")

Use `@radix-ui/react-tabs` with `orientation="vertical"` to replace the single collapsed Advanced `<details>` block with a persistent sidebar + panel layout.

**Key constraints**:

- The app's outer `Tabs.Root` uses `orientation="vertical"` for page-level routing. Sub-tabs must be a **nested `Tabs.Root`** with `orientation="horizontal"`.
- Sub-tabs must be composed at the **`ProfilesPage` level**, not inside `ProfileFormSections` (the form sections component is shared with `InstallPage` modal).

**CSS approach** (uses existing design tokens, `.crosshook-subtab*` classes already in `theme.css`):

```tsx
// Composed at ProfilesPage level — NOT inside ProfileFormSections
import * as Tabs from '@radix-ui/react-tabs';

export function ProfilesPage() {
  return (
    <Tabs.Root defaultValue="general" orientation="horizontal">
      <Tabs.List className="crosshook-subtab-row" aria-label="Profile sections">
        <Tabs.Trigger className="crosshook-subtab" value="general">
          General
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-subtab" value="runtime">
          Runtime
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-subtab" value="advanced">
          Advanced
        </Tabs.Trigger>
      </Tabs.List>
      <Tabs.Content value="general">...</Tabs.Content>
      <Tabs.Content value="runtime">...</Tabs.Content>
      <Tabs.Content value="advanced">...</Tabs.Content>
    </Tabs.Root>
  );
}
```

**CSS bridge** (`[data-state='active']` selector for existing `.crosshook-subtab--active` styles):

```css
/* In theme.css or a new profile-subtabs.css */
.crosshook-subtab[data-state='active'] {
  border-color: rgba(0, 120, 212, 0.45);
  background: linear-gradient(135deg, var(--crosshook-color-accent) 0%, var(--crosshook-color-accent-strong) 100%);
  color: #fff;
}
```

**Confidence**: High

---

## PART 2: Game Metadata and Cover Art (Pass 2 — NEW)

---

## Primary APIs

### Steam Store API

### Endpoint

```
GET https://store.steampowered.com/api/appdetails?appids={app_id}
```

- **Authentication**: None required — publicly accessible
- **Pricing**: Free, no API key needed
- **Format**: JSON

### Response Structure

```json
{
  "730": {
    "success": true,
    "data": {
      "type": "game",
      "name": "Counter-Strike 2",
      "steam_appid": 730,
      "required_age": 0,
      "is_free": true,
      "short_description": "Counter-Strike 2 expands...",
      "detailed_description": "...",
      "header_image": "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/730/header.jpg",
      "capsule_image": "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/730/capsule_231x87.jpg",
      "capsule_imagev5": "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/730/capsule_184x69.jpg",
      "website": "https://www.counter-strike.net/",
      "developers": ["Valve"],
      "publishers": ["Valve"],
      "platforms": { "windows": true, "mac": false, "linux": false },
      "genres": [{ "id": "1", "description": "Action" }],
      "categories": [{ "id": 2, "description": "Single-player" }],
      "release_date": { "coming_soon": false, "date": "Aug 21, 2012" },
      "background": "https://store.steampowered.com/...",
      "screenshots": [
        {
          "id": 0,
          "path_thumbnail": "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/730/..._480x270.jpg",
          "path_full": "https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/730/..._1920x1080.jpg"
        }
      ]
    }
  }
}
```

**Key fields for CrossHook**:

| Field               | Use                                           |
| ------------------- | --------------------------------------------- |
| `name`              | Display name on profile card                  |
| `short_description` | Subtitle or tooltip (HTML — strip tags)       |
| `header_image`      | Horizontal banner (~460×215 equivalent)       |
| `capsule_image`     | Small capsule (231×87)                        |
| `genres`            | Genre tags                                    |
| `categories`        | Feature flags (multiplayer, controller, etc.) |

**Note**: `short_description` is HTML — the frontend or Rust side must strip HTML tags before display.

### Rate Limits

- **Limit**: ~200 successful requests per 5-minute window per IP
- **Error codes**: HTTP 429 (Too Many Requests) or HTTP 403 on throttle
- **Behavior**: No documented retry-after header; exponential backoff recommended
- **Implication for CrossHook**: Profile cards are loaded individually on demand (not bulk-fetched). A single user with hundreds of profiles would not hit this limit in normal use. Cache TTL of 24–48 hours eliminates re-fetches.

**Confidence**: High — rate limit documented in multiple community sources

### Undocumented Status

The appdetails endpoint is **not officially documented** by Valve. The Storefront API page explicitly states: "This API is not actively documented, under development, and not meant for public consumption. It may change at any time without notice." However, it has been stable for years and is widely used in community tooling.

**Implication**: Treat HTTP 404 or unexpected response shapes as graceful-degradation triggers, not errors.

**Confidence**: High (stability record) / Low (official support guarantees)

---

## Steam CDN Image URL Patterns

Steam images can be fetched directly from CDN without API calls or authentication. All URLs are publicly accessible.

### Available Image Formats and Dimensions

| Asset Type             | Dimensions     | URL Pattern                                                                     |
| ---------------------- | -------------- | ------------------------------------------------------------------------------- |
| Header image           | ~460×215       | `https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/header.jpg`          |
| Capsule (horizontal)   | 231×87         | Returned in `appdetails` response as `capsule_image`                            |
| Capsule 616×353        | 616×353        | `https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/capsule_616x353.jpg` |
| **Library portrait**   | **600×900**    | `https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/library_600x900.jpg` |
| Library portrait 2x    | 1200×1800 (2x) | `https://steamcdn-a.akamaihd.net/steam/apps/{appid}/library_600x900_2x.jpg`     |
| Library hero           | 3840×1240      | `https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/library_hero.jpg`    |
| Library header capsule | 920×430        | Steam Deck / client header                                                      |
| Vertical capsule       | 748×896        | Portrait promotional (store)                                                    |

**Recommended for CrossHook profile cards**: `library_600x900.jpg` (portrait 3:4 ratio, matches Figma concept)

**CDN domains** (both serve the same content without auth):

- `https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/`
- `https://steamcdn-a.akamaihd.net/steam/apps/{appid}/`
- `https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{appid}/` (for some hash-based assets returned by appdetails)

**Important**: Not all games have `library_600x900.jpg` available on CDN. When it returns 404, fall back to `header.jpg` (nearly universal), then to placeholder.

**Confidence**: High — URL patterns verified across multiple community tools and discussions

---

### SteamGridDB API

### Overview

SteamGridDB is a **community-contributed** artwork database. It provides alternative/custom art for games, including portrait grids, hero banners, logos, and icons. API access requires a **user-provided API key**.

- **Base URL**: `https://www.steamgriddb.com/api/v2`
- **Auth**: `Authorization: Bearer {api_key}` header on all requests
- **API Key**: Free, obtained from <https://www.steamgriddb.com/profile/preferences/api>
- **Pricing**: Free — no paid tiers documented as of 2025

### Endpoints Relevant to CrossHook

All endpoints below accept a Steam App ID directly via the `/steam/{app_id}` path variant:

```
GET /api/v2/grids/steam/{app_id}   — Portrait grid art (cover images)
GET /api/v2/heroes/steam/{app_id}  — Hero/banner art (wide format)
GET /api/v2/logos/steam/{app_id}   — Game logos (transparent PNG)
GET /api/v2/icons/steam/{app_id}   — Icons (small square)
```

### Grid (Portrait) Response Format

```json
{
  "success": true,
  "data": [
    {
      "id": 123456,
      "score": 42,
      "style": "material",
      "url": "https://www.steamgriddb.com/grid/123456",
      "thumb": "https://www.steamgriddb.com/thumb/123456",
      "tags": [],
      "author": {
        "name": "username",
        "steam64": "76561198...",
        "avatar": "https://..."
      }
    }
  ]
}
```

### Image Types and Typical Dimensions

| Type | Orientation   | Common Dimensions      | Format      | Use in CrossHook        |
| ---- | ------------- | ---------------------- | ----------- | ----------------------- |
| Grid | Portrait (P)  | 600×900, 920×430       | PNG/JPEG    | Profile card cover art  |
| Grid | Landscape (L) | 460×215                | PNG/JPEG    | Card horizontal variant |
| Hero | Wide          | 1920×620, 3840×1240    | PNG/JPEG    | Card hero background    |
| Logo | Transparent   | Variable, up to 1280px | PNG (alpha) | Overlay on cover        |
| Icon | Square        | 256×256, 512×512       | PNG/ICO     | Small indicators        |

**Recommended**: Grid (Portrait, 600×900) as primary art source when Steam CDN `library_600x900.jpg` is not available.

### Query Parameters

```
?dimensions=600x900,920x430  — filter by exact dimensions
?mimes=image/png              — filter by MIME type
?styles=alternate             — filter by art style
?limit=1                      — return only the top result
```

### Rate Limits

SteamGridDB does not publish explicit rate limits in official documentation. Based on community usage and wrapper libraries, the API is described as "generous for personal use." No documented requests-per-minute ceiling was found.

**Recommendation**: Apply the same conservative pattern used for ProtonDB: one request per user action, cache results with a 48-hour TTL.

**Confidence**: Low — no official rate limit documentation found

### Client Libraries (Reference Only — not recommended as dependencies)

| Language | Library                                      | Notes                                   |
| -------- | -------------------------------------------- | --------------------------------------- |
| Rust     | `steamgriddb_api` (PhilipK)                  | Uses reqwest 0.11 (outdated), read-only |
| JS/TS    | `steamgriddb` (SteamGridDB/node-steamgriddb) | Official wrapper, reqwest/axios-based   |
| Python   | `python-steamgriddb`                         | PyPI, maintained                        |

**Recommendation for CrossHook**: Do NOT add these as dependencies. Use `reqwest` (already in the crate) with direct HTTP calls, matching the ProtonDB client pattern exactly.

**Confidence**: High for library existence / Medium for maintenance status

---

## Image Handling Libraries

### Rust-Side

The codebase already uses `reqwest` for HTTP in `crosshook-core`. No new Rust crates are required for basic image download-and-cache.

**Pattern** (already established by ProtonDB client):

```rust
let client = reqwest::Client::builder()
    .timeout(Duration::from_secs(10))
    .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
    .build()?;

let bytes = client.get(&image_url).send().await?.bytes().await?;

// Write to filesystem cache
let cache_path = app_cache_dir.join("images").join(&steam_app_id).join("library_600x900.jpg");
tokio::fs::create_dir_all(cache_path.parent().unwrap()).await?;
tokio::fs::write(&cache_path, &bytes).await?;
```

**Optional crates** (evaluated, not required for MVP):

| Crate                | Version | Use case                                        | Status      |
| -------------------- | ------- | ----------------------------------------------- | ----------- |
| `image`              | 0.25    | Image format validation/conversion              | Active, MIT |
| `http-cache-reqwest` | 0.14    | HTTP-level caching middleware (cacache backend) | Active      |

**Recommendation**: Use `std::fs` / `tokio::fs` with `reqwest` for MVP. Add `image` only if format validation is required. `http-cache-reqwest` adds complexity that is unnecessary given the custom SQLite+filesystem cache already used by this app.

**Confidence**: High

### TypeScript / Frontend Side

**Approach 1 (Recommended): `convertFileSrc` + Tauri asset protocol**

```typescript
import { convertFileSrc } from '@tauri-apps/api/core';

// imagePath is returned from a Tauri invoke command
const assetUrl = convertFileSrc(imagePath);
// Result: asset://localhost/home/user/.local/share/crosshook/cache/images/...
// Use directly in <img src={assetUrl} />
```

**Required `tauri.conf.json` configuration**:

```json
"security": {
  "csp": "default-src 'self' ipc: http://ipc.localhost; img-src 'self' asset: http://asset.localhost",
  "assetProtocol": {
    "enable": true,
    "scope": ["$APPDATA/**/*", "$CACHE/**/*"]
  }
}
```

**Important Linux note**: On Linux, `~/.local/share/` is a hidden directory path. The scope must explicitly include it. Using `$APPDATA` and `$CACHE` predefined variables handles this automatically and does not require hardcoding paths.

**Approach 2 (Fallback): Base64 data URI via Tauri fs plugin**

```typescript
import { readBinaryFile, BaseDirectory } from '@tauri-apps/plugin-fs';

const data = await readBinaryFile('cache/images/{appId}/library_600x900.jpg', {
  baseDir: BaseDirectory.AppData,
});
const base64 = btoa(String.fromCharCode(...new Uint8Array(data)));
const dataUri = `data:image/jpeg;base64,${base64}`;
```

This approach works but is 33% larger in memory due to base64 encoding and requires reading the full file into the renderer process. The `convertFileSrc` approach is preferred.

**Skeleton loading**: CSS-only skeleton pattern (no library needed):

```css
.crosshook-card-art--loading {
  background: linear-gradient(
    90deg,
    var(--crosshook-color-surface) 25%,
    var(--crosshook-color-surface-raised) 50%,
    var(--crosshook-color-surface) 75%
  );
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}

@keyframes shimmer {
  0% {
    background-position: 200% 0;
  }
  100% {
    background-position: -200% 0;
  }
}
```

**Confidence**: High — convertFileSrc pattern confirmed in Tauri v2 official docs and GitHub discussions

---

## Integration with Existing Infrastructure

### Cache Key Namespacing

The `external_cache_entries` table uses `cache_key TEXT NOT NULL UNIQUE` as the primary lookup key. The ProtonDB pattern uses the format `{namespace}:{id}`.

**Established pattern** (from `protondb/models.rs:9-22`):

```rust
pub const PROTONDB_CACHE_NAMESPACE: &str = "protondb";
pub fn cache_key_for_app_id(app_id: &str) -> String {
    format!("{PROTONDB_CACHE_NAMESPACE}:{}", app_id.trim())
}
```

**Proposed cache key namespaces for Steam metadata**:

| Data                    | Cache Key Format               | Storage Location                                                             | TTL                    |
| ----------------------- | ------------------------------ | ---------------------------------------------------------------------------- | ---------------------- |
| Steam app metadata JSON | `steam:appdetails:v1:{app_id}` | `external_cache_entries`                                                     | 24–48 hours            |
| SteamGridDB grid result | `steamgriddb:grid:v1:{app_id}` | `external_cache_entries`                                                     | 48 hours               |
| SteamGridDB hero result | `steamgriddb:hero:v1:{app_id}` | `external_cache_entries`                                                     | 48 hours               |
| Image binary file       | Filesystem path                | `game_image_cache` table + `~/.local/share/crosshook/cache/images/{app_id}/` | Indefinite (no expiry) |

### Metadata JSON vs. Binary Image Split

The existing `external_cache_entries` table stores `payload_json TEXT` — it is designed for **JSON payloads only**, not binary data. Steam metadata (genres, descriptions, image URL strings) belongs there.

Image binary files must be stored separately on the **filesystem** with a dedicated `game_image_cache` SQLite table tracking metadata only (paths, checksums, fetch timestamps). This table does not exist yet and requires a new migration.

### Proposed `game_image_cache` Table (New — Migration v14)

```sql
CREATE TABLE IF NOT EXISTS game_image_cache (
    cache_id        TEXT PRIMARY KEY,
    steam_app_id    TEXT NOT NULL,
    image_type      TEXT NOT NULL,   -- 'library_600x900', 'header', 'hero', etc.
    image_source    TEXT NOT NULL,   -- 'steam_cdn', 'steamgriddb'
    file_path       TEXT NOT NULL,   -- absolute path on disk
    content_type    TEXT,            -- 'image/jpeg', 'image/png'
    file_size       INTEGER,
    fetched_at      TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE (steam_app_id, image_type, image_source)
);
CREATE INDEX IF NOT EXISTS idx_game_image_cache_steam_app_id
    ON game_image_cache(steam_app_id);
```

**Filesystem layout**:

```
~/.local/share/crosshook/cache/images/
└── {steam_app_id}/
    ├── library_600x900.jpg   (from Steam CDN)
    ├── header.jpg             (from Steam CDN, fallback)
    └── sgdb_grid.jpg          (from SteamGridDB, optional)
```

### MetadataStore Integration

The existing `MetadataStore::put_cache_entry` / `get_cache_entry` methods in `metadata/cache_store.rs` are ready to use for Steam metadata JSON with no changes. The Steam metadata lookup follows the same flow as ProtonDB:

1. Check `external_cache_entries` for a valid (non-expired) row with `cache_key = "steam:appdetails:v1:{app_id}"`
2. If miss, fetch from `https://store.steampowered.com/api/appdetails?appids={app_id}`
3. Upsert via `metadata_store.put_cache_entry(source_url, cache_key, payload_json, expires_at)`
4. For images: check `game_image_cache` for an existing file, download to filesystem if absent

### SteamGridDB API Key in Settings

The `AppSettingsData` struct (in `settings/mod.rs`) currently holds `auto_load_last_profile`, `last_used_profile`, `community_taps`, `onboarding_completed`, and `offline_mode`. A new field `steamgriddb_api_key: Option<String>` should be added to this struct.

**Storage classification**: This is a user-editable preference → **TOML settings** (`settings.toml` via `SettingsStore`). Never SQLite metadata, never environment variable.

```rust
// In AppSettingsData
pub steamgriddb_api_key: Option<String>,
```

The key is optional: if absent, SteamGridDB lookup is skipped entirely (graceful degradation).

---

## Degraded Fallback Behavior

| Condition                            | Behavior                                                         |
| ------------------------------------ | ---------------------------------------------------------------- |
| No `steam_app_id` on profile         | Text-only card, no art fetch attempted                           |
| Steam API unavailable / 429          | Show skeleton → text-only card; cache the failure for 15 min     |
| Steam CDN `library_600x900.jpg` 404  | Fall back to `header.jpg`; if also 404, fall back to placeholder |
| No `steamgriddb_api_key` in settings | SteamGridDB step skipped entirely                                |
| SteamGridDB API 401 / 429            | Skip SteamGridDB; proceed with Steam-only art                    |
| No art available from any source     | Show themed placeholder (game controller icon + game name)       |
| Image file deleted from cache        | Re-fetch on next load; do not crash                              |

---

## Constraints and Gotchas

### CrossHook-Specific (Pass 1 — Preserved)

1. **No Tailwind**: shadcn/ui and Tailwind-first libraries are incompatible.
2. **@radix-ui/react-tabs already in package.json** — extending its use is zero cost. The sub-tab CSS classes are already defined in `theme.css`.
3. **Nested Tabs.Root architecture**: The app's outer `Tabs.Root` uses `orientation="vertical"` for page-level routing. Sub-tabs within ProfilesPage must be a **separate nested `Tabs.Root`** with `orientation="horizontal"`.
4. **Composition constraint**: Sub-tabs must be composed at the `ProfilesPage` level, not inside `ProfileFormSections`, due to `InstallPage` modal reuse of `ProfileFormSections`.
5. **CSS variables vs. inline styles**: Prefer CSS classes with design token variables per CLAUDE.md convention.

### Image Caching Specific (Pass 2 — New)

1. **Asset protocol scope on Linux**: `~/.local/share/` is a hidden directory. Must use `$APPDATA` predefined scope variable or explicitly include the hidden path in `tauri.conf.json` `assetProtocol.scope`.
2. **MAX_CACHE_PAYLOAD_BYTES guard**: The existing `put_cache_entry` in `metadata/cache_store.rs` enforces a maximum payload size (stores NULL if exceeded). Steam metadata JSON for a single game is typically 2–5KB, well within limits.
3. **Steam CDN URL stability**: The CDN domain `cdn.cloudflare.steamstatic.com` is widely used but not officially documented. Always use the URL from the `appdetails` response (`header_image` field) as the authoritative source rather than constructing URLs manually.
4. **Rate limit on appdetails**: The `~200 req / 5 min` limit applies per IP. For a desktop app with user's own IP, this is not a practical concern given 24-hour cache TTL and on-demand (not background-batch) fetch.
5. **SteamGridDB API key security**: The key must be stored in `settings.toml` (TOML settings tier), never in SQLite or memory-only. When displayed in SettingsPanel, it should be masked (password input type), and care should be taken not to log it.
6. **`short_description` is HTML**: Steam returns HTML strings for description fields. These must be stripped before display using a safe HTML-to-text utility on the Rust side (e.g., a simple regex strip or a lightweight crate), never rendered as raw HTML in the WebView.

### Tauri / WebKitGTK Linux

1. **WebKitGTK 4.1 (Linux)**: Full CSS Grid, Flexbox, CSS custom properties, CSS transitions — all supported.
2. **Asset protocol v2**: In Tauri v2, the CSP must explicitly include `img-src 'self' asset: http://asset.localhost` for `convertFileSrc` URLs to load. Missing this causes silent image 403s.

---

## Code Examples

### Sub-tabs with existing Radix install (nested root, horizontal orientation)

```tsx
// Uses @radix-ui/react-tabs already in package.json
// Uses .crosshook-subtab-row / .crosshook-subtab / .crosshook-subtab--active already in theme.css
import * as Tabs from '@radix-ui/react-tabs';

export function ProfilesPage() {
  return (
    <Tabs.Root defaultValue="general" orientation="horizontal">
      <Tabs.List className="crosshook-subtab-row" aria-label="Profile sections">
        <Tabs.Trigger className="crosshook-subtab" value="general">
          General
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-subtab" value="runtime">
          Runtime
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-subtab" value="advanced">
          Advanced
        </Tabs.Trigger>
      </Tabs.List>
      <Tabs.Content value="general">{/* Game path, profile name, ProtonDB lookup */}</Tabs.Content>
      <Tabs.Content value="runtime">{/* Proton version, env vars, working directory */}</Tabs.Content>
      <Tabs.Content value="advanced">{/* Steam launch options, less common settings */}</Tabs.Content>
    </Tabs.Root>
  );
}
```

### Rust Tauri command: fetch and cache Steam metadata

```rust
// crosshook-core/src/steam/client.rs (new module)
use reqwest::Client;
use crate::metadata::MetadataStore;

const STEAM_APPDETAILS_URL: &str = "https://store.steampowered.com/api/appdetails";
const STEAM_CACHE_NAMESPACE: &str = "steam:appdetails:v1";
const CACHE_TTL_HOURS: i64 = 48;

pub fn cache_key_for_app_id(app_id: &str) -> String {
    format!("{STEAM_CACHE_NAMESPACE}:{}", app_id.trim())
}

pub async fn fetch_steam_metadata(
    metadata_store: &MetadataStore,
    app_id: &str,
) -> Option<SteamAppMetadata> {
    let cache_key = cache_key_for_app_id(app_id);

    // Try cache first
    if let Ok(Some(payload)) = metadata_store.get_cache_entry(&cache_key) {
        return serde_json::from_str(&payload).ok();
    }

    // Fetch live
    let client = Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))  // 6s — matches protondb/client.rs
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

    let url = format!("{STEAM_APPDETAILS_URL}?appids={app_id}");

    // appdetails response shape: {"{app_id}": {"success": bool, "data": {...}}}
    // Use a typed HashMap — avoids serde_json::Value, consistent with the reqwest .json::<T>() pattern
    // used throughout the existing codebase (see protondb/client.rs).
    let wrapper: std::collections::HashMap<String, SteamAppDetailsEntry> =
        client.get(&url).send().await.ok()?.json().await.ok()?;
    let entry = wrapper.into_values().next().filter(|e| e.success)?;
    let metadata: SteamAppMetadata = entry.data?;

    let payload = serde_json::to_string(&metadata).ok()?;
    let expires_at = (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339();
    let _ = metadata_store.put_cache_entry(&url, &cache_key, &payload, Some(&expires_at));

    Some(metadata)
}
```

### Rust Tauri command: download and cache image to filesystem

```rust
// Returns the local file path (to be converted by frontend with convertFileSrc)
pub async fn fetch_cached_image(
    app_handle: &tauri::AppHandle,
    steam_app_id: &str,
    image_type: &str,        // e.g., "library_600x900"
    image_url: &str,
) -> Option<PathBuf> {
    let cache_dir = app_handle
        .path()
        .app_cache_dir()
        .ok()?
        .join("images")
        .join(steam_app_id);

    let filename = format!("{image_type}.jpg");
    let file_path = cache_dir.join(&filename);

    // Return cached if exists
    if file_path.exists() {
        return Some(file_path);
    }

    // Download
    let client = Client::new();
    let bytes = client.get(image_url).send().await.ok()?.bytes().await.ok()?;

    tokio::fs::create_dir_all(&cache_dir).await.ok()?;
    tokio::fs::write(&file_path, &bytes).await.ok()?;

    Some(file_path)
}
```

### Frontend: display cached image with convertFileSrc

```tsx
import { convertFileSrc } from '@tauri-apps/api/core';
import { invoke } from '@tauri-apps/api/core';
import { useState, useEffect } from 'react';

function GameCoverArt({ steamAppId }: { steamAppId: string }) {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    if (!steamAppId) return;
    invoke<string | null>('get_game_cover_art', { steamAppId })
      .then((filePath) => {
        if (filePath) setSrc(convertFileSrc(filePath));
      })
      .catch(() => setSrc(null));
  }, [steamAppId]);

  if (!src) {
    return <div className="crosshook-card-art crosshook-card-art--placeholder" />;
  }

  return <img className="crosshook-card-art" src={src} alt="" loading="lazy" onError={() => setSrc(null)} />;
}
```

---

## Open Questions

### Pass 1 (Advanced Section Restructuring)

1. **Which sections are actually in the collapsed Advanced area?** Full enumeration of `ProfileFormSections.tsx` sections needed to decide what to promote vs. keep collapsed.
2. **Does the existing Tabs install render anywhere currently?** If yes, where — to understand whether extending it for sub-tabs introduces visual inconsistency.
3. **Controller mode (`data-crosshook-controller-mode`)**: Sub-tab touch targets need to scale appropriately (variables `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` are already defined).

### Pass 2 (Game Metadata and Cover Art)

4. **Settings UI for SteamGridDB API key**: Does the existing `SettingsPanel.tsx` have an appropriate section for external API keys? Or does a new section need to be designed?
5. **Profile card grid vs. list view**: The Figma concept shows a grid/list toggle — is this in scope for issue #52 or a separate follow-up feature?
6. **Which profile list component renders profile cards?** Need to identify the component that maps `ProfileListItem` → card to know where `GameCoverArt` should be injected.
7. **Steam Store API HTML in `short_description`**: Should the Rust side strip HTML before caching, or should the frontend handle it? Stripping on the Rust side is safer (avoids XSS risk in WebView even with Tauri's CSP).
8. **SteamGridDB art freshness**: Community art is updated by contributors. Should the 48-hour TTL apply to SGDB art as well, or should it be longer (e.g., 7 days) since individual game art changes rarely?

---

## Sources

### Radix UI / Frontend Libraries

- [Radix UI Tabs primitives docs](https://www.radix-ui.com/primitives/docs/components/tabs)
- [Radix UI Accordion primitives docs](https://www.radix-ui.com/primitives/docs/components/accordion)
- [shadcn/ui Tabs](https://ui.shadcn.com/docs/components/radix/tabs)
- [Headless UI Tabs](https://headlessui.com/react/tabs)
- [Ark UI home](https://ark-ui.com/)
- [CrabNebula — Best UI Libraries for Tauri](https://crabnebula.dev/blog/the-best-ui-libraries-for-cross-platform-apps-with-tauri/)
- [Nielsen Norman Group — Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/)
- [@radix-ui/react-accordion on npm](https://www.npmjs.com/package/@radix-ui/react-accordion)
- [@radix-ui/react-tabs on npm](https://www.npmjs.com/package/@radix-ui/react-tabs)

### Steam Store API

- [User:RJackson/StorefrontAPI — TF2 Wiki (community-maintained appdetails docs)](https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI)
- [Better Steam Web API Documentation](https://steamwebapi.azurewebsites.net/)
- [Steamworks Graphical Assets Overview](https://partner.steamgames.com/doc/store/assets)
- [Getting the actual 600x900 Library assets — Steam Community discussion](https://steamcommunity.com/discussions/forum/1/4202490864582293420/)
- [Feature request: Add steam cover image fallback using GetItems API — DLSS Swapper #661](https://github.com/beeradmoore/dlss-swapper/issues/661)
- [Gathering Data from the Steam Store API using Python — Nik Davis](https://nik-davis.github.io/posts/2019/steam-data-collection/)
- [Steam Web API constantly rate-limited (Error 429) — Steam Community](https://steamcommunity.com/discussions/forum/1/601902348018676495/)
- [AFCMS/SteamFetch — CLI for downloading Steam library artworks](https://github.com/AFCMS/SteamFetch)

### SteamGridDB API

- [SteamGridDB API v1 (deprecated reference)](https://www.steamgriddb.com/api/v1)
- [SteamGridDB/node-steamgriddb — official JS wrapper](https://github.com/SteamGridDB/node-steamgriddb)
- [PhilipK/steamgriddb_api — Rust wrapper](https://github.com/PhilipK/steamgriddb_api)
- [steamgriddb_api on crates.io](https://crates.io/crates/steamgriddb_api)
- [steamgriddb_api response module docs](https://docs.rs/steamgriddb_api/latest/steamgriddb_api/response/)
- [steamgriddb npm package](https://www.npmjs.com/package/steamgriddb)
- [SteamGridDB Changelog](https://changelog.steamgriddb.com/)

### Image Handling / Tauri

- [Implementing Image Caching with Tauri — Medium](https://losefor.medium.com/implementing-image-caching-with-tauri-enhancing-performance-and-offline-access-6a55c2dbc802)
- [Display an image using the asset protocol — Tauri v2 Discussion #11498](https://github.com/tauri-apps/tauri/discussions/11498)
- [Tauri v2 File System plugin](https://v2.tauri.app/plugin/file-system/)
- [convertFileSrc — Tauri v2 core JS reference](https://v2.tauri.app/reference/javascript/api/namespacecore/)
- [How to download images with Rust — ScrapingAnt](https://scrapingant.com/blog/download-image-rust)
- [http-cache-reqwest crate](https://crates.io/crates/http-cache-reqwest)
- [reqwest crate](https://crates.io/crates/reqwest)

---

## Search Queries Executed

**Pass 1 (preserved)**:

1. `Radix UI tabs accordion components React desktop app Tauri 2026`
2. `shadcn/ui tabs collapsible components progressive disclosure settings page React TypeScript`
3. `Headless UI React tab component keyboard navigation accessibility desktop`
4. `React settings page layout patterns sidebar navigation sub-tabs desktop 2024 2025`
5. `ark-ui zagjs react component library headless tabs accordion npm 2024 2025`
6. `Radix UI npm package size @radix-ui/react-tabs @radix-ui/react-accordion bundle 2024`
7. `Tauri v2 React UI library compatibility bundle size WebView limitations 2024 2025`
8. `progressive disclosure UX pattern settings page advanced options design system React`
9. `React vertical tab sidebar settings layout no external dependency custom CSS 2024`
10. `Tauri v2 WebKitGTK Linux CSS grid flexbox sub-tabs sidebar layout performance 2024`

**Pass 2 (new)**:

11. `Steam Store API appdetails endpoint response format image URLs rate limits 2024 2025`
12. `SteamGridDB API authentication endpoints grids heroes logos rate limits documentation 2024 2025`
13. `Steam store API appdetails response JSON header_image capsule_image library_600x900 background_raw fields example 2024`
14. `Steam CDN image URLs library_600x900 apps/{appid}/library_600x900.jpg community static pattern 2024`
15. `SteamGridDB API v2 swagger documentation grids heroes icons logos by steam app id response format dimensions 2024`
16. `Steam Store appdetails API genres categories short_description background screenshots JSON response complete example`
17. `SteamGridDB API v2 bearer token authentication grids endpoint dimensions formats developer docs`
18. `Rust image download crate reqwest tokio cache filesystem Tauri 2024 2025 image caching pattern`
19. `Tauri v2 image caching rust command download image filesystem invoke frontend display cached image pattern 2024 2025`
20. `Tauri v2 asset protocol convertFileSrc local file serve image frontend display cached filesystem 2024 2025`
21. `http-cache-reqwest crate Rust HTTP caching middleware 2024 reqwest 0.12 features`
22. `Steam app image URL patterns steam/apps/{appid} header.jpg capsule_616x353.jpg no authentication CDN 2024`

---

## Uncertainties and Gaps

- **SteamGridDB rate limits**: Not officially published. Treat as "polite personal use" and apply aggressive local caching (48-hour TTL).
- **SteamGridDB API stability**: The service is community-run. The API has been stable for several years but has no SLA. The feature design must treat it as optional/degradable.
- **Steam CDN URL construction**: The `library_600x900.jpg` URL pattern is widely documented in community tools but not in Valve's official Steamworks docs. Constructing it from `{cdn_base}/{app_id}/library_600x900.jpg` is standard practice; validate availability at runtime with a HEAD or GET request.
- **Steam appdetails `background` field**: The `background` field URL pattern includes a hash component that makes it non-constructible without an API call — it cannot be derived from the app ID alone.
- **Exact bundle sizes**: Individual Radix package sizes are approximate (~90kB); bundlephobia may have updated figures.
- **SteamGridDB image dimensions**: The exact list of dimension strings accepted by the `?dimensions=` filter parameter was not confirmed from official docs — derived from community wrapper code.
- **`short_description` HTML stripping**: No specific Rust HTML stripping crate was researched in depth. A simple regex or a lightweight crate like `ammonia` (allow-list HTML sanitizer) or `scraper` (HTML parser) could be used; this needs a separate evaluation.
