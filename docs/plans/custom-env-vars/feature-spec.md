# Feature Spec: Custom Env Vars Per Profile

## Metadata

- Feature: `custom-env-vars`
- Issue: [#57 feat(profiles): custom environment variables per profile](https://github.com/yandy-r/crosshook/issues/57)
- Goal: ship a feature-complete implementation with no deferred behavior
- Source research:
  - `docs/plans/custom-env-vars/research-external.md`
  - `docs/plans/custom-env-vars/research-business.md`
  - `docs/plans/custom-env-vars/research-technical.md`
  - `docs/plans/custom-env-vars/research-ux.md`
  - `docs/plans/custom-env-vars/research-security.md`
  - `docs/plans/custom-env-vars/research-practices.md`
  - `docs/plans/custom-env-vars/research-recommendations.md`

## Executive Summary

CrossHook should support profile-level custom environment variables that users can define as key-value pairs and apply at launch time. This must integrate with existing launch optimization directives, with deterministic conflict behavior where user custom values override optimization-derived values. The implementation must remain consistent across runtime execution, preview output, and Steam launch-options string generation to avoid behavioral drift. This spec adopts an additive schema change (`launch.custom_env_vars`), a single backend merge source of truth, full profile editor support, and complete test coverage for persistence, validation, and precedence.

## Scope

### In Scope

- Add `launch.custom_env_vars` to profile schema and DTOs.
- Add UI to create/edit/delete custom env vars.
- Merge custom vars with optimization vars at launch.
- Ensure custom vars override optimization vars on conflicts.
- Reflect merged env in preview and Steam launch options.
- Validate key/value input in backend (authoritative).
- Add tests across core affected modules.

### Out of Scope

- Dynamic secret storage/keyring integration.
- Advanced env features (unset/tombstone semantics, profile-level presets for env bundles).

## Product Semantics

## Effective Environment Merge Order

For non-native methods:

1. Host/method runtime env
2. Launch optimization env directives
3. `custom_env_vars` (wins on key conflict)

For `native`:

1. Host env
2. `custom_env_vars`

## Reserved Key Policy

Custom env vars may not override runtime-critical keys:

- `WINEPREFIX`
- `STEAM_COMPAT_DATA_PATH`
- `STEAM_COMPAT_CLIENT_INSTALL_PATH`

Reason: these keys are controlled by runtime resolution and must remain stable.

## Validation Rules

- Key must be non-empty after trim.
- Key must not contain `=`.
- Key/value must not contain NUL.
- Duplicate keys are prevented by data shape and UI behavior.
- Optional hard caps:
  - max keys per profile
  - max key length
  - max value length

## Architecture and File Plan

### Backend (`crosshook-core`)

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
  - Add `launch.custom_env_vars: BTreeMap<String, String>`
  - Add serde defaults and skip-empty serialization

- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
  - Add `custom_env_vars` to `LaunchRequest`
  - Add validation function for custom env keys/values and reserved keys

- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
  - Extend merge/build functions so Steam options generation uses merged env, not optimization-only env

- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`
  - Add helper to apply custom env map to command env

- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
  - Apply custom env after optimization env in proton launch command paths

- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`
  - Include custom env in effective environment list
  - Add source variant for custom env (`profile_custom`)

### Frontend

- `src/crosshook-native/src/types/profile.ts`
  - Add `launch.custom_env_vars: Record<string, string>`

- `src/crosshook-native/src/types/launch.ts`
  - Add `custom_env_vars` to request type
  - Add `profile_custom` env source value

- `src/crosshook-native/src/utils/launch.ts`
  - Include `custom_env_vars` when building `LaunchRequest`

- `src/crosshook-native/src/hooks/useProfile.ts`
  - Add default empty map in profile creation/normalization flows

- `src/crosshook-native/src/components/ProfileFormSections.tsx`
  - Add editable key-value table for custom env vars
  - Inline key validation and duplicate handling UX

- `src/crosshook-native/src/components/LaunchPanel.tsx`
  - Display custom env source grouping in preview modal

## UX Specification

- New subsection under launch/runtime profile configuration:
  - Title: `Custom Environment Variables`
  - Two-column row editor (Key, Value)
  - Add/remove actions
- Help copy:
  - `Custom variables override built-in launch optimization variables when keys conflict.`
- Validation copy examples:
  - invalid key
  - duplicate key
  - reserved key
- Preview modal:
  - show effective merged vars and `Profile custom` source label

## Security Requirements

- Reserved runtime keys are blocked from custom override.
- Values are never logged by default in core logs.
- Validation prevents malformed env payloads that could break spawn behavior.
- Keep shell helper quoting discipline unchanged (no `eval`, quote all expansions).

## Testing Strategy

### Rust Unit Tests

- Profile TOML roundtrip with `launch.custom_env_vars`
- Validation rules (valid/invalid key-value sets)
- Reserved key rejection
- Precedence: custom overrides optimization
- Preview includes custom source and effective value
- Steam launch options include merged/custom-overridden values

### Frontend Checks

- Type integrity for new fields in profile/request types
- Request builder includes `custom_env_vars`
- Profile defaults and load normalization preserve map

### Manual QA Matrix

- `proton_run`: custom only, optimization only, conflict case
- `steam_applaunch`: merged launch-options output reflects custom precedence
- `native`: custom vars apply successfully
- Save/reload profile persistence for custom vars
- Preview alignment with runtime behavior

## Delivery Plan (Feature Complete)

### Phase 1: Model + merge core

- Schema and DTO fields
- Merge helper and validation

### Phase 2: Runtime + preview parity

- Runtime command integration
- Preview and Steam options alignment

### Phase 3: UI authoring and final hardening

- Profile editor UX
- Test completion and docs

Exit condition: all acceptance criteria in issue #57 satisfied, with no deferred behavior.

## Acceptance Checklist

- [ ] Profiles support arbitrary key-value custom env vars
- [ ] Custom vars apply at launch alongside optimization vars
- [ ] Custom vars take precedence on conflict
- [ ] Preview shows merged effective env with custom source
- [ ] Steam launch options generation reflects merged env
- [ ] Runtime-critical reserved keys are blocked
- [ ] Automated tests cover persistence, validation, merge, and preview/runtime parity
