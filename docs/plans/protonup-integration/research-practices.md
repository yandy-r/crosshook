# Engineering Practices Research: ProtonUp Integration

## Executive Summary

The codebase has a mature, consistent architecture that gives protonup-integration a clear implementation path with minimal greenfield work. The three critical infrastructure pieces — `external_cache_entries` TTL cache, Tauri event-based progress streaming, and filesystem-based Proton scanning — all exist and are proven. `libprotonup 0.11.0` is already a declared dependency but currently unused; it covers version listing and the actual download/extract pipeline, so CrossHook only needs to build the thin coordination layer around it. The proposed architecture is not over-engineered: a single `protonup/` module in `crosshook-core` following the `discovery/` or `protondb/` pattern is correct sizing.

---

## Existing Reusable Code

| Module / File                                                                      | Location                                             | Purpose                                                                    | How to Reuse                                                                                                                                                                          |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cache_store::get_cache_entry` / `put_cache_entry` / `evict_expired_cache_entries` | `metadata/cache_store.rs:6–106`                      | `external_cache_entries` TTL read/write/evict on `MetadataStore`           | Call `metadata_store.put_cache_entry(source_url, cache_key, payload_json, expires_at)` to persist fetched version lists; `get_cache_entry(cache_key)` to check before hitting network |
| `MetadataStore::get_cache_entry` / `put_cache_entry`                               | `metadata/mod.rs:523–545`                            | Public façade wrapping `cache_store`                                       | Direct call site — no raw `Connection` needed                                                                                                                                         |
| `MetadataStore::open_in_memory`                                                    | `metadata/db.rs:53–61`                               | In-memory SQLite for tests                                                 | Use in every `#[tokio::test]` to avoid hitting disk                                                                                                                                   |
| `discovery::client` 3-stage cache→live→stale pattern                               | `discovery/client.rs:231–300`                        | Cache-first fetch with stale fallback                                      | Copy the `fetch_source` pattern verbatim; replace RSS parsing with `libprotonup::downloads::list_releases`                                                                            |
| `offline::network::is_network_available`                                           | `offline/network.rs:9–15`                            | TCP probe to DNS resolvers                                                 | Call before making live GitHub requests; skip fetch when offline                                                                                                                      |
| `steam::proton::discover_compat_tools` / `discover_compat_tools_with_roots`        | `steam/proton.rs:24–263`                             | Scans `compatibilitytools.d` directories for installed Proton tools        | Call to enumerate installed GE-Proton versions at runtime; returns `Vec<ProtonInstall>`                                                                                               |
| `steam::discovery::discover_steam_root_candidates`                                 | `steam/discovery.rs`                                 | Finds Steam root paths                                                     | Already called by `list_proton_installs` Tauri command — reuse in a new `protonup_list_installed` command                                                                             |
| `SYSTEM_COMPAT_TOOL_ROOTS` constant                                                | `steam/proton.rs:9–13`                               | System-level compat tool directories                                       | No change needed; `discover_compat_tools` already reads these                                                                                                                         |
| `AppSettingsData::default_proton_path` field                                       | `settings/mod.rs:142`                                | Per-user preferred Proton path TOML setting                                | Add `preferred_ge_proton_version: String` here alongside existing `default_proton_path`                                                                                               |
| `SettingsStore::load` / `save`                                                     | `settings/mod.rs:321–331`                            | Atomic TOML read/write with mutex                                          | Add new setting field with `#[serde(default)]`; zero migration cost                                                                                                                   |
| `OnceLock<reqwest::Client>` singleton                                              | `discovery/client.rs:30` and `protondb/client.rs:26` | Shared HTTP client with user-agent                                         | Use same pattern for protonup's `list_releases` HTTP client; see `http_client()` at `discovery/client.rs:79–94`                                                                       |
| Tauri event streaming (emit `prefix-dep-log` / `prefix-dep-complete`)              | `commands/prefix_deps.rs:27–323`                     | Progress streaming from background tasks to frontend via `AppHandle::emit` | Copy the `AppHandle` + `tauri::async_runtime::spawn` pattern for download progress events                                                                                             |
| `commands/install.rs` `spawn_blocking` pattern                                     | `commands/install.rs:10–37`                          | Wraps blocking core calls for Tauri async commands                         | Use `tauri::async_runtime::spawn_blocking` for any sync libprotonup calls                                                                                                             |
| `ProtonInstall` struct with `name`, `path`, `is_official`, `aliases`               | `steam/models.rs:70–82`                              | Typed representation of installed Proton tool                              | Extend or mirror for available (not-yet-installed) versions; keep `name` and `path` fields consistent                                                                                 |
| `CACHE_TTL_HOURS` / `CACHE_NAMESPACE` constants                                    | `discovery/client.rs:23–26`                          | Namespaced cache key scoping                                               | Use `"protonup:versions:v1:{tool_type}"` as cache key namespace                                                                                                                       |

