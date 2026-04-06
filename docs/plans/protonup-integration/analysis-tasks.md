# ProtonUp Integration — Task Structure Analysis

## Executive Summary

The protonup-integration feature maps cleanly onto 19 discrete tasks across 3 phases. Phase 1 (backend foundation) has a wave structure with 4 parallelizable seed tasks in Wave 1A, followed by 3 parallel mid-wave tasks in 1B/1C that unblock the 2 heavy tasks in 1D, finishing with 1 integration task in 1E. Phase 2 (frontend) has 3 tasks fully parallelizable after the two hook tasks are seeded. Phase 3 (polish) tasks are independent of each other and can be picked up in any order. The GPL-3.0 license blocker must be resolved before any Phase 1 coding begins — it is a pre-Phase-1 gate, not a task within a phase.

**Current state**: `crosshook-core/src/protonup/` directory exists but is empty. `src-tauri/src/commands/protonup.rs` does not exist. No frontend protonup files exist. All implementation is greenfield within an established codebase.

---

## Recommended Phase Structure

### Pre-Phase Gate: License Resolution (not a coding task)

Before any Phase 1 coding begins, confirm whether `libprotonup`'s GPL-3.0 license is compatible with CrossHook's distribution model, or whether Option B (direct `reqwest`/`flate2`/`tar`) is required. This decision changes the `installer.rs` implementation entirely. **Block Phase 1 on this decision.**

- If GPL-3.0 is acceptable: use `libprotonup` APIs directly throughout.
- If Option B is chosen: `installer.rs` and `fetcher.rs` must implement their own HTTP + extraction pipeline using `reqwest`, `flate2`, `tar` (all MIT-clean, already in `Cargo.toml`).

All task estimates below assume Option A (libprotonup). Option B adds approximately 3-4 days of work to `installer.rs` and `fetcher.rs`.

---

### Phase 1: Backend Foundation

**Goal**: All 5 Tauri commands operational and testable via `cargo test`. Zero UI. Security mitigations in place.

**Wave structure**: 1A → (1B ∥ 1C ∥ 1D) → 1E

#### Wave 1A — Seed Tasks (all 4 fully parallel)

| Task                                              | Files Touched                             | Complexity               | Dependency |
| ------------------------------------------------- | ----------------------------------------- | ------------------------ | ---------- |
| 1A-1: Module declaration                          | `crosshook-core/src/lib.rs`               | Trivial (1 line)         | None       |
| 1A-2: Promote `normalize_alias`                   | `crosshook-core/src/steam/proton.rs`      | Trivial (1 token change) | None       |
| 1A-3: Implement `protonup/models.rs` + `error.rs` | `protonup/models.rs`, `protonup/error.rs` | Low (~80 lines)          | None       |
| 1A-4: Settings field addition                     | `crosshook-core/src/settings/mod.rs`      | Low (~5 lines)           | None       |

**Task 1A-1 detail**: Add `pub mod protonup;` to `crosshook-core/src/lib.rs`. Also add `pub mod protonup;` to `src-tauri/src/commands/mod.rs` in this same task (two files, both trivial). This seeds the module hierarchy so all downstream files can compile.

**Task 1A-2 detail**: Change `pub(crate)` to `pub` on `normalize_alias` at `steam/proton.rs:411`. One-token change but must land before `advisor.rs` can import it.

**Task 1A-3 detail**: The highest-priority 1A task. Define all Serde types: `AvailableProtonVersion`, `InstalledProtonVersion` (with `From<ProtonInstall>`), `ProtonVersionListCache`, `ProtonInstallProgress`, `ProtonVersionSuggestion`, `VersionChannel` enum, `InstallPhase` enum, `ProtonupError` enum. Pattern: copy `install/models.rs` structure — `#[derive(Debug, Clone, Serialize, Deserialize)]`, `#[serde(rename_all = "snake_case")]` on enums. Split into `models.rs` (data types) and `error.rs` (error enum with `.message()` and `Display`).

