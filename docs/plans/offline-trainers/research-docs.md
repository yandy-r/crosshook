# Documentation Research: Offline Trainers

## Overview

CrossHook has comprehensive prior research for the offline-trainers feature across seven research axes (business, technical, UX, security, external, practices, recommendations) plus a synthesized feature spec. All documents are in `docs/plans/offline-trainers/`. The feature builds on an already ~80% offline-capable codebase — profiles are local TOML files, trainer binaries are local executables, and community taps clone to local Git repos. The `sha2` crate and `hash_trainer_file()` already exist. The remaining work is trainer type classification, hash caching, graceful degradation, and pre-flight scoring.

---

## Prior Feature Research Summary

### research-business.md — Business Analysis

**Key findings:**

- `trainer.kind` is a **free-form display string** that is never evaluated at runtime. All classification logic must be built from scratch via a new `trainer.trainer_type` field referencing a catalog entry.
- **Offline readiness** is a **0–100 weighted composite score** (not binary): trainer_present=30, hash_valid=15, game_present=20, proton_available=15, prefix_exists=10, network_not_required=10.
- **Score caps**: FLiNG/Standalone → 100; Aurora/WeMod/Unknown → 90 (inherent network dependency); PLITCH → 80.
- Launch posture for offline issues is **Warning, not Fatal** — following the `GamescopeNestedSession` pattern in `launch/request.rs`.
- **Aurora offline keys**: hardware-HWID-bound, 14-day expiry, **require Windows**, do not work on Steam Deck.
- `offline_activated` flag must be stored in **SQLite only** (machine-local), never in portable TOML.
- All path checks must use `effective_profile()` (resolves local overrides).
- `MetadataStore::disabled()` path must be handled — degrade gracefully.
- Community taps are offline-available if workspace directory exists (`~/.local/share/crosshook/community/taps/<slug>`).
- Tap staleness: warn if not synced in >30 days (using `community_taps.last_indexed_at`).

### research-technical.md — Technical Architecture Specification

**Key findings:**

- Proposes a new **`crosshook-core/src/offline/` module** with: `trainer_type.rs`, `readiness.rs`, `network.rs`, `hash.rs`.
- Three new SQLite tables in **migration 13**: `trainer_hash_cache`, `offline_readiness_snapshots`, `community_tap_cache`.
- New Tauri commands file: **`commands/offline.rs`** with `check_offline_readiness`, `verify_trainer_hash`, `check_network_status`, `get_trainer_hash_cache`, `batch_offline_readiness`, `confirm_offline_activation`.
- **Offline readiness state machine**: `unconfigured → hash_recorded → (offline_ready | awaiting_activation) → hash_stale`.
- `TrainerChanged` from `version_store::compute_correlation_status()` triggers `hash_stale` transition.
- Portable vs. machine-local data split: `trainer_type` in TOML (portable); activation state, readiness state, and hash in SQLite (machine-local).
- **Integration points**: `profile/models.rs` (+trainer_type), `profile/health.rs` (+offline checks), `launch/request.rs` (+OfflineReadinessInsufficient error), `community/taps.rs` (offline-aware sync), `settings/mod.rs` (+offline_mode toggle), `startup.rs` (initial readiness computation).

### research-ux.md — UX Research

**Key findings:**

- `OfflineReadinessBadge` on profile selector rows using existing `crosshook-status-chip` CSS pattern from `HealthBadge`.
- Platform-aware Aurora modal: Steam Deck → "ONLINE ONLY" notice; desktop Linux → step-by-step offline key setup guide.
- `isSteamDeck` boolean from `useGamepadNav`'s `GamepadNavState` drives platform variant.
- Pre-flight validation: `CollapsibleSection` on Launch page, collapsed if all pass, expanded if any fail.
- Inline hash verification: 300ms debounce, spinner next to trainer path field, SHA-256 chip on completion.
- **Heroic Games Launcher anti-patterns**: never show "game not installed" when offline; never empty the profile list from cache.
- All new modals need `data-crosshook-focus-root="modal"` for gamepad focus interception.
- Accessibility: 3-factor status signaling (icon + color + text), `aria-label` with full status context.
- **Optimistic UI**: load offline readiness from last cached health snapshot in SQLite — no live I/O on startup.

