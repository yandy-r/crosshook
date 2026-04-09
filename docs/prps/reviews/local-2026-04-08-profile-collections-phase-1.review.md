# Code Review — Local (uncommitted)

**Date**: 2026-04-08
**Branch**: `feat/profile-collections-phase-1`
**Scope**: Profile Collections — Phase 1 (Backend Foundation)
**Reviewer**: Claude Code (automated)
**Decision**: ✅ **Approve**

---

## Summary

Reviewed the 8 files changed by
`docs/prps/archived/profile-collections-phase-1-backend-foundation.plan.md` against
CLAUDE.md standards, common OWASP-style vulnerability patterns, and the project's
repo-specific rules (Tauri IPC, SQLite migration conventions, mock handler sentinel).

**Statistics**:

- 7 files modified, 1 file created (`src/lib/mocks/handlers/collections.ts`)
- 481 lines added, 12 removed
- Compiled clean (`cargo check -p crosshook-core` + src-tauri lib)
- 0 new clippy warnings (30 warnings all pre-existing on main)
- 756 crosshook-core lib tests pass + 3 integration tests, including 8 new tests

**No blocking issues**. Two minor observations and one nit — all acceptable for
merge, no changes required.

---

## What I Checked

### Security

- [x] SQL injection — all queries use parameterized placeholders (`?1`, `?2`, `?3` + `params![...]`). Zero string concatenation with user input. Confirmed 19
      parameter references across `collections.rs`.
- [x] Secret handling — no `.env`, credentials, or tokens touched.
- [x] Input validation — `trim()` + non-empty check on name fields; `Validation`
      errors surface at the free-function boundary before hitting SQL.
- [x] Error information leak — error messages include user-supplied `collection_id`
      / `profile_name` values, which is safe in Tauri's trust model (frontend is
      the same trust boundary as the backend).
- [x] Mock sentinel integrity — every thrown error in
      `src/lib/mocks/handlers/collections.ts` starts with `[dev-mock]`, so the
      `release.yml:105-120` grep is satisfied. Zero new `[dev-mock]` strings leak
      into the production bundle (verified by building and grepping
      `dist/assets/*.js` for `[dev-mock] collection_*`, `registerCollections`, and
      `mock-collection-1` — all zero).
- [x] No `any` types — mock handlers cast `args as { ... }` per CLAUDE.md type
      safety rule.

### CLAUDE.md conformance

- [x] **Tauri IPC snake_case** — `collection_rename`, `collection_update_description`,
      `collections_for_profile` all snake_case.
- [x] **Error handling — throw early, no fallbacks** — `add_profile_to_collection`
      now throws `Validation` instead of swallowing missing profile + warn-logging.
      The Task 2 fix is exactly the "throw errors early and often, do not use
      fallbacks" rule from the CLAUDE.md core principles.
- [x] **`MetadataStoreError::Validation(String)` tuple variant** used
      consistently — no struct syntax slip-ups.
- [x] **Repository pattern** — free function in `collections.rs` → `MetadataStore`
      wrapper in `mod.rs` via `with_conn(...)` → `#[tauri::command]` in
      `src-tauri/src/commands/collections.rs` → registration in `lib.rs`. All
      four layers present for all 3 new commands.
- [x] **Rust naming conventions** — `snake_case` functions, `PascalCase` types.
- [x] **React / TypeScript** — `PascalCase` type (`MockCollectionRow`),
      `camelCase` functions (`nowIso`, `recomputeProfileCounts`, `findById`,
      `registerCollections`). No new scroll containers (not applicable).
- [x] **Platform** — change is native Linux Tauri app; no wine/proton assumption
      leakage.
- [x] **Migration policy** — `migrate_18_to_19` is appended, existing migrations
      are **not** modified (SQLite migrations are immutable once released). The
      one touch to an existing test assertion (see `Observation 2` below) is not a
      migration change.

### Schema v19 migration

- [x] Single `execute_batch` transaction — `BEGIN ... COMMIT` atomicity — if the
      `ALTER TABLE ... ADD COLUMN` succeeds but the table rebuild fails, the
      entire transaction rolls back. No half-migrated state possible.
- [x] **FK cascade correctness** — `collection_profiles.profile_id REFERENCES
  profiles(profile_id) ON DELETE CASCADE` is verified end-to-end by the new
      `migration_18_to_19_adds_sort_order_and_cascade` test (insert profile → add
      to collection → delete profile → assert `COUNT(*) WHERE profile_id='pf-1' ==
  0`). The test also regression-checks the existing collection→membership
      cascade.
