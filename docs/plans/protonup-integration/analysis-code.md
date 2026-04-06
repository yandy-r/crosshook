# ProtonUp Integration — Code Pattern Analysis

This document catalogs the exact code patterns, integration points, and structural conventions that a ProtonUp feature implementation must follow. Patterns are extracted from the six primary reference files and cross-checked against the shared context document. Everything here is actionable.

## Executive Summary

The ProtonUp integration adds three concerns that each map to an existing, well-tested code path: (1) caching a remote release list — copy the `protondb/client.rs` cache-first pattern with `OnceLock<reqwest::Client>`; (2) streaming download progress — copy the `prefix_deps.rs` `AppHandle::emit` + background task pattern; (3) a settings sub-panel — copy `PrefixDepsPanel.tsx` component and its hook structure. Every new file must conform to the Request/Result/Error triple, validate-then-execute, and thin-command-layer patterns documented below. The `pub mod protonup;` declaration in `crosshook-core/src/lib.rs` and the `invoke_handler!` registration in `src-tauri/src/lib.rs` are the two mandatory integration-seam edits.

## Relevant Files

### Core Crate

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/lib.rs` — Module registry; add `pub mod protonup;`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs` — Canonical Request/Result/Error triple; copy struct layout and error enum with `.message()` + `Display`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs` — Canonical validate-then-execute; copy `validate_install_request` → `install_game` call sequence
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/mod.rs` — Canonical module re-export; copy `pub use` surface pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — Canonical `OnceLock<reqwest::Client>` + cache-first fetch + stale fallback; copy verbatim for release-list fetch
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `get_cache_entry`/`put_cache_entry`/`evict_expired_cache_entries` primitives; used directly via `MetadataStore`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` public API; inject via `State<'_, MetadataStore>`; call `.get_cache_entry()`, `.put_cache_entry()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/update/models.rs` — Error enum with `.message()` and exhaustive `Display` match; same pattern needed for `ProtonUpError`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml` — All deps present: `libprotonup`, `reqwest`, `flate2`, `tar`, `sha2`, `rusqlite`, `tokio` — no new deps required
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` — `normalize_alias` at line 411 is `pub(crate)` — must be promoted to `pub` for the protonup advisor module; `discover_compat_tools()` must be called post-install to refresh Proton list

### Tauri IPC Layer

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — Register `ProtonUpInstallState::new()` via `.manage()` and add new commands to `invoke_handler!`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs` — Add `pub mod protonup;`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/prefix_deps.rs` — Direct structural template for the install command: lock acquire, `tauri::async_runtime::spawn`, `AppHandle::emit`, background completion handler
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/update.rs` — `Mutex<Option<u32>>` PID cancellation pattern; `spawn_log_stream` helper for file-based streaming
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/install.rs` — Thin wrapper calling core with `.map_err(|e| e.to_string())`

### Frontend

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonInstalls.ts` — `reload()` counter pattern; call after install completes
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useUpdateGame.ts` — Stage machine, listen-before-invoke, `unlistenRef`, `canStart`/`isRunning`, `completedBeforeInvoke` race guard
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/PrefixDepsPanel.tsx` — UI install pattern: confirm modal, live log `<pre>`, `<progress>`, event filtering by identity fields
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/update.ts` — TS type mirror pattern: `interface`, `type` union for stage, `Record<Error, string>` validation messages map
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/proton.ts` — `ProtonInstallOption` shape; new `ProtonUpRelease` and `ProtonUpInstallProgress` types go in `types/protonup.ts`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useScrollEnhance.ts` — Any new `overflow-y: auto` container must be added to the `SCROLLABLE` selector here

## Architectural Patterns

### Pattern 1: Request/Result/Error Triple

Every domain defines three types in `models.rs`:

**Structure** (`install/models.rs:12-86`):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProtonUpInstallRequest {
    #[serde(default)]
    pub tool_name: String,    // e.g. "GE-Proton"
    #[serde(default)]
    pub version: String,      // e.g. "GE-Proton9-20"
    #[serde(default)]
    pub install_dir: String,  // target directory
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProtonUpInstallResult {
    #[serde(default)]
    pub succeeded: bool,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub installed_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtonUpError {
    Validation(ProtonUpValidationError),
    NetworkFailed { message: String },
    ExtractionFailed { message: String },
    ChecksumMismatch,
    InstallDirPathTraversal,   // security: see research-security.md
}
```

