# Custom Env Vars - Integration Research

## Integration Points

## Core -> Frontend Contract

- `GameProfile.launch` in `src/crosshook-native/src/types/profile.ts` must include `custom_env_vars`.
- `LaunchRequest` in `src/crosshook-native/src/types/launch.ts` must include `custom_env_vars`.
- `src/crosshook-native/src/utils/launch.ts` must pass profile custom vars into request payloads.

## Profile Storage -> Runtime Launch Contract

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` stores `custom_env_vars`.
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` validates incoming map.
- Runtime launch command construction (`script_runner.rs`) applies merged environment.

## Optimizations -> Custom Env Contract

- Current optimization directives are resolved in `optimizations.rs`.
- Steam launch options are also generated there.
- Both directive resolution and launch-options output must include custom overrides via shared merge logic.

## Preview -> Runtime Contract

- `preview.rs` must report final effective values from the same merge algorithm used by runtime command execution.
- Add source attribution for profile-level custom values (`profile_custom`) to preserve UX explainability.

## Launch Method Coverage

Feature-complete integration requires parity across:

- `proton_run`
- `steam_applaunch`
- `native`

No method-specific omission is acceptable for this feature scope.

## Validation + UX Contract

- UI performs inline guidance and duplicate detection.
- Backend remains authoritative for invalid key/value and reserved-key rejection.
- Preview/launch boundaries must return actionable errors if invalid data bypasses UI checks.
