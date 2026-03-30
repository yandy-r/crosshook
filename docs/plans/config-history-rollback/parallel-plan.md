# Parallel Implementation Plan - config-history-rollback

## Overview

This plan implements configuration revision history in CrossHook using metadata-backed snapshots keyed by stable `profile_id`, while preserving TOML as the only live profile source of truth. The MVP delivers a safe vertical slice: deduped revision capture on profile writes, history listing, diff against revision/current, rollback with lineage, and known-good tagging support. Work is sequenced to reduce integration risk: schema and store first, command orchestration second, frontend integration third, then hardening and validation. Security and reliability controls are embedded in each phase so rollback remains auditable, bounded, and failure-tolerant.

## Critically Relevant Files and Documentation

- `docs/plans/config-history-rollback/feature-spec.md`
- `docs/plans/config-history-rollback/shared.md`
- `docs/plans/config-history-rollback/analysis-context.md`
- `docs/plans/config-history-rollback/analysis-code.md`
- `docs/plans/config-history-rollback/analysis-tasks.md`
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`
- `src/crosshook-native/src-tauri/src/commands/profile.rs`
- `src/crosshook-native/src-tauri/src/lib.rs`
- `src/crosshook-native/src/types/`
- `src/crosshook-native/src/hooks/useProfile.ts`
- `src/crosshook-native/src/components/ProfileActions.tsx`
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`

## Implementation Plan

## Phase 1 - Metadata Revision Foundation

### Task 1.1: Add config revision schema and retention constants

**Dependencies:** None  
**READ THESE BEFORE TASK:**

- `docs/plans/config-history-rollback/feature-spec.md`
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`

**Files to Create:**

- none

**Files to Modify:**

- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`

**Instructions:**

1. Add migration for `config_revisions` with fields needed for snapshot payload, source, hash, lineage, known-good marker, and timestamps.
2. Add indexes for profile-order retrieval and dedup checks.
3. Add retention limit constants and helper types in metadata models.
4. Keep migration style and version bumps consistent with existing metadata patterns.

### Task 1.2: Implement metadata store for config revisions

**Dependencies:** Task 1.1  
**READ THESE BEFORE TASK:**

- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`

**Files to Create:**

- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store.rs`

**Files to Modify:**

- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`

**Instructions:**

1. Add insert/list/get/prune methods with profile-scoped queries and deterministic ordering.
2. Implement dedup by comparing with latest revision hash.
3. Add known-good marker mutation with single-active-marker semantics per profile.
4. Expose thin `MetadataStore` wrappers and avoid SQL in callers.

### Task 1.3: Add metadata-level tests for revision semantics

**Dependencies:** Task 1.2  
**READ THESE BEFORE TASK:**

- `docs/plans/config-history-rollback/research-technical.md`
- `docs/plans/config-history-rollback/research-security.md`

**Files to Create:**

- none

**Files to Modify:**

- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (test module only if needed)

**Instructions:**

1. Cover insert/list order, dedup, pruning, and lineage fields.
2. Validate known-good supersede behavior.
3. Include metadata unavailable/disabled behavior checks where relevant.

## Phase 2 - Tauri Command Surface and Rollback Orchestration

### Task 2.1: Add shared snapshot-capture helper in profile command flow

**Dependencies:** Task 1.2  
**READ THESE BEFORE TASK:**

- `src/crosshook-native/src-tauri/src/commands/profile.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`
- `docs/plans/config-history-rollback/research-patterns.md`

**Files to Create:**

- none

**Files to Modify:**

- `src/crosshook-native/src-tauri/src/commands/profile.rs`

**Instructions:**

1. Implement one internal helper that captures canonical post-save profile, syncs metadata, and appends deduped revision.
2. Wire helper into selected write flows (`profile_save`, import and rollback paths at minimum; include optimization/preset paths if MVP decision is yes).
3. Keep existing event emission behavior intact.

### Task 2.2: Implement history list, diff, and rollback commands

**Dependencies:** Task 2.1  
**READ THESE BEFORE TASK:**

- `docs/plans/config-history-rollback/feature-spec.md`
- `docs/plans/config-history-rollback/research-security.md`
- `src/crosshook-native/src-tauri/src/lib.rs`

**Files to Create:**

- none

**Files to Modify:**

- `src/crosshook-native/src-tauri/src/commands/profile.rs`
- `src/crosshook-native/src-tauri/src/lib.rs`

**Instructions:**

1. Add commands for list/diff/rollback (and mark-known-good if included in MVP).
2. Rollback path must verify profile ownership, parse snapshot payload, write via `ProfileStore`, and append rollback lineage row.
3. Return typed payloads and predictable error messaging for frontend handling.

### Task 2.3: Integrate known-good tagging from launch success path

**Dependencies:** Task 1.2  
**READ THESE BEFORE TASK:**

- `docs/plans/config-history-rollback/research-business.md`
- `docs/plans/config-history-rollback/research-technical.md`
- launch-related command files under `src/crosshook-native/src-tauri/src/commands/`

**Files to Create:**

- none

**Files to Modify:**

- relevant launch command file(s) under `src/crosshook-native/src-tauri/src/commands/`
- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store.rs` (if needed)

