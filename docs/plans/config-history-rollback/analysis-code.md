# Analysis - Code Patterns (config-history-rollback)

## Backend patterns to follow

- Create a dedicated metadata submodule for SQL logic (aligned with `version_store.rs` style).
- Extend `MetadataStore` with thin wrappers rather than embedding SQL in command handlers.
- Reuse `observe_profile_write` hash and identity workflow to avoid duplicate identity logic.
- Keep rollback orchestration at command/core boundary where profile save and metadata sync are already coordinated.

## API and DTO patterns

- Keep Tauri command names in snake_case to match existing profile command conventions.
- Split list vs detail payloads:
  - list returns summary rows (lightweight),
  - detail/diff endpoints return body-heavy output only when requested.
- Preserve typed TS DTO parity in `src/types` and invoke wrappers in hooks.

## Frontend patterns

- Follow existing hook + page integration style from profile and health flows.
- Add history trigger in `ProfileActions`, keep primary UI integration in `ProfilesPage`.
- Prefer modal/sheet composition already used by profile-related views.
- Keep compare as read-only UI operation and restore as explicitly confirmed operation.

## Test patterns

- Core metadata tests with in-memory store for insert/list/dedup/prune/lineage.
- Integration-style tests for save->snapshot and rollback->restored TOML + appended lineage row.
- Focus on deterministic ordering and retention behavior for flaky-resistant tests.

## Anti-patterns to avoid

- Treating metadata history as a second canonical source of live profile state.
- Writing snapshot logic in multiple command paths without shared helper.
- Shipping unlimited revision growth or unrestricted diff payload generation.
- Implementing advanced semantic merge logic before basic line-diff correctness is proven.
