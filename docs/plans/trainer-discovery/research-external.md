# External APIs Research: Trainer Discovery

## Executive Summary

No trainer site (FLiNG, WeMod, CheatHappens, MrAntiFun) exposes a documented public API for programmatic trainer discovery. Access must be achieved via:

1. **Steam Web API** (official, free, rate-limited) for installed game version detection via `buildid` fields in local `.acf` manifest files and remote `appdetails` endpoint
2. **PCGamingWiki Cargo/MediaWiki API** (official, free, open) for game metadata cross-referencing
3. **IGDB API** (official, OAuth-gated, free tier) for canonical game metadata
4. **ProtonDB community API** (unofficial, free) — already integrated in CrossHook for compatibility tiers
5. **Web scraping** of trainer sites (FLiNG's XenForo-based forum, MrAntiFun) as a fallback, with legal and rate-limit caveats
6. **Community tap metadata extension** as the primary long-term architecture — CrossHook's existing tap infrastructure is the cleanest solution

CrossHook must NOT host or redistribute trainer binaries. The feature links to download pages only.

**Confidence**: High (based on direct documentation review and multiple corroborating sources)

### Codebase reality check (post-research correction)

After reading the actual codebase, several research recommendations are superseded by existing infrastructure:

- **VDF parsing**: `keyvalues-parser` crate is **not needed** — CrossHook already has a hand-rolled VDF parser at `steam/vdf.rs` (`parse_vdf()`) that handles all `.acf` manifest formats. `steam/manifest.rs` already has `parse_manifest_full()` which extracts `build_id`, `install_dir`, `state_flags`, and `last_updated`.
- **Steam Appdetails API**: Already fully implemented in `steam_metadata/client.rs` (`lookup_steam_metadata()`), with 24-hour cache TTL using `external_cache_entries`.
- **HTTP client pattern**: Three `OnceLock<reqwest::Client>` singletons exist (`PROTONDB_HTTP_CLIENT`, `STEAM_METADATA_HTTP_CLIENT`, one in `game_images`). The trainer discovery module should add its own `TRAINER_DISCOVERY_HTTP_CLIENT` following the exact same pattern (6s timeout, `CrossHook/{version}` User-Agent, `rustls-tls`). Do **not** share the ProtonDB or Steam metadata clients.
- **Cache layer**: `metadata/cache_store.rs` `put_cache_entry()` / `get_cache_entry()` is the established pattern. `MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB) — RSS feed payloads from FLiNG will be well under this limit; individual trainer page payloads will be too.
- **Cache → live → stale fallback**: The three-stage pattern (`load_cached_lookup_row(allow_expired=false)` → `fetch_live_*()` → `load_cached_lookup_row(allow_expired=true)`) is the project standard. Any new external API client must implement this same flow.
- **No auth infrastructure exists**: IGDB requires OAuth (Twitch Client Credentials). Since no token storage or refresh infrastructure exists in crosshook-core, IGDB should be deferred or avoided unless the feature scope explicitly includes building auth infrastructure. Stick to unauthenticated sources for initial implementation.
- **`reqwest` features**: Project uses `json` + `rustls-tls`. No additional features needed for trainer discovery (no multipart, no cookies required).

---

## Primary APIs

### 1. Steam Web API (Store Appdetails)

**Purpose**: Detect installed game version, resolve Steam AppID from game name, fetch canonical game metadata.

**Docs**: <https://partner.steamgames.com/doc/webapi/ISteamApps>  
**Community docs**: <https://steamapi.xpaw.me/>  
**Auth**: Free API key via <https://steamcommunity.com/dev>  
**Rate limits**: 100,000 requests/day for `api.steampowered.com`; `store.steampowered.com` throttles at ~200 requests per 5-minute window  
**Pricing**: Free

**Key endpoints**:

```
GET https://store.steampowered.com/api/appdetails/?appids={appid}
```

Returns: name, steam_appid, required_age, is_free, detailed_description, release_date, developers, publishers, platforms

```
GET https://api.steampowered.com/ISteamApps/GetAppList/v2/
```

Returns: full list of all Steam apps (appid + name) — cacheable, use for name→appid resolution

**Local version detection (preferred for offline)**: Parse `~/.local/share/Steam/steamapps/appmanifest_{appid}.acf` — extract `buildid` field. This is a VDF/KeyValue format.

**Already implemented in CrossHook** (no new crates needed):

- `steam/vdf.rs::parse_vdf()` — full VDF text parser
- `steam/manifest.rs::parse_manifest_full()` — parses `.acf` and returns `ManifestData { build_id, install_dir, state_flags, last_updated }`
- `steam/manifest.rs::find_game_match()` — scans all Steam library steamapps dirs to find which manifest matches a given game executable path

**Also already implemented**: `steam_metadata/client.rs::lookup_steam_metadata()` calls `https://store.steampowered.com/api/appdetails?appids={app_id}` with 24h cache TTL. The trainer discovery module should call this existing function rather than re-implementing the Steam API call.

**Confidence**: High — official Valve documentation, widely used; implementation verified in codebase

---

### 2. PCGamingWiki Cargo/MediaWiki API

**Purpose**: Cross-reference game by Steam AppID to retrieve canonical name, compatibility metadata, and Wine/Proton notes.

**Docs**: <https://www.pcgamingwiki.com/wiki/PCGamingWiki:API>  
**GitHub**: <https://github.com/PCGamingWiki/api>  
**Auth**: None required for read queries  
**Rate limits**: Not documented; treat as a public wiki — cache aggressively  
**Pricing**: Free

**Key endpoints**:

```
# Lookup by Steam AppID → wiki page redirect
GET https://pcgamingwiki.com/api/appid.php?appid={steam_appid}

# Structured Cargo query by Steam AppID
GET https://www.pcgamingwiki.com/w/api.php
  ?action=cargoquery
  &tables=Infobox_game
  &fields=Infobox_game._pageName,Infobox_game.Developers,Infobox_game.Released
  &where=Infobox_game.Steam_AppID HOLDS "{steam_appid}"
  &format=json
```

**Confidence**: High — documented on official PCGamingWiki pages

---

### 3. IGDB API (via Twitch OAuth)

**Purpose**: Canonical game metadata (canonical name, versions, release dates, platforms) for games not on Steam.

**Docs**: <https://api-docs.igdb.com/>  
**Auth**: OAuth 2.0 via Twitch Client Credentials (`client_id` + `client_secret` → access token)  
**Rate limits**: 4 requests/second on free tier  
**Pricing**: Free tier sufficient for discovery use

**Key endpoints** (POST with Apicalypse query body):

```
POST https://api.igdb.com/v4/games
Authorization: Bearer {token}
Client-ID: {client_id}
Body: fields name, slug, first_release_date, platforms, versions; search "{game_name}"; limit 5;

POST https://api.igdb.com/v4/game_versions
fields game, features, games, populated_sortable_value, url;
where game = {game_id};
```

**Rust crates**:

- `igdb-atlas` (2024, active) — <https://crates.io/crates/igdb-atlas>
- `igdb` fork by playonbsd-rs (updated for current API) — <https://github.com/playonbsd-rs/igdb-rs>
- `igdb-api-rust` (protobuf/PROST, type-safe) — <https://github.com/lephyrius/igdb-api-rust>

**Confidence**: High — official documentation, well-maintained ecosystem

---

### 4. ProtonDB Community API (Unofficial)

**Purpose**: Compatibility tier lookup by Steam AppID — already integrated in CrossHook.

**Endpoint already known to CrossHook**:

```
GET https://www.protondb.com/api/v1/reports/summaries/{appId}.json
```

**Community API** (open-source, 31-day data refresh cycle):

- <https://protondb.max-p.me/> (OpenAPI docs)
- Endpoints: `/api/v2/games?appid={appId}`, `/api/v2/reports?appid={appId}`

**Confidence**: High — CrossHook already uses this

---

### 5. FLiNG Trainer Site (Web Scraping / RSS)

**Purpose**: Discover available trainers, version strings, and download page URLs for a given game.

**Site**: <https://flingtrainer.com/>  
**Auth**: None for public listing pages  
**Technology**: XenForo v2 forum software  
**Rate limits**: Must be self-imposed; no published limits. Recommend ≥10s between requests.

**Access patterns (no public API)**:

a) **RSS feed** (XenForo built-in):