**Task 1A-4 detail**: Add `preferred_proton_version: String` with `#[serde(default, skip_serializing_if = "String::is_empty")]` to `AppSettingsData` in `settings/mod.rs`. Zero migration cost. Also update `AppSettingsIpcData` mirror struct with the same field.

#### Wave 1B — Cache Fetcher (unblocked by 1A-1 + 1A-3)

| Task                                  | Files Touched         | Complexity             | Dependency |
| ------------------------------------- | --------------------- | ---------------------- | ---------- |
| 1B-1: Implement `protonup/fetcher.rs` | `protonup/fetcher.rs` | Medium (~80-100 lines) | 1A-1, 1A-3 |

**Task 1B-1 detail**: Cache-first fetch from GitHub via `libprotonup::downloads::list_releases`. Pattern: copy `protondb/client.rs:85-130` (stale-fallback path) and `discovery/client.rs:231-300` (cache→live→stale three-stage). Key decisions: cache key `protonup:version_list:ge-proton`, 24h TTL, serve stale up to 7 days with age indicator; call `offline::network::is_network_available()` before attempting live fetch; use `OnceLock<reqwest::Client>` singleton; normalize raw `libprotonup::Release` → `ProtonVersionListCache` on write; validate `MAX_CACHE_PAYLOAD_BYTES` (512 KiB) — a single page of GE-Proton releases is ~50-150 KB so a single cache key per channel is sufficient.

#### Wave 1C — Scanner (unblocked by 1A-1 + 1A-3)

| Task                                  | Files Touched         | Complexity      | Dependency |
| ------------------------------------- | --------------------- | --------------- | ---------- |
| 1C-1: Implement `protonup/scanner.rs` | `protonup/scanner.rs` | Low (~30 lines) | 1A-1, 1A-3 |

**Task 1C-1 detail**: Thin wrapper over `steam::proton::discover_compat_tools`. Converts `Vec<ProtonInstall>` to `Vec<InstalledProtonVersion>` via the `From<ProtonInstall>` impl defined in `models.rs`. No new filesystem logic; this task is deliberately minimal. The existing `discover_compat_tools_with_roots` handles multi-library Steam roots automatically.

#### Wave 1D — Installer + Advisor (parallel with each other; blocked as noted)

| Task                                    | Files Touched           | Complexity            | Dependency       |
| --------------------------------------- | ----------------------- | --------------------- | ---------------- |
| 1D-1: Implement `protonup/installer.rs` | `protonup/installer.rs` | High (~150-200 lines) | 1A-1, 1A-3       |
| 1D-2: Implement `protonup/advisor.rs`   | `protonup/advisor.rs`   | Low (~60 lines)       | 1A-1, 1A-2, 1A-3 |

**Task 1D-1 detail** (most complex in Phase 1): Download, verify, and extract a Proton version. Orchestrates `libprotonup`'s download API. Pre-flight checks in order: (1) already-installing mutex guard, (2) already-installed idempotency check via scanner, (3) disk space check via `nix::sys::statvfs` (warn if < 2x tarball size — non-blocking), (4) install_dir path validation if user-supplied (must be within `$HOME`, must resolve to a `compatibilitytools.d/` path, must not be a symlink — mirror `metadata/db.rs:open_at_path` symlink detection). Emit `protonup-install-progress` events at each phase: `downloading` → `verifying` → `extracting` → `complete`. All 3 CRITICAL security mitigations are embedded here: C-1 (libprotonup pin already in Cargo.toml), C-2 (install_dir validation), C-3 (archive bomb cap: 8 GB extracted, 50,000 file entries). Atomic installation: extract to `.tmp` suffix, rename atomically on success. Pattern reference: `commands/prefix_deps.rs:234-320` for event emission; `metadata/db.rs:open_at_path` for symlink check.

**Task 1D-2 detail**: Fuzzy version matching for community profile suggestions. Uses `steam::proton::normalize_alias` (promoted in 1A-2) to match community profile `proton_version` strings against installed version names. Returns `ProtonVersionSuggestion` with `is_installed`, `closest_installed`, and `available_version`. Low complexity; pure logic with no filesystem or network I/O.

#### Wave 1E — Tauri Layer + `mod.rs` + Tests

