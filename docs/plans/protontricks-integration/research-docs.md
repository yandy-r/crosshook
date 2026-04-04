# Protontricks Integration — Documentation Research

## Overview

CrossHook has comprehensive documentation across AGENTS.md (authoritative stack/directory reference), feature guides under `docs/features/`, and a complete set of prior research files already present in `docs/plans/protontricks-integration/`. The feature spec (`feature-spec.md`) is the resolved master document integrating outputs from all seven existing research files. The architecture docs in `AGENTS.md` define the persistence classification rules, SQLite schema version (v13/v14), and IPC patterns that every implementation must follow. No documentation gaps block implementation — the only open decision requiring resolution before coding starts is the static vs. dynamic package allowlist strategy.

---

## Architecture Docs

| Document                           | Path                                                                                                                | What It Covers                                                                                                                                                                                                                                                                           |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Agent Rules + Stack Overview       | `AGENTS.md`                                                       | Normative platform rules (CrossHook is a native Linux Tauri v2 app, not Wine), full directory map, SQLite schema inventory (v13, 18 tables), persistence classification table, IPC conventions                                                                                           |
| Feature Spec (master resolved doc) | `docs/plans/protontricks-integration/feature-spec.md`             | Complete resolved specification: architecture diagram, data models (SQLite v15, TOML changes, Rust types), API design (4 IPC commands), business rules, success criteria, phasing strategy, risk register                                                                                |
| Technical Research                 | `docs/plans/protontricks-integration/research-technical.md`       | Component diagram, new module structure (`prefix_deps/`), SQLite migration detail, TOML model changes, IPC command signatures, Tauri event channels (`prefix-dep-log`, `prefix-dep-complete`), integration points into `lib.rs`, `migrations.rs`, `profile/models.rs`, `settings/mod.rs` |
| Architecture Research              | `docs/plans/protontricks-integration/research-recommendations.md` | Resolved technology choices, phasing strategy (6 ordered phases), quick wins, health system integration approach, launch gate integration, onboarding integration                                                                                                                        |
| Steam/Proton Feature Guide         | `docs/features/steam-proton-trainer-launch.doc.md`                | Full launch workflow (steam_applaunch, proton_run, native methods), profile required fields, ConsoleView usage, health dashboard integration — context for how prefix paths and launch flows work                                                                                        |

---

## API Docs

### New IPC Commands (to be created in `src-tauri/src/commands/prefix_deps.rs`)

| Command                      | Direction | Signature Summary                                                          | Notes                                                                               |
| ---------------------------- | --------- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `detect_protontricks_binary` | sync      | `() → DetectBinaryResult { found, binary_path, binary_name, source }`      | Detection order: settings → PATH winetricks → PATH protontricks → Flatpak           |
| `check_prefix_dependencies`  | async     | `{ profile_name, prefix_path, steam_app_id } → CheckPrefixDepsResult`      | Runs `winetricks list-installed`; 30s timeout; upserts SQLite                       |
| `install_prefix_dependency`  | async     | `{ profile_name, prefix_path, packages, steam_app_id } → PrefixDepsResult` | Acquires global lock; builds Command; streams `prefix-dep-log` events; 300s timeout |
| `get_dependency_status`      | sync      | `{ profile_name } → Vec<PackageDependencyState>`                           | Pure SQLite read; no process spawn                                                  |

### Tauri Events (Backend → Frontend)

| Event                 | Payload                                                           | Consumer                                                                                               |
| --------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `prefix-dep-log`      | `string` (ANSI-stripped line)                                     | `ConsoleDrawer` / `ConsoleView` — add one `listen` call alongside existing `launch-log` / `update-log` |
| `prefix-dep-complete` | `{ package: string, succeeded: bool, exit_code: number \| null }` | `DependencyRow` component — transitions chip from `installing` to `installed`/`install_failed`         |

### Existing IPC Commands Relevant to Integration

| Command                         | File                                   | How It Informs the Feature                                                 |
| ------------------------------- | -------------------------------------- | -------------------------------------------------------------------------- |
| `profile_load` / `profile_save` | `src-tauri/src/commands/profile.rs`    | Pattern for IPC wrappers; thin async fn → `spawn_blocking` → core function |
| `launch_game`                   | `src-tauri/src/commands/launch.rs`     | Launch gate: pre-launch dependency check integrates here                   |
| `check_system_readiness`        | `src-tauri/src/commands/onboarding.rs` | Pattern for adding winetricks binary check to readiness panel              |

---

## Development Guides

| Document              | Path                                                                                                          | What It Covers                                                                                                                                                                                                                                               |
| --------------------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Build & Dev Workflow  | `docs/internal-docs/local-build-publish.md`                 | Dev server (`./scripts/dev-native.sh`), AppImage build (`./scripts/build-native.sh`), container build, binary-only flag                                                                                                                                      |
| Engineering Practices | `docs/plans/protontricks-integration/research-practices.md` | Reusable code inventory with exact file paths, modularity design, KISS assessment table, interface design, testability patterns, build vs. depend decisions                                                                                                  |
| Security Research     | `docs/plans/protontricks-integration/research-security.md`  | 22 findings (7 CRITICAL, 14 WARNING, 1 ADVISORY); command injection mitigations (S-01 to S-06); CWE-209 boundary (S-11, S-27); per-prefix concurrent access lock (S-10); `--` flag separator requirement (S-06); host environment preservation guidance for subprocesses (S-07) |

### Key Dev Commands

```bash
./scripts/dev-native.sh                                                         # Hot-reload Tauri dev
./scripts/build-native.sh                                                       # Full AppImage build
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core   # Run core unit tests
```

---

## README Files

There is no root `README.md` in the repository root that is user-facing. Relevant user-facing starting points are:

| Document                | Path                                                                                   | Purpose                                                                                                               |
| ----------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Quickstart              | `docs/getting-started/quickstart.md` | User-facing first-run guide; community profiles, health dashboard, console view sections are relevant to this feature |
| Agent Rules (CLAUDE.md) | `CLAUDE.md`                          | Project-level rules for AI agents; commit prefix rules, IPC naming, Serde requirement, MCP preference                 |

---

## Existing Research Files (Prior Feature Research in This Plan Directory)

All seven research files are complete. Implementers should treat `feature-spec.md` as the primary source of truth — it integrates all research into resolved decisions.

| File                          | Status   | Key Resolved Decisions                                                                                                                           |
| ----------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `research-technical.md`       | Complete | Module structure, SQLite schema, TOML changes, IPC signatures, Tauri event channels, concurrent lock via `PrefixDepsInstallState`                |
| `research-practices.md`       | Complete | Reusable files inventory, `ProtontricksRunner` trait design, `ScopedCommandSearchPath` test pattern, `open_in_memory()` for SQLite tests         |
| `research-external.md`        | Complete | Protontricks and winetricks CLI references, environment variables, exit codes, prefix path convention, `winetricks list-installed` for detection |
| `research-ux.md`              | Complete | All user flows, `DependencyStatusBadge` component (do NOT extend `HealthBadge`), `DepStatus` type, ConsoleDrawer integration, accessibility      |
| `research-business.md`        | Complete | 14 business rules (BR-1 to BR-14), verified allowlisted packages, winetricks-direct as primary tool, 24-hour TTL for check cache                 |
| `research-security.md`        | Complete | 22 findings; CRITICAL items S-01/S-02/S-03/S-06/S-19/S-22/S-27 must be resolved before ship                                                      |
| `research-recommendations.md` | Complete | Winetricks-direct preferred (not protontricks); 6-phase build order; blocking decision: static vs. dynamic allowlist                             |
| `feature-spec.md`             | Complete | Master resolved spec — start here                                                                                                                |

---

## Must-Read Documents (Prioritized Reading List)

### Required (read before writing any code)

1. **`docs/plans/protontricks-integration/feature-spec.md`** — The authoritative resolved spec. Has the complete architecture diagram, all data models, all 4 IPC command specs, business rules, success criteria, and risk register.

2. **`AGENTS.md`** — Platform rules (CrossHook is NOT a Wine app; it orchestrates Wine), directory map, SQLite schema version and table inventory, persistence classification rules, IPC naming conventions. Violation of these rules causes PR rejection.

3. **`docs/plans/protontricks-integration/research-security.md`** — 7 CRITICAL findings that block ship. Read before implementing the runner, validation, or IPC layer. Key: `Command::arg()` (never shell), `--` separator, `env_clear()`, no raw subprocess output to UI.

4. **`docs/plans/protontricks-integration/research-practices.md`** — Exact reusable file inventory with line numbers. Prevents re-implementing what already exists (e.g., `resolve_umu_run_path` pattern at `runtime_helpers.rs:302`, `ScopedCommandSearchPath` test pattern).

### Strongly Recommended (read before implementing each phase)

5. **`docs/plans/protontricks-integration/research-technical.md`** — Deep architecture: component diagram, SQLite migration DDL, TOML field additions, Rust type definitions, IPC integration point list.

6. **`docs/plans/protontricks-integration/research-recommendations.md`** — Phasing strategy and blocking decisions. Read section "Blocking architectural decision" before starting.

7. **`docs/plans/protontricks-integration/research-ux.md`** — UI component design: `DependencyStatusBadge`, `DepStatus` type, ConsoleDrawer event integration, concurrent lock enforcement in UI.

8. **`docs/features/steam-proton-trainer-launch.doc.md`** — How CrossHook currently handles prefixes, launch flows, and the ConsoleView. Required context for integrating the dependency panel and pre-launch gate.

### Reference (consult as needed)

9. **`docs/plans/protontricks-integration/research-business.md`** — All 14 business rules and edge cases. Cross-check when any behavioral question arises (e.g., soft-block vs. hard-block, TTL values, Flatpak handling).

10. **`docs/plans/protontricks-integration/research-external.md`** — CLI reference for winetricks and protontricks; exact env var names, exit codes, prefix path conventions. Reference when building the `Command`.

11. **`docs/getting-started/quickstart.md`** — End-user mental model for community profiles, health dashboard. Ensures the new Prefix Dependencies panel fits the existing user workflow.

---

## Documentation Gaps

| Gap                                                        | Severity | Notes                                                                                                                                                                                                                                                                                   |
| ---------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Blocking decision unresolved: static vs. dynamic allowlist | HIGH     | `research-recommendations.md` calls this out as a blocker before implementation. Static allowlist is simpler and avoids requiring winetricks binary at startup; dynamic (from `winetricks list`) is more complete. This policy decision must be made before `validation.rs` is written. |
| `research-integration.md` IPC command name discrepancy     | LOW      | `research-integration.md` lists `detect_winetricks_binary` and `get_prefix_dependency_states`; `feature-spec.md` uses `detect_protontricks_binary` and `get_dependency_status`. Reconcile before writing the Tauri command handler signatures.                                          |
| `winetricks list-installed` output format undocumented     | MEDIUM   | `research-external.md` documents the command but not the exact output format (newline-delimited? space-delimited? version suffixes?). The feature spec assumes newline-delimited verb names. Verify against actual winetricks output or source before implementing the parser.          |
| `ConsoleDrawer` event listener API undocumented            | LOW      | `research-ux.md` states "adding `prefix-dep-log` to ConsoleDrawer is a one-liner" but the exact `listen` call location in the component is not cited. Read `src/crosshook-native/src/components/ConsoleDrawer.tsx` before implementation.                                               |