```
GET https://flingtrainer.com/category/trainer/feed/
```

Returns XML with latest trainer posts including title (game name + version + option count), link, and publish date. This is the cleanest non-API approach.

b) **Category page scraping**:

```
GET https://flingtrainer.com/category/trainer/
```

Parse HTML for trainer post titles (contain game name + version), links to individual trainer pages.

c) **Individual trainer page** — scrape for:

- Game name
- Trainer version string (typically matches game build/version in title)
- Download link (links out to OneDrive/Google Drive or file host — not direct binary)
- Supported game version(s)

**HTML parsing**: Use `scraper` crate (CSS selectors) + `reqwest` in Rust.

**Important**: FLiNG's trainer download links point to external file hosts (OneDrive, Google Drive). CrossHook should store the trainer page URL, not the file host URL — the latter can expire.

**Confidence**: Medium — site structure confirmed by community tools; RSS availability inferred from XenForo standard behavior, needs verification

---

### 6. MrAntiFun (WeMod-integrated, Forum Scraping)

**Purpose**: Historical/additional trainer source. Note: MrAntiFun joined WeMod; old standalone trainers archived by community.

**Current status**: MrAntiFun's trainers are now in WeMod. Independent site still exists at <https://mrantifun.net/> (XenForo-based), but new releases go through WeMod.

