# ProtonUp Integration — Business Analysis

## Executive Summary

Linux/Steam Deck gamers using CrossHook face a hard stop when a profile references a Proton version not installed on their system: the app reports a missing path with no path forward. This feature adds Proton version management — listing available GE-Proton/Wine-GE releases, installing them from within CrossHook, and surfacing install suggestions when community profiles specify a version not present on disk. All of this must degrade gracefully: profile launches must never be blocked, the UI must function offline using cached data, and if the ProtonUp library fails the user sees guidance rather than an error.

---

## User Stories

### Primary Users

**Linux desktop gamer on Steam / Steam Deck** who uses CrossHook to run trainers alongside games. They are technically comfortable with Linux but should not need to understand internal Proton installation paths.

### Stories

| ID   | As a…                       | I want…                                                                          | So that…                                                          |
| ---- | --------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| US-1 | gamer                       | to see a list of available GE-Proton and Wine-GE releases                        | I can choose the right Proton version for my game                 |
| US-2 | gamer                       | to install a specific Proton version directly in CrossHook                       | I never have to open a terminal or ProtonUp-Qt separately         |
| US-3 | gamer                       | to see real-time download/install progress                                       | I know the install is running and when it is done                 |
| US-4 | gamer                       | to be told which Proton version a community profile recommends                   | I can install exactly the right version without guessing          |
| US-5 | gamer                       | to browse available versions even when I am offline                              | I can plan which version to install when connectivity returns     |
| US-6 | gamer with a broken profile | to be shown an actionable suggestion when my Proton path is missing              | I know immediately how to fix the problem                         |
| US-7 | Steam Deck user             | to set a preferred Proton version in settings                                    | New profiles default to my preferred version automatically        |
| US-8 | gamer                       | to not have a game launch blocked just because the Proton manager is unavailable | I can still launch profiles with manually-configured Proton paths |

---

## Business Rules

### Core Rules

**BR-1 — Never block profile launch.**
Profile launch must proceed if `runtime.proton_path` is valid on disk. ProtonUp integration is an install/suggestion layer on top of the existing launch path — it must not intercept or gate the launch command. This invariant is unconditional. The `suggest_proton_version_for_profile` IPC command returns advisory data only; it never prevents launch.

**BR-2 — Installed versions from filesystem.**
The authoritative list of installed Proton versions is derived at runtime by scanning `~/.steam/root/compatibilitytools.d/` (and the other Steam roots already discovered by `discover_compat_tools`). This list is never persisted to SQLite or TOML; it is always re-derived on demand.

**BR-3 — Available versions via external cache (TTL = 24h).**
The available-versions list (releasable GE-Proton / Wine-GE) is fetched from the GitHub Releases API (via `libprotonup`) and cached in `external_cache_entries` with a 24-hour TTL. The cache key format is `protonup:versions:{tool_name}` (e.g. `protonup:versions:ge-proton`, `protonup:versions:wine-ge`). GE release lists are approximately 80–150 KiB — well within `MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB). A 24h TTL reduces GitHub API calls to 1–2 per day; the unauthenticated rate limit is 60 requests/hour per IP, so no token is required for normal usage.

**BR-4 — TTL and stale-fallback.**
On cache miss or TTL expiry: fetch live, update cache. If the live fetch fails, serve the stale entry with a visible age indicator in the UI. If no entry exists at all (first use, fully offline), show an empty list with an offline message. The stale-fallback window has no hard limit — Steam Deck users on airplane mode must still see a useful cached list however long it has been since the last fetch.

**BR-5 — Two new TOML settings fields.**
Two new fields are added to `AppSettingsData`, both with `#[serde(default)]` for backward-compatible deserialization of existing settings files:

- `preferred_proton_version: String` — user's preferred version tag (e.g. `GE-Proton9-27`); shown as pre-selection in new profile UI.
- `protonup_binary_path: String` — optional override path to a `protonup` binary; empty string means use `libprotonup` directly (the default).

Both fields are stored in `~/.config/crosshook/settings.toml`. The existing `default_proton_path` field is unchanged and serves a different purpose (absolute path for profile use vs. version tag for suggestions).

