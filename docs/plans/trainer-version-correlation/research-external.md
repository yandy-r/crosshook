# External Research: Trainer-Version-Correlation

**Feature:** Trainer and game version correlation with mismatch detection
**Researcher:** research-specialist
**Date:** 2026-03-29
**Scope:** Local desktop app (Rust/Tauri) — filesystem APIs and Rust crates only

---

## Executive Summary

This feature can be built entirely on existing CrossHook infrastructure with minimal new dependencies. The Steam ACF manifest format is already parsed by `steam/vdf.rs`; extending `steam/manifest.rs` to also extract `buildid` and `LastUpdated` is a low-effort, zero-dependency change. Trainer version information has no structured machine-readable metadata format — it lives as human-written strings in filenames and PE version resources — so practical version extraction means: (1) reading the trainer filename, (2) optionally parsing the PE VERSIONINFO resource using `pelite`, or (3) prompting the user to enter it. The `notify-debouncer-full` crate provides solid filesystem watching for change detection. No external web services, databases, or cloud APIs are needed or applicable.

**Confidence:** High — findings are based on direct inspection of CrossHook source code and authoritative crate documentation.

---

## Primary APIs

This feature is purely local — no web services, authentication, or network calls are required or applicable. The "APIs" are local filesystem formats and Rust library interfaces:

| API / Interface                          | Type                                             | Status                                                                                           |
| ---------------------------------------- | ------------------------------------------------ | ------------------------------------------------------------------------------------------------ |
| Steam ACF manifest (`appmanifest_*.acf`) | Local filesystem — VDF/KeyValues format          | Already parsed by `steam/vdf.rs` + `steam/manifest.rs`; needs `buildid`/`LastUpdated` extraction |
| Trainer PE VERSIONINFO resource          | Local filesystem — Windows PE binary format      | Optional; `pelite` crate; defer to v2                                                            |
| Trainer filename version string          | Local filesystem — regex extraction              | Zero cost fallback                                                                               |
| `notify-debouncer-full` crate events     | Rust library — filesystem watch events           | Optional v2; poll on app-open suffices for v1                                                    |
| `semver` crate                           | Rust library — version string parsing/comparison | Optional; defer until range queries needed                                                       |
| CrossHook SQLite metadata DB             | Local — rusqlite via existing `metadata/` layer  | New `version_store.rs` module; migration #9                                                      |

Sections 1–5 below cover each in depth.

---

## 1. Steam Manifest Format (ACF/VDF)

### Format Overview

Steam app manifests use Valve's **KeyValues** format (also called VDF — Valve Data Format). Files are named `appmanifest_<appid>.acf` and live under `<steamlibrary>/steamapps/`. The root key is `AppState`.

**Example manifest snippet:**

```
"AppState"
{
    "appid"         "1245620"
    "Universe"      "1"
    "name"          "Elden Ring"
    "installdir"    "ELDEN RING"
    "buildid"       "14532001"
    "LastUpdated"   "1709856234"
    "StateFlags"    "4"
    "SizeOnDisk"    "52428800000"
    "BytesToDownload"   "0"
    "BytesDownloaded"   "0"
    "AutoUpdateBehavior"    "0"
    "MountedDepots"
    {
        "1245621"   "7654321098765432"
        "1245622"   "9876543210987654"
    }
}
```

### Key Fields for Version Tracking

| Field           | Type                    | Meaning                                                                                                                                                                  |
| --------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `buildid`       | String (integer)        | Steam build identifier — increments with every game update pushed to Steam. This is the canonical "game version" from a compatibility perspective.                       |
| `LastUpdated`   | String (Unix timestamp) | POSIX timestamp of the last successful update. Use to detect update events: if the current `LastUpdated` differs from what was last recorded, the game has been updated. |
| `StateFlags`    | Bitfield                | `4` = fully installed; `1026` = update required/in-progress. Watching for StateFlags to transition back to `4` after being `1026` can confirm a completed update.        |
| `name`          | String                  | Human-readable game name.                                                                                                                                                |
| `appid`         | String                  | Steam AppID, links to profiles via `steam.app_id`.                                                                                                                       |
| `MountedDepots` | Object                  | Depot ID → manifest ID map. Depot manifest IDs also increment with updates and provide fine-grained version tracking.                                                    |

