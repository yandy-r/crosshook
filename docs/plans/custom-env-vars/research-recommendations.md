# Custom Env Vars: Recommendations

## Recommended Delivery Strategy

Ship issue `#57` in dependency-ordered phases, with final phase delivering full feature completion (no deferred behavior).

## Phase Plan

### Phase 1: Core schema + merge engine

- Add `launch.custom_env_vars` in Rust and TypeScript profile models.
- Add `custom_env_vars` to `LaunchRequest`.
- Implement shared env merge helper with precedence: optimization < custom.
- Add backend validation for keys/values and reserved runtime keys.

### Phase 2: Runtime + preview parity

- Apply merged env helper to `proton_run` runtime command paths.
- Apply merged env helper to preview env collection.
- Add `profile_custom` preview source.
- Extend Steam launch options generation to include custom overrides.

### Phase 3: Profile editor UX

- Add custom env editor in `ProfileFormSections`.
- Support add/edit/remove rows.
- Inline validation + duplicate handling.
- Add precedence helper copy.

### Phase 4: Verification + docs

- Rust unit tests for roundtrip, validation, precedence, runtime, preview, and Steam options.
- Frontend type-check/lint checks for new model fields.
- Manual QA across all launch methods.
- Update docs for usage and precedence semantics.

## Non-Deferred Definition

By phase completion, all of the following must be true:

- Profile editing supports custom env vars fully.
- Launch runtime applies custom env vars correctly.
- Preview shows effective merged env and source.
- Conflict precedence is deterministic and tested.
- Steam launch options reflect same merge outcome.

## Recommended Defaults

- Allow custom env vars on all launch methods.
- Block overriding runtime-critical keys (`WINEPREFIX`, `STEAM_COMPAT_*`).
- Keep values free-form (except NUL).
