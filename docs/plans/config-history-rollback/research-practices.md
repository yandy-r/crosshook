# Config history & rollback — engineering practices research

**Scope:** CrossHook feature aligned with issue **#46** (configuration history / rollback), building on the existing **metadata SQLite database** (`MetadataStore`, `~/.local/share/crosshook/metadata.db` via `directories::BaseDirs`).

**Goal of this doc:** Reuse existing patterns, keep the first ship small (KISS), draw clean module boundaries, and make testing and dependency choices explicit.

---

## 1. Existing reusable code to leverage

### Metadata database (primary anchor)

The metadata layer is the right home for **durable, queryable history** that must survive app restarts and stay keyed to stable profile identity.

- **`MetadataStore`** (`crates/crosshook-core/src/metadata/mod.rs`): central façade; `with_conn` / `with_conn_mut`, `is_available()`, and **disabled-store semantics** (`MetadataStore::disabled()` returns success with empty/default results). Any history feature must **degrade gracefully** when metadata is unavailable — same pattern as health/version/launch history.
- **Migrations** (`metadata/migrations.rs`): new tables belong here with a monotonic `user_version` bump; follow existing DDL style and FK patterns (e.g. `version_snapshots` → `profiles.profile_id`).
- **Submodule layout:** mirror `health_store.rs`, `version_store.rs`, `launch_history.rs` — implement SQL in a focused module (e.g. `config_history_store.rs`), expose thin methods on `MetadataStore`, re-export row types from `metadata/models.rs` as needed.

### Profile sync and content identity

- **`profile_sync::observe_profile_write`** (`metadata/profile_sync.rs`): already computes **`content_hash`** as SHA-256 of `toml::to_string_pretty(profile)` and upserts `profiles`. **Reuse this hash** (or the same serialization) to **deduplicate** history rows: if the user saves without semantic change, skip inserting a new revision (avoids noise and DB growth).
- **`lookup_profile_id`** / **`MetadataStore::lookup_profile_id`**: history rows should reference **`profile_id`**, not only `current_filename`, so renames keep history attached (consistent with `launch_operations`, `version_snapshots`, etc.).

### Commands and IPC

- **`src-tauri/src/commands/profile.rs`**: `observe_profile_write_*` helpers already call `metadata_store.observe_profile_write` after saves. **Hook revision capture at the same boundary** (after successful `ProfileStore` write, before/after emit) so disk and metadata stay aligned.
- **Serde IPC structs:** follow existing command style (`Deserialize`/`Serialize`, snake_case field names matching `invoke` from TS).

### Types and domain

- **`GameProfile`** and `ProfileStore` (`crates/crosshook-core/src/profile/`): rollback is “replace in-memory profile + persist TOML” — no new domain type is strictly required for v1; optional labels/reason enums can stay strings until a second consumer appears.

### UI patterns

- **Hooks + `invoke`:** e.g. `useProfile.ts`, `useProfileHealth.ts` — add a small hook for list/diff/rollback with loading/error state like existing flows.
- **Layout:** `CollapsibleSection`, modals (`ProfileReviewModal`, `LauncherPreviewModal`) — a **revision list + confirm rollback** modal fits established patterns; avoid a new top-level page unless the list grows large enough to warrant it (see KISS).

### Analogy: version snapshot pruning

- **`version_store.rs`** + `MAX_VERSION_SNAPSHOTS_PER_PROFILE` (`metadata/models.rs`): demonstrates **append + cap + prune** for per-profile rows. Config history should use the **same retention philosophy** (fixed max revisions per profile, deterministic eviction).

---

## 2. Modular boundaries and API design

### Suggested layering

| Layer                                                                      | Responsibility                                                                                                                                                                                          |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`crosshook-core`** `metadata::config_history_store` (name TBD)           | SQL: insert revision (deduped), list by `profile_id`, fetch by id, optional prune. Pure-ish helpers for diff input (strings / lines).                                                                   |
| **`MetadataStore`**                                                        | Thin wrappers only — no business rules that belong in Tauri.                                                                                                                                            |
| **`crosshook-core`** `profile` or small `config_history` module (optional) | Orchestration: “apply revision to `ProfileStore` + call `observe_profile_write`” if you want CLI reuse later; otherwise keep orchestration in Tauri `profile` command first, extract when CLI needs it. |
| **`src-tauri` commands**                                                   | `list_config_revisions`, `get_config_revision`, `diff_config_revisions`, `rollback_config_revision` — validate names, call store, emit `profiles-changed` on rollback.                                  |
| **Frontend**                                                               | List, diff viewer, rollback confirm; no SQLite knowledge.                                                                                                                                               |

### API shape (illustrative)