- [x] **`sort_order INTEGER NOT NULL DEFAULT 0`** — SQLite accepts this because
      `0` is a constant default, so the backfill on existing rows is automatic.
      The test verifies `PRAGMA table_info(collections)` shows the column with
      `NOT NULL = 1`.
- [x] **Index preserved** — `CREATE INDEX IF NOT EXISTS idx_collection_profiles_profile_id`
      is recreated after the table rebuild. The old index was dropped with the
      old table.
- [x] `db.rs:72` confirms `PRAGMA foreign_keys=ON` on open, so the cascade will
      actually fire at runtime, not just exist as a schema declaration.

### Collections free functions

- [x] **`add_profile_to_collection` fix** — replaced the `let Some(profile_id) = ... else`
      pattern with `ok_or_else` + `Validation`. The `tracing::warn!` is correctly
      dropped (the error is now the surfacing mechanism — no duplicate logging).
- [x] **`rename_collection`** — trims input, rejects empty, issues UPDATE, checks
      `affected == 0` → `Validation`, bubbles UNIQUE collision as `Database`.
      The `affected == 0` branch is load-bearing — without it, renaming a bogus
      id would silently succeed. Tests cover all three paths.
- [x] **`update_collection_description`** — whitespace normalization
      (`Some("   ") → None`) matches the "clear field" UX convention. `affected == 0`
      path mirrors `rename_collection`.
- [x] **`collections_for_profile`** — returns `Ok(vec![])` on unknown profile
      name, not `Err` — matches the documented convention of
      `list_profiles_in_collection`. The JOIN + `sort_order ASC, name ASC`
      ordering matches `list_collections`.
- [x] **`list_collections` ORDER BY update** — the change from `ORDER BY c.name`
      to `ORDER BY c.sort_order ASC, c.name ASC` is safe because migration v19
      backfills `sort_order = 0` for all existing rows, so existing output
      remains name-sorted until Phase 2 starts writing non-zero values.

### Tauri command layer

- [x] Correct arg ordering — positional args come **before**
      `metadata_store: State<'_, MetadataStore>` (Tauri requires `State` last).
- [x] `Result<T, String>` return + `.map_err(map_error)` tail — matches the rest
      of the file exactly.
- [x] `collection_update_description` uses `description: Option<String>` with
      `.as_deref()` to convert to `Option<&str>` — correct Tauri JSON
      `null`/missing → `None` deserialization.
- [x] `collections_for_profile` returns `Vec<CollectionRow>` — uses the now-serializable
      `CollectionRow` (with the `#[allow(dead_code)]` removed).

### Mock handler

- [x] All 9 commands covered: `collection_list`, `collection_create`,
      `collection_delete`, `collection_add_profile`, `collection_remove_profile`,
      `collection_list_profiles`, `collection_rename`,
      `collection_update_description`, `collections_for_profile`.
- [x] Every `throw new Error(...)` starts with `[dev-mock]` — 9 throws checked.
- [x] `recomputeProfileCounts()` is called on every list-returning handler
      (`collection_list`, `collections_for_profile`) — keeps the in-memory counts
      consistent with the membership map.
- [x] Idempotent `collection_remove_profile` matches Rust's
      `remove_profile_from_collection` semantics per `collections.rs:117-120`
      (documented inline in the mock).
- [x] `collection_add_profile` mock rejects empty `profile_name` and unknown
      names (validated against `getStore().profiles`) with `[dev-mock]` errors —
      mirrors the Task 2 Rust fix.
- [x] Seed fixture uses `mock-collection-1` synthetic prefix — complies with
      `.github/workflows/fixture-lint.yml` (no real Steam IDs or PII).

### Tests

- [x] **8 new tests, all passing** (1 migration + 7 metadata store):
  - `migration_18_to_19_adds_sort_order_and_cascade`
  - `test_add_profile_to_collection_missing_profile_errors`
  - `test_rename_collection_updates_name`
  - `test_rename_collection_unknown_id_errors`
  - `test_rename_collection_duplicate_name_errors`
  - `test_update_collection_description_set_and_clear`
  - `test_collections_for_profile_returns_multi_membership`
  - `test_profile_delete_cascades_collection_membership`
- [x] Tests use `MetadataStore::open_in_memory()`, `sample_profile()`,
      `connection(&store)` — the established Phase 3 Collections test idiom.
