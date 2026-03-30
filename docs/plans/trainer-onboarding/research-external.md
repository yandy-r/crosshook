# External API & Library Research: Trainer Onboarding

**Feature:** Trainer Onboarding and Acquisition Guidance (GitHub Issue #37)
**Research Date:** 2026-03-30
**Researcher:** research-specialist

---

## Executive Summary

CrossHook's trainer-onboarding feature (issue #37) covers in-app guidance for finding trainers,
explaining loading modes, first-run readiness checks, and a guided workflow. This document covers
external APIs and libraries relevant to that feature.

**Scope determination (updated after cross-team review):**

### V1 — In scope, zero new dependencies required

All four readiness checks are implementable with existing code and zero new crates:

- Steam installed → `path.exists()` on known Steam roots (already `steam/discovery.rs`)
- Proton available → `path.is_file()` + executable bit (already `install/service.rs:298`)
- Game launched once → `launch_operations` query via `MetadataStore` OR `compatdata/` dir check
- Trainer downloaded → `path.exists() && path.is_file()` (already `install/service.rs:176`)

Key constraints confirmed:

- **No trainer site has a public API.** FLiNG, WeMod, CheatHappens all require manual user download.
- **No automated trainer download** — hard constraint. Onboarding provides guidance text + file picker only.
- **FLiNG is the primary free path** — recommended starting point for new users with `CopyToPrefix`.
- **WeMod requires separate setup** (wemod-launcher) and a WeMod account; must not be implied as frictionless.

### Future features — researched here, deferred to separate issues

- **Steam Store API** — free, no-auth game metadata (name, header image, linux flag). Useful for enriching
  a future onboarding wizard UI but not required for v1 readiness checks.
- **ProtonDB API** — free, no-auth compatibility tier lookup. Already adjacent to `CompatibilityPage.tsx`;
  a natural future addition, not MVP.
- **PE `MZ` magic check** — 2-byte check via `std::fs::File` is sufficient for "wrong file type" guard;
  no PE-parsing crate needed. Full version-string extraction (`goblin`/`pelite`) is a future enhancement.
- **ZIP extraction** — not in v1 spec (spec is guidance for _finding_ trainers, not auto-extracting them).
  `zip` crate is the right choice if this becomes a requirement; open a separate issue.

---

## Primary APIs

### Trainer Distribution Sources

### 1.1 FLiNG Trainer

**Website:** [flingtrainer.com](https://flingtrainer.com/) (primary) + mirror sites flingtrainer.us, flingtrainer.dev

**Distribution model:**

- No public API. Trainers are listed on individual WordPress-style post pages.
- Download links are direct `.zip` file links hosted on the site.
- An A–Z listing at [/all-trainers/](https://flingtrainer.com/all-trainers/) provides a browsable index.
- Archive of pre-2019 trainers at [/uncategorized/my-trainers-archive/](https://flingtrainer.com/uncategorized/my-trainers-archive/)

**File naming convention:**

```
Game.Name.vX.X.Plus.##.Trainer-FLiNG.zip
# Examples:
New.Cycle.EA.Plus.30.Trainer.FLiNG.zip
Flintlock.The.Siege.of.Dawn.v1.1.Plus.22.Trainer-FLiNG.zip
```

**Archive contents:**

- Outer: `.zip` archive
- Inner: one or more `.exe` files (the trainer binary), sometimes a `readme.txt`
- Some trainers include `start_protected_game.exe` for bypassing anti-cheat; users copy it to game dir

**Linux/Steam Deck usage pattern:**

- User downloads `.zip` manually, extracts `.exe`
- Places `.exe` in a SourceDirectory or copies into Proton prefix
- CrossHook's existing `SourceDirectory` and `CopyToPrefix` modes match this flow exactly

**Third-party scraper reference:** [Melon-Studio/FLiNG-Trainer-Collection](https://github.com/Melon-Studio/FLiNG-Trainer-Collection)
uses a Python crawler + SQLite to build a local index from flingtrainer.com HTML. It confirms
there is no API — only HTML scraping works.

**Confidence:** Medium (no official docs; patterns derived from file naming examples and third-party scraper)

---

### 1.2 WeMod

**Website:** [wemod.com](https://www.wemod.com/)

**Distribution model:**

- Fully proprietary Windows desktop app. No public API or documented file format.
- Internal DLL injection components: `TrainerLib-x64`, `CELib_x64` (referenced in community forums)
- MrAntiFun (previously a standalone trainer author) **exclusively distributes through WeMod** as of 2024.

**Linux compatibility:**

- [DeckCheatz/wemod-launcher](https://github.com/DeckCheatz/wemod-launcher) — Python + AGPL-3.0
  - Integrates via Steam launch options: `{path}/wemod %command%`
  - Manages a Proton/Wine prefix specifically for WeMod
  - Recommends GE-Proton; downloads pre-built Wine prefixes automatically
  - Actively maintained as of 2024–2025
- WeMod itself is in development for a first-party Linux/Steam Deck launcher per their community forums

**Integration recommendation for CrossHook:**

- Do NOT attempt programmatic integration. Guide users to install wemod-launcher separately.
- In onboarding UI, detect if wemod-launcher is present and display guidance accordingly.
- Document WeMod as a "wrapper trainer source" that requires its own setup.

**Confidence:** High (multiple community sources, active GitHub project)

---

### 1.3 CheatHappens

**Website:** [cheathappens.com](https://www.cheathappens.com/)

**Distribution model:**

- **Subscription-based** (paid). Most trainers require a premium account.
- Aurora app (all-in-one manager) and Trainer Manager 2.0 (standalone portable app) for Windows.
- No public API. Aurora app is proprietary.
- Free trainers exist at [/trainers_index_free.asp](https://www.cheathappens.com/trainers_index_free.asp)
- Distribution per game version/storefront (Steam vs Origin etc. trainers differ)

**Confidence:** High (official site documentation)

---

### 1.4 MrAntiFun

**Website:** [mrantifun.net](https://mrantifun.net/) (forum/info hub only)

As of 2024, MrAntiFun no longer distributes trainers directly — all new trainers are WeMod-exclusive.
The website remains as a community forum. Legacy trainers (pre-2020) are mirrored on third-party sites.

**Confidence:** High

---

### 1.5 Other Sources (GTrainers, MegaGames, TrainersCity)

These are **community mirrors** that re-host trainers from FLiNG, MrAntiFun, and others. No APIs.
Useful as fallback download links to surface in onboarding guidance text.

---

## 2. Steam Web API _(Future feature reference — not required for v1 onboarding)_

**Docs:** [partner.steamgames.com/doc/webapi_overview](https://partner.steamgames.com/doc/webapi_overview)
**Interactive reference:** [steamapi.xpaw.me](https://steamapi.xpaw.me/)

### 2.1 Store Details Endpoint (No Auth Required)

```
GET https://store.steampowered.com/api/appdetails?appids={appid}
```

- **Authentication:** None (public endpoint)
- **Rate limit:** ~200 requests / 5 minutes (unofficial; approximately 1 req/1.5s to be safe)
- **Response format:** JSON object keyed by appid

**Response fields relevant to onboarding:**

```json
{
  "1234567": {
    "success": true,
    "data": {
      "type": "game",
      "name": "Example Game",
      "steam_appid": 1234567,
      "header_image": "https://...",
      "platforms": {
        "windows": true,
        "mac": false,
        "linux": false
      },
      "linux_requirements": { "minimum": "..." },
      "pc_requirements": { "minimum": "..." },
      "release_date": { "coming_soon": false, "date": "1 Jan, 2024" }
    }
  }
}
```

**Use in onboarding:** Display game name and header image in the guided workflow when a Steam AppID is known. The `platforms.linux` field tells users if a native Linux build exists (relevant to trainer compatibility). `linux_requirements` can be surfaced in compatibility notes.

**Confidence:** High (well-documented, widely used)

---

### 2.2 UpToDateCheck Endpoint

```
GET https://api.steampowered.com/ISteamApps/UpToDateCheck/v1/?appid={appid}&version={version}
```

- **Authentication:** None (public)
- **Use:** Verify if the locally detected game version matches Steam's current version. Useful for warning users when a trainer may be outdated relative to the game.

**Confidence:** High

---

### 2.3 ISteamApps Endpoints (Publisher-Only, Not Applicable)

`GetAppBetas`, `GetAppBuilds`, `GetAppDepotVersions` — all require a publisher API key issued per-app.
These are not accessible to CrossHook. Skip these.

---

## 3. ProtonDB API _(Future feature reference — not required for v1 onboarding; adjacent to existing `CompatibilityPage`)_

### 3.1 Direct Summaries Endpoint (No Auth Required)

**Source:** Undocumented but widely used community pattern

```
GET https://www.protondb.com/api/v1/reports/summaries/{appid}.json
```

- **Authentication:** None
- **Rate limit:** Not documented; treat as courtesy access (~1 req/s)
- **Response format:** JSON

**Confirmed response structure** (verified against AppID 1172470, Apex Legends):

```json
{
  "bestReportedTier": "platinum",
  "confidence": "strong",
  "score": 0.55,
  "tier": "silver",
  "total": 1715,
  "trendingTier": "bronze"
}
```

**Tier values:** `borked` | `bronze` | `silver` | `gold` | `platinum` | `native`

**Use in onboarding:** Show the ProtonDB tier badge next to game name during the readiness check step. A `borked` or missing tier warns the user the game has poor Linux compatibility even before trainer complications arise.

**Confidence:** High (endpoint verified with live request; used by multiple open-source tools)

---

### 3.2 Community API (Self-Hosted, Not Recommended for Production)

[protondb.max-p.me](https://protondb.max-p.me/) — Community-run API, **deployment terminated April 2023** due to cost. Self-hosting required. Data refreshes every 31 days.

**Recommendation:** Use the direct ProtonDB endpoint (`/api/v1/reports/summaries/{appid}.json`) instead of the community API. It is current, reliable, and requires no setup.

---

## 4. Trainer File Validation

### 4.1 PE Magic Byte Check — No New Dependency Required

> **Security finding:** Do NOT add `goblin` or `pelite`. The only use case during onboarding is
> detecting that the user selected an `.exe` rather than a `.zip`, `.txt`, or other wrong file type.
> A full PE parser is unnecessary and expands the attack surface for untrusted binary input.

The `MZ` magic (`0x4D 0x5A`) present at offset 0 of every valid PE file is readable with two bytes
from `std::fs::File` — no crate required:

```rust
use std::fs::File;
use std::io::Read;

fn is_pe_file(path: &Path) -> bool {
    let mut buf = [0u8; 2];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut buf))
        .map(|_| buf == [0x4D, 0x5A])
        .unwrap_or(false)
}
```

**Use in onboarding:** Validate the user-selected trainer file before storing the path. Show an
inline error ("This file does not appear to be a Windows executable") if the magic check fails.

**What this does NOT provide:** Architecture (32 vs 64-bit) and version string extraction. If those
are needed in a future iteration, `goblin` (MIT, 100M+ fuzz runs) or `pelite` (zero-alloc, MIT)
are the candidates — but that decision should be deferred and reviewed for attack surface then.

**Confidence:** High (security team guidance; MZ magic is part of the PE spec)

---

## 5. Archive Handling _(Future feature reference — ZIP extraction is out of v1 scope)_

### 5.1 `zip` Crate

- **Crate:** [crates.io/crates/zip](https://crates.io/crates/zip)
- **Current repo:** [github.com/zip-rs/zip2](https://github.com/zip-rs/zip2) (zip2, same crate name)
- **Use:** Extract FLiNG `.zip` trainer archives to a staging directory before moving to SourceDirectory or Proton prefix

**crosshook-core already has:** `flate2` (for `.tar.gz`) and `tar` — but **no ZIP extractor**. The `zip` crate is a minimal, well-maintained addition.

```rust
use std::io::BufReader;
use zip::ZipArchive;

fn list_trainer_archive(path: &Path) -> anyhow::Result<Vec<String>> {
    let file = std::fs::File::open(path)?;
    let mut archive = ZipArchive::new(BufReader::new(file))?;
    let names = (0..archive.len())
        .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(names)
}
```

**Confidence:** High

---

## 6. HTTP Client for API Calls _(Future feature reference — no HTTP calls required for v1 onboarding)_

### 6.1 `reqwest` — Recommended

- **Crate:** [crates.io/crates/reqwest](https://crates.io/crates/reqwest)
- **Docs:** [docs.rs/reqwest](https://docs.rs/reqwest/latest/reqwest/)
- **Async:** Yes — integrates directly with the existing `tokio` runtime in crosshook-core
- **Features:** JSON deserialization, TLS, redirect handling, user-agent customization

**Pattern for Steam + ProtonDB lookups:**

```rust
// In crosshook-core, add to Cargo.toml:
// reqwest = { version = "0.12", features = ["json"] }

use reqwest::Client;
use serde::Deserialize;

#[derive(Deserialize)]
struct ProtonSummary {
    tier: String,
    #[serde(rename = "bestReportedTier")]
    best_reported_tier: String,
    confidence: String,
    score: f64,
    total: u32,
    #[serde(rename = "trendingTier")]
    trending_tier: String,
}

async fn fetch_protondb_tier(client: &Client, appid: u32) -> anyhow::Result<ProtonSummary> {
    let url = format!(
        "https://www.protondb.com/api/v1/reports/summaries/{appid}.json"
    );
    Ok(client.get(&url).send().await?.json().await?)
}

async fn fetch_steam_app_name(client: &Client, appid: u32) -> anyhow::Result<Option<String>> {
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={appid}"
    );
    let json: serde_json::Value = client.get(&url).send().await?.json().await?;
    Ok(json[appid.to_string()]["data"]["name"].as_str().map(String::from))
}
```

**Alternative — `ureq`:** Synchronous only, smaller binary, no async. Less suitable because crosshook-core uses async tokio throughout.

**Confidence:** High

---

## 7. V1 Dependency Summary

**Zero new Cargo dependencies required for v1 trainer onboarding.**

All readiness checks and onboarding state are implementable with existing crates:

| Existing dep           | Use in onboarding                                                                |
| ---------------------- | -------------------------------------------------------------------------------- |
| `sha2`                 | Optional trainer file fingerprinting (store hash in SQLite for update detection) |
| `rusqlite`             | `launch_operations` query via `MetadataStore` for "game launched once" check     |
| `serde` / `serde_json` | IPC types for new Tauri commands                                                 |
| `directories`          | Resolve `~/.local/share/Steam/steamapps/compatdata/` path                        |

Future crates to add when those features are scoped:

| Crate          | When to add                                                                     |
| -------------- | ------------------------------------------------------------------------------- |
| `reqwest 0.12` | When Steam Store or ProtonDB API enrichment is added as a feature               |
| `zip 2.x`      | When auto-extraction of FLiNG trainer archives becomes a scoped feature         |
| `goblin 0.9`   | When PE arch/version detection becomes a scoped feature (after security review) |
| `pelite 0.10`  | Same as `goblin` — deferred                                                     |

---

## 8. Trainer-to-Game Version Matching _(Future feature reference)_

**Pattern used by the community:**

1. **PE FileVersion comparison** — Extract `FileVersion` from trainer `.exe` and game `.exe` using `pelite`. Compare strings for mismatch warning.
2. **SHA-256 fingerprinting** — crosshook-core already has `sha2`. Computing a SHA-256 of the game `.exe` provides a stable fingerprint that can be stored in SQLite and re-checked to detect game updates (which would invalidate a previously-working trainer).
3. **Filename version embedding** — FLiNG zip filenames encode the game version (e.g., `v1.1`). Parsing this at import time gives a quick compatibility hint without opening the archive.

**Confidence:** High (patterns derived from CheatHappens troubleshooting guide, community forums, PE docs)

---

## Integration Patterns

### Proton/Steam Readiness Detection (Local Filesystem)

> **Practices finding:** Much of this is already implemented. See `docs/plans/trainer-onboarding/research-practices.md`.

Existing CrossHook has the following already in place for readiness checks:

| Check                         | Path/Method                                                   | Already in CrossHook?                                      |
| ----------------------------- | ------------------------------------------------------------- | ---------------------------------------------------------- |
| Steam installed               | `~/.steam/steam` or `~/.local/share/Steam`                    | **Yes** — `steam/discovery.rs`                             |
| Proton available              | `steamapps/common/Proton *` or `compatibilitytools.d/`        | **Yes** — `steam/proton.rs`                                |
| Auto-populate (full pipeline) | `steam_auto_populate` Tauri command → `attempt_auto_populate` | **Yes** — `steam/auto_populate.rs`                         |
| Game run once                 | Query `launch_operations` table in `MetadataStore`            | **Yes** — `metadata/launch_history.rs` via `MetadataStore` |
| Trainer file picker           | `chooseFile` in `src/utils/dialog.ts`                         | **Yes** — used by `InstallField`, `ProtonPathField`        |
| Trainer file present          | User-provided path stat                                       | Minor new logic in existing `install/` flow                |
| Onboarding state persistence  | Boolean flag in `settings.toml`                               | `settings/mod.rs` — add field there                        |

**Game "run once" detection (revised):** Query `launch_operations` from `MetadataStore` rather than
filesystem-checking `compatdata/{appid}/pfx/`. `MetadataStore::is_available()` provides the
graceful-degradation guard if the DB is not yet initialized.

**Confidence:** High (practices team cross-reference; confirmed against existing source files)

---

## 10. Existing Linux Trainer Tool Ecosystem

| Tool                                                                                 | Language    | License  | Status         | Notes                                     |
| ------------------------------------------------------------------------------------ | ----------- | -------- | -------------- | ----------------------------------------- |
| [wemod-launcher](https://github.com/DeckCheatz/wemod-launcher)                       | Python      | AGPL-3.0 | Active 2024–25 | WeMod on Linux via Proton prefix          |
| [FLiNG-Trainer-Collection](https://github.com/Melon-Studio/FLiNG-Trainer-Collection) | C# / Python | Unknown  | Active         | Windows WPF app; crawler for FLiNG site   |
| DeckCheatz ecosystem                                                                 | Python      | AGPL-3.0 | Active         | Downloadable Proton prefixes for trainers |

None of these are Rust libraries CrossHook can link against. They inform integration patterns only.

---

## 11. Constraints and Gotchas

1. **No trainer site has a public API.** Any "trainer catalog" feature must be user-assisted (link
   to download page) or rely on a local filesystem scan of already-downloaded files.

2. **ProtonDB's direct endpoint is undocumented.** It works reliably today but could be removed
   without notice. Cache results in SQLite (which CrossHook already has) to reduce dependency on live access.

3. **Steam Store API rate limit is unspecified officially.** The community consensus is ~200 req/5 min.
   For a local app used by one user, this is never a concern in practice.

4. **PE parsing requires reading bytes into memory.** For large trainer files, stream a small header
   read (first 4KB covers DOS + PE header) rather than loading the entire `.exe`.

5. **Antivirus false positives.** All trainer `.exe` files are flagged by Windows Defender and many AV
   tools. On Linux this is less relevant, but CrossHook should document this clearly in onboarding
   guidance to avoid user confusion.

6. **WeMod is subscription-gated.** Free tier has limitations. CrossHook should not present WeMod as
   a free resource equivalent to FLiNG.

7. **FLiNG zip names contain version info, not always game AppID.** Matching trainer-to-game requires
   fuzzy name matching or manual user association — there is no canonical game-ID field in FLiNG zips.

8. **Full PE parsing crates (`goblin`, `pelite`) are deferred.** The 2-byte `MZ` check covers the
   onboarding validation use case. Richer parsing (arch detection, version strings) may be added in
   a later iteration after dedicated security review of untrusted-binary parsing scope.

---

## 12. Open Questions

1. **Should CrossHook maintain a local trainer catalog/index?** FLiNG has ~3,000+ trainers. An
   indexed local DB seeded from a crawl would enable search-by-game-name, but requires periodic
   refresh and raises legal/ToS concerns about automated scraping.

2. **WeMod detection:** Should CrossHook detect wemod-launcher installation and offer to configure
   it as a launch wrapper, or just document it externally?

3. **Version mismatch severity:** When trainer FileVersion ≠ game FileVersion, should CrossHook
   block launch, show a warning, or just log? Business-level decision needed.

4. **ProtonDB tier threshold:** Is `borked` the only tier that warrants a hard warning, or should
   `bronze` also prompt a caution?

5. **Offline operation:** Should Steam Store and ProtonDB lookups be optional (app works without
   network)? Given CrossHook is a local app, yes — API calls should be best-effort with graceful
   degradation.

---

## 13. Sources

| Resource                           | URL                                                                                                                |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| FLiNG Trainer website              | [flingtrainer.com](https://flingtrainer.com/)                                                                      |
| FLiNG Archive                      | [flingtrainer.com/uncategorized/my-trainers-archive/](https://flingtrainer.com/uncategorized/my-trainers-archive/) |
| FLiNG Trainer Collection (scraper) | [github.com/Melon-Studio/FLiNG-Trainer-Collection](https://github.com/Melon-Studio/FLiNG-Trainer-Collection)       |
| WeMod                              | [wemod.com](https://www.wemod.com/)                                                                                |
| wemod-launcher (Linux)             | [github.com/DeckCheatz/wemod-launcher](https://github.com/DeckCheatz/wemod-launcher)                               |
| WeMod Linux dev post               | [community.wemod.com](https://community.wemod.com/t/wemod-linux-steam-deck-launcher-is-in-development/262286)      |
| MrAntiFun                          | [mrantifun.net](https://mrantifun.net/)                                                                            |
| CheatHappens                       | [cheathappens.com](https://www.cheathappens.com/)                                                                  |
| Steam Web API Overview             | [partner.steamgames.com/doc/webapi_overview](https://partner.steamgames.com/doc/webapi_overview)                   |
| Steam ISteamApps                   | [partner.steamgames.com/doc/webapi/ISteamApps](https://partner.steamgames.com/doc/webapi/ISteamApps)               |
| Steam xpaw docs                    | [steamapi.xpaw.me](https://steamapi.xpaw.me/)                                                                      |
| ProtonDB                           | [protondb.com](https://www.protondb.com/)                                                                          |
| ProtonDB Community API             | [protondb.max-p.me](https://protondb.max-p.me/)                                                                    |
| ProtonDB Community API (GitHub)    | [github.com/Trsnaqe/protondb-community-api](https://github.com/Trsnaqe/protondb-community-api)                     |
| goblin crate                       | [docs.rs/goblin](https://docs.rs/goblin/latest/goblin/)                                                            |
| pelite crate                       | [docs.rs/pelite](https://docs.rs/pelite/latest/pelite/)                                                            |
| pelite VersionInfo                 | [docs.rs/pelite/.../version_info](https://docs.rs/pelite/0.9.0/pelite/resources/version_info/)                     |
| zip crate                          | [crates.io/crates/zip](https://crates.io/crates/zip)                                                               |
| reqwest crate                      | [docs.rs/reqwest](https://docs.rs/reqwest/latest/reqwest/)                                                         |
| ureq crate                         | [github.com/algesten/ureq](https://github.com/algesten/ureq)                                                       |
| Proton FAQ                         | [github.com/ValveSoftware/Proton/wiki/Proton-FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)         |
| ArchWiki Steam                     | [wiki.archlinux.org/title/Steam](https://wiki.archlinux.org/title/Steam)                                           |
| CheatHappens troubleshooting       | [cheathappens.com/trainer_troubleshooting.asp](https://www.cheathappens.com/trainer_troubleshooting.asp)           |
| ProtonDB CLI (reference)           | [github.com/hypeedev/protondb-cli](https://github.com/hypeedev/protondb-cli)                                       |