### research-security.md — Security Research

**Key findings:**

- **W-1**: Aurora/WeMod offline keys must be stored via OS keyring (`keyring` crate v3+), not TOML or SQLite plaintext. Steam Deck fallback: AES-256-GCM encrypted `secrets.db` with `chmod 600`.
- **W-2**: SQLite DB file defaults to 0644 — must call `fs::set_permissions(0o600)` immediately after creation in `metadata/db.rs`.
- **W-3**: Hash comparison at `version_store.rs:203` uses `!=` (timing oracle). Recommend `subtle::ConstantTimeEq` (defer per practices research — not a realistic threat for local process).
- **W-4**: FLiNG trainers are inherently untrusted Windows binaries; malware impersonators exist. Hash verification must be mandatory (not advisory) before launch.
- **W-5**: `offline_activated` must NOT be in portable TOML — Aurora activation is per-device.
- **A-6**: `git_command()` in `community/taps.rs` must add `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0` to prevent gitconfig injection.
- `trainer.offline_activated` state belongs in SQLite (`storage_profile()` boundary), never portable.

### research-external.md — External API Research

**Key findings:**

- **FLiNG**: No API, bot-detection blocks scraping. CrossHook approach: user provides local path, CrossHook caches SHA-256. No auto-download.
- **Aurora**: No API. Windows HWID-bound only. Lifetime PLUS required. **Steam Deck offline is impossible** (hard platform constraint confirmed officially).
- **WeMod**: Unofficial API at `api.wemod.com` — ToS prohibits programmatic access. "Offline mode" is session cache (~10-14 days), not a key system.
- **Connectivity detection**: `std::net::TcpStream::connect_timeout("8.8.8.8:53", 3s)` — no extra crate needed.
- **Git tap sync**: existing `std::process::Command` approach in `taps.rs` is correct — wrap `git fetch` non-zero exit as `SyncStatus::UsedCache` (graceful degradation).
- **Hash persistence**: `version_snapshots.trainer_file_hash` (migration 8→9) already exists — may reuse instead of new table.

### research-practices.md — Codebase Practices & Reuse

**Key findings:**

- `sha256_hex()` and `hash_trainer_file()` are already public in `crosshook-core::metadata`. `sha2 = "0.11.0"` is already in `Cargo.toml`. **Zero new dependencies needed**.
- `TrainerLoadingMode` (enum with `FromStr + as_str + serde + Default + tests`) is the exact pattern to follow for `TrainerType`/`OfflineCapability`.
- `check_system_readiness()` / `evaluate_checks()` in **`onboarding/readiness.rs`** is the closest existing pattern to offline pre-flight — outputs `ReadinessCheckResult { checks: Vec<HealthIssue>, all_passed: bool, ... }`.
- `batch_check_health()` should be called first; offline checks layer on top of profiles that pass base health.
- **`offline_activated` must NOT be in TOML** — use `storage_profile()` / `portable_profile()` boundary.
- `external_cache_entries` table can store tap offline metadata without a new table.
- `version_snapshots.trainer_file_hash` already exists — reuse instead of a new hash table.
- Offline module: extend existing modules (metadata, onboarding, launch) rather than creating new `offline/` top-level module for v1.
- **Proposed minimal v1 scope**: `TrainerKind` enum (20 LOC), `offline_readiness_score` (30 LOC), migration 13 + store (100 LOC), `is_tap_available_offline` (10 LOC), frontend wiring (20 LOC). ~180 LOC total, zero new dependencies.

### research-recommendations.md — Recommendations & Risk Assessment

**Key findings:**

