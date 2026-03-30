# Custom Env Vars: Technical Specification

## Current Code Touchpoints

- Profile schema: `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
- Launch request DTO and validation: `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- Optimization directive resolution: `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
- Runtime command env application: `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`, `script_runner.rs`
- Preview environment rendering: `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`
- Frontend types/request builder: `src/crosshook-native/src/types/{profile.ts,launch.ts}`, `src/crosshook-native/src/utils/launch.ts`
- Profile editing UX: `src/crosshook-native/src/components/ProfileFormSections.tsx`

## Data Model Changes

### Rust profile model

- Extend `LaunchSection` with:
  - `custom_env_vars: BTreeMap<String, String>`
- Serde attributes:
  - `#[serde(rename = "custom_env_vars", default, skip_serializing_if = "BTreeMap::is_empty")]`

### Launch request DTO

- Extend `LaunchRequest` with:
  - `custom_env_vars: BTreeMap<String, String>`
- Preserve existing defaults for backward compatibility.

### Frontend DTOs

- `GameProfile.launch.custom_env_vars: Record<string, string>`
- `LaunchRequest.custom_env_vars: Record<string, string>`
- Ensure defaults in `createEmptyProfile` and normalization paths in `useProfile`.

## Merge and Precedence Contract

Single contract used everywhere:

1. Host/runtime/method env
2. Optimization env directives
3. `custom_env_vars` from profile

Conflict policy:

- Last-write-wins with `custom_env_vars` highest precedence over optimization env values.

Implementation requirement:

- One shared helper in `crosshook-core` to merge env layers.
- Runtime command builders, preview env collection, and Steam launch-options string generation all use this helper.

## Validation Boundaries

### Backend authoritative validation

- Reject invalid keys:
  - empty or whitespace-only
  - contains `=`
  - contains NUL
- Reject values containing NUL.
- Optional caps:
  - max key length
  - max value length
  - max variable count

### Reserved key policy

To avoid breaking launch invariants, block custom overrides for:

- `WINEPREFIX`
- `STEAM_COMPAT_DATA_PATH`
- `STEAM_COMPAT_CLIENT_INSTALL_PATH`

Allow overriding optimization keys (required by issue).

## Preview and Source Attribution

- Add preview source enum variant for custom vars (`profile_custom`).
- When custom overrides optimization for the same key, preview shows the effective custom value/source.
- Keep grouped environment display in `LaunchPanel` updated with new source label.

## Steam Launch Options Parity

- Extend Steam launch-options construction to include merged `custom_env_vars`.
- Do not rely on an optimization-only string builder for final output.

## Backward Compatibility

- Existing TOML profiles load with empty `custom_env_vars`.
- Existing launch behavior unchanged when `custom_env_vars` is empty.
- Additive schema change only; no SQLite migration required.

## Test Matrix

- Profile TOML roundtrip includes `launch.custom_env_vars`.
- Validation tests for invalid keys/values and reserved keys.
- Merge precedence tests (optimization vs custom conflict).
- Runtime command tests ensure effective env contains custom winning values.
- Preview tests ensure source + value reflect final merged env.
- Steam launch-options tests verify custom overrides in generated string.
