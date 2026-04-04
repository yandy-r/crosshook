# Prefix Storage Persistence Follow-Up (Issue #61)

## Status

- [ ] Planned
- [ ] Schema reviewed
- [ ] Migration implemented
- [ ] Backend persistence wired
- [ ] UI freshness/history wired
- [ ] Verified and shipped

## Scope

This document tracks the **deferred persistence phase** for Issue `#61`.

V1 shipped live prefix scanning, orphan detection, stale staged-trainer cleanup, and low-disk launch warnings without introducing new SQLite tables.

This follow-up adds durable scan snapshots and cleanup audit history.

## Storage Boundary

| Data                         | Layer              | Notes                                         |
| ---------------------------- | ------------------ | --------------------------------------------- |
| Low-space threshold override | `settings.toml`    | Optional user-editable preference (MiB)       |
| Last scan snapshot rows      | SQLite metadata DB | Derived filesystem metrics, not user-authored |
| Cleanup audit log            | SQLite metadata DB | Operational history for support/debugging     |
| In-progress scan state       | Runtime memory     | Never persisted                               |

## Persistence and Usability Requirements

- Migration must be additive and backward-compatible.
- Older app versions must safely ignore new rows/tables.
- If metadata DB is unavailable, feature remains functional with live scan; UI shows non-blocking degraded-state message.
- No cleanup operation should depend on metadata availability.
- Users can view persisted freshness/history, but raw rows are not directly edited.

## Proposed Schema

## 1) `prefix_storage_snapshots`

- `id` TEXT PRIMARY KEY
- `resolved_prefix_path` TEXT NOT NULL
- `total_bytes` INTEGER NOT NULL
- `staged_trainers_bytes` INTEGER NOT NULL
- `is_orphan` INTEGER NOT NULL
- `referenced_profiles_json` TEXT NOT NULL
- `stale_staged_count` INTEGER NOT NULL
- `scanned_at` TEXT NOT NULL

Indexes:

- `idx_prefix_storage_snapshots_prefix_path_scanned_at` on (`resolved_prefix_path`, `scanned_at` DESC)
- `idx_prefix_storage_snapshots_scanned_at` on (`scanned_at` DESC)

## 2) `prefix_storage_cleanup_audit`

- `id` TEXT PRIMARY KEY
- `target_kind` TEXT NOT NULL (`orphan_prefix` / `stale_staged_trainer`)
- `resolved_prefix_path` TEXT NOT NULL
- `target_path` TEXT NOT NULL
- `result` TEXT NOT NULL (`deleted` / `skipped`)
- `reason` TEXT NULL
- `reclaimed_bytes` INTEGER NOT NULL
- `created_at` TEXT NOT NULL

Indexes:

- `idx_prefix_storage_cleanup_audit_created_at` on (`created_at` DESC)
- `idx_prefix_storage_cleanup_audit_prefix_path` on (`resolved_prefix_path`)

## Implementation Tasks

## Phase A: Migration + Models

- [ ] Add migration in `crosshook-core` metadata migrations module.
- [ ] Add Rust row models for snapshot and cleanup audit.
- [ ] Add store API methods:
  - [ ] `upsert_prefix_storage_snapshots(...)`
  - [ ] `list_latest_prefix_storage_snapshots(...)`
  - [ ] `insert_prefix_storage_cleanup_audit(...)`
  - [ ] `list_prefix_storage_cleanup_audit(...)`

## Phase B: Command Wiring

- [ ] Update `scan_prefix_storage` command to persist snapshot rows on successful scan.
- [ ] Update `cleanup_prefix_storage` command to persist one audit row per processed target.
- [ ] Keep writes fail-soft (`warn` log + continue).

## Phase C: Frontend Freshness + History

- [ ] Add freshness indicator (`live` vs `cached`, timestamp).
- [ ] Add cleanup history list in Settings section.
- [ ] Show explicit degraded banner when metadata persistence is unavailable.

## Phase D: Settings (Optional)

- [ ] Add `prefix_low_disk_warning_threshold_mb` to `settings.toml` + IPC contracts.
- [ ] Default to `2048` MiB when unset.
- [ ] Use this value in launch advisory logic.

## Verification Plan

- [ ] Migration unit tests: create, upgrade, and compatibility checks.
- [ ] Store method tests: insert/read snapshot and audit rows.
- [ ] Command tests: persistence write failures do not fail command responses.
- [ ] Manual:
  - [ ] run scan, restart app, verify snapshot freshness reads
  - [ ] run cleanup, verify audit entries
  - [ ] simulate metadata disabled, verify live scan + cleanup still work

## Out of Scope

- Cross-machine sync of scan snapshots.
- Background scheduled scans.
- Automatic cleanup without user confirmation.
