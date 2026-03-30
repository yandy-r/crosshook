# Custom Env Vars - Architecture Research

## System Architecture Impact

The `custom-env-vars` feature is an additive profile capability that crosses both persistence and launch execution boundaries. The change begins in profile schema (`launch` section), flows into launch request composition, and must be consumed by all launch output surfaces: runtime process execution, preview rendering, and Steam launch options generation.

## Core Layer Boundaries

- **Persistence boundary**: `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
  - Add `launch.custom_env_vars` as serialized profile state with backward-compatible defaults.
- **Validation boundary**: `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
  - Enforce key/value validity and runtime-reserved key restrictions.
- **Environment resolution boundary**:
  - `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`
  - Build one canonical merge path to avoid behavior drift.
- **Execution boundary**: `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
  - Ensure merged environment is applied for launch methods that execute child commands.
- **Preview/report boundary**: `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`
  - Show effective values and source attribution from the same merge logic.

## Frontend Layer Boundaries

- **Type boundary**:
  - `src/crosshook-native/src/types/profile.ts`
  - `src/crosshook-native/src/types/launch.ts`
- **Request composition boundary**:
  - `src/crosshook-native/src/utils/launch.ts`
- **State defaults and normalization**:
  - `src/crosshook-native/src/hooks/useProfile.ts`
- **Authoring boundary**:
  - `src/crosshook-native/src/components/ProfileFormSections.tsx`

## End-to-End Data Flow

1. User edits `custom_env_vars` in profile UI.
2. Profile is persisted in TOML under `launch.custom_env_vars`.
3. Launch request includes custom map with optimization settings.
4. Backend validates map and computes effective merged env.
5. Effective env is used by:
   - runtime command env injection,
   - launch preview output,
   - Steam launch-options generation.
6. All surfaces reflect the same winning values for conflicts.

## Key Architectural Rule

There must be one environment merge source of truth in `crosshook-core`; no duplicated merge behavior in frontend or in independent runtime/preview code paths.
