# Config History Rollback - Documentation Research

## Primary planning docs in this feature directory

- `docs/plans/config-history-rollback/feature-spec.md`
  - Source of goals, constraints, acceptance criteria, and proposed command surface.
- `docs/plans/config-history-rollback/research-business.md`
  - Product framing, user stories, and refined acceptance criteria.
- `docs/plans/config-history-rollback/research-technical.md`
  - Concrete code integration points and migration strategy.
- `docs/plans/config-history-rollback/research-security.md`
  - Integrity, auditability, and resource-bound controls.
- `docs/plans/config-history-rollback/research-ux.md`
  - Timeline/diff/restore interaction model and accessibility requirements.
- `docs/plans/config-history-rollback/research-practices.md`
  - Reuse and KISS guidance for module boundaries and testing strategy.
- `docs/plans/config-history-rollback/research-external.md`
  - Third-party crates and external pattern references.
- `docs/plans/config-history-rollback/research-recommendations.md`
  - Final architecture recommendation and decision points.

## Critically relevant code docs and repository guidance

- `AGENTS.md`, `CLAUDE.md`, `.cursorrules`
  - Project conventions, architecture map, and workflow expectations.
- `src/crosshook-native/crates/crosshook-core/src/metadata/`
  - Existing metadata store patterns, migrations, and retention behavior.
- `src/crosshook-native/src-tauri/src/commands/`
  - Existing IPC command style and profile write orchestration points.
- `src/crosshook-native/src/types/` and `src/components/`
  - Frontend DTO contracts and profile page composition points.

## Documentation gaps to close during implementation

1. Add implementation notes for config history architecture under `docs/` after MVP merges.
2. Document rollback safety semantics and known-good policy in user-facing help.
3. Add release note language describing feature scope and limitations.

## Required reading order for implementors

1. `feature-spec.md`
2. `research-technical.md`
3. `research-security.md`
4. `research-ux.md`
5. `parallel-plan.md` (after generation)
