# Documentation Research: proton-app-id

**Feature**: Proton App ID & Tri-Art System (optional `steam_app_id` on `RuntimeSection` + tri-art cover/portrait/background)
**Researcher**: docs-researcher
**Date**: 2026-04-02

---

## Overview

This document catalogs all documentation relevant to implementing the proton-app-id feature. A comprehensive feature spec and seven supporting research files already exist in `docs/plans/proton-app-id/`. Repository-level agent rules (`AGENTS.md`, `CLAUDE.md`) impose hard architectural constraints. The existing `game_images/` subsystem in `crosshook-core` already implements the full download/cache pipeline — the primary work is adding `steam_app_id` to `RuntimeSection`, extending custom art from cover-only to three types, adding `GameImageType::Background`, and building UI surfaces.

---

## Architecture Docs

### Primary Feature Specification

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/proton-app-id/feature-spec.md` — **The authoritative implementation contract.** Contains: executive summary, external dependencies table, business rules (BR-1 through BR-14), data models (`RuntimeSection`, `GameSection`, `GameImageType::Background`, TOML format), API design (`import_custom_art`, `fetch_game_cover_art`, `profile_list_summaries`), file-by-file modification list (~16 files + 1 new), 4-phase rollout plan, security considerations, storage boundary classification, persistence/usability section, and resolved decisions. Read this first.

### Repository Agent Rules (Architecture Constraints)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — Repository-level normative guidelines. **MUST read.** Defines:
  - Stack overview (Tauri v2, `crosshook-core` Rust, React 18/TypeScript strict, TOML settings, SQLite WAL)
  - Full directory map of `src/crosshook-native/`
  - SQLite schema table inventory (18 tables, current schema v13)
  - Persistence design classification rules: TOML vs. SQLite vs. memory
  - Hard constraint: binary blobs (images) must NOT use `external_cache_entries` — use filesystem + tracking table
  - Route layout CSS contracts (`crosshook-page-scroll-shell--fill`, `crosshook-route-stack`, etc.)
  - Commands quick reference (`./scripts/dev-native.sh`, `./scripts/build-native.sh`, `cargo test`)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md` — Agent-specific MUST/SHOULD rules. Mirrors AGENTS.md with emphasis on:
  - `crosshook-core` owns all business logic; `src-tauri` is IPC-thin only
  - Tauri IPC: `snake_case` command names, Serde on all boundary types
  - Conventional Commits required; `docs(internal):` prefix for plan/research files
  - Research artifacts must classify all data as TOML settings / SQLite metadata / runtime-only

### Architecture Research (Codebase Analysis — this feature)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/proton-app-id/research-architecture.md` — **Codebase-derived architecture analysis.** Contains: exact file paths with line numbers for all integration points, data flow diagrams (library grid resolution and profile save), precise integration point specs per phase (where `steam_app_id` and tri-art plug in), gotchas with exact line references. Notably surfaces:
  - `client.rs:125-148` — doc comment on `download_and_cache_image()` explaining the fallback chain and 4-step cache lifecycle
  - `import.rs:24-31` — doc comment on `import_custom_cover_art()` explaining idempotency and content-addressed naming
  - `exchange.rs:254-264` — comment block on `sanitize_profile_for_community_export()` — confirms S-03 gap is real (does not clear `custom_cover_art_path`)
  - `models.rs:43` — `skip_serializing_if = "RuntimeSection::is_empty"` TOML section elision pattern
  - `RuntimeSection.tsx:~188` — `proton_run` App ID field currently bound to `steam.app_id` (must be rebound to `runtime.steam_app_id`)
  - `profile.rs (commands):116` — multi-line comment on `capture_config_revision` behavior