| Task                           | Files Touched                                                | Complexity               | Dependency             |
| ------------------------------ | ------------------------------------------------------------ | ------------------------ | ---------------------- |
| 1E-1: Tauri command layer      | `src-tauri/src/commands/protonup.rs`, `src-tauri/src/lib.rs` | Medium (~100 lines)      | 1B-1, 1C-1, 1D-1, 1D-2 |
| 1E-2: Module root + unit tests | `protonup/mod.rs`, `protonup/tests.rs` (or inline)           | Medium (~80 lines tests) | 1B-1, 1C-1, 1D-1, 1D-2 |

**Task 1E-1 detail**: Create `src-tauri/src/commands/protonup.rs` with the 5 Tauri commands:

- `protonup_list_versions` — calls `fetcher::list_available_versions`
- `protonup_install_version` — calls `installer::install_version`, manages `ProtonupInstallState` mutex
- `protonup_cancel_install` — acquires `ProtonupInstallState` lock, calls `.abort()` on the stored `AbortHandle`
- `protonup_list_installed` — calls `scanner::list_installed_versions`
- `get_proton_install_progress` — reads current state from `ProtonupInstallState`

Also modify `src-tauri/src/lib.rs`: add `.manage(ProtonupInstallState::default())` and register all 5 commands in `invoke_handler!`. Pattern: copy `PrefixDepsInstallState` structure from `prefix_deps.rs` — it is the exact model for `ProtonupInstallState`.

**Task 1E-2 detail**: Write `protonup/mod.rs` (public re-exports only). Write unit tests: cache round-trip with `MetadataStore::open_in_memory`, stale-cache fallback when network unavailable, `From<ProtonInstall> for InstalledProtonVersion` conversion correctness, path-traversal guard rejects `..` and absolute paths outside home, idempotency (already-installed returns `AlreadyInstalled` variant), version string allowlist validation.

---

### Phase 2: Core UI

**Goal**: Functional Proton Version Manager visible in Settings panel. Users can browse, filter, and install versions with real-time progress.

**Dependencies**: All Phase 1 tasks complete (specifically: Tauri commands registered and returning correct types).

#### Wave 2A — Hooks (both can be written in parallel since they call different commands)

| Task                               | Files Touched                                             | Complexity          | Dependency       |
| ---------------------------------- | --------------------------------------------------------- | ------------------- | ---------------- |
| 2A-1: `useProtonVersions.ts`       | `src/hooks/useProtonVersions.ts`, `src/types/protonup.ts` | Medium (~100 lines) | Phase 1 complete |
| 2A-2: `useInstallProtonVersion.ts` | `src/hooks/useInstallProtonVersion.ts`                    | Medium (~130 lines) | Phase 1 complete |

**Task 2A-1 detail**: Wraps `protonup_list_versions` and `protonup_list_installed`. Exposes: `availableVersions`, `installedVersions`, `isLoading`, `error`, `isStale`, `cacheAge`, `isOffline`, `refresh()`. Pattern: copy `useProtonDbSuggestions.ts` shape for the data-fetching skeleton. Requires new TypeScript types in `src/types/protonup.ts`: `AvailableProtonVersion`, `InstalledProtonVersion`, `ProtonInstallProgress` mirroring Rust structs with `snake_case` keys.

**Task 2A-2 detail**: Manages install lifecycle. Listens to `protonup-install-progress` events before calling `protonup_install_version`, uses `unlistenRef` cleanup, implements stage machine (idle → downloading → verifying → extracting → complete/error), exposes `canInstall(versionTag)`, `isInstalling`, `currentPhase`, `progress`, `install(request)`, `cancel()`. Pattern: copy `useUpdateGame.ts` shape exactly — listen-before-invoke, unlistenRef, stage machine. Reconnect on mount by calling `get_proton_install_progress`.

#### Wave 2B — Component + Settings + Scroll Registration (all 3 parallel after 2A hooks)

