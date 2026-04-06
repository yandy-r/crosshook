# ProtonUp Integration — Research Recommendations

## Executive Summary

The ProtonUp integration can be built almost entirely from existing infrastructure: `libprotonup` is already declared as a dependency but unused, `external_cache_entries` is the ready-made cache layer, the ProtonDB offline-fallback pattern is a proven template, and `commands/prefix_deps.rs:234-320` is the confirmed event streaming reference. The `crosshook-core/src/protonup/` module directory exists but is empty — this is the correct insertion point. No new SQLite tables are needed. `libprotonup 0.11.0` has been confirmed to provide native async streaming and supports downloading to a caller-specified path, making Option A viable without fallback. The coordination wrapper (`client.rs`, `install.rs`, `advisor.rs`, `models.rs`) is estimated at ~200 lines total — a narrow, focused scope. **One pre-code blocker exists: `libprotonup` is GPL-3.0; CrossHook is MIT. License compatibility must be verified and resolved before any code is written.**

---

## Implementation Recommendations

### Technology Choices

**Use `libprotonup = "0.11.0"` as the primary engine** — after license compatibility is confirmed. The tech-designer has confirmed it provides native async streaming and supports listing, downloading to a caller-specified path, and SHA512 verification. No fallback to direct GitHub API is required. The `protonup_binary_path` settings field should be reserved as a future escape hatch only.

**Cache the version list using a normalized `ProtonVersionListCache` model** — do not store raw `libprotonup::Release` JSON directly. A normalized model decouples CrossHook from upstream struct churn and allows adding `is_stale`, `fetched_at`, and `channel` metadata cleanly.

**Use the Tauri event bus for install progress** (same as `update.rs` / `prefix_deps.rs:234-320`), supplemented by a `get_proton_install_progress` poll command for UI reconnection after navigation away. This gives real-time push during install and state recovery on return.

**`libprotonup` is confirmed to be async** — do not wrap in `spawn_blocking`. Use `.await` directly in the Tauri command handlers.

**Call `is_network_available()` from `offline/network.rs:9` before attempting the GitHub API fetch.** This existing TCP probe (3 resolvers, 300ms timeout) gives a clean offline signal before the `libprotonup` call, matching the ProtonDB client's pre-fetch pattern.

### Known Limitation: No Download Resume

`libprotonup 0.11.0` does not support HTTP range requests. A 500 MB–1.5 GB download interrupted at any point restarts from zero. This is a significant UX regression for users on slow connections. It must be surfaced as a documented known limitation in the UI (e.g., "If the download is interrupted, it will restart from the beginning.") and tracked as a future improvement. Do not attempt to work around this in Phase 1.

### Data Source Separation

Proton version management has two distinct data sources that must remain independent and merge only in the UI layer:

- **Filesystem-derived installed versions**: synchronous, always current, derived from `discover_compat_tools` in `steam/proton.rs` — enumerates all `compatibilitytools.d/` directories across Steam library roots.
- **API-derived available versions**: asynchronous, cached with TTL, fetched from GitHub via `libprotonup`.

The `protonup_list_installed` command returns the filesystem view; `protonup_list_versions` returns the API/cached view. The UI merges them to mark which available versions are already installed.

**Avoid duplicating `list_proton_installs`**: The existing command returns `Vec<ProtonInstall>`; the new `protonup_list_installed` would return `Vec<InstalledProtonVersion>`. Implement `From<ProtonInstall> for InstalledProtonVersion` to convert without duplication. This is the clearest reuse opportunity in the design.

### Version Name Matching

Community profiles store `proton_version` as human-readable free-form strings (e.g., `"GE-Proton9-27"`). Installed tools may have directory names, VDF aliases, and display names that differ. **Do not use ad-hoc string comparison.**

`steam::proton::normalize_alias` is currently `pub(crate)`. The `protonup` advisor module needs it for fuzzy version matching. **Promote `normalize_alias` to `pub`** — this is cleaner than duplicating the logic in `protonup/advisor.rs`. The function has no external-API surface concerns; it is a pure string normalization utility.

### Install Progress Phases

The installer must emit four distinct phases via Tauri events: `downloading` → `verifying` → `extracting` → `complete`. Although the `verifying` phase (SHA512 check) takes under one second, it must be shown explicitly — the security concern about archive integrity makes it worth surfacing to the user. This also distinguishes a silent post-download failure (bad checksum) from a download failure.

### Phasing Strategy