- **List:** returns lightweight rows: `revision_id`, `created_at`, optional `label`/`source` (e.g. `app_save`, `import`, `rollback`), `content_hash` or size — **not** full TOML for every row.
- **Fetch body:** separate command or optional `include_body` to avoid huge payloads in list views.
- **Diff:** accept two revision ids (or “current disk vs revision”) and return a **structured line diff** (or unified text) generated in Rust — keeps behavior identical for UI and future CLI.

### Explicit use of metadata vs TOML-only history

- **Canonical state** remains **`ProfileStore` TOML files** (user-editable, backup-friendly).
- **Metadata DB** holds **historical copies or patches** for rollback UX — not a second source of truth for the live profile. On rollback, write through `ProfileStore` and let existing `observe_profile_write` refresh `profiles.content_hash`.

---

## 3. KISS — avoid over-engineering

**Ship a thin vertical slice first:**

1. On successful save (and optionally import), append a revision **only if `content_hash` changed**.
2. Store **full serialized snapshot** (pretty TOML string or canonical bytes) with a **hard cap** per profile and simple **FIFO or “keep last N”** pruning.
3. Rollback = **load snapshot → validate deserialize → save profile → observe metadata**.

**Defer until there is proven need:**

- Binary diffs, Merkle chains, CRDTs, or event-sourcing.
- Storing only patches (smaller DB but more failure modes and test surface).
- Cross-profile or global history.
- Automatic rollback tied to launch outcome (could **correlate** with `launch_operations` later using timestamps/`profile_id`, but do not block v1 on it).

**Rule of three:** if “get diff” is only used in one UI, a single `diff_two_strings` helper in core is enough; do not build a generic diff framework until a second consumer exists.

---

## 4. Abstraction recommendations

| Extract now                                             | Leave duplicated / inline                                                      |
| ------------------------------------------------------- | ------------------------------------------------------------------------------ |
| SQLite access in `*_store.rs` + `MetadataStore` methods | UI formatting of timestamps (TS vs Rust) — pick one place for “display string” |
| Dedup: compare new hash to **latest revision** hash     | Fancy diff HTML rendering — start with `<pre>` + line prefixes                 |
| Prune helper patterned after `version_store`            | Separate crate for “revision domain”                                           |

---

## 5. Testability guidance

### Unit tests (Rust, `crosshook-core`)

- Use **`MetadataStore::open_in_memory()`** and the existing pattern of grabbing the mutex-guarded `Connection` in tests (see `metadata/mod.rs` `#[cfg(test)]` module).
- Cover: insert idempotency when hash unchanged; prune when over cap; FK behavior with tombstoned/deleted profiles; **disabled store** returns empty list / no-op writes without panic.
- **Diff logic:** table-driven tests on small multi-line strings (add/remove/changed lines).

### Integration-style tests

- **Tauri commands:** optional later; highest value is core + `ProfileStore::with_base_path` temp dir: save profile → mutate → save → list revisions → rollback → assert file bytes or hash matches.

### Frontend

- No mandated test framework in repo today; if adding coverage, prefer **pure TS helpers** for formatting diff output, mocked `invoke`.

---

## 6. Build vs. depend (diff / snapshot logic)

| Approach                                           | Fit for CrossHook                                                    | Notes                                                                                                                               |
| -------------------------------------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| **Store full TOML text**                           | **Recommended for v1**                                               | Simple rollback (`String` → parse → save). DB size managed by **N-revision cap**. Aligns with how `content_hash` is computed today. |
| **Line diff crate** (e.g. `similar`, `dissimilar`) | **Reasonable** if you want readable UI diff without writing a differ | Small, focused deps; no VCS coupling.                                                                                               |
| **`git diff` / libgit2**                           | **Avoid**                                                            | Heavy, overkill, environment-dependent for a launcher config file.                                                                  |
| **Custom minimal LCS diff**                        | Only if dependency policy blocks adds                                | Higher bug risk; prefer a maintained crate.                                                                                         |
| **Operational transform / patches**                | Defer                                                                | Saves space but increases complexity and failure modes.                                                                             |

**Serialization:** use the same canonicalization as `compute_content_hash` (`toml::to_string_pretty`) when storing snapshots so hash comparisons and file round-trips stay consistent.

---

## 7. Summary checklist for implementers

1. **New migration** + `config_history_store` (or equivalent) under `metadata/`, keyed by **`profile_id`**.
2. **Dedup** using existing **`profiles.content_hash`** semantics (SHA-256 of pretty TOML).
3. **Prune** like **`version_snapshots`** — bounded rows per profile.
4. **Wire** revision append where **`observe_profile_write`** already runs (`profile` command path).
5. **Tauri commands** + small **React hook**; modal/section UI consistent with existing components.
6. **Tests** with `open_in_memory()` + disabled-store cases.
7. **KISS:** full snapshots + optional small diff crate; no git, no patch-only storage in v1.

This keeps configuration history aligned with the **metadata DB’s role** as CrossHook’s durable observability and indexing layer while leaving **TOML files** as the single live source of truth for profile data.
