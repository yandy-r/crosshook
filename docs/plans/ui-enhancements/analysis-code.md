# UI Enhancements — Code Analysis

## Executive Summary

The ProtonDB lookup feature provides a complete, production-tested end-to-end template: Rust `OnceLock` HTTP singleton + cache-first lookup in `crosshook-core`, 13-line thin IPC command in `src-tauri`, and a `useProtonDbLookup` hook with `requestIdRef` race guard driving a `ProtonDbLookupCard` component. The Steam metadata / game cover art feature mirrors this pattern exactly. The Profiles page restructuring is a pure React refactor: remove the single outer `CollapsibleSection("Advanced", defaultOpen=false)` that wraps `ProfileFormSections`, promote groups to individual `CollapsibleSection` cards with `className="crosshook-panel"`, and (Phase 3) add `@radix-ui/react-tabs` sub-tabs using already-defined `.crosshook-subtab-*` CSS.

---

## Existing Code Structure

### Backend — `crosshook-core`

| File                                                                      | Role                                                                                              |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`       | Cache-first API client — the exact template for `steam_metadata/client.rs`                        |
| `src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`       | IPC-safe Serde types — template for Steam metadata types                                          |
| `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`          | `MetadataStore` public API: `with_conn`, `with_sqlite_conn`, `put_cache_entry`, `get_cache_entry` |
| `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`  | `get/put/evict` for `external_cache_entries` with 512 KiB cap                                     |
| `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs` | Store submodule pattern — functions take `&Connection` directly                                   |
| `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`   | Sequential `if version < N` migration — currently at schema v13                                   |

### Tauri Layer

| File                                                      | Role                                                               |
| --------------------------------------------------------- | ------------------------------------------------------------------ |
| `src/crosshook-native/src-tauri/src/commands/protondb.rs` | 13-line IPC command — exact template for new commands              |
| `src/crosshook-native/src-tauri/src/commands/mod.rs`      | Module registry — add `pub mod game_metadata;` here                |
| `src/crosshook-native/src-tauri/src/lib.rs`               | `.manage()` for stores; `invoke_handler!` for command registration |

### Frontend

| File                                                            | Role                                                                      |
| --------------------------------------------------------------- | ------------------------------------------------------------------------- |
| `src/crosshook-native/src/hooks/useProtonDbLookup.ts`           | Canonical hook: `requestIdRef` race guard, state machine, `refresh()`     |
| `src/crosshook-native/src/components/ProtonDbLookupCard.tsx`    | Card component template for `GameMetadataCard`                            |
| `src/crosshook-native/src/components/ui/CollapsibleSection.tsx` | `<details>`-based card primitive with `meta` slot                         |
| `src/crosshook-native/src/components/pages/ProfilesPage.tsx`    | Primary restructuring target — contains single `Advanced` wrapper         |
| `src/crosshook-native/src/components/ProfileFormSections.tsx`   | 41k monolith holding all form fields; `reviewMode` prop must be preserved |
| `src/crosshook-native/src/types/protondb.ts`                    | TypeScript IPC types — template for `steam_metadata` types                |
| `src/crosshook-native/src/context/ProfileContext.tsx`           | Single source of truth for profile state                                  |

---

## Implementation Patterns

### 1. Thin IPC Command Layer

The entire `protondb.rs` command is 13 lines:

```rust
// src-tauri/src/commands/protondb.rs
#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();
    Ok(lookup_protondb(&metadata_store, &app_id, force_refresh.unwrap_or(false)).await)
}
```

New `game_metadata.rs` commands follow this exact structure. The command does zero business logic — it delegates entirely to `crosshook-core`.

### 2. Cache-First with Stale Fallback (`client.rs` lines 85–130)

Flow: check valid cache → fetch live on miss → persist on success → load expired cache on network failure → return `Unavailable` on total failure.

```rust
pub async fn lookup_protondb(metadata_store, app_id, force_refresh) -> ProtonDbLookupResult {
    // 1. Cache hit (skip if force_refresh)
    if !force_refresh {
        if let Some(valid_cache) = load_cached_lookup_row(store, &cache_key, false) {
            return cached_result_from_row(...)
        }
    }
    // 2. Live fetch
    match fetch_live_lookup(&app_id).await {
        Ok(result) => { persist_lookup_result(store, &cache_key, &result); result }
        Err(error) => {
            // 3. Stale fallback
            if let Some(stale) = load_cached_lookup_row(store, &cache_key, true) {
                return cached_result_from_row(..., is_stale=true)
            }
            // 4. Unavailable
            ProtonDbLookupResult { state: Unavailable, ... }
        }
    }
}
```

### 3. `OnceLock` HTTP Client Singleton

```rust
static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn protondb_http_client() -> Result<&'static reqwest::Client, ProtonDbError> {
    if let Some(client) = PROTONDB_HTTP_CLIENT.get() { return Ok(client); }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(ProtonDbError::Network)?;
    let _ = PROTONDB_HTTP_CLIENT.set(client);
    Ok(PROTONDB_HTTP_CLIENT.get().expect("..."))
}
```

Each module (`protondb/`, `steam_metadata/`) owns its own `OnceLock` static.

### 4. Store Submodule Pattern (`health_store.rs`)

Functions in submodules take `&Connection` directly (not `&MetadataStore`). `MetadataStore` delegates via `with_conn`:

```rust
// health_store.rs — functions take &Connection
pub fn upsert_health_snapshot(conn: &Connection, ...) -> Result<(), MetadataStoreError> { ... }