**WeMod API (Unofficial/Reverse-Engineered)**:

- Base: `https://api.wemod.com`
- Game trainer endpoint (discovered via community reverse engineering):

  ```
  GET https://api.wemod.com/v3/games/{GameId}/trainer?gameVersions=&locale=en-US&v=2
  ```

- **Legal risk**: WeMod ToS likely prohibits automated access. This endpoint is NOT documented and using it may violate ToS.
- **Recommendation**: Do NOT integrate WeMod's undocumented API. Provide WeMod as a manual link in trainer discovery UI only.

**Confidence**: Low — unofficial endpoint, ToS risk, not recommended for integration

---

### 7. CheatHappens (No Public API)

**Purpose**: Additional trainer source. Subscription-gated.

**Status**: No public API exists. All access requires AURORA desktop app or membership login. Trainers are subscription-gated.

**Recommendation**: Provide a link to CheatHappens search URL for a game:

```
https://www.cheathappens.com/search.asp?q={game_name}
```

No programmatic trainer data available.

**Confidence**: High (confidence in absence of API)

---

## Libraries and SDKs

### Rust Crates Recommended

| Crate                  | Version | Purpose                         | Status                                     | Notes                                                                       |
| ---------------------- | ------- | ------------------------------- | ------------------------------------------ | --------------------------------------------------------------------------- |
| `reqwest`              | 0.12+   | HTTP client                     | Already in project (`json` + `rustls-tls`) | Do not add new HTTP clients                                                 |
| `scraper`              | 0.26+   | HTML parsing with CSS selectors | New dependency needed                      | Only if FLiNG RSS unavailable                                               |
| `serde` + `serde_json` | 1.x     | JSON serialization              | Already in project                         |                                                                             |
| `tokio`                | 1.x     | Async runtime                   | Already in project                         |                                                                             |
| `keyvalues-parser`     | —       | VDF/ACF parsing                 | **NOT NEEDED**                             | `steam/vdf.rs` already handles this                                         |
| `keyvalues-serde`      | —       | Serde for VDF                   | **NOT NEEDED**                             | `steam/manifest.rs` already provides `parse_manifest_full()`                |
| `http-cache-reqwest`   | —       | HTTP caching middleware         | **NOT NEEDED**                             | `metadata/cache_store.rs` + SQLite already provide this                     |
| `reqwest-middleware`   | —       | Middleware stack                | **NOT NEEDED**                             | Project uses direct `reqwest::Client` calls                                 |
| `governor`             | latest  | Rate limiting                   | New dependency if needed                   | Only needed if scraping FLiNG; consider a simple `tokio::time::sleep` first |
| `igdb-atlas`           | —       | IGDB API bindings               | **DEFERRED**                               | No auth infrastructure in project; avoid OAuth complexity in Phase 1        |

