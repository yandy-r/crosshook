# Parallel Implementation Plan: custom-env-vars

## Overview

This plan delivers `custom-env-vars` as a feature-complete capability with no deferred behavior by aligning profile persistence, runtime launch semantics, preview reporting, and Steam launch-options generation behind a single merge contract. The implementation is dependency-aware: first establish schema and validation correctness, then integrate one canonical env merge helper into every launch surface, then complete frontend authoring UX and final verification. The highest-risk area is behavioral drift across runtime/preview/output surfaces, so parity tests are first-class deliverables rather than optional polish. Completion is defined by full acceptance criteria, method parity (`proton_run`, `steam_applaunch`, `native`), and validated security constraints for reserved runtime keys.

## Critically Relevant Files and Documentation

### Core files

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`

### Frontend files

- `src/crosshook-native/src/types/profile.ts`
- `src/crosshook-native/src/types/launch.ts`
- `src/crosshook-native/src/utils/launch.ts`
- `src/crosshook-native/src/hooks/useProfile.ts`
- `src/crosshook-native/src/components/ProfileFormSections.tsx`

### Planning and research docs

- `docs/plans/custom-env-vars/feature-spec.md`
- `docs/plans/custom-env-vars/shared.md`
- `docs/plans/custom-env-vars/research-technical.md`
- `docs/plans/custom-env-vars/research-security.md`
- `docs/plans/custom-env-vars/research-practices.md`

## Implementation Plan

## Phase 1 - Schema, DTOs, and Validation Foundation

### Task 1.1 - Add profile and request schema fields

- **Dependencies**: none
- **READ THESE BEFORE TASK**:
  - `docs/plans/custom-env-vars/feature-spec.md`
  - `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
  - `src/crosshook-native/src/types/profile.ts`
  - `src/crosshook-native/src/types/launch.ts`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
  - `src/crosshook-native/src/types/profile.ts`
  - `src/crosshook-native/src/types/launch.ts`
- **Implementation**:
  - Add `custom_env_vars` to profile launch section with serde default + skip-empty behavior.
  - Add `custom_env_vars` to `LaunchRequest` with backward-compatible defaults.
  - Update TS types to include `Record<string, string>` for profile/request.

### Task 1.2 - Implement backend validation and reserved-key protections

- **Dependencies**: Task 1.1
- **READ THESE BEFORE TASK**:
  - `docs/plans/custom-env-vars/research-security.md`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- **Implementation**:
  - Validate key rules (non-empty, no `=`, no NUL).
  - Validate value rules (no NUL).
  - Block custom overrides for:
    - `WINEPREFIX`
    - `STEAM_COMPAT_DATA_PATH`
    - `STEAM_COMPAT_CLIENT_INSTALL_PATH`
  - Ensure validation issues are surfaced through existing `validate` and `validate_all` flows.

### Task 1.3 - Add/expand tests for schema and validation

- **Dependencies**: Task 1.1, Task 1.2
- **READ THESE BEFORE TASK**:
  - `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- **Implementation**:
  - Add profile TOML roundtrip tests for empty/non-empty `custom_env_vars`.
  - Add validation tests for malformed keys/values and reserved key rejection.

## Phase 2 - Canonical Merge Engine and Launch Surface Parity

### Task 2.1 - Introduce canonical env merge helper in core

- **Dependencies**: Task 1.1, Task 1.2
- **READ THESE BEFORE TASK**:
  - `docs/plans/custom-env-vars/research-practices.md`
  - `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
- **Implementation**:
  - Build one helper that composes effective env with precedence:
    - base/method env
    - optimization env
    - custom env
  - Keep API reusable by runtime execution, preview, and steam options generation.

### Task 2.2 - Wire runtime command env application to canonical helper

- **Dependencies**: Task 2.1
- **READ THESE BEFORE TASK**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
- **Implementation**:
  - Apply merged custom+optimization env where runtime env is injected.
  - Ensure method parity where env injection applies (`proton_run`, `native`, and relevant Steam path behavior).

### Task 2.3 - Wire preview and source attribution to canonical helper

