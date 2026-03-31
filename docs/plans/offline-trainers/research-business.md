# Business Analysis: Offline Trainers

## Executive Summary

Offline-first trainer management enables CrossHook's primary user group — Steam Deck portable gamers — to use fully configured profiles and trainers in environments without network access (travel, LAN parties, airplane mode). The feature requires trainer type classification to determine offline capability, a hash-based cache to verify local file integrity, graceful degradation of network-only features, and a pre-flight validation gate before offline sessions. FLiNG trainers are fully offline-capable; Aurora/WeMod trainers require a one-time online activation step and have an offline mode that must be guided by the UI. Community tap profiles are already stored locally after sync and need no new caching — only a check that the tap was synced at least once.

---

## User Stories

### Primary Users

**Steam Deck Portable Gamer**

- As a Steam Deck user traveling, I want to load and launch a game with my FLiNG trainer without internet access so I do not need to find a hotspot before playing.
- As a Steam Deck user on an airplane, I want CrossHook to tell me which profiles are ready to use offline before I lose connectivity.
- As a Steam Deck user, I want to configure my Aurora trainer's offline activation key while I have internet, so I can use it later when offline.

**Linux Desktop Gamer**

- As a Linux desktop gamer, I want CrossHook to warn me gracefully if tap sync fails due to network unavailability, rather than crashing or losing my existing cached profiles.
- As a Linux desktop gamer who sometimes uses a VPN or metered connection, I want to skip network operations when my connection is slow.

---

## Business Rules

### Core Rules

#### BR-1: Trainer Type Offline Classification

The offline feature introduces a new typed `trainer_type` enum field in `TrainerSection` for programmatic classification. The existing `trainer.kind` free-form string is preserved for display purposes — the two fields coexist. **Critically, no existing code branches on `trainer.kind` at all** — it is stored and displayed but never evaluated at runtime. All trainer-type classification logic must be built from scratch using the new enum.

**TrainerType enum values**: `Unknown | Fling | Aurora | Wemod | Standalone | Custom`

Classification rules:

| trainer_type        | Offline Capable       | Score Cap | Notes                                                                       |
| ------------------- | --------------------- | --------- | --------------------------------------------------------------------------- |
| `Fling`             | Yes — fully offline   | 100       | Standalone `.exe`, no network dependency at runtime                         |
| `Standalone`        | Yes — fully offline   | 100       | Generic standalone executable, treated same as FLiNG                        |
| `Aurora`            | Conditional — warning | 90        | Requires prior online activation; launch proceeds with warning when offline |
| `Wemod`             | Conditional — warning | 90        | Same as Aurora — requires prior activation                                  |
| `Custom`            | Unknown — warning     | 90        | User-defined type; treated conservatively                                   |
| `Unknown` (default) | Unknown — warning     | 90        | Backward-compatible default for existing profiles                           |

**Field coexistence rule**: `trainer.kind` (String) is the human-readable display value and remains unchanged. `trainer.trainer_type` (enum) is the new machine-evaluated field. Both live in the TOML `[trainer]` section and travel with portable profile exports. The `Unknown` default ensures existing profiles without `trainer_type` set behave as before.

#### BR-2: Offline Readiness Scoring

Offline readiness is expressed as a **0–100 weighted composite score**, not a binary pass/fail. This allows partial readiness to be surfaced to the user rather than an opaque block.

**Score weights:**

| Factor               | Weight | Condition                                                      |
| -------------------- | ------ | -------------------------------------------------------------- |
| trainer_present      | 30     | Trainer path configured and file exists on disk                |
| hash_valid           | 15     | SHA-256 hash recorded and matches current file                 |
| game_present         | 20     | Game executable exists on disk                                 |
| proton_available     | 15     | Proton executable exists (method-dependent)                    |
| prefix_exists        | 10     | Proton prefix directory exists (method-dependent)              |
| network_not_required | 10     | trainer_type is `Fling` or `Standalone` (no activation needed) |

**Score interpretation:**

- **100**: Fully offline-ready (FLiNG/Standalone with all files present)
- **90 max**: Aurora/WeMod/Unknown/Custom — network dependency; launch proceeds as Warning
- **< 70**: Likely not usable offline; UI highlights missing factors
- **0**: No paths configured

**Launch posture**: Offline readiness issues are surfaced as **Warnings**, not Fatal errors — following the existing pattern of `GamescopeNestedSession` in `launch/request.rs`. Users may proceed with launch even when the score is below 100. The score informs, it does not block.

