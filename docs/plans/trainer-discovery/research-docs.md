# Documentation Research: Trainer Discovery

## Overview

CrossHook has deep, well-structured documentation covering architecture, developer guidelines, and feature context. The existing trainer-discovery research corpus (`docs/plans/trainer-discovery/`) is comprehensive and production-ready. Key implementation constraints are encoded in `AGENTS.md` and `CLAUDE.md` — both are required reading. The most critical gap: `feature-spec.md` (the merged authoritative spec) supersedes several earlier decisions in the individual research files — read `feature-spec.md` first.

---

## Relevant Files

### Prior Research (Trainer-Discovery Corpus)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/feature-spec.md` — **REQUIRED: Authoritative merged feature spec** (36 KB). Contains final decisions, exact data models, IPC API signatures, file-change manifest, and phase task breakdown. Start here.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-recommendations.md` — Consolidated recommendations: phasing strategy (Phase A MVP → Phase B external → Phase C FTS5), technology choices, reusable infrastructure inventory, risk/trade-off resolutions.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-technical.md` — Detailed technical spec: component diagram, schema changes (migration v17→v18), Rust structs, TypeScript interfaces, IPC command signatures with SQL, performance targets, offline behavior, validation rules.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-practices.md` — Engineering practices: reusable code inventory (with absolute file paths), modularity design, KISS assessment, build vs depend decisions, testability patterns. **Key finding: FTS5 is NOT available** (`rusqlite` uses `bundled` feature only; LIKE search is the only correct Phase A approach).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-security.md` — Security analysis: 0 CRITICAL, 5 WARNING, 5 ADVISORY findings. DMCA §1201 legal risk is the most significant (S1). URL validation, FTS5 injection prevention, WebKitGTK XSS mitigation, cache poisoning, field-length bounds.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-business.md` — Business rules, user stories (US-1 through US-8), domain model, edge cases, anti-pattern boundary ("CrossHook guides, does not host").
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-ux.md` — UX research: primary workflow (Game-First Discovery), UI patterns table, accessibility requirements, performance UX targets (300ms debounce, 500ms search SLA, skeleton loading).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-external.md` — External API analysis: Steam Web API, PCGamingWiki, IGDB, FLiNG RSS feed. Key finding: no trainer site has a public API; community-tap-first is the correct primary architecture. IGDB deferred (requires OAuth not in codebase).

### Architecture & Agent Guidelines

- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — **REQUIRED reading**. Normative implementation guidelines including: architecture rules (`crosshook-core` owns business logic, `src-tauri` is thin IPC only), Tauri IPC conventions (snake_case commands, Serde on all boundary types), Rust/TypeScript naming conventions, scroll container rules (`useScrollEnhance` — any new `overflow-y: auto` container must be added to `SCROLLABLE` selector), SQLite metadata DB current state (schema v13, 18 tables), persistence classification table.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md` — Agent rule precedence, MUST/MUST NOT constraints (mirrors AGENTS.md), quick command reference. MUST read before any implementation.

### Feature Documentation

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md` — Full Steam/Proton trainer launch workflow. Covers launch methods (`steam_applaunch`, `proton_run`, `native`), auto-discovery, trainer loading modes (SourceDirectory vs CopyToPrefix), launcher export lifecycle, health dashboard integration. Essential for understanding trainer-related workflows discovery integrates with.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/profile-duplication.doc.md` — Profile duplication/clone guide. Shows how profile management actions are implemented (relevant for "Import Profile" CTA in discovery).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/getting-started/quickstart.md` — User-facing quickstart. Shows existing community profiles workflow (training data for where discovery panel should fit in UX).

