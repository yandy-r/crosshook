# Shared Context - config-history-rollback

## Overview

`config-history-rollback` addresses issue `#46` by adding a first-class snapshot, diff, and rollback workflow for profile configuration changes. CrossHook now has a mature metadata SQLite layer, so revision history should be stored there and keyed by stable `profile_id`, while TOML remains the live profile source of truth. Existing save and metadata sync paths already provide reliable hook points for snapshot capture, dedup, retention, and rollback lineage. The safest MVP is append-only revisions, line-based diff output, explicit restore confirmation, known-good tagging from successful launches, and clear degraded behavior when metadata is unavailable.

## Product and UX requirements

- User problem: inability to answer "what changed since last working config?"
- Required value:
  - automatic snapshots around profile mutations,
  - revision timeline and compare,
  - safe rollback,
  - known-good tagging.
- UX baseline:
  - `History` entry in profile actions,
  - timeline panel/modal with compare and restore actions,
  - confirmation + undo-oriented restore affordances,
  - loading/empty/error/accessibility-compliant states.

## Architecture and data model

- **Live state**: profile TOML files in `ProfileStore`.
- **History state**: append-only `config_revisions` table in metadata DB.
- **Identity key**: `profile_id`, not filename, to preserve history through rename.
- **Core row attributes**:
  - revision id,
  - `profile_id`,
  - `source`,
  - `content_hash`,
  - `snapshot_toml`,
  - optional lineage pointer (`source_revision_id`),
  - known-good marker,
  - timestamps.
- **Retention and dedup**:
  - skip insert when hash matches latest revision,
  - prune old rows in the same transaction.

## Existing implementation anchors (verified paths)

- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`
- `src/crosshook-native/src-tauri/src/commands/profile.rs`
- `src/crosshook-native/src-tauri/src/lib.rs`
- `src/crosshook-native/src/hooks/useProfile.ts`
- `src/crosshook-native/src/components/ProfileActions.tsx`
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`
- `src/crosshook-native/src/types/`

## Security and reliability constraints

- Verify snapshot ownership and integrity before rollback apply.
- Keep rollback auditable by appending a new revision (never destructive rewrites).
- Enforce resource controls (count/size/rate bounds) to prevent abuse.
- Maintain fail-soft save behavior when metadata is unavailable.
- Expose clear UI error states for unavailable history and rollback failures.

## Decisions to lock before implementation

1. Retention default (`5` vs `20`).
2. Known-good trigger strictness.
3. Write-source coverage included in MVP.
4. Diff output level for MVP (line-based only or field-aware).
5. UI behavior when metadata is unavailable.

## Artifacts used

- `docs/plans/config-history-rollback/feature-spec.md`
- `docs/plans/config-history-rollback/research-architecture.md`
- `docs/plans/config-history-rollback/research-patterns.md`
- `docs/plans/config-history-rollback/research-integration.md`
- `docs/plans/config-history-rollback/research-docs.md`
- `docs/plans/config-history-rollback/research-business.md`
- `docs/plans/config-history-rollback/research-technical.md`
- `docs/plans/config-history-rollback/research-security.md`
- `docs/plans/config-history-rollback/research-ux.md`
- `docs/plans/config-history-rollback/research-practices.md`
- `docs/plans/config-history-rollback/research-external.md`
- `docs/plans/config-history-rollback/research-recommendations.md`
