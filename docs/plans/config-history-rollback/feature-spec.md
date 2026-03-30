# Feature Spec: Config History Rollback

## Executive Summary

CrossHook needs a first-class configuration history workflow so users can answer "what changed since this profile last worked?" and recover quickly. Issue `#46` defines the required user value (auto snapshots, diff, rollback, and last-known-good tagging), and the project now has a metadata SQLite DB that did not exist when earlier research was drafted. The recommended architecture is metadata-backed, append-only snapshot history keyed by stable `profile_id`, while TOML remains the live profile source of truth. This gives rename-safe history, efficient list/diff queries, and transactional retention without introducing a second live config format. MVP should ship a bounded, safe vertical slice: snapshot capture on key write paths, history listing, diff against selected versions/current, one-click rollback with confirmation, and known-good tagging based on successful launches.

## Feature Context

- **Issue:** [feat(profiles): configuration history with diff and rollback support](https://github.com/yandy-r/crosshook/issues/46)
- **Feature name:** `config-history-rollback`
- **Product problem:** users cannot currently determine which profile changes caused regressions.
- **Important constraint update:** previous research assumed filesystem-only persistence; CrossHook now has `MetadataStore` and should use it as the primary history persistence layer.

## Goals

1. Capture immutable config snapshots automatically around profile mutations.
2. Let users list and inspect prior snapshots with timestamps and source.
3. Provide compare/diff between snapshots and current state.
4. Allow safe rollback to a selected snapshot.
5. Mark and surface "last known working" baseline.
6. Enforce bounded retention to control storage growth.

## Non-Goals (MVP)

- Patch-only/delta-chain storage.
- Cross-device sync of history.
- Multi-user or remote actor identity.
- Advanced semantic merge UI.
- Full policy customization UI for retention and tagging logic.

## Existing System Fit

- `ProfileStore` writes full TOML profile state (`profile/toml_store.rs`).
- Tauri commands already centralize write flows (`src-tauri/src/commands/profile.rs`).
- `MetadataStore` already handles launch history, version snapshots, profile identity/rename lineage, and retention-style stores (`metadata/mod.rs`, `metadata/version_store.rs`).
- `profile_sync::observe_profile_write` already computes content hashes and maps writes to stable `profile_id`.

This makes config history a natural addition to the metadata subsystem rather than a separate filesystem history tree.

## Requirements

## Functional Requirements

1. **Auto snapshot on profile mutation**
   - Capture a snapshot after successful persistence for save-like flows.
   - Deduplicate no-op writes based on content hash.
2. **History listing**
   - Return recent snapshots per profile with metadata (`created_at`, source, known-good flag).
3. **Diff**
   - Compare two snapshots or snapshot vs current persisted profile.
4. **Rollback**
   - Restore selected snapshot through normal profile save flow.
   - Record rollback as a new snapshot event (non-destructive history).
5. **Known-good**
   - Tag baseline snapshot associated with successful launch outcome.
6. **Retention**
   - Keep only bounded recent snapshots per profile with deterministic pruning.

## Non-Functional Requirements

- **Safety:** no partial rollback should leave corrupted profile state.
- **Performance:** history list and diff operations should feel immediate for normal profile sizes.
- **Storage bounds:** strict per-profile cap and payload limits.
- **Graceful degradation:** profile editing still works if metadata store is unavailable; history UI reports unavailable state.
- **Auditability:** rollback operations are traceable and debuggable.

## Recommended Technical Design

## Persistence Model (Recommended)

Use SQLite metadata DB for immutable revision history, keyed by `profile_id`.

Suggested table (`config_revisions`):

- `id INTEGER PRIMARY KEY AUTOINCREMENT`
- `profile_id TEXT NOT NULL` (FK to `profiles.profile_id`)
- `profile_name_at_write TEXT NOT NULL`
- `source TEXT NOT NULL` (`manual_save`, `launch_optimization_save`, `preset_apply`, `rollback_apply`, `import`, `migration`, etc.)
- `content_hash TEXT NOT NULL`
- `snapshot_toml TEXT NOT NULL`
- `source_revision_id INTEGER NULL` (lineage for rollback)
- `is_last_known_working INTEGER NOT NULL DEFAULT 0`
- `created_at TEXT NOT NULL`

Indexes:

- `(profile_id, created_at DESC)`
- `(profile_id, id DESC)`
- `(profile_id, content_hash)`
- optional partial index for `(profile_id)` where `is_last_known_working = 1`

Retention:

- Start with fixed cap (recommended 20; issue suggests minimum viable 5).
- Prune in same transaction as insert.

## Write Hook Strategy

Hook snapshot appends where profile writes already occur:

- `profile_save`
- `profile_save_launch_optimizations`
- `profile_apply_bundled_optimization_preset`
- `profile_save_manual_optimization_preset`
- `profile_import_legacy`
- migration-driven profile writes
- rollback command path (as `rollback_apply`)

Order for save-like flows:

1. Save TOML via `ProfileStore`.
2. Reload persisted profile for canonical state.
3. `observe_profile_write` to refresh metadata/profile identity/hash.
4. Append deduped revision row.
5. Emit frontend event.

## New Tauri Command Surface (Proposed)

- `profile_list_config_history(name, limit?, before_id?) -> Vec<ConfigRevisionSummary>`
- `profile_diff_config_history(name, left_revision_id, right_revision_id?) -> ConfigDiff`
- `profile_rollback_config_revision(name, revision_id) -> GameProfile`
- optional: `profile_mark_config_known_good(name, revision_id)` for manual tagging

Diff strategy (MVP):

- Use line-based unified diff output with summary counts.
- Prefer `similar` crate for backend diff generation.
- Keep semantic TOML diffs as follow-up enhancement.

## Known-Good Tagging

Default MVP policy:

- On successful launch completion, mark latest matching revision for profile as known-good.
- Ensure one active known-good pointer per profile (supersede older flag).
- Keep full historical rows immutable.

## Rollback Semantics

Rollback must:

1. Validate selected snapshot belongs to resolved `profile_id`.
2. Parse/deserialize snapshot payload into `GameProfile` before write.
3. Write via `ProfileStore.save`.
4. Re-run metadata observe write.
5. Append rollback snapshot row with lineage (`source_revision_id`).
6. Emit `profiles-changed`.

If metadata is unavailable, rollback history APIs should fail clearly (save/edit remains available).

## UX Specification (MVP)

## Entry and Navigation

- Add `History` action in profile action group.
- Open history as side panel on wide layouts and full-screen modal/sheet on narrow/Deck layouts.

## History Panel

- Timeline list (newest first): timestamp, source badge, known-good marker.
- Selection details with actions:
  - `Compare with current`
  - `Compare with...`
  - `Restore snapshot`

## Diff UX

- Compare modal with left/right selectors.
- Changed-only toggle.
- Display grouped sections and clear added/removed/changed markers.

## Restore UX

- Explicit confirmation dialog with snapshot timestamp/context.
- Default-on pre-restore checkpoint behavior.
- Success toast with undo affordance when possible.

## States and Accessibility

- Loading skeletons for list and diff.
- Clear empty state and retryable error states.
- Keyboard navigation, focus restoration, and status/alert live regions.
- Do not rely on color only for diff state.

## Security and Risk Assessment

## CRITICAL

1. **Snapshot integrity/applicability checks**
   - verify payload hash and profile ownership before apply.
2. **Rollback audit trail**
   - record before/after hashes and result state.
3. **Resource exhaustion controls**
   - strict count/size/rate limits for history and diff requests.

## WARNING

1. Path traversal and secondary path handling in any sidecar mode.
2. Sensitive-value exposure in logs/history rendering.
3. Ambiguity in known-good tagging for indeterminate launch outcomes.

## ADVISORY

1. Optional stronger tamper evidence (e.g., HMAC) if threat model requires.
2. Optional encryption strategy for future sensitive profile fields.

## Acceptance Criteria

1. Saving a modified profile creates an addressable history revision (deduped when unchanged).
2. User can list revisions with timestamps and source metadata.
3. Diff accurately reports changes between selected revisions or revision vs current.
4. Rollback restores selected snapshot content and records rollback lineage.
5. A known-good marker is set from successful launch logic and surfaced in history.
6. Retention cap is enforced automatically.
7. Rename keeps history continuity via stable profile identity.
8. Metadata-unavailable state fails gracefully without crashing profile editing.

## Decisions Needed Before Planning

1. **Retention default:** 5 (issue minimum) vs 20 (safer troubleshooting window).
2. **Known-good policy:** strict clean success only vs broader success semantics.
3. **Initial scope of tracked writes:** only full saves/import/rollback vs include optimization/preset writes in MVP.
4. **Diff output level:** text unified diff only vs field-aware semantic diff in MVP.
5. **Unavailable-mode UX:** hide history controls vs show disabled with explanation.

## Implementation Preview (For `/plan-workflow`)

1. Add migration + metadata module for config revisions.
2. Add MetadataStore wrappers + row DTOs.
3. Wire snapshot capture into profile write command paths.
4. Add diff and rollback command endpoints.
5. Add frontend types + hook methods.
6. Add history panel/modal + diff/restore interactions in profile UI.
7. Add tests:
   - metadata unit tests (insert/dedup/prune/query),
   - integration tests with temp profile store + in-memory metadata,
   - rollback correctness and known-good tagging.

## Research Artifacts

- `docs/plans/config-history-rollback/research-external.md`
- `docs/plans/config-history-rollback/research-business.md`
- `docs/plans/config-history-rollback/research-technical.md`
- `docs/plans/config-history-rollback/research-ux.md`
- `docs/plans/config-history-rollback/research-security.md`
- `docs/plans/config-history-rollback/research-practices.md`
- `docs/plans/config-history-rollback/research-recommendations.md`
