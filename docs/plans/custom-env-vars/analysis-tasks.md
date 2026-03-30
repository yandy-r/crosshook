# Analysis Tasks: custom-env-vars

## Recommended Task Structure

## Phase 1 - Data Model and Validation Foundation

- Add schema and DTO fields for `custom_env_vars` (Rust + TypeScript).
- Implement backend validation and reserved-key protection.
- Add/update unit tests for schema defaults and validation behavior.

## Phase 2 - Canonical Merge Engine and Launch Surface Wiring

- Implement one shared merge helper in core with precedence: optimization < custom.
- Integrate helper into runtime command environment application.
- Integrate helper into preview environment rendering and source attribution.
- Integrate helper into Steam launch-options generation.
- Add parity tests proving all surfaces share effective results.

## Phase 3 - Frontend Authoring UX and Wiring

- Build editable key/value UI in profile form.
- Add inline duplicate/format validation feedback.
- Wire request building and profile defaults/normalization.
- Ensure UX copy explains precedence behavior.

## Phase 4 - Hardening, Verification, and Docs

- Complete test matrix and run full targeted checks.
- Perform manual QA across all launch methods and conflict scenarios.
- Update docs with usage, precedence, reserved keys, and troubleshooting.
- Validate acceptance checklist is fully satisfied (no deferred items).

## Parallelization Guidance

- Phase 1 backend model work and frontend type work can run in parallel.
- Phase 2 runtime/preview integration can be split after merge helper lands.
- Phase 3 UI work can proceed once type/contracts are finalized.
- Phase 4 is sequential for final confidence and sign-off.