**Key requirements**:

- Error enum derives `Serialize + Deserialize` with `#[serde(rename_all = "snake_case")]`
- Implement `.message()` method returning `String` with exhaustive match
- Implement `Display` delegating to `.message()`
- Implement `From<ValidationError>` for `Error`
- All request fields `#[serde(default)]` to allow partial deserialization from frontend

### Pattern 2: Validate-then-Execute

From `install/service.rs:26-40`:

```rust
pub fn validate_install_request(request: &ProtonUpInstallRequest) -> Result<(), ProtonUpValidationError> {
    validate_tool_name(&request.tool_name)?;
    validate_version(&request.version)?;
    validate_install_dir(&request.install_dir)?;  // path traversal check here
    Ok(())
}

pub async fn install_protonup(request: &ProtonUpInstallRequest) -> Result<ProtonUpInstallResult, ProtonUpError> {
    validate_install_request(request)?;  // ALWAYS first
    // ... async network + extraction work
}
```

The validation function is also exposed as a separate `#[tauri::command]` to let the frontend validate before starting (pattern from `update.rs:26-32`).

### Pattern 3: Module-per-Domain with `mod.rs` Re-exports

From `install/mod.rs`:

```
crates/crosshook-core/src/protonup/
├── mod.rs           ← pub use re-exports only
├── models.rs        ← Request/Result/Error/ValidationError types
├── service.rs       ← validate_* + install/delete/list functions
├── client.rs        ← OnceLock HTTP client + cache-first fetch
└── tests.rs         ← #[cfg(test)] integration tests (optional, can inline)
```

`mod.rs` surface:

```rust
mod client;
mod models;
mod service;

pub use models::{ProtonUpError, ProtonUpInstallRequest, ProtonUpInstallResult, ProtonUpRelease};
pub use service::{delete_protonup, install_protonup, list_available_releases, validate_install_request};
```

### Pattern 4: OnceLock HTTP Client Singleton + Cache-First Fetch

From `protondb/client.rs:26-190`:

```rust
const CACHE_TTL_HOURS: i64 = 6;
const REQUEST_TIMEOUT_SECS: u64 = 6;
static PROTONUP_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn protonup_http_client() -> Result<&'static reqwest::Client, ProtonUpError> {
    if let Some(client) = PROTONUP_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| ProtonUpError::NetworkFailed { message: e.to_string() })?;
    let _ = PROTONUP_HTTP_CLIENT.set(client);
    Ok(PROTONUP_HTTP_CLIENT.get().expect("client initialized"))
}

pub async fn list_available_releases(metadata_store: &MetadataStore) -> Vec<ProtonUpRelease> {
    let cache_key = "protonup:versions:v1:ge-proton";  // authoritative per feature-spec.md

    // 1. Check valid cache (acquire lock briefly)
    if let Ok(Some(payload)) = metadata_store.get_cache_entry(cache_key) {
        if let Ok(releases) = serde_json::from_str::<Vec<ProtonUpRelease>>(&payload) {
            return releases;
        }
    }

    // 2. Async network fetch (lock released)
    match fetch_releases_from_github().await {
        Ok(releases) => {
            // 3. Reacquire lock to write cache
            let expires_at = (Utc::now() + ChronoDuration::hours(CACHE_TTL_HOURS)).to_rfc3339();
            if let Ok(payload) = serde_json::to_string(&releases) {
                let _ = metadata_store.put_cache_entry(
                    "https://api.github.com/repos/...",
                    cache_key,
                    &payload,
                    Some(&expires_at),
                );
            }
            releases
        }
        Err(e) => {
            tracing::warn!(%e, "GitHub release fetch failed, checking stale cache");
            // Stale fallback: MetadataStore raw SQL query (see protondb/client.rs:346-394)
            vec![]
        }
    }
}
```

**Critical rule**: Never hold the `Arc<Mutex<Connection>>` across an `.await` point. The read-release-reacquire sequence in `protondb/client.rs:95-130` is non-negotiable.

### Pattern 5: AppHandle::emit Background Install (Closest to ProtonUp)

From `prefix_deps.rs:168-323`:

**Lock state struct** (define in `commands/protonup.rs`):