- Phasing: Phase 1 (FLiNG + hash caching) → Phase 2 (launch integration) → Phase 3 (community + Aurora) → Phase 4 (UI polish).
- Next SQLite migration is **version 13**. No other in-flight features should claim it.
- **TrainerType enum (Option A)** recommended over capability model; aligns with `TrainerLoadingMode`, `CompatibilityRating` codebase patterns.
- Hash mismatch: warn and allow (not hard-block) — trainers get legitimately updated.
- Performance: hash computation must be `tokio::task::spawn_blocking()` to avoid blocking UI thread.
- **Risk**: breaking `trainer.kind` String → enum requires `#[default] Unknown` variant that deserializes from empty string.
- **Risk**: TypeScript types in `src/types/profile.ts` must mirror Rust changes.
- Parallelization map for implementation agents is documented.

### feature-spec.md — Synthesized Feature Specification

**Key findings:**

- **Trainer type system is data-driven via TOML catalog** (same architecture as optimization catalog `launch/catalog.rs`). Only `OfflineCapability` enum is compiled; vendor definitions live in `assets/default_trainer_type_catalog.toml`.
- Extended trainer ecosystem: `standalone` (FLiNG, Abolfazl.k, etc.), `cheat_engine` (CE tables), `aurora`, `wemod`, `plitch`, `unknown`.
- Catalog loads via embedded TOML → community tap contributions → user override file (same priority chain as optimization catalog).
- **Migration 13** tables: `trainer_hash_cache` (UUID PK, profile_id FK, file_path, file_size, file_modified_at, sha256_hash, verified_at), `offline_readiness_snapshots` (profile_id PK, readiness_score, trainer_type, trainer_present, trainer_hash_valid, blocking_reasons JSON, checked_at).
- Issue reference: **CrossHook #44** (offline-first trainer management) and related **#62** (network isolation), **#63** (hash verification).

---

## Architecture Documentation

### Key Architectural Docs

- **`CLAUDE.md`** (project root) — complete codebase map with module descriptions; the authoritative architectural reference
- **`docs/features/steam-proton-trainer-launch.doc.md`** — launch methods (steam_applaunch, proton_run, native), trainer staging, launcher export, console view
- **`docs/getting-started/quickstart.md`** — user-facing quickstart; covers profiles, launch modes, community profiles, health dashboard, CLI
- **`docs/internal-docs/local-build-publish.md`** — build and publish workflow

### Core Architecture Patterns Relevant to Offline Trainers

- **Tauri IPC**: All backend operations via `#[tauri::command]` → React `invoke()`. New `commands/offline.rs` will add 6 new commands.
- **TOML persistence**: Profiles in `~/.config/crosshook/*.toml`. `portable_profile()` vs `storage_profile()` distinction is critical — offline activation state is machine-local and must not cross this boundary.
- **SQLite metadata layer**: `metadata/` using `rusqlite`. Sequential migrations in `migrations.rs` (currently at v12). Migration 13 adds offline tables.
- **Health check pattern**: `profile/health.rs` → `check_profile_health()` / `batch_check_health()` → `HealthIssue` / `ProfileHealthReport`. Offline readiness is a superset/extension of this.
- **Onboarding readiness pattern**: `onboarding/readiness.rs` → `check_system_readiness()` / `evaluate_checks()` — the closest existing pattern to offline pre-flight check; returns `ReadinessCheckResult`.
- **Optimization catalog pattern**: `launch/catalog.rs` — data-driven TOML catalog with embedded default + user override. The trainer type catalog follows this exact pattern.

---

## API Documentation

### Existing IPC Commands (Relevant to Feature)

Defined in `src-tauri/src/commands/`:

- `commands/launch.rs` — `launch_game`, `launch_trainer`, `validate_launch` (will be extended with offline pre-flight)
- `commands/health.rs` — `check_profile_health`, `batch_check_health` (offline readiness check output feeds here)
- `commands/community.rs` — `sync_tap`, `get_community_profiles` (offline fallback needed)
- `commands/profile.rs` — `save_profile` (hash computation must be triggered here)

### New IPC Commands to Create (`commands/offline.rs`)

- `check_offline_readiness(profile_name: String) → OfflineReadinessScore`
- `verify_trainer_hash(trainer_path: String) → HashVerifyResult`
- `check_network_status() → bool`
- `get_trainer_hash_cache(profile_name: String) → Option<TrainerHashEntry>`
- `batch_offline_readiness() → Vec<ProfileOfflineStatus>`
- `confirm_offline_activation(profile_name: String) → Result<(), String>`

