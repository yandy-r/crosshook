# PR Review #278 — feat(onboarding): dedicated host tool dashboard page (#270)

**Reviewed**: 2026-04-17
**Mode**: PR (parallel)
**Author**: yandy-r
**Branch**: `feat/host-tool-dashboard` → `main`
**Head SHA**: `eb4208035827ecbe7cf7546545438567e6d98af2`
**Decision**: REQUEST CHANGES

## Summary

The PR successfully promotes host-tool readiness to a first-class `HostToolsPage` route and lands a solid capability-derivation model, a working umu-run cached-snapshot fallback fix, and extensive mock coverage for browser dev mode. Validation is green across lint, 983 cargo tests, the IPC-contract test, and the frontend build. Requesting changes primarily to enforce the project's mandatory scroll-container registration contract (`SCROLLABLE` + `overscroll-behavior: contain`), address an `N+1` IPC burst from multi-instance `useHostReadiness`, and bring `HostToolCard` in line with the BEM-class-only pattern. No CRITICAL issues; no security vulnerabilities.

## Findings

### CRITICAL

_(none)_

### HIGH

- **[F001]** `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx:159` — Render function (lines 159–461, ~303 LOC) is built entirely with 31 inline `style={{…}}` object literals on a 462-line component. This violates the project pattern that visual structure lives in BEM-like `crosshook-*` classes in dedicated stylesheets; it also creates unstable object identity on every render (hurting memoization downstream), and the file exceeds the 800-line maintainability guideline for any component this leaf-level. [quality]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Extract layout/spacing rules into `host-tool-dashboard.css` (or a sibling `host-tool-card.css`) using BEM classes — `crosshook-host-tool-card`, `crosshook-host-tool-card__header`, `crosshook-host-tool-card__body`, `crosshook-host-tool-card__actions` — and replace every `style={{…}}` with the corresponding `className`. This also eliminates the per-render object churn.

- **[F002]** `src/crosshook-native/src/hooks/useCapabilityGate.ts:23` — `useCapabilityGate` instantiates a fresh `useHostReadiness()` per call. `LaunchOptimizationsPanel` mounts five gates (`gamescope`, `mangohud`, `gamemode`, `prefix_tools`, `non_steam_launch`), so a single panel opens produces five independent hook instances, each firing `get_cached_host_readiness_snapshot` + `get_capabilities` on mount (≈10 IPC calls). The module-level `hasBootstrappedLiveRefresh` flag in `useHostReadiness.ts` races among concurrent instances — on first render all five instances read `false` before any has set it to `true`, so `check_generalized_readiness` (full subprocess probe suite) can be issued five times in parallel. [security]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Introduce a `HostReadinessProvider` at the app root that owns the single `useHostReadiness` state, exposed via a React context. Refactor `useHostReadiness` into a `useHostReadinessContext()` consumer; `useCapabilityGate` reads from that context instead of instantiating new state. This reduces redundant IPC to a single set regardless of consumer count and eliminates the bootstrap flag race entirely. Per AGENTS.md `invoke()` is already required to be hook-wrapped; centralizing via context is the idiomatic next step.

- **[F003]** `src/crosshook-native/src/hooks/useScrollEnhance.ts:8` — The new `HostToolsPage` relies on the shared layout contract's `crosshook-route-stack__body--scroll` scroll pane (see `layout.css:158`), but that selector is absent from the `SCROLLABLE` constant. AGENTS.md and `.cursorrules` both make this registration mandatory: without it, `closest(SCROLLABLE)` from inside the dashboard walks up to `crosshook-page-scroll-body` instead of the real scroller, and the WebKitGTK velocity-momentum enhancement targets the wrong element. Acceptance criterion (`plan §Success Criteria`) explicitly requires the dashboard's scroll pane to be registered. [correctness + quality, concurred]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Append `.crosshook-route-stack__body--scroll` to the `SCROLLABLE` string literal on line 8. If the plan-era `crosshook-host-tool-dashboard__scroll` class was dropped during the settings-panel → dedicated-page refactor, also remove any dead references from the dashboard CSS.

