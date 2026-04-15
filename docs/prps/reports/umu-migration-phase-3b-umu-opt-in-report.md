# Report: umu Migration Phase 3b — umu-database Coverage Warning + HTTP Cache

> Implementation report for
> [`umu-migration-phase-3b-umu-opt-in.plan.md`](../plans/umu-migration-phase-3b-umu-opt-in.plan.md)
> (executed 2026-04-14 on branch `feat/umu-migration-phase-3`). Covers **only** Phase 3b scope; the
> shipped Phase 3a `UmuPreference::Umu` opt-in is reported separately in
> [`umu-migration-phase-3-umu-opt-in-report.md`](./umu-migration-phase-3-umu-opt-in-report.md).

## Executive summary

Phase 3b adds a new `crosshook_core::umu_database` module that tells users — **before they click
Launch** — whether umu-launcher has a protonfix entry for their Steam app id. A dormant HTTP cache
(with ETag/If-None-Match conditional GET) keeps the CSV fresh without depending on a host
`umu-launcher` install, making the feature Flatpak-safe.

All 15 plan tasks shipped across 4 execution batches using the `--team` (parallel sub-agent) mode.
Zero tasks were deferred. Zero scope items were skipped. Five new files, sixteen modified files, one
new workspace dependency (`csv = "1"`).

## Task completion

| Batch | Tasks                  | Outcome                                  |
| ----- | ---------------------- | ---------------------------------------- |
| C1    | C1.1, C1.2, C1.3       | ✅ 3 parallel (csv dep + module)         |
| C2    | C2.1, C2.2, C2.3       | ✅ 3 parallel (preview + IPC + TS)       |
| C3    | C3.1, C3.2, C4.2, C4.3 | ✅ 4 parallel (UI + startup + align)     |
| C4    | C4.1                   | ✅ (HTTP client — executed in C2 batch)  |
| C5/C6 | C5.1, C5.2, C6.1–3     | ✅ 5 parallel (settings + mocks + tests) |
| C7    | C7.1                   | ✅ PRD closeout                          |

Total: **15 / 15** tasks complete. Max parallel width achieved: **5**.

## Files changed

### Rust (crosshook-core)

- `crates/crosshook-core/Cargo.toml` — added `csv = "1"` dep (resolves to v1.4.0).
- `crates/crosshook-core/src/lib.rs` — `pub mod umu_database;`
- `crates/crosshook-core/src/umu_database/mod.rs` — **NEW**.
  `CsvCoverage { Found, Missing, Unknown }` enum, re-exports.
- `crates/crosshook-core/src/umu_database/coverage.rs` — **NEW**. `check_coverage(app_id, store)`,
  `(path, mtime)`-keyed index cache, inline unit test.
- `crates/crosshook-core/src/umu_database/paths.rs` — **NEW**. 5-tier precedence: HTTP cache →
  `/usr/share/umu-protonfixes/` → `/usr/share/umu/` → `/opt/umu-launcher/…/` → `$XDG_DATA_DIRS`.
- `crates/crosshook-core/src/umu_database/client.rs` — **NEW**. `reqwest` singleton, conditional GET
  (`If-None-Match` + `If-Modified-Since`), atomic disk write via `fs::rename`, SQLite metadata
  persistence in `external_cache_entries`. Custom `Error` enum with manual `Display` impl (no
  `thiserror` dep).
- `crates/crosshook-core/src/launch/preview.rs` — `UmuDecisionPreview.csv_coverage` field + 3 new
  `preview_reports_csv_coverage_*` tests.
- `crates/crosshook-core/src/launch/script_runner.rs` — promoted `fn resolve_steam_app_id_for_umu` →
  `pub(crate)` so preview can reuse the precedence.
- `crates/crosshook-core/tests/umu_database_coverage.rs` — **NEW**. 4 integration tests (fixture
  CSV, Found/Missing/Unknown/mtime-invalidation).