All path checks must use `effective_profile()` (local override resolved) and must degrade gracefully when `MetadataStore` is unavailable — the `MetadataStore::disabled()` path must be handled.

#### BR-3: Trainer Hash Cache

When a profile is first saved with a trainer path, or when the trainer path is updated, compute the SHA-256 of the trainer binary and store it in the `version_snapshots` table. This table already supports `trainer_file_hash` as a nullable column (`version_store.rs:VersionSnapshotRow`). The hash serves as the canonical local-file-present signal for offline pre-flight checks.

**Cache invalidation strategy**: SHA-256 recomputation uses a **stat-based fast path** — file size + modification time are compared against the recorded snapshot before reading the full file. If size and mtime match, the stored hash is considered valid without re-reading the binary. If either differs, a full recompute is triggered.

> **Edge case**: On ext4 and similar filesystems, mtime has 1-second resolution. A trainer binary replaced within the same second may produce a false cache hit (stale hash not detected). This is an accepted limitation — the probability is low and the consequence is a warning at next launch rather than data loss.
> **Repurposing note**: `trainer_file_hash` in `version_snapshots` was designed for version correlation (update detection), not offline readiness. The tech design should resolve whether to store the offline-purpose hash in the same column (dual-purpose, simpler) or in a dedicated `offline_readiness` table (clearer semantics, more schema). This is OQ-7 below.

#### BR-4: Community Tap Offline Availability

Community taps are Git repositories cloned locally to `~/.local/share/crosshook/community/taps/<slug>`. After initial sync, profiles are readable from disk without network access. The offline cache rule is: a tap is offline-available if its workspace directory exists on the local filesystem — no additional caching is required. New tap subscriptions require a first-time sync before they are available offline. The `index_tap` function in `community/index.rs` already handles missing workspace directories gracefully by returning a diagnostic entry rather than an error.

**Tap staleness rule**: After initial sync the local clone may become arbitrarily out of date. Business rule: a tap is considered **stale** (not absent) if it has not been synced in more than 30 days. Stale taps are still usable offline but the UI should show a "last synced N days ago" indicator. The `community_taps.last_indexed_at` column in the metadata store already records sync timestamps and can be used to compute staleness.

#### BR-5: Network-Dependent Feature Degradation

Features that require network access must degrade gracefully:

- **Community tap sync**: If network is unavailable, skip sync and surface a non-blocking informational message. Do NOT block profile load or launch.
- **ProtonDB lookup** (future): Show stale data with a network-unavailable indicator rather than failing.
- **Version check** (future SteamCMD/API): Skip gracefully with a "last checked at [date]" message.

The key rule: network failure must never block the core user workflow of profile load → validate → launch.

#### BR-6: Explicit Offline Mode (Optional UX Enhancement)

An explicit offline mode toggle (stored in `AppSettingsData` in `settings/mod.rs`) is optional but valuable for Steam Deck users who know they are about to lose connectivity. When enabled:

- All network operations are skipped immediately without attempting connection.
- Only locally-ready profiles (BR-2) are shown as launchable.
- The app surfaces a persistent banner indicating offline mode is active.

This is a UX enhancement on top of the graceful degradation required by BR-5.

#### BR-7: Aurora/WeMod Offline Key Expiry and Launch Posture

Aurora and WeMod trainers have an offline mode that requires prior key activation while connected. **Aurora offline keys expire in 14 days** (a service-side hard constraint that CrossHook cannot extend or bypass). Business rules:

- The activation step must be performed at least once while online.
- CrossHook does not control or automate the activation — this requires user action within the trainer UI.
- CrossHook's role: detect Aurora/WeMod `trainer_type`, track the activation timestamp locally, enforce expiry at launch time even without network, and warn the user before expiry.
- **Expiry enforcement**: If the locally stored activation timestamp is more than 14 days old, the profile is treated as activation-expired. The offline readiness score drops and an expiry warning is surfaced.
- **3-day pre-expiry warning**: When an Aurora key is within 3 days of expiry, the UI surfaces a warning: "Your Aurora offline key expires in N day(s). Reconnect to renew."
- **Hardware binding**: Aurora offline keys are tied to hardware. Key invalidation due to hardware change is expected behavior, not a bug. CrossHook records this state as `key_hardware_invalid` — distinct from expiry.
- **Fail-soft posture**: When network is down and `trainer_type` is `Aurora` or `Wemod`, the launch proceeds with a logged warning if the key is still valid. An expired or hardware-invalidated key produces a Warning (not Fatal) but the score drops to reflect the degraded state.
- **Score cap**: Aurora/WeMod profiles cap at 90 regardless of file presence, reflecting the inherent network dependency.
- **WeMod ToS constraint**: WeMod requires an active subscription and caches session state for offline use. CrossHook must not attempt to extend offline sessions beyond what WeMod's platform intends. Session token caching scope must be verified against WeMod's Terms of Service before implementation.
- **Stored fields** (new, in TOML `[trainer]` section):
  - `trainer.offline_activated: bool` — user acknowledgment of activation
  - `trainer.offline_key_activated_at: Option<DateTime>` — timestamp of last activation (ISO 8601)
  - `trainer.offline_key_expires_at: Option<DateTime>` — computed expiry (activated_at + 14 days for Aurora)

#### BR-8: Pre-Flight Offline Check

Before entering an offline session (either via explicit mode toggle or when network is unavailable), the app should run a pre-flight validation sweep:

1. Run `check_profile_health` for all profiles (already exists).
2. For each healthy profile, compute the offline readiness score (BR-2).
3. Surface a summary: "N of M profiles score 100 (fully offline-ready)."
4. Profiles scoring < 100 are listed with their missing factors and remediation steps.
5. Aurora/WeMod profiles are flagged with their 90-cap warning regardless of other factors.
6. For Aurora/WeMod profiles with a stored activation timestamp: check expiry. Profiles within 3 days of expiry show a pre-expiry warning. Profiles past expiry show an expiry error with remediation: "Reconnect and re-activate offline mode."

The sweep is informational — it does not prevent launch. Users decide which profiles to use.

### Edge Cases

- **Trainer path changed but hash not updated**: If the user edits the trainer path after the hash was recorded, the stored hash is stale. The hash must be re-computed on profile save.
- **Trainer file modified but path unchanged**: A game update may replace trainer files. The version snapshot `VersionCorrelationStatus::TrainerChanged` state already tracks this — in offline mode, a `TrainerChanged` status should be surfaced as a warning ("trainer binary has changed since last hash — verify it is still compatible").
- **Community tap indexed but profile not installed**: Browsing community profiles offline shows the tap catalog (local SQLite index). The install workflow requires downloading files from the tap, which needs network. Attempting community profile install while offline must fail with a clear error: "Network required to install a community profile."
- **Steam update in progress (`UpdateInProgress`)**: If Steam is updating a game when the user enters offline mode, the version snapshot status is `UpdateInProgress`. This is already handled by `compute_correlation_status` in `version_store.rs`. In offline mode, block launch for profiles in this state.
- **No trainer configured**: Profiles with no trainer path (`trainer.path` is empty) skip all trainer-related offline checks — they are treated as game-only profiles and are fully offline-capable if their paths exist.
- **Local override paths**: The `effective_profile()` method in `profile/models.rs` resolves local-machine-specific path overrides. All offline readiness checks must use `effective_profile()`, not the raw profile, to ensure correct path resolution on the current machine.
- **First launch, no version snapshot yet**: If no version snapshot exists for a profile (`VersionCorrelationStatus::Untracked`), the offline pre-flight should prompt the user to do one online launch to capture the baseline hash before marking the profile as offline-ready.
- **MetadataStore disabled**: CrossHook has a `MetadataStore::disabled()` path for environments where SQLite is unavailable. All offline readiness scoring and hash lookup must degrade gracefully in this state — treat missing metadata as "unknown" rather than erroring. The score contribution of `hash_valid` is simply 0 when the store is unavailable.
- **Stat-based cache false hit**: A trainer binary replaced within 1 second of the last recorded snapshot may not be detected as changed (ext4 mtime resolution). Consequence: `TrainerChanged` is not surfaced at that launch. The hash will be detected as stale on the next sync cycle. Accepted limitation.
- **Aurora key hardware invalidation**: If hardware components are replaced, the Aurora offline key becomes invalid regardless of expiry date. CrossHook cannot detect this automatically — it manifests as a trainer failure at launch time. The `trainer.offline_key_hardware_invalid: bool` flag (set manually or detected on trainer error) records this state and clears the activation timestamp.
- **Aurora key expiry across device transfer**: If a profile is exported and imported on a new device, `offline_key_activated_at` travels with the TOML. The expiry timestamp reflects activation on the original device — the key is likely invalid on the new device. On import, `offline_activated` should be treated as unverified on the new machine.
- **Hash cache TTL policy**: Trainer binary hash entries should have no expiry (or a very long TTL) in `external_cache_entries` — they represent user-verified local files, not transient API responses. The standard eviction via `evict_expired_cache_entries()` must not remove trainer hashes. This is distinct from API response caches which have short TTLs.

