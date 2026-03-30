# Config History Rollback — External Research

Context: prior research predated the metadata DB; this update assumes metadata DB is available and preferred for revision persistence.

## 1) TOML structural diffing + pretty rendering

### Candidate libraries/patterns

- [`toml_edit`](https://docs.rs/toml_edit/latest/toml_edit/) for parse/edit with formatting-preserving AST (`DocumentMut`).
  - Pros: preserves comments/ordering intent for user-facing config; supports formatting controls and spans.
  - Cons: not a semantic diff engine by itself; still requires comparison layer.
- [`toml`](https://docs.rs/toml/latest/toml/) for strict parse/serialize into typed structs or `toml::Value`.
  - Pros: simple, stable serde path; good for validation before rollback apply.
  - Cons: loses original formatting/comments when reserialized.
- [`similar`](https://docs.rs/similar/latest/similar/) for line/word/inline diff rendering (`TextDiff`, unified diff support).
  - Pros: mature Rust diff API, multiple algorithms, timeout/deadline support for large docs.
  - Cons: text-oriented; not TOML-aware semantically.
- [`diffy`](https://docs.rs/diffy/latest/diffy/) for unified patches and patch apply.
  - Pros: easy unified diff output; patch/merge APIs.
  - Cons: also text-level; can be brittle for “semantic rollback” workflows.

### Practical recommendation

- Depend on `toml_edit` + `similar`.
- Build a thin in-house “structural projection” step:
  1. Parse both revisions with `toml_edit`.
  2. Normalize key paths into a flattened map (e.g., `graphics.vsync=true`).
  3. Render two views:
     - Semantic field-change list (added/removed/changed keys).
     - Pretty unified text diff (for power users).
- Build-vs-depend call:
  - **Depend** for parser and diff rendering primitives.
  - **Build** only the TOML path-normalization and UI-oriented semantic grouping (CrossHook-specific behavior).

## 2) Immutable snapshots + retention policies

### Storage model references

- SQLite transactions and isolation: [`lang_transaction`](https://www.sqlite.org/lang_transaction.html)
- SQLite WAL mode for write/read concurrency: [`wal`](https://www.sqlite.org/wal.html)
- SQLite `RETURNING` for atomic insert+id retrieval: [`lang_returning`](https://sqlite.org/lang_returning.html)
- SQLite partial indexes for focused query performance: [`partialindex`](https://sqlite.org/partialindex.html)
- SQLite window functions for pruning policies: [`windowfunctions`](https://sqlite.org/windowfunctions.html)
- SQLite triggers/`RAISE()` for immutability constraints: [`lang_createtrigger`](https://www.sqlite.org/lang_createtrigger.html)

### Recommended persistence pattern

- Append-only `config_revisions` table keyed by stable `profile_id`, never `UPDATE` revision body.
- Add `content_hash` (dedupe), `source` (save/import/rollback), `created_at`, optional `actor`.
- Enforce immutability at DB level:
  - Trigger blocks `UPDATE`/`DELETE` on historical rows except controlled retention job path.
- Retention policy:
  - Start with per-profile keep-last-N (e.g., 20) + optional age cutoff.
  - Prune in same transaction as insert to keep bounded growth.
- Query/index strategy:
  - Composite index `(profile_id, created_at DESC)`.
  - Optional partial index if filtering common states (e.g., “last known good” marker).

### Build-vs-depend call

- **Build** this atop existing metadata DB/rusqlite (no new storage dependency needed).
- **Do not add** external event-store DB; complexity is not justified for current scope.

## 3) Rollback safety patterns

### Patterns with concrete references

- Transaction discipline:
  - Use explicit `BEGIN IMMEDIATE` flow for rollback apply to avoid mid-flight write races ([SQLite transactions](https://www.sqlite.org/lang_transaction.html)).
- Nested safety checkpoints:
  - Use `rusqlite::Savepoint` for staged validation/apply/metadata sync rollback points ([rusqlite Savepoint](https://docs.rs/rusqlite/latest/rusqlite/struct.Savepoint.html)).
- Guardrails before apply:
  - Parse + deserialize candidate snapshot (`toml`/domain model) before writing profile TOML.
  - Create a pre-rollback recovery snapshot automatically.
- Integrity/audit:
  - Store hash of snapshot body and verify before apply.
  - Record rollback event as a new immutable revision (`source=rollback`), not destructive rewind.

### Recommended rollback flow

1. Resolve selected revision by `revision_id`.
2. Validate parse/deserialization.
3. Open transaction/savepoint.
4. Write TOML to profile path.
5. Append new revision capturing post-rollback state.
6. Commit; on any error, rollback and keep current config untouched.

## 4) UX patterns for desktop version-history timelines

### External product patterns

- JetBrains Local History ([docs](https://www.jetbrains.com/idea/help/local-history.html)):
  - Timeline list + side-by-side diff.
  - Restore full file or selected fragments.
  - Labels/bookmarks for major changes.
  - Explicit retention window messaging.
- VS Code Local History in Timeline ([v1.66 notes](https://code.visualstudio.com/updates/v1_66)):
  - Per-file timeline entries on save.
  - User-configurable max entries and file size caps.
  - Restore and compare actions at entry level.
- GitHub Desktop history ([docs](https://docs.github.com/en/desktop/making-changes-in-a-branch/viewing-the-branch-history-in-github-desktop)):
  - Simple chronological history, inspect change details quickly.

### UX implications for CrossHook

- Show per-profile chronological timeline with source badges (`save`, `import`, `rollback`, `auto`).
- Primary CTA: “Preview diff”; secondary CTA: “Rollback”.
- Add two safety affordances:
  - Confirmation modal with “create recovery snapshot” (default ON).
  - “Last known working” pin/label for fast safe restore.
- Communicate retention clearly in UI (“Keeping last N revisions for this profile”).

## 5) Implications for Rust + Tauri stack

- Rust backend should own all diff + rollback logic; frontend consumes typed DTOs via Tauri commands.
- Prefer metadata DB (rusqlite) for revision history, not filesystem `.history` folders.
- Return both semantic changes and unified-text chunks for UI flexibility.
- Keep frontend lean: render timeline/diff, request rollback, display validation errors.
- If any client-side file access is needed, ensure scoped permissions via Tauri FS plugin security model ([plugin docs](https://v2.tauri.app/plugin/file-system/)).

## 6) Final build-vs-depend recommendation

- **Depend:** `toml_edit`, `toml`, `similar` (and existing `rusqlite` stack).
- **Build in-house:** semantic TOML diff projection, retention policy SQL, rollback orchestration, timeline UX behavior.
- **Avoid now:** delta-chain storage and heavy external event-sourcing frameworks.
