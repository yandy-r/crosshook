# Context Analysis: trainer-discovery

## Executive Summary

Trainer discovery extends CrossHook's existing community tap pipeline to make trainer sources searchable. Phase A (MVP) creates a new `trainer_sources` SQLite table indexed from per-game `trainer-sources.json` manifests in community taps, queries it with LIKE-based search, and surfaces results in a new `TrainerDiscoveryPanel`. Zero new Rust dependencies required. The primary implementation risk is legal (DMCA §1201), not technical — opt-in with consent dialog is a hard blocker, not optional.

---

## Schema decision (resolved)

**Authoritative:** [`feature-spec.md`](./feature-spec.md) → **Decisions (Resolved) → Decision 1 — Option B**: trainer source metadata lives in per-game **`trainer-sources.json`** manifests inside community taps and is indexed into a dedicated SQLite **`trainer_sources`** table. Phase A search runs **LIKE** queries against `trainer_sources` (joined to `community_taps` for `tap_url`). **`CommunityProfileRow` / `CommunityProfileMetadata` are not extended** with `source_url` / `source_name` for discovery; those columns are not part of this design.

This matches the correction note at the top of [`analysis-code.md`](./analysis-code.md). [`shared.md`](./shared.md) is aligned to the same Option B story.

> **Planning artifact scope:** Documents under `docs/plans/trainer-discovery/` are **planning-only** until separate implementation PRs land in `crosshook-core`, `src-tauri`, and the React app. Treat `feature-spec.md` as the contract for what to build; do not assume the SQLite schema or IPC surface exists in-tree until those PRs merge.

---

## Architecture Context

- **System Structure**: Business logic in `crosshook-core`; thin IPC handlers in `src-tauri/commands/`; React UI in `src/crosshook-native/src/`. New `discovery/` domain module mirrors `protondb/` structure.
- **Data Flow**: `trainer-sources.json` in tap repo → `index_trainer_sources()` during tap sync → `trainer_sources` SQLite table → `discovery/search.rs` LIKE query → `discovery_search_trainers` IPC command → `useTrainerDiscovery` hook → `TrainerDiscoveryPanel`.
- **Integration Points**:
  - Tap sync: extend `community_index.rs` to walk directories for `trainer-sources.json` alongside `community-profile.json`
  - Import CTA: wire into existing `community_import_profile` IPC — no new import path
  - Hash verification: propagate `sha256` from manifest; enforcement stays in existing `trainer_hash.rs`
  - Version correlation (Phase B): reuse `compute_correlation_status()` from `metadata/version_store.rs`

---

## Critical Files Reference

### Rust — crosshook-core

- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs`: Add `index_trainer_sources()` here — same transaction pattern (DELETE+INSERT, A6 bounds, watermark skip). Must also add URL validation (HTTPS-only).
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Add **`migrate_17_to_18`** after the existing `if version < 17` block for `CREATE TABLE trainer_sources` + indexes. **SQLite `user_version` today ends at 17** after `run_migrations()` (see `migrations.rs`). (`AGENTS.md` may still say “schema version 13” for the table-inventory doc snapshot — **migration numbering follows `user_version` in code.**)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: Add `search_trainer_sources()` public method on `MetadataStore` using `with_conn` wrapper.
- `src/crosshook-native/crates/crosshook-core/src/lib.rs`: Add `pub mod discovery;`
- `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: **Clone** this for `discovery/client.rs` in Phase B — `OnceLock<reqwest::Client>`, cache→live→stale-fallback pattern.
- `src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`: `tokenize()`/`token_hits()` reusable for Phase B fuzzy matching.
- `src/crosshook-native/crates/crosshook-core/src/offline/hash.rs`: Existing hash verification; discovery surfaces `sha256` from manifest without re-implementing.

### Tauri IPC Layer

- `src/crosshook-native/src-tauri/src/lib.rs`: Register discovery commands in `invoke_handler!`.
- `src/crosshook-native/src-tauri/src/commands/community.rs`: **Reference** for sync IPC pattern + mandatory `#[cfg(test)]` contract test block (lines 311–353).
- `src/crosshook-native/src-tauri/src/commands/protondb.rs`: **Reference** for async IPC with `.inner().clone()` across await.

### Frontend

- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: CRITICAL — register `TrainerDiscoveryPanel`'s scroll container in `SCROLLABLE` selector.
- `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`: **Reference** for hook pattern with `requestIdRef.current` race guard.
- `src/crosshook-native/src/components/CommunityBrowser.tsx`: Reference for result card + `matchesQuery()` client-side filter pattern.
- `src/crosshook-native/src/components/pages/CommunityPage.tsx`: Host page — discovery panel sits here as sibling/nested tab.

---

## Patterns to Follow