- `crates/crosshook-core/tests/umu_database_http_cache.rs` — **NEW**. 4 wiremock tests (2xx persist,
  304 no-change, network-err cleanup, ETag roundtrip).

### Tauri (src-tauri)

- `src-tauri/src/commands/umu_database.rs` — **NEW**.
  `#[tauri::command] pub async fn refresh_umu_database`.
- `src-tauri/src/commands/mod.rs` — `pub mod umu_database;`
- `src-tauri/src/lib.rs` — `generate_handler!` registration + non-blocking startup
  `tauri::async_runtime::spawn` that calls `refresh_umu_database()`.

### Frontend (src)

- `src/types/launch.ts` — `UmuCsvCoverage` type alias, `csv_coverage` required field on
  `UmuDecisionPreview`, `UmuDatabaseRefreshStatus` interface.
- `src/components/LaunchPanel.tsx` — 3-state chip modifier (`--umu` / `--proton` / `--warn`),
  coverage line, conditional amber remediation sub-line.
- `src/components/PinnedProfilesStrip.tsx` — optional `umuCoverageWarnByProfile` prop + amber ⚠
  badge (dormant — see "Deferred follow-ups" below).
- `src/components/SettingsPanel.tsx` — "Refresh umu protonfix database" button + `isRefreshing` /
  `lastRefreshStatus` state, placed below the Phase 3a `umu_preference` dropdown.
- `src/lib/mocks/handlers/launch.ts` — populated `umu_decision` (incl. `csv_coverage`) in both the
  validation-issues and populated `preview_launch` branches (closed a latent gap where
  `umu_decision` was always `null` in browser dev mode).
- `src/lib/mocks/handlers/umu_database.ts` — **NEW**. `refresh_umu_database` mock handler.
- `src/lib/mocks/index.ts` — registers the new handler.
- `src/styles/preview.css` — `.crosshook-preview-modal__umu-decision--warn` +
  `.crosshook-preview-modal__umu-decision-warning` rules.
- `src/styles/theme.css` — `.crosshook-pinned-strip__badge` + `--warn` modifier.

### Docs