### Core Rust API Surface (crosshook-core)

**Already exists — reuse directly:**

- `metadata::version_store::hash_trainer_file(path: &Path) -> Option<String>` — SHA-256 file hash
- `metadata::profile_sync::sha256_hex(data: &[u8]) -> String` — generic SHA-256 utility
- `metadata::version_store::compute_correlation_status()` — detects `TrainerChanged` for hash staleness
- `profile::health::check_profile_health()` / `batch_check_health()` — foundation for pre-flight
- `onboarding::readiness::check_system_readiness()` — pattern for new offline readiness check
- `community::taps::CommunityTapStore::sync_tap()` / `sync_many()` — must fail gracefully offline
- `profile::models::GameProfile::effective_profile()` — MUST be used for all path resolution

**New API surface to create:**

```rust
// profile/models.rs or offline/trainer_type.rs
pub enum OfflineCapability { Full, FullWithRuntime, ConditionalKey, ConditionalSession, OnlineOnly, Unknown }

// profile/health.rs
pub fn check_offline_readiness(profile: &GameProfile, stored_hash: Option<&str>, tap_local_paths: &[PathBuf]) -> ReadinessCheckResult;

// metadata/offline_store.rs (or extend version_store.rs)
pub fn upsert_trainer_hash(conn: &Connection, profile_id: &str, path: &str, hash: &str, ...) -> Result<()>;
pub fn load_trainer_hash(conn: &Connection, profile_id: &str, path: &str) -> Result<Option<String>>;

// community/taps.rs
impl CommunityTapStore {
    pub fn is_tap_available_offline(&self, subscription: &CommunityTapSubscription) -> bool;
}
```

---

## Development Guides

### Build & Test

