# Config History Rollback - Business Requirements

## Overview

Issue #46 asks a practical support question users cannot currently answer quickly: **"what changed since last working config?"** In CrossHook's Steam Deck/Linux workflow, users frequently adjust profile settings (launch method, trainer path, Proton/runtime fields, optimizations) between launches, then lose track of which change introduced breakage.

CrossHook now has the right substrate to solve this: profile source-of-truth in TOML files (`~/.config/crosshook/profiles/*.toml`) and operational metadata in SQLite (launch outcomes, last success timestamps, version snapshots, profile identity/history). This feature should connect those two systems into a user-facing "snapshot -> compare -> restore" workflow with explicit guardrails and retention.

## Codebase Evidence (Current State)

- **Profiles are TOML and overwritten on save**: `ProfileStore::save` writes full profile content to `<name>.toml`; no built-in revision chain in file storage.
- **App save entrypoint exists**: Tauri `profile_save` writes TOML then calls metadata `observe_profile_write`.
- **Metadata already tracks launch outcomes and last success**:
  - `launch_operations` records `started/succeeded/failed/abandoned`.
  - `query_last_success_for_profile` and `query_last_success_per_profile` already exist.
- **Metadata tracks stable profile identity and rename lineage**:
  - `profiles.profile_id` plus `profile_name_history`.
  - Important for keeping snapshot history across profile rename.
- **Metadata records content hash per profile write**:
  - `observe_profile_write` computes/stores TOML hash in `profiles.content_hash`.
  - This proves write events are already observed and can trigger snapshot bookkeeping.
- **Version mismatch acknowledgement exists**:
  - `acknowledge_version_change` marks latest version snapshot status as `matched`.
  - Similar concept can inspire "last-known-good config" acknowledgement semantics.
- **Metadata is intentionally fail-soft/optional**:
  - `MetadataStore::disabled()` returns defaults/no-ops.
  - Rollback domain requirements must define behavior when metadata is unavailable.

## Problem Framing (Steam Deck/Linux Launcher Context)

### User pain

- Users tune launch options incrementally (especially `proton_run` + optimizations).
- A later run fails, but the app only shows current config and launch diagnostics, not config delta from prior working state.
- On Steam Deck, sessions are interrupt-driven; users often make multiple quick edits without documenting changes.

### Business impact

- Higher support burden ("it worked yesterday, now it does not").
- Lower trust in profile editing and optimization features.
- Increased churn to external notes/manual file backups instead of in-app workflows.

### Product goal

Enable users to confidently answer:

1. Which config change likely caused regression?
2. What was the last known working config?
3. Can I restore that state safely and quickly?

## User Stories

1. **As a Steam Deck player**, before I save profile edits, I want CrossHook to auto-capture a snapshot so I can undo bad changes without manual file backups.
2. **As a Linux power user**, when a launch fails, I want to compare current config against my last successful configuration and see meaningful field-level diffs.
3. **As a troubleshooting user**, I want to tag a config as "last known good" after successful verification so future diffs anchor to a trusted baseline.
4. **As a storage-conscious user**, I want old snapshots pruned automatically by clear retention rules so metadata does not grow unbounded.
5. **As a cautious user**, I want rollback to be explicit and auditable, not silent, so I know exactly what was restored.

## Functional Requirements

### FR-1: Auto snapshot before profile modification

- On profile-changing operations (at minimum full `profile_save`; optionally launch-optimization partial writes), system creates a pre-change snapshot tied to `profile_id` and timestamp.
- Snapshot payload must be enough to restore exact TOML state (not only hash).
- Snapshot creation must occur before destructive overwrite of prior profile content.

### FR-2: Last-known-good tagging

- System can assign "known good" marker to a snapshot.
- Default strategy: automatically tag on successful launch completion for selected profile.
- Manual override: user can mark current config as known good even if automatic tagging is unavailable/ambiguous.
- At most one active "known good" pointer per profile (new tag supersedes older one, history retained).

### FR-3: Diff view against baseline

- User can compare:
  - current config vs last-known-good snapshot;
  - current config vs selected prior snapshot.
- Diff granularity: field-level across major sections (`game`, `trainer`, `injection`, `steam`, `runtime`, `launch`).
- UX should flag high-risk changed fields (launch method, executable/trainer paths, Proton/runtime paths, optimization IDs/presets).

### FR-4: Rollback execution

- User can restore selected snapshot to active profile.
- Rollback writes restored TOML via normal save pathway and emits profile-changed events.
- Rollback operation itself must create a new snapshot/audit event (so rollback is reversible and traceable).

### FR-5: Retention policy

- Enforce bounded snapshot history per profile using deterministic pruning.
- MVP retention can be count-based (e.g., keep N latest + preserve tagged known-good).
- Deleting or tombstoning profile metadata should make snapshot retrieval inaccessible for active UI usage.

### FR-6: History visibility and metadata

