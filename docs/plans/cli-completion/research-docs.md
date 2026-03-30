# Documentation Research: CLI Completion

## Overview

All required documentation for implementing CLI completion already exists in `docs/plans/cli-completion/`. The feature spec is comprehensive, covering architecture, JSON schemas, security mitigations, phasing strategy, and code examples. No gaps in primary documentation — the prior research team produced a thorough set of research files that fully inform implementation. The quickstart guide at `docs/getting-started/quickstart.md` will need a new CLI section added as part of Phase 5.

---

## Architecture Docs

| Document               | Path                                                    | Content                                                                                                          |
| ---------------------- | ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| Feature Spec (primary) | `docs/plans/cli-completion/feature-spec.md`             | Full architecture diagram, CLI-to-core call map, data models, JSON schemas, security findings, phasing breakdown |
| Technical Spec         | `docs/plans/cli-completion/research-technical.md`       | File-level relevant paths, command specs with I/O schemas, dispatch pattern, gotchas (12 documented)             |
| Practices Research     | `docs/plans/cli-completion/research-practices.md`       | Architectural patterns, reusable code inventory, KISS assessment, modularity recommendations                     |
| Recommendations        | `docs/plans/cli-completion/research-recommendations.md` | Risk matrix, alternative approaches (A/B/C), phased task breakdown with dependencies                             |
| CLAUDE.md (project)    | `CLAUDE.md`                                             | Codebase architecture map, module hierarchy, key patterns, build commands                                        |

**Architecture decision of record**: Direct Core Calls (Thin Wrapper Pattern) — each handler calls `crosshook-core` inline, no shared presentation layer. Reference implementation: `handle_diagnostics_command` at `crates/crosshook-cli/src/main.rs:145–177`.

---

## API Docs

| Document                  | Path                                             | Content                                                                                                                                                          |
| ------------------------- | ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| External/Library Research | `docs/plans/cli-completion/research-external.md` | All core function signatures with file paths, clap v4 dispatch patterns, integration patterns (5), constraints and gotchas (7), code skeletons for every command |
| Business Analysis         | `docs/plans/cli-completion/research-business.md` | Core function signatures, per-command workflows, domain model, Tauri-to-CLI equivalence table                                                                    |
| Security Research         | `docs/plans/cli-completion/research-security.md` | Process spawning security, path handling rules, 2 CRITICAL + 5 WARNING + 6 ADVISORY findings, secure coding guidelines with Rust snippets                        |

### Key Core Function Signatures (from research-external.md and research-technical.md)

- `ProfileStore::list(&self) -> Result<Vec<String>, ProfileStoreError>` — `profile/toml_store.rs:273`
- `ProfileStore::import_legacy(&self, path: &Path) -> Result<GameProfile, ProfileStoreError>` — `profile/toml_store.rs:324`
- `export_community_profile(profiles_dir: &Path, name: &str, output: &Path) -> Result<CommunityExportResult, CommunityExchangeError>` — `profile/exchange.rs:158`
- `discover_steam_root_candidates(path: impl AsRef<Path>, diag: &mut Vec<String>) -> Vec<PathBuf>` — `steam/discovery.rs:11`
- `discover_steam_libraries(roots: &[PathBuf], diag: &mut Vec<String>) -> Vec<SteamLibrary>` — `steam/libraries.rs:8` (NOT re-exported from `steam/mod.rs`)
- `attempt_auto_populate(req: &SteamAutoPopulateRequest) -> SteamAutoPopulateResult` — `steam/auto_populate.rs:12`
- `build_proton_game_command(req: &LaunchRequest, log: &Path) -> io::Result<Command>` — `launch/script_runner.rs:61`
- `build_native_game_command(req: &LaunchRequest, log: &Path) -> io::Result<Command>` — `launch/script_runner.rs:121`

### Inline Doc Comments in Source (load-bearing for implementers)

- `crosshook-core/src/launch/mod.rs` — module-level doc comment on launch orchestration primitives; read before Phase 4
- `crosshook-core/src/steam/discovery.rs` — doc comment on `discover_steam_root_candidates()` explains discovery priority order: configured path wins, then native Linux fallbacks (`~/.steam/root`, `~/.local/share/Steam`), then Flatpak Steam Deck path
- `crosshook-core/src/launch/script_runner.rs:23–35` — constants document staging directories and supported dependency extensions for trainer copy-to-prefix mode; relevant to understanding how `build_proton_game_command` stages trainers
- `crosshook-core/src/profile/toml_store.rs` — `///` doc comments on `duplicate()` and `generate_unique_copy_name()` explain collision-free naming (not directly needed for CLI but shows store conventions)

---

## Development Guides

| Document         | Path                                               | Content                                                                                                                                                        |
| ---------------- | -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UX Research      | `docs/plans/cli-completion/research-ux.md`         | Output format standards, error message format (`error: / hint:`), exit codes 0–6, stderr/stdout discipline, competitive analysis (gh, docker, kubectl, Lutris) |
| Quickstart Guide | `docs/getting-started/quickstart.md`               | Existing user guide (needs CLI section added in Phase 5)                                                                                                       |
| Build Scripts    | `scripts/build-native.sh`, `scripts/dev-native.sh` | AppImage build flow, dev server startup                                                                                                                        |