**Instructions:**

1. On successful launch outcome, mark the appropriate latest revision as known-good.
2. Ensure only one active known-good revision per profile.
3. Handle ambiguous success semantics according to decided policy.

## Phase 3 - Frontend History and Restore UX

### Task 3.1: Add history and diff TypeScript contracts

**Dependencies:** Task 2.2  
**READ THESE BEFORE TASK:**

- `src/crosshook-native/src/types/index.ts`
- `docs/plans/config-history-rollback/research-ux.md`

**Files to Create:**

- `src/crosshook-native/src/types/profile-history.ts`

**Files to Modify:**

- `src/crosshook-native/src/types/index.ts`

**Instructions:**

1. Add DTO types for history summary/detail, diff output, and rollback responses.
2. Keep naming aligned with command payloads and existing typing conventions.

### Task 3.2: Extend profile hook with history actions

**Dependencies:** Task 3.1, Task 2.2  
**READ THESE BEFORE TASK:**

- `src/crosshook-native/src/hooks/useProfile.ts`
- `src/crosshook-native/src/hooks/useProfileHealth.ts`

**Files to Create:**

- none

**Files to Modify:**

- `src/crosshook-native/src/hooks/useProfile.ts`

**Instructions:**

1. Add invoke wrappers for list/diff/rollback (and known-good mark if applicable).
2. Provide loading/error state handling for history operations.
3. Refresh profile + health state after rollback success.

### Task 3.3: Implement History UI (timeline, compare, restore confirmation)

**Dependencies:** Task 3.2  
**READ THESE BEFORE TASK:**

- `src/crosshook-native/src/components/ProfileActions.tsx`
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`
- `docs/plans/config-history-rollback/research-ux.md`

**Files to Create:**

- optional new component file under `src/crosshook-native/src/components/` (history panel/modal)

**Files to Modify:**

- `src/crosshook-native/src/components/ProfileActions.tsx`
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`
- related style file(s) under `src/crosshook-native/src/styles/` (if needed)

**Instructions:**

1. Add `History` entry action.
2. Render timeline list with source/known-good metadata.
3. Support compare with current and selected revision.
4. Require explicit confirmation before restore and show success/error feedback.
5. Implement loading/empty/error and keyboard/focus accessible behaviors.

## Phase 4 - Hardening, Validation, and Acceptance

### Task 4.1: Add rollback integrity and resource-bound checks

**Dependencies:** Task 2.2  
**READ THESE BEFORE TASK:**

- `docs/plans/config-history-rollback/research-security.md`
- metadata modules under `src/crosshook-native/crates/crosshook-core/src/metadata/`

**Files to Create:**

- none

**Files to Modify:**

- `src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store.rs`
- `src/crosshook-native/src-tauri/src/commands/profile.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` (limits, if needed)

**Instructions:**

1. Add ownership and payload validity checks on rollback.
2. Enforce count/size limits for history and diff operations.
3. Ensure deterministic error handling without partial apply states.

### Task 4.2: Add integration tests for end-to-end revision and rollback correctness

**Dependencies:** Task 2.2, Task 4.1  
**READ THESE BEFORE TASK:**

- existing metadata and command test modules
- `docs/plans/config-history-rollback/feature-spec.md`

**Files to Create:**

- test file(s) under `src/crosshook-native/crates/crosshook-core/tests/` or existing module test locations

**Files to Modify:**

- relevant test module(s) in metadata/commands as needed

**Instructions:**

1. Test save -> revision append -> diff -> rollback -> lineage append.
2. Test rename continuity via `profile_id`.
3. Test metadata unavailable behavior for history endpoints.

### Task 4.3: Acceptance sweep against feature criteria

**Dependencies:** Tasks 3.3 and 4.2  
**READ THESE BEFORE TASK:**

- `docs/plans/config-history-rollback/feature-spec.md`
- `docs/plans/config-history-rollback/research-business.md`

**Files to Create:**

- none

**Files to Modify:**

- implementation files only for final gaps found

**Instructions:**

1. Verify all acceptance criteria in `feature-spec.md` are met or explicitly deferred.
2. Capture unresolved policy decisions and document scope cut lines.
3. Prepare concise implementation notes for follow-up PR/release documentation.

## Advice

- Start with full snapshot storage and line-based diff; optimize only with evidence.
- Keep snapshot capture logic centralized to avoid inconsistent behavior across write flows.
- Avoid coupling UI details to raw DB shape; stabilize DTO contracts first.
- Treat rollback as a privileged safety workflow: strict checks, explicit confirmation, and robust error paths.
- Preserve developer velocity by shipping the MVP vertical slice first, then iterating on richer diff semantics and retention controls.
