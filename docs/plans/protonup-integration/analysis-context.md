# Context Analysis: protonup-integration

## Executive Summary

ProtonUp integration adds in-app Proton version management (browse, install, delete) to CrossHook by activating the already-declared `libprotonup = "0.11.0"` crate alongside the existing `external_cache_entries` SQLite TTL cache and `discover_compat_tools` filesystem scanner. The feature delivers three phases: backend-only foundation (Phase 1), core Settings UI (Phase 2), and community-profile integration + Wine-GE + cleanup (Phase 3). A GPL-3.0 license compatibility decision must be resolved before any code links `libprotonup`; Option B (direct `reqwest`/`flate2`/`tar`, all MIT-clean) is the fallback with ~300 extra lines.

---

## Architecture Context

- **System Structure**: Three-layer: `crosshook-core` owns all logic → `src-tauri/commands/` thin `#[tauri::command]` wrappers → React hooks calling `invoke()`. New `protonup/` module fits this model as a domain module under `crosshook-core/src/`.
- **Data Flow**: Frontend `invoke('list_available_proton_versions')` → Tauri command → `crosshook-core::protonup::fetcher` → check `MetadataStore::get_cache_entry` (release lock) → async GitHub via `libprotonup` → `put_cache_entry` → return `Vec<AvailableProtonVersion>`. Install: `invoke('install_proton_version')` → acquire `ProtonupInstallState` lock → `libprotonup` download stream → `AppHandle::emit("protonup-install-progress", ...)` events → on completion emit `protonup-install-complete` → frontend calls `useProtonInstalls.reload()`.
- **Integration Points**: `crosshook-core/src/lib.rs` (add `pub mod protonup;`), `src-tauri/src/commands/mod.rs` (add `pub mod protonup;`), `src-tauri/src/lib.rs` (register 5 commands + `.manage(ProtonupInstallState)`), `SettingsPanel.tsx` (add `<ProtonVersionManager />`), `useScrollEnhance.ts` (register any new `overflow-y: auto` container).

---

## Critical Files Reference

- `src/crosshook-native/crates/crosshook-core/src/lib.rs`: Add `pub mod protonup;` here — the `protonup/` directory exists but is **not yet declared**
- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: Copy the cache-first fetch pattern (lines 85-130) verbatim into `protonup/fetcher.rs` — do not abstract
- `src/crosshook-native/src-tauri/src/commands/prefix_deps.rs`: **Primary implementation template** — lines 234-320 are the proven `AppHandle::emit` event streaming pattern for install progress
- `src/crosshook-native/src-tauri/src/commands/update.rs`: `UpdateProcessState` / `cancel_update` pattern for cancellable install with `tokio::sync::Mutex<Option<AbortHandle>>`
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `get_cache_entry`/`put_cache_entry`; `MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB) — GE-Proton release list is ~80-150 KB, fits safely
- `src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: `discover_compat_tools`, `normalize_alias` (promote from `pub(crate)` to `pub` for advisor), `safe_enumerate_directories` follows symlinks (acceptable for read, not write)
- `src/crosshook-native/crates/crosshook-core/src/steam/models.rs`: `ProtonInstall` struct — implement `From<ProtonInstall> for InstalledProtonVersion` to avoid duplicating discovery logic
- `src/crosshook-native/crates/crosshook-core/src/offline/network.rs:9`: `is_network_available()` — call before any GitHub API fetch
- `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs`: Symlink detection pattern (`symlink_metadata()` + `is_symlink()` guard) — mirror this for install target validation
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:142`: `default_proton_path` — add `preferred_ge_proton_version: String` with `#[serde(default)]` here
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: **Critical** — any new `overflow-y: auto` container must be registered in `SCROLLABLE` selector or dual-scroll jank occurs
- `src/crosshook-native/src/hooks/useUpdateGame.ts`: Frontend template for streaming install hooks (listen before invoke, `unlistenRef`, stage machine)
- `src/crosshook-native/src/components/PrefixDepsPanel.tsx`: UI template for background install with live progress
- `src/crosshook-native/src/components/SettingsPanel.tsx`: 49 KB file — integration point for new `<ProtonVersionManager />` sub-panel

---

## Patterns to Follow