```rust
pub struct ProtonUpInstallState {
    pub lock: ProtonUpInstallLock,  // Mutex<Option<String>> tracking active install version
}

impl ProtonUpInstallState {
    pub fn new() -> Self { ... }
}
```

**Command signature**:

```rust
#[tauri::command]
pub async fn install_protonup_version(
    version: String,
    tool_name: String,
    install_dir: String,
    app: AppHandle,
    metadata_store: State<'_, MetadataStore>,
    install_state: State<'_, ProtonUpInstallState>,
    settings_store: State<'_, SettingsStore>,
) -> Result<(), String> {
```

**Background spawn pattern** (from `prefix_deps.rs:234-320`):

```rust
// Acquire global install lock FIRST (returns Err if already installing)
let guard = install_state.lock.try_acquire(version.clone())
    .await
    .map_err(|e| e.to_string())?;

// Spawn download + extract in background
tauri::async_runtime::spawn(async move {
    let _install_guard = guard;  // dropped when task exits

    // Emit progress events during download
    let _ = app.emit("protonup-install-progress", ProtonUpProgressPayload {
        version: version.clone(),
        stage: "downloading",
        percent: 0,
        message: "Starting download...".to_string(),
    });

    match download_and_extract(&version, &tool_name, &install_dir).await {
        Ok(installed_path) => {
            let _ = app.emit("protonup-install-complete", ProtonUpCompletePayload {
                version: version.clone(),
                succeeded: true,
                installed_path,
                message: String::new(),
            });
        }
        Err(e) => {
            let _ = app.emit("protonup-install-complete", ProtonUpCompletePayload {
                version: version.clone(),
                succeeded: false,
                installed_path: String::new(),
                message: e.to_string(),
            });
        }
    }
});

Ok(())  // Return immediately; frontend listens for completion event
```