- **[F004]** `src/crosshook-native/src/styles/layout.css:158` — The `.crosshook-route-stack__body--scroll` rule declares `overflow-y: auto` (line 162) but is missing `overscroll-behavior: contain`. AGENTS.md: _"Inner scroll containers should also use `overscroll-behavior: contain` to prevent scroll-chaining to outer containers."_ Pre-existing rule, but **exposed by this PR** — the new `HostToolsPage` is the first sidebar route with substantial scrollable content using this shared class. Scroll events will bleed to the outer `crosshook-page-scroll-body`. [quality]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Add `overscroll-behavior: contain;` to the `.crosshook-route-stack__body--scroll` block in `src/crosshook-native/src/styles/layout.css`. (This file is not in the PR diff, but the fix should ship with this PR since the PR is what brings the class into production use as a page-level scroller.)

### MEDIUM

- **[F005]** `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs:1` — File is 1007 lines; TOML parsing, map loading/merging, global singleton, and `derive_capabilities` logic are all co-located. Exceeds the 800-line maintainability ceiling. [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Split TOML parsing and map-loading into `capability_loader.rs` (or `capability/loader.rs`) and keep `capability.rs` focused on struct definitions, `CapabilityState`, and `derive_capabilities`. The existing module-as-directory pattern (`onboarding/mod.rs` re-exports) fits this naturally.

- **[F006]** `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs:390` — When `get_capabilities` serves a cached SQLite snapshot, `synthesize_umu_run_check` hits the `None` arm and calls `resolve_umu_run_path()` on **every** invocation. Under Flatpak, that function reads `/run/host/env/PATH` and traverses pipx venv candidate directories — repeated avoidable I/O on every panel mount and every stale-check refresh. The fix from the PR description (live probe fallback) is correct for the cache-empty case but unnecessary when the cached snapshot already contains a populated `resolved_path`. [security]
  - **Status**: Open
  - **Category**: Performance
  - **Suggested fix**: Before falling back to `resolve_umu_run_path()`, check whether `result.tool_checks` already contains an `umu_run` entry with a non-empty `resolved_path`; if so, treat it as a positive detection and skip the live probe. This keeps the cache authoritative and limits the live probe to true cold-start / cache-miss paths.

- **[F007]** `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs:405` — `synthesize_umu_run_check` sets `HostToolInstallCommand.alternatives = guidance.description.clone()`. The `description` is a human-readable rationale (e.g. `"Install umu-launcher on your Arch-based host to enable …"`), not an alternative install method. `HostToolCard` renders the `alternatives` field as a labeled "alternative install methods" block — users see the purpose text restated in the wrong slot while the actual catalog `alternatives` string is silently dropped because `UmuInstallGuidance` does not expose it. [correctness]
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: Change `alternatives: guidance.description.clone()` to `alternatives: String::new()` as a minimal fix, or (preferred) extend `UmuInstallGuidance` with an optional `alternatives: Option<String>` field sourced from the catalog and propagate it here. Add a unit test that asserts the `alternatives` field is empty when the catalog provides no alternatives.

- **[F008]** `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs:202` — `read_child_pipe` calls `pipe.read_to_end(&mut buffer)` with no size cap. The 1500 ms `DETAIL_PROBE_TIMEOUT` kills the subprocess but does not bound memory: a misbehaving or shadowed host binary (e.g. a wrapper on PATH intercepting `gamescope`) could write many megabytes to stdout/stderr before exiting, all of which is buffered. Version parsing only uses the first matching line, so unbounded buffering buys nothing. [security]
  - **Status**: Open
  - **Category**: Security
  - **Suggested fix**: Cap the read — `pipe.take(VERSION_OUTPUT_CAP).read_to_end(&mut buffer)` where `VERSION_OUTPUT_CAP` is a small constant (4–16 KiB is ample). Add `use std::io::Read;` if not already in scope. Add a regression test using a scripted stdout that writes > cap bytes and asserts the parser still returns the expected version from the first line.

- **[F009]** `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs:1` — File is 1082 lines, exceeding the 800-line ceiling. `detect_host_distro_family*`, `build_umu_install_advice`, `evaluate_checks_inner`, and dismissal helpers are all co-located. [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Extract `detect_host_distro_family*` into `distro.rs` and the install-advice fallback table into `install_advice.rs`; keep `readiness.rs` focused on check evaluation + dismissal orchestration. Extractions are mechanical — each helper already has a clear surface.

- **[F010]** `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs:171` — `build_umu_install_advice` hard-codes per-distro install command strings (`sudo pacman -S umu-launcher`, `sudo dnf install …`, `nix profile install …`, etc.) as a Rust-side fallback. Plan acceptance criterion 4 requires all install commands to live in the catalog TOML so there's a single authoritative source; silently divergent Rust constants is exactly the drift that criterion was designed to prevent. [quality]
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Remove the per-distro fallback match. When the catalog is absent, return a generic "catalog not loaded; see docs" guidance (or `Err`) rather than a silently-stale literal. Update tests that assert specific command strings to read from the catalog. Alternatively, if this fallback is genuinely load-bearing (e.g. for tests that don't load the catalog), hoist the strings into a single `const` table in the catalog module and have both the catalog default and the fallback read from it.

- **[F011]** `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx:153` — Four `console.error(…)` calls left in production error paths (lines 153, 321, 369, 438) for "error opening docs", "error probing details", "error copying command", "error copying path". These ship to the browser console in production builds. [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Remove all four `console.error` calls. User-initiated action errors (copy, probe) should surface through UI state — an `error` field on the card or a toast via the existing toast mechanism — not the console. If diagnostic logging is genuinely desired, route it through the repo's structured logger (whatever `src-tauri` is using) via an IPC event, not `console.*`.

- **[F012]** `src/crosshook-native/src/hooks/useHostReadiness.ts:125` — In `refresh()`, if `get_cached_host_readiness_snapshot` throws a non-mock error (e.g. transient DB contention), the inner catch re-throws and skips `setSnapshot(nextSnapshot)` on line 138. The freshly fetched live `check_generalized_readiness` result is discarded and the user sees an error banner while the snapshot stays stale, even though `nextSnapshot` is already populated from live data. [correctness]
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: Commit the live data first: move `setSnapshot(nextSnapshot)` to just after `nextSnapshot = snapshotFromReadinessResult(result)` (line 118), then attempt the cached-snapshot fetch as an _optional_ enrichment that upgrades `nextSnapshot` via a subsequent `setSnapshot` if it succeeds. Never let an ancillary IPC failure discard the primary live result.

- **[F013]** `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts:29` — `console.warn(…)` left in the `check_gamescope_session` failure path. The `cancelled` guard already silently swallows the error for unmounted components. [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Remove the `console.warn`. If the failure is recoverable, surface it through the hook's return value (e.g. an `error` field). Otherwise it is truly a silent no-op and the log adds noise.

### LOW

- **[F014]** `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs:143` — After `child.try_wait()` returns `Ok(Some(_status))`, `read_child_pipe` uses blocking `read_to_end` on the captured stdout/stderr. For `--version` probes this is instantaneous in practice, but if a tool's grandchild explicitly inherits the pipe FD (legal via `dup2`), the pipe will not reach EOF until the grandchild also exits. `DETAIL_PROBE_TIMEOUT` guards only the poll loop, not the pipe drain. [correctness]
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: After the exit-poll loop, collect pipe output with a bounded `take(VERSION_OUTPUT_CAP).read_to_end(…)` (see F008), which gives a natural ceiling and pairs well with the timeout. This is a defense-in-depth fix rather than a real bug today.

- **[F015]** `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx:253` — The "Details" button does not carry `disabled={isProbingDetails}`. The label changes to "Loading details…" as a visual cue, but the button remains clickable, so rapid clicks can fire multiple concurrent `probe_host_tool_details` commands for the same `tool_id`. [correctness]
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: Add `disabled={isProbingDetails}` to the "Details" button. The command is idempotent so nothing breaks, but it saves IPC round-trips and matches the visual affordance.

- **[F016]** `src/crosshook-native/src/components/host-readiness/HostToolDashboardHandoff.tsx:12` — Two inline styles (`style={{ marginBottom: 12 }}` and `style={{ minHeight: 'var(--crosshook-touch-target-min)' }}`) where BEM utility classes would fit the project's class-only pattern. [quality]
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Add modifier classes to `host-tool-dashboard.css` (e.g. `crosshook-host-tool-dashboard-handoff__description`, `crosshook-host-tool-dashboard-handoff__action`) and use them instead of inline styles.

- **[F017]** `src/crosshook-native/src/components/host-readiness/HostToolMetricsHero.tsx:17` — `SkeletonHero` uses the index `i` as the React key for skeleton cards (`key={i}`). The skeleton is presentational and never re-ordered, so this is harmless today, but the project convention prefers stable semantic keys. [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Replace `[0, 1, 2, 3].map((i) => …)` with four explicit static elements (no map), or with a named constant array of keys (`'total' | 'required' | 'capabilities' | 'missing'`) matching the real hero tiles.

- **[F018]** `src/crosshook-native/src/hooks/useHostReadiness.ts:7` — `let hasBootstrappedLiveRefresh = false` is module-level and persists for the lifetime of the JS module. Vite HMR or test module-cache reuse can leave it `true` across component remounts, suppressing the bootstrap refresh a freshly-mounted tree expects. (Also feeds into F002's race.) [security]
  - **Status**: Open
  - **Category**: Performance
  - **Suggested fix**: If F002 is addressed by lifting state into a `HostReadinessProvider`, store this flag in a `useRef` or provider-scoped variable so it resets on provider unmount. Otherwise, keep module-level but document the trade-off and export a `__resetBootstrapFlagForTesting` hatch.

## Validation Results

| Check      | Result | Notes                                                                                                                                                                                                  |
| ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Type check | Pass   | `tsc --noEmit` clean; `biome` 247 files clean                                                                                                                                                          |
| Lint       | Pass   | `./scripts/lint.sh` — rustfmt, clippy (no warnings), biome, tsc, shellcheck all clean                                                                                                                  |
| Tests      | Pass   | `cargo test -p crosshook-core` → **983 passed, 0 failed** (970 unit + 13 across integration bins); IPC-contract test passes                                                                            |
| Build      | Pass   | `npm run build` (vite v8.0.5) produced `dist/` in 379 ms. Pre-existing warnings: dynamic-import of `@tauri-apps/api/core`, 894 KB `index-*.js` chunk > 500 KB threshold — both present before this PR. |

## Files Reviewed

Read at PR head `eb4208035827ecbe7cf7546545438567e6d98af2`:

- `.gitignore` (Modified)
- `docs/prps/plans/completed/host-tool-dashboard-270.plan.md` (Modified)
- `docs/prps/reports/host-tool-dashboard-270-report.md` (Modified)
- `docs/research/flatpak-bundling/14-recommendations.md` (Modified)
- `src/crosshook-native/assets/default_capability_map.toml` (Added)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/onboarding.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/settings.rs` (Modified)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/App.tsx` (Modified)
- `src/crosshook-native/src/components/GamescopeConfigPanel.tsx` (Modified)
- `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx` (Modified)
- `src/crosshook-native/src/components/MangoHudConfigPanel.tsx` (Modified)
- `src/crosshook-native/src/components/OnboardingWizard.tsx` (Modified)
- `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx` (Modified)
- `src/crosshook-native/src/components/host-readiness/CapabilitySummaryStrip.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/CapabilityTile.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/CapabilityTilesSection.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/HostDelegationBanner.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/HostToolDashboardHandoff.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/HostToolFilterBar.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/HostToolInventory.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/HostToolMetricsHero.tsx` (Added)
- `src/crosshook-native/src/components/host-readiness/HostToolStatusToolbar.tsx` (Added)
- `src/crosshook-native/src/components/icons/SidebarIcons.tsx` (Modified)
- `src/crosshook-native/src/components/layout/ContentArea.tsx` (Modified)
- `src/crosshook-native/src/components/layout/PageBanner.tsx` (Modified)
- `src/crosshook-native/src/components/layout/Sidebar.tsx` (Modified)
- `src/crosshook-native/src/components/layout/routeMetadata.ts` (Modified)
- `src/crosshook-native/src/components/pages/HostToolsPage.tsx` (Added)
- `src/crosshook-native/src/hooks/useCapabilityGate.ts` (Added)
- `src/crosshook-native/src/hooks/useHostReadiness.ts` (Added)
- `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts` (Modified)
- `src/crosshook-native/src/styles/host-tool-dashboard.css` (Added)
- `src/crosshook-native/src/styles/variables.css` (Modified)
- `src/crosshook-native/src/types/onboarding.ts` (Modified)
- `src/crosshook-native/src/types/settings.ts` (Modified)
- `src/crosshook-native/src/utils/capabilityDocs.ts` (Added)
- `tasks/lessons.md` (Modified)

## Reviewer Breakdown

Three parallel reviewers ran (`--parallel` mode):

| Reviewer             | Focus                                  | Raised                  |
| -------------------- | -------------------------------------- | ----------------------- |
| correctness-reviewer | Correctness, Type Safety, Completeness | 3 MEDIUM, 2 LOW         |
| security-reviewer    | Security, Performance                  | 1 HIGH, 2 MEDIUM, 1 LOW |
| quality-reviewer     | Pattern Compliance, Maintainability    | 3 HIGH, 5 MEDIUM, 2 LOW |

F003 was flagged by both the correctness and quality reviewers (de-duplicated; kept under quality with the correctness concurrence noted).