**Note**: The codebase already implements VDF parsing, Steam `.acf` manifest parsing, Steam appdetails API lookup, and HTTP caching via SQLite. The only genuine new library candidate is `scraper` for HTML parsing if FLiNG RSS is unavailable.

### External Service SDKs

- **Steam Web API**: Already implemented in `steam_metadata/client.rs`. Call `lookup_steam_metadata()` — do not re-implement.
- **PCGamingWiki**: Use raw `reqwest` + `serde_json`. No SDK needed.
- **IGDB**: Defer to Phase 2; requires building OAuth token refresh infrastructure from scratch.

---

## Integration Patterns

### Pattern 1: Local Version Detection (Offline-First)

**Use existing infrastructure — no new code needed:**

```rust
// steam/manifest.rs::parse_manifest_full() already implements this
// steam/libraries.rs provides find_steam_libraries() to locate steamapps dirs
// trainer_discovery can accept a SteamGameMatch (from steam/manifest.rs) as input
// and read build_id directly from ManifestData
```

The `parse_manifest_full()` function already validates that `build_id` is numeric-only and handles all edge cases (missing fields, corrupted manifests).

### Pattern 2: Steam AppID → Canonical Game Info

**Use existing infrastructure — no new code needed:**

```rust
// steam_metadata/client.rs::lookup_steam_metadata() already implements this
// It follows cache → live → stale-fallback with 24h TTL
// Returns SteamMetadataLookupResult { app_id, state, app_details, from_cache, is_stale }
```

### Pattern 3: Trainer Page Discovery via RSS

New code required. Fetch FLiNG RSS feed, parse XML entries, cache with 1h TTL in `external_cache_entries`:

```rust
// Each RSS item title format: "Game Name v{version} (+{N} Trainer)"
// Store: game_name (normalized), trainer_version, trainer_page_url, option_count
// Cache key: "trainer_discovery::fling_index" with 1h TTL
// On fetch failure: fall back to expired cache row (same pattern as protondb/client.rs)
```

### Pattern 4: HTTP Caching

**Use existing infrastructure — no new middleware needed:**

CrossHook's `metadata/cache_store.rs` + SQLite `external_cache_entries` is the project-standard cache. `http-cache-reqwest` middleware would add dependencies for functionality already provided.

Rate limiting for scraping: a simple `tokio::time::sleep(Duration::from_secs(10))` between FLiNG page requests is sufficient. Reserve `governor` crate for Phase 2 if scraping at higher frequency.

- FLiNG RSS: fetch at most once per hour (TTL enforced by cache)
- FLiNG individual trainer page scrapes: 10-15s delay between requests if scraping in bulk

### Pattern 5: Community Tap Metadata Extension

The cleanest long-term approach extends CrossHook's existing tap infrastructure. A tap index would include trainer metadata:

```toml
# community tap trainer metadata entry (proposed format)
[[trainers]]
game_name = "Elden Ring"
steam_appid = 1245620
trainer_name = "Elden Ring v1.12 +25 Trainer"
trainer_version = "1.12"
game_version_buildid = "15395736"
source = "fling"
download_page_url = "https://flingtrainer.com/elden-ring-trainer/"
sha256 = ""  # empty until user downloads and verifies
options = ["Infinite HP", "Infinite FP", "No Weight Limit"]
last_verified = "2025-11-01"
```

---

## Constraints and Gotchas

### Legal Constraints

1. **Linking vs hosting**: Linking to trainer download pages is generally lower legal risk than hosting files. CrossHook explicitly does NOT redistribute binaries — this is correct and must be maintained. **Confidence**: Medium (legal landscape is fact-specific; consult counsel for production release)

2. **DMCA anti-circumvention**: Trainers that bypass DRM may trigger DMCA §1201 issues for the trainer creator, but indexing/linking is distinct from creating/hosting. Precedent (hiQ v. LinkedIn) supports legality of indexing publicly accessible links. **Confidence**: Low (legal uncertainty; jurisdiction-dependent)

