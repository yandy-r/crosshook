# Analysis - Task Structure (config-history-rollback)

## Suggested phase structure

## Phase 1 - Metadata foundation

- Add schema migration and models for `config_revisions`.
- Implement metadata store operations:
  - append (dedup),
  - list/get,
  - known-good mark,
  - prune.
- Add lower-level unit tests for retention and integrity rules.

## Phase 2 - Command surface and orchestration

- Add shared helper for snapshot capture in profile write flows.
- Wire helper into selected profile command mutation paths.
- Add history list/diff/rollback commands plus known-good command if included.
- Add command-level tests for rollback behavior and error propagation.

## Phase 3 - Frontend integration

- Add types for revision summary/detail/diff responses.
- Extend profile hook layer with history actions and state handling.
- Add History action, timeline UI, compare modal, and restore confirmation UX.
- Add availability, loading, empty, and error states.

## Phase 4 - Verification and hardening

- Verify known-good tagging behavior from launch success integration.
- Add resource bound checks for snapshot count/size and diff workload.
- Validate rollback auditability and lineage fields.
- Run backend tests and smoke check frontend interaction paths.

## Parallelization opportunities

- Phase 1 metadata table/model work and frontend type scaffolding can start in parallel once command DTO contract is drafted.
- Diff rendering UI and restore confirmation UI can proceed in parallel after command contracts are stable.
- Security validation test additions can run in parallel with UX polish once core rollback command is operational.

## Dependency-critical milestones

1. Migration + metadata APIs complete.
2. Command interfaces stabilized and wired.
3. Frontend end-to-end flow functional.
4. Hardening and acceptance criteria validation complete.