### Build Commands (from CLAUDE.md)

```bash
# Test crosshook-core (run after every change)
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core

# Dev mode (Tauri hot-reload)
./scripts/dev-native.sh

# Production AppImage
./scripts/build-native.sh
```

---

## README Files

| Document    | Path        | Content                                                                          |
| ----------- | ----------- | -------------------------------------------------------------------------------- |
| Root README | `README.md` | User-facing overview, launch modes, feature list, build instructions, docs links |

No directory-level or module-level READMEs exist in the Rust crates. The CLAUDE.md project file serves as the architectural README for contributors.

---

## Feature Specs and Prior Research

All 8 research files exist and are complete. Reading order by priority:

| File                          | Lines | Purpose                                                                                 |
| ----------------------------- | ----- | --------------------------------------------------------------------------------------- |
| `feature-spec.md`             | 577   | Master spec — start here. Architecture, schemas, security, phasing, decisions resolved  |
| `research-technical.md`       | 586   | Deep technical: file paths, command specs, gotchas #1–12, decision rationale            |
| `research-external.md`        | 515   | Code skeletons for every command, library references, integration patterns              |
| `research-business.md`        | 351   | Business rules per command, edge cases, domain model, Tauri equivalence table           |
| `research-practices.md`       | 108   | KISS assessment, reusable code inventory, testability guidance                          |
| `research-recommendations.md` | 315   | Risk matrix, phasing, alternative implementation approaches A/B/C                       |
| `research-security.md`        | 403   | 2 CRITICAL security findings (C-1, C-2), WARNING and ADVISORY findings with mitigations |
| `research-ux.md`              | 571   | Exit codes, output format, error UX, competitive analysis                               |

---

## Must-Read Documents

### Required Before Starting Implementation

1. **`docs/plans/cli-completion/feature-spec.md`** — The single most important document. Contains the resolved architecture, all JSON output schemas, the `launch_request_from_profile()` code template, security hard stops (C-1, C-2), and the 5-phase task breakdown. Read completely before writing any code.

2. **`docs/plans/cli-completion/research-technical.md`** — Required companion to the feature spec. Contains 12 documented gotchas (notably: `discover_steam_libraries` is NOT re-exported from `steam/mod.rs`; `export_community_profile` takes `profiles_dir: &Path` not a `&ProfileStore`). These will cause compilation failures if missed.

3. **`docs/plans/cli-completion/research-security.md`** — Security findings C-1 and C-2 are explicitly called out in the feature spec as hard stops that must ship with mitigations. C-1 requires helper script path runtime validation. C-2 requires import path containment checks. Do not skip.

4. **`CLAUDE.md`** (project file) — Project conventions, commit message rules, build commands, workspace layout, and Rust/TypeScript conventions. Must be followed.

### Required Before Phase 4 (Launch Completion)

5. **`docs/plans/cli-completion/research-external.md`** — Contains the complete `launch_request_from_profile()` code skeleton and all three method dispatch skeletons. Phase 4 is the most complex task; this file has the exact code shape to follow.

6. **`docs/plans/cli-completion/research-practices.md`** — Contains the full reusable-code inventory with file paths and line numbers. Prevents re-implementing functions that already exist (notably: `resolve_steam_client_install_path()` duplication issue at `main.rs:236`).

7. **`crosshook-core/src/launch/script_runner.rs:23–35`** and **`crosshook-core/src/steam/discovery.rs`** inline doc comments — Read these before implementing Phase 4 launch dispatch and Phase 3 steam discovery respectively.

### Nice-to-Have

8. **`docs/plans/cli-completion/research-ux.md`** — Output format details, error message standards, exit code definitions. Useful during Phase 5 (polish) but less critical than the above.

9. **`docs/plans/cli-completion/research-business.md`** — Domain model and per-command business rules. Mostly synthesized into the feature spec; useful for clarifying edge case behavior.

10. **`docs/plans/cli-completion/research-recommendations.md`** — Alternative approach analysis and risk assessment. Useful if the recommended approach hits a blocker.

---

## Documentation Gaps

1. **No CLI section in quickstart guide** — `docs/getting-started/quickstart.md` covers GUI workflows only. A CLI usage section is an explicit acceptance criterion (Phase 5). Suggested addition: `## Using the CLI`, with examples for each of the 7 commands.

2. **No inline doc comments on CLI command variants** — `crates/crosshook-cli/src/args.rs` has test coverage but no `///` doc comments on `Command` or subcommand variants. The feature spec calls this out as a Phase 1 quick win (adds quality `--help` output with zero logic changes).

3. **JSON output schemas are not yet versioned or formally documented** — The feature spec documents the schemas but notes they should be marked "unstable for v1". A follow-up docs task would lock schemas and publish them as stable API.

4. **No external docs links for `crosshook-core` public API** — The crate has no generated rustdoc. To navigate the public API surface, implementers must read source files directly per the file paths in `research-technical.md` and `research-practices.md`.

5. **`docs/plans/custom-env-vars/research-architecture.md` exists but has no equivalent for cli-completion** — The architecture researcher on the prior feature produced a separate architecture research file; the cli-completion research set does not have one. Architecture information is distributed across `feature-spec.md` and `research-technical.md` instead, which is sufficient.