### buildid Semantics

- `buildid` is an opaque monotonically-increasing integer assigned by Valve's build system (SteamPipe).
- It is **not** a semantic version — no major/minor/patch structure.
- Two installations of the same game on different machines will have the same `buildid` if they are on the same Steam branch/beta.
- Comparing stored `buildid` to current `buildid` is sufficient for mismatch detection.
- `buildid` is available publicly via SteamDB for any public game.

### Update Detection Pattern

The simplest reliable pattern:

1. When a profile is saved, read and store `buildid` + `LastUpdated` from the manifest.
2. At launch time (or on app open), re-read the manifest and compare `buildid` to stored value.
3. If `buildid` differs → game has been updated → check trainer compatibility.

For live detection, watch the ACF file with `notify` (see §5).

**Confidence:** High — based on direct reading of CrossHook's `vdf.rs`/`manifest.rs` and community documentation at [steamfiles docs](https://github.com/leovp/steamfiles/blob/master/docs/acf_overview.rst).

### What CrossHook's manifest.rs Already Does

`steam/manifest.rs` currently reads only `appid` and `installdir`. It uses the custom `parse_vdf()` from `steam/vdf.rs`, which fully supports nested key access via `get_child()` and `find_descendant()`. Adding `buildid` and `LastUpdated` extraction requires only two additional `get_child("buildid")` / `get_child("lastupdated")` calls inside the existing `parse_manifest()` function.

The `BTreeMap`-backed `VdfNode` and case-insensitive key lookup (`normalize_key` lowercases all keys) means `buildid` and `LastUpdated` are accessible immediately.

---

## 2. Trainer Version Sources

### The Core Problem

Trainers (FLiNG, WeMod, MrAntiFun, etc.) have **no standardized machine-readable version metadata format**. Version information is spread across:

- Filename conventions (e.g., `EldenRing-v1.12.3-FLiNG.exe`)
- PE VERSIONINFO resource (embedded in the `.exe`)
- The trainer distributor's website (not accessible offline)

### FLiNG Trainer Version Patterns

FLiNG trainers follow loose conventions:

- Website page lists: `Game Version: v1.0-v1.5.0+` and `Last Updated: 2024.03.15`
- Filenames: `EldenRingTrainer-14Options-FLiNG-v1.12.3.exe` (human-generated, inconsistent)
- No JSON/XML manifest file is distributed alongside the trainer executable
- The trainer `.exe` contains a Windows PE VERSIONINFO resource with fields like `FileVersion`, `ProductVersion`, `ProductName`, `FileDescription`

**Practical extraction approach for FLiNG:**

1. **Filename parsing** — extract a version string from the filename using a regex (e.g., `v\d+[\.\d]*`) — low reliability but zero cost.
2. **PE VERSIONINFO** — read the embedded Windows version resource from the `.exe` — reliable but requires a PE parsing crate.
3. **User entry** — ask the user to type or confirm the trainer version when saving a profile — most reliable for display purposes.

### WeMod Version Approach

WeMod's **Version Guard** system (described at [this Medium post](https://medium.com/wemod/version-guard-781d5e152a13)):

- Creates snapshots of game state per update, storing compressed deltas
- Pins trainers to specific game versions
- This is a server-side managed approach (WeMod syncs trainer-to-version mappings centrally)
- **Not replicable locally** without WeMod's infrastructure

WeMod trainer files themselves have PE VERSIONINFO resources. CrossHook can observe the trainer `.exe`'s filesystem `mtime` (last modification time) as a proxy for "trainer version changed."

### PE VERSIONINFO Resource Extraction (Rust)