---

## Workflows

### Primary Workflows

#### WF-1: Configure Profile for Offline Use

1. User opens a profile in the Profiles page.
2. User saves the profile with a trainer path configured.
3. On save, CrossHook computes SHA-256 of the trainer binary and records it as a version snapshot.
4. If trainer type is `aurora` or `wemod`, CrossHook displays an info banner: "Aurora trainers require offline activation. Open the trainer, activate offline mode, and then mark it done here."
5. User activates offline mode in the Aurora trainer UI (manual step, outside CrossHook).
6. User returns to CrossHook and marks offline activation complete for this profile.
7. Profile is now flagged as offline-ready.

#### WF-2: Launch Offline (FLiNG Trainer)

1. User opens CrossHook without network.
2. App attempts community tap sync — fails silently with a non-blocking notification.
3. User navigates to Profiles, selects a FLiNG profile.
4. App runs offline pre-flight: paths exist, trainer hash recorded, type is `fling` → offline-ready.
5. User launches normally — no network dependency in the launch path.

#### WF-3: Aurora Offline Key Setup

1. User is online and setting up a new Aurora profile.
2. Profile has `trainer_type = Aurora`.
3. On profile save, CrossHook shows an "Offline Setup Required" info modal explaining:
   - Aurora offline keys are valid for 14 days and must be renewed by reconnecting.
   - Steps to activate in Aurora: open trainer, go to settings, enable offline mode.
   - A "Mark as Activated" button once the user has completed the steps.
4. User completes activation in Aurora trainer UI.
5. User clicks "Mark as Activated" in CrossHook.
6. CrossHook records `trainer.offline_activated = true`, `trainer.offline_key_activated_at = now()`, and `trainer.offline_key_expires_at = now() + 14 days`, then saves the profile.
7. Profile is now offline-ready with a 14-day expiry window.

#### WF-6: Aurora Key Expiry Renewal

1. CrossHook pre-flight (or startup check) detects `offline_key_expires_at` is within 3 days.
2. UI shows a persistent warning: "Aurora offline key for [Profile Name] expires in N day(s). Connect and reactivate to renew."
3. User connects to network, opens Aurora trainer, re-enables offline mode.
4. User clicks "Mark as Reactivated" in CrossHook.
5. CrossHook updates `offline_key_activated_at` and `offline_key_expires_at`, saves profile.
6. Warning is cleared; 14-day window resets.

#### WF-4: Pre-Flight Validation Before Offline Session

1. User toggles "Offline Mode" in Settings (or app detects no network).
2. App runs `check_profile_health` across all profiles.
3. For healthy profiles, checks trainer hash presence, Aurora activation state, and key expiry timestamp.
4. Displays: "3 of 5 profiles are offline-ready."
5. Broken/unready profiles are shown with specific remediation steps.
6. User can proceed to launch any offline-ready profile.

#### WF-5: Community Tap Offline Usage

1. User subscribed to a tap while online; tap was synced to local disk.
2. User opens CrossHook offline.
3. Community page loads from local SQLite index — no network needed.
4. User can browse and view community profiles.
5. If user attempts to install a community profile, app shows: "Network required to install community profiles."
6. Existing locally-installed profiles from taps function normally.

### Error Recovery Workflows

#### EWF-1: Tap Sync Fails (Network Unavailable)

1. App startup: attempts to sync subscribed taps.
2. Git command fails with network error.
3. App catches `CommunityTapError::Git` — logs the failure.
4. App continues startup with existing local tap data.
5. UI shows a dismissable notification: "Community tap sync unavailable — showing cached profiles."

#### EWF-2: Trainer Hash Missing (Untracked Profile)

1. User attempts to use offline pre-flight for a profile without a recorded hash.
2. Pre-flight reports: "Trainer hash not recorded. Connect to the internet and launch once to capture baseline."
3. Profile is flagged as offline-not-ready until a successful launch records the hash.

