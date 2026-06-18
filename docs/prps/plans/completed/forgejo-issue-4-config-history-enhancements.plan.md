# Plan: Config history enhancements — semantic diff, retention UI, UX polish

## Summary

Incrementally enhance the shipped config-history MVP (GitHub #46 / Forgejo child of #3) without destabilizing rollback. Work proceeds in four vertical slices: migrate the custom LCS unified diff to the [`similar`](https://docs.rs/similar) crate, add a TOML-aware semantic diff mode, expose a user-configurable retention cap in Settings, and polish the history panel with a collapse-unchanged-hunks toggle. HMAC tamper evidence remains explicitly deferred (advisory threat model; no user pull).

## User Story

As a profile maintainer, I want config history diffs to highlight meaningful TOML field changes (not just line noise), control how many revisions are kept, and skim large profiles quickly — so I can trust rollback when a tweak breaks my trainer launch without reading raw unified diff hunks.

## Problem → Solution

**Current state**: `config_revisions` (SQLite v11) stores deduped `snapshot_toml` with SHA-256 integrity checks and a hardcoded `MAX_CONFIG_REVISIONS_PER_PROFILE = 20` (`metadata/models.rs`). Diffing uses a hand-rolled LCS unified diff in `src-tauri/src/commands/profile/config_history.rs` (`compute_unified_diff`, 2000-line cap, 512 KiB IPC limit). The UI (`ConfigHistoryPanel` + `config-history/*`) renders all diff lines with no collapse toggle. Forgejo [#4](https://git.home.rfamily.dev/yandy/crosshook/issues/4) tracks follow-ups scoped out of the MVP.

**Desired state**: Same persistence contract; richer diff output and settings-driven retention; UI toggles between line unified and semantic TOML views; optional hunk collapsing for readability. Each slice ships independently with tests and mock handlers.

## Metadata

- **Complexity**: Medium (4 incremental slices; ~15–20 files; no SQLite migration)
- **Source issue**: Forgejo [#4](https://git.home.rfamily.dev/yandy/crosshook/issues/4) (GitHub #123)
- **Tracker**: Forgejo [#3](https://git.home.rfamily.dev/yandy/crosshook/issues/3) P1/P2 hygiene
- **Depends on**: Config history MVP (#46) — `config_history_store`, IPC commands `profile_config_history_*`, `ConfigHistoryPanel`
- **Estimated files**: 3 new Rust modules + ~12 edits across core, src-tauri, frontend, mocks, settings
- **Non-goals**: Remote backup/sync, per-profile retention (global-only initially), HMAC signing (defer to separate issue if threat model changes)

---

## Storage Boundary & Persistence

| Datum                                                             | Classification        | Notes                                                                                                                     |
| ----------------------------------------------------------------- | --------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `config_revisions.*` (snapshots, hashes, `is_last_known_working`) | **SQLite metadata**   | Existing v11 table; unchanged schema                                                                                      |
| Retention cap (`max_revisions_per_profile`)                       | **TOML settings**     | New field on `AppSettingsData` (e.g. `[config_history] max_revisions = 20`); replaces compile-time constant at prune time |
| Semantic diff output                                              | **Runtime-only**      | Computed on demand from two `snapshot_toml` strings; not stored                                                           |
| Diff view mode (`unified` \| `semantic`)                          | **Runtime-only**      | Session preference in React state; optional `localStorage` later — not in scope slice 1                                   |
| Collapse-unchanged toggle                                         | **Runtime-only**      | Ephemeral UI state                                                                                                        |
| HMAC key (if ever added)                                          | **Filesystem secret** | Out of scope; would live under CrossHook data dir, not SQLite                                                             |

### Persistence & usability

- **Migration**: No DB migration. Settings field is additive with `#[serde(default)]` defaulting to `20` (current constant).
- **Offline**: Fully local; all enhancements work without network.
- **Degraded fallback**: MetadataStore unavailable → history panel error state; profile editing continues. Rollback failure leaves profile unchanged. Semantic diff parse failure → fall back to unified line diff with inline notice.
- **User visibility**: Revisions list + diff/rollback in Hero Detail history panel; retention cap editable in Settings; snapshots not exposed as editable files.

---

## Incremental Slices

Slices ship as separate PRs (`Part of #4`) in dependency order. Each slice has its own acceptance criteria and test gate.

### Slice 1 — `similar` crate migration (foundation)

**Goal**: Replace custom LCS in `compute_unified_diff` with `similar::TextDiff` while preserving output shape (`ConfigDiffResult`: `diff_text`, `added_lines`, `removed_lines`, `truncated`).

| Task                            | Owner layer                                       | Notes                                                                                   |
| ------------------------------- | ------------------------------------------------- | --------------------------------------------------------------------------------------- |
| 1.1 Add `similar` workspace dep | `crosshook-core/Cargo.toml`                       | Pin version; justify in PR (maintenance reduction)                                      |
| 1.2 Extract diff helper to core | `crosshook-core/src/profile/config_diff.rs` (new) | Move logic out of src-tauri; keep constants (`DIFF_MAX_LINES`, `MAX_DIFF_OUTPUT_BYTES`) |
| 1.3 Wire src-tauri command      | `commands/profile/config_history.rs`              | Thin wrapper calling core helper                                                        |
| 1.4 Unit tests                  | `config_diff` tests                               | Identical-input empty diff; truncation flag; byte cap                                   |

**Acceptance**: Existing config history integration tests pass; diff output format unchanged for frontend.

### Slice 2 — Semantic TOML diff

**Goal**: Add optional semantic diff mode that parses both snapshots with `toml` crate, walks `GameProfile`-equivalent table structure (or generic `toml::Value` tree), and returns grouped field changes.

| Task                     | Owner layer                                                     | Notes                                                                                              |
| ------------------------ | --------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| 2.1 Semantic diff engine | `crosshook-core/src/profile/config_semantic_diff.rs` (new)      | Section/key paths, `added`/`removed`/`changed` with old/new values; stable sort                    |
| 2.2 Extend IPC response  | `ConfigDiffResult` + `profile_config_diff`                      | Add `mode: "unified" \| "semantic"` request param; `semantic_changes: Option<Vec<SemanticChange>>` |
| 2.3 TS types + mock      | `types/profile-history.ts`, `mocks/handlers/profile.ts`         | Mirror Serde shapes                                                                                |
| 2.4 UI mode toggle       | `config-history/DiffView.tsx` or sibling `SemanticDiffView.tsx` | Toggle in revision detail; default unified for parity                                              |
| 2.5 Tests                | Core unit + Vitest component smoke                              | Reorder-only TOML keys → no false "changed"; section move detection                                |

**Acceptance**: User can switch diff mode; semantic view hides line-order noise; unified mode still available.

### Slice 3 — Retention configuration UI

**Goal**: Replace hardcoded prune limit with settings-backed value.

| Task                         | Owner layer                                         | Notes                                                                      |
| ---------------------------- | --------------------------------------------------- | -------------------------------------------------------------------------- |
| 3.1 Settings type            | `settings/types.rs`                                 | `ConfigHistorySettings { max_revisions: u32 }` nested or flat; clamp 5–100 |
| 3.2 Plumb to prune           | `config_history_store/mod.rs`                       | Accept limit param from caller; remove direct const usage at insert        |
| 3.3 Load settings at capture | `capture_config_revision` call sites                | Read settings once per write path                                          |
| 3.4 Settings UI              | Settings panel Profiles or Advanced section         | Number input + help text referencing storage impact                        |
| 3.5 IPC + mock               | `settings_save` round-trip, mock default            |                                                                            |
| 3.6 Tests                    | Settings serde default; prune respects custom limit |

**Acceptance**: Changing setting affects next insert prune; existing revisions below new cap retained until next write.

### Slice 4 — UX polish (collapse unchanged hunks)

**Goal**: Improve readability for large profiles.

| Task                | Owner layer                 | Notes                                                                     |
| ------------------- | --------------------------- | ------------------------------------------------------------------------- |
| 4.1 Collapse filter | `config-history/helpers.ts` | Filter unified diff lines to changed hunks + `DIFF_CONTEXT_LINES` context |
| 4.2 Toggle control  | `RevisionDetail.tsx`        | "Show unchanged sections" checkbox; default collapsed                     |
| 4.3 Semantic parity | `SemanticDiffView`          | N/A — semantic view is already compact                                    |
| 4.4 a11y            | aria labels on toggle       | Mirror existing ConfigHistoryPanel patterns                               |
| 4.5 Vitest          | helpers unit tests          | Hunk collapse with context                                                |

**Acceptance**: Large profile diff readable at a glance; toggle restores full unified output.

---

## Batches (Slice 1 detail)

| Batch | Tasks    | Depends On | Parallel |
| ----- | -------- | ---------- | -------- |
| B1    | 1.1, 1.2 | —          | 2        |
| B2    | 1.3, 1.4 | B1         | 2        |

Slices 2–4 each follow the same batch pattern in their own PR.

---

## Mandatory Reading

| Priority | File                                                                     | Why                                           |
| -------- | ------------------------------------------------------------------------ | --------------------------------------------- |
| P0       | `src-tauri/src/commands/profile/config_history.rs`                       | Current diff + IPC commands                   |
| P0       | `crosshook-core/src/metadata/config_history_store/mod.rs`                | Insert, dedup, prune                          |
| P0       | `crosshook-core/src/metadata/models.rs`                                  | `MAX_CONFIG_REVISIONS_PER_PROFILE`, row types |
| P0       | `src/components/ConfigHistoryPanel.tsx` + `config-history/*`             | UI integration points                         |
| P0       | Forgejo #4 issue body                                                    | Acceptance + storage table                    |
| P1       | `crosshook-core/tests/config_history_integration.rs`                     | Integration test patterns                     |
| P1       | `settings/types.rs`                                                      | Settings extension pattern                    |
| P1       | `src/hooks/profile/useProfileHistory.ts`                                 | Frontend invoke wiring                        |
| P2       | `docs/prps/plans/completed/github-issue-468-launch-hooks-schema.plan.md` | Plan doc structure reference                  |

## External Documentation

- **similar**: <https://docs.rs/similar> — `TextDiff::from_lines`, unified diff formatting
- **toml**: workspace crate — parse snapshots for semantic diff (already used for profiles)

---

## Testing Strategy

| Layer            | Coverage                                                                                    |
| ---------------- | ------------------------------------------------------------------------------------------- |
| Rust unit        | `config_diff`, `config_semantic_diff`, settings clamp, prune with custom limit              |
| Rust integration | Existing `config_history_integration.rs` + semantic diff fixture profiles                   |
| Vitest           | `config-history/helpers` collapse logic; DiffView mode toggle smoke                         |
| Manual           | Native dev: edit profile → history → diff both modes → rollback → settings retention change |

Target: no regression in rollback integrity checks (SHA-256 verify on restore).

---

## Risks & Mitigations

| Risk                                          | Mitigation                                                          |
| --------------------------------------------- | ------------------------------------------------------------------- |
| Semantic diff false positives on TOML reorder | Compare normalized `toml::Value` trees, not raw text                |
| IPC payload growth                            | Keep semantic changes bounded; truncate with flag like unified diff |
| Settings set below current revision count     | Prune on next insert only; document in UI                           |
| `similar` output drift breaks CSS             | Keep `+`/`-`/`@@` line prefixes; snapshot golden tests              |

---

## Success Criteria

- [ ] Slice 1 merged: custom LCS removed; `similar` powers unified diff
- [ ] Slice 2 merged: semantic TOML diff available in UI
- [ ] Slice 3 merged: retention cap user-configurable via Settings
- [ ] Slice 4 merged: collapse-unchanged toggle shipped
- [ ] All slices: mocks updated; `cargo test -p crosshook-core`; `npm test` for touched TS
- [ ] ROADMAP / #3 tracker updated when #4 closes

---

## PR Checklist

- PR title: Conventional Commits (`feat(profiles): …`, `feat(settings): …`)
- Link: `Part of #4` (Forgejo)
- Labels: `type:feature`, `area:profiles`, `area:ui`
- No SQLite migration unless HMAC slice is later promoted