**BR-6 — Auto-suggest from community profiles.**
When the user browses or imports a community profile that has a non-null, non-empty `community_profiles.proton_version` value, CrossHook must check whether that version is installed. If it is not, surface a non-blocking suggestion: "This profile recommends GE-Proton9-27 — [Install]". The suggestion must never block import or use of the profile. If `proton_version` is null or an empty string, no suggestion is shown.

**BR-7 — ProtonUp binary not required at runtime.**
CrossHook uses `libprotonup = "0.11.0"` (already in `crosshook-core/Cargo.toml`) for Rust-native API access to release catalog and install logic. A separate `protonup` or `protonup-qt` binary is not required. The optional `protonup_binary_path` setting allows advanced users to override this. If the library's underlying install mechanism encounters an unrecoverable error, show clear guidance rather than a panic or silent failure.

**BR-8 — Download and install are ephemeral state.**
Active download progress (bytes received, percentage, speed) and install status are runtime-only, held in a Tauri-managed `ProtonUpInstallState` struct (`Arc<Mutex<HashMap<…, …>>>`). They must not be persisted to SQLite or TOML. The UI receives updates via Tauri events.

**BR-9 — One active install at a time (backend mutex).**
Only one Proton version install may be in progress at a time per CrossHook session. This constraint is enforced at the backend level using a lock modeled after `PrefixDepsInstallLock` in `crosshook-core/src/prefix_deps/lock.rs`. The lock is an `Arc<Mutex<Option<String>>>` wrapping the in-progress version tag; `try_acquire` returns a RAII guard on success or an `AlreadyInstalling` error variant if locked. UI may additionally disable the install button, but the backend is the authoritative guard.

**BR-10 — Offline mode respects settings flag.**
When `AppSettingsData.offline_mode` is `true`, no network calls for version lists are made. The UI shows the last cached list with cache age, or an offline message if no cache exists. Installed version scanning (filesystem) always works regardless of offline mode.

**BR-11 — Disk space warning before download.**
GE-Proton tarballs are approximately 500 MB–1.5 GB. Before starting a download, CrossHook must query available disk space on the install target partition and warn the user if available space is below 2 GB (covers the largest tarballs plus extraction headroom). There is no existing disk space check elsewhere in the codebase — this is a new pattern. The warning is non-blocking: the user may proceed after acknowledgement.

**BR-12 — No resumable downloads.**
`libprotonup 0.11.0` does not support HTTP range requests. An interrupted install starts from zero. The UI must not suggest "resuming" a download; if progress is interrupted, it restarts. Incomplete artefacts must be cleaned up before retrying.

**BR-13 — Version tag validation is advisory, not strict.**
Version strings in `preferred_proton_version` and `community_profiles.proton_version` are accepted as free text. CrossHook does not enforce a strict regex at settings-save time, because `libprotonup` is the authoritative source of valid version tags and validating at save time would require a network call from a settings handler (violating offline-first). When a stored string does not match the canonical pattern (`GE-Proton[0-9]+-[0-9]+` or `Wine-GE-[0-9]+-[0-9]+`), the UI shows a soft warning ("This version tag format is unrecognized"). Cross-checking against the live or cached available list happens at suggestion display time, not at save time.

### Edge Cases

