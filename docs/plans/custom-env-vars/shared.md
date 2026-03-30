# Shared Context: custom-env-vars

## Overview

`custom-env-vars` is a cross-layer feature that adds profile-defined environment variables with deterministic precedence over optimization-derived directives while preserving launch parity across runtime, preview, and Steam launch-options output. The implementation is primarily additive, but correctness depends on enforcing one canonical merge path and backend-authoritative validation to avoid drift and security regressions. The key risk is fragmented env assembly logic across `script_runner`, `preview`, and optimization command generation; this plan treats merge centralization as a hard requirement. Feature completion requires full method parity (`proton_run`, `steam_applaunch`, `native`), UI authoring support, and comprehensive tests for persistence, validation, precedence, and output consistency.

## Critically Relevant Files and Why

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` - profile schema source of truth (`launch` section), serialization defaults, and backward compatibility.
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` - launch request shape and validation boundary where custom env data must be enforced.
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` - shared env manipulation utilities; best place for canonical merge helpers.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs` - optimization directive output and Steam launch-options generation currently tied to optimization-only inputs.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` - runtime command env application path for launch execution.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` - preview rendering path that must show effective merged env and source attribution.
- `src/crosshook-native/src/types/profile.ts` - frontend profile contract and editor state typing.
- `src/crosshook-native/src/types/launch.ts` - frontend launch request and preview source typing.
- `src/crosshook-native/src/utils/launch.ts` - request builder wiring from profile to backend payload.
- `src/crosshook-native/src/hooks/useProfile.ts` - profile defaults/normalization so new field is always stable in state.
- `src/crosshook-native/src/components/ProfileFormSections.tsx` - primary profile editor surface for custom env authoring UX.

## Existing Patterns To Follow

- Additive schema evolution with defaults for backward compatibility.
- Backend-authoritative validation for launch-critical invariants.
- Thin frontend responsibilities (editing + wiring) with core-owned semantics.
- Deterministic precedence rules and reusable helper-based composition.
- High-value testing around shared helpers instead of per-surface duplicated logic.

## Security and Correctness Constraints

- Block runtime-critical key overrides:
  - `WINEPREFIX`
  - `STEAM_COMPAT_DATA_PATH`
  - `STEAM_COMPAT_CLIENT_INSTALL_PATH`
- Reject malformed key/value inputs (empty key, `=`, NUL).
- Avoid accidental secret-value leakage in routine logs/diagnostics.
- Keep launch behavior deterministic and identical between preview and runtime.

## Required Read-Before-Implement References

- `docs/plans/custom-env-vars/feature-spec.md`
- `docs/plans/custom-env-vars/research-technical.md`
- `docs/plans/custom-env-vars/research-security.md`
- `docs/plans/custom-env-vars/research-practices.md`