#### EWF-3: Trainer Binary Changed (`TrainerChanged`)

1. User checks offline readiness for a profile.
2. Version correlation status is `TrainerChanged` (binary on disk differs from recorded hash).
3. Pre-flight warning: "Trainer binary has changed. Verify the trainer is still compatible before offline use."
4. User can re-record the hash by launching once online.

---

## Domain Model

### Key Entities

**TrainerType** (new concept — currently a free-form `trainer.kind` string)

- Values: `fling`, `aurora`, `wemod`, `unknown`
- Determines: offline capability classification
- Lives in: `profile/models.rs::TrainerSection::kind`

**OfflineReadinessStatus** (new computed concept, contributes to score in BR-2)

- `ready`: all conditions met (FLiNG/Standalone with all files — score 100)
- `not_ready_paths`: one or more paths missing/broken
- `not_ready_hash_missing`: trainer hash not yet recorded
- `not_ready_aurora_not_activated`: Aurora/WeMod, activation not confirmed
- `not_ready_unknown_type`: trainer type unclassified
- `aurora_key_expiring_soon`: key valid but within 3-day warning window
- `aurora_key_expired`: activation timestamp > 14 days ago
- `aurora_key_hardware_invalid`: hardware change invalidated key

**TrainerOfflineActivation** (new fields on profile, TOML `[trainer]` section)

- `trainer.offline_activated: bool` — user acknowledgment of activation; portable
- `trainer.offline_key_activated_at: Option<DateTime>` — ISO 8601 timestamp of last activation; portable (but context-dependent on new devices)
- `trainer.offline_key_expires_at: Option<DateTime>` — computed expiry (activated_at + 14 days for Aurora); portable
- `trainer.offline_key_hardware_invalid: bool` — set when hardware change invalidates the key; machine-local in practice
- Only meaningful for `aurora`/`wemod` trainer types
- **14-day expiry** is a hard Aurora service constraint; CrossHook enforces it locally without network

**OfflineCache** (community taps)

- Already exists: `~/.local/share/crosshook/community/taps/<slug>` (Git clone)
- Offline availability: boolean — directory exists vs. not

**VersionSnapshot** (already exists — `metadata/version_store.rs`)

- Contains `trainer_file_hash: Option<String>` — SHA-256 of trainer binary
- Contains `VersionCorrelationStatus` — tracks trainer vs. game version alignment
- Key field for offline readiness: presence of `trainer_file_hash`

### State Transitions

**Profile Offline Readiness** state machine:

```
[Unconfigured] ──save with trainer path──> [Hash Recording]
[Hash Recording] ──SHA-256 success──> [Hash Recorded]
[Hash Recorded] + trainer is FLiNG/Standalone ──> [Offline Ready]
[Hash Recorded] + trainer is Aurora/WeMod ──> [Awaiting Activation]
[Awaiting Activation] ──user marks activated + timestamp stored──> [Offline Ready (Key Valid)]
[Offline Ready (Key Valid)] ──within 3 days of expiry──> [Offline Ready (Key Expiring Soon)]
[Offline Ready (Key Expiring Soon)] ──user reconnects + reactivates──> [Offline Ready (Key Valid)]
[Offline Ready (Key Valid)] ──past 14 days since activation──> [Key Expired]
[Key Expired] ──user reconnects + reactivates──> [Offline Ready (Key Valid)]
[Offline Ready (Key Valid)] ──hardware change detected──> [Key Hardware Invalid]
[Key Hardware Invalid] ──user reactivates on new hardware──> [Offline Ready (Key Valid)]
[Offline Ready (Key Valid)] ──trainer binary changed──> [Hash Stale / Needs Recheck]
[Hash Stale] ──launch once (online) and re-hash──> [Offline Ready (Key Valid)]
```

**Community Tap Offline State**:

```
[Not Subscribed] ──subscribe and sync──> [Local Cache Present]
[Local Cache Present] ──network available──> [Syncing]
[Syncing] ──success──> [Local Cache Present (Updated)]
[Syncing] ──failure (no network)──> [Local Cache Present (Stale)]
[Not Subscribed] / [Never Synced] ──offline check──> [Not Offline Available]
```

---

## Existing Codebase Integration

The following existing constructs are directly relevant to this feature:

**Profile Model (`profile/models.rs`)**

