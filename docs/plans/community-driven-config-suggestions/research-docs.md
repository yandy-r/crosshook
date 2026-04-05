# Documentation Research: Community-Driven Configuration Suggestions

## Overview

This document catalogs all documentation relevant to implementing the community-driven-config-suggestions feature. The feature extends CrossHook's existing ProtonDB integration to surface actionable configuration suggestions during profile creation and editing. Comprehensive prior research exists in this directory from an earlier session. The remaining work is primarily frontend wiring of already-implemented backend infrastructure.

---

## Architecture Docs

### Agent and Repo Rules

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md` — Agent policy for this repo: business logic in `crosshook-core`, thin IPC in `src-tauri`, Tauri commands use `snake_case`, Serde required on all IPC boundary types. Persistence classification rules mandatory for all feature plans.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — Full stack overview (Tauri v2, Rust core, React/TypeScript frontend, SQLite + TOML persistence), directory map, SQLite schema table inventory (18 tables, current schema v13), scroll container rules (`useScrollEnhance`), route layout CSS classes.

### Architecture in Prior/Current Research

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/community-driven-config-suggestions/research-architecture.md` — **Current session architecture research.** Full component inventory with exact file paths for all Rust, Tauri IPC, and React/TypeScript files. Data flow diagram. Confirms catalog has 25 entries. Identifies `LaunchSubTabs.tsx` and `LaunchPage.tsx` as the existing Apply flow orchestrators; notes `ProtonDbLookupCard.tsx`'s `onApplyEnvVars` callback is wired but the catalog-matching path for `enabled_option_ids` is not yet implemented.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/community-driven-config-suggestions/research-technical.md` — **Primary architecture doc for this feature.** Component diagram, data models with full Rust struct definitions, three-tier suggestion architecture (catalog bridge → heuristic → ML), all Tauri command signatures, text extraction pipeline details, caching strategy, scalability constraints, and list of files to create/modify.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/community-driven-config-suggestions/research-recommendations.md` — Phasing strategy (Phase 0: security + catalog bridge; Phase 1: apply-to-profile UI; Phase 2: enhanced aggregation; Phase 3+: ML deferred), risk assessment table, integration challenges.

---

## API Docs

