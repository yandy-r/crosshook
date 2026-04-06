# Documentation Research: ProtonUp Integration

## Overview

The protonup-integration feature is comprehensively pre-researched. Eight planning documents in `docs/plans/protonup-integration/` cover architecture, data models, API design, security, UX, and implementation practices. A final `feature-spec.md` synthesizes these into an authoritative implementation spec. The codebase documentation in `AGENTS.md`, `CLAUDE.md`, and `CONTRIBUTING.md` provides binding agent guidelines, directory maps, and development conventions that govern all implementation work.

---

## Architecture Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — **REQUIRED**. Authoritative agent/developer reference: stack overview, directory map, full SQLite schema inventory (18 tables, v13), Tauri IPC naming conventions, and persistence design classification. The `crosshook-core` / `src-tauri` split and `snake_case` IPC command requirements are enforced here.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md` — **REQUIRED**. Project policy for AI agents; overlaps with AGENTS.md on architecture rules; adds: commit/PR/label taxonomy, release workflow (`git-cliff`/`CHANGELOG.md`), `docs(internal):` prefix requirement for plan/research commits, and `useScrollEnhance` scroll container registration rule.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/research-technical.md` — **REQUIRED**. Full architecture design, component diagram, data models, API contracts for all 5 Tauri commands, system constraints (streaming download, filesystem permissions, error recovery), and the complete list of files to create/modify.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/feature-spec.md` — **REQUIRED**. Synthesized implementation spec. Authoritative source for business rules (BR-1 through BR-13), edge cases, success criteria, 3-phase implementation plan with task breakdown, and resolved decisions.

---

