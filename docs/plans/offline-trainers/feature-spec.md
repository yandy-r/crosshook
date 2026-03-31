# Feature Spec: Offline-First Trainer Management

## Executive Summary

CrossHook needs explicit offline-first trainer management for Steam Deck portable use (issue #44). The codebase is already ~80% offline-capable — profiles are local TOML files, trainer binaries are local executables, community taps clone to local Git repos, and `sha2`/`hash_trainer_file()` already exist. The trainer ecosystem spans multiple publishers with distinct offline profiles: **standalone .exe trainers** (FLiNG, Abolfazl.k, XiaoXing, MrAntiFun legacy) are fully offline; **app-based trainers** (Aurora, WeMod, PLITCH) have varying online requirements; and **Cheat Engine tables** need the CE runtime but no network. The remaining work is: (1) a `TrainerType` enum covering this spectrum, (2) SHA-256 hash caching with stat-based invalidation for offline integrity verification, (3) graceful degradation for network-dependent features, and (4) a pre-flight offline readiness scoring system integrated into the existing health check infrastructure. No new Rust crate dependencies are required.

## External Dependencies

### APIs and Services

#### FLiNG Trainers

- **Documentation**: [flingtrainer.com](https://flingtrainer.com/)
- **Authentication**: None — standalone Windows executables, no DRM, no phone-home
- **Distribution**: RAR/ZIP archives containing trainer `.exe` + readme, per-game-version specific
- **Offline Capability**: **Fully offline**. No network dependency at runtime once extracted.
- **CrossHook Integration**: User provides local path to extracted `.exe`. CrossHook caches SHA-256 hash for integrity verification. No API scraping or auto-download.
- **Constraints**: Bot detection on flingtrainer.com blocks automated access. Version-pinned executables require manual re-download when games update.

#### Aurora (CheatHappens)

- **Documentation**: [Offline Key Guide](https://cheathappens.zendesk.com/hc/en-us/articles/4451585703315) | [Key Request](https://cheathappens.zendesk.com/hc/en-us/articles/4408862962835)
- **Authentication**: In-app manual workflow only. No public API.
- **Offline Key Mechanism**:
  - Hardware-bound to Windows HWID (uses Windows registry keys)
  - Requires Lifetime PLUS membership (~$40 one-time)
  - One key per machine, 14-day expiry on downloaded trainers
  - **Does NOT work on Steam Deck or Linux** — HWID collection requires native Windows
- **CrossHook Integration**: Info modal with step-by-step instructions for desktop Linux users running Aurora via Proton. Steam Deck users see an "Online Only" notice. No automation possible.
- **Constraints**: No API endpoints. Key generation is entirely in-app and manual. Renewal requires internet.

#### WeMod

- **Documentation**: [Terms of Service](https://www.wemod.com/terms) | [ToS;DR](https://tosdr.org/en/service/2354)
- **Authentication**: Unofficial API at `api.wemod.com` — **ToS prohibits programmatic access**
- **Offline Capability**: Rolling session cache (10-14 days official, often ~24 hours in practice). Not a key-based system. Session invalidated on app close + restart while offline.
- **CrossHook Integration**: Display clear limitation notice. WeMod offline is user-managed. CrossHook must NOT automate WeMod API calls.
- **Constraints**: No public API, ToS prohibits automation, session expiry is unpredictable, Linux support runs through Proton.

#### PLITCH (MegaDev)

- **Documentation**: [plitch.com](https://www.plitch.com/en)
- **Authentication**: Account required, online verification every session
- **Offline Capability**: **Always-online**. No meaningful offline mode — internet required to verify cheats on each launch.
- **CrossHook Integration**: Classify as `OnlineOnly`. Show clear badge: "PLITCH requires internet — no offline support."
- **Linux/Proton**: No official support. Community Proton issue open ([ValveSoftware/Proton#7746](https://github.com/ValveSoftware/Proton/issues/7746)). Unsupported.

#### Other Standalone Trainers (Abolfazl.k, XiaoXing, MrAntiFun legacy, Razor1911)

- **Distribution**: Standalone `.exe` files in ZIP/7z/RAR archives, same as FLiNG
- **Offline Capability**: **Fully offline**. No DRM, no phone-home, no network dependency
- **CrossHook Integration**: Same as FLiNG — classify as `Standalone`. User provides local path, CrossHook caches hash.
- **Notable publishers**:
  - **Abolfazl.k**: Distributed via MegaGames and similar aggregators
  - **XiaoXing**: Primarily Chinese-language titles, limited English coverage
  - **MrAntiFun**: Legacy releases are standalone `.exe`; newer releases redirect to FLiNG's web loader (online dependency — classify those as `AppBased`)
  - **Razor1911**: Historical/archival only, no active release pipeline
- **Aggregator sites** (GameCopyWorld, MegaGames): Index trainers from the above publishers. The trainers themselves are standalone `.exe` files.

#### Cheat Engine Tables (FearlessRevolution, community)

- **Documentation**: [fearlessrevolution.com](https://fearlessrevolution.com/) | [Cheat Engine Wiki](https://wiki.cheatengine.org/)
- **Format**: `.ct` table files requiring Cheat Engine runtime (portable or installed)
- **Offline Capability**: **Fully offline** once Cheat Engine is present on disk. CE itself is open source (GPLv2).
- **CrossHook Integration**: Classify as `CheatEngine`. Offline readiness requires both the `.ct` file AND the Cheat Engine executable to be present. Pre-flight checks validate both paths.
- **Linux/Proton**: CE runs via Proton but is architecturally fragile — memory scanning through Proton is unreliable. Some tables work, many do not.

### Trainer Ecosystem Summary

| Classification                | Sources                                                    | Offline Capable                | Score Cap           |
| ----------------------------- | ---------------------------------------------------------- | ------------------------------ | ------------------- |
| **Standalone**                | FLiNG, Abolfazl.k, XiaoXing, MrAntiFun (legacy), Razor1911 | Fully offline                  | 100                 |
| **App-based (session cache)** | WeMod                                                      | ~10-14 day session cache       | 90                  |
| **App-based (offline key)**   | Aurora/CheatHappens                                        | 14-day HWID key (Windows only) | 90                  |
| **App-based (always-online)** | PLITCH                                                     | No offline support             | 80                  |
| **Runtime-dependent**         | Cheat Engine tables                                        | Offline if CE installed        | 100 (if CE present) |
| **Hybrid**                    | MrAntiFun (new releases)                                   | Depends on download path       | Varies              |

### Libraries and SDKs

| Library    | Version  | Purpose                     | Status                    |
| ---------- | -------- | --------------------------- | ------------------------- |
| `sha2`     | 0.11.0   | SHA-256 hash computation    | **Already in Cargo.toml** |
| `rusqlite` | 0.39.0   | SQLite metadata persistence | **Already in Cargo.toml** |
| `chrono`   | existing | Timestamp handling          | **Already in Cargo.toml** |
| `std::net` | stdlib   | Network connectivity probe  | **No dependency needed**  |

**No new crate dependencies required.** The `keyring` crate (v3+) is recommended as a future enhancement for secure offline key storage (see Security Considerations).

### External Documentation

- [RustCrypto/sha2](https://docs.rs/sha2): SHA-256 streaming hash API
- [Valve Steamworks Deck Guidelines](https://partner.steamgames.com/doc/steamdeck/recommendations): Controller UX requirements
- [CheatHappens Steam Deck Tool](https://www.cheathappens.com/steamdecktool.asp): Aurora's dedicated Deck tool (no offline key support)

## Business Requirements

### User Stories

**Primary User: Steam Deck Portable Gamer**

- As a Steam Deck user traveling, I want to launch a game with my FLiNG trainer without internet so I can play on a plane or train.
- As a Steam Deck user, I want CrossHook to show me which profiles are offline-ready **before** I lose connectivity.
- As a Steam Deck user, I want community tap profiles to remain browseable from local cache when offline.

**Secondary User: Linux Desktop Gamer**

- As a Linux desktop gamer with Aurora, I want CrossHook to guide me through offline key setup while I have internet, so I can use the trainer offline later.
- As a Linux gamer on a metered connection, I want CrossHook to skip network operations gracefully rather than hanging or crashing.

### Business Rules

1. **Trainer Type Classification (BR-1)**: Trainer types are **data-driven via a TOML catalog**, following the same pattern as the optimization catalog (#129/`launch/catalog.rs`). A default catalog ships with the app defining known trainer types and their offline capabilities. Users can add custom trainer types via a config directory override file. The existing `trainer.kind` free-form string continues to hold the vendor display name (e.g., "FLiNG", "Abolfazl.k"). A new `trainer.trainer_type` string field references a catalog entry by `id`.

   **Default catalog entries** (shipped in `assets/default_trainer_type_catalog.toml`):

   | id             | offline_capability    | score_cap | label                 | examples                                                   |
   | -------------- | --------------------- | --------- | --------------------- | ---------------------------------------------------------- |
   | `standalone`   | `full`                | 100       | Standalone Trainer    | FLiNG, Abolfazl.k, XiaoXing, MrAntiFun (legacy), Razor1911 |
   | `cheat_engine` | `full_with_runtime`   | 100       | Cheat Engine Table    | FearlessRevolution `.ct` tables (requires CE runtime)      |
   | `aurora`       | `conditional_key`     | 90        | Aurora (CheatHappens) | 14-day HWID key, Windows-only offline                      |
   | `wemod`        | `conditional_session` | 90        | WeMod                 | Session-cache ~10-14 days                                  |
   | `plitch`       | `online_only`         | 80        | PLITCH (MegaDev)      | Always-online, no offline support                          |
   | `unknown`      | `unknown`             | 90        | Unknown               | Default for unclassified profiles                          |

   **Offline capability enum** (the only compiled enum — classifies behavior, not vendors):
   - `full` — fully offline, no network dependency
   - `full_with_runtime` — offline if required runtime (e.g., Cheat Engine) is present
   - `conditional_key` — offline with time-limited activation key
   - `conditional_session` — offline via cached session, user-managed
   - `online_only` — requires internet every session
   - `unknown` — capability not determined

   Adding a new trainer source (e.g., a future "TrainerX") requires only adding a TOML entry — no code change, no recompilation, no release. Community taps can also contribute trainer type definitions alongside profiles.

2. **Offline Readiness Scoring (BR-2)**: Weighted 0-100 composite score per profile:

   | Factor                 | Weight | Condition                           |
   | ---------------------- | ------ | ----------------------------------- |
   | `trainer_present`      | 30     | Trainer file exists on disk         |
   | `hash_valid`           | 15     | SHA-256 hash cached and matches     |
   | `game_present`         | 20     | Game executable exists              |
   | `proton_available`     | 15     | Proton path exists                  |
   | `prefix_exists`        | 10     | WINEPREFIX directory exists         |
   | `network_not_required` | 10     | Trainer type is FLiNG or Standalone |

   Aurora/WeMod/Custom/Unknown profiles cap at 90. PLITCH caps at 80 (always-online). `Standalone` and `CheatEngine` (with CE present) can reach 100. Score informs but does **not** block launch (Warning severity, not Fatal).

3. **Hash Cache (BR-3)**: SHA-256 computed on profile save, stored in `version_snapshots.trainer_file_hash`. Stat-based fast path (file size + mtime) avoids re-reading on subsequent checks. Cache invalidated when trainer path changes.

4. **Community Tap Offline (BR-4)**: Taps are offline-available if local workspace directory exists. Stale after 30 days without sync (warning, not blocking). `community_taps.last_indexed_at` already tracks sync timestamps.

5. **Graceful Degradation (BR-5)**: Network failure **never** blocks profile load or launch. Community tap sync fails silently with informational message. All reads from SQLite/TOML remain fully functional offline.

6. **Aurora/WeMod Offline Activation (BR-7)**: Activation is manual (user completes in trainer UI). CrossHook tracks `offline_activated` flag (stored in SQLite, **not** in portable TOML), `offline_key_activated_at` timestamp, and enforces 14-day expiry locally. 3-day pre-expiry warning. Expired keys produce Warning, not Fatal.

7. **Pre-Flight Check (BR-8)**: Batch validation of all profiles producing "N of M offline-ready" summary. Informational only — does not prevent launch.

### Edge Cases

| Scenario                                | Expected Behavior                                | Notes                                  |
| --------------------------------------- | ------------------------------------------------ | -------------------------------------- |
| Trainer path changed after hash cached  | Re-compute hash on profile save                  | Auto-invalidation                      |
| Trainer binary replaced (same path)     | `TrainerChanged` status via mtime check          | Warning at next launch                 |
| Community profile install while offline | Fail with "Network required" message             | Browse allowed, install blocked        |
| No trainer configured                   | Skip all trainer checks, game-only profile       | Fully offline-capable                  |
| First launch, no version snapshot       | Prompt: "Launch once online to capture baseline" | Cannot verify without initial hash     |
| Aurora profile imported on new device   | `offline_activated` cleared on import            | Hardware-bound keys don't transfer     |
| `MetadataStore` disabled                | Degrade gracefully, hash_valid = 0               | Score components with missing data = 0 |

### Success Criteria

- [ ] FLiNG profiles with all paths present launch without any network connection
- [ ] Aurora/WeMod profiles show offline readiness warning (score capped at 90) with activation guide
- [ ] Community taps browseable from cache when offline; install attempts fail clearly
- [ ] Tap sync failure at startup does not block profile load or launch
- [ ] Pre-flight accurately identifies offline-ready profiles before connectivity loss
- [ ] Trainer hash computed and stored on save; stale hash detected and warned
- [ ] No features silently fail due to missing network (issue #44 acceptance criteria)

## Technical Specifications

### Architecture Overview

```
Frontend (React/TypeScript)
  OfflineStatusBadge ─── TrainerTypeSelect ─── OfflineReadinessPanel
            │                    │                       │
            └────────────────────┴───────────────────────┘
                                 │ invoke()
Tauri IPC Layer
  commands/offline.rs (NEW)     commands/launch.rs (MODIFIED)
  - check_offline_readiness     - launch_game (add pre-flight)
  - verify_trainer_hash         - launch_trainer (add gate)
  - check_network_status
  - batch_offline_readiness

crosshook-core Library
  profile/models.rs (EXTENDED)   metadata/ (EXTENDED)        launch/ (MODIFIED)
  - TrainerType enum             - offline_store.rs (NEW)    - request.rs (new errors)
  - TrainerSection +type         - migrations.rs (+v13)      - script_runner.rs (guards)

  profile/health.rs (EXTENDED)   community/taps.rs (EXTENDED)
  - offline readiness checks     - offline-aware sync fallback

SQLite (metadata.db)
  trainer_hash_cache ── offline_readiness_snapshots ── community_tap_offline_state
```

### Data Models

#### Trainer Type Catalog (Data-Driven)

The trainer type system uses a **data-driven TOML catalog** — the same architecture as the optimization catalog (`launch/catalog.rs`). Only the offline capability classification is a compiled enum; vendor definitions live in TOML.

**`OfflineCapability` Enum (Rust — the only compiled enum):**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OfflineCapability {
    /// Fully offline — no network dependency at runtime
    Full,
    /// Offline if required runtime (e.g., Cheat Engine) is present on disk
    FullWithRuntime,
    /// Offline with time-limited activation key (e.g., Aurora 14-day HWID key)
    ConditionalKey,
    /// Offline via cached session, user-managed (e.g., WeMod ~10-14 days)
    ConditionalSession,
    /// Always requires internet — no offline support (e.g., PLITCH)
    OnlineOnly,
    #[default]
    /// Capability not determined
    Unknown,
}
```

**`TrainerTypeEntry` (TOML catalog entry):**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerTypeEntry {
    pub id: String,                          // e.g., "standalone", "aurora", "wemod"
    pub label: String,                       // Display name: "Standalone Trainer"
    pub offline_capability: OfflineCapability,
    pub score_cap: u8,                       // Max offline readiness score (0-100)
    #[serde(default)]
    pub requires_runtime: String,            // Optional: path field name for required runtime (e.g., "cheat_engine_path")
    #[serde(default)]
    pub activation_expiry_days: Option<u16>, // Optional: key/session expiry in days (e.g., 14 for Aurora)
    #[serde(default)]
    pub platform_restriction: String,        // Optional: "windows_only" for Aurora HWID binding
    #[serde(default)]
    pub description: String,                 // Help text for UI tooltip
    #[serde(default)]
    pub info_modal: String,                  // Optional: ID of instructional modal to show
    #[serde(default)]
    pub community: bool,                     // Whether contributed by a community tap
}
```

**Default catalog** shipped at `assets/default_trainer_type_catalog.toml`:

```toml
catalog_version = 1

[[trainer_type]]
id = "standalone"
label = "Standalone Trainer"
offline_capability = "full"
score_cap = 100
description = "Standalone .exe trainers (FLiNG, Abolfazl.k, XiaoXing, MrAntiFun, etc.). Fully offline-capable."

[[trainer_type]]
id = "cheat_engine"
label = "Cheat Engine Table"
offline_capability = "full_with_runtime"
score_cap = 100
requires_runtime = "cheat_engine_path"
description = "Cheat Engine .ct table files. Offline if Cheat Engine is installed."

[[trainer_type]]
id = "aurora"
label = "Aurora (CheatHappens)"
offline_capability = "conditional_key"
score_cap = 90
activation_expiry_days = 14
platform_restriction = "windows_only"
info_modal = "aurora_offline_setup"
description = "Aurora trainers require a 14-day HWID-bound offline key. Does not work on Steam Deck."

[[trainer_type]]
id = "wemod"
label = "WeMod"
offline_capability = "conditional_session"
score_cap = 90
activation_expiry_days = 14
info_modal = "wemod_offline_info"
description = "WeMod caches sessions for ~10-14 days. Offline mode is user-managed."

[[trainer_type]]
id = "plitch"
label = "PLITCH (MegaDev)"
offline_capability = "online_only"
score_cap = 80
description = "PLITCH requires internet for every session. No offline support."

[[trainer_type]]
id = "unknown"
label = "Unknown"
offline_capability = "unknown"
score_cap = 90
description = "Trainer type not classified. Offline capability unknown."
```

**Profile TOML integration:** `trainer.trainer_type` is a string referencing a catalog entry `id`. Defaults to `"unknown"` for backward compatibility. `trainer.kind` (the existing free-form string) remains for vendor display name.

**Catalog loading** follows the optimization catalog pattern:

1. Embedded default TOML (`include_str!`)
2. Community tap contributions (merged)
3. User override file at `config_dir/trainer_type_catalog.toml` (highest priority)
4. Loaded into a `TrainerTypeCatalog` at startup via `OnceLock`

#### `trainer_hash_cache` Table (Migration 13)

| Field            | Type    | Constraints            | Description                           |
| ---------------- | ------- | ---------------------- | ------------------------------------- |
| cache_id         | TEXT    | PK                     | UUID primary key                      |
| profile_id       | TEXT    | FK → profiles, CASCADE | Profile reference                     |
| file_path        | TEXT    | NOT NULL               | Trainer executable path               |
| file_size        | INTEGER | NOT NULL               | File size for stat-based fast path    |
| file_modified_at | TEXT    | NOT NULL               | ISO 8601 mtime for cache invalidation |
| sha256_hash      | TEXT    | NOT NULL               | 64-char lowercase hex                 |
| verified_at      | TEXT    | NOT NULL               | Last verification timestamp           |

**Indexes:** `UNIQUE(profile_id, file_path)`, `idx_verified_at`

#### `offline_readiness_snapshots` Table (Migration 13)

| Field              | Type    | Constraints                 | Description           |
| ------------------ | ------- | --------------------------- | --------------------- |
| profile_id         | TEXT    | PK, FK → profiles, CASCADE  | One row per profile   |
| readiness_score    | INTEGER | NOT NULL                    | 0-100 composite       |
| trainer_type       | TEXT    | NOT NULL, DEFAULT 'unknown' | Cached trainer type   |
| trainer_present    | INTEGER | NOT NULL                    | Boolean (0/1)         |
| trainer_hash_valid | INTEGER | NOT NULL                    | Boolean               |
| blocking_reasons   | TEXT    |                             | JSON array of strings |
| checked_at         | TEXT    | NOT NULL                    | ISO 8601              |

#### `community_tap_offline_state` Table (Migration 13)

Companion table to `community_taps` (avoids ALTER TABLE):

| Field                | Type    | Constraints                      | Description                     |
| -------------------- | ------- | -------------------------------- | ------------------------------- |
| tap_id               | TEXT    | PK, FK → community_taps, CASCADE | Tap reference                   |
| cache_status         | TEXT    | NOT NULL, DEFAULT 'unknown'      | cached/stale/missing            |
| cached_at            | TEXT    |                                  | Last successful cache timestamp |
| cached_profile_count | INTEGER | NOT NULL, DEFAULT 0              | Profiles in local cache         |

#### TypeScript Types

```typescript
// types/offline.ts
export type OfflineCapability =
  | 'full'
  | 'full_with_runtime'
  | 'conditional_key'
  | 'conditional_session'
  | 'online_only'
  | 'unknown';

export interface TrainerTypeEntry {
  id: string; // Catalog entry key (e.g., "standalone", "aurora")
  label: string; // Display name
  offline_capability: OfflineCapability;
  score_cap: number; // Max offline readiness score
  requires_runtime?: string; // Optional runtime path field
  activation_expiry_days?: number; // Key/session expiry
  platform_restriction?: string; // e.g., "windows_only"
  description: string;
  info_modal?: string; // Instructional modal ID
  community: boolean;
}

export interface OfflineReadinessReport {
  profile_name: string;
  readiness_score: number; // 0-100
  trainer_type: TrainerType;
  checks: OfflineReadinessChecks;
  blocking_reasons: string[];
  checked_at: string;
}

export interface OfflineReadinessChecks {
  trainer_present: boolean;
  trainer_hash_valid: boolean;
  game_files_present: boolean;
  proton_available: boolean;
  prefix_exists: boolean;
  network_required: boolean;
}
```

### API Design

#### `check_offline_readiness`

**Purpose**: Compute offline readiness for a single profile.

**Request:** `{ name: string }`
**Response (200):** `OfflineReadinessReport`
**Errors:** `"profile not found"`, `"failed to compute readiness"`

#### `batch_offline_readiness`

**Purpose**: Compute offline readiness for all profiles (health dashboard).

**Request:** (none)
**Response (200):** `Vec<OfflineReadinessReport>`

#### `verify_trainer_hash`

**Purpose**: Compute and cache SHA-256 hash for a trainer executable.

**Request:** `{ profile_name: string, trainer_path: string }`
**Response (200):** `{ sha256: string, file_size: number, cached: boolean, verified_at: string }`
**Errors:** `"trainer file not found"`, `"hash computation failed"`

#### `check_network_status`

**Purpose**: Probe network connectivity.

**Request:** (none)
**Response (200):** `{ connected: boolean, method: string, checked_at: string }`

### System Integration

#### Files to Create

| Path                                                  | Purpose                                                            |
| ----------------------------------------------------- | ------------------------------------------------------------------ |
| `crates/crosshook-core/src/offline/mod.rs`            | Module root, re-exports                                            |
| `crates/crosshook-core/src/offline/trainer_type.rs`   | `OfflineCapability` enum, `TrainerTypeEntry`, `TrainerTypeCatalog` |
| `assets/default_trainer_type_catalog.toml`            | Default trainer type catalog (data-driven, shipped with AppImage)  |
| `crates/crosshook-core/src/offline/readiness.rs`      | Offline readiness scoring                                          |
| `crates/crosshook-core/src/offline/network.rs`        | Network connectivity probe                                         |
| `crates/crosshook-core/src/offline/hash.rs`           | Streaming SHA-256 with cache                                       |
| `crates/crosshook-core/src/metadata/offline_store.rs` | SQLite CRUD for offline tables                                     |
| `src-tauri/src/commands/offline.rs`                   | Tauri IPC handlers                                                 |
| `src/types/offline.ts`                                | TypeScript types                                                   |
| `src/components/OfflineStatusBadge.tsx`               | Readiness badge                                                    |
| `src/components/OfflineReadinessPanel.tsx`            | Detail panel                                                       |
| `src/components/OfflineTrainerInfoModal.tsx`          | Aurora/WeMod instructional modal                                   |
| `src/hooks/useOfflineReadiness.ts`                    | Frontend offline state hook                                        |

#### Files to Modify

| Path                                               | Change                                                                    |
| -------------------------------------------------- | ------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs`                 | Add `pub mod offline;`                                                    |
| `crates/crosshook-core/src/profile/models.rs`      | Add `trainer_type: String` field to `TrainerSection` (references catalog) |
| `crates/crosshook-core/src/profile/health.rs`      | Inject offline readiness into health checks                               |
| `crates/crosshook-core/src/metadata/migrations.rs` | Add `migrate_12_to_13()`                                                  |
| `crates/crosshook-core/src/metadata/mod.rs`        | Register `offline_store`                                                  |
| `crates/crosshook-core/src/launch/request.rs`      | Add `OfflineReadinessInsufficient` validation variant                     |
| `crates/crosshook-core/src/community/taps.rs`      | Offline-aware sync fallback + git hardening                               |
| `crates/crosshook-core/src/settings/mod.rs`        | Add `offline_mode: bool` to `AppSettingsData`                             |
| `src-tauri/src/commands/mod.rs`                    | Register offline command module                                           |
| `src-tauri/src/lib.rs`                             | Register IPC commands                                                     |
| `src/types/index.ts`                               | Re-export offline types                                                   |
| `src/types/profile.ts`                             | Add `trainer_type` to profile types                                       |
| `src/components/pages/LaunchPage.tsx`              | Pre-flight offline check                                                  |
| `src/components/pages/ProfilesPage.tsx`            | Trainer type selector                                                     |
| `src/components/pages/HealthDashboardPage.tsx`     | Offline readiness column                                                  |
| `src/components/ProfileFormSections.tsx`           | Trainer type dropdown                                                     |
| `src/components/CommunityBrowser.tsx`              | Cache status display                                                      |
| `src/hooks/useLaunchState.ts`                      | Offline pre-flight gate                                                   |

## UX Considerations

### User Workflows

#### Primary Workflow: Offline Launch (FLiNG)

1. **Open CrossHook** — launches from local state, no network required
2. **Navigate to Launch** — profile selector shows `OFFLINE READY` badge (green pill)
3. **Select profile** — card shows: trainer present (green check), hash verified (green check), FLiNG badge
4. **Press Launch** — pre-flight panel expands showing all checks passing
5. **Launch proceeds** — no network calls in the path

#### Aurora Offline Key Setup (Desktop Linux Only)

1. **Open profile** with `trainer_type = Aurora`
2. **See amber badge**: `AURORA` with tooltip "Requires offline key for offline use"
3. **Click "Set up offline key"** — instructional modal opens with step-by-step guide:
   - Step 1: Open Aurora in your Proton prefix
   - Step 2: Avatar menu → Offline Key tab → Generate key
   - Step 3: Open each trainer from Favorites to download offline copy
   - Step 4: Note 14-day expiry — launch online to refresh
4. **Mark as configured** — toggle persists in SQLite (not TOML)
5. **Profile shows** `AURORA (Offline Ready)` badge with expiry countdown

**Steam Deck**: Aurora badge shows `ONLINE ONLY` (red) with message: "Aurora does not support offline mode on Steam Deck." The "Set up offline key" CTA does not appear. Uses `isSteamDeck` from `useGamepadNav`.

#### PLITCH Profile (Always-Online Notice)

1. **Open profile** with `trainer_type = Plitch`
2. **See red badge**: `PLITCH (ONLINE ONLY)` — "PLITCH requires internet for every session"
3. **Offline readiness score capped at 80** regardless of file presence
4. **No offline setup flow** — no modal, just a persistent informational badge
5. **If offline**: Pre-flight shows clear message: "PLITCH trainers cannot be used without internet."

#### Cheat Engine Profile (Runtime-Dependent)

1. **Open profile** with `trainer_type = CheatEngine`
2. **Pre-flight checks**: Both the `.ct` table file AND the Cheat Engine executable path must be valid
3. **If both present**: Green badge `CE TABLE (OFFLINE READY)` — score can reach 100
4. **If CE missing**: Amber badge with recovery action "Locate Cheat Engine executable"

#### Error Recovery: Pre-Flight Failure

1. **Select profile** — badge shows `NOT READY` (red)
2. **Press Launch** — pre-flight expands showing: Trainer file: MISSING (red X, path shown)
3. **Inline action**: "Locate file" (opens file picker)
4. **After relocation** — pre-flight re-runs, shows green, launch enabled

### UI Patterns

| Component            | Pattern                                               | Notes                                                              |
| -------------------- | ----------------------------------------------------- | ------------------------------------------------------------------ |
| `OfflineStatusBadge` | `crosshook-status-chip` reuse from `HealthBadge`      | Green/amber/red pill, aria-label with full status                  |
| `TrainerTypeBadge`   | Pill badge in profile form                            | STANDALONE/CE=green, AURORA/WEMOD=amber, PLITCH=red, UNKNOWN=muted |
| Pre-flight panel     | `CollapsibleSection`                                  | Collapsed if all pass, expanded if any fail                        |
| Aurora info modal    | `role="dialog"` + `data-crosshook-focus-root="modal"` | Gamepad-accessible, step-by-step                                   |
| Community offline    | Inline info banner (blue/accent)                      | "Showing cached profiles from [date]"                              |

### Accessibility Requirements

- All badges: `role="button"`, `tabIndex={0}`, `aria-label` with full status description
- Modal: `role="dialog"`, `aria-modal="true"`, `aria-labelledby`
- Touch targets: >=48px (56px in controller mode)
- Three-factor status: color + icon + text (WCAG 2.1 AA compliant)

### Performance UX

- **Loading**: Offline readiness computed from cached SQLite snapshots at startup (instant). Fresh validation runs in background.
- **Hash computation**: 50-500ms for typical trainers (5-50MB). Inline spinner next to trainer path field, not full-page overlay.
- **Pre-flight**: All checks concurrent. Per-check results stream in. Target: <200ms for stat-based checks.

## Recommendations

### Implementation Approach

**Recommended Strategy**: Evolutionary extension of existing modules, phased rollout starting with FLiNG (highest value, lowest complexity).

**Phasing:**

1. **Phase 1 — Foundation** (3-4 days): TrainerType enum, hash caching, offline readiness scoring
2. **Phase 2 — Launch Integration** (2-3 days): Pre-launch hash verification, offline-aware validation, launch history preservation
3. **Phase 3 — Community & Aurora** (3-4 days): Tap offline caching, Aurora/WeMod info modal, community readiness integration
4. **Phase 4 — UI Polish** (2-3 days): Status badges, graceful degradation, pre-flight dashboard

**Total**: 10-14 days, compressible to 5-7 days with 2-3 parallel agents.

### Technology Decisions

| Decision                | Recommendation                                                       | Rationale                                                              |
| ----------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Trainer classification  | Data-driven TOML catalog (same pattern as optimization catalog #129) | Extensible without recompilation; community taps can contribute types  |
| Hash caching            | SQLite `trainer_hash_cache` + stat-based invalidation                | Existing `hash_trainer_file()` available, fast path via mtime          |
| Network detection       | TCP probe to 8.8.8.8:53 (2s timeout)                                 | No new deps, stdlib only. Optional for pre-flight; passive for runtime |
| Community tap cache     | Serve from existing Git clone + SQLite index                         | Taps already local after sync; zero new infrastructure                 |
| Offline launch behavior | Warn but allow (fail-soft)                                           | Consistent with existing `ValidationSeverity::Warning` pattern         |
| Migration               | Single v12→v13 with all three tables                                 | Feature ships atomically                                               |

### Quick Wins

- **Trainer type catalog**: Data-driven TOML catalog following optimization catalog pattern, user-extensible, one PR
- **Hash-on-save**: Call existing `hash_trainer_file()` on profile save, one migration
- **Tap "last synced"**: Display `last_indexed_at` from existing `community_taps` table — data already in SQLite

### Future Enhancements

- **Offline profile bundles**: Self-contained archive (TOML + trainer + Proton ref) for USB/SD transfer
- **Network isolation synergy** (#62): Auto-enable `unshare --net` when offline readiness confirmed
- **Offline health integration**: Add `offline_ready` dimension to existing `health_snapshots`

## Risk Assessment

### Technical Risks

| Risk                                   | Likelihood | Impact | Mitigation                                                                       |
| -------------------------------------- | ---------- | ------ | -------------------------------------------------------------------------------- |
| Hash computation slow on 1GB+ trainers | Low        | Medium | Streaming SHA-256 (sha2 supports this); most trainers are 1-50MB                 |
| Git cache disk growth                  | Medium     | Low    | Single-branch cloning already used; taps typically <10MB                         |
| Network detection false positives      | Medium     | Medium | Passive detection as primary; optional TCP probe for pre-flight only             |
| New `trainer_type` field on profiles   | Low        | Low    | Additive String field with `#[serde(default)]` → `"unknown"`; no breaking change |
| Migration 13 conflicts                 | Low        | High   | Confirm no in-flight features claim this migration number                        |
| Steam Deck mtime resolution (1s)       | Low        | Low    | Accepted limitation; worst case is one-launch stale hash window                  |

### Integration Challenges

- **Profile TOML backward compatibility**: New `trainer_type` field defaults to `"unknown"` via `#[serde(default)]`. Existing profiles without the field work unchanged.
- **Frontend type sync**: TypeScript must load `TrainerTypeEntry[]` from the catalog via Tauri IPC and use it to populate dropdowns and drive badge display.
- **Health system integration**: Extend `HealthIssue` with offline fields rather than parallel reporting system.

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| Finding                                    | Risk                            | Mitigation                                                      | Alternatives                                                |
| ------------------------------------------ | ------------------------------- | --------------------------------------------------------------- | ----------------------------------------------------------- |
| W-1: Offline keys in plaintext             | Credential exposure             | OS keyring via `keyring` crate; AES-256 fallback for Steam Deck | Defer if Aurora/WeMod key storage is out of v1 scope        |
| W-2: SQLite 0644 permissions               | Local data exposure             | `chmod 0600` on metadata.db after creation                      | —                                                           |
| W-3: Hash comparison timing oracle         | Theoretical timing attack       | `subtle::ConstantTimeEq` or inline constant-time comparison     | Practices research argues deferrable for local-only process |
| W-4: FLiNG trainers are untrusted binaries | Malware execution               | Mandatory hash verification + network isolation default-on      | Trust dialog on first launch from new hash                  |
| W-5: `offline_activated` in portable TOML  | False trust assertion on import | Store in SQLite only, not TOML profile                          | —                                                           |

#### Advisories — Best Practices

- A-1: Community tap directory permissions → `0700` (deferral: low-risk public data)
- A-2: Hash cache invalidation on file modification → stat-based check (implemented in design)
- A-5: Add `cargo audit` to CI (deferral: preventive measure, no active advisories)
- A-6: Git command hardening → add `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null` (low-effort, accept)

## Task Breakdown Preview

### Phase 1: Foundation

**Focus**: Trainer type model, hash caching, offline readiness scoring
**Parallelization**: Tasks 1A and 1B run concurrently; 1C depends on both

**Tasks**:

- **1A**: Create data-driven trainer type catalog (`default_trainer_type_catalog.toml` + `TrainerTypeCatalog` loader following `launch/catalog.rs` pattern), add `OfflineCapability` enum, add `trainer_type: String` field to `TrainerSection`, update TypeScript types, add trainer type dropdown to `ProfileFormSections.tsx` populated from catalog with per-type offline capability tooltips, unit tests (~8 files)
- **1B**: SQLite migration 13, `trainer_hash_store.rs` in metadata, call `hash_trainer_file()` on save, Tauri command `compute_trainer_hash` (~5 files)
- **1C**: `check_offline_readiness()` using `ReadinessCheckResult` pattern from `onboarding/readiness.rs`, Tauri commands for single + batch check (~4 files)

### Phase 2: Launch Integration

**Focus**: Wire offline readiness into launch pipeline
**Dependencies**: Phase 1 complete
**Parallelization**: Tasks 2A, 2B, 2C all run concurrently

**Tasks**:

- **2A**: Pre-launch hash verification, `ValidationError::TrainerHashMismatch` variant, mismatch dialog (~4 files)
- **2B**: Offline-specific help text in validation errors, `LaunchPanel.tsx` offline warnings (~3 files)
- **2C**: Verify launch history works offline, preserve correlation status from last online check (~2 files)

### Phase 3: Community & Aurora

**Focus**: Community tap offline caching, Aurora/WeMod instructional modals
**Parallelization**: 3A and 3B run concurrently; 3C depends on 3A

**Tasks**:

- **3A**: Community page "last synced" display, sync failure fallback to cached SQLite data, manual refresh button (~4 files)
- **3B**: `OfflineTrainerInfoModal.tsx` — platform-aware (Steam Deck vs desktop), step-by-step Aurora guide, "Mark as configured" toggle (~3 files)
- **3C**: Extend offline readiness for community tap staleness, display on community profile cards (~2 files)

### Phase 4: UI Polish

**Focus**: Status indicators, graceful degradation, pre-flight dashboard
**Dependencies**: Phases 1-3 complete
**Parallelization**: 4A and 4B concurrent; 4C depends on both

**Tasks**:

- **4A**: `OfflineStatusBadge` on profile cards in `ProfilesPage.tsx`, integrate with `HealthBadge` pattern (~3 files)
- **4B**: Graceful degradation across all pages — community sync button offline state, settings page offline handling, `useNetworkStatus` hook (~5 files)
- **4C**: "Offline Readiness" section on Health Dashboard, batch "Prepare for Offline" button (~3 files)

### Parallelization Map

```
Phase 1:  [1A: Trainer Type] ────┐
          [1B: Hash Caching] ────┴──→ [1C: Offline Readiness]
                                              │
Phase 2:  [2A: Hash Verify] ─────────────────┤
          [2B: Offline Validation] ───────────┤  (concurrent)
          [2C: History Preserve] ─────────────┤
                                              │
Phase 3:  [3A: Tap Cache] ──────┐             │
          [3B: Aurora Modal] ───┴──→ [3C: Community Readiness]
                                              │
Phase 4:  [4A: Status Badges] ──┐             │
          [4B: Degradation UI] ─┴──→ [4C: Pre-Flight Dashboard]
```

**Maximum parallelism**: 2-3 agents per phase. Cross-phase dependencies are sequential.

## Decisions Needed

1. **Module placement**: Create new `offline/` module (tech-designer recommendation) vs extend existing modules (practices-researcher recommendation)?
   - **Recommendation**: New `offline/` module for catalog, capability enum, and readiness logic; extend existing modules for integration points
   - **Impact**: Affects file organization and import paths

2. **Hash verification on mismatch**: Block launch (strict/security) vs warn and allow (permissive/UX)?
   - **Recommendation**: Warn with "Update Hash" / "Cancel" dialog — trainers get legitimately updated
   - **Impact**: Affects launch pipeline behavior

3. **Network detection approach**: Active probe (TCP) vs passive (try-and-handle-failure)?
   - **Recommendation**: Passive as default; optional active probe for explicit pre-flight
   - **Impact**: Affects Phase 4 complexity

4. **Aurora/WeMod key storage scope for v1**: Full keyring integration or defer to advisory?
   - **Recommendation**: Defer keyring to post-v1 if Aurora key storage is not automated. The info modal approach doesn't store keys — it just tracks the user's self-reported activation state in SQLite
   - **Impact**: Reduces Phase 3 complexity significantly

5. **Constant-time hash comparison**: Add `subtle` crate (security recommendation) vs defer (practices recommendation)?
   - **Recommendation**: Defer — local process, no network boundary. Add comment explaining reasoning
   - **Impact**: Zero vs one new dependency

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): FLiNG/Aurora/WeMod API analysis, library recommendations, code examples
- [research-business.md](./research-business.md): User stories, business rules (BR-1 through BR-8), workflows, state machine
- [research-technical.md](./research-technical.md): Architecture, SQLite schemas, Tauri IPC commands, codebase changes
- [research-ux.md](./research-ux.md): Competitive analysis (Heroic anti-patterns), Aurora modal design, Steam Deck UX
- [research-security.md](./research-security.md): 5 WARNING findings, secure coding guidelines, trade-off recommendations
- [research-practices.md](./research-practices.md): Existing reusable code, KISS assessment, build-vs-depend analysis
- [research-recommendations.md](./research-recommendations.md): Phased implementation strategy, risk matrix, parallelization map
