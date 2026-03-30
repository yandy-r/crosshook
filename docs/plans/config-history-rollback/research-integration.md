# Config History Rollback - Integration Research

## Integration surfaces

## Core metadata integration

- `crates/crosshook-core/src/metadata/migrations.rs`
  - Add schema migration for `config_revisions`.
- `crates/crosshook-core/src/metadata/mod.rs`
  - Add facade methods for list/get/diff inputs/append/rollback lineage and known-good tagging.
- `crates/crosshook-core/src/metadata/models.rs`
  - Add row + DTO-facing structs and retention constants.

## Profile lifecycle integration

- `src/crosshook-native/src-tauri/src/commands/profile.rs`
  - Hook snapshot append into write commands:
    - `profile_save`
    - `profile_save_launch_optimizations`
    - `profile_apply_bundled_optimization_preset`
    - `profile_save_manual_optimization_preset`
    - `profile_import_legacy`
  - Add new commands:
    - `profile_list_config_history`
    - `profile_diff_config_history`
    - `profile_rollback_config_revision`
    - optional `profile_mark_config_known_good`

## Launch success integration

- `launch_operations` and last-success queries already exist in metadata.
- Tag known-good revision from successful launch completion path.
- Maintain one active known-good marker per profile while preserving full immutable history.

## Frontend integration

- `src/crosshook-native/src/types/`
  - Add typed models for revision summary, diff response, rollback result.
- `src/crosshook-native/src/hooks/useProfile.ts` (or dedicated hook)
  - Add history list/diff/rollback actions with loading/error state.
- `src/crosshook-native/src/components/ProfileActions.tsx`
  - Add `History` action button.
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`
  - Add history panel/modal, compare flow, restore confirmation, and success/error toasts.

## Event and refresh integration

- Reuse `profiles-changed` event after rollback apply.
- Trigger profile and health refresh on restore completion.
- Keep compare action read-only and side-effect free.

## Testing integration

- `crosshook-core` unit tests:
  - metadata insert/dedup/prune/list/lineage.
- command integration tests:
  - save -> history row,
  - rollback -> restored TOML + appended rollback row.
- UI behavior tests (if added later):
  - loading/empty/error states and confirmation gating.