## API Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/research-external.md` — **REQUIRED**. Complete `libprotonup 0.11.0` public API surface (`downloads`, `sources`, `apps`, `files` modules with signatures), GitHub Releases REST API spec, 5 integration patterns (Tauri Channel progress, `ProgressWriter<W>`, TTL cache via `external_cache_entries`, install flow), and 5 resolved Q&A on runtime, pagination, tool name keys, CVE status, and rate limit headers.

  Key external docs referenced:
  - [libprotonup docs.rs](https://docs.rs/libprotonup/latest/libprotonup/)
  - [GitHub REST API — List releases](https://docs.github.com/en/rest/releases/releases)
  - [GitHub REST API — Rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api)
  - [Tauri v2 — Channels (progress streaming)](https://v2.tauri.app/develop/calling-frontend/)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/research-security.md` — **REQUIRED before writing extraction or IPC code**. 3 CRITICAL findings (CVE chain in `astral-tokio-tar`, `install_dir` path traversal, archive bomb), 5 WARNING findings, required mitigations. Includes verified code patterns for `validate_install_dir`, `is_safe_extraction_path`, URL hostname allowlist, and version string sanitization regex.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml` — Dependency manifest confirming: `libprotonup = "0.11.0"`, `reqwest = "0.13.2"`, `sha2 = "0.11.0"`, `rusqlite = "0.39.0"`, `nix = "0.31.2"`, `tokio` (with `sync` feature). No new dependencies needed.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/Cargo.toml` — `src-tauri` dependencies: `crosshook-core` (path), `tauri = "2"`, `tokio`, `chrono`. Confirms `tokio::sync` arrives via `crosshook-core`'s dependency tree.

---

## Development Guides

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/research-practices.md` — **REQUIRED**. Lists every reusable module with exact file paths and line numbers (cache_store, discover_compat_tools, prefix_deps event streaming, OnceLock HTTP client, settings serde patterns). KISS assessment table, modularity design, testability patterns (in-memory MetadataStore, tempfile fixtures), and the prohibition on abstracting the cache-first fetch pattern (copy, don't abstract).

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/research-recommendations.md` — **REQUIRED**. Final implementation recommendations: technology choices (libprotonup vs. Option B), phasing strategy with task-level breakdown for all 3 phases, quick wins, risk table (7 technical risks with mitigations), integration challenges, and resolved/unresolved open questions. Critical pre-implementation blocker: **GPL-3.0 license compatibility must be resolved before any code using `libprotonup` is written.**

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CONTRIBUTING.md` — Development setup, prerequisites, build commands (`./scripts/build-native.sh`, `./scripts/dev-native.sh`), test command (`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`), project architecture summary.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/tasks/lessons.md` — Repository-specific lessons learned: scroll debugging, launch diagnostics, IPC patterns, GitHub issue CLI limitations. Read to avoid known pitfalls (e.g., `gh issue create --template` does not combine with `--body`).

---

## README Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/README.md` — Product overview: CrossHook is a native Linux AppImage (Tauri v2 + React) that orchestrates trainer/game launches via Steam/Proton. Does NOT run under Wine/Proton itself. User state at `~/.config/crosshook/`. Relevant to understanding the Proton Selector feature context.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md` — End-user workflow doc for Steam and Proton launches. Describes how `runtime.proton_path` is used, auto-populate, Steam library discovery paths, and troubleshooting. Relevant for understanding the broken-Proton-path UX that protonup-integration resolves.

---

## Existing Planning Documents (Summary)

All 8 prior research artifacts in `docs/plans/protonup-integration/`:

| File                          | Summary                                                                                                                                                                 |
| ----------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `feature-spec.md`             | Synthesized spec: executive summary, external deps, business rules, data models, 5 Tauri commands, UX patterns, 3-phase task breakdown, risk table, resolved decisions  |
| `research-technical.md`       | Architecture diagram, complete Rust type definitions, full API contracts, 4 architectural decisions with rationale, 5 open questions (mostly resolved)                  |
| `research-external.md`        | libprotonup API surface, GitHub API spec, 5 integration code patterns, 5 Q&A resolved (pagination=30/page, tool names, CVE status, rate limit headers)                  |
| `research-practices.md`       | Reuse table (8 modules with file paths), KISS assessment, modularity design, testability patterns, build-vs-depend matrix                                               |
| `research-recommendations.md` | Option A vs B vs C, phasing with task breakdown, risk table, integration challenges, GPL-3.0 blocker                                                                    |
| `research-security.md`        | 3 CRITICAL + 5 WARNING + 5 ADVISORY findings, code patterns for path validation, archive bomb limits, symlink checks                                                    |
| `research-ux.md`              | 5 user flows, component vocabulary (CollapsibleSection, status chips, progress bar), competitive analysis (ProtonUp-Qt, Heroic, Lutris, Steam), API-to-UX binding table |
| `research-business.md`        | 8 user stories, 13 business rules (BR-1 through BR-13), domain model with data classification, existing codebase integration map                                        |

---

## Must-Read Documents (Prioritized)

### Tier 1 — Read Before Writing Any Code

1. `docs/plans/protonup-integration/feature-spec.md` — The authoritative spec. Start here.
2. `AGENTS.md` — Binding implementation rules: IPC naming, architecture boundaries, schema inventory.
3. `docs/plans/protonup-integration/research-recommendations.md` — Pre-code blockers (GPL-3.0 license), phasing, risk mitigations.
4. `docs/plans/protonup-integration/research-security.md` — Security mitigations are ship-blocking; must be implemented before extraction code ships.

### Tier 2 — Read During Phase 1 Implementation

5. `docs/plans/protonup-integration/research-technical.md` — Rust types, API contracts, files to create/modify.
6. `docs/plans/protonup-integration/research-external.md` — libprotonup API signatures, integration code patterns.
7. `docs/plans/protonup-integration/research-practices.md` — Reuse points with exact line numbers; testability patterns.

### Tier 3 — Read During Phase 2/3 (UI) Implementation

8. `docs/plans/protonup-integration/research-ux.md` — Component vocabulary, user flows, API-to-UX binding, accessibility requirements.
9. `docs/plans/protonup-integration/research-business.md` — Business rules, edge cases, domain model.
10. `CLAUDE.md` — `useScrollEnhance` scroll registration requirement; commit/PR conventions.

---

## Documentation Gaps

The existing planning documents are thorough. The following items are noted as unresolved or require verification before implementation:

1. **GPL-3.0 license resolution** — `libprotonup` is GPL-3.0; CrossHook is MIT. This is the highest-priority pre-code blocker. No code using `libprotonup` should be written until this is resolved (Option A: legal acceptance; Option B: MIT-clean direct GitHub API + `reqwest`/`flate2`/`tar`).

2. **`protonup/` module origin** — The directory `src/crosshook-native/crates/crosshook-core/src/protonup/` reportedly exists but is not declared in `lib.rs`. Verify its contents before creating files (may contain prior stub code).

3. **`astral-tokio-tar` version in `Cargo.lock`** — Must verify `grep "astral-tokio-tar" src/crosshook-native/Cargo.lock` shows `0.6.x` and that `tokio-tar` (abandoned) does not appear.

4. **`community_profiles.proton_version` field format** — The field is free-form TEXT in SQLite. No documentation specifies what formats community tap maintainers use in practice. The advisor's fuzzy matching via `normalize_alias` handles this, but real-world data samples would improve confidence.

5. **`AppSettingsIpcData` sync requirement** — The IPC DTO in `src-tauri/src/commands/settings.rs` must be manually kept in sync with `AppSettingsData`. No automated check exists; this is flagged as an integration challenge in `research-recommendations.md`.

6. **Wine-GE install path** — `research-external.md` notes Wine-GE installs to `~/.local/share/lutris/runners/wine/`, not Steam's `compatibilitytools.d/`. For CrossHook's Steam-focused workflow, Wine-GE scope in Phase 1 may not apply. Deferred to Phase 3 per `feature-spec.md`.
