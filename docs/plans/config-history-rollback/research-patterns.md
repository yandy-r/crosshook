# Config History Rollback - Pattern Research

## Existing patterns to reuse

## Metadata module pattern

- Keep SQL and row mapping in a focused metadata submodule (similar to `version_store.rs`, `health_store.rs`, `launch_history.rs`).
- Expose thin facade methods from `MetadataStore` in `metadata/mod.rs`.
- Keep DTO/row types in metadata models for command boundary reuse.

## Write-hook pattern in Tauri commands

- Existing profile commands already follow:
  - save/mutate in `ProfileStore`,
  - sync metadata with `observe_profile_write`,
  - emit `profiles-changed`.
- Config history should plug into this same command-level boundary to avoid drift and race-prone duplicate logic.

## Retention and bounded history pattern

- Existing version snapshot logic uses bounded per-profile retention.
- Apply same policy style:
  - insert revision,
  - prune old rows in same transaction,
  - preserve deterministic order (`created_at`, id).

## Fail-soft metadata pattern

- Metadata store can be disabled and returns defaults/no-ops.
- Continue this behavior for non-critical save operations.
- Use explicit, user-facing failures for history-specific commands that require metadata.

## Identity continuity pattern

- Profile lifecycle already uses stable `profile_id` with rename lineage.
- History rows must key on `profile_id` to remain continuous across rename events.

## Recommended implementation patterns

1. **Dedup by hash**: compare latest revision hash before insert.
2. **Append-only history**: rollback adds a new revision; never overwrite old rows.
3. **Structured command DTOs**: summaries for list, full payload for detail/diff.
4. **Diff-first in backend**: Rust computes diff payload; frontend renders.
5. **Conservative MVP UI**: action entry point in profile actions, side panel/modal for timeline, explicit restore confirmation.

## Avoid for MVP

- Patch-chain storage or complex merge logic.
- Separate sidecar history tree as primary storage.
- Broad generic abstraction before a second consumer exists.