**Payload types** (follow `prefix_deps.rs:28-42`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProtonUpProgressPayload {
    version: String,
    stage: String,   // "downloading" | "extracting" | "verifying"
    percent: u8,
    message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProtonUpCompletePayload {
    version: String,
    succeeded: bool,
    installed_path: String,
    message: String,
}
```

### Pattern 6: Thin Tauri Command Layer

From `commands/install.rs` (confirmed in `prefix_deps.rs`):

```rust
// NO business logic in command files
// Call core → .map_err(|e| e.to_string())

#[tauri::command]
pub async fn list_protonup_releases(
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<ProtonUpRelease>, String> {
    crosshook_core::protonup::list_available_releases(&metadata_store)
        .await
        .map_err(|e| e.to_string())
}
```

IPC command name uses `snake_case` matching frontend `invoke()` calls. Register in `invoke_handler!` in `lib.rs`.

### Pattern 7: Frontend Stage Machine (listen-before-invoke)

From `useUpdateGame.ts:192-263` — the race guard is load-bearing:

```typescript
// Track whether completion arrived before invoke resolved
let completedBeforeInvoke = false;

// STEP 1: Subscribe BEFORE invoking
const unlisten = await listen<ProtonUpCompletePayload>('protonup-install-complete', (event) => {
  if (event.payload.version !== targetVersion) return; // filter by identity
  completedBeforeInvoke = true;
  if (event.payload.succeeded) {
    setStage('complete');
    protonInstalls.reload(); // trigger useProtonInstalls reload
  } else {
    setStage('failed');
    setError(event.payload.message);
  }
  unlistenRef.current = null;
  unlisten();
});
unlistenRef.current = unlisten;

// STEP 2: Invoke (returns immediately per pattern)
await invoke<void>('install_protonup_version', { version, toolName, installDir });

// STEP 3: Only transition to 'installing' if not already done
if (!completedBeforeInvoke) {
  setStage('installing');
}
```

The `unlistenRef` pattern at `useUpdateGame.ts:103-108` ensures cleanup on unmount and reset.

### Pattern 8: Frontend reload() Counter

From `useProtonInstalls.ts:33-37`:

```typescript
const [reloadVersion, setReloadVersion] = useState(0);

const reload = useCallback(() => {
  setReloadVersion((current) => current + 1);
}, []);
```

Incrementing an integer triggers `useEffect` re-run without resetting other state. The `ProtonUpPanel` calls `protonInstalls.reload()` in the completion handler.

### Pattern 9: TypeScript Type Mirror

From `types/update.ts` — every Rust type that crosses IPC must have a TS mirror in `types/protonup.ts`:

```typescript
// Mirror ProtonUpRelease from Rust
export interface ProtonUpRelease {
  tag_name: string; // snake_case matching Rust serde field names
  published_at: string;
  download_url: string;
  checksum_url: string | null;
  size_bytes: number;
  tool_name: string;
}

export interface ProtonUpInstallProgress {
  version: string;
  stage: 'downloading' | 'extracting' | 'verifying';
  percent: number;
  message: string;
}

export type ProtonUpInstallStage = 'idle' | 'preparing' | 'installing' | 'complete' | 'failed';

export interface ProtonUpValidationState {
  fieldErrors: Partial<Record<keyof ProtonUpInstallRequest, string>>;
  generalError: string | null;
}

// Mirror validation error messages for field mapping
export const PROTONUP_VALIDATION_MESSAGES: Record<ProtonUpValidationError, string> = {
  ToolNameRequired: 'A Proton tool must be selected.',
  VersionRequired: 'A version must be selected.',
  InstallDirRequired: 'An install directory is required.',
  InstallDirNotDirectory: 'The install directory path must be a directory.',
};
```

Keep validation message strings in sync with `ProtonUpValidationError::message()` in Rust.

### Pattern 10: MetadataStore State Injection

From `prefix_deps.rs:94-96` and `lib.rs:197-203`:

```rust
// lib.rs — managed state registration
.manage(metadata_store)                            // already registered
.manage(commands::update::UpdateProcessState::new())
.manage(commands::prefix_deps::PrefixDepsInstallState::new())
.manage(commands::protonup::ProtonUpInstallState::new())  // add this

// command function — injection via State
pub async fn install_protonup_version(
    metadata_store: State<'_, MetadataStore>,
    install_state: State<'_, ProtonUpInstallState>,
    // ...
```

`MetadataStore` is already managed; no additional `.manage()` needed for it.

## Integration Points

### Directory Status

`crates/crosshook-core/src/protonup/` does **not** exist yet — the directory must be created. The first commit must add both to and the content together, otherwise the crate will not compile.

### Files to Create

| File                                                    | Purpose                                                                                                          |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/protonup/mod.rs`             | Domain module entry; `pub use` re-exports                                                                        |
| `crates/crosshook-core/src/protonup/models.rs`          | `ProtonUpRelease`, `ProtonUpInstallRequest`, `ProtonUpInstallResult`, `ProtonUpError`, `ProtonUpValidationError` |
| `crates/crosshook-core/src/protonup/client.rs`          | `OnceLock<reqwest::Client>` singleton; `list_available_releases()` with cache-first + stale fallback             |
| `crates/crosshook-core/src/protonup/service.rs`         | `validate_install_request`, `install_protonup`, `delete_protonup`                                                |
| `src-tauri/src/commands/protonup.rs`                    | `ProtonUpInstallState`, 5 `#[tauri::command]` functions                                                          |
| `src/crosshook-native/src/hooks/useProtonUp.ts`         | Stage machine, listen-before-invoke, `canInstall`, `isInstalling`                                                |
| `src/crosshook-native/src/components/ProtonUpPanel.tsx` | Releases list, install/delete UI, log output, confirmation modal                                                 |
| `src/crosshook-native/src/types/protonup.ts`            | TS mirrors for all IPC types                                                                                     |

### Files to Modify

| File                                                    | Change                                                                            |
| ------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs:19`                   | Add `pub mod protonup;`                                                           |
| `src-tauri/src/commands/mod.rs`                         | Add `pub mod protonup;`                                                           |
| `src-tauri/src/lib.rs:202-203`                          | Add `.manage(commands::protonup::ProtonUpInstallState::new())`                    |
| `src-tauri/src/lib.rs:207-322`                          | Register 5 new commands in `invoke_handler!`                                      |
| `src/crosshook-native/src/components/SettingsPanel.tsx` | Add `<ProtonUpPanel />` sub-section                                               |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`    | Add `ProtonUpPanel`'s scroll container to `SCROLLABLE` selector                   |
| `crates/crosshook-core/src/steam/proton.rs:411`         | Change `pub(crate) fn normalize_alias` → `pub fn normalize_alias` (one-word edit) |

## Code Conventions

### Rust

- Module path: `crosshook_core::protonup::*`
- Error display: `impl fmt::Display for ProtonUpError { fn fmt(&self, f) { f.write_str(&self.message()) } }`
- Tauri command names: `snake_case` — `list_protonup_releases`, `install_protonup_version`, `delete_protonup_version`, `validate_protonup_install`, `get_default_protonup_install_dir`
- State struct: `pub struct ProtonUpInstallState { pub lock: ProtonUpInstallLock }` mirroring `PrefixDepsInstallState`
- Log warnings (not errors) for fail-soft operations: `tracing::warn!(%e, "context message")`
- Keep lock guard in scope until async task exits: `let _install_guard = guard;`

### TypeScript/React

- Hook: `useProtonUp()` returns `{ releases, stage, progress, error, install, deleteVersion, reset }`
- Component: `<ProtonUpPanel />` — no props needed if using shared preferences context
- BEM classes: `crosshook-protonup`, `crosshook-protonup__releases`, `crosshook-protonup__log`
- Event filtering: filter `event.payload.version !== targetVersion` (same as `PrefixDepsPanel.tsx:81-88` filtering by `profile_name`/`prefix_path`)
- Log line cap: `prev.slice(-200)` matching `PrefixDepsPanel.tsx:87`

### IPC Event Names

| Event                       | Payload type              | Direction          |
| --------------------------- | ------------------------- | ------------------ |
| `protonup-install-progress` | `ProtonUpProgressPayload` | backend → frontend |
| `protonup-install-complete` | `ProtonUpCompletePayload` | backend → frontend |

## Dependencies and Services

### Already Available (no new deps)

| Crate         | Current version                   | Use                                                        |
| ------------- | --------------------------------- | ---------------------------------------------------------- |
| `libprotonup` | `0.11.0` (declared, unused)       | API surface for releases list (pending GPL-3.0 resolution) |
| `reqwest`     | `0.13.2`                          | HTTP client for GitHub API (Option B fallback)             |
| `flate2`      | `1`                               | `.tar.gz` decompression                                    |
| `tar`         | `0.4`                             | Archive extraction                                         |
| `sha2`        | `0.11.0`                          | SHA-256 checksum verification                              |
| `serde_json`  | `1`                               | Cache payload serialization                                |
| `rusqlite`    | `0.39.0`                          | `external_cache_entries` TTL cache                         |
| `tokio`       | `1` (fs, process, rt, sync, time) | Async runtime                                              |

### Service Dependencies (injected via Tauri State)

| State type             | Already managed    | Inject via                        |
| ---------------------- | ------------------ | --------------------------------- |
| `MetadataStore`        | Yes (`lib.rs:197`) | `State<'_, MetadataStore>`        |
| `SettingsStore`        | Yes (`lib.rs:198`) | `State<'_, SettingsStore>`        |
| `ProtonUpInstallState` | No — must add      | `State<'_, ProtonUpInstallState>` |

### Cache Key Convention

Cache key for GitHub releases: `protonup:versions:v1:ge-proton` (per `feature-spec.md`; `shared.md` uses a different key — `feature-spec.md` is authoritative).

Source URL for `put_cache_entry`: `https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases`.

## Gotchas and Warnings

### Never Hold Mutex Across Await

`MetadataStore.with_conn()` acquires `Arc<Mutex<Connection>>` synchronously. Holding it across an `.await` will deadlock. The cache-first pattern in `protondb/client.rs:95-130` shows the correct sequence: read (acquire + release) → async I/O → write (acquire + release).

### Install Lock Pattern

`PrefixDepsInstallState.lock` prevents concurrent installs. The protonup equivalent `ProtonUpInstallState.lock` must use the same `try_acquire` / guard-drop approach. The `_install_guard = guard` kept alive in the spawned task ensures the lock is held for the duration of the download+extract, not just until the command returns.

### listen-before-invoke Race Condition

The `completedBeforeInvoke` flag in `useUpdateGame.ts:227` prevents the UI from regressing to `'installing'` if the backend completes before the `invoke` promise resolves. This is not hypothetical — fast local installs (from cache or SSD) can trigger the completion event before the JS event loop processes the `invoke` response.

### useScrollEnhance Registration (CRITICAL)

Any `overflow-y: auto` container added to `ProtonUpPanel.tsx` (e.g., the releases list or log output `<pre>`) must be added to the `SCROLLABLE` selector in `useScrollEnhance.ts`. Missing this causes dual-scroll jank on WebKitGTK. See `CLAUDE.md` and `shared.md:45`.

### MAX_CACHE_PAYLOAD_BYTES Enforcement

`MAX_CACHE_PAYLOAD_BYTES = 524_288` (512 KiB) is defined at `metadata/models.rs:152`. `cache_store.rs:37-46` silently stores `NULL` if the payload exceeds this limit. GE-Proton release list is typically 80–150 KB — fits safely. Still verify before caching a full unpaginated response.

### GPL-3.0 Licensing Blocker

`libprotonup = "0.11.0"` is declared in `Cargo.toml` but must not be linked until the GPL-3.0 vs. MIT conflict is resolved. Option B (`reqwest` + `flate2` + `tar`, all MIT) is the safe default. See `research-recommendations.md`. The `client.rs` implementation must be structured so that the `libprotonup` import can be swapped in by replacing only the `fetch_releases_from_github()` and `download_archive()` functions.

### Path Traversal in install_dir

`research-security.md` flags `install_dir` as a path traversal risk. The `validate_install_dir` function in `service.rs` must canonicalize the path and verify it does not escape the allowed root (e.g., `~/.local/share/Steam/compatibilitytools.d`). Do not rely on the user-supplied string directly for filesystem operations.

### PrefixDepsPanel Event Filtering

`PrefixDepsPanel.tsx:81-88` filters events by both `profile_name` and `prefix_path`. The ProtonUp equivalent must filter by `version` (and optionally `tool_name`) to avoid cross-contamination if multiple install attempts are somehow triggered. The lock prevents concurrent installs, but event listeners from a previous unmounted component may still receive stale events without the filter.

### SettingsPanel.tsx File Size

`shared.md:41` notes `SettingsPanel.tsx` is 49KB. Adding `<ProtonUpPanel />` should be done via import + component placement, not by inlining the panel code into `SettingsPanel.tsx`. Keep the panel component self-contained.

## Task-Specific Guidance

### Phase 1: Core + IPC (models, client, service, commands)

1. Create `protonup/models.rs` by copying `install/models.rs` and `update/models.rs` as templates, adapting field names and error variants to the ProtonUp domain.
2. Create `protonup/client.rs` by copying `protondb/client.rs` pattern: `OnceLock`, `protonup_http_client()`, cache-first with stale fallback. The only logic change is the GitHub Releases API endpoint and the `ProtonUpRelease` deserialization struct.
3. Create `protonup/service.rs` with `validate_install_request` and `install_protonup`. For Option B: implement download via `reqwest`, hash check via `sha2`, extract via `flate2`+`tar`. Follow validate-then-execute.
4. Create `commands/protonup.rs` copying `prefix_deps.rs` structure. The `ProtonUpInstallState` lock struct, `AppHandle::emit` loop, and background spawn are all direct copies with different type/event names.
5. Edit `lib.rs` to add `.manage(commands::protonup::ProtonUpInstallState::new())` after `PrefixDepsInstallState` registration (line ~203) and register 5 new commands in `invoke_handler!`.

### Phase 2: Frontend (types, hook, component)

1. Create `types/protonup.ts` mirroring all IPC types. Stage type should be `'idle' | 'preparing' | 'installing' | 'complete' | 'failed'`.
2. Create `hooks/useProtonUp.ts` using `useUpdateGame.ts` as the stage machine template and `useProtonInstalls.ts` for the `reload()` counter pattern.
3. Create `components/ProtonUpPanel.tsx` using `PrefixDepsPanel.tsx` as the UI template: confirm modal, `<progress>`, `<pre>` log output, event filtering by version.
4. Import `<ProtonUpPanel />` into `SettingsPanel.tsx` in the appropriate settings section.
5. Register any new `overflow-y: auto` containers in `useScrollEnhance.ts`.

### Testing

- Rust unit tests: mirror `install/service.rs:334-507` pattern; use `tempfile::tempdir()` for filesystem operations; use `#[tokio::test]` for async tests.
- Validation tests: one test per `ValidationError` variant, asserting the specific enum variant is returned (pattern from `install/service.rs:397-451`).
- Cache tests: test that a second call within TTL returns cached data without a network request (use `MetadataStore::open_in_memory()`).
- Frontend: no configured test framework; verify via dev build and manual smoke test per `CLAUDE.md`.
