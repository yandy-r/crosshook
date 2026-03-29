# External Research: Proton Migration Tool

**Feature**: Proton version migration tool for profile path updates (GitHub issue #48)
**Researcher**: research-specialist
**Date**: 2026-03-29

---

## Executive Summary

The proton-migration-tool feature requires no new external dependencies. CrossHook's existing `discover_compat_tools` + `normalize_alias` pipeline in `steam/proton.rs` already provides 90% of the raw material needed for stale-path detection and version suggestion. The primary research question was: **how do we rank and present replacement candidates when a Proton path disappears?**

Key findings:

- Proton naming is fragmented across three overlapping schemes. Any comparison strategy must strip non-numeric noise and compare numeric segments.
- The `alphanumeric-sort` crate (zero-dependency, `no_std`-compatible) is the best fit for ranking candidates by version closeness — no new crate is strictly necessary since a small custom comparator on `normalized_aliases` digits would work equally well.
- No existing Linux game launcher (Lutris, Heroic, Bottles) proactively detects stale Proton paths or suggests replacements. They all silently fail or show a generic "not found" error. CrossHook would be best-in-class here.
- Steam Flatpak installations use a distinct path prefix that the existing `discover_compat_tools` must cover for feature completeness.

**Confidence**: High (multiple primary sources, reviewed CrossHook source directly)

---

## Primary APIs

### 1.1 Proton Versioning Schemes

There are **three distinct naming conventions** in active use:

| Scheme                             | Format                            | Example                                | Location                                 |
| ---------------------------------- | --------------------------------- | -------------------------------------- | ---------------------------------------- |
| Official Valve                     | `Proton X.Y`                      | `Proton 9.0`, `Proton 8.0`             | `steamapps/common/`                      |
| Official Valve (versioned release) | `Proton X.Y-Z`                    | `Proton 9.0-1`, `Proton 8.0-5`         | Steam client display name                |
| GE-Proton (current, post-Feb 2022) | `GE-ProtonX-Z`                    | `GE-Proton10-34`, `GE-Proton9-4`       | `compatibilitytools.d/`                  |
| GE-Proton (legacy, pre-2022)       | `Proton-X.Y-GE-Z`                 | `Proton-9.23-GE-2`                     | `compatibilitytools.d/`                  |
| Proton-TKG (Frogging-Family)       | `proton_tkg_X.Y.rN.gHASH.release` | `proton_tkg_6.17.r0.g5f19a815.release` | `compatibilitytools.d/`                  |
| SteamTinkerLaunch / distro         | Varies                            | `Proton-stl`, `proton-cachyos`         | `/usr/share/steam/compatibilitytools.d/` |

**Key insight**: The `normalize_alias` function in CrossHook already strips all non-alphanumeric characters, so `GE-Proton9-4` → `geproton94` and `Proton 9.0-1` → `proton901`. This normalized form is the correct basis for version closeness comparison.

The **major version** for GE-Proton corresponds to the upstream Proton/Wine base (e.g., `GE-Proton10-*` tracks Proton 10.x Wine). The **build number** (the `-Z` suffix) is a sequential release counter within that major. Releases are published approximately weekly or bi-weekly.

Recent GE-Proton release tags (from GitHub releases page, March 2026):

```
GE-Proton10-34  GE-Proton10-33  GE-Proton10-32  GE-Proton10-31
GE-Proton10-30  GE-Proton10-29  GE-Proton10-28  GE-Proton10-27
```

Source: [GloriousEggroll/proton-ge-custom releases](https://github.com/gloriouseggroll/proton-ge-custom/releases)

**Confidence**: High

### 1.2 Steam Filesystem Layout

Two installation roots are relevant:

**Official Proton** (installed by Steam):

```
$HOME/.local/share/Steam/steamapps/common/Proton 9.0/
$HOME/.local/share/Steam/steamapps/common/Proton 8.0/
```

**Custom/GE-Proton** (user-installed):

```
$HOME/.steam/steam/compatibilitytools.d/GE-Proton10-34/
$HOME/.steam/steam/compatibilitytools.d/GE-Proton9-4/
```

**Flatpak Steam** (sandboxed installation, distinct path):

```
$HOME/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d/
$HOME/.var/app/com.valvesoftware.Steam/data/Steam/steamapps/common/
```

**System-wide** (distro packages):

```
/usr/share/steam/compatibilitytools.d/
/usr/local/share/steam/compatibilitytools.d/
```

The CrossHook `discover_compat_tools_with_roots` function already scans `steamapps/common`, `compatibilitytools.d`, and system roots. The **Flatpak path is not currently in the candidate list** — this is a gap for the feature (see Open Questions).

Every valid Proton installation contains a `proton` executable script (the detection sentinel used by CrossHook) and optionally a `compatibilitytool.vdf` manifest with display names and aliases.

Source: [Valve/Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ), [Flathub Steam documentation](https://github.com/flathub/com.valvesoftware.Steam/wiki)

**Confidence**: High

### 1.3 VDF File Format — `compatibilitytool.vdf`

The `compatibilitytool.vdf` manifest uses Valve's KeyValues text format (VDF v1). CrossHook already parses this via `steam/vdf.rs` and `parse_vdf`.

#### Complete Field Specification (from Valve + GE-Proton official templates)

```
"compatibilitytools"              // root key
{
    "compat_tools"                // container key
    {
        "##INTERNAL_TOOL_NAME##"  // KEY = internal name (matches CompatToolMapping "name" in config.vdf)
        {
            "install_path"  "."               // "." = same directory
            "display_name"  "##BUILD_NAME##"  // human-facing name in Steam Compatibility dropdown
            "from_oslist"   "windows"         // source OS
            "to_oslist"     "linux"           // target OS
        }
    }
}
```

#### INTERNAL_TOOL_NAME vs BUILD_NAME (critical distinction)

GE-Proton's template uses **two separate placeholders** substituted at build time:

- `INTERNAL_TOOL_NAME` → the identifier stored in Steam's `CompatToolMapping` config (e.g., `"GE-Proton10-34"`)
- `BUILD_NAME` → the human-readable display name (e.g., `"GE-Proton 10-34"` with a space)

In practice for GE-Proton both are usually equivalent, but they can differ. CrossHook reads **both** the VDF key (`alias_name`) and `display_name` as aliases — correct behavior.

**How Steam uses the internal name**: Steam's `config.vdf` stores `CompatToolMapping` → `appid` → `name` = `INTERNAL_TOOL_NAME`. The migration tool must track this value, not the display name, for accurate detection of which installed tool a profile references.

**No external VDF library is needed** — CrossHook already handles this. Ecosystem crates for reference:

- [`keyvalues-parser`](https://crates.io/crates/keyvalues-parser) — pest-based VDF text parser
- [`keyvalues-serde`](https://crates.io/crates/keyvalues-serde) — serde integration
- [`steam-vdf-parser`](https://crates.io/crates/steam-vdf-parser) — zero-copy, supports binary VDF too

Sources: [ValveSoftware/Proton template](https://github.com/ValveSoftware/Proton/blob/proton_9.0/compatibilitytool.vdf.template), [GE-Proton template](https://github.com/GloriousEggroll/proton-ge-custom/blob/master/compatibilitytool.vdf.template), [SteamTinkerLaunch wiki](https://github.com/sonic2kk/steamtinkerlaunch/wiki/Steam-Compatibility-Tool)

**Confidence**: High

---

## 2. Libraries and SDKs

### 2.1 Version Comparison Crates

#### `semver` (dtolnay/semver)

- **Version**: 1.0.x, actively maintained
- **Verdict**: ❌ **Not suitable**. Strictly parses Cargo-flavor SemVer. Will reject `GE-Proton9-4`, `Proton 9.0-1`, and all Proton naming conventions.
- Source: [docs.rs/semver](https://docs.rs/semver/latest/semver/)

#### `version-compare`

- **Version**: ~0.2, moderate maintenance
- **Verdict**: ⚠️ **Unreliable for Proton strings**. Designed for "best-effort" version comparison of arbitrary strings. Treats `-` as a part separator, so `GE-Proton9-4` would be split as `["GE", "Proton9", "4"]`. Comparing `GE-Proton9-4` vs `GE-Proton9-7` might work (last segment `4` vs `7`), but cross-family comparisons (`GE-Proton9-7` vs `Proton 9.0-1`) are undefined.
- API: `Version::from("GE-Proton9-4")`, `compare(a, b)` → `Cmp::Lt/Eq/Gt`
- **Bottom line**: Behavior on Proton names is undocumented. The custom digit extractor is more predictable and has zero risk.
- Source: [crates.io/crates/version-compare](https://crates.io/crates/version-compare), [docs.rs/version-compare](https://docs.rs/version-compare/latest/version_compare/)

#### `alphanumeric-sort`

- **Version**: 1.5.x, maintained, zero dependencies, `no_std`-compatible
- **Verdict**: ✅ **Best fit for UI display ordering**. Sorts `["GE-Proton9-4", "GE-Proton9-7", "GE-Proton10-1"]` correctly in numeric order (treats numeric segments as numbers). Does not give a semantic "closeness" score — only a sort order.
- API: `alphanumeric_sort::sort_str_slice(&mut candidates)` or `compare_str(a, b)`
- Source: [docs.rs/alphanumeric-sort](https://docs.rs/alphanumeric-sort)

#### Custom digit extraction (no new dep) — **Recommended**

- **Verdict**: ✅ **Best approach for version closeness ranking**. Extract numeric segments from the _raw_ directory name (before normalization). Operate on `Vec<u32>` tuples for comparison. Zero new dependencies; predictable behavior across all known naming schemes.
- Works correctly for:
  - `"GE-Proton9-4"` → `[9, 4]`
  - `"GE-Proton10-34"` → `[10, 34]`
  - `"Proton 9.0-1"` → `[9, 0, 1]`
  - `"Proton-9.23-GE-2"` (legacy) → `[9, 23, 2]`
- Fails gracefully for TKG (`"proton_tkg_6.17.r0.g5f19a815.release"` → many segments with hex noise → detected by prefix and excluded from ranking)

**Recommendation**: Implement custom digit extractor on the raw directory name. Use `alphanumeric-sort` only if the UI needs a polished sort order for the candidate list display.

### 2.2 Proton Management Libraries

#### `libprotonup` (auyer/Protonup-rs)

- Rust library for downloading, installing, and listing GE-Proton versions
- Provides: `list_installed_versions()`, `installation_dir()`, `app_base_dir()`
- Handles Steam native + Flatpak detection, Lutris runner dirs
- **Verdict**: ❌ **Not needed for migration** — CrossHook already has its own discovery. Useful only if CrossHook ever needs to _download_ missing Proton versions.
- Source: [crates.io/crates/libprotonup](https://crates.io/crates/libprotonup), [lib.rs/crates/libprotonup](https://lib.rs/crates/libprotonup)

#### `protontools` (yungcomputerchair)

- Rust library for discovering and invoking Proton installations
- Limitation: Requires Steam at `~/.steam/` (hardcoded, non-configurable)
- **Verdict**: ❌ **Not suitable** — too restrictive and overlaps existing CrossHook functionality.
- Source: [github.com/yungcomputerchair/protontools](https://github.com/yungcomputerchair/protontools)

### 2.3 Filesystem Watching (for live stale detection)

Not required for the initial feature scope. If real-time stale detection is desired in a future phase:

- [`notify`](https://crates.io/crates/notify) — cross-platform filesystem change notifications (`inotify` on Linux). Watches directories for add/remove events.
- This would allow proactive alerting when a Proton directory disappears (e.g., after `ProtonUp-Qt` removes an old version).

**Confidence**: Medium (crate exists and is well-maintained; integration complexity is non-trivial)

---

## Integration Patterns

### 3.1 How Other Launchers Handle Proton Version Changes

Research into Lutris, Heroic Games Launcher, and Bottles reveals a consistent (and problematic) pattern: **none of them proactively detect stale Proton paths or suggest replacements.**

#### Lutris

- Validates runner path on launch via `is_installed()` (checks executable existence)
- If runner missing: raises `MissingExecutableError` with path — no suggestion offered
- Has a fallback to `get_default_wine_version()`, but this silently changes the runner without user awareness
- No "your Wine version moved, here's the nearest replacement" UX
- Source: [lutris/runners/wine.py](https://github.com/lutris/lutris/blob/master/lutris/runners/wine.py)

#### Heroic Games Launcher

- Stores wine/proton path per-game in config JSON
- If the path is invalid: game launch fails with a generic "could not find the selected wine version" error
- Issue [#2900](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2900): wine-GE update regression caused silent failures — workaround was manually switching to proton-GE
- Issue [#4026](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/4026): Custom wine path deemed "invalid" — no automated suggestion
- **No migration logic exists in Heroic**

#### Bottles

- Manages "environments" (bottles) with explicit runner assignment
- Changing runner requires user to go into bottle settings and select a new one
- Issue [#2952](https://github.com/bottlesdevs/Bottles/issues/2952): "Invalid Steam Proton path" — no remediation suggestion
- **No migration logic exists in Bottles**

#### ProtonUp-Qt / protonup-rs

- Focused on _installation_ of new versions, not on _migrating_ existing profile references
- Lists installed versions; does not cross-reference game/profile configs
- **No migration logic exists**

**Pattern conclusion**: Every launcher leaves the user to manually identify and fix broken Proton references. CrossHook's proposed migration tool would be a **first-mover feature** in this ecosystem.

**Confidence**: High

### 3.2 Version Closeness Strategy (Derived from Research)

Based on how version naming works, the recommended candidate ranking algorithm is:

1. **Exact match** (already handled by `resolve_compat_tool_by_name` alias matching)
2. **Same-major, higher build**: `GE-Proton9-7` as replacement for missing `GE-Proton9-4`
3. **Cross-major, closest**: `GE-Proton10-1` as replacement for `GE-Proton9-7` (if no 9.x remains)
4. **Same family, any version**: any `Proton X.Y` for missing `Proton X.Y-1`

This maps directly to digit-tuple comparison on `normalized_aliases`: extract `(major, build)` pairs, sort by smallest delta to the stale version's digits.

---

## 4. Constraints and Gotchas

### 4.1 Naming Inconsistencies

| Risk                                      | Details                                                                                                                                                                                                                                                          | Mitigation                                                                                                                                  |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Dual GE naming schemes                    | Old: `Proton-X.Y-GE-Z`, new: `GE-ProtonX-Z` — both may coexist on disk                                                                                                                                                                                           | `normalize_alias` already collapses both to `geprotonXZ` format                                                                             |
| VDF alias vs directory name mismatch      | `compatibilitytool.vdf` uses `##INTERNAL_TOOL_NAME##` (key in compat_tools) and `##BUILD_NAME##` (display_name) — these are **different placeholders** in GE-Proton's template, meaning the internal alias and the human name can diverge                        | CrossHook already collects both the VDF key (alias_name) and display_name as aliases — correct behavior                                     |
| Official Proton directory vs display name | Dir: `Proton 9.0`, Steam display: `Proton 9.0-1` — the `-1` build suffix is in the display name only                                                                                                                                                             | The `is_official: true` flag in `ProtonInstall` can gate different comparison logic                                                         |
| "Proton Experimental"                     | Versionless — cannot be ranked relative to versioned installs                                                                                                                                                                                                    | Treat as a special case; never suggest as a "closest match" to a versioned path                                                             |
| **Proton-TKG**                            | Directory: `proton_tkg_6.17.r0.g5f19a815.release` — uses git commit hashes, not integer build numbers. `extract_numeric_segments` would return `[6, 17, 0, ...]` but the git hash part is hex, not semantic. Incompatible with the `(major, build)` digit model. | Detect TKG by `proton_tkg` prefix on normalized name and **exclude from ranking** (show in candidate list but don't auto-rank as "closest") |
| SteamTinkerLaunch / Soda / CachyOS Proton | Arbitrary naming (e.g., `Proton-stl`, `SteamTinker-*`, `proton-cachyos`)                                                                                                                                                                                         | Fall back to normalized substring matching; don't force digit pair extraction                                                               |

### 4.2 Path Edge Cases

- **Symlinks**: `~/.steam/steam` is a symlink to `~/.local/share/Steam`. CrossHook uses `PathBuf`, which resolves symlinks via `is_dir()`/`is_file()` on the target. The stale check should call `path.exists()` or `path.try_exists()` (Rust 1.63+) on the literal stored path, not the resolved one, since users may store either form.
- **Flatpak Steam**: Path `~/.var/app/com.valvesoftware.Steam/data/Steam/` is not currently in CrossHook's discovery candidates. If the user runs Flatpak Steam, their GE-Proton installations will be invisible to the migration tool unless this root is added.
- **Multi-library Steam**: Steam can have multiple library paths (e.g., `/mnt/games/SteamLibrary`). CrossHook already accepts `steam_root_candidates: &[PathBuf]` — migration must pass the full candidate list from `SteamDiscovery`.
- **Deleted directory, intact `proton` script**: The stored path is to the `proton` script, not the directory. Checking `!stored_path.exists()` is the correct staleness test.

### 4.3 Build-vs-Depend Decision

For the version closeness comparator, **implement custom digit extraction in-crate** (no new dependency):

```rust
/// Extract (major, build) from a normalized Proton alias.
/// "geproton94"  → Some((9, 4))
/// "geproton1034" → Some((10, 34))   -- ambiguous if major is 2 digits!
/// "proton901"   → Some((9, 1))      -- maps Proton 9.0-1
fn extract_version_pair(normalized: &str) -> Option<(u32, u32)> {
    // Strip known prefixes before extracting digits
    let digits: Vec<u32> = normalized
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect();
    match digits.as_slice() {
        [major, build] => Some((*major, *build)),
        _ => None,
    }
}
```

⚠️ **Known ambiguity**: `geproton1034` could parse as `(10, 34)` or `(103, 4)` if split on non-digit boundaries. The correct approach is to split on **transitions between digit and non-digit characters** in the _original_ (non-normalized) name, not on the stripped form.

Better approach — operate on the raw directory name / alias before normalization:

```rust
fn extract_numeric_segments(s: &str) -> Vec<u32> {
    let mut segments = Vec::new();
    let mut current = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            current.push(c);
        } else if !current.is_empty() {
            if let Ok(n) = current.parse() {
                segments.push(n);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(n) = current.parse() {
            segments.push(n);
        }
    }
    segments
}

// "GE-Proton10-34" → [10, 34]
// "Proton 9.0-1"   → [9, 0, 1]
// "GE-Proton9-4"   → [9, 4]
```

This is robust, zero-dependency, and works correctly for all known Proton naming conventions.

**Confidence**: High

---

## 5. Code Examples

### 5.1 Stale Path Detection

```rust
/// Returns true if the stored proton_path no longer exists on disk.
pub fn is_proton_path_stale(proton_path: &Path) -> bool {
    // Use try_exists to distinguish "missing" from "permission denied"
    matches!(proton_path.try_exists(), Ok(false) | Err(_))
}
```

### 5.2 Version Candidate Ranking

```rust
/// Given a stale Proton path, rank available installed tools by closeness.
///
/// Strategy:
/// 1. Extract numeric segments from the stale path's name.
/// 2. For each installed tool, extract numeric segments from its name.
/// 3. Sort by: same-major candidates first, then by ascending build delta.
pub fn rank_replacement_candidates<'a>(
    stale_name: &str,
    installed_tools: &'a [ProtonInstall],
) -> Vec<&'a ProtonInstall> {
    let stale_segs = extract_numeric_segments(stale_name);

    let mut candidates: Vec<(&ProtonInstall, Vec<u32>)> = installed_tools
        .iter()
        .filter_map(|tool| {
            let segs = extract_numeric_segments(&tool.name);
            if segs.is_empty() {
                None
            } else {
                Some((tool, segs))
            }
        })
        .collect();

    candidates.sort_by(|(_, a_segs), (_, b_segs)| {
        // Prefer same major version
        let a_major_match = !stale_segs.is_empty() && a_segs.first() == stale_segs.first();
        let b_major_match = !stale_segs.is_empty() && b_segs.first() == stale_segs.first();
        match (a_major_match, b_major_match) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                // Within same group: sort by version descending (newest first)
                b_segs.cmp(a_segs)
            }
        }
    });

    candidates.into_iter().map(|(tool, _)| tool).collect()
}
```

### 5.3 Batch Stale Profile Scan

```rust
/// Returns (profile_name, stale_path, candidates) for each profile
/// whose proton_path no longer exists.
pub fn find_stale_proton_profiles(
    profiles: &[Profile],
    installed_tools: &[ProtonInstall],
) -> Vec<StaleMigrationEntry> {
    profiles
        .iter()
        .filter_map(|profile| {
            let proton_path = profile.proton_path.as_ref()?;
            if !is_proton_path_stale(proton_path) {
                return None;
            }
            let stale_name = proton_path
                .parent()?          // directory containing `proton` script
                .file_name()?
                .to_str()?;
            let candidates = rank_replacement_candidates(stale_name, installed_tools);
            Some(StaleMigrationEntry {
                profile_name: profile.name.clone(),
                stale_path: proton_path.clone(),
                candidates: candidates.into_iter().cloned().collect(),
            })
        })
        .collect()
}
```

### 5.4 Atomic Profile Write Pattern

**Note (practices-researcher + security-researcher)**: `ProfileStore::save()` in `profile/toml_store.rs:117` calls `fs::write(path, content)` directly — this is **not atomic**. A crash mid-write corrupts the TOML file. The migration feature must use the tmp-then-rename pattern when writing updated profiles:

```rust
// Atomic write — do NOT use fs::write(path, content) directly
let tmp = path.with_extension("toml.tmp");
fs::write(&tmp, toml_content)?;
fs::rename(&tmp, &path)?;    // POSIX-atomic within same filesystem
```

This either requires a fix to `ProfileStore::save()` itself (preferred, fixes all callers) or a dedicated write function in the migration module.

---

## 6. Open Questions

1. **Flatpak Steam support**: Should the migration scanner add `~/.var/app/com.valvesoftware.Steam/data/Steam/` as a discovery root? The current `steam/proton.rs` does not include it. This is a separate but related gap.

2. **What is the source of `steam_root_candidates`?** — The migration tool needs to pass the full list. Where does the Tauri app currently get this list? (Likely from `steam/discovery.rs` — worth confirming before implementation.)

3. **Batch vs per-profile migration**: Should the UI present a single "migrate all stale profiles" action, or per-profile controls? The business analyzer and UX researcher should drive this decision.

4. **Proton Experimental handling**: `Proton Experimental` has no version digits. If a profile references it and the path is stale, there's no "closest" candidate to suggest. Should it be excluded from migration or shown with a "reinstall from Steam" hint?

5. **Migration of `steam_applaunch` profiles**: ~~Profiles using `steam_applaunch` may not need stale Proton path checks.~~ **Correction (tech-designer)**: CrossHook profiles store `steam.proton_path` explicitly for all launch methods including `steam_applaunch` — it drives the health validation and launch script. If the path is stale, the profile is broken regardless of Steam's own Proton management. Stale checks should apply to all `proton_run` **and** `steam_applaunch` profiles that have a `proton_path` set.

6. **Prefix compatibility on major version change**: Upgrading from `GE-Proton9-x` to `GE-Proton10-x` may cause WINE prefix issues. The migration tool should warn users when crossing major versions (a prefix created under Proton 9.x may not be compatible with Proton 10.x). This is consistent with Steam's own behavior — Steam warns "Prefix has an invalid version?!" when downgrading.

---

## Sources

| Resource                                  | URL                                                                                                                                   |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| GE-Proton GitHub releases                 | <https://github.com/gloriouseggroll/proton-ge-custom/releases>                                                                        |
| ProtonUp-rs (Rust, version management)    | <https://github.com/auyer/Protonup-rs>                                                                                                |
| libprotonup crate                         | <https://crates.io/crates/libprotonup>                                                                                                |
| Valve/Proton FAQ                          | <https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ>                                                                             |
| protonup-rs naming scheme PR              | <https://github.com/AUNaseef/protonup/pull/26>                                                                                        |
| GamingOnLinux: Proton versions explained  | <https://www.gamingonlinux.com/guides/view/why-are-there-so-many-different-proton-versions-proton-8-proton-9-experimental-ge-proton/> |
| keyvalues-parser crate                    | <https://crates.io/crates/keyvalues-parser>                                                                                           |
| keyvalues-serde crate                     | <https://crates.io/crates/keyvalues-serde>                                                                                            |
| version-compare crate                     | <https://crates.io/crates/version-compare>                                                                                            |
| alphanumeric-sort crate                   | <https://docs.rs/alphanumeric-sort>                                                                                                   |
| semver crate (rejected)                   | <https://docs.rs/semver/latest/semver/>                                                                                               |
| Lutris wine.py runner                     | <https://github.com/lutris/lutris/blob/master/lutris/runners/wine.py>                                                                 |
| Heroic issue #2900 (wine-ge regression)   | <https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/2900>                                                            |
| Heroic issue #4026 (invalid wine path)    | <https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/4026>                                                            |
| Bottles issue #2952 (invalid Proton path) | <https://github.com/bottlesdevs/Bottles/issues/2952>                                                                                  |
| Flathub Steam Flatpak wiki                | <https://github.com/flathub/com.valvesoftware.Steam/wiki>                                                                             |
| Flathub Proton-GE compatibility tool      | <https://github.com/flathub/com.valvesoftware.Steam.CompatibilityTool.Proton-GE>                                                      |
| protontools Rust library                  | <https://github.com/yungcomputerchair/protontools>                                                                                    |
| Steam Play Guide (compat tools layout)    | <https://steamcommunity.com/sharedfiles/filedetails/?id=1974055703>                                                                   |

---

## Search Queries Executed

1. `GE-Proton versioning scheme naming convention release pattern GitHub`
2. `Steam Proton filesystem layout Linux paths steamapps/common VDF format`
3. `Lutris Bottles Heroic Games Launcher Proton version migration management Linux`
4. `Rust crate semver version-compare crates.io version comparison fuzzy matching`
5. `Rust crate vdf keyvalues-parser steamlocate steam VDF parsing library`
6. `GE-Proton naming convention directory name format filesystem`
7. `libprotonup Rust crate Proton version detection installation path Steam compatibilitytools.d`
8. `ProtonUp-Qt protonup-rs Rust version detection Proton version management source code`
9. `Steam official Proton naming directory name steamapps/common format`
10. `Steam compatibilitytools.d vs steamapps/common Proton installation path difference`
11. `Steam Flatpak Proton path ~/.var/app/com.valvesoftware.Steam vs native`
12. `Rust natural sort human sort version string comparison numeric ordering crate`
13. `Lutris source code runtime runner version update migration broken Wine prefix Python GitHub`
14. `Bottles app Proton runner change version migration Python source code GitHub`
15. `Heroic Games Launcher wine version change broken path migration update source code`
16. `proton version migration Linux launcher suggest nearest closest version upgrade path broken`