| Task                                   | Files Touched                                   | Complexity               | Dependency |
| -------------------------------------- | ----------------------------------------------- | ------------------------ | ---------- |
| 2B-1: `ProtonVersionManager` component | `src/components/ProtonVersionManager.tsx` (new) | High (~300 lines)        | 2A-1, 2A-2 |
| 2B-2: Settings panel integration       | `src/components/SettingsPanel.tsx`              | Medium (~50 lines added) | 2A-1, 2A-2 |
| 2B-3: Scroll container registration    | `src/hooks/useScrollEnhance.ts`                 | Trivial (add selector)   | 2B-1       |

**Task 2B-1 detail**: New component following `PrefixDepsPanel.tsx` structural parallel. Two `CollapsibleSection` groups: Installed (filesystem, instant) and Available (cache/network, skeleton rows on load). Live text filter (debounced 200ms), sort dropdown via `ThemedSelect`, type filter chips (GE-Proton initially). Version row with inline progress bar during install: `Downloading… 45% (no resume support)` → `Verifying checksum…` → `Extracting…` → `Installed` chip. Cache-age banner following `crosshook-community-browser__cache-banner` pattern. Offline state: show cached list with age, disable install buttons with explanation. All rows keyboard-navigable; `aria-label` on Install/Cancel buttons; `role="progressbar"` on progress bar; `role="status"` on cache-age banner.

**Task 2B-2 detail**: Add `ProtonVersionManager` as a `CollapsibleSection` within `SettingsPanel.tsx`. Add preferred version selector dropdown bound to `settings.preferred_proton_version`. Add stale-preference warning when the preferred version is no longer installed. File is 49k — be surgical; add only the new section without touching existing settings sections.

**Task 2B-3 detail**: Add the `ProtonVersionManager`'s scrollable container selector to `SCROLLABLE` in `useScrollEnhance.ts`. **This is a ship-blocking requirement** — skipping it causes dual-scroll jank on WebKitGTK. Must be done whenever the component's DOM structure is finalized.

---

### Phase 3: Polish and Community Integration

**Goal**: Auto-suggest from community profiles, Wine-GE support, cleanup UI.

**Dependencies**: Phase 2 complete. All Phase 3 tasks are independent of each other.

| Task                                        | Files Touched                                                                       | Complexity | Dependency |
| ------------------------------------------- | ----------------------------------------------------------------------------------- | ---------- | ---------- |
| 3-1: Community profile auto-suggest         | `src/components/CommunityBrowser.tsx`, `src/hooks/useCommunityProfiles.ts` (modify) | Medium     | Phase 2    |
| 3-2: Post-install profile path suggestion   | `src/hooks/useProfile.ts` or `src/components/ProfileEditor` (modify)                | Low-Medium | Phase 2    |
| 3-3: Wine-GE support                        | `protonup/fetcher.rs`, `src/hooks/useProtonVersions.ts`, `ProtonVersionManager.tsx` | Medium     | Phase 2    |
| 3-4: Cleanup UI (orphan detection + delete) | New component or extension of `ProtonVersionManager.tsx`                            | Medium     | Phase 2    |

**Task 3-1 detail**: Cross-reference community profile `proton_version` field against installed versions via `suggest_proton_version_for_profile` Tauri command (wraps `advisor.rs`). Show `[Not Installed]` chip on community profile cards where `proton_version` is non-null and not present in installed list. Import wizard: show "Required Proton version: GE-Proton9-27 [Not Installed]" with optional install checkbox. Profile import never blocked.

**Task 3-3 detail**: Add `VersionChannel::WineGe` variant; add second cache key `protonup:version_list:wine-ge`; add channel toggle UI in `ProtonVersionManager`; update `fetcher.rs` to accept `VersionChannel` parameter and select appropriate `libprotonup::VariantParameters`.

---

## Task Granularity Recommendations

### Correct sizing

The 1-3 files per task rule is met throughout:

- Wave 1A tasks: 1-2 files each, all under 100 new lines
- Wave 1D-1 (installer.rs): 1 file, ~150-200 lines — this is the one task that pushes the complexity ceiling but cannot be split further without artificial seams (security mitigations and install logic are inseparable)
- Phase 2 component task (2B-1): 1 new file, ~300 lines — acceptable for a UI component; split into sub-components if it grows beyond 400 lines during implementation