3. **WeMod ToS**: WeMod's Terms of Service likely prohibit automated access to their API. Do NOT integrate undocumented WeMod endpoints. Provide WeMod as a "search" link only (e.g., `https://www.wemod.com/cheats/{slug}-trainers`). **Confidence**: High

4. **FLiNG scraping**: FLiNG's site does not have a documented Terms of Service against scraping, but best practices require respecting `robots.txt` and rate limits. **Confidence**: Medium

### Rate Limiting Strategy

- Cache Steam `appdetails` responses for 24 hours (game metadata rarely changes)
- Cache RSS feeds for 1 hour (trainer releases are infrequent)
- Store trainer metadata in CrossHook's existing `external_cache_entries` SQLite table
- Implement exponential backoff on 429/503 responses
- For scraping: minimum 10-15 second delay between requests

### Data Freshness

- Trainer versions are typically coupled to game patch cycles (not daily)
- Game `buildid` from Steam changes with each game update
- RSS feeds from FLiNG are sufficient for weekly/on-demand refresh
- No need for realtime data — nightly or on-demand cache refresh is appropriate

### Offline-First Requirements

CrossHook's existing `external_cache_entries` table already supports this pattern:

- Trainer discovery metadata can be stored in SQLite with `expires_at` TTL
- Local `.acf` file parsing requires no network — works fully offline
- Community tap trainer metadata (TOML) can be bundled with tap updates

### Version Matching Complexity

Trainer sites do NOT consistently use Steam `buildid`. They typically use:

- Game version string (e.g., "v1.12.3") — extracted from the game executable or Steam changelog
- Their own versioning ("v2024.01.15")
- Steam `buildid` is more reliable for automation but requires Steam-specific lookups

Matching strategy: fuzzy match on game name + best-effort version string comparison. Store as separate fields, not a foreign key constraint.

---

## Code Examples

### Local Version Detection (Use Existing Infrastructure)

No new code or crates needed. Call the existing manifest parser directly:

```rust
// In crosshook-core: steam/manifest.rs already exports these
use crate::steam::manifest::parse_manifest_full;
use crate::steam::libraries::find_steam_libraries; // see steam/libraries.rs

// Get build_id for a known Steam app:
let manifest_path = steamapps_dir.join(format!("appmanifest_{appid}.acf"));
match parse_manifest_full(&manifest_path) {
    Ok(data) => {
        // data.build_id: String — numeric build ID, empty if not present
        // data.install_dir: String
        // data.state_flags: Option<u32>
        // data.last_updated: Option<u64>
    }
    Err(msg) => tracing::warn!(appid, "Failed to parse manifest: {msg}"),
}
```

### Steam App Metadata (Use Existing Infrastructure)

No new code needed. Call the existing `steam_metadata` module:

```rust
// In crosshook-core: steam_metadata/client.rs already exports this
use crate::steam_metadata::client::lookup_steam_metadata;

// Returns SteamMetadataLookupResult with app name, header_image, genres
// Handles cache → live → stale-fallback automatically
let result = lookup_steam_metadata(&metadata_store, &app_id, false).await;
```

### New HTTP Client for Trainer Discovery (Follow Established Pattern)

```rust
// In a new trainer_discovery/client.rs module — mirror protondb/client.rs
use std::sync::OnceLock;
use std::time::Duration;

const REQUEST_TIMEOUT_SECS: u64 = 6;
static TRAINER_DISCOVERY_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn trainer_discovery_http_client() -> Result<&'static reqwest::Client, TrainerDiscoveryError> {
    if let Some(client) = TRAINER_DISCOVERY_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(TrainerDiscoveryError::Network)?;
    let _ = TRAINER_DISCOVERY_HTTP_CLIENT.set(client);
    Ok(TRAINER_DISCOVERY_HTTP_CLIENT
        .get()
        .expect("trainer discovery HTTP client initialized"))
}
```

### Cache Key Convention (Follow Established Pattern)