- **Version already installed (reinstall idempotency)**: If the user attempts to install a version already present in `compatibilitytools.d`, the result is a no-op with clear positive feedback ("GE-Proton9-27 is already installed") rather than an error or a duplicate install. The install button is disabled for already-installed versions.
- **Partial download / interrupted install**: On restart, CrossHook must not assume a previous download completed. Install state is ephemeral; `libprotonup` is responsible for cleanup. The UI reports the restart from zero without implying resumability.
- **Multiple Steam library roots**: `install_dir` in `ProtonInstallRequest` is optional — when absent it defaults to the detected Steam `compatibilitytools.d` (native or Flatpak). When multiple roots exist, the user should be able to select the target from the detected list.
- **Conflicting tool names / heuristic matching**: After installing a new version, the UI must re-invoke `list_proton_installs` to refresh the dropdown without restarting the app.
- **Community profile proton_version format**: `community_profiles.proton_version` is TEXT, nullable, free-form, advisory only. Matching against installed tools uses `normalize_alias` (strips non-alphanumeric, lowercases). No suggestion is shown for null or empty values.
- **Version required but not installed and not available (network error + no cache)**: Show guidance ("This profile recommends a version that cannot be verified right now — check your connection and try again"). Never block profile launch. This is the lowest-priority degraded state.
- **Steam Deck read-only filesystem**: Installs to `~/.steam/root/compatibilitytools.d/` work normally. System-level compat tool roots (e.g. `/usr/share/steam/compatibilitytools.d/`) are read-only; CrossHook must not attempt to install there and must surface a clear error if such a path is selected.
- **GitHub API rate limit (429)**: At 24h TTL, normal usage is 1–2 API calls per day per user, far below the 60 req/hr unauthenticated limit. If the API returns a 429, treat it as a fetch failure and fall back to stale cache.

---

## Workflows

### Primary: Browse and Install a Version

```
1. User opens Settings or a new "Proton Manager" section
2. CrossHook loads installed versions from filesystem (sync, fast)
3. CrossHook loads available versions:
   a. Check external_cache_entries for cache_key 'protonup:versions:ge-proton'
   b. Cache hit (not expired) → display immediately, skip fetch
   c. Cache miss or expired → fetch from libprotonup / GitHub Releases API
      - Success: update cache, display list
      - Failure: serve stale cache if present (show cache age)
      - No cache at all: show offline/empty state
4. User filters / browses versions (installed vs. available)
5. User clicks "Install" on a non-installed version
6. Backend checks disk space on install target partition
   - Available space < 2 GB: show warning with required vs. available; user may proceed or cancel
7. Backend acquires ProtonInstallLock
   - Lock held: return AlreadyInstalling error, UI shows "Another install is in progress"
8. Backend starts async download + install via libprotonup
9. Frontend shows real-time progress (bytes, %) via Tauri events from ProtonUpInstallState
10. On completion: release lock, re-scan installed versions via list_proton_installs, update UI
11. Optional: user sets newly-installed version as preferred_proton_version in settings
```

### Secondary: Community Profile Suggests Missing Version

```
1. User browses community profiles (CommunityBrowser.tsx)
2. CrossHook queries installed Proton versions from filesystem
3. For each community profile row with a non-null, non-empty proton_version:
   - Normalize the version string via normalize_alias
   - Check against normalized_aliases of installed ProtonInstall entries
4. If NOT installed: display suggestion badge/button "Install GE-ProtonX-Y"
5. User clicks suggestion → enters Install workflow at step 5 above
6. Profile is still importable/usable regardless of whether user installs
```

### Error Recovery: Profile with Missing Proton Path

```
1. User opens ProfileHealth or LaunchPage with a profile referencing a Proton path
2. Health check classifies Proton path as Stale (NotFound) or Broken
3. Existing remediation text says "Re-browse to the file or verify the path is correct"
4. Enhanced remediation: "Proton version 'GE-Proton9-27' may not be installed.
   [Browse Available Versions]"
5. The link opens the version manager
6. User installs correct version, re-selects path in profile
```

### Offline: Version List with Cache Age

```
1. User opens ProtonUp manager while offline (or offline_mode = true)
2. Backend returns stale cache payload with fetched_at timestamp
3. Frontend shows version list with "Last updated X hours ago" indicator
4. Install button is disabled with tooltip "Network required to install"
5. Installed versions are still shown (filesystem scan works offline)
```

### Library Error / Degraded Mode

```
1. libprotonup encounters an unrecoverable configuration or network error
2. Backend returns structured error (not panic)
3. Frontend shows: "Proton version management is temporarily unavailable. [Details]"
4. Install feature degraded but profile launch is unaffected
```

---

## Domain Model

### Entities