### Prior Feature for Reference Pattern (UI Enhancements — same art infrastructure)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/ui-enhancements/feature-spec.md` — The preceding feature that established `game_image_cache` (v14 migration), `fetch_game_cover_art` IPC command, `useGameCoverArt` hook, `GameCoverArt` component, `MediaSection` single-slot UI, and the `convertFileSrc` asset protocol pattern. The proton-app-id feature extends all of these — read the UI enhancements spec to understand the baseline.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/ui-enhancements/research-technical.md` — Technical architecture of the UI enhancements feature. Shows the full component tree, `ProfileContext` state flow, `game_image_cache` SQL DDL, IPC command design, and how stale/unavailable states are handled. **Required reading before touching `game_images/` Rust modules.**

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protondb-lookup/feature-spec.md` — Established the external HTTP client pattern (`reqwest` singleton, `external_cache_entries` key naming, `MetadataStore` cache-key convention). The `game_images/client.rs` singleton was added for UI enhancements; the ProtonDB spec established the module structure to mirror.

---

## API Docs

### Steam CDN (No Auth Required)

From `docs/plans/proton-app-id/research-external.md`:

| Art Slot   | CDN File Pattern                    | Dimensions |
| ---------- | ----------------------------------- | ---------- |
| Cover      | `header.jpg`                        | 920×430    |
| Portrait   | `library_600x900_2x.jpg` (fallback chain) | 1200×1800 |
| Background | `library_hero.jpg`                  | 3840×1240  |

**Base URL**: `https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/`
**Auth**: None. No rate limits documented.
**External docs**: [Steamworks Library Assets](https://partner.steamgames.com/doc/store/assets/libraryassets)

### SteamGridDB API (Bearer Token, User-Provided)

From `docs/plans/proton-app-id/research-external.md`:

**Base URL**: `https://www.steamgriddb.com/api/v2`
**Auth**: `Authorization: Bearer <api_key>`

| Art Slot   | Endpoint                                              |
| ---------- | ----------------------------------------------------- |
| Cover      | `GET /grids/steam/{id}?dimensions=460x215,920x430`    |
| Portrait   | `GET /grids/steam/{id}?dimensions=342x482,600x900`    |
| Background | `GET /heroes/steam/{id}`                              |