```rust
// Cache key format mirrors protondb (protondb::{app_id}) and steam_metadata
const TRAINER_DISCOVERY_CACHE_NAMESPACE: &str = "trainer_discovery";

fn cache_key_for_game(normalized_game_name: &str) -> String {
    format!("{TRAINER_DISCOVERY_CACHE_NAMESPACE}::{normalized_game_name}")
}

// Use metadata_store.put_cache_entry(source_url, &cache_key, &payload, Some(&expires_at))
// and metadata_store.get_cache_entry(&cache_key) via with_sqlite_conn
// Cache TTL: 1 hour for RSS feed index, 24h for individual trainer page scrapes
```

### FLiNG RSS Feed Fetch (New Code Required)

```rust
// Fetch and parse the FLiNG RSS/Atom feed for new trainer entries
// Payload will be well under MAX_CACHE_PAYLOAD_BYTES (512 KiB)
async fn fetch_fling_rss_index(
    client: &reqwest::Client,
) -> Result<Vec<TrainerIndexEntry>, TrainerDiscoveryError> {
    // Verify this URL is live before relying on it:
    let rss_url = "https://flingtrainer.com/category/trainer/feed/";
    let xml_text = client
        .get(rss_url)
        .send()
        .await
        .map_err(TrainerDiscoveryError::Network)?
        .error_for_status()
        .map_err(TrainerDiscoveryError::Network)?
        .text()
        .await
        .map_err(TrainerDiscoveryError::Network)?;
    parse_rss_entries(&xml_text)
}
```

### PCGamingWiki Lookup by Steam AppID

```rust
// No SDK needed — one endpoint, no auth
pub async fn resolve_pcgw_page(
    client: &reqwest::Client,
    appid: u32,
) -> Result<Option<String>, TrainerDiscoveryError> {
    let url = format!("https://pcgamingwiki.com/api/appid.php?appid={appid}");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(TrainerDiscoveryError::Network)?;
    // PCGW returns a 302 redirect to the wiki article page
    Ok(Some(resp.url().to_string()))
}
```

---

## Open Questions

1. **FLiNG RSS feed URL**: Needs empirical verification that `https://flingtrainer.com/category/trainer/feed/` is live and well-formed. XenForo typically provides this but it may be disabled.

2. **FLiNG trainer page structure**: The HTML structure of individual trainer pages needs inspection to confirm CSS selectors for version strings, option lists, and download links.

3. **Version string matching**: No standard exists for correlating trainer version strings with Steam `buildid`. Need to evaluate fuzzy matching approaches (e.g., date-based heuristics, semantic version comparison) and acceptable false-positive rate.

4. **Community tap trainer metadata format**: The proposed TOML schema above is a strawman. Needs alignment with CrossHook's existing `community_profiles` table schema and tap format specification.

5. **IGDB rate limits for non-commercial use**: The free tier (4 req/s) is sufficient for on-demand lookup. Need to confirm whether CrossHook's use case qualifies for free tier long-term.

6. **MrAntiFun legacy trainers**: Community archives (~23 GB) of pre-WeMod MrAntiFun trainers exist. If community taps want to reference these, the index must clearly flag them as legacy/unsupported.

7. **Anti-scraping measures**: FLiNG may use Cloudflare or similar anti-bot protection. Need to test whether `reqwest` with a standard browser User-Agent string is sufficient, or if more sophisticated approaches are needed.

---

## Sources

