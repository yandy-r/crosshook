# Config History Rollback — Recommendation Synthesis

Assumptions applied:

- Requirements come from issue `#46`.
- Earlier research predates metadata DB maturity.
- Metadata DB is now available and should drive architecture.

## Recommended Architecture (Option A: Metadata-backed snapshots, TOML remains source of truth)

Use the metadata DB for revision history, keyed by stable `profile_id`, while keeping live profile state in TOML files.

### Why this is the recommended default

- **Rename-safe history:** `profile_id` survives filename changes; history does not break on `profile_rename`.
- **Fits existing architecture:** follows existing metadata patterns (`version_snapshots`, `launch_operations`, migrations, retention pruning).
- **Operationally simple rollback:** restore = load snapshot -> deserialize `GameProfile` -> `ProfileStore::save` -> `observe_profile_write`.
- **Degrades safely:** if metadata store is unavailable, app still edits/saves profiles (history UI can show unavailable/empty).
- **Enables “last known working”:** reuse launch outcome recording to tag revision associated with successful launches.

### Concrete shape

- Add `config_revisions` table (suggested): `revision_id`, `profile_id`, `snapshot_toml`, `content_hash`, `source`, `created_at`, `is_last_known_working`.
- Insert revision on profile save/import/rollback only when `content_hash` differs from latest revision.
- Cap per-profile revisions (MVP: 5, matching issue text) with deterministic pruning.
- Diff command compares two revisions (or current-vs-revision) and returns unified text/structured lines for UI.

## Alternative Options and Tradeoffs

### Option B: Filesystem-only `.history/<profile>/` snapshots

- **Pros:** easy to inspect manually; no migration needed.
- **Cons:** weak identity across rename/duplicate, harder query/filter/tagging, duplicates data model outside metadata conventions.
- **Verdict:** acceptable fallback, but poorer long-term fit now that metadata DB exists.

### Option C: Patch/delta storage instead of full snapshots

- **Pros:** lower storage footprint at scale.
- **Cons:** much higher complexity (reconstruction reliability, corruption surface, testing burden).
- **Verdict:** defer; over-engineered for initial retention size (N=5..20).

### Option D: Pure `profiles.content_hash` audit without full snapshot body

- **Pros:** minimal storage.
- **Cons:** cannot satisfy rollback acceptance criteria; no practical diff context.
- **Verdict:** insufficient for issue #46.

## Phased Implementation Plan

### MVP (ship issue #46 acceptance criteria)

1. **Schema + core store**
   - Add migration for `config_revisions`.
   - Add metadata store methods: append/list/get/mark_last_known_working/prune.
2. **Capture revisions**
   - Hook profile writes (save/import/rollback path) after successful TOML save.
   - Deduplicate by hash; keep last 5 revisions.
3. **Rollback command**
   - Restore selected revision into profile TOML and resync metadata row/hash.
4. **Diff command + UI**
   - List revisions with timestamp/source.
   - Show revision diff.
   - One-click rollback with confirmation.
5. **Known working tagging**
   - On successful launch completion, mark the latest revision for that profile as `is_last_known_working=true`.

### Follow-up (post-MVP hardening/value)

- Increase retention configurability (global or per-profile setting).
- Add richer diff UX (field-aware grouping, not just line diff).
- Add optional auto-snapshot before migration-like operations (bulk preset changes/import transforms).
- Add correlation UX with version snapshots (`game_updated` + last known working revision hint).

## Key Risks and Mitigations

- **Risk: noisy revision spam from trivial saves**
  - **Mitigation:** hash-based dedupe against latest revision; optional debounce window.
- **Risk: metadata/TOML divergence on partial failures**
  - **Mitigation:** treat TOML save as authoritative; append revision only after successful save; keep rollback idempotent.
- **Risk: rollback to invalid/outdated schema content**
  - **Mitigation:** deserialize/validate before save; block restore with actionable error.
- **Risk: unclear “last known working” semantics (steam_applaunch indeterminate exits)**
  - **Mitigation:** define explicit rule in product decision (see below); keep audit metadata (`source`, timestamp).
- **Risk: DB growth over time**
  - **Mitigation:** strict retention cap + pruning in same transaction as insert.

## Decisions Requiring Product/Maintainer Input

1. **Retention policy:** fixed at 5 (issue default) vs configurable (and max cap).
2. **What counts as “known working”:**
   - strict `exit_code == 0` only, or include current indeterminate success semantics.
3. **Snapshot scope:** all profile writes vs selected write sources only (save/import/rollback/automations).
4. **Diff granularity in MVP:** plain unified text vs field-aware semantic diff.
5. **Metadata unavailable behavior:** hide feature entirely vs read-only warning with no-op actions.
6. **Rollback side effects:** should rollback implicitly create a new revision entry (recommended: yes, with `source=rollback`).
