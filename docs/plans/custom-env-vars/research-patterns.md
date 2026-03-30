# Custom Env Vars - Patterns Research

## Existing Project Patterns To Reuse

## 1) Additive Profile Schema Evolution

- Existing profile model changes use serde defaults to keep old profiles loadable.
- Apply the same pattern for `launch.custom_env_vars`:
  - default empty map on deserialize,
  - skip serializing when empty to avoid noisy TOML diffs.

## 2) Backend-Authoritative Validation

- CrossHook centralizes launch validation in core request validation.
- Follow that pattern: key/value validation and reserved-key blocking must live in Rust, not only UI.

## 3) Thin Frontend, Strong Core

- Frontend currently mainly edits profile state and forwards launch requests.
- Keep frontend responsible for editing UX and immediate feedback only.
- Keep final semantics in core so CLI/Tauri and preview/runtime stay aligned.

## 4) Launch Surface Parity

- Existing launch system already risks drift between runtime directives and preview reporting.
- Required pattern for this feature: one shared merge helper consumed by all surfaces.

## 5) Deterministic Conflict Resolution

- Reproducibility in profiles requires stable behavior.
- Use canonical order:
  - method/base env,
  - optimization env,
  - profile custom env (wins).

## 6) Security-by-Validation

- Runtime-critical env keys are treated as controlled invariants.
- Block these from custom override:
  - `WINEPREFIX`
  - `STEAM_COMPAT_DATA_PATH`
  - `STEAM_COMPAT_CLIENT_INSTALL_PATH`

## 7) Test-Driven Path Around Shared Helper

High-value tests should target the canonical merge helper and then verify each surface consumes it:

- merge precedence,
- reserved-key rejection,
- preview/runtime parity,
- steam options parity.