**Phase 1 — Foundation (backend, no UI)**

- Wire the `protonup/` module into `crosshook-core/src/lib.rs` as the first step — the directory exists but is not yet declared as a module.
- Implement `crosshook-core/src/protonup/` with `mod.rs`, `client.rs`, `models.rs`, `install.rs`, `advisor.rs`
- `models.rs`: `ProtonVersionListCache` (normalized, not raw `libprotonup::Release`), `InstalledProtonVersion` with `From<ProtonInstall>`, `VersionChannel` enum (`GeProton`, `WineGe`), `InstallPhase` enum (`Downloading`, `Verifying`, `Extracting`, `Complete`)
- `client.rs`: list available GE-Proton releases via `libprotonup`, call `is_network_available()` before the API fetch, cache in `external_cache_entries` with a **24-hour TTL** and serve stale for up to **7 days** with age indicator, using cache key `protonup:version_list:ge-proton`; mirror the ProtonDB offline-fallback pattern exactly (`protondb/client.rs:85-130`)
- `install.rs`: download + extract via `libprotonup` to the correct `compatibilitytools.d/` directory; install target derived from `default_steam_client_install_path()` + optional `ProtonInstallRequest.install_dir` override; validate `install_dir` if user-supplied (must be within home directory bounds, no symlinks); emit phase events via `AppHandle::emit` (see `prefix_deps.rs:234-320`); SHA512 verify before extraction; check disk space via `nix::sys::statvfs` before starting; idempotent (already-installed = no-op with clear result)
- `advisor.rs`: fuzzy version matching against `proton_version` strings from community profiles using `normalize_alias` (promoted to `pub`) and `resolve_compat_tool_by_name`
- Add `preferred_ge_proton_version: String` (serde default empty) to `AppSettingsData`; validate against the installed list on settings load and include a `preferred_version_stale: bool` field in `AppSettingsIpcData`
- `ProtonupInstallState` with `tokio::sync::Mutex<Option<AbortHandle>>` for both concurrent-install protection and cancellation
- Register Tauri commands: `protonup_list_versions`, `protonup_install_version`, `protonup_cancel_install`, `protonup_list_installed`, `get_proton_install_progress`
- Write unit tests for cache logic, version list deserialization, `From<ProtonInstall>` conversion, path-traversal guard, idempotency, stale-cache fallback

**Phase 2 — Core UI**

- `ProtonVersionManager` component: version browser with filter/sort, install button, progress indicator with phase labels (`Downloading… (no resume support)`, `Verifying…`, `Extracting…`), known-limitation note about no resume
- `useProtonVersions.ts` hook: wraps `protonup_list_versions`, exposes versions, loading state, error, offline/stale indicator with cache age
- `useInstallProtonVersion.ts` hook: manages install lifecycle (idle → downloading → verifying → extracting → complete/error), listens to `protonup-install-progress` and `protonup-install-complete` Tauri events, calls `get_proton_install_progress` on mount for reconnection — mirrors `useUpdateGame.ts` shape
- Add to Settings panel: display of installed versions (via `useProtonInstalls`), preferred version selector, stale-preference warning
- Error states: GitHub unavailable (stale cache fallback), already-installed (no-op feedback), insufficient disk space (pre-install warning)
- Add to profile health check: warn when `proton_path` points to a non-existent directory
- Any new version list panel with `overflow-y: auto` must be registered in `useScrollEnhance.ts` `SCROLLABLE` selector

**Phase 3 — Polish and Community Integration**

- Community profile auto-suggest: cross-reference `proton_version` against installed versions via `advisor.rs`; offer one-click install if not present; use `normalize_alias` matching, not exact string equality
- Profile proton-path suggestion: after successful install, offer to apply the new path to the open profile
- Wine-GE support: second cache key `protonup:version_list:wine-ge`, `VersionChannel::WineGe` variant, UI toggle between channels
- Cleanup UI: list installed versions with size, flag orphans (no profile references the version), delete with reclaim estimate

### Quick Wins

1. `protonup_list_installed` reuses `discover_compat_tools` from `steam/proton.rs` with a `From<ProtonInstall>` conversion — near-zero new logic.
2. The offline stale-cache fallback path in `protondb/client.rs:85-130` adapts directly into `protonup/client.rs`.
3. `cancel_proton_install` adds ~20 lines to the command handler, following the `cancel_update` pattern exactly.
4. `AppSettingsData.default_proton_path` already exists; `preferred_ge_proton_version` slots alongside it with serde default = empty string.
5. `is_network_available()` in `offline/network.rs:9` is a one-call pre-flight check — no new network probe logic needed.
6. `commands/prefix_deps.rs:234-320` is the proven `AppHandle::emit` event streaming template — use it directly for install phase events.

