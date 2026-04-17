# Implementation Plan: Flatpak Host Tool Status Dashboard + Shared Capability Gating (Issue #270)

> Source: GitHub issue [#270](https://github.com/yandy-r/crosshook/issues/270)
> Upstream (landed): [#269](https://github.com/yandy-r/crosshook/issues/269) / PR #277 (`09bec4b feat(onboarding): sqlite-backed host readiness catalog`) — SQLite schema v21 with `host_readiness_catalog`, `readiness_nag_dismissals`, `host_readiness_snapshots`.
> Mode: parallel-shaped (hierarchical IDs, `Depends on [...]` annotations, `Batches` summary). Handoff target: `/ycc:prp-implement --parallel`.

## Overview

Layer a single **Tool Status Dashboard** on top of the already-landed host readiness catalog and introduce a **shared capability gating model** that every UI surface (settings, profile, launch, optimization catalog) consults instead of re-probing. The dashboard consumes the existing `check_generalized_readiness` Tauri command + cached `host_readiness_snapshots` row; the gating model is a thin derived view over the same data that turns "tool X available?" into "capability Y enabled, rationale Z, install guidance W." No schema break, no per-page probe duplication, no silent degradation.

## Restated Requirements

- Single Flatpak-focused tool status surface (detected tools, versions, paths, readiness) reachable from settings/onboarding.
- Required launch prerequisites (runtime/compatibility) visually separated from optional enhancements (performance/overlay/prefix_tools).
- Disabled capability toggles explain **why** (missing tool) and **how to enable** (copy command, docs link) inline — no silent no-ops.
- Readiness facts sourced once (`check_generalized_readiness` result + snapshot cache); consumed by dashboard, onboarding, settings, profile, launch.
- Stale/offline data explicitly marked.
- Dashboard never implies bundling is supported; copy reinforces host-delegation model.
- Additive on SQLite schema v21; additive on TOML settings.

## Risks & Unknowns

- **API shape drift**: Current `HostToolCheckResult` has `{tool_id, display_name, is_available, is_required, category, docs_url, install_guidance}` — no `version`, no `resolved_path`. The dashboard requires these; must be **additive optional fields** on the Rust struct + TS mirror, populated by a thin `probe_host_tool_details(tool_id)` command. Default `None` keeps existing callers intact (Serde `#[serde(default)]`, TS `?`).
- **Scattered ad-hoc gating**: Gating lives in 6+ places today (`GamescopeConfigPanel.isDisabled`, `MangoHudConfigPanel`, `LaunchOptimizationsPanel`, `SteamLaunchOptionsPanel`, `useLaunchPrefixDependencyGate`, optimization-catalog validation in `utils/mapValidationToNode.ts`). Consolidating prematurely risks regressing Phase 5b umu/onboarding UX. Mitigation: new capability hook is **additive**; existing panels call it opportunistically in Phase D, one file at a time, keeping today's fallbacks until each panel is migrated. No capability is re-implemented in the first pass that the onboarding wizard depends on.
- **Scroll container registration**: Any new dashboard scroll pane must be added to the `SCROLLABLE` selector in `src/crosshook-native/src/hooks/useScrollEnhance.ts`. Easy to forget; encoded as an explicit step.
- **Steam Deck 1280x800**: Dashboard cards must wrap/stack at narrow widths; card grid uses CSS `grid-template-columns: repeat(auto-fill, minmax(…))` with a 280-320px min to fit two columns on Deck. Manual verification step documented.
- **Freshness semantics**: `checked_at` in `host_readiness_snapshots` is the only truth-of-freshness we have. We must decide a staleness window; plan uses 24h default (configurable later, not in this PR).
- **Browser dev mode**: All new IPC calls must have corresponding mock handlers in `src/crosshook-native/src/lib/mocks/handlers/` to keep `verify:no-mocks` CI and browser dev mode intact.

## Phases

- **Phase A — Core capability model (crosshook-core)**: pure Rust module that turns a `ReadinessCheckResult` into a list of `Capability { id, label, required_tools, optional_tools, state: Available|Degraded|Unavailable, rationale, install_hints }`, plus optional `tool_version` / `resolved_path` fields on `HostToolCheckResult`.
- **Phase B — IPC & frontend data hook**: additive Tauri commands (details probe, snapshot fetch) + a single `useHostReadiness()` frontend hook that centralizes fetch/refresh/freshness logic.
- **Phase C — Dashboard UI**: `HostToolDashboard` + `HostToolCard` + `CapabilitySummaryStrip` + filter/search; consumed by a new Settings sub-tab and exposed from onboarding as a "View dashboard" entry point.
- **Phase D — Gating wiring**: consolidate scattered gating by having each optimization panel read from `useCapabilityGate(id)` and render standardized "disabled + rationale + CTA" markup. Phased per-panel for safety.
- **Phase E — Preferences (TOML) & persistence polish**: dismissed-hints map and dashboard presentation defaults stored in settings.toml; stale badge; offline fallback.
- **Phase F — Docs + acceptance verification**: research artifact cross-links, label taxonomy, changelog, AGENTS doc touch-ups.

---

## Batches Summary (parallelizable steps)

Each batch must complete before the next begins. Inside a batch, steps are file-ownership-disjoint and safe to parallelize.

- **Batch 1 (discovery, doc-only, fully parallel)**: 0.1, 0.2, 0.3
- **Batch 2 (core model + mirror types, parallel)**: A.1, A.2, A.3, A.4, B.1 (type file only)
- **Batch 3 (IPC additive commands + lib.rs wiring; serial within src-tauri, A.5 parallel)**: A.5, B.2, B.3
- **Batch 4 (frontend hook + utils + mocks, parallel)**: B.4, B.5, B.6, C.1 (CSS vars)
- **Batch 5 (UI primitives, parallel)**: C.2, C.3, C.4, C.5
- **Batch 6 (Dashboard composition + scroll registration + settings entry)**: C.6, C.7, C.8
- **Batch 7 (gating wiring — fully parallel, one panel per step)**: D.1, D.2, D.3, D.4, D.5
- **Batch 8 (preferences + freshness, parallel)**: E.1, E.2, E.3
- **Batch 9 (docs + acceptance, parallel)**: F.1, F.2, F.3

---

## Hierarchical Steps

### Phase 0 — Discovery / doc confirmation

**0.1 Confirm readiness API surface is complete for the dashboard's needs.** _Depends on []._

- Files: read-only sweep of `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`, `…/onboarding/mod.rs`, `…/metadata/readiness_snapshot_store.rs`.
- Action: document in the plan scratchpad whether the current `HostToolCheckResult` covers `version` and `resolved_path`. It does **not** — Phase A.1 must add them.
- Output: inline note in `docs/internal/host-tool-dashboard-notes.md` (new, single-shot, <30 LOC).
- Risk: Low.

**0.2 Confirm scattered gating inventory.** _Depends on []._

- Files: read-only: `GamescopeConfigPanel.tsx`, `MangoHudConfigPanel.tsx`, `LaunchOptimizationsPanel.tsx`, `SteamLaunchOptionsPanel.tsx`, `useLaunchPrefixDependencyGate.ts`, `utils/mapValidationToNode.ts`, `utils/launch.ts`.
- Action: add a table to `docs/internal/host-tool-dashboard-notes.md` listing each `isDisabled` / disabled-rationale source so Phase D has a checklist.
- Risk: Low.

**0.3 Confirm browser-dev mock surface for readiness.** _Depends on []._

- Files: read-only: `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts`.
- Action: note which commands are mocked; new commands in Phase B must be mocked too.
- Risk: Low.

### Phase A — Core capability model (`crosshook-core`)

**A.1 Extend `HostToolCheckResult` with `tool_version` and `resolved_path`.** _Depends on [0.1]._

- File: `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs` (~+8 LOC).
- Action: add `#[serde(default)] pub tool_version: Option<String>` and `#[serde(default)] pub resolved_path: Option<String>`. Additive only — no existing call sites break; snapshot JSON is forward-compatible (extra fields ignored on decode, missing fields default).
- Risk: Low.

**A.2 Add `probe_host_tool_details()` helper.** _Depends on [A.1]._

- File: new `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs` (~120 LOC) + `mod.rs` re-export.
- Action: for a given tool id, resolve binary path via existing `platform::host_command_exists()` / `which` semantics, run `"{binary} --version"` (or tool-specific arg table) through `host_std_command()`, parse first line. Time-bound each probe (<2s). Return `HostToolDetails { tool_id, tool_version, resolved_path }`. No panics — on failure, fields are `None`.
- Unit tests: table-driven parser tests for gamescope/mangohud/gamemoderun/umu-run/winetricks/protontricks version strings.
- Risk: Medium (version string parsing variance — cover with regex fallback to "raw first line").

**A.3 Introduce `capability` module.** _Depends on [A.1]._

- File: new `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` (~200 LOC).
- Action: declare:
  - `pub enum CapabilityState { Available, Degraded, Unavailable }`.
  - `pub struct Capability { id: String, label: String, category: String, state: CapabilityState, rationale: Option<String>, required_tool_ids: Vec<String>, optional_tool_ids: Vec<String>, missing_required: Vec<HostToolCheckResult>, missing_optional: Vec<HostToolCheckResult>, install_hints: Vec<HostToolInstallCommand> }`.
  - `pub fn derive_capabilities(result: &ReadinessCheckResult) -> Vec<Capability>` — pure function; maps curated capability-to-tools table (gamescope → `[gamescope]`, mangohud → `[mangohud]`, gamemode → `[gamemode]`, prefix_tools → `[winetricks, protontricks]`, non_steam_launch → required `[umu_run]`).
- Capability table lives in `assets/default_capability_map.toml` (Phase A.4), loaded by catalog pattern for override parity.
- Tests: `derive_capabilities` with a fixture result (all-available / one-missing-required / one-missing-optional) asserting `state` and rationale strings.
- Risk: Medium.

**A.4 Ship default capability map TOML.** _Depends on [A.3]._

- File: new `src/crosshook-native/crates/crosshook-core/assets/default_capability_map.toml` (~60 LOC).
- Action: declare capability entries with `id`, `label`, `required_tools`, `optional_tools`, `category`. Mirror `ReadinessCatalog` loader: `pub fn global_capability_map()` with `OnceLock`, optional user override at `host_capability_map.toml`.
- Tests: catalog parse / merge / override tests (mirror `catalog.rs` tests, ~30 LOC).
- Risk: Low.

**A.5 Export from `onboarding/mod.rs`.** _Depends on [A.3, A.4]._

- File: `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs` (+3 LOC).
- Action: `pub mod capability; pub mod details; pub use capability::{Capability, CapabilityState, derive_capabilities, global_capability_map}; pub use details::{HostToolDetails, probe_host_tool_details};`.
- Risk: Low.

### Phase B — IPC + data hook

**B.1 Mirror TS types.** _Depends on [A.1, A.3]._

- File: `src/crosshook-native/src/types/onboarding.ts` (+40 LOC).
- Action: add optional `tool_version?: string | null`, `resolved_path?: string | null` to `HostToolCheckResult`. Add `CapabilityState` union, `Capability` interface, `HostToolDetails` interface.
- Risk: Low.

**B.2 Add `probe_host_tool_details` Tauri command.** _Depends on [A.2]._

- File: `src/crosshook-native/src-tauri/src/commands/onboarding.rs` (+30 LOC).
- Action: `#[tauri::command] pub fn probe_host_tool_details(tool_id: String) -> Result<HostToolDetails, String>`. Sanitize path via existing `sanitize_display_path`.
- Register handler in `src/crosshook-native/src-tauri/src/lib.rs` `invoke_handler!` macro (serial with B.3 on the same file — keep B.2 adds first in the batch).
- Risk: Low.

**B.3 Add `get_capabilities` + `get_cached_host_readiness_snapshot` Tauri commands.** _Depends on [A.3, A.4, B.2]._

- File: `src/crosshook-native/src-tauri/src/commands/onboarding.rs` (+60 LOC) and registration in `lib.rs`.
- Actions:
  - `get_capabilities(store, metadata)` → loads latest readiness snapshot (cached-path) or falls back to live probe when no snapshot exists, feeds into `derive_capabilities`, returns `Vec<Capability>`.
  - `get_cached_host_readiness_snapshot(metadata)` → returns `Option<HostReadinessSnapshotRow>` shaped for the frontend (`{checked_at, detected_distro_family, tool_checks, all_passed, critical_failures, warnings}`); `None` when MetadataStore is disabled or no snapshot.
- Never re-probe inside `get_capabilities` unless snapshot missing — the dashboard uses a refresh action to run `check_generalized_readiness` explicitly.
- Risk: Low-Medium.

**B.4 Add mock handlers for new commands.** _Depends on [B.2, B.3]._

- File: `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts` (+50 LOC).
- Action: stub realistic responses for `probe_host_tool_details`, `get_capabilities`, `get_cached_host_readiness_snapshot`. Required for browser dev mode and `verify:no-mocks` sentinel.
- Risk: Low.

**B.5 Add `useHostReadiness` hook.** _Depends on [B.1, B.2, B.3]._

- File: new `src/crosshook-native/src/hooks/useHostReadiness.ts` (~160 LOC).
- Action: owns `{ snapshot, capabilities, isStale, lastCheckedAt, isRefreshing, error, refresh(), probeTool(toolId) }`. On mount: fetch cached snapshot + capabilities; if none, trigger initial `check_generalized_readiness`. `refresh()` calls `check_generalized_readiness` (which persists) and re-fetches capabilities. Derives `isStale` (checked_at > 24h).
- Consumed by dashboard + any gating hook that needs capability state.
- Risk: Medium.

**B.6 Add `useCapabilityGate` hook.** _Depends on [B.5]._

- File: new `src/crosshook-native/src/hooks/useCapabilityGate.ts` (~80 LOC).
- Action: thin selector: `useCapabilityGate(capabilityId) → { state, rationale, missingRequired, installHint, onDismiss, onCopyCommand, docsUrl }`. Reads from `useHostReadiness`. Memoized by capability id.
- Risk: Low.

### Phase C — Dashboard UI

**C.1 Add dashboard CSS variables and class scaffolding.** _Depends on []._

- File: `src/crosshook-native/src/styles/variables.css` (+8 LOC) and a new `src/crosshook-native/src/styles/host-tool-dashboard.css` (~120 LOC) imported from `src/crosshook-native/src/styles/index.css`.
- Action: define `--crosshook-capability-available`, `--…-degraded`, `--…-unavailable` tokens; BEM `crosshook-host-tool-dashboard__…` classes; grid with `minmax(280px, 1fr)`; `overflow-y: auto; overscroll-behavior: contain;` on the scroll pane.
- Risk: Low.

**C.2 `HostToolCard` component.** _Depends on [B.1]._

- File: new `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx` (~160 LOC).
- Action: renders one tool row: `display_name`, category badge, required/optional chip, availability icon, `tool_version`, `resolved_path` (monospace, truncated with title-tooltip via existing `InfoTooltip`), status message. Mirrors existing `HostToolsReadinessSection` visual semantics. Expandable install-help area reuses copy-command / open-docs / dismiss-reminder buttons.
- Risk: Low.

**C.3 `CapabilitySummaryStrip` component.** _Depends on [B.1]._

- File: new `src/crosshook-native/src/components/host-readiness/CapabilitySummaryStrip.tsx` (~90 LOC).
- Action: small top-of-dashboard strip: "X of Y required host tools ready | Z optional capabilities available". Also renders a stale-data warning badge when `isStale`.
- Risk: Low.

**C.4 `HostToolFilterBar` component.** _Depends on [B.1]._

- File: new `src/crosshook-native/src/components/host-readiness/HostToolFilterBar.tsx` (~120 LOC).
- Action: category filter (All / Runtime / Performance / Overlay / Compatibility / Prefix tools), availability filter (All / Available / Missing / Required-missing), free-text search. Filter state is runtime-only (component state, not persisted). Emits change events; never mutates backend.
- Risk: Low.

**C.5 `HostDelegationBanner` component.** _Depends on []._

- File: new `src/crosshook-native/src/components/host-readiness/HostDelegationBanner.tsx` (~60 LOC).
- Action: inline banner stating: "CrossHook runs games on the host. These tools must be installed on the host, not inside the Flatpak sandbox." Ensures acceptance criterion 3 (never imply sandbox bundling). Dismissable? No — it's structural copy, not a nag.
- Risk: Low.

**C.6 `HostToolDashboard` composition.** _Depends on [B.5, C.2, C.3, C.4, C.5]._

- File: new `src/crosshook-native/src/components/host-readiness/HostToolDashboard.tsx` (~220 LOC).
- Action: uses `useHostReadiness()`; renders `HostDelegationBanner` + `CapabilitySummaryStrip` + `HostToolFilterBar` + grouped `HostToolCard` list (required tools group first, optional second). Refresh button invokes `refresh()`. Empty state, loading spinner, error banner, stale-data callout. Group expansion state is runtime-only. Dashboard root scroll wrapper uses class `crosshook-host-tool-dashboard__scroll`.
- Risk: Medium.

**C.7 Register dashboard scroll pane with `useScrollEnhance`.** _Depends on [C.6]._

- File: `src/crosshook-native/src/hooks/useScrollEnhance.ts` (+1 token in `SCROLLABLE`).
- Action: append `.crosshook-host-tool-dashboard__scroll` to the `SCROLLABLE` selector string. Mandatory per AGENTS/CLAUDE rules.
- Risk: Low.

**C.8 Expose dashboard in Settings and add onboarding cross-link.** _Depends on [C.6]._

- Files: `src/crosshook-native/src/components/SettingsPanel.tsx` (+~40 LOC: new "Host Tools" section) **and** `src/crosshook-native/src/components/OnboardingWizard.tsx` (+~15 LOC: "Open full dashboard" link in the review/runtime stage).
- Action: Settings section renders `<HostToolDashboard />` inside an existing collapsible section. Onboarding wizard gets a non-modal button that opens the Settings dashboard route (via existing navigation). Both entries route to the same single source — no duplicate mount of `useHostReadiness`.
- Risk: Low-Medium (SettingsPanel is 1300+ LOC; add new section at a clean insertion point near existing onboarding toggles to minimize diff surface).

### Phase D — Gating wiring (parallelizable per-panel)

**D.1 `GamescopeConfigPanel` consumes `useCapabilityGate('gamescope')`.** _Depends on [B.6]._

- File: `src/crosshook-native/src/components/GamescopeConfigPanel.tsx` (~+25 LOC).
- Action: when capability state is `Unavailable`, render a standardized "disabled + rationale + CTA" banner above the existing content and force `isDisabled=true` regardless of `config.enabled`. When `Degraded`, show inline rationale with a warning tone. Existing `isDisabled` behavior preserved.
- Risk: Low.

**D.2 `MangoHudConfigPanel` consumes `useCapabilityGate('mangohud')`.** _Depends on [B.6]._

- File: `src/crosshook-native/src/components/MangoHudConfigPanel.tsx` (~+20 LOC). Same pattern as D.1.
- Risk: Low.

**D.3 `LaunchOptimizationsPanel` consumes capability gate for each optimization id that declares a host-tool dep.** _Depends on [B.6]._

- File: `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx` (~+40 LOC).
- Action: for each optimization row, if its `dependency_missing` validation (existing `LaunchOptimizationDependencyMissing`) matches a capability, render the shared rationale + copy-command CTA. Replace current silent degradation with this shared markup. Today's validation path stays authoritative; the capability hook only augments copy + CTA.
- Risk: Medium (heavy file, many rows). Keep diff localized to the row template.

**D.4 `SteamLaunchOptionsPanel` advisory (not gating) consumes capability probe.** _Depends on [B.6]._

- File: `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx` (~+15 LOC).
- Action: advisory banner when gamescope/mangohud/gamemoderun tokens are present in the built command line but the capability is `Unavailable`. Does not block save.
- Risk: Low.

**D.5 `useLaunchPrefixDependencyGate` defers to capability model for `prefix_tools`.** _Depends on [B.6]._

- File: `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts` (~+20 LOC).
- Action: augment existing gate with capability state so the hook's returned rationale matches the dashboard's wording exactly (eliminates copy drift).
- Risk: Low.

### Phase E — Preferences + persistence polish

**E.1 Settings.toml preference: `host_tool_dashboard_dismissed_hints`.** _Depends on []._

- File: `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (~+20 LOC).
- Action: add `#[serde(default)] pub host_tool_dashboard_dismissed_hints: Vec<String>` — list of capability ids whose inline "install hint" banner the user has dismissed in settings-surfaced gating rows. Tool-level dismissals remain in SQLite via existing `readiness_nag_dismissals` (per-tool TTL). Capability-level dismissals are user-editable preferences, never expire, classified as TOML.
- TS mirror in `src/crosshook-native/src/types/settings.ts`.
- Tests: backward-compat load without key, roundtrip preserves.
- Risk: Low.

**E.2 Dashboard presentation defaults in settings.toml.** _Depends on [E.1]._

- File: same `settings/mod.rs` (~+10 LOC).
- Action: `#[serde(default)] pub host_tool_dashboard_default_category_filter: Option<String>` (e.g. `"all"`, `"runtime"`); defaults to `None` meaning "all". Runtime-only filter state can initialize from this.
- Risk: Low.

**E.3 Offline/stale fallback path in `useHostReadiness`.** _Depends on [B.5, C.3]._

- File: `src/crosshook-native/src/hooks/useHostReadiness.ts` (~+30 LOC).
- Action: when MetadataStore is disabled (`get_cached_host_readiness_snapshot` returns `null`) and the live probe fails, surface `error` with actionable copy ("readiness cache unavailable — run checks manually"). Dashboard renders the banner + refresh CTA; never fakes availability.
- Risk: Low.

### Phase F — Docs + acceptance

**F.1 User-facing doc note.** _Depends on [C.6, E.1]._

- File: new `docs/internal/host-tool-dashboard.md` (~120 LOC) — follows `docs(internal)` commit prefix rule.
- Action: describe the dashboard, capability model, TOML keys, SQLite tables touched, and the "Detect-Guide-Degrade" mapping.
- Risk: Low.

**F.2 Cross-link research and update issue body hooks.** _Depends on [F.1]._

- File: `docs/research/flatpak-bundling/14-recommendations.md` (append cross-link subsection at the bottom, ~10 LOC).
- Action: note that Phase 1 task 1.3 and Phase 2 task 2.1/2.5 are now implemented by this plan's Phases A-E.
- Risk: Low.

**F.3 Verification checklist + changelog entry.** _Depends on [C.6, D.5, E.1]._

- Files: update PR description template section + prep a `feat(ui):` changelog entry naming the dashboard. No manual `CHANGELOG.md` edit (driven by git-cliff).
- Action: ensure commit titles use user-visible phrasing for dashboard/capability work and `docs(internal):` for plan/research work.
- Risk: Low.

---

## Persistence & Usability

- **Migration**: None. Additive on SQLite schema v21 (`host_readiness_catalog`, `readiness_nag_dismissals`, `host_readiness_snapshots` already present). Additive on settings.toml (two new keys, both `#[serde(default)]`, backward compatible).
- **Offline / stale marking**: `CapabilitySummaryStrip` renders a "Snapshot from {time} — refresh to re-probe" badge whenever `checked_at > 24h` or the snapshot source is the cache fallback. Individual cards show a smaller "cached" chip when rendered from snapshot. The dashboard never pretends stale data is fresh.
- **Degraded fallback (metadata DB unavailable)**: `get_cached_host_readiness_snapshot` returns `None`; `useHostReadiness` falls back to a live probe via `check_generalized_readiness`. On probe failure, the dashboard renders an error banner + manual refresh. Existing per-tool dismissal commands already handle the "metadata disabled" case via `require_readiness_metadata`.
- **User visibility / editability**: users can (a) dismiss per-tool install hints (already possible, TTL'd in SQLite), (b) dismiss capability-level hints permanently (new, TOML), (c) set a default category filter (new, TOML), (d) run manual refresh from the dashboard. Filter state and panel expansion are explicitly runtime-only.

## Storage Classification Table

| Datum                                                         | Classification                                                                                 | Rationale                                                                         |
| ------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| `HostToolCheckResult.tool_version` (new)                      | Runtime-only (probe-derived) / cached into existing SQLite `host_readiness_snapshots` row JSON | Added to existing snapshot payload, not a new column.                             |
| `HostToolCheckResult.resolved_path` (new)                     | Runtime-only / cached into same JSON                                                           | Same.                                                                             |
| `Capability` list (derived)                                   | Runtime-only                                                                                   | Pure derivation from catalog + readiness snapshot. Never persisted independently. |
| `host_tool_dashboard_dismissed_hints: Vec<String>`            | TOML settings                                                                                  | User preference, never expires, fully editable.                                   |
| `host_tool_dashboard_default_category_filter: Option<String>` | TOML settings                                                                                  | User preference.                                                                  |
| Per-tool nag dismissals                                       | SQLite (`readiness_nag_dismissals`) **existing**                                               | Already landed with #269; unchanged.                                              |
| Snapshot payload                                              | SQLite (`host_readiness_snapshots`) **existing**                                               | Already landed with #269; unchanged schema, additive JSON fields.                 |
| Filter/search/expansion UI state                              | Runtime-only                                                                                   | Ephemeral component state.                                                        |

## Testing Strategy

Unit tests (run with the mandatory verification command after Rust changes):

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

Unit tests to add:

- `capability::derive_capabilities` — all-available, one-missing-required, one-missing-optional, empty `tool_checks` fixture (A.3).
- `capability::global_capability_map` parse / override / merge (A.4).
- `details::parse_version_line` table-driven for gamescope/mangohud/gamemoderun/umu-run/winetricks/protontricks (A.2).
- Settings backward-compat + roundtrip for `host_tool_dashboard_dismissed_hints` (E.1) and default filter (E.2).

IPC smoke tests:

- Existing pattern: `command_signatures_match_expected_ipc_contract` in `src-tauri/src/commands/onboarding.rs` extended to cover `probe_host_tool_details`, `get_capabilities`, `get_cached_host_readiness_snapshot` (B.2, B.3).

Manual UI verification (no configured frontend test framework; use dev scripts):

- `./scripts/dev-native.sh` — on a machine with gamescope missing, confirm the dashboard shows gamescope as missing-optional with install command and `GamescopeConfigPanel` renders a disabled banner.
- `./scripts/dev-native.sh --browser` — confirm new mock handlers return dashboard payload; filter + search + refresh function; stale badge appears when mocked `checked_at` is 48h old.
- Steam Deck rendering check at 1280x800: dashboard grid collapses to 2 columns; cards wrap; scroll behaves (`useScrollEnhance` registered).
- Flatpak sandbox check (if available): confirm copy shows "install on host" language; no "bundled" wording.

## Success Criteria

Mapped to the four acceptance criteria in the issue:

- [ ] **(Acc. 1)** A single `HostToolDashboard` surface is reachable from Settings and from the onboarding wizard, listing detected host tools, versions, paths, required-vs-optional grouping, and readiness.
- [ ] **(Acc. 2)** Disabled capability toggles (at minimum gamescope, MangoHud, prefix_tools, launch optimizations rows) render a standardized rationale + install-command + docs CTA via `useCapabilityGate`.
- [ ] **(Acc. 3)** Dashboard copy and `HostDelegationBanner` explicitly state host-delegation; no string anywhere implies sandbox bundling is supported.
- [ ] **(Acc. 4)** Dashboard, onboarding, and gated panels all consume `useHostReadiness` / `useCapabilityGate` — no per-page re-probe, no duplicated install-command strings (verifiable by grepping for raw `pacman -S umu-launcher` etc. outside `crosshook-core` catalog TOML).
- [ ] `cargo test -p crosshook-core` passes with new tests.
- [ ] `useScrollEnhance` `SCROLLABLE` selector contains `.crosshook-host-tool-dashboard__scroll`.
- [ ] `verify:no-mocks` CI sentinel still passes (mocks only in `lib/mocks/`).
- [ ] Settings.toml additions are backward compatible (round-trip tests).

## Out of Scope

- The readiness probe layer itself (owned by #269 / PR #277, already landed).
- Any form of sandbox-side bundling.
- macOS / non-Linux platforms.
- Auto-installing host tools from inside CrossHook (only detection + guidance).
- Proton download manager (unrelated, Phase 3 of research).
- Performance benchmarking of `flatpak-spawn --host` (Phase 1.5 of research, separate).
- Full migration of every ad-hoc disabled-state string to the shared model (we migrate the high-value panels in D.1-D.5; long-tail follow-ups can be tracked in future issues labeled `type:refactor`, `area:ui`).

---

## Relevant Files

- `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/onboarding/catalog.rs`
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/readiness_snapshot_store.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/readiness_dismissal_store.rs`
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`
- `src/crosshook-native/src-tauri/src/commands/onboarding.rs`
- `src/crosshook-native/src-tauri/src/lib.rs`
- `src/crosshook-native/src/types/onboarding.ts`
- `src/crosshook-native/src/types/settings.ts`
- `src/crosshook-native/src/hooks/useOnboarding.ts`
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`
- `src/crosshook-native/src/components/ReadinessChecklist.tsx`
- `src/crosshook-native/src/components/OnboardingWizard.tsx`
- `src/crosshook-native/src/components/SettingsPanel.tsx`
- `src/crosshook-native/src/components/GamescopeConfigPanel.tsx`
- `src/crosshook-native/src/components/MangoHudConfigPanel.tsx`
- `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`
- `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`
- `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts`
- `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts`
- `src/crosshook-native/src/styles/variables.css`
- `docs/research/flatpak-bundling/11-patterns.md`
- `docs/research/flatpak-bundling/13-opportunities.md`
- `docs/research/flatpak-bundling/14-recommendations.md`
