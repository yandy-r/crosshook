# Offline Trainers: Implementation Recommendations & Risk Assessment

**Date**: 2026-03-31
**Issue**: #44 -- Offline-first trainer management for Steam Deck portable use
**Related Issues**: #62 (network isolation), #63 (hash verification)
**Scope**: Full feature plan across 4 phases, suitable for parallel agent execution

---

## Executive Summary

CrossHook's architecture is already 80% offline-capable: profiles are local TOML files, trainer binaries are local executables, and community taps clone to local Git repositories. The remaining work is (1) explicit trainer type classification to differentiate FLiNG (fully offline) from Aurora/WeMod (conditional), (2) trainer hash caching for integrity verification without network, (3) graceful degradation for network-dependent features, and (4) a pre-flight offline readiness check. The `sha2` crate and `hash_trainer_file()` function already exist -- the foundation is in place.

---

## Implementation Recommendations

### Approach: Evolutionary Extension, Not Greenfield

The codebase already contains the building blocks. The implementation should extend existing modules rather than creating new top-level modules. Key principle: CrossHook should work offline by default without the user enabling an "offline mode" toggle. Graceful degradation, not explicit mode switching.

### Technology Choices

| Component                 | Recommendation                                                | Rationale                                                                   |
| ------------------------- | ------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Trainer type model        | `TrainerType` enum in `profile/models.rs`                     | `trainer.kind` String field exists but is unused -- natural evolution point |
| Hash caching              | Extend `version_snapshots` + new `trainer_hashes` table       | `hash_trainer_file()` and `sha256_hex()` already exist in metadata          |
| Network detection         | Simple TCP connect probe (2s timeout)                         | Avoid background daemon; check on demand when user triggers sync            |
| Offline cache (community) | Leverage existing Git clones + `external_cache_entries` table | Taps already persist locally after `sync_tap()`                             |
| Pre-flight checks         | Extend `onboarding/readiness.rs` pattern                      | `HealthIssue`/`HealthIssueSeverity` framework is reusable                   |
| Frontend state            | New `useOfflineStatus` hook                                   | Follows existing hook pattern (`useLaunchState`, `useProfileHealth`)        |

### Phasing Strategy

**Phase 1 (Foundation)** -- FLiNG first as quick win. FLiNG trainers are standalone executables that work fully offline. This phase adds trainer type classification and hash caching, delivering immediate value for the most common trainer type.

**Phase 2 (Launch Integration)** -- Wire offline readiness into the launch pipeline. Pre-flight checks validate all dependencies are locally available. Launch validation gains offline-aware error messages.

**Phase 3 (Community & Aurora)** -- Community tap offline caching with "last synced" metadata. Aurora/WeMod info modal explaining activation requirements for offline use.

**Phase 4 (UI Polish)** -- Offline status indicators, health badges, and graceful degradation across all network-dependent UI surfaces.

### Quick Wins (Can Ship Independently)

1. **Trainer type enum**: Convert `trainer.kind: String` to `TrainerType` enum in one PR. Zero runtime behavior change, pure type improvement.
2. **Hash-on-save**: Call existing `hash_trainer_file()` when profile is saved, store in a new metadata column. One migration + one Tauri command change.
3. **Tap "last synced" display**: Add `last_indexed_at` from `community_taps` table to the CommunityBrowser UI. The data already exists in SQLite.

---

## Improvement Ideas

### Related Features That Complement Offline Support

