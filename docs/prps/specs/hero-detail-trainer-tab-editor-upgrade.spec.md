# Spec: Hero Detail Trainer Tab Editor Upgrade

## Problem Statement

The Hero Detail Trainer tab is still a read-only summary even though the rest of Hero Detail has become the per-game editing surface. Users who need trainer DLL hooks, injection settings, or injection lifecycle feedback must leave the per-game flow or cannot configure the behavior at all. The cost is a misleading UI: trainer injection state appears visible, but the critical editable controls and runtime feedback are missing.

## Requirements

### Functional

| #   | Requirement                                                                                                                             | Priority | Notes                                                                                                                                                                                                                                                         |
| --- | --------------------------------------------------------------------------------------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F1  | Replace the read-only Hero Detail Trainer tab with a three-section editor for loaded hooks, injection config, and recent injection log. | Must     | Issue #479 target layout; current tab is the `case 'trainer'` read-only block in `src/crosshook-native/src/components/library/HeroDetailPanels.tsx:287`.                                                                                                      |
| F2  | Let users add, remove, rename, retarget, enable, and disable per-profile loaded DLL hooks from the Trainer tab.                         | Must     | Reuse the `HookListPanel` row affordances where possible (`src/crosshook-native/src/components/library/HookListPanel.tsx:38`), but avoid conflating script hooks and DLL hooks without an explicit kind/filter decision.                                      |
| F3  | Persist loaded hook edits through the selected profile's TOML data with autosave parity and visible save/error status.                  | Must     | Existing launch hook autosave persists through ProfileContext (`src/crosshook-native/src/components/library/launch/useHeroLaunchHooksAutosave.ts:24`); trainer edits should follow the same selected-profile guard pattern.                                   |
| F4  | Add per-profile injection config fields for Method, Stage, Timeout, and Fallback.                                                       | Must     | Current `InjectionSection` only has `dll_paths` and `inject_on_launch` (`src/crosshook-native/crates/crosshook-core/src/profile/models/game_meta.rs:30`); the TypeScript profile mirrors only those fields (`src/crosshook-native/src/types/profile.ts:115`). |
| F5  | Ensure injection config is either consumed by the trainer injection runtime or clearly disabled/annotated until supported.              | Must     | The issue comment says these fields are not consumed by the launch pipeline today; the final UI must not imply runtime behavior that does not exist.                                                                                                          |
| F6  | Show a recent injection log that live-tails injection lifecycle events for the current profile/session.                                 | Must     | Existing backend streaming emits generic `launch-log` events (`src/crosshook-native/src-tauri/src/commands/launch/streaming.rs:93`) and the frontend must subscribe through the shared adapter (`src/crosshook-native/src/lib/events.ts:7`).                  |
| F7  | Preserve Hero Detail profile-scope safety so edits never write to a different selected profile than the displayed game/profile.         | Must     | `HeroDetailLaunchTab` already gates edits on selected-profile ownership before writing hooks (`src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx:24`).                                                                                      |
| F8  | Hide or disable trainer injection editing for launch methods that do not support trainer injection.                                     | Should   | `TrainerSection` already hides for native launch (`src/crosshook-native/src/components/profile-sections/TrainerSection.tsx:31`).                                                                                                                              |
| F9  | Provide browser-dev mock coverage for any new injection log command/event so UI tests can exercise the Trainer tab without Tauri.       | Should   | Browser dev mode requires the shared adapter path for command/event mocks.                                                                                                                                                                                    |

### Non-Functional

| #   | Requirement                         | Target                                                                                                                                                                                                                                | Rationale                                                               |
| --- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| NF1 | Backward-compatible profile loading | Existing profiles load with default injection config and empty DLL hook declarations.                                                                                                                                                 | Profile TOML changes must be additive and safe for old profiles.        |
| NF2 | Event freshness                     | Injection lifecycle rows appear within 1 second p95 while a launch/trainer session is active.                                                                                                                                         | Users need live confirmation that hooks loaded and processes attached.  |
| NF3 | Log bounds                          | Trainer tab keeps a bounded recent tail, such as the latest 200 rows or equivalent byte cap.                                                                                                                                          | Prevent long launch sessions from growing UI memory without bound.      |
| NF4 | Storage boundary                    | User-editable loaded hooks and injection config live in profile TOML; live injection log rows are runtime-only unless reused by existing launch diagnostics; no new SQLite table unless planning chooses persisted injection history. | Matches CrossHook's settings-vs-metadata-vs-runtime persistence policy. |
| NF5 | Host boundary                       | Any host-tool work introduced by injection runtime changes routes through the platform host gateway when it touches denylisted host tools.                                                                                            | Maintains ADR-0001 Flatpak/AppImage parity.                             |

