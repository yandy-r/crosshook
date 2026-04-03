# Research: External APIs for proton-app-id Art and Metadata

## Executive Summary

> **Implementation status note (updated 2026-04-02):** Both Steam CDN and SteamGridDB integration are **already implemented** in `crosshook-core`. The initial research was conducted without knowledge of the existing code. This document has been updated to reflect actual implementation state. See §11 (Codebase Reality) for the authoritative picture.

Two primary sources provide game art for Proton games with a known Steam App ID:

1. **Steam CDN** — deterministic URL patterns (no auth, no SDK) return cover/portrait/hero images directly from `cdn.cloudflare.steamstatic.com`. Zero API calls needed for the three art types CrossHook requires once an appid is known.
2. **SteamGridDB (SGDB)** — community-curated art (grids/heroes/logos/icons) with a free API (API key required). Covers the full tri-art system (cover = grid, portrait = vertical grid, background = hero). The best fallback when Steam CDN lacks an image or the user prefers community art.

IGDB (Twitch auth, Twitch account required) is a viable distant third option but adds credential complexity and is unnecessary given Steam CDN + SGDB coverage. It is noted but not recommended as a primary integration target.

**Confidence**: High — multiple independent sources, official Steamworks docs, and active open-source implementations (steamgriddb_api Rust crate, SteamTinkerLaunch, Heroic, steamgrid Go tool) corroborate all major findings.

---

## Primary APIs

### 1. Steam CDN — Direct Image URL Patterns

### Overview

Steam stores all published game assets on a public CDN. No API key is required. Images are retrieved via predictable URL patterns given an appid. This is the lowest-friction source for Steam games that have a known `steam_app_id`.

**CDN Base**: `https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/`
**Legacy CDN Base**: `https://cdn.akamai.steamstatic.com/steam/apps/{appid}/`
**Store Asset Base**: `https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/{appid}/`

### Known Stable URL Patterns

| Art Type             | CrossHook Role      | URL Pattern                    | Dimensions | Format   |
| -------------------- | ------------------- | ------------------------------ | ---------- | -------- |
| Header               | Cover (horizontal)  | `{cdn}/header.jpg`             | 920×430    | JPEG     |
| Library Capsule      | Portrait (vertical) | `{cdn}/library_600x900.jpg`    | 600×900    | JPEG/PNG |
| Library Capsule 2x   | Portrait (HiDPI)    | `{cdn}/library_600x900_2x.jpg` | 1200×1800  | JPEG     |
| Library Hero         | Background          | `{cdn}/library_hero.jpg`       | 3840×1240  | JPEG     |
| Library Hero (small) | Background          | `{cdn}/library_hero_blur.jpg`  | 1920×620   | JPEG     |
| Capsule 616x353      | Cover (wide)        | `{cdn}/capsule_616x353.jpg`    | 616×353    | JPEG     |
| Library Header       | Library banner      | `{cdn}/capsule_231x87.jpg`     | 231×87     | JPEG     |

**Steamworks-documented library asset specs:**

- Library Capsule: 600×900px PNG (primary); 300×450px auto-generated half-size
- Library Header: 920×430px PNG
- Library Hero: 3840×1240px PNG (primary); 1920×620px auto-generated half-size
- Library Logo: 1280px wide / 720px tall PNG with transparent background

### Art Type Mapping to CrossHook Tri-Art System

| CrossHook Art Slot | Steam CDN File                           | Fallback                |
| ------------------ | ---------------------------------------- | ----------------------- |
| Cover              | `library_600x900.jpg` (portrait capsule) | `header.jpg`            |
| Portrait           | `library_600x900.jpg`                    | `capsule_616x353.jpg`   |
| Background         | `library_hero.jpg`                       | `library_hero_blur.jpg` |

### Authentication and Rate Limits

- **Auth**: None required — all URLs are publicly accessible
- **Rate limits**: Not officially documented; no hot-linking policy found
- **Reliability**: Steam CDN uses Cloudflare + Akamai globally, high availability
- **Policy note**: The Steam API ToU (100k calls/day) applies to the Steam Web API (`api.steampowered.com`), not to CDN image downloads

**Confidence**: High — patterns documented in Steamworks official docs and confirmed by multiple community tools (steamgrid, SteamFetch, dlss-swapper)