- **MetadataStore Facade**: All SQLite reads/writes go through `with_conn` / `with_conn_mut` on `MetadataStore`. Sub-store functions receive `&Connection` or `&mut Connection` directly. `metadata/mod.rs:97–159`.
- **Thin IPC Command Handlers**: ~50–100 lines. Inject `State<'_, T>`, delegate to `crosshook-core`, map errors with `.map_err(|e| e.to_string())`. `commands/protondb.rs:49–57`.
- **IPC Contract Tests**: Every `commands/*.rs` ends with `#[cfg(test)]` casting each handler to its explicit function-pointer type. MANDATORY. `commands/community.rs:311–353`.
- **Watermark-Skip Indexing**: Compare `last_head_commit` before re-indexing; transactional DELETE+INSERT with `Immediate` transaction; A6 field-length bounds before INSERT. `metadata/community_index.rs:22–165`.
- **Cache-First Fetch**: normalize → check fresh cache → live fetch → persist → stale fallback. `protondb/client.rs:85–130`.
- **Domain Module Layout**: `mod.rs` (re-exports) + `models.rs` + `search.rs` (+ `client.rs` in Phase B). Mirror `protondb/` directory structure.
- **Serde on IPC Boundary**: `#[serde(rename_all = "camelCase")]` on result structs (frontend gets camelCase). Optional fields: `#[serde(default, skip_serializing_if = "Option::is_none")]`. State enums: `#[serde(rename_all = "snake_case")]` with `#[default]`.
- **Frontend Hook Pattern**: `requestIdRef.current` increment for stale request cancellation; `useState<T | null>`; return `{ data, loading, error, refresh }`. `useProtonDbSuggestions.ts`.

---

## Cross-Cutting Concerns

- **DMCA §1201 (S1 — Hard blocker)**: `discovery_enabled = false` default in `settings.toml` (opt-in; matches `feature-spec.md` Decision 2). Consent dialog with legal disclaimer MUST appear on first enable before any results render.
- **URL validation (S2, S6)**: HTTPS-only enforced at index time in `index_trainer_sources()` — reject `http://` and non-URL strings before INSERT.
- **A6 field-length bounds (S7)**: Apply `check_a6_bounds` to `source_name`, `source_url`, `notes`, `trainer_version`, `game_version` fields during indexing.
- **XSS / external link safety (S5)**: All source URLs opened via Tauri `open()` plugin. Never `<a href>` navigation in WebKitGTK WebView. Never `dangerouslySetInnerHTML`.
- **`useScrollEnhance` registration**: Any `overflow-y: auto` container added in `TrainerDiscoveryPanel` must be in the `SCROLLABLE` selector — missing this causes dual-scroll jank on WebKitGTK.
- **IPC sync/async split**: `discovery_search_trainers` is a synchronous SQLite query (fast, no network). Phase B external search is async HTTP. Do not combine — tap results must never wait on network.
- **MetadataStore disabled path**: Return empty results (`Ok(TrainerSearchResponse { results: vec![], total_count: 0 })`), never panic.
- **No frontend tests**: Only `cargo test -p crosshook-core` with `MetadataStore::open_in_memory()` for Rust unit tests. No Jest/Vitest for hooks or components.

---

## Parallelization Opportunities

1. **Rust schema + UI scaffolding** (parallel): `migrations.rs` v18 + `community_index.rs` extension vs. `types/discovery.ts` + `TrainerDiscoveryPanel.tsx` skeleton
2. **Rust models + TS interfaces** (parallel): `discovery/models.rs` and `src/types/discovery.ts` mirror each other exactly — can be drafted simultaneously
3. **Pure function tests** (independent): `MetadataStore::open_in_memory()` unit tests for search + URL validation can be written before IPC wiring is complete
4. **Legal disclaimer dialog** (independent): Can be built as a standalone component gated by `discovery_enabled` without backend integration

**Hard sequential dependencies:**

1. `migrations.rs` **`migrate_17_to_18`** → `community_index.rs` (`index_trainer_sources` needs the table to exist)
2. `discovery/models.rs` + `search.rs` → `commands/discovery.rs`
3. `commands/discovery.rs` registered in `invoke_handler!` → `useTrainerDiscovery.ts` runnable
4. Legal disclaimer flow → Phase A can ship

---

## Implementation Constraints

- **FTS5 unavailable in Phase A/B**: `rusqlite` is compiled with `bundled` only today. **Phase A and Phase B** use **LIKE** on `trainer_sources`. **Phase C** (see `feature-spec.md`) enables FTS5 via `features = ["bundled-full"]` (or equivalent) plus a dedicated migration for the FTS virtual table — see Phase C tasks there.
- **Trainer versions are not semver**: Display `trainer_version` strings as-is. Never feed to `semver` crate — they contain suffixes like `+DLC`, `Build 12345` that semver rejects.
- **No new import mechanism**: "Import Profile" CTA in the UI calls existing `community_import_profile` IPC. Do not duplicate import logic.
- **Zero new dependencies for Phase A**: All required crates (`reqwest`, `rusqlite`, `serde`, `tokio`, `sha2`) are already in `Cargo.toml`.
- **New table in Phase A**: `trainer_sources` is a new relational table. **`migrate_17_to_18`** creates it with `ON DELETE CASCADE` from `community_taps`.
- **SQLite `user_version` is 17 before trainer-discovery**: Confirmed by the last `pragma_update` guard in `metadata/migrations.rs` (`version < 17` → set `17`). Trainer-discovery adds **18**.

---

## Key Recommendations

- Start with `migrations.rs` + `models.rs` (Rust) and `types/discovery.ts` (TypeScript) — these define the data contract and unblock all parallel work.
- Implement `index_trainer_sources()` before the search query — search is useless without indexed data.
- Write `MetadataStore::open_in_memory()` unit tests for `index_trainer_sources` + `search_trainer_sources` before wiring IPC — catches schema bugs cheaply.
- Build the legal disclaimer dialog early; it is a Phase A ship blocker.
- Register `TrainerDiscoveryPanel` scroll container in `useScrollEnhance.ts` on first UI commit — do not leave it as a follow-up.
- Keep `TrainerDiscoveryPanel` under 800 lines; extract `TrainerResultCard.tsx` as a separate component.