**External docs**: [SteamGridDB API v2](https://www.steamgriddb.com/api/v2)

Confirmed allow-list for redirect policy (from security-researcher, 2026-04-02):
- `cdn.cloudflare.steamstatic.com`
- `steamcdn-a.akamaihd.net`
- `www.steamgriddb.com`
- `cdn2.steamgriddb.com`

### Tauri IPC Commands (New/Modified)

From `docs/plans/proton-app-id/feature-spec.md` and `research-technical.md`:

```rust
// GENERALIZED (was import_custom_cover_art):
#[tauri::command]
pub fn import_custom_art(
    source_path: String,
    art_type: Option<String>,  // "cover" | "portrait" | "background"; defaults to "cover"
) -> Result<String, String>

// EXTENDED (add "background" arm):
#[tauri::command]
pub fn fetch_game_cover_art(
    app_id: String,
    image_type: Option<String>,  // "cover" | "hero" | "capsule" | "portrait" | "background"
    // ... existing params
) -> Result<Option<String>, String>

// UPDATED (return effective_steam_app_id, custom_portrait_art_path):
#[tauri::command]
pub fn profile_list_summaries(...) -> Result<Vec<ProfileSummary>, String>
```

---

## Development Guides

### Build and Dev Scripts

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/internal-docs/local-build-publish.md` — Local build workflow, dev-server startup, AppImage build options (`--binary-only` for fast iteration), container build for CI reproducibility, and the `prepare-release.sh` sequence. Essential reference for testing and releasing.

### Security Implementation Guide

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/proton-app-id/research-security.md` — **REQUIRED for any implementer touching image download, HTTP client, or settings IPC.** Contains severity-leveled findings:
  - **S-01/S-06 (WARNING)**: Add redirect-policy domain allow-list to `GAME_IMAGES_HTTP_CLIENT` with Rust code snippet
  - **S-02 (WARNING)**: `settings_load` IPC leaks raw SGDB API key — return `has_steamgriddb_api_key: bool` only
  - **S-03 (WARNING)**: `sanitize_profile_for_community_export` must clear all three custom art path fields
  - **S-05 (WARNING)**: `fetch_game_cover_art` unknown `image_type` silently defaults to Cover — fail explicitly
  - **S-12 (WARNING)**: HTTP 401/403 from SGDB falls to stale cache — add `AuthFailure` variant, fall back to CDN
  - **S-13 (ADVISORY)**: Add 12-digit length cap to `steam_app_id` validation

### Test Command

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

---

## README Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/getting-started/quickstart.md` — End-user quickstart. Confirms `steam.app_id` and `[steam]` section are already first-class fields for `steam_applaunch` profiles. Documents ProtonDB guidance card behavior (triggered by non-empty App ID). Confirms `proton_run` requires only prefix path and Proton path — no App ID currently. This is the baseline the proton-app-id feature extends.

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md` — Deep dive on launch methods, ProtonDB guidance card, auto-populate, and launcher export. Explains existing `steam_applaunch` App ID fields that guide the new `runtime.steam_app_id` design. Background for implementers on why `steam.*` fields are launch-specific.

---

## Existing Feature Research (Summary)

All seven prior research files live in `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/proton-app-id/`:

| File | Scope | Key Findings |
|------|-------|--------------|
| `research-technical.md` | Data models, API design, Rust modules, integration points | Full `RuntimeSection`/`GameSection` struct changes; `GameImageType::Background` implementation; 16-file modification list; `is_empty()` exclusion note; `resolveArtAppId()` design; portrait candidate URL chain already in `client.rs` |
| `research-business.md` | User stories, business rules, domain model, workflows | 14 business rules covering art priority chain (BR-1), media-only `steam_app_id` (BR-2), 24h TTL (BR-7), custom art idempotency (BR-5), per-type independence (BR-10), portability split (BR-6); 5 user stories; 5 workflows; state machine for art lifecycle |
| `research-external.md` | Steam CDN, SteamGridDB API, existing codebase state | Both APIs already implemented in `game_images/`; 4 existing `GameImageType` variants; confirmed SGDB CDN domain is `cdn2.steamgriddb.com`; `webp` advisory does not apply; security gaps requiring action before ship |
| `research-practices.md` | Code reuse, modularity, KISS assessment, testability | Full reuse inventory: `download_and_cache_image`, `validate_image_bytes`, `safe_image_cache_path`, `game_image_cache` table, `import_custom_cover_art`; no new dependencies; existing test patterns (`MetadataStore::open_in_memory()`, `tempfile::tempdir()`); flat per-type fields preferred over BTreeMap |
| `research-ux.md` | User workflows, art slot design, competitive analysis | Three-slot media section (Cover 2.14:1, Portrait 2:3, Background 16:9); source badge (Custom/Auto/Not Set); thumbnail preview (optimistic); App ID field placement options A vs. B; competitive analysis (Playnite, Heroic, Lutris, Steam); must-have vs. nice-to-have UX list |
| `research-security.md` | Security findings by severity, secure coding guidelines | No CRITICAL blockers; 5 WARNINGs that must be addressed before shipping; S-01/S-06 redirect policy; S-02 API key leak; S-03 community export path disclosure; S-05 silent image_type default; S-12 auth failure behavior; Rust code snippets for redirect policy and settings DTO |
| `research-recommendations.md` | Phasing strategy, risk assessment, alternative approaches | Strongly recommends Option A (new `runtime.steam_app_id`); 4-phase rollout with task count estimates; 3 quick wins (test existing pipeline, add numeric validation, fix `GameCoverArt` null gate); 6 key decisions needed from team lead |

---

## Must-Read Documents (Prioritized for Implementers)

### REQUIRED before writing any code

1. **`docs/plans/proton-app-id/feature-spec.md`** — Authoritative scope, data models, file lists, API signatures, and phasing. Every decision is documented here.
2. **`AGENTS.md`** — Hard architectural constraints: `crosshook-core` owns business logic, IPC-thin `src-tauri`, `snake_case` commands, persistence classification, filesystem-not-SQLite for image blobs.
3. **`docs/plans/proton-app-id/research-security.md`** — Required before touching `game_images/client.rs`, `settings.rs`, or community export. Contains code-ready Rust snippets for all WARNING-level gaps.

### REQUIRED for specific phases

4. **`docs/plans/proton-app-id/research-architecture.md`** — Required before any code changes. Contains exact file paths, line numbers, and precise integration point specs per phase. The data flow diagrams show the current art resolution path end-to-end. Read before making any edits to understand exactly where each change lands.
5. **`docs/plans/proton-app-id/research-technical.md`** — Required before modifying Rust data models or React components. Full struct change specs, `is_empty()` note, `ProfileSummary` DTO design, `effective_profile()`/`storage_profile()` update points.
6. **`docs/plans/proton-app-id/research-business.md`** — Required before implementing profile-save logic, portability rules, or art resolution chain. The four-place `LocalOverrideGameSection` update requirement (BR-6) is critical to get right.
7. **`docs/plans/proton-app-id/research-practices.md`** — Required before creating new functions. Reuse inventory prevents duplication. Shows `import_custom_art` generalization pattern and closed-enum subdirectory routing.
8. **`docs/plans/ui-enhancements/research-technical.md`** — Required before touching `game_images/` Rust module. Established the baseline this feature extends.

### RECOMMENDED (implementation context)

9. **`docs/plans/proton-app-id/research-recommendations.md`** — Approach evaluation, risk table, and phasing rationale. Identifies 3 quick-win no-code-change tests, the migration concern for Option A, and background art scope caveat.
10. **`docs/plans/proton-app-id/research-ux.md`** — Required before implementing `MediaSection.tsx` or the App ID field in `RuntimeSection.tsx`. Contains thumbnail-first slot design, source badge pattern, and optimistic upload flow.
11. **`docs/plans/proton-app-id/research-external.md`** — Read before modifying `game_images/client.rs` or `steamgriddb.rs`. Contains confirmed CDN fallback chains, actual codebase state (existing `GameImageType` variants), and confirmed redirect allow-list domains.
12. **`docs/getting-started/quickstart.md`** — Confirms current `steam_applaunch` TOML profile structure and what `proton_run` currently requires. Baseline for understanding what changes.

### REFERENCE (as needed)

13. **`docs/features/steam-proton-trainer-launch.doc.md`** — Background on why `steam.*` fields are launch-specific. Informs the semantic rationale for Option A (`runtime.steam_app_id`).
14. **`docs/internal-docs/local-build-publish.md`** — Dev/build commands. Reference when setting up or testing the build.
15. **`docs/plans/protondb-lookup/feature-spec.md`** — HTTP client module structure pattern used by `game_images/`. Reference if modifying the client singleton.

---

## Documentation Gaps

The following areas have no written documentation and require reading source code or the architecture research directly:

1. **`game_images/` module API surface** — No dedicated written doc, but `research-architecture.md` provides data flow diagrams and integration specs. For exact function signatures and invariants, read `src/crosshook-native/crates/crosshook-core/src/game_images/` source. Key inline comments: `client.rs:125-148` (fallback chain doc comment on `download_and_cache_image`), `import.rs:24-31` (idempotency doc comment on `import_custom_cover_art`).

2. **`effective_profile()` / `storage_profile()` semantics** — No dedicated doc. `research-business.md` BR-6 documents the portability contract; `research-architecture.md` lists the exact field-by-field update points. The merge logic itself requires reading `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` lines ~408–470.

3. **`sanitize_profile_for_community_export` gap (S-03)** — `exchange.rs:254-264` has a comment block explaining what the function preserves. Architecture research confirms the function calls `portable_profile()` then manually clears specific paths but does NOT clear `custom_cover_art_path`. All three custom art paths must be explicitly cleared after the `portable_profile()` call.

4. **`useGameCoverArt` hook null gate clarification** — Architecture research clarifies the "null gate bug": the hook correctly short-circuits when `customUrl` is truthy (no IPC call needed). The real issue is that `profile_list_summaries` only populates `steamAppId` from `steam.app_id` (not `runtime.steam_app_id`), so `proton_run` profiles always get `steamAppId = undefined`. Phase 1 fix is in `profile_list_summaries` (call `resolve_art_app_id()`), not in the hook itself.

5. **`game_image_cache` SQLite table schema** — The v14 DDL is documented in `ui-enhancements/research-technical.md` but not in any standalone reference doc. The `image_type TEXT` column accepts arbitrary strings — no migration needed for "background". Unique key is `(steam_app_id, image_type, source)`.

6. **`import_custom_cover_art` error path behavior** — Current `MediaSection.tsx` catch block stores the unimported path on failure (security/correctness concern in `research-ux.md` open questions). Read the component source before implementing the generalized `import_custom_art` to avoid repeating the bug.

7. **`settings_load` frontend consumers** — No index of which frontend components call `settings_load`. Must grep the source before changing the return type for S-02 mitigation. This is a required pre-ship audit step.

8. **`RuntimeSection.tsx` proton_run App ID binding** — Architecture research confirms line ~188 is currently bound to `profile.steam.app_id`. Phase 1 requires rebinding to `profile.runtime.steam_app_id`. The `steam_applaunch` App ID field (line ~60) must remain on `steam.app_id` — they are different fields for different launch methods.