- [FLiNG Trainer](https://flingtrainer.com/) — Primary trainer source site
- [WeMod Community API discussion](https://community.wemod.com/t/api/8373) — WeMod undocumented API thread
- [wemod-deck GitHub](https://github.com/wemod-deck/wemod-deck) — WeMod API reverse engineering
- [CheatHappens Trainer Index](https://www.cheathappens.com/trainers_index.asp?letter=I) — CheatHappens structure
- [MrAntiFun](https://mrantifun.net/) — Legacy trainer forum (WeMod-integrated)
- [Game Cheats Manager GitHub](https://github.com/dyang886/Game-Cheats-Manager) — Open-source multi-source trainer manager
- [FLiNG Trainer Collection GitHub](https://github.com/Melon-Studio/FLiNG-Trainer-Collection) — Community WPF client with SQLite
- [Steam Web API ISteamApps](https://partner.steamgames.com/doc/webapi/ISteamApps) — Official Valve documentation
- [Steam Store Appdetails](https://store.steampowered.com/api/appdetails/) — Storefront API
- [Steam Web API community docs](https://steamapi.xpaw.me/) — Unofficial comprehensive docs
- [PCGamingWiki API](https://www.pcgamingwiki.com/wiki/PCGamingWiki:API) — Official PCGW API documentation
- [IGDB API docs](https://api-docs.igdb.com/) — Official IGDB API
- [igdb-atlas crate](https://crates.io/crates/igdb-atlas) — 2024 Rust IGDB bindings
- [ProtonDB Community API](https://protondb.max-p.me/) — Community ProtonDB API
- [ProtonDB community-api GitHub](https://github.com/Trsnaqe/protondb-community-api) — API source
- [Nexus Mods GraphQL API](https://graphql.nexusmods.com/) — Integration pattern reference
- [Nexus Mods REST API docs](https://api-docs.nexusmods.com/) — REST pattern reference
- [keyvalues-parser crate](https://crates.io/crates/keyvalues-parser) — VDF/ACF parsing in Rust
- [keyvalues-serde crate](https://crates.io/crates/keyvalues-serde) — Serde integration
- [scraper crate](https://crates.io/crates/scraper) — HTML parsing in Rust
- [http-cache-reqwest](https://lib.rs/crates/http-cache-reqwest) — HTTP caching middleware
- [Ethical web scraping 2025](https://scrapingapi.ai/blog/ethical-web-scraping) — Rate limiting and robots.txt guidance
- [XenForo RSS feed discussion](https://xenforo.com/community/threads/rss-feed-from-a-specific-forum.219696/) — XenForo RSS support confirmation
- [SteamDB FAQ](https://steamdb.info/faq/) — SteamDB data sourcing methodology

## Search Queries Executed

1. "FLiNG trainer site API download index trainer metadata 2024 2025"
2. "WeMod API public endpoints trainer database documentation"
3. "Steam Web API game version detection appdetails manifest parsing"
4. "Nexus Mods API v2 public endpoints mod metadata GraphQL 2024"
5. "PCGamingWiki API mediawiki game information endpoints"
6. "rust crate scraper reqwest HTML parsing web scraping library 2024"
7. "MrAntiFun trainer download site structure RSS feed trainer metadata"
8. "CheatHappens trainer API download programmatic access game trainer index"
9. "game trainer legal considerations linking copyright DMCA distribution"
10. "SteamDB API game version build history public endpoints documentation"
11. "ProtonDB API public endpoints game compatibility data JSON"
12. "rust serde_json HTTP caching rate limiting reqwest middleware tower-http 2024"
13. "IGDB API game version detection metadata programmatic Rust client 2024"
14. "FLiNG trainer site web scraping RSS feed XenForo forum trainer list programmatic"
15. "Steam appmanifest acf file parsing local game version detection Rust"
16. "keyvalues-parser vdf-rs Rust crate Steam VDF KeyValue format parsing crates.io"
17. "rate limiting web scraping polite crawling respect robots.txt legal considerations 2024"
18. "game trainer metadata standard format JSON community database open source trainer index"

## Uncertainties and Gaps

- **No standard trainer metadata format exists** across the community — each tool (GCM, FLiNG Collection, WeMod) uses its own schema
- **FLiNG RSS feed is unverified** — XenForo supports it but the site may have disabled it; needs live testing
- **Version matching heuristics** require empirical data — there is no reliable algorithmic solution without building a mapping table
- **WeMod's data is locked behind their platform** — there is no legal path to programmatic access without ToS risk
- **Legal risk of scraping trainer sites** is low but non-zero — trainer sites themselves occupy legally ambiguous territory, which complicates the risk analysis for a tool that references them
- **IGDB does not track trainer availability** — it provides game metadata only, not trainer compatibility