| Entity                              | Storage                                                   | Description                                                                                        |
| ----------------------------------- | --------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `InstalledProtonVersion`            | Runtime only (filesystem scan)                            | A `ProtonInstall` row from `discover_compat_tools` — name, path, is_official, aliases              |
| `AvailableProtonVersion`            | `external_cache_entries` (JSON, TTL=24h)                  | A release entry from libprotonup / GitHub Releases — name, version, tag, download URL, asset size  |
| `preferred_proton_version`          | TOML settings (`settings.toml`)                           | User's preferred version tag (e.g. `GE-Proton9-27`), shown as pre-selection in new profile UI      |
| `protonup_binary_path`              | TOML settings (`settings.toml`)                           | Optional override path to a `protonup` binary; empty string = use libprotonup directly             |
| `default_proton_path`               | TOML settings (existing)                                  | User's default Proton executable path — already in `AppSettingsData`                               |
| `community_profiles.proton_version` | SQLite metadata DB (existing, read-only for this feature) | Version string stored by community tap indexer — nullable, free-form, advisory only                |
| `ProtonUpInstallState`              | Runtime only (Tauri `Arc<Mutex<HashMap>>`)                | Per-session install progress: bytes downloaded, total, status, version tag                         |

### State Transitions: Version Installation

```
Available → DiskCheck → Downloading → Installing → Installed
                ↓            ↓             ↓
           InsufficientDisk  Failed       Failed (restart from zero — no resume)
```

`Installed` transitions back to being an `InstalledProtonVersion` visible in the filesystem-derived list.

### Data Classification

| Datum                              | Classification                             | Rationale                                                                 |
| ---------------------------------- | ------------------------------------------ | ------------------------------------------------------------------------- |
| Available version list             | `external_cache_entries` (SQLite, TTL=24h) | External API data, needs offline fallback and cache age visibility        |
| Installed version list             | Runtime only                               | Authoritative source is the filesystem; stale DB copy would be misleading |
| User's preferred version           | TOML settings                              | User-editable preference, survives app restart                            |
| protonup binary override           | TOML settings                              | User-editable override, survives app restart                              |
| Download/install progress          | Runtime only (Tauri managed state)         | Ephemeral; irrelevant after session                                       |
| Community profile `proton_version` | SQLite (existing)                          | Already stored by community tap indexer; read-only for this feature       |

---

## Existing Codebase Integration

### Proton Discovery (Already Built)

- `crosshook-core/src/steam/proton.rs` — `discover_compat_tools()` scans all Steam roots and `compatibilitytools.d` for installed Proton versions. Returns `Vec<ProtonInstall>`.
- `crosshook-core/src/steam/models.rs` — `ProtonInstall` struct with name, path, is_official, aliases, normalized_aliases.
- `src-tauri/src/commands/steam.rs` — `list_proton_installs` IPC command. Already used by the frontend via `useProtonInstalls` hook.
- `src/crosshook-native/src/hooks/useProtonInstalls.ts` — existing React hook that calls `list_proton_installs`.
- `src/crosshook-native/src/components/ui/ProtonPathField.tsx` — existing UI for Proton path selection using the hook.

### External Cache (Already Built)