- **Module-per-Domain**: `protonup/` directory with `mod.rs`, `models.rs`, `fetcher.rs`, `installer.rs`, `advisor.rs`, `error.rs` — re-exports from `mod.rs` only; `src/crosshook-native/crates/crosshook-core/src/install/mod.rs`
- **Request/Result/Error Triple**: `*Request`, `*Result`, `*Error` in `models.rs`; `#[serde(rename_all = "snake_case")]`; `Display` via `.message()`; `From<ValidationError>`; `src/crosshook-native/crates/crosshook-core/src/install/models.rs`
- **Cache-First Fetch (Copy, Don't Abstract)**: Read cache → release Mutex → async network → reacquire Mutex → write; never hold Mutex across `.await`; `protondb/client.rs:85-130`
- **OnceLock HTTP Client Singleton**: `static OnceLock<reqwest::Client>`, lazily initialized with `CrossHook/<version>` UA; `protondb/client.rs:26`
- **AppHandle::emit Streaming**: Return `Result<(), String>` immediately from command; background task emits `protonup-install-progress` and `protonup-install-complete` (kebab-case, consistent with `update-log`, `update-complete`, `prefix-dep-log`); `commands/prefix_deps.rs:234-320`
- **Managed Tauri State**: `ProtonupInstallState` = `tokio::sync::Mutex<Option<AbortHandle>>`; registered via `.manage()` in `src-tauri/src/lib.rs`
- **Thin Tauri Command Layer**: No business logic in command files; `.map_err(|e| e.to_string())`; `commands/install.rs`
- **Frontend Invoke Hook**: `useState` + `useEffect` + `invoke()` + `reload()`; for streaming: listen before invoke, `unlistenRef`, stage machine; `useUpdateGame.ts`
- **Listen-Before-Invoke Race Guard**: Set `completedBeforeInvoke = true` inside the event handler before calling `invoke()`; after `invoke()` returns, only transition to `'installing'` stage if `!completedBeforeInvoke`. Without this, a fast install can emit `protonup-install-complete` before the hook has subscribed, silently dropping the completion event. Pattern source: `useUpdateGame.ts`
- **Validate-then-Execute**: `validate_install_dir()` before any extraction; `install/service.rs`

---

## Cross-Cutting Concerns

- **GPL-3.0 Blocker**: `libprotonup` is GPL-3.0; CrossHook is MIT. **No code using `libprotonup` may be written until this is resolved.** Feature spec decision #5 says to update CrossHook license if it makes sense — discuss with team. Option B fallback is `reqwest`/`flate2`/`tar` (all MIT-clean, already deps).
- **Security — 3 CRITICALs must ship-block**: (1) `astral-tokio-tar` CVE chain — resolved by `libprotonup 0.11.0` pinning `astral-tokio-tar 0.6.0`; still requires wrapper path-prefix guard; (2) `install_dir` path traversal — validate against `$HOME` + known Steam library paths, reject `..`, use `symlink_metadata()` before write; (3) Archive bomb — enforce 8 GB byte cap and 50,000 file count cap in CrossHook extraction wrapper.
- **BR-1 Unconditional**: Profile launch must never be gated by ProtonUp state. The feature is install/suggestion only — never intercepts launch.
- **MetadataStore Mutex Discipline**: Never hold across `.await`. Pattern: lock → read → unlock → await network → lock → write → unlock. Deadlock risk is real.
- **Scroll Registration**: Every `overflow-y: auto` container in the new UI panel must be added to `SCROLLABLE` in `useScrollEnhance.ts`. This is a CLAUDE.md requirement.
- **Settings DTO Sync**: `AppSettingsIpcData` in `commands/settings.rs` must manually mirror `AppSettingsData`. New `preferred_ge_proton_version` and `preferred_version_stale: bool` fields must appear in both.
- **No Resume**: `libprotonup 0.11.0` has no HTTP range request support. Document as a known limitation in the UI. Do not attempt a workaround in Phase 1.
- **Progress Polling Interval**: 200ms (not 500ms as in `update.rs`) — downloads are large and users expect responsive progress bars.

---

## Parallelization Opportunities

### Phase 1 — Three Waves

**Wave 0 — Seed tasks (strictly sequential, ~5 min each, block everything else):**

1. Add `pub mod protonup;` to `crosshook-core/src/lib.rs` + create `protonup/mod.rs` (must land in same commit)
2. Promote `normalize_alias` to `pub` in `steam/proton.rs` (one-line change, unblocks advisor)

**Wave 1 — Parallel (after Wave 0):**

- `protonup/models.rs` (all Serde types, `From<ProtonInstall>`, `VersionChannel`, `InstallPhase`)
- `protonup/error.rs` (typed `ProtonupError` enum)
- `AppSettingsData` settings field (`preferred_proton_version`) — different file, no dependency on models
- `protonup/fetcher.rs` (cache-first fetch; depends on models and error)

**Wave 2 — After Wave 1 models complete:**

- `protonup/installer.rs` — the most complex Phase 1 task (~150-200 lines); security mitigations C-1/C-2/C-3 are embedded here, not separate tasks
- `protonup/advisor.rs` — blocked on both `normalize_alias` (Wave 0) and `models.rs` (Wave 1)

**Wave 3 — After all core modules:**

- `src-tauri/src/commands/protonup.rs` (new file — does not exist yet; 5 commands + `ProtonupInstallState`)
- Unit tests (cache logic, `From<ProtonInstall>`, path validation, idempotency, stale fallback)

### Phase 2

Frontend hooks and UI component can be parallelized:

- `useProtonVersions.ts` and `useInstallProtonVersion.ts` hooks (independent of each other)
- `ProtonVersionManager` component (blocked on hooks)
- Settings panel additions (partially independent of full component)
- `useScrollEnhance.ts` registration (independent, can do immediately)

### Phase 3

All Phase 3 tasks are mutually independent once Phase 1+2 are complete:

- Community profile auto-suggest (`advisor.rs` already exists)
- Wine-GE support (second cache key + enum variant)
- Cleanup UI (orphan detection)

---

## Implementation Constraints

- **No new DB tables in Phase 1**: `external_cache_entries` (migration 3→4, schema v18) is sufficient. Cache key convention: `protonup:versions:v1:ge-proton`.
- **No new Cargo dependencies**: `libprotonup`, `reqwest`, `sha2`, `nix`, `rusqlite`, `tokio` are all already in `crosshook-core/Cargo.toml`.
- **Pin `libprotonup` exactly**: Use `= "0.11.0"` (not `"0.11"`) — 0.x API may break on any minor bump. Verify `Cargo.lock` shows `astral-tokio-tar 0.6.x`, no `tokio-tar` (abandoned).
- **GE-Proton only in Phase 1**: Wine-GE deferred to Phase 3. Avoids scope creep on `VersionChannel` enum variants.
- **Primary Steam root in Phase 1**: Multi-library install target deferred to Phase 3. Auto-detect via `libprotonup::apps::AppInstallations`.
- **Embed in Settings, not new route**: Adding `<ProtonVersionManager />` to `SettingsPanel.tsx` avoids touching `AppRoute` union type in `Sidebar.tsx`/`ContentArea.tsx`/`App.tsx`.
- **`protonup/` directory already exists but is empty**: Both `pub mod protonup;` in `lib.rs` and `mod.rs` content must land in the same commit or the build fails.
- **Steam path not in settings**: `AppSettingsData` has no steam path field. Commands must call `default_steam_client_install_path()` from `commands/steam.rs` or accept as parameter.
- **`MetadataStore::disabled()` fallback**: When SQLite fails at startup, `MetadataStore` initializes as disabled. Fetcher must degrade to always-fetch-from-network when cache is unavailable.
- **File naming**: Use `fetcher.rs` (not `client.rs`) — `feature-spec.md` takes precedence over `research-recommendations.md` which uses the older naming.
- **Security mitigations are embedded, not separate tasks**: C-1 (path-prefix guard), C-2 (`install_dir` validation), C-3 (archive bomb caps) all live inside `installer.rs`. They are not standalone tasks but are ship-blocking acceptance criteria for that one task.
- **`src-tauri/src/commands/protonup.rs` does not exist**: It is a new file to create in Wave 3 of Phase 1.

---

## Key Recommendations

- **Resolve GPL-3.0 license first** — block all implementation on this decision. If Option B, all infrastructure is already in place (`reqwest`, `flate2`, `tar`).
- **Start with the two one-liners**: `pub mod protonup;` in `lib.rs` + `normalize_alias` promoted to `pub`. These are zero-risk and unblock all parallel Phase 1 work.
- **Copy, don't abstract**: `protondb/client.rs:85-130` (cache-first fetch) and `commands/prefix_deps.rs:234-320` (event streaming) should be copied verbatim and adapted. Two call sites do not warrant a shared abstraction.
- **Security guards are non-negotiable before Phase 1 ships**: `install_dir` path validation, archive bomb caps (8 GB / 50,000 files), `symlink_metadata()` on install target. These are CRITICAL and ship-blocking.
- **Implement `From<ProtonInstall> for InstalledProtonVersion`** as the single bridge between existing `discover_compat_tools` and new `protonup_list_installed` command — avoids any duplicate filesystem scanning logic.
- **Cache stale fallback must be indefinite** (BR-4): Steam Deck users on airplane mode need a usable cached list with age indicator. No expiry ceiling. 24h TTL for refresh, 7-day stale window is the spec.
- **Run `cargo audit` after wiring `libprotonup`**: Verify no `tokio-tar` (abandoned) and `astral-tokio-tar = "0.6.x"` in `Cargo.lock`.