### ProtonDB Integration

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/community-driven-config-suggestions/research-external.md` — **Definitive API reference for this feature.** All three ProtonDB endpoints (summary, counts, report feed) with actual implemented URLs, the reverse-engineered report feed hash formula (`report_feed_id`), PCGamingWiki Cargo API, Steam Store API, all Rust crates already in `Cargo.toml`, integration patterns, gotchas. Includes actual code examples from `client.rs`, `aggregation.rs`, `models.rs`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protondb-lookup/feature-spec.md` — Feature spec for the predecessor ProtonDB lookup feature (issue #53, CLOSED). Documents the original endpoint discovery and advisory-only design constraint.

### Security Analysis

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/community-driven-config-suggestions/research-security.md` — **Must-read before implementing any accept/apply flow.** Severity-leveled findings S1–S9. Critical: S2 (missing `LD_PRELOAD`, `PATH`, `HOME`, `LD_*` prefix in `RESERVED_ENV_KEYS`) must ship before any apply flow. Mitigation code provided. Also covers XSS risk (S5), HTTP response size limit (S6), and dep audit (S8-S9).

---

## Development Guides

### Build and Dev Scripts

- `/home/yandy/Projects/github.com/yandy-r/crosshook/scripts/build-native.sh` — Primary build script (`./scripts/build-native.sh`); `--binary-only` skips AppImage bundling (faster for dev iteration).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/scripts/dev-native.sh` — Dev mode with hot-reload (`./scripts/dev-native.sh`).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/scripts/prepare-release.sh` — Required before tagging releases; validates `CHANGELOG.md`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/CONTRIBUTING.md` — Dev setup prerequisites (Rust, Node.js 20+, system libs), clone/build steps, commit conventions, PR process.

### Testing

Verification command per `AGENTS.md` and `CLAUDE.md`:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

No frontend test framework is configured; use dev/build scripts for UI behavior.

### CI/CD

- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/workflows/release.yml` — Only workflow; triggers on `v*` tags; builds and uploads AppImage to GitHub Release.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/pull_request_template.md` — PR checklist: `build-native.sh --binary-only`, `cargo test`, AppImage build if touching build/packaging, plus area-specific checks for launch/, steam/, profile/, UI components.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/ISSUE_TEMPLATE/` — YAML form templates (`bug_report.yml`, `feature_request.yml`, `compatibility_report.yml`). Required for issue creation; blank issues are disabled.

---

## Feature Plans

### This Feature

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/community-driven-config-suggestions/feature-spec.md` — **Top-level spec. Start here.** Executive summary, all user stories, business rules (BR-1 through BR-9), edge cases table, technical architecture overview, data models, API design (all 3 Tauri commands), UX considerations, security considerations, task breakdown by phase, resolved decisions.

### Related Completed Features (Pattern References)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protondb-lookup/` — Full research and spec for the predecessor ProtonDB lookup feature that this feature builds on. The `research-integration.md`, `research-architecture.md`, and `research-patterns.md` files document what was built and why — important context for understanding current code structure.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/game-details-modal/` — Most recently completed feature. `implementation-handoff.md` and `manual-checklist.md` give templates for implementation handoff documents. `follow-up-issues.md` shows how follow-up issues were tracked post-ship.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protontricks-integration/` — Completed integration with external tool. Demonstrates the pattern for feature research → spec → implementation when touching profile forms.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/library-home/` — Completed library/game list feature; relevant because the game-details and library pages are adjacent to where ProtonDB tier badges could eventually appear (see UX research nice-to-have #17).

---

## Prior Research (Existing Files in This Directory)

All files produced in a previous research session. They are **comprehensive and accurate** — confirmed by cross-referencing actual source code. Read them before implementing.

| File                          | What it covers                                                                                                                                                                                     |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `feature-spec.md`             | Consolidated spec with all decisions, data models, API design, phasing, and security requirements                                                                                                  |
| `research-architecture.md`    | Current-session architecture research: full component inventory, data flow, confirmed catalog count (25), identified Apply flow orchestrators                                                      |
| `research-technical.md`       | Architecture, Rust data structs, Tauri command signatures, three-tier suggestion engine, files to create/modify                                                                                    |
| `research-external.md`        | ProtonDB API endpoints, hash formula, PCGamingWiki, Steam Store API, all relevant Rust crates                                                                                                      |
| `research-business.md`        | User stories, business rules (BR-1 to BR-16), edge cases (EC-1 to EC-11), domain model, state transitions, storage boundary classification                                                         |
| `research-security.md`        | Severity-leveled security findings S1–S9; S2 (LD_PRELOAD family) is CRITICAL and must ship first                                                                                                   |
| `research-ux.md`              | All user workflows (4 primary), UI anatomy, confidence display, dismissal patterns, competitive analysis (ProtonDB, Lutris, Bottles, Steam Deck, Heroic), error states table, performance UX       |
| `research-practices.md`       | Existing reusable code inventory, profile creation wizard integration point (Step 3 of `OnboardingWizard.tsx`), KISS assessment, testability patterns (golden fixtures, in-memory `MetadataStore`) |
| `research-recommendations.md` | Phasing strategy, technology choices, risk assessment, alternative approaches (Options A–E), task breakdown preview                                                                                |

---

## README Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/README.md` — Project overview; describes the three launch modes (`steam_applaunch`, `proton_run`, `native`), feature list, build instructions. Confirms ProtonDB guidance is already documented in the quickstart.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/getting-started/quickstart.md` — User-facing quickstart; includes a "ProtonDB guidance" section, confirming the existing lookup feature is user-visible. Relevant to ensure the new feature matches user expectations set by this guide.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md` — Steam/Proton workflow guide; describes `steam_applaunch` and `proton_run` contexts where ProtonDB suggestions are shown.

---

## Must-Read Documents (Prioritized for Implementers)

### REQUIRED reading before writing any code

1. **`feature-spec.md`** (this directory) — Consolidated spec with all decisions. The single source of truth.
2. **`research-security.md`** (this directory) — S2 (`LD_PRELOAD` family) must be fixed before ANY apply flow ships. Read the mitigation code.
3. **`AGENTS.md`** (repo root) — Architecture rules, directory map, SQLite schema, scroll container requirements.
4. **`research-architecture.md`** (this directory) — Current component inventory; confirms which files exist and the current Apply flow wiring state.
5. **`research-technical.md`** (this directory) — Data models, command signatures, files to create/modify. Has exact Rust struct definitions.
6. **`research-practices.md`** (this directory) — Existing reusable code inventory and `OnboardingWizard.tsx` integration point.

### REQUIRED reading for specific areas

- **Implementing `suggestions.rs`**: `research-technical.md` (three-tier architecture, `derive_suggestions()` pseudocode, catalog env index builder), `research-external.md` (env var safety validation code examples).
- **Implementing Tauri commands**: `research-technical.md` (API Design section), `AGENTS.md` (IPC rules: `snake_case` names, Serde on all types).
- **Implementing frontend hook and UI**: `research-practices.md` (`OnboardingWizard.tsx` integration point, wire pattern), `research-ux.md` (all workflows and UI anatomy), `research-technical.md` (TypeScript interfaces).
- **SQLite migration (schema v17)**: `feature-spec.md` (SQL DDL for `suggestion_dismissals`), `AGENTS.md` (migration pattern reference at `metadata/migrations.rs`).
- **Conflict resolution**: `research-business.md` (EC-7, BR-5, BR-7), `research-ux.md` (Workflow 2, `ProtonDbOverwriteConfirmation` dialog reuse).
- **ODbL compliance**: `research-external.md` (ODbL constraints section), `research-business.md` (BR-15).

### Nice-to-have context

- `research-business.md` — Domain model and state transition diagrams, storage boundary classification table.
- `research-ux.md` — Competitive analysis (Lutris, Bottles, Steam Deck, Heroic) for UX pattern decisions.
- `research-recommendations.md` — Risk assessment table, alternative approaches considered.
- `docs/plans/protondb-lookup/` — History of the predecessor feature; confirms what was already built.

---

## Documentation Gaps

The following information is not captured in documentation and must be verified by reading source code before implementing:

1. **Current schema version**: `AGENTS.md` says v13 but `feature-spec.md` targets schema v17 for `suggestion_dismissals`. The current actual schema version must be confirmed by reading `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` before writing the migration.

2. **`RESERVED_ENV_KEYS` exact current state**: The architecture researcher confirms the current blocklist in `aggregation.rs` covers `WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, and the `STEAM_COMPAT_*` prefix — but NOT `LD_PRELOAD`, `PATH`, `HOME`, etc. (the S2 gap). Read `aggregation.rs` to confirm before patching.

3. **Frontend env-key mirror sync (CRITICAL)**: `CustomEnvironmentVariablesSection.tsx` line 5-6 has the comment `/** Mirrors RESERVED_CUSTOM_ENV_KEYS in crosshook-core launch/request.rs */`. When the Rust blocklist in `aggregation.rs` is expanded for the S2 security fix, this frontend Set **must also be updated** to remain in sync. The source of truth is `launch/request.rs`, not `aggregation.rs` — check whether these two Rust-side lists need to be reconciled as part of the S2 fix.

4. **`is_safe_env_key` / `is_safe_env_value` visibility**: `research-technical.md` notes these must be made `pub(crate)` for accept-time re-validation. Current visibility (private vs. pub) must be confirmed in `aggregation.rs`.

5. **`OnboardingWizard.tsx` current state**: `research-practices.md` describes the step 3 integration point with line number references. Must be verified against current source before implementing, as the wizard may have changed since the research was written.

6. **`useScrollEnhance.ts` SCROLLABLE selector**: Any new scrollable container in the suggestion panel must be registered per `AGENTS.md`. The exact selector syntax is in `src/crosshook-native/src/hooks/useScrollEnhance.ts`.

7. **Config revision source enum**: `research-technical.md` mentions recording a config revision with source `ProtonDbSuggestion`. The valid source values for `config_revisions` must be confirmed from the profile or metadata models before use.

8. **Optimization catalog entry count**: Confirmed **25 entries** by `research-architecture.md` (read directly from `assets/default_optimization_catalog.toml`). The `research-technical.md` table listing only 21 known env-to-catalog mappings is a subset — 4 catalog entries have no corresponding ProtonDB env var pattern in the mapping table.