### Development Guides

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/research/additional-features/implementation-guide.md` — Feature implementation priority guide (#78). Contains: SQLite schema current state (v13), features requiring new migrations vs reusing existing tables, phase ordering (Phases 1-7), anti-pattern checklist (mandatory before starting any feature). Issue #67 (trainer discovery) is Phase 7 / P3.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/research/additional-features/deep-research-report.md` — The original deep research analysis. Contains priority matrix (P0/P1/P2/P3), effort/impact analysis for all features, trainer discovery flagged as 3/8 perspectives support / Very High effort / Phase 7. Read for strategic context.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/internal-docs/local-build-publish.md` — Local build and publish procedures.

### GitHub Templates and Workflow

- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/pull_request_template.md` — PR template. All PRs must: link `Closes #...`, check off build verification (`./scripts/build-native.sh --binary-only`, `cargo test -p crosshook-core`), follow conditional checklist for touched areas.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/ISSUE_TEMPLATE/feature_request.yml` — Feature request YAML form. Required for any new GitHub issues.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/workflows/release.yml` — CI release workflow reference.

### Issue Tracking

- **GitHub Issue #67** (`feat(profiles): trainer discovery and search integration`) — The canonical tracking issue for this feature. Open, labeled `area:profiles`, `area:security`, `priority:low`, `type:feature`. Contains storage boundary classification and persistence/usability section.

---

## Architectural Patterns

- **Business logic separation**: All discovery logic goes in `crosshook-core/src/discovery/`. The `src-tauri/src/commands/discovery.rs` file is a thin IPC adapter only (~20-50 lines). No business logic in command handlers.
- **IPC naming**: Tauri commands use `snake_case` (e.g., `discovery_search_trainers`). Frontend `invoke()` must match exactly. All IPC-crossing types need `#[derive(Serialize, Deserialize)]`.
- **Module structure pattern**: New modules follow `crosshook-core/src/{domain}/mod.rs` + focused subfiles (`models.rs`, `search.rs`, `client.rs`, `matching.rs`). Mirror the `protondb/` module layout.
- **Cache pattern**: All external data flows through `metadata/cache_store.rs` `put_cache_entry` / `get_cache_entry` on `external_cache_entries`. Never build a bespoke cache.
- **HTTP client singleton**: `OnceLock<reqwest::Client>` with 6s timeout and CrossHook User-Agent. Create a separate `TRAINER_DISCOVERY_HTTP_CLIENT` — do not share the ProtonDB or Steam metadata clients.
- **React hooks wrap IPC**: Frontend uses `useTrainerDiscovery.ts` (mirrors `useProtonDbSuggestions.ts`). Request-ID race guard (`requestIdRef`) is the cancellation pattern for stale async responses.
- **Scroll containers**: Any new `overflow-y: auto` container must be added to `SCROLLABLE` in `src/crosshook-native/src/hooks/useScrollEnhance.ts`. Inner containers use `overscroll-behavior: contain`.
- **Persistence classification** (mandatory before implementation): TOML settings = user preferences, SQLite metadata = operational/cache/history, in-memory = ephemeral UI state.

---

## Gotchas & Edge Cases