1. **Network isolation (#62)**: `unshare --net` becomes a free security win in offline scenarios. Consider auto-enabling when offline readiness is confirmed for a profile.

2. **Hash verification (#63)**: Core dependency for offline trainers. Implement hash caching first; verification becomes a natural extension. The hash-on-save from Phase 1 feeds directly into pre-launch verification.

3. **Version correlation (existing)**: `compute_correlation_status()` already detects game/trainer version drift. Offline mode should preserve the last-known correlation status and display it even when unable to refresh.

4. **Launcher export synergy**: Exported `.sh` scripts (from `export/launcher.rs`) are inherently offline. Add an offline readiness badge to the launcher export flow to indicate which exports will work without network.

### Future Enhancements

- **Offline profile bundles**: Export a self-contained archive (profile TOML + trainer binary + Proton version reference) for transfer between machines via USB/SD card.
- **Stale cache warnings**: When community taps haven't been synced in >30 days, show a non-blocking warning on the Community page.
- **Offline health score integration**: Extend the existing `health_snapshots` table to include an `offline_ready` dimension in the profile health score.

---

## Risk Assessment

### Technical Risks

| Risk                                                  | Likelihood | Impact | Mitigation                                                                                                                                                                       |
| ----------------------------------------------------- | ---------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hash computation performance on large trainers (>1GB) | Low        | Medium | Most trainers are 1-50MB. For large files, use streaming SHA-256 (sha2 supports this). Add a progress indicator for files >100MB.                                                |
| Git cache disk usage growth                           | Medium     | Low    | Community taps use `--single-branch` cloning already. Add cache size monitoring and manual cleanup button. Taps are typically <10MB.                                             |
| Network detection false positives                     | Medium     | Medium | Don't rely on a single probe. Check multiple targets (DNS resolution + TCP connect). But keep it simple -- no daemon.                                                            |
| Trainer type auto-detection accuracy                  | Medium     | Low    | Don't try to auto-detect from binary analysis. Let users select from dropdown (FLiNG, Aurora, WeMod, Other). Default to "Unknown" which assumes offline-capable.                 |
| SQLite migration complexity                           | Low        | High   | Migration 13 adds `trainer_hashes` table. Use the established migration pattern (versioned, transactional). Test with `open_in_memory()` as existing tests do.                   |
| Breaking change to `trainer.kind` field               | Medium     | Medium | Existing profiles have empty `trainer.kind` strings. The enum must have a `#[default] Unknown` variant that deserializes from empty string. Test roundtrip with legacy profiles. |
| Gamescope interaction in offline mode                 | Low        | Low    | Gamescope is a local compositor -- no network dependency. No special handling needed.                                                                                            |

### Integration Challenges

1. **Profile TOML backward compatibility**: Converting `trainer.kind` from String to enum requires careful serde handling. The `#[serde(rename_all = "snake_case")]` pattern used elsewhere (e.g., `TrainerLoadingMode`) works here. Empty strings must deserialize to `Unknown`.

2. **Tauri IPC surface expansion**: Each new backend capability needs a corresponding `#[tauri::command]`. Estimate 4-6 new commands across all phases. Follow the existing pattern in `commands/launch.rs` where each command is a thin wrapper around `crosshook-core` functions.

3. **Frontend type sync**: TypeScript types in `src/types/profile.ts` must mirror Rust changes. The `GameProfile` interface needs `trainer.type` updated from `string` to a union type matching `TrainerType` enum variants.

4. **Health system integration**: The offline readiness check should feed into the existing `HealthDashboardPage` without creating a parallel reporting system. Extend `HealthIssue` with offline-specific fields rather than creating new types.

### Performance Considerations

- **Trainer hashing at save time**: Hashing a 50MB file takes ~50ms on modern hardware. Acceptable for save operations, but must be async to avoid blocking the UI thread. Use `tokio::task::spawn_blocking()`.
- **Batch offline readiness check**: Validating all profiles scans the filesystem for each path. For 20 profiles with 5 paths each, that's 100 `Path::exists()` calls -- negligible (<1ms total). No optimization needed.
- **Community tap cache reads**: Reading cached index from SQLite is faster than re-parsing the Git working tree. The `community_profiles` table already stores denormalized index data.

### Security Considerations

- **Hash storage location**: Store trainer hashes in SQLite metadata DB (already secured with 0o600 permissions and symlink detection). Do NOT store hashes in the TOML profile file where users could modify them.
- **TOCTOU risk**: A trainer file could be modified between hash check and launch. Mitigation: compute hash immediately before launching, not during a separate "check" step. This is already how `build_proton_game_command` flows -- validation and launch are adjacent.
- **Community tap integrity**: Git provides commit-level integrity. For additional assurance, consider storing the HEAD commit hash in SQLite after each sync and verifying it matches before serving cached profiles offline.

---

## Alternative Approaches

### Option A: TrainerType Enum (Recommended)

Add a `TrainerType` enum to `TrainerSection` in `profile/models.rs`, replacing the unused `kind: String` field.

```
enum TrainerType { FLiNG, Aurora, WeMod, Unknown }
```

**Pros**: Type-safe, enables compile-time exhaustive matching for offline behavior, natural fit with existing model.
**Cons**: Breaking change to TOML serialization (mitigated by serde default).
**Effort**: Low (1-2 days)

### Option B: Capability-Based Model

Instead of vendor classification, model offline behavior as capabilities.

```
enum OfflineCapability { Full, RequiresActivation, OnlineOnly }
```

**Pros**: More flexible, vendor-agnostic, forward-compatible.
**Cons**: Harder to auto-detect, less intuitive for users ("What does RequiresActivation mean?"), abstracts away the trainer brand that users actually recognize.
**Effort**: Low-Medium (2-3 days)

### Option C: Metadata-Only (No Profile Model Changes)

Store all offline metadata in SQLite only, without modifying the profile TOML model.

**Pros**: Zero breaking changes to existing profiles or TOML format.
**Cons**: Offline readiness metadata is invisible in profile TOML exports, harder to include in community profile manifests, creates a hidden dependency on the metadata DB.
**Effort**: Low (1-2 days)

**Recommendation**: Option A (TrainerType enum). It aligns with the codebase's preference for strong typing (see `TrainerLoadingMode`, `CompatibilityRating`, `GamescopeFilter` enums) and provides the foundation that Options B and C build on. The capability model from Option B can be derived from the TrainerType at runtime without needing its own field.

### Network Detection Alternatives

| Approach                              | Pros                                   | Cons                                             | Effort   |
| ------------------------------------- | -------------------------------------- | ------------------------------------------------ | -------- |
| TCP probe to well-known host          | Simple, fast, reliable                 | False positive if firewall blocks specific hosts | Very Low |
| DNS resolution check                  | Tests actual name resolution path      | DNS caching may cause false positives            | Low      |
| `NetworkManager` D-Bus API            | Authoritative on systemd-based systems | Not available on all Linux distros, complex      | Medium   |
| Passive detection (no explicit check) | Zero overhead, graceful by design      | No proactive warning capability                  | None     |

**Recommendation**: Passive detection as the primary approach. CrossHook should try operations and handle failures gracefully, rather than proactively checking network status. Add an optional TCP probe only for the "pre-flight offline readiness" feature where users explicitly want to verify before going offline.

### Cache Strategy Alternatives

| Strategy                              | Pros                                   | Cons                                                   |
| ------------------------------------- | -------------------------------------- | ------------------------------------------------------ |
| SQLite `external_cache_entries` table | Already exists, queryable, size-capped | Requires migration to add offline-specific columns     |
| Filesystem cache directory            | Simple, transparent, easy to inspect   | No built-in TTL, harder to query metadata              |
| Embedded key-value store (sled, redb) | Fast, purpose-built                    | New dependency, second storage engine alongside SQLite |

**Recommendation**: SQLite `external_cache_entries` table. It already exists with the right columns (`cache_key`, `payload_json`, `expires_at`). No new dependencies needed.

---

## Task Breakdown Preview

### Phase 1: Foundation (Estimated: 3-4 days)

**Task Group 1A: Trainer Type Model** (can parallelize with 1B)

- Add `TrainerType` enum to `profile/models.rs` with serde support
- Update `TrainerSection.kind` from `String` to `TrainerType`
- Add backward-compatible deserialization (empty string -> `Unknown`)
- Update TypeScript `GameProfile.trainer.type` to union type
- Update `ProfileFormSections.tsx` with trainer type dropdown
- Add unit tests for enum serialization roundtrip with legacy profiles
- Complexity: Low | Files: ~6 | Dependencies: None

**Task Group 1B: Trainer Hash Caching** (can parallelize with 1A)

- Add `trainer_hashes` table via SQLite migration 13
- Schema: `profile_id TEXT, trainer_path TEXT, sha256_hash TEXT, file_size INTEGER, computed_at TEXT`
- Create `trainer_hash_store.rs` in metadata module (upsert, lookup, delete)
- Call `hash_trainer_file()` on profile save when trainer path is non-empty
- Store hash result via new metadata function
- Add Tauri command `compute_trainer_hash` for on-demand recomputation
- Complexity: Medium | Files: ~5 | Dependencies: None

**Task Group 1C: Offline Readiness Check** (depends on 1A, 1B)

- Add `check_offline_readiness(profile: &GameProfile)` to `onboarding/readiness.rs`
- Check: trainer file exists, trainer hash cached, game exe exists, Proton path exists
- Return `ReadinessCheckResult` using existing `HealthIssue` pattern
- Add Tauri command `check_profile_offline_readiness`
- Add batch variant `check_all_profiles_offline_readiness`
- Complexity: Medium | Files: ~4 | Dependencies: 1A (trainer type informs checks), 1B (hash lookup)

### Phase 2: Launch Integration (Estimated: 2-3 days)

**Task Group 2A: Pre-Launch Hash Verification** (depends on Phase 1)

- Before launch, recompute trainer hash and compare against cached hash
- On mismatch: show warning dialog with "Update Hash" / "Cancel Launch" options
- Add `ValidationError::TrainerHashMismatch` variant to request.rs
- Wire into `validate()` function conditionally (only when hash is cached)
- Complexity: Medium | Files: ~4 | Dependencies: Phase 1 complete

**Task Group 2B: Offline-Aware Launch Validation** (can parallelize with 2A)

- Add offline-specific help text to validation errors (e.g., "This path is required for offline launch")
- Add `ValidationSeverity::OfflineWarning` for non-fatal offline issues
- Update `LaunchPanel.tsx` to display offline warnings distinctly
- Complexity: Low | Files: ~3 | Dependencies: Phase 1 complete

**Task Group 2C: Launch History Offline Preservation** (can parallelize with 2A, 2B)

- Ensure `launch_operations` table writes work when network is unavailable (already SQLite-based, so this is verification)
- Preserve `version_snapshots` correlation status from last online check
- Display "last verified: [date]" when correlation cannot be refreshed
- Complexity: Low | Files: ~2 | Dependencies: Phase 1 complete

### Phase 3: Community & Aurora (Estimated: 3-4 days)

**Task Group 3A: Community Tap Offline Cache** (can parallelize with 3B)

- Add "last synced at" display to `CommunityBrowser.tsx` using existing `community_taps.last_indexed_at`
- When sync fails (no network), fall back to cached `community_profiles` from SQLite
- Add clear UI messaging: "Showing cached profiles (last synced: [date])"
- Add manual "Refresh Cache" button that only appears when online
- Complexity: Medium | Files: ~4 | Dependencies: None (standalone)

**Task Group 3B: Aurora/WeMod Info Modal** (can parallelize with 3A)

- When trainer type is `Aurora` or `WeMod`, show info modal explaining offline requirements
- Content: "Aurora trainers require a one-time activation key. Ensure your key is activated while online before going offline."
- Modal should appear on first launch or when offline readiness check detects Aurora type
- Add `OfflineTrainerInfoModal.tsx` component
- Complexity: Low | Files: ~3 | Dependencies: Phase 1 (TrainerType enum)

**Task Group 3C: Community Profile Offline Readiness** (depends on 3A)

- Extend offline readiness check to include community tap staleness
- If tap hasn't synced in >30 days, add informational health issue
- Display offline readiness status on community profile cards
- Complexity: Low | Files: ~2 | Dependencies: 3A (cache metadata)

### Phase 4: UI Polish (Estimated: 2-3 days)

**Task Group 4A: Offline Status Indicators** (can parallelize with 4B)

- Add offline readiness badge to profile cards in `ProfilesPage.tsx`
- Badge states: "Offline Ready" (green), "Partial" (yellow), "Not Ready" (red)
- Integrate with existing `HealthBadge.tsx` pattern
- Complexity: Low | Files: ~3 | Dependencies: Phase 1 (readiness check)

**Task Group 4B: Graceful Degradation UI** (can parallelize with 4A)

- When network unavailable: community sync button shows "Offline - Using Cache"
- Settings page: disable tap add/remove when offline
- Launch page: show offline readiness summary alongside existing health checks
- Add `useNetworkStatus` hook (passive detection, re-checks on user action)
- Complexity: Medium | Files: ~5 | Dependencies: Phase 3 (cache fallback)

**Task Group 4C: Pre-Flight Dashboard** (depends on 4A, 4B)

- New "Offline Readiness" section on Health Dashboard page
- Shows all profiles with offline readiness status
- One-click "Prepare for Offline" that triggers hash computation for all profiles
- Complexity: Medium | Files: ~3 | Dependencies: Phase 1-3 complete

### Parallelization Map

```
Phase 1:  [1A: Trainer Type] ----\
          [1B: Hash Caching] -----+---> [1C: Offline Readiness]
                                            |
Phase 2:         [2A: Hash Verify] ---------+
                 [2B: Offline Validation] ---+  (all three parallel)
                 [2C: History Preserve] -----+
                                            |
Phase 3:  [3A: Tap Cache] ------\           |
          [3B: Aurora Modal] ----+---> [3C: Community Readiness]
                                            |
Phase 4:  [4A: Status Badges] --\           |
          [4B: Degradation UI] --+---> [4C: Pre-Flight Dashboard]
```

**Maximum parallelism within phases**: 2-3 agents working simultaneously.
**Cross-phase dependencies**: Each phase depends on the previous, but within a phase, task groups are largely independent.
**Total estimated effort**: 10-14 days of work, compressible to ~5-7 days with 2-3 parallel agents.

---

## Key Decisions Needed

1. **Trainer type enum vs capability model**: Recommendation is TrainerType enum (Option A), but the user should confirm before implementation begins.

2. **Hash verification behavior on mismatch**: Should hash mismatch block launch (strict) or warn and allow (permissive)? Recommendation: warn with option to update hash, since trainers do get legitimately updated.

3. **Community tap staleness threshold**: How many days before cached taps trigger a warning? Recommendation: 30 days for informational warning, never block usage.

4. **Network detection strategy**: Passive (try and handle failure) vs active (probe before attempting)? Recommendation: passive as default, with optional pre-flight active check.

5. **Migration numbering**: Next migration is version 13. Confirm no other in-flight features are claiming this migration number.

6. **Module placement**: Extend existing modules (metadata, onboarding, launch) vs create new `offline/` module? Recommendation: extend existing modules to avoid crate bloat.

---

## Open Questions

1. Should the `TrainerType` enum be extensible (with an `Other(String)` variant) or closed? Closed is simpler but may need updating as new trainer sources emerge.

2. Should offline readiness status be persisted in SQLite (like health snapshots) or computed on-demand? Persisted enables trend tracking; on-demand is simpler and always fresh.

3. How should the CLI (`crosshook-cli`) expose offline readiness? A `crosshook check-offline` subcommand would be natural but the CLI currently has 6/7 placeholder commands.

4. Should community profile manifests include a `trainer_sha256` field for cross-machine hash verification? This enables a "trust on first use" model where the community profile author's hash is the baseline.

5. Does Aurora trainer activation persist across Proton prefix rebuilds? If not, the offline readiness check must also verify prefix integrity for Aurora trainers.

---

## Relevant Files

### Backend (Rust)

- `crates/crosshook-core/src/profile/models.rs` -- `TrainerSection.kind` field (target for TrainerType enum)
- `crates/crosshook-core/src/metadata/version_store.rs` -- `hash_trainer_file()`, existing SHA-256 implementation
- `crates/crosshook-core/src/metadata/profile_sync.rs` -- `sha256_hex()`, reusable hashing utility
- `crates/crosshook-core/src/metadata/migrations.rs` -- Migration pattern (next: version 13)
- `crates/crosshook-core/src/metadata/models.rs` -- `VersionSnapshotRow`, `VersionCorrelationStatus`
- `crates/crosshook-core/src/community/taps.rs` -- `CommunityTapStore`, Git sync operations
- `crates/crosshook-core/src/community/index.rs` -- `CommunityProfileIndex`, tap indexing
- `crates/crosshook-core/src/launch/request.rs` -- `validate()`, `ValidationError` enum
- `crates/crosshook-core/src/onboarding/readiness.rs` -- `check_system_readiness()`, readiness check pattern
- `crates/crosshook-core/src/settings/mod.rs` -- `AppSettingsData` (4 fields, extensible)
- `crates/crosshook-core/src/profile/community_schema.rs` -- `CommunityProfileManifest` (potential `trainer_sha256` field)

### Frontend (React/TypeScript)

- `src/types/profile.ts` -- `GameProfile` interface, `TrainerLoadingMode` type
- `src/components/CommunityBrowser.tsx` -- Community tap browsing (add cache status)
- `src/components/LaunchPanel.tsx` -- Launch controls (add offline warnings)
- `src/components/HealthBadge.tsx` -- Health badge pattern (reuse for offline badge)
- `src/components/ProfileFormSections.tsx` -- Profile editor (add trainer type dropdown)
- `src/components/pages/HealthDashboardPage.tsx` -- Health dashboard (add offline readiness section)
- `src/hooks/useLaunchState.ts` -- Launch state hook (extend for offline checks)

### Tauri IPC

- `src-tauri/src/commands/launch.rs` -- Launch commands (add hash verification)
- `src-tauri/src/commands/community.rs` -- Community commands (add cache fallback)
- `src-tauri/src/commands/health.rs` -- Health commands (add offline readiness)