- [x] FK cascade test does a **raw SQL hard-delete** (not `observe_profile_delete`
      which is a soft-delete), correctly exercising the cascade path. The
      comment block explains why — protects future readers from "why did you
      bypass the soft-delete API?"
- [x] `test_rename_collection_duplicate_name_errors` asserts
      `Database { .. }`, not `Validation` — correct, because UNIQUE is enforced
      at the SQLite layer (no pre-check in Rust, which is the plan's intentional
      design).
- [x] No regressions — all 748 pre-existing tests still pass (748 + 8 = 756).

---

## Observations (non-blocking)

### Obs 1 — Minor divergence between Rust and mock error classes for duplicate-name create

**Where**: `mocks/handlers/collections.ts:60-62` vs `collections.rs:46-68`.

The mock's `collection_create` pre-checks for a duplicate name and throws
`[dev-mock] collection_create: duplicate collection name: <name>`. The Rust
`create_collection` does **not** pre-check; it relies on SQLite's UNIQUE
constraint to fire, which surfaces as
`MetadataStoreError::Database { action: "insert a new collection", source: SqliteFailure(...) }`.

**Impact**: Both paths throw on duplicate create, but the error message shape
differs. Frontend code that parses the error string would see different content
in mock mode vs native mode.

**Recommendation**: Not a blocker for Phase 1. Phase 2's rename/create modal
should not depend on exact error message text — it should catch the error and
show the user "name already in use" regardless of the error class. If the
frontend ends up needing to disambiguate, the plan documents this as a
follow-up option (optionally pre-check in Rust for a cleaner error class — see
Risks table in the original plan).

No change requested.

### Obs 2 — `migration_17_to_18_creates_trainer_sources_table` assertion loosened from `==` to `>=`

**Where**: `migrations.rs:1089-1094`.

The existing test used `assert_eq!(version, 18)` for the exact schema version.
That assertion would fail the moment migration 19 was appended. I changed it
to `assert!(version >= 18, ...)` to match the looser convention used by the
older migration tests (`14_to_15` at line 919, `15_to_16` at line 980,
`16_to_17` at line 1017).

**Impact**: Strictly a test quality improvement. Prevents needing to touch this
test each time a new migration is added. Plan notes this as the only deviation
from the plan spec.

**Recommendation**: Accept as-is. Optionally, the same treatment could be
retroactively applied to any future test that uses exact-equality on the
latest schema version, but that's out of scope for this PR.

### Obs 3 — Mock `collection_create` ID uses `Date.now().toString(36)` (nit)

**Where**: `mocks/handlers/collections.ts:63`.

Two rapid back-to-back calls in the same millisecond produce the same id
(`mock-collection-${Date.now().toString(36)}`), which would then trip the
duplicate-name check on the next call but not the duplicate-id check (there
isn't one). This would manifest as the `collections` array containing two rows
with the same `collection_id`, which would break the `findById` behavior for
the second row.

**Impact**: Theoretical. Devs rarely click "Create" twice within 1ms. This is mock-only and has zero production impact.

**Recommendation**: Not worth fixing now. If Phase 2 adds a "Create N
collections at once" test fixture or rapid-click scenario, swap to
`crypto.randomUUID()` then. Zero action required for Phase 1.

---

## Strengths

- **Verbatim pattern matching** — every new function mirrors an existing one
  structurally. Low cognitive load for reviewers and future maintainers.
- **Test coverage is end-to-end** — the FK cascade test actually inserts,
  deletes, and asserts on raw SQL, not just on the schema declaration. The
  migration test exercises the cascade behavior, not just "did the column
  exist".
- **Mock/native parity is respected** — mock semantics intentionally mirror
  Rust semantics, including the subtle idempotent-remove vs strict-add
  asymmetry.
- **Clean separation of layers** — free function → store wrapper → Tauri
  command → handler registration. No logic leakage across layers.
- **Error boundary hygiene** — the Task 2 fix ("replace silent no-op with
  typed Validation error") is textbook throw-early-fail-loud. `tracing::warn!`
  was correctly removed, not left as a duplicate of the error surfacing.
- **Report quality** — the implementation report documents validation results
  with command + exit code, separates pre-existing warnings from new ones, and
  provides manual test commands for native verification.

---

## Decision

**✅ Approve.** No blocking issues, no change requests.

Ready for:

1. `/ycc:prp-pr` — create the pull request
2. Commit split per the report's commit plan (4 Conventional Commits linking
   `Closes #73` with `type:feature`, `area:profiles`, `priority:high` labels)