```bash
# Development server
./scripts/dev-native.sh

# Full AppImage build
./scripts/build-native.sh

# Run crosshook-core tests (relevant for offline feature)
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

### Testing Patterns for Offline Feature

- **In-memory SQLite**: `db::open_in_memory()` + `run_migrations()` for DB-backed tests
- **`tempfile::tempdir()`**: create mock trainer executables for hash tests
- **Pure functions**: `offline_readiness_score()` should be pure (no I/O inside) — I/O at callsite in Tauri command layer
- **Test cases to cover** (from practices research):
  - `offline_readiness_score` returns `score: 0` when trainer path empty
  - `offline_readiness_score` returns `hash_verified: false` when `stored_hash == None`
  - Upsert/load hash round-trips in in-memory DB
  - `is_tap_available_offline` returns `false` when workspace doesn't exist
  - `TrainerKind::from_str("fling")` and unknown string → `TrainerKind::Unknown`

---

## README Files

| File                                               | Content                                                                                                                           |
| -------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| `CLAUDE.md`                                        | Project overview, complete codebase architecture map, build commands, code conventions, commit message standards — **start here** |
| `docs/getting-started/quickstart.md`               | User-facing quickstart; launch modes, CLI usage, profiles, community                                                              |
| `docs/features/steam-proton-trainer-launch.doc.md` | Steam/Proton trainer launch deep-dive; trainer staging, launcher export, console view                                             |
| `docs/internal-docs/local-build-publish.md`        | Build and publish workflow internals                                                                                              |

---

## Must-Read Documents (Prioritized for Implementers)

### REQUIRED (read before writing any code)

1. **`docs/plans/offline-trainers/feature-spec.md`** — Synthesized spec with final decisions on data model, catalog architecture, SQLite tables, and API surface. This supersedes individual research files where they conflict.
2. **`docs/plans/offline-trainers/research-business.md`** — Business rules (BR-1 through BR-8), all edge cases, the domain model including state machine, and existing codebase integration points.
3. **`docs/plans/offline-trainers/research-technical.md`** — Architecture diagram, component diagram, new modules, integration points table, state machine diagram, portable vs. machine-local data split.
4. **`CLAUDE.md`** — Authoritative codebase architecture map; must understand before touching any module.

### REQUIRED for specific areas

5. **`docs/plans/offline-trainers/research-practices.md`** — What already exists and must be reused; where to place new code; v1 minimal scope (~180 LOC); addenda with final decisions on module placement.
6. **`docs/plans/offline-trainers/research-security.md`** — Security findings; W-1 (key storage), W-2 (DB permissions), W-4 (untrusted binaries), W-5 (activation flag portability), A-6 (git hardening). Several are MUST-FIX.
7. **`docs/plans/offline-trainers/research-recommendations.md`** — Implementation phasing, task breakdown, parallelization map, risk assessment, key decisions needed.

### NICE-TO-HAVE

8. **`docs/plans/offline-trainers/research-ux.md`** — Component designs, accessibility requirements, competitive anti-patterns (Heroic), platform-aware Aurora modal, error states table.
9. **`docs/plans/offline-trainers/research-external.md`** — FLiNG/Aurora/WeMod API constraints, library recommendations, code examples for hash verification and offline-safe tap sync.
10. **`docs/features/steam-proton-trainer-launch.doc.md`** — Launch method internals; useful for understanding the launch pipeline where offline guards will be injected.
11. **`docs/plans/offline-trainers/research-integration.md`** — Integration-focused report covering schema tables v1–v12, config paths, and external service constraints (produced by integration researcher).

---

## Integration Reference

### Config and Data Paths

| Resource       | Path                                              |
| -------------- | ------------------------------------------------- |
| TOML profiles  | `~/.config/crosshook/*.toml`                      |
| Settings       | `~/.config/crosshook/settings.toml`               |
| SQLite DB      | `~/.local/share/crosshook/metadata.db`            |
| Community taps | `~/.local/share/crosshook/community/taps/<slug>/` |
| Launch logs    | `~/.local/share/crosshook/logs/`                  |

> **Note**: The security research doc (`research-security.md`) shows the DB path as `~/.config/crosshook/metadata.db` — the integration researcher confirms the correct path is `~/.local/share/crosshook/metadata.db`. Verify via `paths.rs` in `src-tauri/src/` before writing migration code.

### Key Inline Code Documentation (Worth Reading Directly)

- `metadata/version_store.rs:178-211` — `compute_correlation_status` explains `UpdateInProgress` state flag semantics (Steam state_flags=4 = fully installed)
- `metadata/db.rs:68-136` — WAL mode setup and integrity check rationale
- `profile/models.rs:325-381` — `effective_profile()` / `storage_profile()` / `portable_profile()` doc comments explain the portable vs. machine-local path contract
- `community/taps.rs:421-466` — Security rationale for branch name and SHA validation (injection prevention)

---

## Documentation Gaps

1. **No inline code comments on `trainer.kind`**: The field at `profile/models.rs:TrainerSection` has no comment explaining that it's display-only and never evaluated. This caused confusion during research — implementers should add a `// Display-only; use trainer_type for offline capability classification` comment.

2. **No doc on `portable_profile()` vs `storage_profile()` boundary**: The distinction is critical for offline activation state but is only discoverable by reading `profile/models.rs`. A short note in `CLAUDE.md` or in the models file would prevent incorrect portability decisions.

3. **Migration 13 has no claimant yet**: The research confirms next migration is v13, but no document tracks which feature claims which migration number. Risk: two parallel features may both target v13.

4. **`onboarding/readiness.rs` not cross-referenced in feature plans**: The practices research found it late (addendum). It's the closest existing analog to the offline pre-flight check and should be listed as a primary reference in the feature spec.

5. **No test fixtures for legacy profile TOML**: Testing backward compatibility of `trainer.kind` (empty string → `TrainerKind::Unknown`) requires sample TOML fixtures. None exist yet.

6. **`research-technical.md` proposes a new `offline/` top-level module** while `research-practices.md` argues against it for v1. The feature-spec.md appears to follow the technical spec (showing `offline/` module), but the decision should be made explicit. The practices addendum is the more recent/reconciled view.