- `crosshook-core/src/metadata/cache_store.rs` — `get_cache_entry`, `put_cache_entry`, `evict_expired_cache_entries`. Operate on the `external_cache_entries` SQLite table.
- `crosshook-core/src/protondb/client.rs` — exemplar of the 3-stage cache→live→stale pattern with TTL, offline detection, and Tauri IPC.
- `crosshook-core/src/discovery/client.rs` — another exemplar using the same cache pattern for trainer RSS search.
- Schema: migration `3_to_4` created `external_cache_entries` with `cache_id, source_url, cache_key, payload_json, payload_size, fetched_at, expires_at, created_at, updated_at`.
- `crosshook-core/src/metadata/models.rs` — `MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB); GE-Proton release lists are ~80–150 KiB, well within this limit.

### Concurrent Install Lock (Already Built — Replicate)

- `crosshook-core/src/prefix_deps/lock.rs` — `PrefixDepsInstallLock` uses `Arc<Mutex<Option<String>>>` to prevent concurrent prefix dependency installs. `try_acquire(prefix_path)` returns a RAII `PrefixDepsLockGuard` on success or `PrefixDepsError::AlreadyInstalling` if locked. A `ProtonInstallLock` with the same shape must be created for this feature.

### Community Profiles (Already Built)

- `crosshook-core/src/profile/community_schema.rs` — `CommunityProfileMetadata.proton_version: String`.
- `crosshook-core/src/metadata/models.rs` — `CommunityProfileRow.proton_version: Option<String>` — nullable in the DB row.
- `src/crosshook-native/src/hooks/useCommunityProfiles.ts` — `proton_version` already in the profile entry type.
- `src/crosshook-native/src/components/CommunityBrowser.tsx` — already renders `proton_version` in the profile list.

### Settings (Already Built)

- `crosshook-core/src/settings/mod.rs` — `AppSettingsData` receives two new fields: `preferred_proton_version: String` and `protonup_binary_path: String`, both with `#[serde(default)]`.
- `src-tauri/src/commands/settings.rs` — `AppSettingsIpcData` and `SettingsSaveRequest` DTOs must be updated to include both new fields.

### Profile Health (Already Built)

- `crosshook-core/src/profile/health.rs` — `check_profile_health` already detects missing Proton paths and emits `HealthIssue` with `field = "runtime.proton_path"` and remediation text. The remediation text can be enriched to link to the version manager when the referenced version name is recognizable.

### libprotonup (Added, Module Empty)

- `crosshook-core/Cargo.toml` — `libprotonup = "0.11.0"` already present.
- `crosshook-core/src/protonup/` — directory created, empty. This is where the new `protonup` module will live.
- `crosshook-core/src/lib.rs` — `pub mod protonup;` is not yet registered; must be added when the module is implemented.

---

## Success Criteria

1. Users can view the full list of available GE-Proton and Wine-GE releases in CrossHook without opening a terminal.
2. Users can install any listed version and see real-time progress.
3. After install, the new version appears in the Proton path dropdown on the profile form without restarting the app.
4. When a community profile recommends a version not installed, a non-blocking suggestion is shown with a direct link to install.
5. The feature works offline: cached version list (24h TTL, indefinite stale-fallback) is shown with age indicator; install is disabled with explanation.
6. Profile launch is never blocked or delayed by ProtonUp integration.
7. If `libprotonup` fails or the network is unavailable, the app shows actionable guidance rather than a crash or silent failure.
8. A disk space warning is shown before any download begins when available space is below 2 GB.
9. Attempting to install an already-installed version results in a clear "already installed" message, not an error or duplicate install.
10. An interrupted install restarts from zero with no stale state carried over.

---

## Open Questions

1. **Install target directory (multiple Steam roots)**: Should CrossHook let the user choose from detected Steam library roots when more than one exists, or always default to the primary root?
2. **Version categories display**: Does libprotonup expose GE-Proton and Wine-GE in a single call or separate requests? Should they be shown in separate sections or mixed with a filter?
3. ~~**TTL value**~~: **Resolved** — 24h TTL; GitHub API unauthenticated rate limit (60 req/hr) supports low-frequency fetching. GE release lists are 80–150 KiB, within cache size limits.
4. ~~**Preferred version vs. default path**~~: **Resolved** — Both fields are kept; they serve different purposes (`default_proton_path` is an absolute path for profile use; `preferred_proton_version` is a version tag for UI pre-selection and suggestions).
5. ~~**Concurrent install guard**~~: **Resolved** — Backend mutex using `ProtonInstallLock` (modeled after `PrefixDepsInstallLock`). UI may additionally disable the button, but the backend is authoritative.
6. **Disk space threshold**: 2 GB is the recommended threshold. Confirm with UX whether this should be a fixed constant or a configurable setting.
7. ~~**Version tag validation**~~: **Resolved** — Free-text accepted at save time; soft UI warning for strings not matching canonical patterns. Cross-checking against the available list happens at suggestion display time, not save time.
8. ~~**Version required but unavailable (network error + no cache)**~~: **Resolved** — Show guidance, never block launch.