The `pelite` crate ([docs.rs/pelite](https://docs.rs/pelite)) is the recommended approach:

- Pure Rust, no `unsafe`, cross-platform (can read Windows PE from Linux)
- Memory-safe, zero-allocation design
- Provides `pe.resources()?.version_info()` API
- Returns `FileVersion` (4-part numeric: `1.2.3.4`) and `ProductVersion` (string)

The `goblin` crate ([github.com/m4b/goblin](https://github.com/m4b/goblin)) parses PE structure but stops before parsing the version resource table — not sufficient for this use case.

**Recommended approach:** `pelite` for PE version resource extraction, with filename regex as a fallback.

**Confidence:** Medium — PE VERSIONINFO existence in trainer EXEs is typical but not guaranteed for all trainer sources.

---

## 3. Version Comparison Libraries (Rust Crates)

### Steam buildid Comparison

Steam `buildid` is an integer stored as a string. No library needed:

```rust
// Parse both as u64 and compare
let stored_build: u64 = stored_buildid.parse().unwrap_or(0);
let current_build: u64 = current_buildid.parse().unwrap_or(0);
let mismatch = current_build != stored_build;
let updated = current_build > stored_build;
```

### Semantic Version Strings — `semver` crate

- **Crate:** [semver v1.0.27](https://crates.io/crates/semver) (MIT/Apache-2.0)
- **Use case:** If trainer or game version strings follow SemVer (e.g., `1.2.3`, `1.0.0-beta.1`)
- **Key API:**
  - `Version::parse("1.2.3")` → parsed version
  - `VersionReq::parse(">=1.2, <2.0")` → range matching
  - `VersionReq::matches(&version)` → boolean compatibility check
- **Limitation:** SemVer is rare in game version strings. Most game versions are `1.12.3` (no pre-release/build metadata) — these parse fine. However `v1.12.3+` (FLiNG notation for "and above") does not parse as SemVer.

```rust
use semver::{Version, VersionReq};

let game_ver = Version::parse("1.12.3").unwrap();
let compat_req = VersionReq::parse(">=1.0, <=1.12.3").unwrap();
assert!(compat_req.matches(&game_ver));
```

### Arbitrary Version Strings — `versions` crate

- **Crate:** [versions](https://crates.io/crates/versions) (MIT)
- **Use case:** Parses non-SemVer version strings common in the gaming world (e.g., `1.0.0.1`, `v2024.3`, `1.12.3.456`, `2024.03.15`)
- **Handles:** Epoch versions, alphanumeric components, complex ordering
- **Limitation:** Less commonly used, smaller community than `semver`

### Recommendation

For CrossHook's use case:

1. **buildid mismatch:** Integer comparison — no library needed.
2. **Trainer version tracking:** Store as opaque string; compare with `==` for exact match or use `semver` if versions are well-formed.
3. **Version range compatibility:** `semver::VersionReq` if game developers publish SemVer; otherwise store confirmed-compatible `buildid` ranges as `(min_buildid, max_buildid)` pairs.

**Confidence:** High — based on authoritative crate documentation at [docs.rs/semver](https://docs.rs/semver/latest/semver/).

---

## 4. Community Sharing Patterns

### ProtonDB

- **Data format:** JSON reports per game (keyed by Steam AppID)
- **Fields per report:** `rating` (Borked/Bronze/Silver/Gold/Platinum), `protonVersion`, `os`, `gameSettings`, `timestamp`
- **Version tracking:** ProtonDB reports include `gameSettings` free text but **no structured game version / buildid field**
- **Offline use:** Raw data dumps are downloadable but are not relevant for trainer-version correlation
- **Community API:** [github.com/Trsnaqe/protondb-community-api](https://github.com/Trsnaqe/protondb-community-api) — a web service, not usable offline

**Relevance to CrossHook:** ProtonDB's rating model (Borked/Bronze/Silver/Gold/Platinum) directly maps to CrossHook's existing `CompatibilityRating` enum. The display pattern (badge per game with tier and user count) is the established UX for Linux game compatibility.

### PCGamingWiki

- **Data format:** MediaWiki + Cargo extension, queryable via REST API
- **Fields:** Extensive — DRM type, save locations, API support, OS versions
- **Version tracking:** Tracks engine versions and release dates, **not trainer compatibility**
- **Relevance:** Source of truth for `buildid`-to-human-version mappings (e.g., "Steam buildid 14532001 = Elden Ring v1.12.3 SOTE")
- **Access:** `https://www.pcgamingwiki.com/w/api.php?action=cargoquery&...` — requires internet, not suitable for offline feature

### CrossHook Community Taps (Existing)

The most relevant "community sharing" mechanism is CrossHook's own tap system. The `CommunityProfileMetadata` struct already has:

- `game_version: String` — currently display-only
- `trainer_version: String` — currently display-only
- `compatibility_rating: CompatibilityRating` — ProtonDB-style rating

The human-authored `game_version` string (e.g., `"1.15.2"`) is the right field to share in community profiles — it is meaningful to other users. The `buildid` integer is a local machine artifact (Steam's internal monotonic integer) that has no portable meaning outside the installing machine's Steam client. **Do not add `game_buildid` to `CommunityProfileMetadata`.**

`game_build_id` belongs in the local version correlation record (e.g., a new `metadata/version_store.rs` module) alongside the profile ID, snapshot timestamp, and trainer version snapshot. That record is runtime state, not portable profile data.

**Confidence:** High — based on direct reading of `community_schema.rs` and `migrations.rs`.

---

## 5. File Monitoring — `notify` and Debouncer Crates

### `notify` crate

- **Version:** 8.2.0 ([docs.rs/notify](https://docs.rs/notify/latest/notify/), [crates.io](https://crates.io/crates/notify))
- **License:** MIT/Apache-2.0
- **Maintenance:** Actively maintained; used by cargo-watch, rust-analyzer, Deno
- **Linux backend:** inotify (kernel-level, efficient)
- **API:**

```rust
use notify::{recommended_watcher, RecursiveMode, Watcher, Event};
use std::sync::mpsc;

let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
let mut watcher = recommended_watcher(tx)?;
watcher.watch(&steamapps_path, RecursiveMode::NonRecursive)?;

for event in rx {
    if let Ok(ev) = event {
        // ev.paths contains affected file paths
        // ev.kind is EventKind::Modify(ModifyKind::Data(_)) for content changes
    }
}
```

### `notify-debouncer-full` crate

- **Version:** 0.7.0 ([docs.rs](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/))
- **Purpose:** Wraps `notify` to collapse rapid successive events (e.g., Steam writing multiple updates to the ACF file)
- **Key feature over `notify-debouncer-mini`:** Tracks filesystem IDs and stitches rename events together — important because Steam may write to a temp file then rename to the ACF filename
- **API:**

```rust
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::time::Duration;

let mut debouncer = new_debouncer(
    Duration::from_secs(2),  // 2s quiet period before emitting
    None,                     // use default file ID cache
    |result: DebounceEventResult| {
        match result {
            Ok(events) => {
                for event in events {
                    // event.paths, event.kind
                }
            }
            Err(errors) => { /* handle watch errors */ }
        }
    }
)?;

debouncer.watcher().watch(&steamapps_path, RecursiveMode::NonRecursive)?;
```

### Linux inotify Limitations to Note

- Not 100% reliable for very large directories (resource limits on watched inotify instances)
- NFS and some virtual filesystems do not emit events
- `/proc` and `/sys` do not emit change events
- `PollWatcher` is the fallback if inotify fails (slight performance cost)

### When to Watch vs. When to Poll

For CrossHook's use case, a **hybrid approach** is recommended:

1. **On app open and on profile load:** Always re-read the ACF manifest (synchronous poll). This catches any updates that occurred while the app was closed.
2. **While app is open (optional):** Use `notify-debouncer-full` to watch the `steamapps/` directory for changes to ACF files. On change, re-read the relevant manifest and check for buildid change.

Given that game updates are infrequent and users primarily interact via the Launch page, polling on app open is sufficient for v1. Filesystem watching is a v2 enhancement.

**Confidence:** High — based on [docs.rs/notify](https://docs.rs/notify/latest/notify/) and [docs.rs/notify-debouncer-full](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/).

---

## 6. Code Examples

### Example A: Extend manifest.rs to Extract buildid and LastUpdated

```rust
// In steam/manifest.rs — extend parse_manifest() return type
#[derive(Debug, Clone)]
pub struct ManifestInfo {
    pub app_id: String,
    pub install_dir: String,
    pub build_id: Option<String>,      // "14532001"
    pub last_updated: Option<u64>,     // Unix timestamp
}

fn parse_manifest(manifest_path: &Path) -> Result<ManifestInfo, String> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|e| format!("unable to read manifest: {e}"))?;
    let root = parse_vdf(&content).map_err(|e| e.to_string())?;
    let app_state = root.get_child("AppState").unwrap_or(&root);

    let app_id = app_state
        .get_child("appid")
        .and_then(|n| n.value.as_ref())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| extract_app_id_from_manifest_path(manifest_path))
        .unwrap_or_default();

    let install_dir = app_state
        .get_child("installdir")
        .and_then(|n| n.value.as_ref())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_default();

    let build_id = app_state
        .get_child("buildid")
        .and_then(|n| n.value.as_ref())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    let last_updated = app_state
        .get_child("lastupdated")  // VdfNode normalizes to lowercase
        .and_then(|n| n.value.as_ref())
        .and_then(|v| v.trim().parse::<u64>().ok());

    Ok(ManifestInfo { app_id, install_dir, build_id, last_updated })
}
```

### Example B: buildid Mismatch Detection

```rust
// Stored in metadata SQLite or profile TOML
struct VersionSnapshot {
    build_id: String,        // e.g., "14532001"
    recorded_at: i64,        // Unix timestamp when snapshot was taken
    trainer_version: String, // opaque string from filename or PE resource
}

fn check_for_mismatch(
    snapshot: &VersionSnapshot,
    current_manifest: &ManifestInfo,
) -> Option<MismatchWarning> {
    let current = current_manifest.build_id.as_deref().unwrap_or("");
    if !current.is_empty() && current != snapshot.build_id {
        Some(MismatchWarning {
            stored_build_id: snapshot.build_id.clone(),
            current_build_id: current.to_string(),
            trainer_version: snapshot.trainer_version.clone(),
        })
    } else {
        None
    }
}
```

### Example C: Filesystem Watcher for steamapps Directory

```rust
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use notify::RecursiveMode;
use std::path::PathBuf;
use std::time::Duration;

fn watch_steamapps(steamapps_path: PathBuf, on_manifest_change: impl Fn(PathBuf) + Send + 'static) {
    let debouncer = new_debouncer(
        Duration::from_secs(3),
        None,
        move |result: DebounceEventResult| {
            if let Ok(events) = result {
                for event in events {
                    for path in &event.paths {
                        let is_acf = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|n| n.starts_with("appmanifest_") && n.ends_with(".acf"))
                            .unwrap_or(false);
                        if is_acf {
                            on_manifest_change(path.clone());
                        }
                    }
                }
            }
        },
    );

    if let Ok(mut d) = debouncer {
        let _ = d.watcher().watch(&steamapps_path, RecursiveMode::NonRecursive);
        // Keep debouncer alive — store it in app state
    }
}
```

### Example D: PE Version Resource Extraction (pelite)

```rust
// Requires: pelite = "0.10" in Cargo.toml
// Note: pelite is NOT currently in crosshook-core's Cargo.toml
use pelite::FileMap;
use pelite::pe64::Pe; // or pe32 for 32-bit executables

fn read_trainer_file_version(exe_path: &std::path::Path) -> Option<String> {
    let map = FileMap::open(exe_path).ok()?;
    let pe = pelite::PeFile::from_bytes(map.as_ref()).ok()?;
    let resources = pe.resources().ok()?;
    let version_info = resources.version_info().ok()?;

    // FileVersion is 4-part numeric: major.minor.patch.build
    let fv = version_info.fixed()?.dwFileVersion;
    Some(format!("{}.{}.{}.{}",
        fv.Major, fv.Minor, fv.Patch, fv.Build))
}
```

---

## Integration Patterns

How to wire version correlation into the existing CrossHook codebase:

### Pattern 1: Extend `steam/manifest.rs` (zero new dependencies)

Add `build_id: Option<String>` and `last_updated: Option<u64>` to the return type of `parse_manifest()`. The existing `VdfNode::get_child()` already handles case-insensitive lookup — `get_child("buildid")` and `get_child("lastupdated")` are the only additions needed. See Example A in §6.

### Pattern 2: New `metadata/version_store.rs` module (migration #9)

Follow the exact pattern of existing metadata modules (`launcher_sync.rs`, `health_store.rs`). Add a `version_snapshots` table to the SQLite database via migration #9 in `migrations.rs`:

```sql
CREATE TABLE version_snapshots (
    profile_id          TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
    game_build_id       TEXT,          -- Steam buildid at snapshot time
    game_last_updated   INTEGER,       -- LastUpdated Unix timestamp
    trainer_mtime       INTEGER,       -- trainer exe mtime at snapshot time (optional)
    trainer_version     TEXT,          -- opaque string from filename/PE/user entry
    manifest_path       TEXT,          -- which .acf file to re-read
    snapshot_taken_at   TEXT NOT NULL
);
```

`game_build_id` is **local runtime state** — not stored in `CommunityProfileMetadata` (see §4).

### Pattern 3: Mismatch check at launch time

In the `launch/` module (or as a pre-launch hook in `src-tauri/commands/launch.rs`), before executing the launch script:

1. Load the stored `version_snapshot` for the profile from SQLite.
2. Re-read the ACF manifest at `manifest_path`.
3. Compare `current_build_id != snapshot.game_build_id`.
4. If mismatch: emit a warning event to the frontend via Tauri event or return a structured pre-launch warning in the command response.
5. If no mismatch (or after user confirms): proceed with launch and update `snapshot_taken_at`.

### Pattern 4: Snapshot capture on profile save

When a profile is saved (`profile/toml_store.rs`), if `steam.app_id` is set, attempt to read the corresponding ACF manifest and write/update the version snapshot. This is best-effort — if the manifest is not found (non-Steam game, library not mounted), skip silently.

### Pattern 5: Community tap version annotation (v2)

When a user marks a launch as "working" (post-launch feedback), record the `(game_version_string, trainer_version_string)` pair in the profile's `CommunityProfileMetadata`. This is the human-readable equivalent of the `buildid` snapshot and is shareable via community taps. The `game_version` field already exists in `CommunityProfileMetadata` — no schema change to community profiles needed.

---

## 7. Open Questions

1. **Trainer version source priority:** Should the system prefer (a) PE VERSIONINFO FileVersion, (b) filename-parsed version, or (c) user-entered version? Each has different reliability. A ranked fallback strategy would work: PE resource → filename regex → user prompt.

2. **buildid-to-human-version mapping:** Steam `buildid` integers are opaque. Mapping `buildid` to a human-readable version like "v1.12.3 SOTE" requires either: (a) a PCGamingWiki lookup (online), (b) user annotation, or (c) reading the game's own version from its executable (requires game-specific parsing). Storing only `buildid` for mismatch detection is simpler and sufficient.

3. **Multi-library steamapps:** A game could be installed under multiple Steam libraries. The existing `manifest.rs` handles this by iterating all libraries. The version tracking should record the `manifest_path` so the correct manifest is re-read on next check.

4. **Trainer exe without PE resources:** Some trainers are packed/protected (e.g., by Themida, VMProtect) and the version resource is stripped or encrypted. `pelite` would fail silently — this case should fall through to filename parsing or user entry.

5. **Non-Steam games:** Profiles with `native` or `proton_run` launch methods without a `steam.app_id` have no ACF manifest to read. Version tracking for these would rely entirely on trainer version string + game executable `mtime`.

6. **Schema migration:** Adding `game_build_id` and `trainer_version_at_snapshot` to the `profiles` table (or a new `version_snapshots` table) would be schema migration #9. The existing migration pattern in `migrations.rs` handles this straightforwardly.

---

## Sources

- [Steam ACF Format Overview — steamfiles](https://github.com/leovp/steamfiles/blob/master/docs/acf_overview.rst)
- [notify crate docs — docs.rs](https://docs.rs/notify/latest/notify/)
- [notify-debouncer-full docs — docs.rs](https://docs.rs/notify-debouncer-full/latest/notify_debouncer_full/)
- [semver crate docs — docs.rs](https://docs.rs/semver/latest/semver/)
- [versions crate — crates.io](https://crates.io/crates/versions)
- [pelite crate — docs.rs](https://docs.rs/pelite)
- [goblin crate — github.com/m4b/goblin](https://github.com/m4b/goblin)
- [valve_kv_tools — crates.io](https://crates.io/crates/valve_kv_tools)
- [vdf-reader — crates.io](https://crates.io/crates/vdf-reader)
- [acf-parser — crates.io](https://crates.io/crates/acf-parser)
- [WeMod Version Guard — Medium](https://medium.com/wemod/version-guard-781d5e152a13)
- [ProtonDB Community API — github.com/Trsnaqe](https://github.com/Trsnaqe/protondb-community-api)
- [PCGamingWiki API — pcgamingwiki.com](https://www.pcgamingwiki.com/wiki/PCGamingWiki:API)
- [FLiNG Trainer — flingtrainer.com](https://flingtrainer.com/)
- [Valve KeyValues format — developer.valvesoftware.com](https://developer.valvesoftware.com/wiki/KeyValues)
- CrossHook source: `steam/manifest.rs`, `steam/vdf.rs`, `profile/community_schema.rs`, `metadata/migrations.rs`

---

## Search Queries Executed

1. `Steam ACF VDF appmanifest format buildid LastUpdated fields version tracking`
2. `Rust notify crate filesystem watching inotify Steam manifest change detection`
3. `FLiNG trainer version metadata format game version compatibility information`
4. `Rust semver crate version comparison build ID semantic versioning crates.io`
5. `ProtonDB game compatibility data format API version tracking community database`
6. `Steam appmanifest ACF StateFlags buildid update detection Rust parsing example code`
7. `WeMod trainer metadata version API game compatibility format JSON`
8. `PCGamingWiki game version compatibility data format API trainer compatibility tracking`
9. `trainer file metadata version string extraction PE file header version resource Windows executable`
10. `Rust goblin exe-rs pefile PE Windows executable version resource parsing crate`

---

## Uncertainties & Gaps

- **pelite maintenance status:** Could not verify current release date or download stats from crates.io (JS-gated). Recommend verifying `pelite` is actively maintained before adding it as a dependency.
- **notify v8 API stability:** The `notify` crate has had breaking changes in the past (v4→v5→v6). The v8.2.0 API documented here should be verified against the current `Cargo.lock` if `notify` is already transitively present.
- **FLiNG PE resource consistency:** Not empirically verified that all FLiNG trainers contain VERSIONINFO resources — this is an assumption based on PE toolchain conventions.
- **WeMod trainer format on disk:** WeMod's trainer infrastructure on Linux (via Wine/WINE bottles) may differ from Windows; their actual stored file format was not accessible for direct inspection.