- `TrainerSection::kind` — where trainer type classification lives (currently free-form string, needs enum enforcement or validation layer)
- `GameProfile::effective_profile()` — must be used for all offline readiness path checks to resolve local overrides correctly
- `LocalOverrideSection` — machine-specific path overrides affect trainer path resolution

**Health Check (`profile/health.rs`)**

- `check_profile_health()` — the foundation for offline pre-flight path validation; directly reusable
- `HealthStatus::{Healthy, Stale, Broken}` — maps directly to offline readiness categories
- `batch_check_health()` — enables the batch pre-flight check across all profiles

**Version Store (`metadata/version_store.rs`)**

- `hash_trainer_file()` — SHA-256 hash computation, already implemented; core primitive for trainer cache
- `VersionSnapshotRow::trainer_file_hash` — storage field for recorded trainer hash
- `VersionCorrelationStatus::TrainerChanged` — signals stale hash condition for offline re-check
- `compute_correlation_status()` — pure comparison function, no I/O, directly usable in pre-flight

**Launch Validation (`launch/request.rs`)**

- `ValidationError` enum — pattern to follow for offline-specific validation errors
- `LaunchValidationIssue` — IPC-friendly validation result type; extend for offline issues

**Community Taps (`community/taps.rs`, `community/index.rs`)**

- `CommunityTapStore::sync_tap()` — this is the network operation that must fail gracefully
- `CommunityTapError::Git` — the error type surfaced on network failure during git operations
- `index_tap()` — already returns gracefully when workspace directory does not exist (diagnostic entry rather than error)

**Cache Store (`metadata/cache_store.rs`)**

- `external_cache_entries` table — generic key/value cache with expiry; could store network-fetched data (e.g., ProtonDB results) for offline use
- `MAX_CACHE_PAYLOAD_BYTES = 524_288` — size limit for cached payloads

**Settings (`settings/mod.rs`)**

- `AppSettingsData` — a minimal flat struct with 4 fields; no sections. Adding `offline_mode: bool` is straightforward. However, any per-profile offline metadata (activation state, hash status) does NOT belong here — it belongs in SQLite. AppSettings should only hold app-level switches (offline mode toggle), not per-profile state.
- `community_taps: Vec<CommunityTapSubscription>` — already persists tap subscriptions

---

## Success Criteria

1. A profile with a FLiNG trainer and all paths present on disk can launch without any network connection.
2. A profile with an Aurora/WeMod trainer shows an offline readiness warning (score capped at 90) and surfaces an activation guide; launch is not blocked but the warning is logged and displayed.
3. Community tap profiles browseable from local cache when offline; install attempts fail with a clear message.
4. Community tap sync failure at startup does not block profile load or launch.
5. Pre-flight validation accurately identifies which profiles are offline-ready before the user loses connectivity.
6. Trainer hash is computed and stored on profile save; stale hash is detected and surfaced as a warning.
7. Explicit offline mode toggle in settings skips all network operations immediately.

---

## Open Questions

1. ~~**TrainerType enum enforcement**~~ **RESOLVED**: A new `trainer.trainer_type` enum field coexists with the existing `trainer.kind` free-form string. `kind` remains for display; `trainer_type` drives offline classification logic. `Unknown` is the default for backward compatibility.

2. ~~**Aurora activation persistence scope**~~ **RESOLVED**: `trainer.offline_activated` lives in TOML (portable), consistent with `trainer_type` and the existing `portable_profile()` / `storage_profile()` pattern. Per the tech design, `trainer_type` is a portable field.

3. **Offline mode trigger**: Should offline mode be manual-only (explicit toggle), automatic on network loss detection, or both? Automatic detection adds complexity but is more Steam Deck-friendly.

4. **WeMod classification**: WeMod has both a free trainer tier and a subscription tier. Offline capability may differ between tiers. Is WeMod in scope for this feature, or is it explicitly out of scope?

5. **Community profile install from tap**: Is installing a community profile (which requires git clone/pull from remote) considered an offline-blocked operation, or should CrossHook support a "sideload" path where the user has manually placed the profile JSON on disk?

6. **Tap staleness threshold**: The 30-day staleness threshold for community taps is an assumed default. Should this be user-configurable, or is a fixed threshold acceptable?

7. **Hash repurposing vs. dedicated column**: Should offline trainer hash storage reuse `version_snapshots.trainer_file_hash` (already computed, dual-purpose) or be stored in a dedicated `offline_readiness` metadata table? Reuse is simpler but conflates version correlation with offline readiness semantics.
