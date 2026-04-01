# Documentation Research: UI Enhancements (Profiles Page Restructuring + Game Metadata & Cover Art)

**Feature**: Restructure Profiles page Advanced section + Steam Store API game metadata and cover art integration (issue #52)
**Researcher**: docs-researcher
**Date**: 2026-04-01

---

## Overview

This document catalogs all documentation relevant to implementing the UI enhancements feature — Profiles page restructuring with sub-tab navigation, game metadata integration via Steam Store API, and cover art caching. The feature spec (`feature-spec.md`) is the authoritative implementation contract. Seven prior research files cover architecture, UX, security, practices, business rules, external APIs, and recommendations. Repository-level agent rules (`AGENTS.md`, `CLAUDE.md`) impose hard constraints on architecture, naming, commit format, and PR workflow that implementers must follow.

---

## Architecture Docs

### Primary Feature Specification

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/ui-enhancements/feature-spec.md` — **The authoritative implementation contract**. Contains: executive summary, external dependencies table, business rules, data model (`game_image_cache` SQL DDL), API signatures for `fetch_game_metadata` and `fetch_game_cover_art`, file-by-file list of what to create and modify, phasing (Phase 0–4), risk assessment, persistence classification, and decisions log. Read this first and last.

### Repository Agent Rules (Architecture Constraints)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — Repository-level normative guidelines. **MUST read**. Defines:
  - Stack overview table (Tauri v2, `crosshook-core` Rust, React/TypeScript strict, TOML settings, SQLite WAL)
  - Full directory map of `src/crosshook-native/`
  - SQLite schema table inventory (18 tables, current schema v13)
  - Persistence design classification rules (TOML vs. SQLite vs. memory)
  - Hard rule: do not cache binary blobs in `external_cache_entries` (512 KiB cap — use filesystem + tracking table for images)
  - Commands quick reference (`./scripts/dev-native.sh`, `./scripts/build-native.sh`, `cargo test`)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md` — Agent-specific rules (MUST/SHOULD). Mirrors AGENTS.md with emphasis on:
  - `crosshook-core` owns all business logic; `src-tauri` is IPC-thin only
  - Tauri IPC: `snake_case` command names, Serde on all boundary types
  - Conventional Commits required; `docs(internal):` prefix for files under `docs/plans/`, `docs/research/`, `docs/internal/`
  - Label taxonomy (only `type:`, `area:`, `platform:`, `priority:`, `status:` families)

### Prior Feature for Reference Pattern (ProtonDB Lookup)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protondb-lookup/feature-spec.md` — **The template this feature follows**. ProtonDB lookup is the immediate predecessor — it established `reqwest` HTTP client pattern, `external_cache_entries` cache-key naming (`protondb:summary:v1:{appId}`), the `MetadataStore::put_cache_entry`/`get_cache_entry` pair, and the `ProtonDbLookupResult` IPC DTO shape. The steam metadata module should mirror this module structure exactly.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protondb-lookup/research-technical.md` — Technical architecture of ProtonDB lookup. Shows exact module layout (`protondb/mod.rs`, `client.rs`, `models.rs`, `aggregation.rs`), `external_cache_entries` key naming conventions, IPC command design, and how stale/unavailable states are differentiated from hard failures.

---

## API Docs

### Steam Store API

- **Endpoint**: `GET https://store.steampowered.com/api/appdetails?appids={id}` — returns JSON with `name`, `short_description`, `genres`, `header_image` URL. No API key required.
- **Feature spec reference**: `feature-spec.md` lines 9–24 — includes all image URL patterns (header_image 460×215, capsule, library portrait, hero).
- **External docs**: <https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI>

### SteamGridDB API (Phase 3 only)

- **Endpoint**: `GET /grids/steam/{id}`, `/heroes/steam/{id}`, `/logos/steam/{id}` — requires user-provided Bearer token.
- **Feature spec reference**: `feature-spec.md` lines 25–35.
- **External docs**: <https://www.steamgriddb.com/api/v2>
- **Security note**: API key must be stored in `settings.toml` with UX warning; do not log it. See `research-security.md` section K1 for migration path toward OS keyring.

### Radix UI Tabs (already installed)

- **Package**: `@radix-ui/react-tabs` v1.1.13 — already in `package.json`
- **External docs**: <https://www.radix-ui.com/primitives/docs/components/tabs>
- **Usage pattern**: `research-external.md` contains the full `Tabs.Root`/`Tabs.List`/`Tabs.Trigger`/`Tabs.Content` JSX snippet and keyboard navigation matrix.

### Tauri v2 Asset Protocol

- **Purpose**: Required for rendering locally cached images via `asset://` URL in the webview.
- **Frontend**: `import { convertFileSrc } from '@tauri-apps/api/core'` — converts absolute file path to `asset://` URL.
- **Config**: Must add `assetProtocol.enable=true` + scope `$LOCALDATA/cache/images/**` to `tauri.conf.json`; must add `img-src 'self' asset: http://asset.localhost` to CSP.
- **External docs**: <https://v2.tauri.app/security/csp/>
- **Security detail**: `research-security.md` sections C1 and C2 provide exact JSON config snippets for both `tauri.conf.json` and `capabilities/default.json`.

---

## Development Guides

### Build and Dev Scripts

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/internal-docs/local-build-publish.md` — Local build workflow, dev-server startup, AppImage build options (`--binary-only` for fast iteration), and container build for CI reproducibility.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CONTRIBUTING.md` — Developer setup: prerequisites (Rust stable, Node 20+, GTK3/WebKit2GTK), `./scripts/install-native-build-deps.sh` automation, dev mode, test command, project architecture summary, and commit/PR workflow.

### Security Implementation Guides

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/ui-enhancements/research-security.md` — **REQUIRED for any implementer touching image download, cache construction, or Tauri config**. Contains:
  - `validate_image_bytes()` Rust implementation using `infer` crate for SVG rejection (finding I1)
  - `safe_image_cache_path()` Rust implementation for path traversal prevention (finding I2)
  - `asset://` / CSP exact JSON configuration (findings C1, C2)
  - SHA-256 checksum verification pattern (finding I5)
  - API key logging prevention (`#[tracing::instrument(skip(api_key))]`)
  - Full severity-leveled findings table (WARNINGs W1, W3, I1, I2, K1; ADVISORYs A1–A4, I3–I7, C1–C3)

---

## README Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/README.md` — Project overview, launch modes (steam_applaunch, proton_run, native), feature list, build quickstart, and links to feature guides. Good orientation for understanding the three runner methods that gate section visibility in the profile editor.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/getting-started/quickstart.md` — End-user quickstart. Documents profile TOML structure (including `[steam] app_id` field already present), ProtonDB guidance behavior, custom environment variable precedence rules, and the Install Game workflow. Confirms `steam.app_id` is already a first-class field — no profile schema change needed.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md` — Steam/Proton launch workflow detail. Covers auto-populate, launcher export, console view, dry run. Useful background for understanding what the profile editor is building toward.

---

## Prior Research Files (this feature)

All seven prior research files live in `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/ui-enhancements/`:

| File                          | Scope                                                         | Key Findings                                                                                                                                                                                                                                                                           |
| ----------------------------- | ------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `research-technical.md`       | Component hierarchy, data models, API design, Rust modules    | Full component tree diagram; `game_image_cache` SQL DDL; IPC command signatures; CSS pattern inventory (`crosshook-subtab-*` already in theme.css); `ProfileContext` state flow showing tab switches cannot lose context-held state                                                    |
| `research-ux.md`              | Game launcher UX patterns, cover art loading, grid/list views | Current layout pain points; confirmed sub-tab CSS infrastructure exists but is unused; `ProfileGameCard` component design with props interface; cover art shimmer skeleton pattern; controller-mode touch targets                                                                      |
| `research-security.md`        | Image download security, path traversal, asset protocol CSP   | Severity-leveled findings; Rust code snippets for `validate_image_bytes` and `safe_image_cache_path`; full `tauri.conf.json` config blocks; see **Security Implementation Guides** above                                                                                               |
| `research-practices.md`       | Code reuse, KISS assessment, build-vs-depend decisions        | Inventory of reusable files (ProfileFormSections, CollapsibleSection, ThemedSelect, ContentArea, etc.); ProtonDB cache pattern reuse analysis; cover art card grid engineering against existing `crosshook-community-browser__profile-grid` CSS; `ProfileGameCard` component interface |
| `research-external.md`        | APIs, library versions, Steam CDN URL patterns                | Verified `@radix-ui/react-tabs` v1.1.13 already installed; full Radix Tabs JSX snippet; Steam CDN URL patterns; SteamGridDB endpoint formats; `reqwest` + `MetadataStore` reuse confirmation; `convertFileSrc` asset protocol usage                                                    |
| `research-business.md`        | User stories, section inventory, business rules               | Full section-by-section inventory of `ProfileFormSections`; current layout container hierarchy; datum classification for persistence; section groupings for card/tab layout                                                                                                            |
| `research-recommendations.md` | Unified phasing, approach evaluation, decisions               | Approach A (cards) vs. B (sub-tabs) vs. hybrid evaluation; dependency analysis table; design intent evidence for sub-tabs (unused CSS tokens); Phase 0–4 task list with parallelization notes; technology decision rationale table                                                     |

---

## GitHub Workflow Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/.github/pull_request_template.md` — PR template. All PRs must link `Closes #<issue>`, fill the type of change checkboxes, and verify the build checklist. Relevant checklist items for this feature: `src/components/` and `src/hooks/` UI changes; `crates/crosshook-core/src/profile/` if profile types change.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CONTRIBUTING.md` (also under **Development Guides**) — Covers issue template requirements (YAML form templates, never title-only issues), label taxonomy, and branch/commit conventions.

---

## Must-Read Documents (Prioritized Reading List)

### REQUIRED before writing any code

1. **`docs/plans/ui-enhancements/feature-spec.md`** — The authoritative scope, data models, file lists, API signatures, and phasing. Everything else is supporting context.
2. **`AGENTS.md`** — Hard architectural constraints: `crosshook-core` owns business logic, `src-tauri` is IPC-thin, `snake_case` command names, persistence classification rules, 512 KiB `external_cache_entries` cap.
3. **`docs/plans/ui-enhancements/research-security.md`** — Required for any work touching image download, filesystem cache, or `tauri.conf.json`. Contains code-ready Rust snippets for SVG rejection and path traversal mitigation.

### REQUIRED for specific phases

4. **`docs/plans/ui-enhancements/research-technical.md`** — Required before touching Rust modules or React component decomposition. Full component tree, `ProfileContext` state flow, CSS pattern inventory, IPC command design.
5. **`docs/plans/ui-enhancements/research-practices.md`** — Required before creating new components. Inventory of reusable existing components and CSS grid infrastructure. Prevents duplication.
6. **`docs/plans/protondb-lookup/research-technical.md`** — Required before implementing `steam_metadata/` Rust module. This is the exact pattern to mirror: HTTP client, cache-key naming, DTO structure.

### RECOMMENDED (implementation context)

7. **`docs/plans/ui-enhancements/research-recommendations.md`** — Approach evaluation and phasing rationale. Read before planning Phase 0–3 tasks to understand trade-offs already analyzed.
8. **`docs/plans/ui-enhancements/research-ux.md`** — Required for `GameCoverArt`, `GameMetadataBar`, and `ProfileGameCard` component implementations. Contains shimmer skeleton pattern and controller-mode requirements.
9. **`docs/plans/ui-enhancements/research-external.md`** — Read if implementing Steam Store API client or SteamGridDB integration. Contains Steam CDN URL domain allowlist and Radix Tabs JSX reference.
10. **`docs/getting-started/quickstart.md`** — Confirms `steam.app_id` TOML field already exists; documents ProtonDB card behavior as a UX precedent for the Steam metadata card.

### REFERENCE (as needed)

11. **`docs/plans/ui-enhancements/research-business.md`** — Full section inventory of `ProfileFormSections`; datum classification; business rules. Reference when deciding what goes in which card/tab.
12. **`docs/internal-docs/local-build-publish.md`** — Dev/build commands. Reference when setting up or troubleshooting the build.
13. **`CONTRIBUTING.md`** — PR and issue workflow. Reference before opening issues or PRs.

---

## Documentation Gaps

The following areas have no existing documentation and may require implementers to consult source code directly:

1. **`MetadataStore` API surface** — No dedicated doc for `put_cache_entry`/`get_cache_entry` signatures, TTL semantics, or the 512 KiB cap enforcement behavior. Must read `src/crosshook-native/crates/crosshook-core/src/metadata/` source directly. The ProtonDB research (`research-technical.md`) contains practical usage examples.

2. **`CollapsibleSection` component props** — No dedicated component API doc. The `meta` prop (inline badge slot) and controlled/uncontrolled behavior must be inferred from the component source at `src/crosshook-native/src/components/ui/CollapsibleSection.tsx`.

3. **Controller mode CSS system** — No dedicated guide explaining the `:root[data-crosshook-controller-mode='true']` override pattern. The UX research (`research-ux.md`) documents the sub-tab-specific overrides; the full system requires reading `variables.css` and `theme.css` directly.

4. **`ProfileContext` / `useProfile` hook interface** — No dedicated hook API doc. The technical research (`research-technical.md`) documents the state flow diagram; the exact hook interface must be read from `src/crosshook-native/src/hooks/useProfile.ts`. A JSDoc block at `src/crosshook-native/src/contexts/ProfileContext.tsx` lines 1–8 explains the ownership split between `ProfileContext` (profile CRUD/selection) and `PreferencesContext` (settings/recent files) — read before touching the context hierarchy.

5. **`tauri.conf.json` / capabilities schema** — No internal doc explaining the current capability surface or how to extend it. The security research (`research-security.md`, section C1–C3) provides the exact configuration blocks needed for this feature.

6. **Migration pattern** — No dedicated migration guide beyond what is in `AGENTS.md`'s migrations.rs reference. For the v14 `game_image_cache` migration, read the existing v13 migration in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` directly as the template.

7. **Cache-first lookup with stale fallback** — The clearest documentation is inline in `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`. It is the primary reference for implementing the `steam_metadata` client, showing how to differentiate stale cache from hard failure. No separate written guide exists.