// metadata/mod.rs — MetadataStore delegates
pub fn upsert_health_snapshot(&self, ...) -> Result<(), MetadataStoreError> {
    self.with_conn("upsert a health snapshot", |conn| {
        health_store::upsert_health_snapshot(conn, ...)
    })
}
```

New `game_image_store.rs` follows this same pattern.

### 5. Sequential Migration Pattern (`migrations.rs`)

**Critical**: `if`, not `else if`. Each migration runs independently if the version is below the threshold:

```rust
if version < 13 { migrate_12_to_13(conn)?; pragma_update("user_version", 13); }
if version < 14 { migrate_13_to_14(conn)?; pragma_update("user_version", 14); }
```

New v14 migration adds `game_image_cache` table. Tests use `MetadataStore::open_in_memory()` with `run_migrations(&conn)`.

### 6. Frontend Hook State Machine (`useProtonDbLookup.ts`)

Five states: `idle/loading/ready/stale/unavailable`. The `requestIdRef` race guard prevents stale responses from overwriting newer ones:

```typescript
const requestId = ++requestIdRef.current;
setLoading(true);
const result = await invoke<ProtonDbLookupResult>('protondb_lookup', { appId, forceRefresh });
if (requestId !== requestIdRef.current) return; // discard stale response
setLookup(normalizeLookupResult(result));
```

On appId change, `useEffect` cancels the previous request by incrementing `requestIdRef.current` before the new lookup fires.

### 7. Serde IPC Type Conventions (`models.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProtonDbLookupState { #[default] Idle, Loading, Ready, Stale, Unavailable }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProtonDbLookupResult {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<ProtonDbSnapshot>,
}
```

All IPC structs: `Serialize + Deserialize + Default`. Enums: `rename_all = "snake_case"`. Optional fields: `#[serde(default, skip_serializing_if = "Option::is_none")]`.

### 8. `CollapsibleSection` Card Primitive

```tsx
<CollapsibleSection
  title="Runtime"
  className="crosshook-panel"
  defaultOpen={true}
  meta={<HealthBadge ... />}
>
  {children}
</CollapsibleSection>
```

Props: `title`, `defaultOpen` (bool), controlled `open/onToggle`, `meta` (ReactNode slot), `className`. The root element is `<details>`. Meta slot renders in the `<summary>` next to the title.

### 9. ProfilesPage Current Structure (Restructuring Target)

At `ProfilesPage.tsx:622`, all profile editing content is nested under a single:

```tsx
<CollapsibleSection title="Advanced" defaultOpen={false} meta={...}>
  <ProfileFormSections ... />
  {/* health issues panel */}
  {/* export/launcher controls */}
</CollapsibleSection>
```

The restructuring removes this outer wrapper and promotes sections (Identity, Game Path, Runtime, Environment/ProtonDB, Trainer) to individual `CollapsibleSection` cards.

### 10. `reviewMode` Contract in `ProfileFormSections`

`ProfileFormSections` has a `reviewMode?: boolean` prop that collapses empty optional fields and disables some edit controls. This prop is consumed by `InstallPage.tsx`. Any structural extraction must preserve `reviewMode` behavior:

- `trainerCollapsed = reviewMode && profile.trainer.path.trim().length === 0`
- `workingDirectoryCollapsed = reviewMode && profile.runtime.working_directory.trim().length === 0`
- `onApplyEnvVars` is disabled when `reviewMode` is true

---

## Integration Points

### Files to Create

| File                                              | Purpose                                                                    |
| ------------------------------------------------- | -------------------------------------------------------------------------- |
| `crosshook-core/src/steam_metadata/client.rs`     | Cache-first metadata fetch — mirrors `protondb/client.rs`                  |
| `crosshook-core/src/steam_metadata/models.rs`     | `SteamMetadataResult`, `SteamMetadataState`, `SteamAppDetails` Serde types |
| `crosshook-core/src/steam_metadata/mod.rs`        | Module entry point; re-exports public API                                  |
| `crosshook-core/src/metadata/game_image_store.rs` | `get/put` for `game_image_cache` table — mirrors `health_store.rs`         |
| `src-tauri/src/commands/game_metadata.rs`         | Thin IPC commands: `game_metadata_lookup`, `game_cover_art_path`           |
| `src/hooks/useGameMetadata.ts`                    | Clones `useProtonDbLookup` state machine for Steam metadata                |
| `src/types/steam_metadata.ts`                     | TypeScript types mirroring Rust Serde output                               |
| `src/components/GameMetadataCard.tsx`             | Cover art + metadata display — mirrors `ProtonDbLookupCard`                |
| `src/components/GameCoverArt.tsx`                 | `convertFileSrc` + shimmer skeleton image component                        |

### Files to Modify

| File                                        | Change                                                                                           |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `crosshook-core/src/metadata/migrations.rs` | Add `if version < 14` block for `game_image_cache` table                                         |
| `crosshook-core/src/metadata/mod.rs`        | Add `game_image_store` submodule; expose `get/put_game_image_cache`                              |
| `crosshook-core/Cargo.toml`                 | Add `infer ~0.16` dependency                                                                     |
| `src-tauri/src/commands/mod.rs`             | Add `pub mod game_metadata;`                                                                     |
| `src-tauri/src/lib.rs`                      | Register new commands in `invoke_handler!`                                                       |
| `src-tauri/tauri.conf.json`                 | Add `img-src 'self' asset: http://asset.localhost` to CSP                                        |
| `src-tauri/capabilities/default.json`       | Add `fs:allow-read-file` scoped to `$LOCALDATA/crosshook/cache/images/**` + asset protocol scope |
| `src/components/pages/ProfilesPage.tsx`     | Remove outer `CollapsibleSection("Advanced")` wrapper; add individual section cards              |
| `src/components/ProfileFormSections.tsx`    | Extract section groups as separate components (Phase 2+)                                         |
| `src/styles/theme.css`                      | Add cover art, shimmer skeleton, and new section card CSS                                        |

---

## Code Conventions

### Rust

- Module layout: `steam_metadata/mod.rs`, `steam_metadata/client.rs`, `steam_metadata/models.rs` (mirrors `protondb/`)
- Error type: private `enum SteamMetadataError` with `fmt::Display`, mirrors `ProtonDbError`
- Cache key format: `steam:appdetails:v1:{app_id}` (namespaced, version-tagged)
- Image path construction: `~/.local/share/crosshook/cache/images/{steam_app_id}/cover.{ext}`
- All `MetadataStore` delegates use `with_conn` (graceful degradation) not `with_sqlite_conn` (strict)
- `tracing::warn!` for non-fatal errors; `tracing::info!` for significant state changes