- Snapshot list includes timestamp, source reason (auto-save/rollback/manual tag/import), and quick status hints (known-good, pre-change, post-rollback).
- If launch outcome data exists, show nearest success/failure context to aid triage.

## Non-Functional Requirements

### NFR-1: Safety and integrity

- No partial write should leave profile in invalid/empty state.
- Rollback and snapshot creation must be transactional at metadata layer where possible.

### NFR-2: Performance (Deck-friendly)

- Snapshot and diff operations should feel immediate for normal profile sizes.
- UI must remain responsive during autosave-heavy editing sessions.

### NFR-3: Storage bounds

- Snapshot storage growth must be capped by retention policy.
- Large snapshot payloads should not degrade startup or health dashboard operations.

### NFR-4: Explainability

- User-facing labels/messages must explain "what changed" in domain terms, not raw storage internals.

### NFR-5: Graceful degradation

- If metadata DB unavailable (supported architecture), app must fail soft:
  - profile save still works;
  - rollback/history UI clearly indicates unavailable state;
  - no silent data-loss claims.

### NFR-6: Privacy/local-first

- Snapshot data remains local (same profile privacy model as current TOML + local metadata DB).
- No telemetry requirement.

## Acceptance Criteria (Refined, Testable)

### AC-1 Auto snapshot on save

- Given an existing profile, when user saves modified profile, then a new snapshot representing **pre-save state** exists and can be listed for that profile.
- Verify by modifying a field, saving, then rollback to latest snapshot restores prior value.

### AC-2 Known-good tagging on successful launch

- Given a profile launch recorded as succeeded, when launch completion persists, then current config (or corresponding pre/post snapshot policy) is tagged as known good.
- Verify last-known-good pointer updates after success and is queryable by profile.

### AC-3 Diff correctness

- Given two snapshots with differences in at least 3 sections, when user opens diff, then changed fields are reported accurately with old/new values and unchanged fields are omitted/collapsed.

### AC-4 Rollback correctness and auditability

- Given snapshot S and current config C, when user rolls back to S, then on-disk TOML equals S content and history records rollback event with timestamp/user action source.

### AC-5 Retention enforcement

- Given >N snapshots for a profile, when new snapshot is created, then oldest non-protected snapshots are pruned and list length respects policy.
- Known-good snapshot is retained even when older than retention window (if policy says pinned).

### AC-6 Rename continuity

- Given profile rename, when user opens config history, then prior snapshots remain associated with renamed profile (via stable profile identity, not filename only).

### AC-7 Fail-soft metadata unavailability

- Given metadata store disabled/unavailable, when user saves profile, then save succeeds and history/rollback UI presents explicit "feature unavailable" state without crash.

## Edge Cases and Operational/Business Risks

1. **Autosave burst writes**: launch optimization autosave can generate many writes quickly; without debouncing-aware policy this may create noisy snapshot spam.
2. **Concurrent writes/races**: existing note in `save_launch_optimizations` states last-write-wins; snapshot capture must avoid mismatching snapshot->result ordering.
3. **Ambiguous "working" signal**: `steam_applaunch` may be marked succeeded on indeterminate helper exit; known-good automation may over-tag.
4. **Rollback to incompatible environment**: historical config may reference stale paths/old Proton; rollback should restore config but still surface health warnings.
5. **Storage bloat**: full TOML snapshot per edit can grow DB; retention and optional compression may become necessary.
6. **Profile delete semantics**: metadata soft-deletes profiles; requirements must define whether snapshot data is hidden-only or physically purged.
7. **User confusion with manual edits outside app**: direct TOML edits can bypass expected snapshot points unless filesystem-scan hooks are included in scope.
8. **Corrupt or partial snapshot records**: diff/rollback flow must validate snapshot payload and provide recoverable error states.

## Priority and Scope

## MVP (Issue #46 core value)

1. Auto snapshot before full profile save.
2. Known-good marker from successful launch (plus manual mark action as fallback).
3. Current vs known-good diff UI.
4. One-click rollback to selected snapshot.
5. Count-based retention with known-good protection.
6. Basic failure-safe messaging when metadata unavailable.

## Follow-up (Post-MVP)

1. Snapshot capture for partial writes (`profile_save_launch_optimizations`) with noise controls.
2. Rich diff UX (section filters, semantic labels, risk scoring).
3. "Rollback preview" impact checks (path existence, launch-method compatibility).
4. Import/manual-file-edit detection and snapshoting via filesystem sync pipeline.
5. Time-window + count hybrid retention and user-configurable retention policy.
6. Exportable history bundle for support/debug workflows.

## Suggested Requirement Decisions Needed Before Implementation

1. **Snapshot granularity**: full profile snapshot vs section-level snapshot.
2. **Known-good trigger**: strict clean success only vs include indeterminate success paths.
3. **Retention defaults**: per-profile max snapshot count and known-good pinning policy.
4. **Rollback semantics**: hard replace full profile always, or allow selective section rollback.
5. **Metadata unavailable behavior**: hide feature entirely vs read-only shell with explanation.