---

## Modularity Design

### Recommended Module Boundary

Create `crosshook-core/src/protonup/` following the `protondb/` pattern (not `discovery/` which is more complex). Recommended submodules:

```
src/protonup/
├── mod.rs          — public re-exports only
├── scanner.rs      — wraps steam::proton::discover_compat_tools, filters to GE-Proton
├── fetcher.rs      — calls libprotonup::downloads::list_releases with cache→live→stale
├── installer.rs    — orchestrates download + extract via libprotonup, emits progress
└── types.rs        — AvailableProtonVersion, InstalledProtonVersion, InstallProgress, ProtonupError
```

### Shared vs. Feature-Specific

**Use from shared infrastructure (no duplication):**

- `MetadataStore` — already managed Tauri state; `protonup` module takes `&MetadataStore` like `protondb` does
- `is_network_available()` — call directly from `fetcher.rs`
- `http_client()` pattern — duplicate the `OnceLock<reqwest::Client>` singleton inside `fetcher.rs` (same as `protondb` did; three separate clients is fine given they have different timeout/config needs)
- `discover_compat_tools` + `discover_steam_root_candidates` — import and call directly from `scanner.rs`

**Feature-specific (new, scoped to `protonup/`):**

- `ProtonupError` enum — this module's specific error types
- `AvailableProtonVersion` struct — derived from libprotonup's `Release` but adapted for CrossHook serialization
- Install progress event payloads — only for this module

### Do NOT Extract

The fetcher's cache logic should mirror `discovery/client.rs:231–300` by copy, not by abstracting a shared "cache-first fetch" helper. The two call sites have meaningfully different payload shapes, cache namespaces, and error semantics. Three call sites (add `protondb`) would justify extraction; two is fine as-is.

---

## KISS Assessment

| Option                                            | Complexity | Coverage                                                                                            | Recommendation                                                             |
| ------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Wrap `libprotonup` APIs directly                  | Low        | High — handles HTTP, extraction, SHA2 verification, temp files                                      | **Preferred**                                                              |
| Roll custom GitHub API client + tarball extractor | High       | Complete, but redundant — `reqwest`, `flate2`, `tar` are already deps                               | Avoid: `libprotonup` already does this                                     |
| Shell out to `protonup-rs` CLI binary             | Medium     | Works but not guaranteed installed; binary detection + error surface identical to protontricks case | Avoid: user must install separately, breaks offline, no progress streaming |
| Use `libprotonup` for listing, custom for install | Medium     | Partial reuse but splits responsibility arbitrarily                                                 | Avoid: no benefit over full delegation                                     |

**Finding:** `libprotonup 0.11.0` is already in `Cargo.toml`. It provides `downloads::list_releases(VariantParameters)` (version listing), `downloads::download_to_async_write(...)` (streaming download with progress hooks), and extraction via `astral-tokio-tar` + `async-compression`. This covers the hardest parts of the feature. CrossHook only needs to build the coordination layer: cache management, progress event emission, and Tauri IPC surface.

**KISS risk to flag:** `libprotonup` is GPL-3.0, while CrossHook's license needs to be compatible. Verify license compatibility before shipping. This is the only meaningful concern with the dependency.

---

## Abstraction vs. Repetition