### TypeScript / React

- Hook file: `useGameMetadata.ts` — export `UseGameMetadataResult` interface + `useGameMetadata(appId)` function
- State values lowercase snake_case matching Rust serde output (`'idle' | 'loading' | 'ready' | 'stale' | 'unavailable'`)
- Normalize all optional fields from IPC with `?? null` / `?? []` / `?? ''` in a `normalizeResult` function
- Components: `PascalCase` filename; `crosshook-` BEM class prefix; `className?` prop for external overrides
- CSS display state: use `display: none` (not conditional render) to preserve state across tab switches — critical for `CustomEnvironmentVariablesSection`

### CSS

- New section card classes: `crosshook-game-cover-art`, `crosshook-game-metadata-bar`, `.crosshook-cover-art__skeleton`
- Shimmer animation defined in `theme.css` using `@keyframes crosshook-shimmer`
- Sub-tab classes already exist: `.crosshook-subtab-row`, `.crosshook-subtab`, `.crosshook-subtab--active`
- CSS variables for sub-tabs already defined in `variables.css`: `--crosshook-subtab-min-height`, `--crosshook-subtab-padding-inline`

---

## Dependencies and Services

### Rust Crates (existing unless noted)

- `reqwest` — HTTP client (already in `Cargo.toml`)
- `rusqlite` — SQLite (already in `Cargo.toml`)
- `serde` / `serde_json` — IPC serialization (already present)
- `chrono` — timestamp / TTL handling (already present)
- `tracing` — structured logging (already present)
- `infer ~0.16` — **NEW**: magic-byte MIME detection for image validation; SVG rejection

### Frontend (existing unless noted)

- `@tauri-apps/api/core` — `invoke()` for IPC calls
- `@tauri-apps/api/tauri` — `convertFileSrc()` for asset protocol URLs
- `@radix-ui/react-tabs` — **NEW (Phase 3)**: sub-tab navigation

---

## Gotchas and Warnings

1. **`reviewMode` prop is a shared contract** — `ProfileFormSections` is used by both `ProfilesPage` and `InstallPage` with `reviewMode={true}`. Any section extraction must thread `reviewMode` through to maintain collapse behavior on `InstallPage`.

2. **`CustomEnvironmentVariablesSection` holds local `rows` state** — if this component is conditionally rendered (unmounted) during tab switches, the user loses unsaved env var state. Must use `display: none` CSS, not `{condition && <Component />}`.

3. **`ProfileActions` must stay outside any collapsible** — the Save/Delete/Duplicate/Rename bar in `ProfileActions.tsx` must not be nested inside a `CollapsibleSection` or tab panel. It should remain at the top level of the profile editor.

4. **`profileSelector` prop is not optional on all paths** — `ProfileFormSections` uses a union type: `ProfileFormSectionsBaseProps & { profileSelector: ... } | ProfileFormSectionsBaseProps & { profileSelector?: undefined }`. The `profileSelector` field drives the profile selector dropdown. Do not remove it.

5. **Cache key namespace collision risk** — ProtonDB uses `protondb:{app_id}` as cache key. Steam metadata must use `steam:appdetails:v1:{app_id}` — the `v1` version tag allows future format changes without manual cache eviction.

6. **Migration ordering** — migrations run as sequential `if version < N` blocks, not `else if`. If a new migration is added at the wrong position in the file, it will silently skip. Always append at the end and verify `user_version` increment is correct.

7. **Image validation with `infer`** — the `infer` crate inspects magic bytes, not file extensions. SVGs must be explicitly rejected because `infer` may not detect SVG (it is text-based). The `research-security.md` document has a ready-made `validate_image_bytes` snippet.