- **Dependencies**: Task 2.1
- **READ THESE BEFORE TASK**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`
  - `src/crosshook-native/src/types/launch.ts`
- **Implementation**:
  - Use merged effective env from canonical helper in preview.
  - Add/propagate `profile_custom` source attribution.
  - Show winning values for key conflicts in preview output.

### Task 2.4 - Wire Steam launch options generation to canonical helper

- **Dependencies**: Task 2.1
- **READ THESE BEFORE TASK**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
- **Implementation**:
  - Ensure launch-options command includes custom env overrides, not optimization-only data.
  - Preserve existing wrapper resolution behavior and error surfacing.

### Task 2.5 - Add parity tests across runtime/preview/steam outputs

- **Dependencies**: Task 2.2, Task 2.3, Task 2.4
- **READ THESE BEFORE TASK**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
- **Implementation**:
  - Add tests proving identical effective values across all surfaces.
  - Explicitly test conflict where custom overrides optimization key.

## Phase 3 - Frontend Editing UX and Request Wiring

### Task 3.1 - Add profile defaults and request wiring

- **Dependencies**: Task 1.1
- **READ THESE BEFORE TASK**:
  - `src/crosshook-native/src/hooks/useProfile.ts`
  - `src/crosshook-native/src/utils/launch.ts`
  - `src/crosshook-native/src/types/profile.ts`
  - `src/crosshook-native/src/types/launch.ts`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/src/hooks/useProfile.ts`
  - `src/crosshook-native/src/utils/launch.ts`
- **Implementation**:
  - Add empty-map defaulting/normalization in profile lifecycle.
  - Ensure launch request payload always includes `custom_env_vars`.

### Task 3.2 - Build profile custom env editor

- **Dependencies**: Task 3.1
- **READ THESE BEFORE TASK**:
  - `docs/plans/custom-env-vars/research-ux.md`
  - `src/crosshook-native/src/components/ProfileFormSections.tsx`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/src/components/ProfileFormSections.tsx`
- **Implementation**:
  - Add key/value row editor with add/remove actions.
  - Add inline validation messaging for invalid keys and duplicates.
  - Add precedence helper copy near the editor.
  - Keep keyboard and focus behavior accessible.

### Task 3.3 - Ensure preview UI labeling reflects custom source

- **Dependencies**: Task 2.3, Task 3.1
- **READ THESE BEFORE TASK**:
  - `src/crosshook-native/src/components/LaunchPanel.tsx`
  - `src/crosshook-native/src/types/launch.ts`
- **Files to Create**: none
- **Files to Modify**:
  - `src/crosshook-native/src/components/LaunchPanel.tsx`
  - any dependent preview display components if required by type changes
- **Implementation**:
  - Show `Profile custom` source labels in preview grouping.
  - Ensure rendered values match backend-provided effective env.

## Phase 4 - Hardening, Verification, and Completion Gates

### Task 4.1 - Complete test matrix and run verification suite

- **Dependencies**: Phase 1, Phase 2, Phase 3 complete
- **READ THESE BEFORE TASK**:
  - `docs/plans/custom-env-vars/feature-spec.md`
  - all modified launch/profile tests in core
- **Files to Create**: none
- **Files to Modify**:
  - any failing test files identified during verification
- **Implementation**:
  - Run focused core tests and workspace checks for touched areas.
  - Add missing tests for uncovered acceptance behaviors.
  - Verify no regression in existing launch optimization paths.

### Task 4.2 - Manual QA for method parity and conflict behavior

- **Dependencies**: Task 4.1
- **READ THESE BEFORE TASK**:
  - `docs/plans/custom-env-vars/research-recommendations.md`
- **Files to Create**: none
- **Files to Modify**:
  - docs/checklists if manual validation notes are tracked
- **Implementation**:
  - Validate `proton_run`, `steam_applaunch`, `native` behavior with:
    - custom-only env,
    - optimization-only env,
    - conflict (custom override) env.
  - Confirm preview and runtime outcomes match.

### Task 4.3 - Documentation and final acceptance closure

- **Dependencies**: Task 4.1, Task 4.2
- **READ THESE BEFORE TASK**:
  - `docs/plans/custom-env-vars/feature-spec.md`
- **Files to Create**:
  - update/add user-facing docs for custom env vars (path based on repo doc conventions)
- **Files to Modify**:
  - relevant docs in `docs/` describing launch configuration
- **Implementation**:
  - Document syntax, precedence, reserved keys, and troubleshooting.
  - Mark acceptance checklist fully complete only after test + QA evidence.

## Dependency Graph Summary

- **Independent early tasks**:
  - Task 1.1 backend schema and frontend type updates can run in parallel streams.
- **Critical chain**:
  - Task 1.1 -> Task 1.2 -> Task 2.1 -> (Task 2.2, Task 2.3, Task 2.4 in parallel) -> Task 2.5 -> Phase 3 tasks -> Phase 4 closure.
- **Max dependency depth**: 7 steps.

## Advice

- Implement the canonical merge helper before touching preview/runtime surface behavior to minimize rework.
- Treat parity tests as design constraints, not post-hoc checks; they prevent the core drift risk this feature is prone to.
- Keep frontend validation user-friendly but never rely on it for correctness; backend validation must stay authoritative.
- Enforce a strict completion gate: no deferred TODOs, no unimplemented launch-method paths, and no documentation gaps around precedence/security semantics.