### Extract (warrant a shared helper)

None at this time. The codebase correctly applies the rule-of-three: only `metadata/cache_store.rs` is shared across modules, because every module uses it. Do not create a new abstraction for protonup-specific patterns.

### Repeat (copy the pattern, don't abstract)

- **Cache-first fetch pattern** — copy from `discovery/client.rs:231` into `protonup/fetcher.rs`. Different payload types and namespaces make abstraction premature.
- **OnceLock HTTP client** — copy the 15-line pattern from `discovery/client.rs:79–94`. Each module owns its timeout config.
- **Tauri event streaming** — copy from `commands/prefix_deps.rs:234–320`. The event names and payload shapes differ per feature; there is no type-safe way to abstract this without overengineering.

### Existing Overlap to Avoid Duplicating

- `installed` version listing: `steam::proton::discover_compat_tools` already exists and is already called by `commands/steam::list_proton_installs`. The new `protonup_list_installed` command should call `discover_compat_tools` directly rather than re-implementing filesystem scanning. The `protonup/scanner.rs` module should be a thin filter/transformer over the existing steam scanner, not a replacement.

---

## Interface Design

### Public `crosshook-core` API Surface (`protonup/mod.rs`)

```rust
pub use fetcher::list_available_versions;   // async, network+cache
pub use installer::install_version;         // async, emits progress
pub use scanner::list_installed_versions;   // sync, filesystem scan
pub use types::{AvailableProtonVersion, InstalledProtonVersion, InstallProgress, ProtonupError};
```

### Minimal Tauri IPC Commands (`commands/protonup.rs`)

Five commands cover the full feature:

1. `protonup_list_available(tool_type: String, force_refresh: Option<bool>)` → `Result<Vec<AvailableProtonVersion>, String>`
   - Calls `list_available_versions(&metadata_store, tool_type, force_refresh)`
   - Async; uses existing `MetadataStore` state

2. `protonup_list_installed(steam_client_install_path: Option<String>)` → `Result<Vec<InstalledProtonVersion>, String>`
   - Calls existing `discover_compat_tools` + filters; thin wrapper over `list_proton_installs` logic
   - Sync via `spawn_blocking`

3. `protonup_install_version(tool_type: String, version_tag: String, app: AppHandle)` → `Result<(), String>`
   - Calls `install_version(...)` which emits `"protonup-progress"` and `"protonup-complete"` events
   - Fire-and-return like `install_prefix_dependency`

4. `protonup_check_binary()` → `Result<ProtonupBinaryStatus, String>`
   - Detects whether `protonup-rs` CLI binary is present (for fallback guidance message only)
   - Sync

5. `protonup_get_community_required_versions(profile_names: Vec<String>)` → `Result<Vec<String>, String>`
   - Queries `community_profiles.proton_version` via `MetadataStore` for the auto-suggest feature
   - Sync

### Frontend Hook Pattern

Follow `useExternalTrainerSearch.ts` for data fetching and `usePrefixDeps.ts` for event-driven progress:

```typescript
// src/hooks/useProtonUpVersions.ts — list + install
// src/types/protonup.ts — AvailableProtonVersion, InstalledProtonVersion, InstallProgress
```

Do not add a new context provider. Wire `invoke` calls and `listen` for events directly in the hook, same pattern as `prefix_deps.ts`.

---

## Testability Patterns

### Rust — Existing Patterns to Follow

**In-memory MetadataStore:** `MetadataStore::open_in_memory()` is used in `metadata/mod.rs` test suite (lines 2375–2503). Every `protonup/` test that touches cache should use this. Zero disk I/O.

**Filesystem mocks via `tempfile`:** `install/service.rs:336–507` shows the established pattern — `tempdir()` provides isolation for filesystem scanning tests. Use for `scanner.rs` tests with synthetic `compatibilitytools.d` trees.

**Trait injection for network:** `protondb/client.rs` and `discovery/client.rs` both use internal functions taking `&reqwest::Client`. For testability in `fetcher.rs`, accept an optional `&reqwest::Client` in the internal fetch function so tests can substitute a mock server (e.g. `wiremock` or `httpmock`). The public function always uses the `OnceLock` singleton.