8. **Path traversal in image cache** — `steam_app_id` from IPC is untrusted. Use `safe_image_cache_path` from `research-security.md` to canonicalize before writing. Never interpolate `app_id` directly into a filesystem path.

9. **`OnceLock` initialization** — `OnceLock::set` may return `Err` if called concurrently. The pattern `let _ = PROTONDB_HTTP_CLIENT.set(client);` discards the duplicate and then calls `.get().expect(...)`. Do not unwrap `.set()`.

10. **`MetadataStore::disabled()` graceful degradation** — `with_conn` returns `T::default()` when the store is unavailable (SQLite failed to open). New code using `MetadataStore` for image cache must tolerate a `None` result from lookups. Never use `with_sqlite_conn` for code paths that should degrade gracefully.

11. **`profileSelector` in `ProfileFormSections` vs. top-level selector in `ProfilesPage`** — `ProfilesPage` already has a top-level profile dropdown at line ~600. The `profileSelector` prop in `ProfileFormSections` renders a second selector inside the form body. Both are currently inside the `Advanced` wrapper. During restructuring, the top-level selector stays always-visible; the form-body selector can remain in the Identity section card.

12. **`ui/ProtonPathField.tsx` imports from `ProfileFormSections.tsx`** — `src/components/ui/ProtonPathField.tsx` imports `formatProtonInstallLabel` and `ProtonInstallOption` directly from `../ProfileFormSections`. This is a layering violation (`ui/` depending on a page-level component). `formatProtonInstallLabel` and `ProtonInstallOption` must be extracted to `src/utils/proton.ts` (or similar) in **Phase 0** before any section splitting of `ProfileFormSections` can occur — otherwise splitting will create a circular import.

13. **`injection.*` fields in `GameProfile` are intentionally absent from the form UI** — the `injection` section of `GameProfile` is never rendered in `ProfileFormSections` or any section card. Do not accidentally expose these fields during restructuring. If encountered in the profile data type, skip them.

---

## Task-Specific Guidance

### Phase 0 — CSS / Sub-tab infrastructure

No new files needed. Verify `.crosshook-subtab-row`, `.crosshook-subtab`, `.crosshook-subtab--active` exist in `theme.css` and that `--crosshook-subtab-min-height` is in `variables.css` before Phase 3.

### Phase 1 — ProfilesPage restructuring

**Minimal surgery**: in `ProfilesPage.tsx` at line 622, replace the single `<CollapsibleSection title="Advanced" defaultOpen={false}>` wrapper with individual section cards. The inner `ProfileFormSections` component and its `reviewMode` contract are unchanged in Phase 1. Only the outer wrapper is removed.

### Phase 2 — `ProfileFormSections` section extraction

Each section (`Identity`, `Game Path`, `Runtime`, `Environment/ProtonDB`, `Trainer`) becomes its own component extracted from the monolith. The `reviewMode`, `onUpdateProfile`, `profile`, and `launchMethod` props must be threaded into each extracted component. `CustomEnvironmentVariablesSection` must never be conditionally unmounted.

### Phase 3 — Sub-tabs

Use `@radix-ui/react-tabs` for nested tab navigation within the Advanced card. Tab state should be `useState` (not URL-based). CSS visibility via `display: none` on inactive tab panels to preserve component state.

### Phase 4 — Steam metadata and cover art

1. Create `crosshook-core/src/steam_metadata/` mirroring `protondb/` layout.
2. Add `game_image_cache` migration at v14 in `migrations.rs`.
3. Add `game_metadata.rs` command module in `src-tauri/src/commands/`.
4. Register command in `lib.rs` `invoke_handler!`.
5. Create `useGameMetadata.ts` cloning `useProtonDbLookup.ts` state machine.
6. Create `GameMetadataCard.tsx` and `GameCoverArt.tsx`.
7. Update `tauri.conf.json` CSP and `capabilities/default.json` for asset protocol.
8. Add `GameMetadataCard` to the `Environment/ProtonDB` section card alongside `ProtonDbLookupCard`.