### Store API: `appdetails` Endpoint

For retrieving metadata + image URLs programmatically:

```
GET https://store.steampowered.com/api/appdetails?appids={appid}
```

Response includes `header_image`, `capsule_image`, `capsule_imagev5` fields. This API is undocumented and unofficial ("not meant for public consumption"), subject to change, and may return 429 under heavy load. For CrossHook, direct CDN URL construction is preferred over this endpoint.

**Confidence**: Medium — widely used but officially unsupported

---

## 2. SteamGridDB API

**Docs**: <https://www.steamgriddb.com/api/v2>
**Base URL**: `https://www.steamgriddb.com/api/v2`
**Auth**: `Authorization: Bearer <api_key>` header (free, requires SteamGridDB account)
**Key obtainable at**: <https://www.steamgriddb.com/profile/preferences/api>

### Asset Types and Mapping

| SGDB Asset Type       | CrossHook Art Slot   | Notes                             |
| --------------------- | -------------------- | --------------------------------- |
| **Grid (vertical)**   | Portrait             | 600×900 — primary portrait source |
| **Grid (horizontal)** | Cover                | 460×215 or 920×430                |
| **Hero**              | Background           | 1920×620 or 3840×1240             |
| **Logo**              | Overlay only         | 1280×720 transparent PNG          |
| **Icon**              | Not needed initially | 16×16 to 256×256 PNG/ICO          |

### Key Endpoints

```
# Look up SGDB game ID from Steam App ID
GET /api/v2/games/steam/{steam_app_id}

# Search for a game by name
GET /api/v2/search/autocomplete/{term}

# Fetch grids (cover + portrait)
GET /api/v2/grids/game/{sgdb_game_id}
GET /api/v2/grids/steam/{steam_app_id}    # direct Steam appid lookup

# Fetch heroes (background)
GET /api/v2/heroes/game/{sgdb_game_id}
GET /api/v2/heroes/steam/{steam_app_id}

# Fetch logos
GET /api/v2/logos/game/{sgdb_game_id}
GET /api/v2/logos/steam/{steam_app_id}

# Fetch icons
GET /api/v2/icons/game/{sgdb_game_id}
GET /api/v2/icons/steam/{steam_app_id}
```

### Query Parameters

All image type endpoints accept:

- `dimensions` — comma-separated list, e.g. `600x900,1200x1800`
- `styles` — `alternate`, `blurred`, `white_logo`, `material`, `no_logo`
- `types` — `static`, `animated`
- `nsfw` — `true`, `false`, `any` (default: `false`)
- `humor` — `true`, `false`, `any` (default: `false`)

### Response Format

```json
{
  "success": true,
  "data": [
    {
      "id": 12345,
      "score": 100,
      "style": "material",
      "url": "https://cdn2.steamgriddb.com/grid/abc123.jpg",
      "thumb": "https://cdn2.steamgriddb.com/grid/thumb/abc123.jpg",
      "tags": [],
      "author": {
        "name": "username",
        "steam64": "76561...",
        "avatar": "https://..."
      },
      "width": 600,
      "height": 900,
      "mime": "image/jpeg"
    }
  ]
}
```

Width/height metadata included in responses (added 2021).

### Authentication Error Handling

HTTP 401 response = invalid/expired API key. Clients should clear stored key and prompt re-entry.

### Rate Limits

No publicly documented per-hour or per-day rate limits found. The API requires authentication (Bearer token), which implies server-side tracking. Recommendation: implement exponential backoff on 429 responses. SteamTinkerLaunch and steamgrid tools do not implement explicit rate limiting — they rely on sequential per-game requests at natural human pace.

**Confidence**: Medium — rate limit specifics are undocumented; behavior inferred from community tool implementations

### Pricing

- **Free** for API key registration
- No paid tiers discovered

### Terms of Service

Terms page renders via JavaScript; exact ToS content was not extractable. Based on community usage patterns (SteamTinkerLaunch, Heroic, Cartridges, steamgrid all use it freely), personal/application use with API key appears unrestricted. Formal verification recommended before production release.

**Confidence**: Low — ToS content not parsed; community precedent suggests permissive use

---

## 3. Steam Web API (Official)

