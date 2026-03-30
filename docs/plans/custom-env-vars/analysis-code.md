# Analysis Code: custom-env-vars

## Current Code Signals

- `LaunchSection` already carries optimization data but no custom env map yet.
- `LaunchRequest` and validators are centralized in `launch/request.rs`, making this the correct place to add key/value and reserved-key checks.
- Runtime env directives are currently resolved via optimization helpers and consumed in multiple paths (`script_runner.rs`, `preview.rs`, `optimizations.rs` steam command builder).
- Frontend request composition is centralized in `src/crosshook-native/src/utils/launch.ts`, which is the key wiring point once types are expanded.

## Refactor/Extension Hotspots

- Add `custom_env_vars` to:
  - profile model launch section,
  - launch request model,
  - frontend profile and launch request types.
- Introduce a canonical env merge helper in core and replace direct/duplicated env stitching in:
  - runtime command build path,
  - preview environment construction,
  - Steam launch-options command generation.

## Regression Risks

- Partial updates where runtime supports custom vars but preview does not.
- Steam launch options still reflecting optimization-only directives.
- Missing defaults causing existing profiles or imported profiles to omit new field safely.
- UI-only validation without backend checks allowing invalid launch requests.

## Test Targets With Highest ROI

- request validation for malformed keys/values + reserved keys,
- merge precedence correctness,
- runtime/preview/steam parity from same merged source,
- profile TOML roundtrip with empty and non-empty maps.
