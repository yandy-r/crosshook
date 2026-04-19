# PR Review #354 — test(e2e): Spike Tauri E2E via tauri-driver for WebKitGTK coverage gap

**Reviewed**: 2026-04-19
**Mode**: PR
**Author**: app/anthropic-code-agent
**Branch**: `claude/spike-tauri-e2e-webkitgtk` → `main`
**Decision**: REQUEST CHANGES

## Summary

Docs-only spike research (4 markdown files, 1686 additions) for evaluating `tauri-driver` as a WebKitGTK E2E tool. The analysis structure is sound and the adopt/defer/drop framework is usable, but two correctness bugs would actively mislead a reader who tries to execute the prototype: the wrong GitHub issue is referenced throughout (`#350` points at an already-merged vitest PR; the actual spike issue is `#347`), and the debug binary path assumes a nested `src-tauri/target/` that does not exist in CrossHook's Cargo workspace (actual location is `src/crosshook-native/target/debug/crosshook-native`). A third inconsistency (`binary` vs `application` as the `tauri:options` capability key) will cause the sample test to fail as written.

## Findings

### CRITICAL

_None._

### HIGH

- **[F001]** `docs/research/tauri-webkitgtk-e2e-spike/README.md:3` — Wrong issue reference throughout all 4 files. `#350` is an already-merged PR titled "test(frontend): add vitest and rtl seed suite" (state: closed, merged as commit `e3f1f97`). The actual spike issue is **#347** ("[Spike]: Tauri E2E via tauri-driver for WebKitGTK coverage gap", open). Every reader who follows these links lands on the wrong ticket. Affected locations: `README.md` lines 3, 187, 203; `01-research-findings.md` lines 3, 438 (Success Criteria section header), 527, 570; `02-prototype-setup.md` line 548; `03-decision-framework.md` lines 281, 312, 351.
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: Global replace `#350` → `#347` across the 4 docs. Also update the markdown link targets (e.g. `[#350](https://github.com/yandy-r/crosshook/issues/350)` → `[#347](https://github.com/yandy-r/crosshook/issues/347)`).

- **[F002]** `docs/research/tauri-webkitgtk-e2e-spike/02-prototype-setup.md:136` — Wrong Tauri debug binary path. Docs say `./src-tauri/target/debug/crosshook-native`, but CrossHook uses a **Cargo workspace** at `src/crosshook-native/Cargo.toml` (members: `crates/crosshook-core`, `crates/crosshook-cli`, `src-tauri`). With a workspace, `cargo build` places artifacts in the workspace root's `target/`, not in each member's directory. The actual binary lives at `src/crosshook-native/target/debug/crosshook-native`. Following the doc verbatim triggers the `beforeSession` hook's "Tauri binary not found" error on the first run. Affected locations: `01-research-findings.md` line 162; `02-prototype-setup.md` lines 136, 343, 471.
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: Change all `src-tauri/target/debug/crosshook-native` references to `target/debug/crosshook-native` (relative to `src/crosshook-native/`). In `wdio.conf.js`, compute the path as `path.resolve(__dirname, '../../target/debug/crosshook-native')` (two levels up from `tests/e2e/`, not three).

### MEDIUM