**Base URL**: `https://api.steampowered.com/`
**Auth**: API key (free, <https://steamcommunity.com/dev/apikey>)
**Rate Limit**: 100,000 calls/day
**Docs**: <https://partner.steamgames.com/doc/webapi_overview>

Relevant interfaces:

- `ISteamApps` — app list, build history
- `IStoreBrowseService` — store browsing (undocumented details)
- `IStoreService` — GetAppList
- `IProductInfoService` — product information

The official Steam Web API does **not** expose library art URLs directly. It provides game metadata (name, genres, tags) but not the CDN image paths. For art, direct CDN URL construction (Section 1) is preferred over API calls.

**ToU restrictions relevant to CrossHook:**

- 100,000 calls/day max
- Must keep API key confidential
- Cannot degrade Steam performance
- No endorsement/affiliation implied

---

## 4. IGDB (Twitch/IGDB) — Evaluated, Not Recommended

**URL**: <https://www.igdb.com/api>
**Auth**: Twitch OAuth2 client credentials (requires Twitch account + 2FA)
**Rate limits**: 4 requests/second (free tier)
**Rust client**: `igdb-rs` (<https://github.com/CarlosLanderas/igdb-rs>)

IGDB provides cover art, screenshots, and metadata. It can look up games by Steam appid via the `external_games` endpoint. However:

- Requires Twitch developer account (external identity dependency)
- OAuth2 token refresh adds complexity
- SteamGridDB + Steam CDN covers the same art use case with simpler auth
- No direct Steam appid → art URL shortcut (requires game search + cover lookup)

**Not recommended** as a primary integration for CrossHook's proton-app-id feature. Could be a future fallback if SGDB coverage is insufficient for a game.

---

## 5. Libraries and SDKs

### Rust Crates

#### `steamgriddb_api` (Recommended, with caveats)

- **crates.io**: <https://crates.io/crates/steamgriddb_api>
- **GitHub**: <https://github.com/PhilipK/steamgriddb_api>
- **Author**: PhilipK
- **Last commit**: August 21, 2021 (inactive)
- **Dependencies**: `reqwest`, `serde`
- **Supports**: Search, `get_images_for_id`, platform ID URL builder
- **QueryType variants**: `Grid` confirmed; Hero/Logo/Icon likely present but only Grid is documented in README
- **Limitation**: Read-only (no upload); last maintained in 2021

```rust
use steamgriddb_api::{Client, QueryType::Grid};

let client = Client::new("your_api_key");
let games = client.search("Portal 2").await?;
let game = games.iter().next().ok_or("not found")?;
let images = client.get_images_for_id(game.id, &Grid(None)).await?;
// images[0].url contains the image URL
```

Platform-based lookup (avoids extra search step when appid is known):

```rust
use steamgriddb_api::{get_images_by_platform_ids_url, QueryType::Grid};

let url = get_images_by_platform_ids_url(
    "https://www.steamgriddb.com/api/v2",
    &Platform::Steam,
    &["570"],  // Dota 2 Steam appid
    &Grid(None),
);
// url is the endpoint to GET
```

**Verdict**: Functional but unmaintained since 2021. Usable for the initial implementation; consider forking or writing a thin wrapper over `reqwest` directly for Hero/Logo/Icon support if `QueryType` variants are incomplete.

#### `steam-web-api` / `steam-api-client`

- Read-only wrappers for the official Steam Web API
- Not needed for art downloads (Steam CDN URLs are constructed, not queried)
- Potentially useful if metadata (game name, genres) from the official API is needed

#### `reqwest` (Async HTTP — already a Tauri/Rust standard)

For direct CDN downloads, `reqwest` alone is sufficient:

```rust
use reqwest::Client;
use tokio::fs;

async fn download_steam_art(appid: u32, dest: &std::path::Path) -> anyhow::Result<()> {
    let url = format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
        appid
    );
    let client = Client::new();
    let bytes = client.get(&url).send().await?.bytes().await?;
    fs::write(dest, bytes).await?;
    Ok(())
}
```

#### `image` crate (optional, for validation/resizing)

- **crates.io**: <https://crates.io/crates/image>
- Formats: AVIF, BMP, EXR, GIF, HDR, ICO, JPEG, PNG, QOI, TGA, TIFF, WebP
- Synchronous; wrap in `tokio::task::spawn_blocking` for async context
- Only needed if CrossHook must validate/resize/transcode downloaded assets

#### `http-cache-reqwest` (optional, for HTTP caching)

- **docs.rs**: <https://docs.rs/http-cache-reqwest>
- Adds HTTP caching layer to `reqwest::Client`
- Respects `Cache-Control` headers
- Useful for reducing redundant CDN hits during app restarts

### Node/JS (frontend) — Not Applicable

CrossHook's art download logic should live in `crosshook-core` (Rust), not the React frontend. Frontend only displays art via `convertFileSrc` for local files.

---

## Integration Patterns

### 6. From Existing Launchers

### SteamTinkerLaunch (Bash)

- Calls SGDB API with Steam appid directly: `/grids/steam/{appid}`, `/heroes/steam/{appid}`, `/logos/steam/{appid}`
- Downloads first result from each endpoint
- Allows user to override by re-running with explicit SGDB game ID
- Does not implement rate limiting — sequential downloads at human interaction pace

### steamgrid (Go — boppreh/steamgrid)

- Steam CDN first, SGDB fallback chain
- Configurable filters: `styles`, `types`, `nsfw`, `humor`, `dimensions`
- HTTP timeout 10s, no explicit backoff
- Constructs filter query string dynamically, skipping params set to "any"

### Heroic Games Launcher (TypeScript)

- Auto-downloads from SGDB when user adds a sideload game by title
- Uses SGDB search (`/search/autocomplete/{title}`) to find game ID, then fetches first grid result
- Known limitation: first result sometimes incorrect; issue filed for browsable search

### Cartridges (Python/GTK4)

- Automatic cover download from SteamGridDB on game import
- Supports animated covers (types=animated)
- Open source: <https://codeberg.org/kramo/cartridges>

### Pattern Summary

Common architecture across all launchers:

```
1. Resolve game ID:
   - If steam_app_id known → use /games/steam/{appid} or direct /grids/steam/{appid}
   - Else → /search/autocomplete/{name} → pick first result

2. For each art type [cover, portrait, background]:
   a. Try Steam CDN URL (if steam_app_id available) — no API call
   b. If CDN returns 404/error → call SGDB endpoint
   c. Cache result locally

3. Resolution priority:
   custom_upload > auto_downloaded > placeholder
```

---

## 7. Constraints and Gotchas

### Steam CDN

- URL patterns are stable but not officially documented — Valve could change file names
- Some older games may lack `library_600x900.jpg` (portrait capsule) — fall back to `header.jpg`
- Library Hero (`library_hero.jpg`) was added in the Steam library redesign; pre-2019 games may lack it
- CDN returns HTTP 404 (not 200 with error) for missing assets — easy to detect

### SteamGridDB

- API key is user-scoped — CrossHook must store the user's SGDB key (sensitive credential)
- No anonymous access; all requests require Bearer token
- Responses contain both `url` (full) and `thumb` (thumbnail) — download full for storage
- Some games return 0 results for strict dimension filters; use permissive defaults then filter locally
- `steamgriddb_api` Rust crate is unmaintained (last 2021); may not expose all QueryType variants

### Rate Limits

| Source           | Limit           | Notes                                                    |
| ---------------- | --------------- | -------------------------------------------------------- |
| Steam CDN        | None documented | Behave respectfully; no hammering                        |
| SGDB API         | Not published   | Implement 429 backoff                                    |
| Steam Web API    | 100k/day        | Not needed for art; apply only if metadata endpoint used |
| Store appdetails | Not published   | Unofficial endpoint; use sparingly                       |

### Security

- SGDB API key must be stored in CrossHook's credential store (not TOML config in plaintext)
- Image downloads come from user-provided appids — validate appid is numeric before constructing URLs
- Downloaded images should be stored in `$LOCALDATA/crosshook/media/` with sanitized filenames
- Do not display remote images directly in WebView — download locally, serve via `convertFileSrc`

### Offline Scenarios

- Steam CDN and SGDB are internet-dependent — CrossHook must gracefully show placeholder art when offline
- SQLite cache should record `last_fetched` timestamp and image path for offline serving
- Art already downloaded remains available offline indefinitely

---

## 8. Code Examples

> **Note:** The sketches below were written before the existing implementation was discovered. See §11 for the actual code. These are retained for reference on the API shape.

### Full Art Resolution Chain (Rust sketch)

```rust
use reqwest::Client;

/// Download cover art for a Proton profile with a known Steam app ID.
/// Tries Steam CDN first, falls back to SteamGridDB.
pub async fn resolve_cover_art(
    http: &Client,
    steam_app_id: u32,
    sgdb_api_key: Option<&str>,
) -> anyhow::Result<Option<Vec<u8>>> {
    // 1. Try Steam CDN (portrait capsule)
    let cdn_url = format!(
        "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900.jpg",
        steam_app_id
    );
    let resp = http.get(&cdn_url).send().await?;
    if resp.status().is_success() {
        return Ok(Some(resp.bytes().await?.to_vec()));
    }

    // 2. Fallback: SteamGridDB vertical grid
    if let Some(key) = sgdb_api_key {
        let sgdb_url = format!(
            "https://www.steamgriddb.com/api/v2/grids/steam/{}?dimensions=600x900",
            steam_app_id
        );
        let sgdb_resp = http
            .get(&sgdb_url)
            .bearer_auth(key)
            .send()
            .await?;

        if sgdb_resp.status().is_success() {
            let body: serde_json::Value = sgdb_resp.json().await?;
            if let Some(image_url) = body["data"][0]["url"].as_str() {
                let img = http.get(image_url).send().await?.bytes().await?;
                return Ok(Some(img.to_vec()));
            }
        }
    }

    Ok(None) // Both sources failed; show placeholder
}
```

### Steam CDN Background Art

```rust
let hero_url = format!(
    "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_hero.jpg",
    steam_app_id
);
```

### SteamGridDB Hero Lookup

```rust
let url = format!(
    "https://www.steamgriddb.com/api/v2/heroes/steam/{}",
    steam_app_id
);
let resp = client
    .get(&url)
    .bearer_auth(sgdb_key)
    .send()
    .await?
    .json::<serde_json::Value>()
    .await?;
let hero_url = resp["data"][0]["url"].as_str();
```

---

## 9. Open Questions

1. **SGDB terms of service**: The ToS page requires JavaScript to render; exact commercial/application usage terms were not parsed. Verify before CrossHook ships SGDB integration.

2. **~~`steamgriddb_api` QueryType completeness~~**: Resolved — CrossHook does not use the `steamgriddb_api` crate. Integration is implemented directly with `reqwest` in `steamgriddb.rs`.

3. **Steam CDN portrait fallback gap**: Pre-2019 games may lack `library_600x900_2x.jpg`. Already mitigated — `portrait_candidate_urls()` in `client.rs` tries `_2x.jpg` → `library_600x900.jpg` → `header.jpg` in sequence.

4. **SGDB API key storage**: The current `fetch_steamgriddb_image` accepts `api_key: &str` — storage location is a caller concern. Where the key is persisted (TOML vs SQLite vs keyring) is not yet determined. Avoid TOML plaintext.

5. **Background art type**: Adding `GameImageType::Background` would map to `heroes/steam/{app_id}` in `build_endpoint()` — same SGDB endpoint as `Hero`. The `Hero` and `Background` types would be identical at the SGDB API level; they differ only in CrossHook's usage context. This collision needs a deliberate decision: share the endpoint, or differentiate via dimension filtering.

6. **Animated covers**: SGDB supports `types=animated` (WEBP/APNG). Out of scope for initial implementation but the API supports it via the `types` query param.

7. **`download_url` stored for SteamGridDB in DB**: `client.rs:310` hardcodes the grid endpoint URL regardless of image type. For `Hero`-type images this records `/grids/steam/{id}` instead of `/heroes/steam/{id}`. Minor data quality issue; no functional impact.

---

## 11. Codebase Reality (updated 2026-04-02)

This section records the actual state of the implementation discovered after initial research was written.

### Files

| File                                                   | Role                                                            |
| ------------------------------------------------------ | --------------------------------------------------------------- |
| `crates/crosshook-core/src/game_images/steamgriddb.rs` | SteamGridDB fetch — `fetch_steamgriddb_image`, `build_endpoint` |
| `crates/crosshook-core/src/game_images/client.rs`      | HTTP singleton, CDN URLs, download/cache pipeline, validation   |

### Existing `GameImageType` variants

```rust
GameImageType::Cover    // SGDB: grids?dimensions=460x215,920x430  / CDN: header.jpg
GameImageType::Hero     // SGDB: heroes (no dim filter)             / CDN: library_hero.jpg
GameImageType::Capsule  // SGDB: grids?dimensions=342x482,600x900  / CDN: capsule_616x353.jpg
GameImageType::Portrait // SGDB: grids?dimensions=342x482,600x900  / CDN: library_600x900_2x.jpg (with fallback chain)
```

`Background` does **not** exist yet. Adding it requires:

1. New `GameImageType::Background` variant
2. `build_endpoint` arm → `("heroes", None)` (same as `Hero`)
3. `build_download_url` arm → `library_hero.jpg` (same as `Hero`)
4. `filename_for` arm → `"background"`

### Actual fallback order (client.rs)

With `api_key = Some(key)`:

1. SteamGridDB API (type-specific endpoint)
2. Steam CDN (type-specific URL; portrait uses 3-URL candidate chain)
3. Stale cache hit (expired but file still on disk)
4. `None`

With `api_key = None`:

1. Steam CDN
2. Stale cache
3. `None`

### HTTP client spec (client.rs)

- `reqwest::Client` singleton via `OnceLock`
- Timeout: 15 seconds
- User-Agent: `CrossHook/{CARGO_PKG_VERSION}`
- TLS: `rustls-tls-webpki-roots` (confirmed via `cargo tree`)
- 5 MB streaming cap with chunk-by-chunk enforcement
- Magic-byte validation via `infer` crate: allow-list is `image/jpeg`, `image/png`, `image/webp`
- SVG and HTML unconditionally rejected

### SteamGridDB CDN domain (confirmed 2026-04-02)

API responses return image URLs hosted on **`cdn2.steamgriddb.com`** (e.g. `https://cdn2.steamgriddb.com/file/sgdb-cdn/grid/<hash>.png`). The hypothetical `img.steamgriddb.com` does **not** appear in any known API responses. Older responses may reference `s3.amazonaws.com/steamgriddb/` (legacy, pre-CDN-migration).

For the redirect-policy domain allow-list (finalized by security-researcher 2026-04-02):

- `cdn.cloudflare.steamstatic.com` — Steam CDN primary
- `steamcdn-a.akamaihd.net` — Steam CDN legacy
- `www.steamgriddb.com` — SGDB API origin
- `cdn2.steamgriddb.com` — SGDB image CDN

HTTPS only. This is the complete allow-list for `GAME_IMAGES_HTTP_CLIENT`.

### `webp` image codec dependency (confirmed absent 2026-04-02)

`cargo tree -p crosshook-core | grep -v webpki | grep -i webp` returns **no output**. All `webp`-named entries in the tree are `rustls-webpki` and `webpki-roots` — TLS certificate crates unrelated to RUSTSEC-2024-0443. The advisory does not apply to crosshook-core.

### Security controls already in place

- `app_id` validated as pure decimal digits before any URL construction
- `safe_image_cache_path` canonicalizes and prefix-checks against base dir
- Downloaded filenames are generated internally, never from remote response
- Image bytes validated by magic-byte detection before write to disk
- `api_key` excluded from tracing spans via `skip(api_key)`

### Security gaps requiring implementation action (from security-researcher, 2026-04-02)

| ID        | Severity                          | Gap                                                               | Required action                                                                                                                                                                                            |
| --------- | --------------------------------- | ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S-01/S-06 | WARNING                           | No redirect-policy domain allow-list on `GAME_IMAGES_HTTP_CLIENT` | Add `.redirect(Policy::custom(...))` restricting redirects to the four allow-listed domains above, HTTPS only                                                                                              |
| S-02      | WARNING                           | `settings_load` IPC command returns raw SGDB API key to frontend  | Return `has_steamgriddb_api_key: bool` only; never send the key value to the frontend                                                                                                                      |
| S-12      | WARNING (escalated from ADVISORY) | HTTP 401/403 from SGDB falls through to stale-cache silently      | Detect 401/403 separately from 429/network errors; fall back to Steam CDN (not stale cache) on auth failure; surface a distinguishable error state so UI can prompt "API key invalid — update in Settings" |
| S-13      | ADVISORY                          | `app_id` decimal check accepts strings longer than a valid u32    | Add 12-digit length cap (Steam app IDs are ≤10 digits; 12 gives safe headroom)                                                                                                                             |
| S-15      | ADVISORY                          | SGDB key stored in TOML plaintext                                 | Future hardening: `keyring` crate (cross-platform) if pursued; not blocking for initial release                                                                                                            |

---

## 10. Search Queries Executed

1. `Steam Web API game art images endpoints documentation 2024 2025`
2. `SteamGridDB API documentation endpoints authentication rate limits 2024`
3. `Steam CDN image URL patterns header capsule library art appid`
4. `Rust crates Steam API SteamGridDB game art download crates.io 2024`
5. `Lutris Heroic Bottles game launcher art download Steam SteamGridDB implementation`
6. `Steam store API appdetails endpoint artwork images capsule header hero format 2024`
7. `SteamGridDB API v2 grids heroes logos icons endpoints image dimensions authentication key 2024`
8. `SteamGridDB icons endpoint API v2 dimensions PNG transparent background game icon`
9. `SteamGridDB API rate limiting per day per hour free tier anonymous requests`
10. `Cartridges game launcher art download SteamGridDB implementation Python source`
11. `steam web API terms of use rate limits 100000 requests commercial use restrictions`
12. `SteamGridDB grids dimensions hero dimensions icons PNG`
13. `SteamTinkerLaunch SteamGridDB implementation bash script download hero grid logo icon`
14. `IGDB API game art cover art free alternative to SteamGridDB metadata Rust`
15. `SteamGridDB changelog 2024 2025 API v3 new features rate limits`
16. `Tauri v2 Rust async file download save image local storage pattern media cache`
17. `steam store API IStoreBrowseService GetItems appid game art metadata 2024`
18. `Rust image crate imageproc image processing resize download async 2024`
19. `SteamTinkerLaunch SteamGridDB bash API endpoints hero grid logo`
20. `Heroic Games Launcher SteamGridDB art download implementation code GitHub 2024`

---

## Sources

- [Steamworks Library Assets Documentation](https://partner.steamgames.com/doc/store/assets/libraryassets)
- [Steamworks Store Graphical Assets](https://partner.steamgames.com/doc/store/assets/standard)
- [Steam Web API Overview (Steamworks)](https://partner.steamgames.com/doc/webapi_overview)
- [Steam Web API Terms of Use](https://steamcommunity.com/dev/apiterms)
- [SteamGridDB API v1 (deprecated)](https://www.steamgriddb.com/api/v1)
- [SteamGridDB Terms of Service](https://www.steamgriddb.com/terms)
- [SteamGridDB Changelog](https://changelog.steamgriddb.com/)
- [steamgriddb_api Rust crate — crates.io](https://crates.io/crates/steamgriddb_api)
- [steamgriddb_api GitHub (PhilipK)](https://github.com/PhilipK/steamgriddb_api)
- [node-steamgriddb (official JS wrapper)](https://github.com/SteamGridDB/node-steamgriddb)
- [SteamGridDB.NET (craftersmine)](https://github.com/craftersmine/SteamGridDB.NET)
- [UWPHook SteamGridDbApi.cs (C# reference implementation)](https://github.com/BrianLima/UWPHook/blob/master/UWPHook/SteamGridDb/SteamGridDbApi.cs)
- [steamgrid Go tool (boppreh)](https://github.com/boppreh/steamgrid)
- [SteamTinkerLaunch SteamGridDB wiki](https://github.com/sonic2kk/steamtinkerlaunch/wiki/SteamGridDB)
- [Heroic Games Launcher GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher)
- [Cartridges game launcher (Codeberg)](https://codeberg.org/kramo/cartridges)
- [Steam Storefront API (unofficial, RJackson)](https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI)
- [SteamGridDB API docs (Duke Yin)](https://tech.dukeyin.com/2023/06/15/steamgriddb-api/)
- [IGDB API](https://www.igdb.com/api)
- [reqwest crate](https://github.com/seanmonstar/reqwest)
- [http-cache-reqwest docs](https://docs.rs/http-cache-reqwest)
- [image crate](https://crates.io/crates/image)
- [Tauri v2 File System plugin](https://v2.tauri.app/plugin/file-system/)