## Technical Approach

**Strategy**: Promote the Trainer tab into a focused editor that reuses existing Hero Detail profile context, profile autosave, and hook-row UI patterns, while adding typed backend/profile support for DLL-specific hook declarations, injection config, and injection lifecycle telemetry.

**Architecture Decisions**:

- Profile TOML is the persistence layer for injection config and loaded hook declarations because they are user-editable per-profile settings.
- DLL loaded hooks need an explicit model boundary, either `LaunchHook.kind = "dll"` with filtered views or a separate injection hook type, because existing launch hooks are script-hook oriented.
- Injection log delivery should use the shared frontend event adapter, either as a structured injection-specific event or as typed/filterable launch-log payloads, because raw Tauri listeners bypass browser dev mocks.
- The Tauri command layer should remain thin; injection config validation, launch consumption, and lifecycle event production belong in `crosshook-core`.

**Key Components**:

- `HeroDetailTrainerTab`: Trainer-tab editor that replaces the read-only `HeroDetailPanels` trainer case.
- `LoadedHookListPanel`: DLL-kind hook editor that reuses or wraps `HookListPanel` row behavior.
- `InjectionConfigPanel`: Method, Stage, Timeout, and Fallback controls bound to profile TOML.
- `InjectionLogTail`: Bounded live log view scoped to the active profile/session.
- `InjectionSection` / profile models: Additive TOML schema and generated TypeScript types for new injection config data.
- Launch/injection runtime telemetry: Emits structured lifecycle events such as DLL loaded, game process attached, hook initialized, timeout, fallback, and failure.

## Integration Points

| System/Service                            | Direction        | Protocol                                     | Notes                                                                                            |
| ----------------------------------------- | ---------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Hero Detail UI                            | inbound          | React props/context                          | Trainer tab reads and writes the same selected profile context used by Profiles and Launch tabs. |
| Profile persistence                       | both             | TOML via profile load/save IPC               | Loaded hooks and injection config round-trip with additive defaults.                             |
| `crosshook-core` launch/injection runtime | both             | Rust APIs                                    | Runtime consumes supported injection config fields and emits lifecycle telemetry.                |
| Tauri IPC/events                          | outbound         | `#[tauri::command]` / Tauri events           | New or extended telemetry must keep command names snake_case and payloads Serde-compatible.      |
| Browser dev mocks                         | inbound/outbound | `callCommand()` / `subscribeEvent()` adapter | Any new command/event needs mock handlers so CI/browser tests do not require native Tauri.       |

## Risks & Unknowns

| Risk                                                                   | Likelihood | Impact | Mitigation                                                                                                                  |
| ---------------------------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------- |
| Script-hook and DLL-hook data models get merged too loosely.           | M          | H      | Decide the kind/filter boundary before planning implementation; test round-trip serialization for both script and DLL rows. |
| UI exposes injection config before runtime honors it.                  | M          | H      | Gate controls, annotate unsupported behavior, or include runtime consumption in the same implementation phase.              |
| Generic launch-log noise makes the injection log hard to trust.        | M          | M      | Prefer structured, source-tagged injection lifecycle events scoped by profile/session.                                      |
| Hero Detail displays one profile while ProfileContext writes another.  | M          | H      | Reuse the mismatch guard pattern from `HeroDetailLaunchTab` and add tests for displayed-vs-selected profile mismatch.       |
| Additive profile schema still breaks sparse or legacy profile imports. | L          | H      | Use serde defaults, generated TS defaults, and Rust/TOML round-trip tests.                                                  |

## Open Questions

- [ ] Should loaded DLL hooks extend `LaunchHook` with an explicit `kind`, or should injection hooks use a separate profile model?
- [ ] What are the exact allowed Method, Stage, Timeout, and Fallback values, and which are supported by the current runtime?
- [ ] Should unsupported injection config fields be disabled until runtime consumption lands, or should this feature include runtime consumption as a Must-have?
- [ ] Should injection log delivery be a new `injection-log` event or structured/filterable `launch-log` payloads?
- [ ] Does "recent injection log" mean runtime-only live tail, or should launch history persist injection lifecycle excerpts for later inspection?
- [ ] What is the expected fallback behavior when a DLL hook fails: warn-and-continue, skip trainer, stop launch, or user-selectable?

---

_Source: GitHub issue #479 and issue comment from 2026-06-05; local codebase grounding against current checkout._
_Generated: 2026-06-05T18:46:03Z_
_Status: DRAFT - ready for prp-plan_