**No mockall needed:** The codebase does not use `mockall`. Follow the same approach — use trait-free dependency injection via function parameters or test-specific builder functions, not mock objects.

### Frontend

No test framework is configured (`CLAUDE.md`: "There is no configured frontend test framework"). Follow existing practice — no new test infrastructure needed for this feature.

---

## Build vs. Depend

| Capability                                     | Build Custom                                                  | Use `libprotonup`                                            | Verdict                             |
| ---------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------ | ----------------------------------- |
| List available GE-Proton versions (GitHub API) | Would need `reqwest` + JSON deserialization + pagination      | `downloads::list_releases(VariantParameters)` — covered      | Use `libprotonup`                   |
| Download tarball with progress                 | Would need streaming `reqwest` + progress tracking            | `downloads::download_to_async_write(...)` — covered          | Use `libprotonup`                   |
| Extract `.tar.gz` to `compatibilitytools.d`    | `flate2` + `tar` already in deps; ~50 lines                   | `libprotonup` wraps `astral-tokio-tar` + `async-compression` | Use `libprotonup`                   |
| SHA2 checksum verification                     | `sha2` already in deps; ~20 lines                             | `libprotonup::hashing` — covered                             | Use `libprotonup` (simpler)         |
| List installed Proton versions (filesystem)    | `steam::proton::discover_compat_tools` — **already built**    | Not in libprotonup scope                                     | **Use existing CrossHook code**     |
| TTL cache for version lists                    | `metadata::cache_store` — **already built**                   | Not in libprotonup scope                                     | **Use existing CrossHook code**     |
| TOML settings for preferred version            | `settings/mod.rs` struct field — trivial add                  | Not applicable                                               | **Add to existing settings struct** |
| Progress events to frontend                    | `AppHandle::emit` pattern — **already established**           | Not in libprotonup scope                                     | **Use existing CrossHook pattern**  |
| Community profile Proton version hints         | `metadata_store` query on `community_profiles.proton_version` | Not applicable                                               | **Use existing MetadataStore**      |

**Assessment:** libprotonup covers exactly the network/download/extract pipeline that CrossHook should not reimplement. Everything else reuses existing CrossHook infrastructure. This is the correct build-vs-depend split.

---

## Open Questions

1. **License compatibility:** `libprotonup` is GPL-3.0. Confirm CrossHook's license permits linking against GPL-3.0 code before implementation begins. If not compatible, the download/extract pipeline must be built from `reqwest` + `flate2` + `tar` (all MIT/Apache), which is feasible given existing deps.

2. **libprotonup API stability:** The crate is at 0.11.0 with 24 versions published; the API surface may change. Pin the version in `Cargo.toml` exactly (`= "0.11.0"` not `"0.11"`) to prevent surprise breakage. Document that upgrades require manual validation.

3. **Default install path:** GE-Proton installs to `~/.steam/root/compatibilitytools.d/` or `~/.local/share/Steam/compatibilitytools.d/`. `libprotonup` uses `dirs` for path resolution. Verify that its resolved path is consistent with what `discover_compat_tools` scans, so installed versions appear immediately in the installed list without a restart.

4. **GitHub API rate limiting:** `list_releases` hits the GitHub API unauthenticated. The 60 req/hour anonymous limit is unlikely to be hit in practice (the TTL cache prevents repeated fetches), but the cache TTL (suggest 6 hours, matching `protondb/client.rs:23`) should be documented as the mitigation.

5. **Install concurrency:** The `prefix_deps` module uses `PrefixDepsInstallLock` to prevent concurrent installs. ProtonUp installs should have a similar global install lock to prevent double-downloading. Reuse the `Arc<Mutex<Option<String>>>` pattern from `prefix_deps/lock.rs`.

6. **`protonup` module in `lib.rs`:** The `protonup` directory exists in `crosshook-core/src/` but is **not yet listed in `lib.rs`**. Whoever begins implementation must add `pub mod protonup;` to `crosshook-core/src/lib.rs:18`.