- **[F003]** `docs/research/tauri-webkitgtk-e2e-spike/01-research-findings.md:158` — Inconsistent `tauri:options` capability key. `01-research-findings.md` uses `binary: './src-tauri/.../crosshook-native'` while `02-prototype-setup.md:154` uses `application: TAURI_BINARY`. These are two different keys and only one is accepted by `tauri-driver`. Per tauri-driver v2 capability schema the correct key is **`application`**; the `01-research-findings.md` example will silently fail (driver won't launch the app).
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: In `01-research-findings.md` lines 156-166, change `binary:` to `application:` so both docs use the same — and correct — key.

- **[F004]** `docs/research/tauri-webkitgtk-e2e-spike/01-research-findings.md:64` — Self-contradicting WebKitGTK dev-package name. Section 2 line 64 shows `sudo apt-get install libwebkit2gtk-4.0-dev webkit2gtk-driver`, but Section 6 line 195 correctly documents CrossHook's CI as `libwebkit2gtk-4.1-dev` (confirmed in `.github/workflows/lint.yml:42`). The `-4.0-` variant is legacy (Ubuntu 20.04-era) and installing it on a current Ubuntu/Debian pulls an incompatible header set for CrossHook's Tauri v2 build.
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: Change line 64 to `sudo apt-get install libwebkit2gtk-4.1-dev webkit2gtk-driver` to match the rest of the doc and the actual CI configuration.

- **[F005]** `docs/research/tauri-webkitgtk-e2e-spike/02-prototype-setup.md:537` — Broken relative links to nonexistent follow-up docs. `[03-ci-integration.md](./03-ci-integration.md)` (line 537) and `[04-decision-log.md](./04-decision-log.md)` (line 539) are both referenced as next steps, but neither file exists in this PR. The existing Phase 3 doc is named `03-decision-framework.md` and there is no Phase 4 doc.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Replace `03-ci-integration.md` with `03-decision-framework.md`. Drop the `04-decision-log.md` reference or replace it with "record findings directly in `03-decision-framework.md` § Scoring Template".

- **[F006]** `docs/research/tauri-webkitgtk-e2e-spike/README.md:1` — Missing persistence/usability classification required by `CLAUDE.md`. The repo's "Research and planning quality bar" mandates that every feature plan/research artifact classify new/changed data (TOML settings, SQLite metadata, or runtime-only) and include a persistence/usability section covering migration, offline behavior, degraded fallback, and user visibility. This spike introduces no persisted data — but the docs don't say so explicitly, which makes them non-conforming to repo policy and forces reviewers to re-derive the classification.
  - **Status**: Open
  - **Category**: Completeness
  - **Suggested fix**: Add a short "Persistence & Usability" section (e.g. in `README.md` or at the top of `01-research-findings.md`) stating: "No persisted data — tauri-driver artifacts are runtime-only (test binaries, driver logs, CI caches). No TOML/SQLite changes; no migration, no offline/degraded handling required; test configuration is developer-facing only."

### LOW

- **[F007]** `docs/research/tauri-webkitgtk-e2e-spike/01-research-findings.md:23` — Unverified "current state" claims (Tauri `v2.10.3` stable "March 2026", `tauri-driver v2.0.5` released "February 2026", "pre-alpha" label). These numbers aren't cross-referenced against upstream and may not survive review; at minimum they should be dated and cite sources so they can be re-verified before the prototype actually runs.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Add a "Sources checked: YYYY-MM-DD" footer under each version claim, or link each version directly to the upstream release (crates.io, GitHub release page).

- **[F008]** `docs/research/tauri-webkitgtk-e2e-spike/README.md:5` — Duplicated/drifting status metadata. `README.md` says "Research phase complete; prototype pending"; individual docs are marked "✅ Complete"; `01-research-findings.md:5` says "Research Phase" (no "Complete"). Minor, but re-reading the docs in 3 months will make the discrepancy confusing.
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Pick a single source of truth (the `README.md` § Decision Status table) and shorten the per-doc headers to just date + link back to README.

## Validation Results

| Check      | Result                                 |
| ---------- | -------------------------------------- |
| Type check | Skipped (docs-only; no source changes) |
| Lint       | Skipped (docs-only; no source changes) |
| Tests      | Skipped (docs-only; no source changes) |
| Build      | Skipped (docs-only; no source changes) |

Rationale: all 4 changed files are `.md` under `docs/research/`. Repo lint config targets `src/`, `scripts/`, Rust, and TypeScript — no docs-only lint rule is wired up. No code path, test, or build output is affected by this PR.

## Files Reviewed

- `docs/research/tauri-webkitgtk-e2e-spike/README.md` (Added)
- `docs/research/tauri-webkitgtk-e2e-spike/01-research-findings.md` (Added)
- `docs/research/tauri-webkitgtk-e2e-spike/02-prototype-setup.md` (Added)
- `docs/research/tauri-webkitgtk-e2e-spike/03-decision-framework.md` (Added)