- **FTS5 is NOT available**: `rusqlite` is configured with `features = ["bundled"]` only — no `bundled-full`, no FTS5. The `community_profiles_fts` virtual table described in `research-technical.md` would silently fail at runtime. LIKE-based search is the only correct Phase A approach. FTS5 requires an explicit feature flag change tracked as a separate issue.
- **Trainer versions are not semver**: Real trainer versions look like "v1.0 +DLC", "Build 12345", "2024.12.05". The `semver` crate rejects most of these. Version matching must use advisory text (display the community-provided string as-is), not computed semver comparison.
- **MAX_CACHE_PAYLOAD_BYTES = 524,288**: `external_cache_entries` silently stores `NULL payload_json` for payloads exceeding 512 KiB. Individual trainer source metadata per game is small (~1-5 KiB), but aggregate search indexes would exceed this. Do not attempt to cache full game lists in a single entry.
- **Discrepancy between research files**: `research-recommendations.md` says "zero new tables" for Phase A; `feature-spec.md` (the authoritative doc) requires a new `trainer_sources` table (migration v17→v18). The feature-spec decision supersedes the earlier recommendation. The separate `trainer-sources.json` file structure per game directory (not a single `source_url` field on `CommunityProfileMetadata`) is the resolved approach.
- **Schema v17, not v13**: The implementation guide shows the DB at schema v13 (18 tables), but `feature-spec.md` references migration v17→v18. Actual current schema version must be verified against `metadata/migrations.rs` before starting migration work.
- **WebKitGTK XSS**: Trainer source URLs from community taps are untrusted user-controlled text. Never render via `dangerouslySetInnerHTML` or `<a href>` navigation. Always use Tauri's `open()` shell plugin. Validate HTTPS-only before opening.
- **DMCA §1201 legal risk**: Linking to trainer download sources carries anti-trafficking liability risk. The feature must be opt-in (`discovery_enabled = false` default in `settings.toml`) with a one-time legal disclaimer on first enable. This is a hard requirement, not optional UX polish.
- **IPC command split (sync vs async)**: The fast tap search (`discovery_search_trainers` — synchronous SQLite query) must be a separate command from the slow external search (Phase B — async HTTP). Do not mix fast and slow operations in a single command; tap results must not wait on network availability.
- **`useScrollEnhance` registration**: If `TrainerDiscoveryPanel` has a scrollable results list, its container must be added to the `SCROLLABLE` selector in `useScrollEnhance.ts` to prevent dual-scroll jank in WebKitGTK.
- **Import via existing flow**: Discovery's "Import Profile" CTA must wire into the existing `community_import_profile` IPC command and import wizard — do not build a parallel import mechanism.

---

## Must-Read Documents (Prioritized)

### Required Before Writing Any Code

1. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/feature-spec.md` — Final decisions, data models, IPC signatures, file manifest. The authoritative implementation contract.
2. `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — Architecture rules, naming conventions, SQLite table inventory, persistence classification. Normative guidelines that override general best practices.
3. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-practices.md` — Reusable code inventory with exact file paths, FTS5 unavailability confirmation, KISS assessment, module pattern.

### Required for Specific Implementation Areas

4. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-technical.md` — Exact SQL queries (Phase A LIKE, Phase B FTS5), schema change SQL, Rust struct definitions, IPC response examples, system constraints table.
5. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-security.md` — URL validation pattern, FTS5 injection prevention, WebKitGTK XSS mitigation, DMCA legal requirements (S1 is a hard stop).
6. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-ux.md` — UI workflow, component patterns table, accessibility requirements, badge reuse patterns.
7. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md` — Trainer workflow context; required for understanding how discovery integrates with existing profile/trainer sections.

### Nice-to-Have for Full Context

8. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-recommendations.md` — Phasing rationale, alternative approaches, resolved trade-offs.
9. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/trainer-discovery/research-business.md` — User stories, business rules, edge case table.
10. `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/research/additional-features/implementation-guide.md` — Feature priority context, SQLite schema state, storage boundary checkpoint methodology.

---

## Documentation Gaps

- **No dedicated API reference for `MetadataStore`**: The public API of `MetadataStore` (what methods are exposed, their signatures) is only discoverable by reading the source. Implementers need to read `crosshook-core/src/metadata/mod.rs` and `metadata/community_index.rs` directly.
- **No community tap schema documentation**: The format for `community-profile.json` and the new `trainer-sources.json` is defined in the feature spec but not in any end-user or developer-facing reference doc. Tap maintainers have no published schema reference.
- **Schema version gap**: The implementation guide shows schema v13; `feature-spec.md` assumes v17→v18 migration. There is no single document that tracks the full migration history with current schema version. The ground truth is `metadata/migrations.rs`.
- **No `useScrollEnhance` registration documentation**: The requirement to register new scroll containers in `useScrollEnhance.ts` is documented in `AGENTS.md` but there is no checklist or developer guide that makes this discoverable at component-authoring time. Easy to miss.
- **No frontend test strategy for this feature**: `research-practices.md` confirms there is no configured frontend test framework. The test strategy for `TrainerDiscoveryPanel.tsx` and `useTrainerDiscovery.ts` is undefined — only pure Rust functions and `MetadataStore::open_in_memory()` tests have a clear path.