### Anti-patterns to avoid

- **Do not split `installer.rs` security into a separate task.** The path-traversal guard and archive bomb cap are integral to the installer logic; they cannot be layered in after the fact without creating a window of insecure code.
- **Do not split `models.rs` and `error.rs` into separate tasks.** They are small, co-dependent, and trivially fast to implement together. Two tasks that each take 15 minutes are worse overhead than one 30-minute task.
- **Do not create a "wiring" task separate from the module implementations.** The `pub mod protonup;` declarations in `lib.rs` and `commands/mod.rs` should be done as the first task (1A-1) so that all downstream files can compile independently as they are written.

---

## Dependency Analysis

```
Pre-Phase Gate: License decision
        │
        ▼
[Wave 1A — fully parallel]
1A-1 (lib.rs mod decl)     1A-2 (normalize_alias)     1A-3 (models + error)     1A-4 (settings field)
        │                          │                          │
        └──────────────────────────┼──────────────────────────┘
                                   │
                    ┌──────────────┼──────────────┐
                    ▼              ▼               ▼
             1B-1 (fetcher)   1C-1 (scanner)  1D-1 (installer)
                    │              │               │
                    │         (also needs)         │
                    │          1A-2 ───────► 1D-2 (advisor)
                    │              │               │
                    └──────────────┴───────────────┘
                                   │
                    ┌──────────────┴──────────────┐
                    ▼                              ▼
             1E-1 (tauri layer)            1E-2 (mod.rs + tests)
                    │
                    ▼
[Phase 2 — all unblocked after Phase 1]
         2A-1 (useProtonVersions)  ∥  2A-2 (useInstallProtonVersion)
                    │                              │
                    └──────────────┬───────────────┘
                                   ▼
               2B-1 (component)  ∥  2B-2 (settings)  → 2B-3 (scroll)
                    │
                    ▼
[Phase 3 — all independent]
  3-1 (community suggest)  ∥  3-2 (path suggest)  ∥  3-3 (wine-ge)  ∥  3-4 (cleanup)
```

### Critical path

The longest sequential chain through Phase 1:

**1A-3 (models.rs) → 1D-1 (installer.rs) → 1E-1 (tauri layer)**

This chain cannot be parallelized. `installer.rs` is the bottleneck and should be assigned to the most experienced developer on the team.

### Blocker identification

| Blocker                  | What it blocks         | Resolution                              |
| ------------------------ | ---------------------- | --------------------------------------- |
| GPL-3.0 license decision | All of Phase 1         | Team decision before coding             |
| 1A-3 (models.rs)         | 1B-1, 1C-1, 1D-1, 1D-2 | Complete first among 1A tasks           |
| 1A-2 (normalize_alias)   | 1D-2 (advisor.rs)      | Fast change; land early                 |
| 1D-1 (installer.rs)      | 1E-1 (tauri layer)     | Longest single task; prioritize         |
| Phase 1 complete         | Phase 2 start          | Full wave 1E must be green              |
| 2A-1 + 2A-2 (hooks)      | 2B-1, 2B-2             | Hooks define the TypeScript API surface |

---

## File-to-Task Mapping

### New files (create)

| File                                       | Task | Notes                                         |
| ------------------------------------------ | ---- | --------------------------------------------- |
| `crosshook-core/src/protonup/models.rs`    | 1A-3 | All Serde types; `From<ProtonInstall>`        |
| `crosshook-core/src/protonup/error.rs`     | 1A-3 | `ProtonupError` enum; `.message()`; `Display` |
| `crosshook-core/src/protonup/fetcher.rs`   | 1B-1 | Cache-first GitHub fetch                      |
| `crosshook-core/src/protonup/scanner.rs`   | 1C-1 | Thin wrapper over `discover_compat_tools`     |
| `crosshook-core/src/protonup/installer.rs` | 1D-1 | Download + verify + extract + security        |
| `crosshook-core/src/protonup/advisor.rs`   | 1D-2 | Profile version fuzzy matching                |
| `crosshook-core/src/protonup/mod.rs`       | 1E-2 | Public re-exports only                        |
| `src-tauri/src/commands/protonup.rs`       | 1E-1 | 5 Tauri command handlers                      |
| `src/hooks/useProtonVersions.ts`           | 2A-1 | List hook with stale/offline state            |
| `src/hooks/useInstallProtonVersion.ts`     | 2A-2 | Install lifecycle hook                        |
| `src/types/protonup.ts`                    | 2A-1 | TS mirrors of Rust types                      |
| `src/components/ProtonVersionManager.tsx`  | 2B-1 | Primary UI component                          |