---

## Improvement Ideas

### Related Features

- **Auto-update GE-Proton**: Background check for newer releases on startup; surface a notification badge. Reuse the startup async-spawn pattern from `lib.rs:188-192`.
- **Version pinning per profile**: Store the GE-Proton version name in the profile TOML under `[runtime.locked_proton_version]`. If the locked version is removed from disk, surface a health warning via the advisor module.
- **Cleanup old unused versions**: After listing installed versions, calculate which profiles reference each and flag orphans. Offer delete with a size-reclaim estimate — parallel to `scan_prefix_storage` / `cleanup_prefix_storage`.
- **Steam Deck space awareness**: Show the pre-install disk-space check result prominently, not just as a blocking error. On Steam Deck `~/.var` is the constrained partition.

### Future Enhancements

- **Download resume**: HTTP range request support would be the single biggest UX improvement for this feature. Requires either upstream `libprotonup` support or a switch to Option B for download only.
- **Scheduled background refresh**: Refresh the version list cache on startup (2-second delay, same pattern as `lib.rs:188`) to avoid stale-only state on first open.
- **Multi-Steam-library install target**: Let users choose which Steam library's `compatibilitytools.d/` to install to for space-constrained systems.
- **Proton version tagging in launch history**: Record the Proton version used in `launch_operations` metadata rows for post-hoc debugging.

---

## Risk Assessment

### Technical Risks

| Risk                                                                             | Severity     | Likelihood | Mitigation                                                                                                                                                    |
| -------------------------------------------------------------------------------- | ------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`libprotonup` is GPL-3.0; CrossHook is MIT — license conflict**                | **CRITICAL** | **High**   | **Resolve before writing any code.** Option A (legal review / relicensing / dual-license), Option B (LGPL exception if upstream offers one), Option C (Option B direct GitHub API + own extraction — MIT-clean alternative) |
| `astral-tokio-tar` CVE chain in `libprotonup::files::unpack_file` (CVE-2025-62518, CVE-2025-59825, CVE-2026-32766) | HIGH     | **Resolved** | All three CVEs fixed in `astral-tokio-tar >= 0.6.0`; `Cargo.lock` confirms `libprotonup 0.11.0` pulls `0.6.0`. CrossHook must still add a wrapper-level path-prefix guard (defense in depth). |
| **`install_dir` user-supplied path — no documented validation in `libprotonup`**   | **CRITICAL** | Medium     | Must validate against `$HOME` and known Steam library paths before calling `unpack_file`. Reject empty `install_dir` (derive from `default_steam_client_install_path()` only). Use `symlink_metadata()` on `compatibilitytools.d/` before any write (db.rs pattern). |
| No download resume — large download failure requires full restart                | HIGH         | Medium     | Document as known limitation; do not block Phase 1; track as future improvement; show UX warning before download starts                                       |
| **Archive bomb — no decompression size or file count limit in `libprotonup::files::unpack_file`** | **CRITICAL** | Medium     | CrossHook wrapper must enforce hard caps: 8 GB bytes and 50,000 files. GE-Proton is 300-600 MB compressed → 1-3 GB expanded; caps are generous but block malicious archives. Abort + delete partial files on violation. |
| `libprotonup` 0.x API breaking change                                            | HIGH         | Medium     | Pin to `=0.11.0`; add integration tests that exercise the API surface; treat every bump as potentially breaking                                               |
| GitHub rate limit under corporate NAT/VPN (60 req/hr shared)                     | HIGH         | Low-Medium | Cache with 24-hour TTL; serve stale up to 7 days; `force_refresh = true` is the only bypass                                                                   |
| Download corruption (network interruption)                                       | HIGH         | Medium     | SHA512 verify after download before extraction; delete partial files on failure; emit `verifying` phase so user sees verification happening                   |
| Disk space exhaustion during download or extraction                              | HIGH         | Medium     | Check `statvfs` before starting; warn at <2× tarball size free; on Steam Deck treat as soft warning not hard block                                            |
| Concurrent install via second Tauri call                                         | HIGH         | Low        | `tokio::sync::Mutex<Option<AbortHandle>>` — UI-only guards are insufficient since two Tauri calls can race                                                    |
| `preferred_ge_proton_version` going stale silently                               | MEDIUM       | Medium     | Validate against installed list on settings load; include `preferred_version_stale: bool` in `AppSettingsIpcData`                                             |
| `MAX_CACHE_PAYLOAD_BYTES` (512 KiB) too small for version list                   | LOW          | Low        | GitHub Releases API response for all GE-Proton versions is ~80-150 KB — within the 512 KiB cap                                                                |