- `docs/prps/prds/umu-launcher-migration.prd.md` — 6 edits: Decisions Log row, 2 Storage Boundary
  rows, Open Questions resolution, Phase 3b subrow, Phase 4 prerequisite note, GitHub issues table
  (#247, #263, #251-as-duplicate).

## Validation results

| Level                  | Command                                                 | Result                                         |
| ---------------------- | ------------------------------------------------------- | ---------------------------------------------- |
| 1 — rustfmt            | `cargo fmt --all -- --check`                            | ✅ clean                                       |
| 2 — clippy             | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ clean                                       |
| 3 — unit + integration | `cargo test -p crosshook-core`                          | ✅ 909 tests pass (896 lib + 13 integration)   |
| 4 — TypeScript         | `tsc --noEmit` + `biome check`                          | ✅ 230 files clean                             |
| 5 — full lint suite    | `./scripts/lint.sh`                                     | ✅ rustfmt + clippy + biome + tsc + shellcheck |

**Test breakdown (new tests)**:

- `umu_database::coverage::tests::index_contains_fixture_entry` (inline)
- `umu_database::paths::tests::resolve_returns_none_when_no_candidate_exists` (inline)
- `tests/umu_database_coverage.rs`: 4 tests (Found, Missing, Unknown, mtime-invalidation) — 1.00s
- `tests/umu_database_http_cache.rs`: 4 tests (2xx persist, 304 unchanged, network-err, ETag
  roundtrip) — 0.14s
- `launch::preview::tests::preview_reports_csv_coverage_*`: 3 new preview-level tests

## Implementation notes & pragmatic decisions

1. **Skeleton stub for C4.1.** C1.1 seeded a `client.rs` stub (`Result<_, String>` error,
   `Err("not yet implemented")` body) so the module skeleton compiled before C4.1 replaced it.
   Parallel Batch 2 execution would have failed compile mid-batch otherwise. C4.1 later replaced the
   stub with the real `Error` enum and logic; the Tauri command's `.map_err(|e| e.to_string())`
   continued to work because `Error` impls `Display`.

2. **No `thiserror` introduced.**
   `Error { Network(reqwest::Error), Io(std::io::Error), Metadata(String), Base(String) }` uses a
   manual `Display` + `Error::source()` impl, per the plan's GOTCHA. Zero new transitive deps beyond
   `csv`.

3. **Body stays on disk, not in SQLite.** `external_cache_entries.payload_json` stores only ETag +
   Last-Modified + body_sha256 + body_bytes (<1 KB). The ~900 KB CSV body lives at
   `~/.local/share/crosshook/umu-database.csv`, written atomically via `tempfile::NamedTempFile`
   in the target directory + `persist()` (unique temp name per write, then rename over the
   target path). This respects `MAX_CACHE_PAYLOAD_BYTES` in `metadata/cache_store.rs`.

4. **Test URL override needed an env-var fallback.**
   `#[cfg(test)] pub fn set_source_url_for_test(...)` in `client.rs` works for the library's inline
   `#[cfg(test)]` suite but is invisible to integration-test binaries (they link the library in
   non-test mode). C6.2's teammate added a 5-line `CROSSHOOK_TEST_UMU_DATABASE_URL` env-var check in
   `source_url()` — production behavior unchanged because the env var is never set outside test
   binaries.

5. **Clippy `await_holding_lock` in HTTP tests.** The integration tests hold a
   `std::sync::Mutex<()>` across `refresh_umu_database().await` to serialize process-global env
   mutation (`HOME`, `XDG_DATA_HOME`). Explicitly `#![allow]`d at file scope with a comment
   explaining the intent. Using `tokio::sync::Mutex` here offers no benefit and complicates poison
   recovery.

6. **PinnedProfilesStrip badge is dormant.** The amber ⚠ badge prop was added additively, but the
   component has **no consumer in the current codebase** (verified with repo-wide grep). The prop
   defaults to `undefined`, the badge never renders. This is acceptable per the plan's C3.2 GOTCHA
   ("only render when a cached preview exists"). Any future caller that wants the badge passes the
   map — no per-render IPC triggered from the strip.

7. **Latent mock gap closed.** `preview_launch` mock in browser dev mode previously omitted
   `umu_decision` entirely, making the new chip invisible in the browser. C2.2 populated both
   branches (validation-issues → `null`; populated → a heuristic `found`/`missing` based on
   `game_path` containing `mock-missing`).

## Storage boundary classification

| Data                                                        | Storage                    | Why                                                                                                   |
| ----------------------------------------------------------- | -------------------------- | ----------------------------------------------------------------------------------------------------- |
| `UmuDecisionPreview.csv_coverage`                           | Runtime-only               | Computed per preview; never persisted. Field on the Rust struct; flows through IPC to the UI.         |
| `~/.local/share/crosshook/umu-database.csv`                 | Operational/cache metadata | Persisted cache file on disk, refreshed from upstream and rebuilt if deleted.                         |
| `external_cache_entries` row `cache_key="umu-database:csv"` | SQLite metadata            | ETag + Last-Modified + body_sha256 + fetched_at + expires_at govern conditional GET revalidation.     |
| No new TOML settings                                        | N/A                        | Coverage check is always-on. No user toggle. No `install_nag_dismissed_at`, no refresh-interval knob. |

## Persistence & usability

- **Migration / backward compatibility**: Existing cached CSVs remain usable. If the cache file is
  missing or stale, the next refresh rebuilds it from upstream metadata without requiring a user
  migration step.
- **Offline behavior**: When the network is unavailable, CrossHook can continue using the last
  cached CSV and the associated SQLite metadata; refreshes simply fail until connectivity returns.
- **Degraded / failure fallback**: If refresh or revalidation fails, the previous cache stays in
  place and coverage lookups can continue against the existing on-disk CSV instead of blocking the
  UI.
- **User visibility / editability**: The CSV is a non-user-editable cache artifact. Users may
  inspect or delete it indirectly by clearing the cache, but there are no TOML settings that control
  this behavior.

## Deferred / NOT in Phase 3b (per plan)

- **Per-app-id HTTP auto-resolve** (#251). Closed as duplicate of #247. Full-CSV cache is
  sufficient.
- **Upstream umu-database PR for Witcher 3** (#262). Not CrossHook-side.
- **Flatpak manifest change** (`--filesystem=xdg-data/umu:create`). Deferred to Phase 5. HTTP path
  is the current escape hatch.
- **`csv_coverage` on Steam-applaunch or native previews.** Inherits Phase 3a's `proton_run`-only
  gate.
- **Auto-flip to Proton on missing coverage.** Respects user intent; amber chip is advisory only.

## Follow-ups after merge

1. **Runner-dropdown coverage badge**. User requested post-implementation: render the same ⚠ warning
   next to the Runtime → umu launcher selector in SettingsPanel / ProfileForm the moment a user
   picks `Umu` for a profile whose app id lacks coverage. The backend (`check_coverage`) and TS
   plumbing are ready — estimate ~1–2 hour UI task. Should be filed as a `feat:umu-launcher` /
   `area:ui` issue.
2. **Plan archival + PR close-outs**. At merge-time: move `umu-migration-phase-3-umu-opt-in.plan.md`
   and `umu-migration-phase-3b-umu-opt-in.plan.md` to `docs/prps/plans/completed/`. PR body:
   `Closes #247, #263; closes #251 (duplicate of #247)`.
3. **Witcher 3 upstream PR**. File in #262 follow-up. Not CrossHook scope.
4. **Phase 4 gating**. Per updated PRD: "Phase 4 (Auto default-on) gated on #263 + #247 landed AND
   2-week observation clean."

## Rollback plan

If 3b ships and the chip generates false positives (e.g. upstream CSV regression drops a row):

- Users can flip the per-profile `umu_preference = proton` (Phase 3a escape hatch).
- The advisory chip never blocks launch — offline users, Flatpak users on first launch, and
  degraded-DB users all see `Unknown` and proceed normally.
- No emergency revert needed.

## Acceptance criteria (final)

- [x] All 15 continuation tasks complete.
- [x] `csv = "1"` added; no other new workspace deps.
- [x] `CsvCoverage { Found, Missing, Unknown }` exists, serde round-trip works.
- [x] `resolve_umu_database_path()` respects 5-tier precedence.
- [x] `UmuDecisionPreview.csv_coverage` populated for all `proton_run` previews; `Unknown` when
      app_id empty or no CSV reachable.
- [x] LaunchPanel chip surfaces amber warning + remediation copy when
      `will_use_umu && csv_coverage === 'missing'`.
- [x] PinnedProfilesStrip badge prop wired (dormant until a caller supplies the map — per plan
      GOTCHA, no per-render IPC).
- [x] `refresh_umu_database` IPC + SettingsPanel button work end-to-end.
- [x] Startup `tokio::spawn` background refresh non-blocking.
- [x] ETag / `If-None-Match` roundtrip verified by wiremock test.
- [x] Offline startup logs `warn` + leaves existing cache intact.
- [x] Browser dev mode mock populates `umu_decision.csv_coverage`.
- [x] PRD Decisions Log + Storage Boundary + Open Questions + Implementation Phases + GitHub Issues
      all updated.
- [x] No changes to `packaging/flatpak/*.yml`, `onboarding/readiness.rs`, `export/launcher.rs`,
      Phase 4/5 code paths, or Steam-specific code paths.