### Modified files

| File                                      | Task | Change                                                |
| ----------------------------------------- | ---- | ----------------------------------------------------- |
| `crosshook-core/src/lib.rs`               | 1A-1 | Add `pub mod protonup;`                               |
| `src-tauri/src/commands/mod.rs`           | 1A-1 | Add `pub mod protonup;`                               |
| `crosshook-core/src/steam/proton.rs`      | 1A-2 | `pub(crate)` → `pub` on `normalize_alias` at line 411 |
| `crosshook-core/src/settings/mod.rs`      | 1A-4 | Add `preferred_proton_version` field + IPC mirror     |
| `src-tauri/src/lib.rs`                    | 1E-1 | `.manage(ProtonupInstallState)` + register commands   |
| `src/components/SettingsPanel.tsx`        | 2B-2 | Add ProtonVersionManager section                      |
| `src/hooks/useScrollEnhance.ts`           | 2B-3 | Add ProtonVersionManager scroll selector              |
| `src/hooks/useCommunityProfiles.ts`       | 3-1  | Integrate version suggestion                          |
| `src/components/CommunityBrowser.tsx`     | 3-1  | Not-installed chip                                    |
| `crosshook-core/src/protonup/fetcher.rs`  | 3-3  | `VersionChannel` parameter                            |
| `src/hooks/useProtonVersions.ts`          | 3-3  | Channel toggle support                                |
| `src/components/ProtonVersionManager.tsx` | 3-3  | Channel toggle UI                                     |

---

## Optimization Opportunities

### Parallel execution within Phase 1

Wave 1A tasks are all 1-file, no-conflict changes. They can be assigned to 4 different developers and merged in any order. The only merge conflict risk is if two developers both touch `lib.rs` (task 1A-1 adds the `protonup` module, but no other 1A task touches `lib.rs`).

### Fast-path sequence for solo developer

If a single developer is implementing Phase 1:

1. 1A-1 (module declarations) — 10 minutes
2. 1A-2 (normalize_alias) — 5 minutes
3. 1A-3 (models.rs + error.rs) — 45 minutes
4. 1A-4 (settings field) — 15 minutes
5. 1B-1 (fetcher.rs) + 1C-1 (scanner.rs) — simultaneously (fetcher is async network, scanner is sync filesystem; no conflicts)
6. 1D-2 (advisor.rs) — 45 minutes
7. 1D-1 (installer.rs) — 2-3 hours (security mitigations + event emission)
8. 1E-1 + 1E-2 (tauri layer + tests) — 2 hours

Estimated Phase 1 solo duration: 7-9 hours of implementation time.

### Quick wins (confirmed)

These tasks are unusually fast relative to their value:

- **1A-2 (normalize_alias promotion)**: 1 token change, enables advisor.rs
- **1C-1 (scanner.rs)**: ~30 lines, reuses `discover_compat_tools` entirely
- **1D-2 (advisor.rs)**: ~60 lines of pure string matching logic
- **2B-3 (scroll registration)**: 1 line change in `useScrollEnhance.ts`, ship-blocking if missed

---

## Implementation Strategy Recommendations

### 1. Implement `protonup/mod.rs` as a stub first

Even though 1E-2 creates the final `mod.rs`, create a placeholder `mod.rs` in task 1A-1 so that `pub mod protonup;` in `lib.rs` compiles from the start. This lets all 1A-1D tasks compile independently during development without waiting for all siblings to complete. The placeholder is just:

```rust
// Populated as modules are implemented
```

### 2. Define `ProtonupInstallState` in `models.rs`, not in the Tauri layer

The state type should live in `crosshook-core` near the rest of the protonup types, with the Tauri `commands/protonup.rs` using it as `State<'_, ProtonupInstallState>`. This keeps the command layer thin. Pattern: see how `PrefixDepsInstallState` is defined (check `prefix_deps` module location).

### 3. Keep security validation in a dedicated `security.rs` or inline in `installer.rs`

The three security mitigations (C-1: libprotonup pin, C-2: install_dir validation, C-3: archive bomb cap) should be co-located with the installation code. Do not put them in a shared security module — they are specific to the installer's execution path. A `validate_install_request` function at the top of `installer.rs` that performs all pre-flight checks (matching the Validate-then-Execute pattern from `install/service.rs`) is the right structure.

### 4. Pin `libprotonup` version in `Cargo.toml` explicitly

The dependency should be `libprotonup = "=0.11.0"` (exact pin) rather than `"0.11.0"` (which allows patch upgrades). Given the CVE chain in `astral-tokio-tar`, an accidental upgrade to a `libprotonup` version that downgrades `astral-tokio-tar` would re-introduce vulnerabilities. Use an exact pin and upgrade intentionally with `cargo audit` verification.

### 5. Verify `Cargo.lock` after any dependency change

After any change to `Cargo.toml` or `Cargo.lock`, run:

```bash
grep "astral-tokio-tar" src/crosshook-native/Cargo.lock
# Expected: version = "0.6.x" — NOT 0.5.x
grep "name = \"tokio-tar\"" src/crosshook-native/Cargo.lock
# Expected: not found — abandoned crate must not appear
```

This is a ship-blocking verification step that belongs in the 1D-1 task definition.

### 6. Frontend TypeScript types go in a new `src/types/protonup.ts`

Do not add protonup types to the existing `src/types/proton.ts` (which only has `ProtonInstallOption`). Create a new `protonup.ts` to keep concerns separated and avoid conflicts with Phase 3 Wine-GE additions.

### 7. Test-drive the cache round-trip first

The most important unit test to write is the cache round-trip: write a `ProtonVersionListCache` to an in-memory `MetadataStore`, read it back, verify the `fetched_at` and TTL expiry logic. This test catches serialization bugs and cache key mismatches that would be painful to debug through the full Tauri stack.

---

## Relevant Reference Patterns (with exact locations)

| Pattern                                   | Reference Location                                   | Use In Task                 |
| ----------------------------------------- | ---------------------------------------------------- | --------------------------- |
| Cache-first fetch with stale fallback     | `protondb/client.rs:85-130`                          | 1B-1 (fetcher.rs)           |
| Three-stage cache→live→stale              | `discovery/client.rs:231-300`                        | 1B-1 (fetcher.rs)           |
| Event streaming with AppHandle::emit      | `commands/prefix_deps.rs:234-320`                    | 1D-1 (installer.rs), 1E-1   |
| Request/Result/Error triple               | `install/models.rs`                                  | 1A-3 (models.rs)            |
| Validate-then-Execute                     | `install/service.rs`                                 | 1D-1 (installer.rs)         |
| Symlink detection before filesystem write | `metadata/db.rs:open_at_path` (line ~15)             | 1D-1 (installer.rs)         |
| OnceLock HTTP client singleton            | `protondb/client.rs:26`                              | 1B-1 (fetcher.rs)           |
| Managed state with Mutex                  | `commands/prefix_deps.rs` (`PrefixDepsInstallState`) | 1E-1                        |
| Frontend listen-before-invoke             | `useUpdateGame.ts`                                   | 2A-2                        |
| Frontend install hook structure           | `hooks/usePrefixDeps.ts`                             | 2A-2 (backing hook pattern) |
| In-memory SQLite for tests                | `metadata/db.rs:53-61` (`open_in_memory`)            | 1E-2 (tests)                |
| Network availability probe                | `offline/network.rs:9` (`is_network_available`)      | 1B-1 (fetcher.rs)           |