### Integration Challenges

- **`normalize_alias` visibility**: Currently `pub(crate)` in `steam/proton.rs`. Must be promoted to `pub` for the `protonup/advisor.rs` module to use it. This is a one-line change with no behavioral risk.
- **`protonup/` module not wired**: The directory exists in `crosshook-core/src/` but is not declared in `lib.rs`. The first implementation commit must add `pub mod protonup;` to `crosshook-core/src/lib.rs`.
- **`list_proton_installs` overlap**: Existing command returns `Vec<ProtonInstall>`; new `protonup_list_installed` returns `Vec<InstalledProtonVersion>`. Implement `From<ProtonInstall> for InstalledProtonVersion` to avoid duplicating discovery logic.
- **`get_proton_install_progress` poll command**: Needed for UI reconnection after navigation. Must read from `ProtonupInstallState` without blocking. Returns the current phase or `None` if idle.
- **Tauri event naming consistency**: Existing events use kebab-case (`update-log`, `update-complete`, `prefix-dep-log`). New events: `protonup-install-progress`, `protonup-install-complete`.
- **Settings IPC DTO extension**: `AppSettingsIpcData` in `commands/settings.rs` must stay in sync with `AppSettingsData` manually. The new `preferred_ge_proton_version` and `preferred_version_stale` fields must appear in both structs.
- **Frontend scroll container**: Any new panel with a version list uses `overflow-y: auto` and must be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts` per CLAUDE.md, or dual-scroll jank will result.

### Performance Concerns

- Download is inherently slow (500 MB–1.5 GB) with no resume. The progress polling interval should be 200ms (not 500ms as in `update.rs`) to keep the progress bar responsive.
- Version list fetch: one GitHub API call and one JSON parse; network-bound, not compute-bound.
- Stale-cache reads (offline) are fast SQLite reads with no network wait.

### Security Concerns

See the full security evaluation in `docs/plans/protonup-integration/research-security.md`. Updated totals: **3 CRITICAL, 5 WARNING**. Priority order:

1. **GPL-3.0 license compliance (C-0)** — must resolve before linking `libprotonup`
2. **`install_dir` path traversal (C-2)** — no documented validation; must reject unsafe paths, derive default from Steam root only, apply `symlink_metadata()` guard on write target
3. **Archive bomb (C-3)** — no decompression size/file-count limit; wrapper must enforce 8 GB / 50,000-file hard caps with abort+cleanup
4. **`astral-tokio-tar` CVE chain (C-1)** — CVE-2025-62518, CVE-2025-59825, CVE-2026-32766; resolved by `libprotonup 0.11.0` pulling `astral-tokio-tar 0.6.0`; add defense-in-depth path-prefix guard in wrapper
5. SHA512 checksum verification before extraction (confirmed supported by libprotonup)
6. No user-supplied version string interpolation into subprocess arguments (not applicable if staying with libprotonup, becomes relevant if shelling out)
---

## Alternative Approaches

### Option A — `libprotonup` as Primary Engine (RECOMMENDED — pending license resolution)

**Description**: Use `libprotonup = "0.11.0"` (already in `crosshook-core/Cargo.toml`) for version listing, async streaming download to a caller-specified path, and SHA512 verification. CrossHook owns the cache layer, install lock, progress event emission, and extraction safety audit.

**Pros**:

- Zero new crate dependencies
- Native async — no `spawn_blocking` needed
- Typed API for GE-Proton and Wine-GE release metadata
- Supports download to caller-specified path
- Includes SHA512 verification

**Cons**:

- **GPL-3.0 license — CRITICAL blocker requiring legal review before proceeding**
- 0.x semver; breaking changes possible with any minor version bump
- No HTTP range request support (no download resume) — known limitation
- `astral-tokio-tar` CVE chain resolved in pinned `0.6.0`; wrapper must still add path-prefix guard and archive bomb caps (defense in depth)
- Less control over retry behavior and per-request timeouts

**Effort**: Low — `protonup/` module directory already exists; crate already pulled in; API confirmed viable. **Blocked on license compatibility.**

---

### Option B — Direct GitHub API + Custom Download/Extract (MIT-clean fallback)

**Description**: Use `reqwest` (already a dep) to call the GitHub Releases API directly, and `flate2`/`tar` (both already deps) for extraction.

**Pros**:

- Full control over request handling, retries, timeouts, byte-level progress
- No dependency on 0.x crate API stability
- Can add HTTP range request support for download resume
- Full control over extraction path validation
- **MIT-clean — no license conflict**

**Cons**:

- ~300 additional lines of code
- GitHub API response format changes are unguarded
- ETag/conditional GET needed for polite caching

**Effort**: Medium. Becomes the recommended approach if `libprotonup` GPL-3.0 is unresolvable.

---

### Option C — Shell Out to `protonup-rs` CLI Binary

Not recommended. Hard external dependency, no type-safe results, injection risk. See prior analysis.

---

**Recommendation**: Use Option A after confirming GPL-3.0 license compatibility. If the license cannot be resolved, fall back to Option B — all existing infrastructure (`reqwest`, `flate2`, `tar`, `external_cache_entries`, `is_network_available()`) is in place for a clean Option B implementation.

---

## Task Breakdown Preview

### Pre-Phase — Blockers (Must Resolve Before Code)

| Task                                                                                   | Complexity | Notes                                              |
| -------------------------------------------------------------------------------------- | ---------- | -------------------------------------------------- |
| **Resolve `libprotonup` GPL-3.0 vs CrossHook MIT license conflict**                   | Unknown    | Legal review; unblocks Option A or confirms Option B |
| Implement `install_dir` validation + archive bomb caps in `install.rs` wrapper                           | Medium     | Required before any extraction code ships; `astral-tokio-tar` CVE chain already resolved by pin to `0.6.0` |

### Phase 1 — Foundation

| Task                                                                                                                                         | Complexity | Parallelizable                       |
| -------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ------------------------------------ |
| Wire `pub mod protonup;` into `crosshook-core/src/lib.rs`                                                                                    | Trivial    | First — unblocks all Phase 1         |
| Promote `normalize_alias` to `pub` in `steam/proton.rs`                                                                                      | Low        | Immediately — unblocks advisor       |
| Implement `protonup/models.rs` (`ProtonVersionListCache`, `InstalledProtonVersion`, `From<ProtonInstall>`, `VersionChannel`, `InstallPhase`) | Low        | Yes                                  |
| Implement `protonup/client.rs` (list versions, `is_network_available()` pre-check, 24h TTL, 7-day stale fallback, offline mirror of protondb pattern) | Medium     | After models                         |
| Implement `protonup/install.rs` (async libprotonup download, phase events via `AppHandle::emit`, SHA512, disk check, path validation, idempotency) | High       | After models                         |
| Implement `protonup/advisor.rs` (fuzzy community profile version matching via `normalize_alias`)                                             | Low        | After `normalize_alias` promoted     |
| Add `preferred_ge_proton_version` + `preferred_version_stale` to `AppSettingsData` + `AppSettingsIpcData`                                    | Low        | Parallel                             |
| `ProtonupInstallState` (`tokio::sync::Mutex<Option<AbortHandle>>`) + 5 Tauri commands                                                        | Low        | After core modules                   |
| Unit tests: cache, `From<ProtonInstall>`, path-validation, idempotency, stale-cache fallback                                                 | Medium     | After implementations                |

### Phase 2 — Core UI

| Task                                                                                                 | Complexity | Parallelizable |
| ---------------------------------------------------------------------------------------------------- | ---------- | -------------- |
| `useProtonVersions.ts` hook (list + offline/stale state + cache age)                                 | Low        | Yes            |
| `useInstallProtonVersion.ts` hook (phase streaming, cancel, `get_proton_install_progress` reconnect) | Medium     | Yes            |
| `ProtonVersionManager` component (browse, filter, install, phase progress, no-resume warning)        | High       | After hooks    |
| Settings panel additions (installed list, preferred version picker, stale warning)                   | Medium     | After hooks    |
| Profile health warning for missing Proton path                                                       | Low        | Yes            |

### Phase 3 — Polish

| Task                                                                                                   | Complexity | Parallelizable       |
| ------------------------------------------------------------------------------------------------------ | ---------- | -------------------- |
| Community profile auto-suggest via `advisor.rs`                                                        | Medium     | Yes                  |
| Post-install profile path suggestion modal                                                             | Low        | Yes                  |
| Wine-GE support (`VersionChannel::WineGe`, second cache key)                                           | Medium     | After Phase 1 client |
| Installed version cleanup UI with orphan detection                                                     | Medium     | Yes                  |
| Download resume (HTTP range requests) — requires libprotonup upstream support or Option B for download | High       | Future               |

---

## Key Decisions Needed

0. **`libprotonup` GPL-3.0 license compatibility**: CrossHook is MIT. Linking a GPL-3.0 library requires the binary distribution to comply with GPL-3.0 copyleft. **This must be resolved before any code using `libprotonup` is written.** Three paths: (a) legal review and accept GPL-3.0 relicensing obligations, (b) check if `libprotonup` upstream offers an LGPL or dual-license exception, (c) proceed directly with Option B (direct GitHub API + `reqwest`/`flate2`/`tar`, all MIT-clean deps already in the project).

1. **`install.rs` wrapper security guards**: The `astral-tokio-tar` CVE chain (CVE-2025-62518, CVE-2025-59825, CVE-2026-32766) is resolved by `libprotonup 0.11.0` pinning `astral-tokio-tar 0.6.0`. CrossHook still must implement: (a) path-prefix validation that every extracted entry stays within the `install_dir` (defense in depth), (b) hard caps of 8 GB decompressed bytes and 50,000 files to prevent archive bomb, (c) `symlink_metadata()` check on `compatibilitytools.d/` before any write. These are not optional.

2. **Disk space threshold**: 2× tarball size free is recommended for extraction safety. On Steam Deck with constrained `~/.var` partition, consider making this a soft warning rather than a hard block.

3. **GE-Proton only in Phase 1, or GE-Proton + Wine-GE together?** GE-Proton only is recommended to keep Phase 1 scope tight. Wine-GE adds a `VersionChannel` enum variant and a second cache key — low additional effort but increases the testing surface.

4. **Cache TTL configurability**: 24h primary TTL is simple and correct for most users. Making it configurable adds complexity for minimal gain. Recommend keeping it fixed at 24h with a manual `force_refresh` escape.

5. **UI placement**: Settings tab (not a new sidebar panel) is recommended — this is a per-machine setup concern, not a per-profile concern.

6. **Install target when multiple Steam libraries exist**: Default to the primary Steam root in Phase 1; add multi-library selection in Phase 3.

---

## Open Questions (Resolved)

The following questions from the initial research were answered by the tech-designer's architecture analysis:

- **Does `libprotonup` support async?** Yes — native async, no `spawn_blocking` needed.
- **Does it download to a caller-specified path?** Yes — `ProtonInstallRequest.install_dir` override supported.
- **Does it verify checksums?** Yes — SHA512 verification included.
- **Is download resume supported?** No — known limitation, document and track for future.
- **Is `is_network_available()` available?** Yes — `offline/network.rs:9`, TCP probe, 300ms timeout, ready to use.
- **What is the event streaming reference?** `commands/prefix_deps.rs:234-320` — confirmed by practices-researcher.
- **What is the implementation scope?** ~200 lines of coordination wrapper across `client.rs`, `install.rs`, `advisor.rs`, `models.rs`.
- **Are the `astral-tokio-tar` CVEs resolved in the pinned version?** Yes — CVE-2025-62518 (PAX desync, CVSS 8.1), CVE-2025-59825 (symlink TOCTOU), CVE-2026-32766 (PAX extension validation) are all fixed in `astral-tokio-tar >= 0.6.0`; `Cargo.lock` confirms `libprotonup 0.11.0` pulls `0.6.0`. Wrapper-level guards (path-prefix + archive bomb caps) are still required as defense in depth.

## Open Questions (Unresolved)

- **`libprotonup` GPL-3.0 license resolution**: Highest-priority blocker. Which path — legal acceptance, upstream exception, or Option B — gates the entire implementation.
- **Actual GitHub API response size for all GE-Proton releases**: Must confirm it fits within `MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB). Expected ~80-150 KB but unverified.
- **`protonup/` directory origin**: Was it pre-created as a placeholder, or was prior code removed? Affects whether there are implicit expectations to match.
- **`preferred_ge_proton_version` granularity**: Exact version tag (e.g., `GE-Proton9-27`) is more deterministic but requires user action on each release. A major version family would be more durable but harder to validate against the installed list.
