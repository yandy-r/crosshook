# Plan: Profile Collections — Phase 3 (Per-Collection Launch Defaults)

## Summary

Deliver the "behavior" leg of profile collections: a collection carries its own `LaunchSection` subset (`method`, `custom_env_vars`, `optimizations`, `gamescope`, `trainer_gamescope`, `mangohud`, `network_isolation`) and those overrides merge into the profile at load time **when the profile is loaded inside a collection context**. Introduces a new `CollectionDefaultsSection` serde type, a new `effective_profile_with(&CollectionDefaultsSection)` merge layer on `GameProfile` (precedence: base → collection defaults → local_override), schema v19 → v20 adding a nullable `defaults_json TEXT` column on `collections`, 2 new IPC commands (`collection_get_defaults`, `collection_set_defaults`), an extension of `profile_load` to accept optional `collection_id`, browser dev-mode mocks for the 2 new commands + the extended `profile_load`, a new inline `<CollectionLaunchDefaultsEditor>` section inside the existing `<CollectionViewModal>` (Phase 2), a "Open in Profiles page →" link-out, and the frontend plumbing that passes `activeCollectionId` into `loadProfile` from LaunchPage call sites only.

Parallel-safe with Phase 2 (already in progress on `#178`). Integrates via the `activeCollectionId` field Phase 2 plumbs through `ProfileContext`.

## User Story

As a **power user who organizes their 80-profile library into collections like "Steam Deck (Docked)" and "Steam Deck (Handheld)"**, I want my **collection to carry its own launch defaults (env vars, gamescope resolution, optimizations)** so that **when I launch a profile from inside a collection, those overrides apply automatically without editing the base profile** — and my non-collection launches of the same profile are completely unaffected.

## Problem → Solution

**Current state (after Phase 2)**: Collections are a visual/organizational layer — they group profiles, give them a sidebar home, and let the user filter the Active-Profile dropdown. They carry no behavior. A "Steam Deck Docked" collection and a "Steam Deck Handheld" collection both launch the same profile the same way. The only existing merge layer on `GameProfile` is `local_override` (`models.rs:486-545`), which applies at `effective_profile()` call time. The `effective_profile()` function has **no parameters** and **no concept of a collection context**. `profile_load` (`commands/profile.rs:230-233`) returns the raw storage profile.

**Desired state (after Phase 3)**: Each collection persists an optional `CollectionDefaultsSection` (a subset of `LaunchSection`) as inline JSON in a new `collections.defaults_json TEXT` column. A new `effective_profile_with(&self, defaults: Option<&CollectionDefaultsSection>)` method extends the existing merge function with a NEW middle layer (`base → collection defaults → local_override`). The existing `effective_profile(&self)` becomes a thin shim that forwards `None`, so **zero existing call sites need to change**. `profile_load` accepts an optional `collection_id`; when present, it fetches the collection's defaults from the metadata store and returns the merged profile. A new inline editor inside `<CollectionViewModal>` lets users edit the simple fields, with a "Open in Profiles page →" link-out for fields the PRD explicitly kept out of inline editing.

## Metadata

- **Complexity**: **Medium** (≈14 files touched, ≈700 lines of source + ≈400 lines of tests, zero new dependencies, follows Phase 1 patterns exactly for the Rust layer and Phase 2 patterns for the TS layer)
- **Source PRD**: [`docs/prps/prds/profile-collections.prd.md`](../prds/profile-collections.prd.md)
- **PRD Phase**: **Phase 3 — Per-collection launch defaults**
- **Source Issue**: [`yandy-r/crosshook#179`](https://github.com/yandy-r/crosshook/issues/179)
- **Depends on**: Phase 1 (`#177`, merged) for the collections IPC foundation, `CollectionRow`, schema v19, and the `[dev-mock]` mock layer. **Parallel-safe with Phase 2 (`#178`, in-progress)** — Phase 3 consumes `activeCollectionId` which Phase 2 adds to `ProfileContext`.
- **Estimated Files**: 14 (2 CREATE, 12 UPDATE)
- **Schema target**: **v19 → v20** (v19 landed in Phase 1 via `migrate_18_to_19`)

## Storage / Persistence

Phase 3 adds one persisted datum and one ephemeral datum. Both are classified below per CLAUDE.md storage-boundary rules.

| Datum                                        | Classification                             | Where it lives                                                                                                                                                       | Migration / compat                                                                                                                                                                                 |
| -------------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Per-collection launch defaults**           | Operational metadata (SQLite)              | `collections.defaults_json TEXT NULL` — new column added in v19→v20. Body is a JSON-serialized `CollectionDefaultsSection` (serde_json), or `NULL` when no defaults. | **v19 → v20**: `ALTER TABLE collections ADD COLUMN defaults_json TEXT`. Nullable; existing rows backfill to `NULL`. Additive, non-destructive. Downgrade not supported per repo policy.            |
| `collection_id` passed to `profile_load`     | Ephemeral (IPC call-site argument)         | No persistence                                                                                                                                                       | N/A — reset every call                                                                                                                                                                             |
| `activeCollectionId` reading from context    | Ephemeral runtime (memory; set by Phase 2) | React state in `ProfileContext`                                                                                                                                      | **Phase 2 delivers this.** Phase 3 only CONSUMES it at specific call sites (the LaunchPage profile-selector → loadProfile bridge). `ProfilesPage` never reads it for loading — see Task 11 GOTCHA. |
| Merged profile (`effective_profile_with`)    | Runtime-only (computed; never persisted)   | In-memory only                                                                                                                                                       | N/A                                                                                                                                                                                                |
| Browser dev-mode mock defaults fixture state | Ephemeral runtime (memory; dev-only)       | `MockCollectionDefaults` map in `src/lib/mocks/handlers/collections.ts`                                                                                              | Resets on page reload                                                                                                                                                                              |

**Storage decision — inline JSON TEXT column, not a new table.** Rationale:

- Defaults are always looked up 1-to-1 with a collection, never joined, aggregated, or queried per-field. A `collection_launch_defaults` table would add a needless FK and join without query benefit.
- `bundled_optimization_presets.option_ids_json` (`migrations.rs:446-451`) sets a precedent for storing a serde-serialized Rust structure in a nullable JSON TEXT column. Read pattern at `commands/profile.rs:200-210`.
- `CollectionDefaultsSection` is inherently nested (contains nested `GamescopeConfig`, `MangoHudConfig`, `BTreeMap`), and a normalized schema would require ≥4 sub-tables. JSON serialization is strictly simpler.
- A future sibling table (`collection_env_var_overrides`, `collection_optimization_overrides`) can still be added per the PRD's "generic Collection<T>" decision if Phase 4+ needs per-field querying — the inline column does not block this.

**Offline behavior**: 100% local. No network. Read/write via existing metadata SQLite path.

**Degraded / fallback behavior**:

- If `MetadataStore` is disabled, `get_collection_defaults` returns `Ok(None)` via the `with_conn` `T: Default` constraint (where `T = Option<CollectionDefaultsSection>` and `Option<_>::default() = None`). `set_collection_defaults` silently succeeds as a no-op — matches existing Phase 1 wrapper behavior (`rename_collection`, etc.).
- If `defaults_json` is corrupt JSON, `get_collection_defaults` returns `Err(MetadataStoreError::Corrupt("corrupt collection defaults JSON for <id>"))`. The Tauri command surfaces this as a friendly error string to the frontend; the frontend falls back to "no defaults" (the merge layer degrades to a no-op) and shows a toast. **No data loss**: the raw JSON remains on disk and the user can clear it by saving new defaults, which overwrites the column.
- If `profile_load` is called with `collection_id = Some("nonexistent")`, the command fetches defaults, finds `None`, applies an empty merge layer (which is a no-op because no fields are `Some`), and returns the raw storage profile — indistinguishable from passing `collection_id = None`. This is intentional graceful degradation per the issue body ("defaults read fails → fallback to base profile + local_override").

**User visibility / editability**:

- The PRD-chosen inline-editable subset (`method`, `custom_env_vars`, `optimizations`, `gamescope`, `trainer_gamescope`, `mangohud`, `network_isolation`) is fully editable inside `<CollectionViewModal>` via the new `<CollectionLaunchDefaultsEditor>` section. Users see the current defaults, can set/clear them, and save.
- All other `LaunchSection` fields (notably `presets`, `active_preset`) are **out of scope** per the PRD decision "LaunchSection only, with redirect to specific pages for more advanced work". The inline editor surfaces an "Open in Profiles page →" link to the Profiles page with the collection filter active.
- Per-collection defaults are **not directly file-editable** in v1 (no TOML export mechanism exists — that ships in Phase 4). The only v1 escape hatch for editing is the inline modal editor.

---

## UX Design

### Before (Phase 2)

```
+---------------------------------------------+
| Collection view modal                       |
|   Header: Action / Adventure [Rename] [Del] |
|   Search: [____________]                    |
|   ----------------------------------------- |
|   Elden Ring [Launch] [Edit] [Remove]       |
|   Cyberpunk  [Launch] [Edit] [Remove]       |
|   Sekiro     [Launch] [Edit] [Remove]       |
+---------------------------------------------+
| Launch Elden Ring from this collection      |
|   → uses the profile's base LaunchSection   |
|   → no collection-level overrides applied   |
+---------------------------------------------+
```

### After (Phase 3)

```
+---------------------------------------------------+
| Collection view modal                             |
|   Header: Action / Adventure [Rename] [Delete]    |
|                                                    |
|   [▼ Collection launch defaults] [Reset all]      |
|   ┌─────────────────────────────────────────────┐ |
|   │ Method:       (inherit) / native / proton  │ |
|   │ Optimizations:[ ] gamemoderun [x] mangohud │ |
|   │ Gamescope:    [ ] enabled  1920×1080       │ |
|   │ MangoHUD:     [x] enabled                  │ |
|   │ Network iso:  (inherit) / on / off         │ |
|   │ Env vars:     KEY    │ VALUE      │ [+]    │ |
|   │               DXVK_HUD│1           │ [×]    │ |
|   │               PROTON_LOG│1         │ [×]    │ |
|   │                                             │ |
|   │ [Open in Profiles page →] [Save] [Cancel]  │ |
|   └─────────────────────────────────────────────┘ |
|   ----------------------------------------------- |
|   Search: [____________]                          |
|   Elden Ring [Launch] [Edit] [Remove]             |
|   Cyberpunk  [Launch] [Edit] [Remove]             |
|   Sekiro     [Launch] [Edit] [Remove]             |
+---------------------------------------------------+
| Launch Elden Ring from this collection            |
|   → loadProfile('elden-ring', {collectionId})     |
|   → Rust: effective_profile_with(Some(defaults))  |
|   → collection env vars + gamescope merged in     |
|   → profile's own local_override still wins       |
+---------------------------------------------------+
```

### Interaction Changes

| Touchpoint                                                           | Before                                                                                       | After                                                                                                                                                                                                                                                                               | Notes                                                                                                                                                             |
| -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CollectionViewModal` (Phase 2)                                      | Header + search + member list                                                                | Same + collapsible **Collection launch defaults** section (collapsed by default on first open)                                                                                                                                                                                      | Add as a `<details>` element above the search block so the default state is non-distracting.                                                                      |
| Launching a profile from LaunchPage when `activeCollectionId` is set | `invoke('profile_load', { name })` → raw profile → buildProfileLaunchRequest → `launch_game` | `invoke('profile_load', { name, collectionId })` → Rust merges `effective_profile_with(Some(&defaults))` → returns merged profile → buildProfileLaunchRequest reads merged fields (env vars, gamescope, etc.) naturally → `launch_game` receives LaunchRequest reflecting overrides | The merge is transparent to `buildProfileLaunchRequest` — it just reads `profileState.profile.launch.custom_env_vars` as today, which is already the merged view. |
| Launching the same profile from the Library page (no collection)     | Same as before                                                                               | **Unchanged** — Library page passes `collectionId: undefined`, backend returns raw storage profile                                                                                                                                                                                  | Editor integrity: `ProfilesPage` never passes `collectionId`, so user-visible editable state is always the storage profile.                                       |
| `invoke('profile_load', { name })` (no collectionId)                 | Returns raw storage profile                                                                  | **Unchanged** — identical behavior                                                                                                                                                                                                                                                  | Backward-compat guarantee. Phase 3 is strictly additive at the IPC boundary.                                                                                      |
| `invoke('collection_get_defaults', { collectionId })`                | Command does not exist                                                                       | Returns `CollectionDefaultsSection \| null`                                                                                                                                                                                                                                         | New command                                                                                                                                                       |
| `invoke('collection_set_defaults', { collectionId, defaults })`      | Command does not exist                                                                       | Writes defaults to `defaults_json` column; `null` clears the column                                                                                                                                                                                                                 | New command                                                                                                                                                       |
| `./scripts/dev-native.sh --browser` + collection defaults call       | `[dev-mock] Unhandled command: collection_get_defaults`                                      | Returns mocked data                                                                                                                                                                                                                                                                 | Per `[dev-mock]` sentinel (CLAUDE.md)                                                                                                                             |

---

## Mandatory Reading

Read these files **before** starting. The plan references file:line throughout — do not skip context-loading.

| Priority | File                                                                        | Lines                                         | Why                                                                                                                                                                                                                                                                                    |
| -------- | --------------------------------------------------------------------------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **P0**   | `docs/prps/prds/profile-collections.prd.md`                                 | all                                           | Source PRD. Phase 3 section, Decisions Log ("Override scope per collection", "Merge layer placement", "Precedence order"), Persistence section                                                                                                                                         |
| **P0**   | `docs/prps/archived/profile-collections-phase-1-backend-foundation.plan.md` | all                                           | Phase 1 plan (archived) — set the pattern conventions (Validation-tuple errors, table-rebuild migrations, mock handler shape, `[dev-mock]` prefix, `with_conn` usage, command registration). Phase 3 **mirrors these exactly**.                                                        |
| **P0**   | `docs/prps/plans/profile-collections-phase-2-sidebar-view-modal.plan.md`    | Task 6, Tasks 8–9                             | Phase 2's `activeCollectionId` plumbing into `ProfileContext` and `<CollectionViewModal>` structure. Phase 3 EXTENDS the modal.                                                                                                                                                        |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`          | 323–397, 486–585, 755–824, 1150–1160          | `LaunchSection` full struct, `effective_profile()` full body, existing tests to mirror, `normalize_preset_selection`                                                                                                                                                                   |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs`    | 1–280                                         | Phase 1's free-function conventions and insertion point. `list_collections`/`rename_collection` shapes to mirror for `get_collection_defaults`/`set_collection_defaults`                                                                                                               |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`     | 165–181, 216–236, 822–852, 1124–1206          | `run_migrations` dispatch to extend, `ALTER TABLE ADD COLUMN` precedent (`migrate_1_to_2`), `migrate_18_to_19` pattern, migration test pattern                                                                                                                                         |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`            | 98–115, 452–526, 1492–1544, 2541–2720         | `with_conn` + `T: Default`, Phase 1 collection wrappers, `sample_profile`/`connection`, existing collection tests                                                                                                                                                                      |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`         | 1–60, 294–303                                 | `MetadataStoreError` variants (`Validation(String)` tuple; new `Corrupt(String)` usage pattern), `CollectionRow` (no more `#[allow(dead_code)]`)                                                                                                                                       |
| **P0**   | `src/crosshook-native/src-tauri/src/commands/profile.rs`                    | 225–280                                       | `profile_load` current signature + `profile_list_summaries` `effective_profile()` call site                                                                                                                                                                                            |
| **P0**   | `src/crosshook-native/src-tauri/src/commands/collections.rs`                | 1–96                                          | All 9 Phase 1 command handlers — the shape Phase 3 mirrors                                                                                                                                                                                                                             |
| **P0**   | `src/crosshook-native/src-tauri/src/lib.rs`                                 | 195–295                                       | `tauri::Builder::manage(metadata_store)` (line 201) and the `tauri::generate_handler!` registration block lines 281–290. New Phase 3 commands insert after line 289.                                                                                                                   |
| **P0**   | `src/crosshook-native/src/lib/mocks/handlers/collections.ts`                | all (Phase 1)                                 | The mock handler shape. Phase 3 adds to the same file: 2 new handlers + a `mockDefaults` store.                                                                                                                                                                                        |
| **P0**   | `src/crosshook-native/src/lib/mocks/handlers/profile.ts`                    | 180–210                                       | Current `profile_load` mock — Phase 3 extends this to accept `collectionId` and return merged data                                                                                                                                                                                     |
| **P0**   | `src/crosshook-native/src/hooks/useProfile.ts`                              | 549–598                                       | `loadProfile` function — Phase 3 extends its options and the IPC args                                                                                                                                                                                                                  |
| **P0**   | `src/crosshook-native/src/context/ProfileContext.tsx`                       | all                                           | Phase 2 added `activeCollectionId: string \| null` + `setActiveCollectionId` here. Phase 3 reads it.                                                                                                                                                                                   |
| **P0**   | `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`   | all                                           | Phase 2 modal — Phase 3 adds the inline defaults editor section to its body                                                                                                                                                                                                            |
| **P0**   | `src/crosshook-native/src/components/pages/LaunchPage.tsx`                  | 24–90                                         | Where `activeCollectionId` is already read from context; Phase 3 adds the call-site that passes it to `loadProfile`                                                                                                                                                                    |
| **P1**   | `src/crosshook-native/src/hooks/useCollectionMembers.ts`                    | all                                           | Pattern for a new `useCollectionDefaults(collectionId)` hook (`requestSeqRef`, caching, `set state` on success)                                                                                                                                                                        |
| **P1**   | `src/crosshook-native/src/types/profile.ts`                                 | all                                           | Where `CollectionDefaults` TS type goes                                                                                                                                                                                                                                                |
| **P1**   | `src/crosshook-native/src/lib/mocks/wrapHandler.ts`                         | 38–60                                         | `EXPLICIT_READ_COMMANDS` — new `collection_get_defaults` must be added here explicitly (its name begins with `collection_` not `get_`, so the READ_VERB_RE doesn't match)                                                                                                              |
| **P2**   | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`     | 440–460 (`bundled_optimization_presets`)      | Precedent for a nullable JSON TEXT column in a collection-scoped table — closest we have to Phase 3's new `defaults_json` column                                                                                                                                                       |
| **P2**   | `src/crosshook-native/src-tauri/src/commands/profile.rs`                    | 200–218 (`bundled_optimization_presets` read) | Precedent for `serde_json::from_str(&row.option_ids_json)` read pattern for JSON TEXT columns                                                                                                                                                                                          |
| **P2**   | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                        | 1–20                                          | `SCROLLABLE` selector — the new inline defaults editor sits inside `.crosshook-modal__body` which is already enhanced, so **no new selector needed unless** the inline editor itself introduces a nested `overflow-y: auto` (e.g. a long env-var list) — see Task 12 GOTCHA            |
| **P2**   | `src/crosshook-native/src/App.tsx`                                          | 105–125                                       | Existing deep-link pattern (`selectProfile(name)` + `setRoute('profiles')`) — Phase 3's "Open in Profiles page →" link uses the same pattern with one addition: `setActiveCollectionId(collectionId)` is already set at the modal level, so the link only needs `setRoute('profiles')` |

## External Documentation

**No external research needed** — Phase 3 uses only well-understood internal patterns:

- Rust serde + `skip_serializing_if` for optional fields (existing in `LaunchSection`)
- `serde_json::to_string`/`from_str` (already used in `commands/profile.rs:200-210`)
- SQLite `ALTER TABLE ... ADD COLUMN ... TEXT` (existing pattern in `migrate_1_to_2`)
- React `useState`/`useCallback`/`useEffect` + existing `callCommand<T>` wrapper

Zero new dependencies. Zero third-party integrations.

---

## Patterns to Mirror

All snippets below are **verbatim from the codebase**. Follow them exactly — the plan is single-pass implementation-ready only if the new code is indistinguishable in style from the existing code.

### SERDE_TYPE — `LaunchSection` subset pattern

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models.rs:323-397
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchSection {
    #[serde(default)]
    pub method: String,
    #[serde(default, skip_serializing_if = "LaunchOptimizationsSection::is_empty")]
    pub optimizations: LaunchOptimizationsSection,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub presets: BTreeMap<String, LaunchOptimizationsSection>,
    #[serde(rename = "active_preset", default, skip_serializing_if = "String::is_empty")]
    pub active_preset: String,
    #[serde(rename = "custom_env_vars", default, skip_serializing_if = "BTreeMap::is_empty")]
    pub custom_env_vars: BTreeMap<String, String>,
    #[serde(default = "default_network_isolation", skip_serializing_if = "is_default_network_isolation")]
    pub network_isolation: bool,
    #[serde(default, skip_serializing_if = "GamescopeConfig::is_default")]
    pub gamescope: GamescopeConfig,
    #[serde(default, skip_serializing_if = "GamescopeConfig::is_default")]
    pub trainer_gamescope: GamescopeConfig,
    #[serde(default, skip_serializing_if = "MangoHudConfig::is_default")]
    pub mangohud: MangoHudConfig,
}

impl Default for LaunchSection {
    fn default() -> Self {
        Self {
            method: String::new(),
            optimizations: LaunchOptimizationsSection::default(),
            presets: BTreeMap::new(),
            active_preset: String::new(),
            custom_env_vars: BTreeMap::new(),
            network_isolation: true,   // ← MANUAL default, not derived
            gamescope: GamescopeConfig::default(),
            trainer_gamescope: GamescopeConfig::default(),
            mangohud: MangoHudConfig::default(),
        }
    }
}
```

**Conventions for `CollectionDefaultsSection` (the new type)**:

- Derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default` — because every field is `Option<T>` or an empty `BTreeMap`, `Default` can be derived (no manual impl needed, unlike `LaunchSection`).
- Every scalar field is `Option<T>` with `#[serde(default, skip_serializing_if = "Option::is_none")]` — "None means inherit from profile; Some replaces."
- `custom_env_vars` is a flat `BTreeMap<String, String>` with `#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]` — **additive merge**, not replacement (see merge semantics in `EFFECTIVE_PROFILE_MERGE` below).

### EFFECTIVE_PROFILE_MERGE — current implementation

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models.rs:486-545
pub fn effective_profile(&self) -> Self {
    let mut merged = self.clone();

    if !self.local_override.game.executable_path.trim().is_empty() {
        merged.game.executable_path = self.local_override.game.executable_path.clone();
    }
    // ... similar trim-and-replace for ~10 other path fields ...
    if !self.local_override.trainer.extra_protontricks.is_empty() {
        merged
            .trainer
            .required_protontricks
            .extend(self.local_override.trainer.extra_protontricks.clone());
    }
    // ... more path merges ...

    merged
}
```

**Phase 3 extension strategy**:

- Rename the existing method to `effective_profile_with(&self, defaults: Option<&CollectionDefaultsSection>) -> Self`.
- Apply collection defaults **after the `let mut merged = self.clone()` line but BEFORE any `local_override` merges**. This honors the PRD precedence: base → collection defaults → local_override.
- Add a thin backward-compat shim: `pub fn effective_profile(&self) -> Self { self.effective_profile_with(None) }`.
- This keeps all **13 existing non-test call sites** of `effective_profile()` working with **zero diff**. Only the tests that exercise Phase 3's new merge layer need new code.

### METADATA_JSON_COLUMN — read pattern

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/profile.rs:200-210
let enabled_option_ids: Vec<String> = serde_json::from_str(&row.option_ids_json)
    .map_err(|e| format!("corrupt bundled preset {} option list: {e}", row.preset_id))?;
```

**Write pattern** (to mirror in `set_collection_defaults`):

```rust
let json = serde_json::to_string(defaults).map_err(|e| MetadataStoreError::Database {
    action: "serialize collection defaults to JSON",
    source: /* see TASK 4 — we wrap the JSON error into the closest existing variant */
})?;
```

**Corrupt-read handling** uses `MetadataStoreError::Corrupt(String)` (declared at `models.rs:1-60`).

### MIGRATION_ADD_COLUMN_TEXT — simple nullable column

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:216-227
fn migrate_1_to_2(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "ALTER TABLE profiles ADD COLUMN source TEXT;
         UPDATE profiles SET source = 'initial_census' WHERE source IS NULL;",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 1 to 2",
        source,
    })?;

    Ok(())
}
```

**`migrate_19_to_20` is almost trivial** — `defaults_json TEXT` is nullable and defaults to `NULL` (no backfill needed):

```rust
fn migrate_19_to_20(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch("ALTER TABLE collections ADD COLUMN defaults_json TEXT;")
        .map_err(|source| MetadataStoreError::Database {
            action: "run metadata migration 19 to 20",
            source,
        })?;
    Ok(())
}
```

### MIGRATION_DISPATCH — append new version check

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:174-181 (current latest)
if version < 19 {
    migrate_18_to_19(conn)?;
    conn.pragma_update(None, "user_version", 19_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "set user_version to 19",
            source,
        })?;
}
```

New block appended **after** line 181:

```rust
if version < 20 {
    migrate_19_to_20(conn)?;
    conn.pragma_update(None, "user_version", 20_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "set user_version to 20",
            source,
        })?;
}
```

### REPOSITORY_PATTERN — free function in `collections.rs`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs:215-244
// (rename_collection; pattern to mirror for set_collection_defaults)
pub fn rename_collection(
    conn: &Connection,
    collection_id: &str,
    new_name: &str,
) -> Result<(), MetadataStoreError> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(MetadataStoreError::Validation(
            "collection name must not be empty".to_string(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let affected = conn
        .execute(
            "UPDATE collections SET name = ?1, updated_at = ?2 WHERE collection_id = ?3",
            params![trimmed, now, collection_id],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "rename a collection",
            source,
        })?;

    if affected == 0 {
        return Err(MetadataStoreError::Validation(format!(
            "collection not found: {collection_id}"
        )));
    }

    Ok(())
}
```

**Conventions to mirror**:

- `pub fn name(conn: &Connection, ...) -> Result<T, MetadataStoreError>`
- Validate inputs, return `Validation(String)` **tuple** variant on bad input
- `updated_at` refreshed to `Utc::now().to_rfc3339()`
- `if affected == 0 { return Err(Validation(format!("collection not found: {id}"))); }` for "collection missing" semantics
- `action` strings are infinitive verbs ("rename a collection", "read collection defaults", "write collection defaults")

### REPOSITORY_WRAPPER — `MetadataStore::*` in `mod.rs`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:499-526 (Phase 1)
pub fn rename_collection(
    &self,
    collection_id: &str,
    new_name: &str,
) -> Result<(), MetadataStoreError> {
    self.with_conn("rename a collection", |conn| {
        collections::rename_collection(conn, collection_id, new_name)
    })
}
```

**`with_conn` constraint**: the closure return type `T` must implement `Default`. For `get_collection_defaults` returning `Option<CollectionDefaultsSection>`, this is automatically satisfied because `Option<_>::default() = None`. For `set_collection_defaults` returning `()`, `() : Default` is also fine. The disabled-store path returns `Ok(None)` / `Ok(())` automatically.

### SERVICE_PATTERN — Tauri command handler

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/collections.rs:65-74
#[tauri::command]
pub fn collection_rename(
    collection_id: String,
    new_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .rename_collection(&collection_id, &new_name)
        .map_err(map_error)
}
```

**Conventions**:

- `snake_case` command names (CLAUDE.md MUST rule)
- `Result<T, String>` return, `.map_err(map_error)` tail (`map_error = |e: impl ToString| e.to_string()` already defined at `commands/collections.rs:4`)
- Positional args come **before** `State<'_, MetadataStore>` — Tauri requires `State` last
- No `#[tauri::command(rename_all = "camelCase")]` attribute — rest of file uses default snake_case

### COMMAND_REGISTRATION — insertion point

```rust
// SOURCE: src/crosshook-native/src-tauri/src/lib.rs:281-290 (Phase 1, updated)
commands::collections::collection_list,
commands::collections::collection_create,
commands::collections::collection_delete,
commands::collections::collection_add_profile,
commands::collections::collection_remove_profile,
commands::collections::collection_list_profiles,
commands::collections::collection_rename,
commands::collections::collection_update_description,
commands::collections::collections_for_profile,
// ← INSERT Phase 3 commands HERE
commands::profile::profile_set_favorite,
```

### MOCK_HANDLER_PATTERN — `registerCollections`

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/collections.ts (Phase 1)
export function registerCollections(map: Map<string, Handler>): void {
  map.set('collection_rename', async (args): Promise<null> => {
    const { collectionId, newName } = args as {
      collectionId: string;
      newName: string;
    };
    const trimmed = (newName ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] collection_rename: collection name must not be empty');
    }
    const target = findById(collectionId);
    if (!target) {
      throw new Error(`[dev-mock] collection_rename: collection not found: ${collectionId}`);
    }
    target.name = trimmed;
    target.updated_at = nowIso();
    return null;
  });
}
```

**Conventions**:

- **Every thrown error string MUST start with `[dev-mock]`** — the `.github/workflows/release.yml:105-120` `verify:no-mocks` sentinel greps for this literal prefix to ensure no mock code leaked into production bundles.
- Cast `args` with `as { ... }` — never `any` (CLAUDE.md type safety rule).
- Return `null` (not `undefined`, not `void`) from mutators to match the Tauri `Result<(), String>` → `null` JSON serialization.
- Module-scope mutable state (e.g., `let mockDefaults: Map<string, MockCollectionDefaults>`) resets on page reload.

### TEST_STRUCTURE — metadata test for collections

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:2541-2720 (Phase 1)
#[test]
fn test_rename_collection_updates_name() {
    let store = MetadataStore::open_in_memory().unwrap();
    let id = store.create_collection("Old Name").unwrap();

    store.rename_collection(&id, "New Name").unwrap();

    let collections = store.list_collections().unwrap();
    assert_eq!(collections.len(), 1);
    assert_eq!(collections[0].name, "New Name");
}
```

**Conventions**:

- `MetadataStore::open_in_memory().unwrap()` — never temp files
- Test names: `test_<verb>_<subject>[_<edge>]` snake_case
- Direct SQL assertions via `connection(&store)` (returns a `MutexGuard`; call `drop(conn)` before re-locking for a second query)
- `sample_profile()` factory at `mod.rs:1492` for profile fixtures

### TEST_STRUCTURE — `effective_profile` merge test

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models.rs:757-768
#[test]
fn effective_profile_prefers_local_override_paths() {
    let mut profile = sample_profile();
    profile.game.executable_path = "/portable/game.exe".to_string();
    profile.local_override.game.executable_path = "/local/game.exe".to_string();
    profile.runtime.proton_path = "/portable/proton".to_string();
    profile.local_override.runtime.proton_path = "/local/proton".to_string();

    let effective = profile.effective_profile();
    assert_eq!(effective.game.executable_path, "/local/game.exe");
    assert_eq!(effective.runtime.proton_path, "/local/proton");
}
```

**Phase 3 adds tests in the same style in the same module**, exercising the new `effective_profile_with(Some(&defaults))` merge + the precedence invariant (collection defaults override profile base; local_override overrides collection defaults).

### FRONTEND_HOOK_PATTERN — `useCollectionMembers` style

```ts
// SOURCE: src/crosshook-native/src/hooks/useCollectionMembers.ts (Phase 2)
export function useCollectionMembers(collectionId: string | null) {
  const [memberNames, setMemberNames] = useState<string[]>([]);
  const [membersForCollectionId, setMembersForCollectionId] = useState<string | null>(null);
  const [membersLoading, setMembersLoading] = useState<boolean>(false);
  const requestSeqRef = useRef(0);

  useEffect(() => {
    const seq = ++requestSeqRef.current;
    if (collectionId === null) {
      setMemberNames([]);
      setMembersForCollectionId(null);
      return;
    }
    setMembersLoading(true);
    callCommand<string[]>('collection_list_profiles', { collectionId })
      .then((names) => {
        if (seq !== requestSeqRef.current) return;
        setMemberNames(names);
        setMembersForCollectionId(collectionId);
      })
      .finally(() => {
        if (seq !== requestSeqRef.current) return;
        setMembersLoading(false);
      });
  }, [collectionId]);

  return { memberNames, membersForCollectionId, membersLoading };
}
```

**Phase 3's `useCollectionDefaults(collectionId)` mirrors this**: `requestSeqRef` for race-safe fetches, `loading` flag, `defaults` state keyed by `collectionId`.

---

## Files to Change

| #   | File                                                                                            | Action | Justification                                                                                                                                                                          |
| --- | ----------------------------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`                              | UPDATE | Add `CollectionDefaultsSection` type + helpers; extend `effective_profile` into `effective_profile_with` + shim; add new tests                                                         |
| 2   | `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs` (if needed)                     | UPDATE | Re-export `CollectionDefaultsSection` alongside `LaunchSection` / `GameProfile`                                                                                                        |
| 3   | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`                         | UPDATE | Add `migrate_19_to_20` + dispatch; add migration test                                                                                                                                  |
| 4   | `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs`                        | UPDATE | Add `get_collection_defaults`, `set_collection_defaults` free functions                                                                                                                |
| 5   | `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`                                | UPDATE | Add `MetadataStore::get_collection_defaults` + `set_collection_defaults` wrappers; add tests                                                                                           |
| 6   | `src/crosshook-native/src-tauri/src/commands/collections.rs`                                    | UPDATE | Add `collection_get_defaults`, `collection_set_defaults` Tauri commands                                                                                                                |
| 7   | `src/crosshook-native/src-tauri/src/commands/profile.rs`                                        | UPDATE | Extend `profile_load` to accept `collection_id: Option<String>`; fetch defaults + return merged profile when present                                                                   |
| 8   | `src/crosshook-native/src-tauri/src/lib.rs`                                                     | UPDATE | Register 2 new commands in `tauri::generate_handler!`                                                                                                                                  |
| 9   | `src/crosshook-native/src/lib/mocks/handlers/collections.ts`                                    | UPDATE | Add `collection_get_defaults` / `collection_set_defaults` mocks + `mockDefaults` store                                                                                                 |
| 10  | `src/crosshook-native/src/lib/mocks/handlers/profile.ts`                                        | UPDATE | Extend `profile_load` mock to accept `collectionId` and merge mock defaults locally (so browser dev-mode doesn't drift from real Rust behavior)                                        |
| 11  | `src/crosshook-native/src/lib/mocks/wrapHandler.ts`                                             | UPDATE | Add `collection_get_defaults` to `EXPLICIT_READ_COMMANDS` (name doesn't match the `get_`-prefix regex)                                                                                 |
| 12  | `src/crosshook-native/src/types/profile.ts` (or a new `collections.ts`)                         | UPDATE | Add `CollectionDefaults` TypeScript interface mirroring the Rust serde type                                                                                                            |
| 13  | `src/crosshook-native/src/hooks/useProfile.ts`                                                  | UPDATE | Extend `loadProfile(name, loadOptions?)` to accept `collectionId?: string` and pass it to `invoke('profile_load', ...)`                                                                |
| 14  | `src/crosshook-native/src/hooks/useCollectionDefaults.ts`                                       | CREATE | New hook wrapping `collection_get_defaults` / `collection_set_defaults` — mirrors `useCollectionMembers`                                                                               |
| 15  | `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`                       | UPDATE | Add inline `<CollectionLaunchDefaultsEditor>` section; pass `onOpenInProfilesPage` callback; thread collection through                                                                 |
| 16  | `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx`            | CREATE | New component: the inline editor UI — simple fields + env var table + Save/Reset/Open in Profiles page                                                                                 |
| 17  | `src/crosshook-native/src/components/pages/LaunchPage.tsx`                                      | UPDATE | At the Active-Profile dropdown's onChange, call `loadProfile(name, { collectionId: activeCollectionId ?? undefined })` so collection defaults flow in                                  |
| 18  | `src/crosshook-native/src/App.tsx`                                                              | UPDATE | Wire the `onOpenInProfilesPage` callback from `<CollectionViewModal>` — calls `setRoute('profiles')`; `activeCollectionId` is already set from Phase 2                                 |
| 19  | `src/crosshook-native/src/styles/components/collection-launch-defaults-editor.css` (or similar) | CREATE | BEM-like `crosshook-collection-launch-defaults-editor__*` classes for the new editor; also register any new scroll container in `useScrollEnhance.ts:9` if applicable (Task 12 Gotcha) |

**Count**: 19 files — 2 CREATE, 17 UPDATE. Slightly over the Medium-complexity threshold but each file has a tightly scoped change. Could be reduced to Medium if the CSS file is inlined into an existing stylesheet.

## NOT Building

- **Per-collection override of `LaunchSection.presets` / `active_preset`** — explicitly excluded. Preset coupling is too complex for v1 (see `LaunchSection::normalize_preset_selection` at `models.rs:385-397` for why). The PRD's "link-out to Profiles page" covers this: users who want preset overrides go through the profile-level editor.
- **Per-collection override of non-`LaunchSection` sections** (`game`, `trainer`, `steam`, `runtime`, `injection`) — PRD decision: "LaunchSection only with redirect to specific pages for more advanced work". Excluded from v1.
- **A separate `collection_launch_defaults` SQLite table** — rejected in favor of inline `defaults_json TEXT` column per Storage decision. Future work can introduce it if per-field querying becomes necessary without a destructive migration.
- **A new `profile_load_effective` Tauri command** — rejected in favor of extending `profile_load`. The issue body explicitly chooses "extend `profile_load` to avoid IPC duplication".
- **Changing existing `effective_profile()` call sites** — the 13 non-test call sites (`commands/launch.rs:165`, `commands/profile.rs:254`, `toml_store.rs:161`, etc.) stay untouched. The shim pattern `fn effective_profile(&self) -> Self { self.effective_profile_with(None) }` makes this change zero-ripple.
- **Merging collection defaults at save time** — the merge is load-time only. When the user saves a profile from the editor, the storage TOML contains only the profile's own fields (never the merged view). Enforced by ProfilesPage never passing `collectionId` to `loadProfile`.
- **Auto-reload of the profile when `activeCollectionId` changes** — Phase 3 does NOT add a `useEffect` on `activeCollectionId` that triggers `loadProfile`. The merge only applies on the next explicit load. This keeps the behavior predictable and avoids surprising re-loads mid-edit.
- **TOML export/import of per-collection defaults** — Phase 4 work. Phase 3 only persists defaults locally.
- **Edit-history / undo for defaults changes** — not in scope; save overwrites.
- **Multi-collection defaults stacking** (a profile in multiple collections; which collection's defaults win?) — out of scope. A profile is launched from AT MOST one active collection context. The PRD is clear: `activeCollectionId` is scalar, not a set.
- **Soft-delete of collection defaults** — hard overwrite; `null` clears the column.
- **Validation of env-var names / values** beyond trim and non-empty key — the editor accepts any string; runtime validation is the launch layer's responsibility (as today).
- **A generic `<SectionEditor>` primitive** — the inline editor reuses existing form primitives (`ThemedSelect`, `<input>`, etc.) directly. Extracting a shared primitive is a future polish.
- **Changing `CollectionRow` to derive `Deserialize`** — only needed if Phase 3 wants to deserialize a `CollectionRow` from JSON on the backend, which we do NOT do. The new defaults column is separate from `CollectionRow` and is fetched via its own query path.

---

## Step-by-Step Tasks

### Task 1: Add `CollectionDefaultsSection` serde type in `profile/models.rs`

- **ACTION**: Define the new struct + unit helpers next to `LaunchSection` in `models.rs`.
- **IMPLEMENT**: Insert immediately after the `impl LaunchSection { ... }` block at `models.rs:397`:

  ```rust
  /// Collection-scoped overrides for the `LaunchSection` overrideable subset.
  /// Each `Option<T>` field means "inherit from profile when None, replace when Some".
  /// `custom_env_vars` is an **additive merge**: collection entries are union'd with the
  /// profile's `launch.custom_env_vars` and collection keys win on collision.
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
  pub struct CollectionDefaultsSection {
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub method: Option<String>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub optimizations: Option<LaunchOptimizationsSection>,
      #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
      pub custom_env_vars: BTreeMap<String, String>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub network_isolation: Option<bool>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub gamescope: Option<GamescopeConfig>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub trainer_gamescope: Option<GamescopeConfig>,
      #[serde(default, skip_serializing_if = "Option::is_none")]
      pub mangohud: Option<MangoHudConfig>,
  }

  impl CollectionDefaultsSection {
      /// Returns true when no field would influence a profile merge.
      pub fn is_empty(&self) -> bool {
          self.method.is_none()
              && self.optimizations.is_none()
              && self.custom_env_vars.is_empty()
              && self.network_isolation.is_none()
              && self.gamescope.is_none()
              && self.trainer_gamescope.is_none()
              && self.mangohud.is_none()
      }
  }
  ```

- **MIRROR**: `LaunchSection` (`models.rs:323-397`) for serde attributes; `LocalOverrideSection::is_empty` (`models.rs:411-418`) for `is_empty` shape.
- **IMPORTS**: none new — `Serialize`, `Deserialize`, `BTreeMap`, `LaunchOptimizationsSection`, `GamescopeConfig`, `MangoHudConfig` are already in scope.
- **GOTCHA**:
  - **Excluded fields**: `presets` and `active_preset` are deliberately OUT of the subset. `LaunchSection::normalize_preset_selection` (`models.rs:385-397`) couples `active_preset` to `optimizations` and would require complex re-normalization at merge time. The PRD's "link-out to Profiles page" covers users who want preset-level collection overrides.
  - **`network_isolation` as `Option<bool>`**: the profile-level default is `true` (via manual `impl Default for LaunchSection`). `Option<bool>` gives us three states: `None` = inherit profile (true or whatever the profile says), `Some(true)` = explicitly force on, `Some(false)` = explicitly force off. The `skip_serializing_if` ensures empty defaults don't bloat the JSON blob.
  - **`Default` can be derived** — unlike `LaunchSection`, every field of `CollectionDefaultsSection` is `Option<T>` or an empty `BTreeMap`, so `#[derive(Default)]` yields the correct "no overrides" value. Do **not** write a manual `impl Default`.
  - **Do not derive `Hash`** — `BTreeMap<String, String>` makes this impossible without custom impl and we don't need it.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` succeeds with zero warnings. The `is_empty()` helper is immediately used by Task 2's merge function.

### Task 2: Extend `effective_profile` → `effective_profile_with` + backward-compat shim

- **ACTION**: Rename the existing method to `effective_profile_with(&self, defaults: Option<&CollectionDefaultsSection>) -> Self`, apply collection defaults as a NEW middle layer between base and `local_override`, and re-introduce `effective_profile(&self) -> Self` as a thin shim.
- **IMPLEMENT**: In `models.rs:486-545`, replace the body with:

  ```rust
  /// Returns the effective profile used at runtime.
  ///
  /// Precedence (lowest → highest):
  ///   1. Base profile (`self`)
  ///   2. Collection defaults (if provided) — per-collection overrides from `CollectionDefaultsSection`
  ///   3. `local_override.*` — machine-specific paths always win last
  pub fn effective_profile_with(
      &self,
      defaults: Option<&CollectionDefaultsSection>,
  ) -> Self {
      let mut merged = self.clone();

      // ── Layer 2: collection defaults ────────────────────────────────────
      if let Some(d) = defaults {
          if let Some(ref method) = d.method {
              if !method.trim().is_empty() {
                  merged.launch.method = method.clone();
              }
          }
          if let Some(ref opts) = d.optimizations {
              merged.launch.optimizations = opts.clone();
          }
          if !d.custom_env_vars.is_empty() {
              // Additive merge — collection keys win on collision, profile
              // keys without a collision are preserved.
              for (k, v) in &d.custom_env_vars {
                  merged.launch.custom_env_vars.insert(k.clone(), v.clone());
              }
          }
          if let Some(ni) = d.network_isolation {
              merged.launch.network_isolation = ni;
          }
          if let Some(ref gs) = d.gamescope {
              merged.launch.gamescope = gs.clone();
          }
          if let Some(ref tgs) = d.trainer_gamescope {
              merged.launch.trainer_gamescope = tgs.clone();
          }
          if let Some(ref mh) = d.mangohud {
              merged.launch.mangohud = mh.clone();
          }
      }

      // ── Layer 3: local_override (unchanged) ─────────────────────────────
      if !self.local_override.game.executable_path.trim().is_empty() {
          merged.game.executable_path = self.local_override.game.executable_path.clone();
      }
      // ... KEEP ALL EXISTING local_override MERGE LINES VERBATIM ...
      // (lines 492-541 of the current body — do NOT modify any of them)

      merged
  }

  /// Backward-compat shim: call sites that don't know about collection
  /// defaults get the base profile merged with `local_override` only.
  pub fn effective_profile(&self) -> Self {
      self.effective_profile_with(None)
  }
  ```

  **Critical**: lines 492-541 of the current function body (all the `local_override` trim-and-replace assignments + the `extra_protontricks` `.extend()` block) must be **moved verbatim** into the new body, _after_ the collection-defaults block. Do not rewrite or refactor them as part of this task — a pure extraction keeps the diff reviewable.

- **MIRROR**: The existing `effective_profile` body (`models.rs:486-545`) is the template for the `local_override` layer. The new collection-defaults layer mirrors the `if !field.trim().is_empty() { merged.X = ... }` idiom with `Option`-awareness added.
- **IMPORTS**: `CollectionDefaultsSection` — already in scope (defined in Task 1 in the same file).
- **GOTCHA**:
  - **PRECEDENCE IS LOAD-BEARING**. `base → collection defaults → local_override` is the PRD decision. If an implementer accidentally swaps layers 2 and 3 (collection defaults AFTER local_override), the existing test `effective_profile_prefers_local_override_paths` at `models.rs:758` will FAIL (collection defaults would stomp `/local/game.exe`) — the test acts as a regression guard. Run it frequently.
  - **`method: Option<String>` is handled with an extra empty-trim check**: `if method.trim().is_empty() { /* skip */ }`. This matches the PRD's "None means inherit; Some replaces, but an empty string doesn't accidentally clobber a profile's method".
  - **`custom_env_vars` is ADDITIVE, not REPLACING**. Do not `merged.launch.custom_env_vars = d.custom_env_vars.clone();` — that would wipe the profile's own env vars. Use `for (k, v) in ... { insert(...) }` so the profile's env vars are preserved and the collection's keys win only on collision.
  - **Do NOT touch `storage_profile()` or `portable_profile()` (`models.rs:547-584`)** — they call `effective_profile()` internally, which now goes through the shim and passes `None`. They MUST NOT bake collection defaults into the storage profile, because collection defaults are a launch-time runtime concept and would corrupt the profile TOML if materialized at save time.
  - **The 13 non-test call sites of `effective_profile()` continue to work unchanged** because the shim exists. Do NOT try to migrate any of them to `effective_profile_with` as part of this task — that is out of scope and not needed for correctness.
- **VALIDATE**:
  - `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` succeeds
  - Existing test `effective_profile_prefers_local_override_paths` still passes (precedence invariant)
  - Existing test `storage_profile_roundtrip_is_idempotent` still passes (shim behavior)

### Task 3: Add unit tests for `effective_profile_with`

- **ACTION**: Add four tests in `models.rs` under the existing `mod tests` block (near line 758 where the other `effective_profile_*` tests live).
- **IMPLEMENT**: Append after `effective_profile_merges_portrait_and_background_from_local_override` (~`models.rs:1160`):

  ```rust
  #[test]
  fn effective_profile_with_none_equals_shim() {
      let mut profile = sample_profile();
      profile.launch.custom_env_vars.insert("DXVK_HUD".into(), "1".into());
      profile.local_override.game.executable_path = "/local/game.exe".into();

      let via_shim = profile.effective_profile();
      let via_with_none = profile.effective_profile_with(None);
      assert_eq!(via_shim, via_with_none, "shim must equal explicit None");
  }

  #[test]
  fn effective_profile_with_merges_collection_defaults_between_base_and_local_override() {
      let mut profile = sample_profile();
      // Base profile: empty env vars, default gamescope, portable paths
      profile.launch.custom_env_vars.insert("PROFILE_ONLY".into(), "A".into());
      profile.game.executable_path = "/portable/game.exe".into();
      profile.local_override.game.executable_path = "/local/game.exe".into();

      let mut defaults = CollectionDefaultsSection::default();
      defaults.custom_env_vars.insert("COLLECTION_ONLY".into(), "B".into());
      defaults.custom_env_vars.insert("PROFILE_ONLY".into(), "OVERRIDDEN".into());
      defaults.network_isolation = Some(false);
      defaults.method = Some("proton_run".into());

      let merged = profile.effective_profile_with(Some(&defaults));

      // ── Layer 3 (local_override) still wins last ──
      assert_eq!(merged.game.executable_path, "/local/game.exe");

      // ── Layer 2 (collection defaults) applies ──
      assert_eq!(merged.launch.method, "proton_run");
      assert_eq!(merged.launch.network_isolation, false);
      assert_eq!(
          merged.launch.custom_env_vars.get("COLLECTION_ONLY").cloned(),
          Some("B".into())
      );
      // ── Collection key wins on collision ──
      assert_eq!(
          merged.launch.custom_env_vars.get("PROFILE_ONLY").cloned(),
          Some("OVERRIDDEN".into())
      );
  }

  #[test]
  fn effective_profile_with_none_fields_do_not_overwrite_profile() {
      let mut profile = sample_profile();
      profile.launch.method = "native".into();
      profile.launch.network_isolation = true;
      profile.launch.gamescope = GamescopeConfig::default();
      profile.launch.custom_env_vars.insert("PROFILE_KEY".into(), "retained".into());

      // Empty defaults: every Option is None, BTreeMap is empty → no-op merge
      let defaults = CollectionDefaultsSection::default();
      assert!(defaults.is_empty());
      let merged = profile.effective_profile_with(Some(&defaults));

      assert_eq!(merged.launch.method, "native");
      assert_eq!(merged.launch.network_isolation, true);
      assert_eq!(
          merged.launch.custom_env_vars.get("PROFILE_KEY").cloned(),
          Some("retained".into())
      );
      // ── Profile env vars never dropped ──
      assert_eq!(merged.launch.custom_env_vars.len(), 1);
  }

  #[test]
  fn effective_profile_with_ignores_whitespace_only_method() {
      let mut profile = sample_profile();
      profile.launch.method = "native".into();

      let mut defaults = CollectionDefaultsSection::default();
      defaults.method = Some("   ".into()); // whitespace-only should NOT clobber profile

      let merged = profile.effective_profile_with(Some(&defaults));
      assert_eq!(merged.launch.method, "native", "whitespace method must not clobber profile");
  }
  ```

- **MIRROR**: `effective_profile_prefers_local_override_paths` (`models.rs:757-768`) — same setup + assertion shape.
- **IMPORTS**: `CollectionDefaultsSection` (same file), `GamescopeConfig` (already in scope), `sample_profile()` fixture (same module).
- **GOTCHA**:
  - **`sample_profile()` exists in this module** at the test module level. Reuse it — do NOT construct a `GameProfile` literal.
  - **`assert_eq!` on `CollectionDefaultsSection` requires `PartialEq`** — which Task 1 derives. If the derive is missed, this test won't compile; that's the feedback signal.
  - **Do not test `custom_env_vars` iteration order** — `BTreeMap` is ordered, but tests that rely on iteration order are fragile. Assert per-key via `.get(...)`.
  - **The `effective_profile_with_none_equals_shim` test is critical** — it guarantees the backward-compat shim is behaviorally identical. If this test fails, some callsite of `effective_profile()` may have subtly changed behavior.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile::models::tests::effective_profile_with` passes all four tests.

### Task 4: Add schema migration 19 → 20

- **ACTION**: Add `migrate_19_to_20` function + dispatch block in `metadata/migrations.rs`.
- **IMPLEMENT**:
  1. Insert new dispatch block in `run_migrations` **after** the `if version < 19` block (around `migrations.rs:181`):

     ```rust
     if version < 20 {
         migrate_19_to_20(conn)?;
         conn.pragma_update(None, "user_version", 20_u32)
             .map_err(|source| MetadataStoreError::Database {
                 action: "set user_version to 20",
                 source,
             })?;
     }
     ```

  2. Append the `migrate_19_to_20` function **after** `migrate_18_to_19` (around `migrations.rs:852`):

     ```rust
     fn migrate_19_to_20(conn: &Connection) -> Result<(), MetadataStoreError> {
         conn.execute_batch(
             "ALTER TABLE collections ADD COLUMN defaults_json TEXT;",
         )
         .map_err(|source| MetadataStoreError::Database {
             action: "run metadata migration 19 to 20",
             source,
         })?;
         Ok(())
     }
     ```

- **MIRROR**: `migrate_1_to_2` (`migrations.rs:216-236`) for the simple single-ALTER pattern; dispatch shape from `migrations.rs:174-181`.
- **IMPORTS**: none — `Connection`, `MetadataStoreError` already in scope.
- **GOTCHA**:
  - **Nullable `TEXT` column — no `NOT NULL`, no `DEFAULT`** — existing rows automatically get `NULL` on `ALTER TABLE ADD COLUMN`. If you accidentally add `NOT NULL DEFAULT '{}'`, existing rows break because SQLite can't deserialize `{}` into `CollectionDefaultsSection` without additional logic.
  - **No transaction needed for a single `ALTER TABLE`** — rusqlite wraps `execute_batch` in its own DDL handling. Unlike `migrate_18_to_19` (which had multiple DDL statements and needed `BEGIN/COMMIT`), the single `ALTER` here is atomic.
  - **Do NOT modify `migrate_18_to_19`** or any prior migration — SQLite migrations are immutable once released.
  - **PRD says "schema v20 (or higher)"** — the "or higher" is only if Phase 2 adds an intermediate migration. As of writing, Phase 2's in-progress work does NOT add a migration (it's frontend-only), so Phase 3 targets exactly v20. **Verify this assumption before starting**: `grep -n "migrate_.*_to_.*(conn: &Connection)" src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs | tail` should show `migrate_18_to_19` as the latest. If Phase 2 has added anything, shift numbers accordingly.
- **VALIDATE**:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core migrations` passes
  - Task 5's migration test `migration_19_to_20_adds_defaults_json_column` passes
  - Fresh install reports `user_version = 20`

### Task 5: Add migration test for 19 → 20

- **ACTION**: Add an inline test in `migrations.rs` inside `mod tests` verifying the column exists, is nullable, and round-trips a JSON value.
- **IMPLEMENT**: Append to `migrations.rs` inside the `mod tests` block (after `migration_18_to_19_adds_sort_order_and_cascade`):

  ```rust
  #[test]
  fn migration_19_to_20_adds_defaults_json_column() {
      let conn = db::open_in_memory().unwrap();
      run_migrations(&conn).unwrap();

      let version: u32 = conn
          .pragma_query_value(None, "user_version", |row| row.get(0))
          .unwrap();
      assert_eq!(version, 20);

      // Verify the defaults_json column exists, is TEXT, and is nullable.
      let mut stmt = conn.prepare("PRAGMA table_info(collections)").unwrap();
      let columns: Vec<(String, String, i64)> = stmt
          .query_map([], |row| {
              Ok((
                  row.get::<_, String>(1)?, // name
                  row.get::<_, String>(2)?, // type
                  row.get::<_, i64>(3)?,    // notnull
              ))
          })
          .unwrap()
          .collect::<Result<Vec<_>, _>>()
          .unwrap();
      let defaults_json = columns
          .iter()
          .find(|(name, _, _)| name == "defaults_json")
          .expect("defaults_json column should exist");
      assert_eq!(defaults_json.1, "TEXT");
      assert_eq!(defaults_json.2, 0, "defaults_json should be nullable");

      // Round-trip a JSON payload.
      conn.execute(
          "INSERT INTO collections (collection_id, name, created_at, updated_at, defaults_json)
           VALUES ('col-1', 'Test', datetime('now'), datetime('now'), ?1)",
          [r#"{"method":"proton_run"}"#],
      )
      .unwrap();
      let payload: Option<String> = conn
          .query_row(
              "SELECT defaults_json FROM collections WHERE collection_id = 'col-1'",
              [],
              |row| row.get(0),
          )
          .unwrap();
      assert_eq!(payload.as_deref(), Some(r#"{"method":"proton_run"}"#));

      // NULL round-trip.
      conn.execute(
          "INSERT INTO collections (collection_id, name, created_at, updated_at)
           VALUES ('col-2', 'Test2', datetime('now'), datetime('now'))",
          [],
      )
      .unwrap();
      let empty: Option<String> = conn
          .query_row(
              "SELECT defaults_json FROM collections WHERE collection_id = 'col-2'",
              [],
              |row| row.get(0),
          )
          .unwrap();
      assert_eq!(empty, None, "defaults_json should default to NULL");
  }
  ```

- **MIRROR**: `migration_18_to_19_adds_sort_order_and_cascade` (`migrations.rs:1124-1206`) — same style: `PRAGMA table_info`, verify version, exercise a round-trip.
- **IMPORTS**: none — `db`, `run_migrations`, `Connection` already in scope.
- **GOTCHA**:
  - **`notnull` column in `PRAGMA table_info` is 0 for nullable, 1 for NOT NULL** — the assertion `assert_eq!(defaults_json.2, 0, ...)` checks nullability.
  - **The round-trip asserts literal JSON string** — we don't `serde_json::from_str` here because this test is DDL-level, not Rust-level. The serde round-trip is tested in the metadata store tests (Task 7).
  - **`user_version` must equal 20** — if Phase 2 added an intermediate migration, this test fails loudly. Adjust the version number accordingly (and re-number the migration function).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core migration_19_to_20_adds_defaults_json_column` passes.

### Task 6: Add `get_collection_defaults` + `set_collection_defaults` free functions

- **ACTION**: Add two new free functions in `metadata/collections.rs` for read/write of `defaults_json`.
- **IMPLEMENT**: Append after `collections_for_profile` (around `collections.rs:280`):

  ```rust
  use crate::profile::models::CollectionDefaultsSection;

  pub fn get_collection_defaults(
      conn: &Connection,
      collection_id: &str,
  ) -> Result<Option<CollectionDefaultsSection>, MetadataStoreError> {
      let json: Option<String> = conn
          .query_row(
              "SELECT defaults_json FROM collections WHERE collection_id = ?1",
              params![collection_id],
              |row| row.get(0),
          )
          .map_err(|source| match source {
              rusqlite::Error::QueryReturnedNoRows => MetadataStoreError::Database {
                  action: "read collection defaults",
                  source,
              },
              other => MetadataStoreError::Database {
                  action: "read collection defaults",
                  source: other,
              },
          })?;

      let Some(json) = json else {
          return Ok(None);
      };
      if json.trim().is_empty() {
          return Ok(None);
      }

      let parsed: CollectionDefaultsSection =
          serde_json::from_str(&json).map_err(|e| {
              MetadataStoreError::Corrupt(format!(
                  "corrupt collection defaults JSON for {collection_id}: {e}"
              ))
          })?;
      Ok(Some(parsed))
  }

  pub fn set_collection_defaults(
      conn: &Connection,
      collection_id: &str,
      defaults: Option<&CollectionDefaultsSection>,
  ) -> Result<(), MetadataStoreError> {
      // Serialize or clear. `None` or an empty defaults → NULL column.
      let json: Option<String> = match defaults {
          Some(d) if !d.is_empty() => Some(serde_json::to_string(d).map_err(|e| {
              MetadataStoreError::Corrupt(format!(
                  "failed to serialize collection defaults for {collection_id}: {e}"
              ))
          })?),
          _ => None,
      };

      let now = Utc::now().to_rfc3339();
      let affected = conn
          .execute(
              "UPDATE collections SET defaults_json = ?1, updated_at = ?2 WHERE collection_id = ?3",
              params![json, now, collection_id],
          )
          .map_err(|source| MetadataStoreError::Database {
              action: "write collection defaults",
              source,
          })?;

      if affected == 0 {
          return Err(MetadataStoreError::Validation(format!(
              "collection not found: {collection_id}"
          )));
      }

      Ok(())
  }
  ```

- **MIRROR**: `rename_collection` (`collections.rs:215-244`) for the UPDATE + affected-row pattern; `commands/profile.rs:200-210` for the `serde_json::from_str` read pattern.
- **IMPORTS**: Add `use crate::profile::models::CollectionDefaultsSection;` at the top of `collections.rs`. `serde_json` needs to be a dependency of `crosshook-core` — **verify** via `grep serde_json src/crosshook-native/crates/crosshook-core/Cargo.toml`. If it isn't listed, add it to `[dependencies]` (it's very likely already present; used elsewhere in the crate).
- **GOTCHA**:
  - **`query_row` returns `rusqlite::Error::QueryReturnedNoRows` when the collection_id doesn't match any row** — this is different from "defaults_json IS NULL" (which returns a row with NULL). We want the former to surface as an error ("collection not found"), the latter to return `Ok(None)`. The match arm above wraps `QueryReturnedNoRows` into a `Database` error; the frontend will see "no such collection" as an error string, matching `rename_collection` semantics.
    - **Alternative simpler design**: use `query_row_and_then` or do a two-step (`SELECT EXISTS ...` first). The current design uses `query_row` which returns `QueryReturnedNoRows` for missing collection — the Tauri layer will forward this. If the existing Phase 1 get-like function handles this differently, mirror it. **Verify with Phase 1's `list_profiles_in_collection`** which has the same "collection missing" concern.
  - **`is_empty()` from Task 1 is used** to short-circuit empty-defaults writes into a NULL column write — so saving "all fields cleared" writes NULL, not `{}`.
  - **`MetadataStoreError::Corrupt(String)` is a tuple variant** — same shape as `Validation(String)`. **Do not** use struct syntax.
  - **`serde_json::to_string`** returns `Result<String, serde_json::Error>`. We wrap the error into `Corrupt` because a serialization failure here indicates a bug (the struct should always round-trip). The user doesn't see this unless they do something weird.
  - **`updated_at` bump on write**: even though `defaults_json` is the only column changing, refreshing `updated_at` matches the `rename_collection` / `update_collection_description` pattern and gives sidebar/UI layers a cache-invalidation signal.
  - **Do NOT add a read path that bypasses the metadata store** — all access MUST go through `MetadataStore::with_conn`.
- **VALIDATE**: `cargo build --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` succeeds.

### Task 7: Add `MetadataStore` wrappers + integration tests

- **ACTION**: Add `get_collection_defaults` / `set_collection_defaults` wrappers to `MetadataStore` in `mod.rs`, plus integration tests.
- **IMPLEMENT**:
  1. Add the wrappers inside the `Phase 3: Collections` section of `impl MetadataStore { ... }` (after `collections_for_profile` at `mod.rs:526`):

     ```rust
     pub fn get_collection_defaults(
         &self,
         collection_id: &str,
     ) -> Result<Option<CollectionDefaultsSection>, MetadataStoreError> {
         self.with_conn("read collection defaults", |conn| {
             collections::get_collection_defaults(conn, collection_id)
         })
     }

     pub fn set_collection_defaults(
         &self,
         collection_id: &str,
         defaults: Option<&CollectionDefaultsSection>,
     ) -> Result<(), MetadataStoreError> {
         self.with_conn("write collection defaults", |conn| {
             collections::set_collection_defaults(conn, collection_id, defaults)
         })
     }
     ```

  2. Add `use crate::profile::models::CollectionDefaultsSection;` near the other imports at the top of `mod.rs` if it isn't already in scope from `crate::profile::*`.

  3. Append tests in `mod tests` (after Phase 1's Collection tests, around `mod.rs:2720`):

     ```rust
     #[test]
     fn test_collection_defaults_set_and_get_roundtrip() {
         let store = MetadataStore::open_in_memory().unwrap();
         let id = store.create_collection("Steam Deck").unwrap();

         // Initially, no defaults.
         let none = store.get_collection_defaults(&id).unwrap();
         assert!(none.is_none());

         let mut defaults = CollectionDefaultsSection::default();
         defaults.method = Some("proton_run".into());
         defaults.custom_env_vars.insert("DXVK_HUD".into(), "1".into());
         defaults.network_isolation = Some(false);

         store.set_collection_defaults(&id, Some(&defaults)).unwrap();

         let loaded = store.get_collection_defaults(&id).unwrap();
         let loaded = loaded.expect("defaults should be set");
         assert_eq!(loaded.method.as_deref(), Some("proton_run"));
         assert_eq!(loaded.network_isolation, Some(false));
         assert_eq!(
             loaded.custom_env_vars.get("DXVK_HUD").cloned(),
             Some("1".into())
         );
     }

     #[test]
     fn test_collection_defaults_clear_writes_null() {
         let store = MetadataStore::open_in_memory().unwrap();
         let id = store.create_collection("Temp").unwrap();

         let mut defaults = CollectionDefaultsSection::default();
         defaults.method = Some("native".into());
         store.set_collection_defaults(&id, Some(&defaults)).unwrap();

         // Clearing via None writes NULL.
         store.set_collection_defaults(&id, None).unwrap();
         let loaded = store.get_collection_defaults(&id).unwrap();
         assert!(loaded.is_none());

         // Clearing via empty-defaults struct ALSO writes NULL (is_empty() guard).
         store
             .set_collection_defaults(&id, Some(&CollectionDefaultsSection::default()))
             .unwrap();
         let loaded = store.get_collection_defaults(&id).unwrap();
         assert!(loaded.is_none(), "empty defaults should normalize to NULL");
     }

     #[test]
     fn test_collection_defaults_unknown_id_errors_on_set() {
         let store = MetadataStore::open_in_memory().unwrap();
         let mut defaults = CollectionDefaultsSection::default();
         defaults.method = Some("native".into());
         let result = store.set_collection_defaults("no-such-id", Some(&defaults));
         assert!(matches!(result, Err(MetadataStoreError::Validation(_))));
     }

     #[test]
     fn test_collection_defaults_corrupt_json_returns_corrupt_error() {
         let store = MetadataStore::open_in_memory().unwrap();
         let id = store.create_collection("Corrupt").unwrap();

         // Force a corrupt JSON payload via raw SQL.
         let conn = connection(&store);
         conn.execute(
             "UPDATE collections SET defaults_json = ?1 WHERE collection_id = ?2",
             params!["{not-valid-json", id],
         )
         .unwrap();
         drop(conn);

         let result = store.get_collection_defaults(&id);
         assert!(
             matches!(result, Err(MetadataStoreError::Corrupt(_))),
             "corrupt JSON should surface as Corrupt, not Database"
         );
     }

     #[test]
     fn test_collection_defaults_cascades_on_collection_delete() {
         let store = MetadataStore::open_in_memory().unwrap();
         let id = store.create_collection("Scratch").unwrap();

         let mut defaults = CollectionDefaultsSection::default();
         defaults.method = Some("native".into());
         store.set_collection_defaults(&id, Some(&defaults)).unwrap();

         store.delete_collection(&id).unwrap();

         // After delete, reading defaults should error because the collection row is gone.
         let result = store.get_collection_defaults(&id);
         assert!(result.is_err(), "deleted collection defaults read should fail");
     }
     ```

- **MIRROR**: `rename_collection` / `update_collection_description` wrappers (`mod.rs:499-517`) for shape; `test_rename_collection_updates_name` / `test_update_collection_description_set_and_clear` for test style.
- **IMPORTS**: `CollectionDefaultsSection` from `crate::profile::models`; `MetadataStoreError` already in scope.
- **GOTCHA**:
  - **`with_conn`'s `T: Default` requirement is satisfied**: `Option<CollectionDefaultsSection>::default() = None` works for `get`; `() : Default` works for `set`. The disabled-store path returns `Ok(None)` / `Ok(())` automatically.
  - **`test_collection_defaults_cascades_on_collection_delete` does NOT depend on FK cascade** — it depends on the fact that `get_collection_defaults` errors when the row is missing. The `defaults_json` column is inside the `collections` row itself; deleting the row deletes the column.
  - **`test_collection_defaults_corrupt_json_returns_corrupt_error` requires using the `connection(&store)` helper and dropping the guard before re-locking**. Matches the Phase 1 test convention in `test_profile_delete_cascades_collection_membership`.
  - **`sample_profile()` is not needed** — these tests exercise the metadata layer only, not profile content. `create_collection` is all the setup required.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core test_collection_defaults` passes all five tests.

### Task 8: Add 2 new Tauri commands (`collection_get_defaults`, `collection_set_defaults`)

- **ACTION**: Add 2 new `#[tauri::command]` handlers to `src-tauri/src/commands/collections.rs`.
- **IMPLEMENT**: Append to `src-tauri/src/commands/collections.rs` after `collections_for_profile` at line 96:

  ```rust
  use crosshook_core::profile::models::CollectionDefaultsSection;

  #[tauri::command]
  pub fn collection_get_defaults(
      collection_id: String,
      metadata_store: State<'_, MetadataStore>,
  ) -> Result<Option<CollectionDefaultsSection>, String> {
      metadata_store
          .get_collection_defaults(&collection_id)
          .map_err(map_error)
  }

  #[tauri::command]
  pub fn collection_set_defaults(
      collection_id: String,
      defaults: Option<CollectionDefaultsSection>,
      metadata_store: State<'_, MetadataStore>,
  ) -> Result<(), String> {
      metadata_store
          .set_collection_defaults(&collection_id, defaults.as_ref())
          .map_err(map_error)
  }
  ```

- **MIRROR**: `collection_rename` / `collection_update_description` (`commands/collections.rs:65-85`) exactly.
- **IMPORTS**: Add `use crosshook_core::profile::models::CollectionDefaultsSection;` at the top of the file (next to the existing `use crosshook_core::metadata::{CollectionRow, MetadataStore};`). **Verify path** — if `profile::models` isn't re-exported, use the full path or add a re-export at `crosshook_core::profile`.
- **GOTCHA**:
  - **`defaults: Option<CollectionDefaultsSection>`** in the command signature — Tauri accepts `null` / missing as `None`, and a full struct as `Some`. The `.as_ref()` converts to `Option<&CollectionDefaultsSection>` for the store wrapper.
  - **Command name `collection_get_defaults`** — Tauri commands must be snake*case (CLAUDE.md). The name does NOT start with `get*`so the`wrapHandler.ts` `READ_VERB_RE`won't auto-classify it as a read. Task 12 adds it explicitly to`EXPLICIT_READ_COMMANDS`.
  - **`CollectionDefaultsSection` must be `Serialize + Deserialize`** — both are derived in Task 1, so this Just Works.
  - **Do NOT add `#[tauri::command(rename_all = "camelCase")]`** — the rest of the file uses default snake_case; Phase 1 established this.
  - **Positional args before `State`** — Tauri requires `State<'_, ...>` as the last parameter.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/src-tauri/Cargo.toml` succeeds. `tauri::generate_handler!` macro expansion works.

### Task 9: Extend `profile_load` to accept `collection_id: Option<String>`

- **ACTION**: Extend `profile_load` in `src-tauri/src/commands/profile.rs` to accept an optional `collection_id` parameter. When present, fetch the collection's defaults and return the merged profile.
- **IMPLEMENT**: Replace the current body at `commands/profile.rs:230-233`:

  ```rust
  #[tauri::command]
  pub fn profile_load(
      name: String,
      collection_id: Option<String>,
      store: State<'_, ProfileStore>,
      metadata_store: State<'_, MetadataStore>,
  ) -> Result<GameProfile, String> {
      let profile = store.load(&name).map_err(map_error)?;

      // When a collection context is provided, merge the collection's defaults
      // into the profile via `effective_profile_with`. The returned profile
      // still has its `local_override` layer applied last.
      match collection_id {
          Some(ref cid) if !cid.trim().is_empty() => {
              let defaults = metadata_store
                  .get_collection_defaults(cid)
                  .map_err(map_error)?;
              Ok(profile.effective_profile_with(defaults.as_ref()))
          }
          _ => Ok(profile),
      }
  }
  ```

- **MIRROR**: `profile_list_summaries` (`commands/profile.rs:245-279`) for the `effective_profile()` call pattern; `collection_update_description` (`commands/collections.rs:77-85`) for the `Option<String>` argument pattern.
- **IMPORTS**: Add `use crosshook_core::metadata::MetadataStore;` at the top of `commands/profile.rs` if not already present. The `effective_profile_with` method is already available via the `GameProfile` import.
- **GOTCHA**:
  - **Backward compat preservation**: existing callers (e.g., `useProfile.ts:567` currently calls `invoke('profile_load', { name: trimmed })` with no `collectionId`) remain correct because Tauri deserializes missing / null arguments as `None`. The `match None => Ok(profile)` path returns the raw storage profile — identical to pre-Phase-3 behavior.
  - **Empty-string `collection_id` is treated as no collection context** — the `!cid.trim().is_empty()` guard catches this. Prevents a degenerate `collection_id = ""` from hitting the metadata store.
  - **Collection row missing for `collection_id`**: `get_collection_defaults` returns `Err(Database { ... })` via `QueryReturnedNoRows`. The error surfaces to the frontend as a string. **Alternatively**, we could swallow this and return the raw profile (more forgiving). **Decision**: surface the error so the frontend can show "collection not found — launching with base profile only" and the user knows why defaults weren't applied. Document this in the Gotcha here.
  - **Corrupt `defaults_json` in the DB**: `get_collection_defaults` returns `Err(Corrupt(_))`. The Tauri map_error forwards the string. The frontend should catch this at `useProfile.loadProfile`, show a toast, and retry without the collection_id. Task 14 adds this retry.
  - **Adding `metadata_store: State<'_, MetadataStore>` requires `.manage(metadata_store.clone())` already in effect**. `src-tauri/src/lib.rs:201` calls `.manage(metadata_store)` — `MetadataStore` is already `Clone`-shared across handlers, confirmed by Phase 1 usage.
  - **No change to `profile_save`** — the save path does NOT consume collection defaults. Editor integrity is preserved.
- **VALIDATE**:
  - `cargo check --manifest-path src/crosshook-native/src-tauri/Cargo.toml` succeeds
  - Calling `invoke('profile_load', { name: 'foo' })` (no collectionId) returns the storage profile as before — verified by the existing `useProfile` call site continuing to work unchanged
  - Calling `invoke('profile_load', { name: 'foo', collectionId: 'col-1' })` returns the merged profile — verified in Task 10 mock + manual smoke

### Task 10: Register 2 new commands in `tauri::generate_handler!`

- **ACTION**: Add 2 lines to `src/crosshook-native/src-tauri/src/lib.rs` in the collection command block.
- **IMPLEMENT**: After `commands::collections::collections_for_profile,` at line 289:

  ```rust
  commands::collections::collection_list,
  commands::collections::collection_create,
  commands::collections::collection_delete,
  commands::collections::collection_add_profile,
  commands::collections::collection_remove_profile,
  commands::collections::collection_list_profiles,
  commands::collections::collection_rename,
  commands::collections::collection_update_description,
  commands::collections::collections_for_profile,
  commands::collections::collection_get_defaults,   // ← NEW
  commands::collections::collection_set_defaults,   // ← NEW
  commands::profile::profile_set_favorite,
  ```

- **MIRROR**: Phase 1's registration additions (`lib.rs:287-289`).
- **IMPORTS**: none.
- **GOTCHA**:
  - **Trailing commas are mandatory** — `tauri::generate_handler!` is a macro; missing comma produces a confusing expansion error.
  - **Insertion order is cosmetic** but grouping related commands keeps the diff clean.
  - **`profile_load` signature change requires re-generation** of the handler wrappers — `cargo check` on the `src-tauri` crate will fail on stale usage if the signature doesn't match the TS call shape.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/src-tauri/Cargo.toml --all-targets` succeeds.

### Task 11: Add browser dev-mode mocks for defaults commands + extend `profile_load` mock

- **ACTION**: Extend `src/lib/mocks/handlers/collections.ts` with 2 new handlers and a `mockDefaults` store; extend `src/lib/mocks/handlers/profile.ts`'s `profile_load` handler to accept `collectionId` and merge the mock defaults locally so dev-mode doesn't drift from real Rust behavior.
- **IMPLEMENT**:
  1. Extend `src/lib/mocks/handlers/collections.ts`. Add a module-scope `MockCollectionDefaults` map and two handlers inside `registerCollections`:

     ```ts
     // Shape mirrors Rust CollectionDefaultsSection. All fields optional.
     export interface MockCollectionDefaults {
       method?: string;
       optimizations?: { enabled_option_ids: string[] };
       custom_env_vars?: Record<string, string>;
       network_isolation?: boolean;
       gamescope?: unknown; // opaque — mirror Rust GamescopeConfig
       trainer_gamescope?: unknown;
       mangohud?: unknown;
     }

     const mockDefaults = new Map<string, MockCollectionDefaults>();

     function isDefaultsEmpty(d: MockCollectionDefaults | undefined): boolean {
       if (!d) return true;
       return (
         d.method === undefined &&
         d.optimizations === undefined &&
         (d.custom_env_vars === undefined || Object.keys(d.custom_env_vars).length === 0) &&
         d.network_isolation === undefined &&
         d.gamescope === undefined &&
         d.trainer_gamescope === undefined &&
         d.mangohud === undefined
       );
     }

     export function getMockCollectionDefaults(collectionId: string): MockCollectionDefaults | undefined {
       return mockDefaults.get(collectionId);
     }
     ```

     Then inside `registerCollections(map)`:

     ```ts
     map.set('collection_get_defaults', async (args): Promise<MockCollectionDefaults | null> => {
       const { collectionId } = args as { collectionId: string };
       if (!findById(collectionId)) {
         throw new Error(`[dev-mock] collection_get_defaults: collection not found: ${collectionId}`);
       }
       const d = mockDefaults.get(collectionId);
       return d && !isDefaultsEmpty(d) ? d : null;
     });

     map.set('collection_set_defaults', async (args): Promise<null> => {
       const { collectionId, defaults } = args as {
         collectionId: string;
         defaults: MockCollectionDefaults | null;
       };
       const target = findById(collectionId);
       if (!target) {
         throw new Error(`[dev-mock] collection_set_defaults: collection not found: ${collectionId}`);
       }
       if (defaults === null || isDefaultsEmpty(defaults)) {
         mockDefaults.delete(collectionId);
       } else {
         mockDefaults.set(collectionId, { ...defaults });
       }
       target.updated_at = nowIso();
       return null;
     });
     ```

  2. Extend `src/lib/mocks/handlers/profile.ts`'s `profile_load` handler to accept `collectionId` and apply the merge locally. At the current implementation around lines 191-199:

     ```ts
     map.set('profile_load', async (args): Promise<GameProfile | null> => {
       const { name, collectionId } = args as { name: string; collectionId?: string };
       const profile = getStore().profiles.get(name) ?? null;
       if (profile === null) return null;
       if (collectionId === undefined || collectionId === null || collectionId.trim() === '') {
         return profile;
       }
       // Apply the same merge as the Rust effective_profile_with.
       const defaults = getMockCollectionDefaults(collectionId);
       if (!defaults) return profile;
       return applyMockCollectionDefaults(profile, defaults);
     });
     ```

     And add the merge helper at the top of `profile.ts` (or in a shared `merge.ts` under `mocks/`):

     ```ts
     import type { MockCollectionDefaults } from './collections';
     import { getMockCollectionDefaults } from './collections';

     function applyMockCollectionDefaults(profile: GameProfile, d: MockCollectionDefaults): GameProfile {
       // Deep-clone to avoid mutating the store's canonical copy.
       const merged: GameProfile = JSON.parse(JSON.stringify(profile));

       if (typeof d.method === 'string' && d.method.trim() !== '') {
         merged.launch.method = d.method;
       }
       if (d.optimizations) {
         merged.launch.optimizations = {
           enabled_option_ids: [...(d.optimizations.enabled_option_ids ?? [])],
         };
       }
       if (d.custom_env_vars) {
         merged.launch.custom_env_vars = {
           ...(merged.launch.custom_env_vars ?? {}),
           ...d.custom_env_vars, // collection wins on collision
         };
       }
       if (typeof d.network_isolation === 'boolean') {
         merged.launch.network_isolation = d.network_isolation;
       }
       if (d.gamescope !== undefined) {
         (merged.launch as unknown as { gamescope: unknown }).gamescope = d.gamescope;
       }
       if (d.trainer_gamescope !== undefined) {
         (merged.launch as unknown as { trainer_gamescope: unknown }).trainer_gamescope = d.trainer_gamescope;
       }
       if (d.mangohud !== undefined) {
         (merged.launch as unknown as { mangohud: unknown }).mangohud = d.mangohud;
       }

       return merged;
     }
     ```

- **MIRROR**: Phase 1's `collection_rename` mock handler (`handlers/collections.ts:~90`) for the shape; existing `profile_load` mock (`handlers/profile.ts:191-199`) for the starting point.
- **IMPORTS**: `GameProfile` type from `../../../types` (matching existing imports in `profile.ts`).
- **GOTCHA**:
  - **`[dev-mock]` prefix on every thrown error** — `.github/workflows/release.yml:105-120` greps for this literal to verify no mock code leaked into production. Every `throw new Error(...)` in the new handlers MUST start with `[dev-mock]`.
  - **`JSON.parse(JSON.stringify(profile))` deep-clone** is correct for the mock because `GameProfile` is serde-friendly (no cycles, no `Date`, no `undefined` surprises). In production Rust this is a `.clone()`, but JS structuredClone may not be universally available in test contexts; JSON.parse/stringify is the safest portable clone.
  - **Collection get returns `null`, not `undefined`** when no defaults exist — matches Tauri `Option<T>` → `null` serialization.
  - **`collection_get_defaults` argument name is `collectionId` in JS**, not `collection_id` — Tauri camelCase convention on the JS side; Rust stays snake_case. Phase 1 established this (`collection_rename` uses `newName` not `new_name` on the JS side).
  - **Empty-defaults normalization on set**: the `isDefaultsEmpty` check ensures `setDefaults(id, { method: "" })` is NOT stored as non-empty — prevents a stale "all keys empty" blob from polluting the mock. Matches the Rust `is_empty()` guard in Task 6.
  - **Do NOT track profiles in `mockDefaults`** — defaults are keyed only by `collectionId`. The profile layer handles its own state.
  - **Do NOT auto-register profiles in the profile mock to match real-store names** — the existing mock fixture is enough. The defaults-merge smoke test uses an existing mock profile.
- **VALIDATE**:
  - `pnpm --dir src/crosshook-native type-check` passes (no TS errors in the new code)
  - `./scripts/dev-native.sh --browser` starts without console errors on a `callCommand('collection_get_defaults', { collectionId: 'mock-collection-1' })` call
  - Manual smoke: call `callCommand('collection_set_defaults', { collectionId: 'mock-collection-1', defaults: { method: 'proton_run', custom_env_vars: { DXVK_HUD: '1' } } })` then `callCommand('profile_load', { name: '<existing>', collectionId: 'mock-collection-1' })` and verify the returned profile has the merged env var

### Task 12: Register `collection_get_defaults` in `wrapHandler.ts` read allow-list

- **ACTION**: Add `collection_get_defaults` to the `EXPLICIT_READ_COMMANDS` array in `src/crosshook-native/src/lib/mocks/wrapHandler.ts`.
- **IMPLEMENT**: In `wrapHandler.ts` (around lines 38-58, the `EXPLICIT_READ_COMMANDS` list):

  ```ts
  const EXPLICIT_READ_COMMANDS: ReadonlySet<string> = new Set([
    // ... existing entries ...
    'profile_load',
    // ... other entries ...
    'collection_get_defaults', // ← NEW (name starts with "collection_", not "get_", so READ_VERB_RE doesn't match)
  ]);
  ```

- **MIRROR**: The existing `profile_load` entry and other `collection_*` reads in the set.
- **IMPORTS**: none.
- **GOTCHA**:
  - **`collection_set_defaults` does NOT need to be added** — writes are matched by a separate pattern (or pass through unchanged; consult the file structure). The set/get naming asymmetry is intentional.
  - **The `READ_VERB_RE` uses `^(get_|list_|...)` which anchors at the start** — `collection_get_defaults` starts with `collection_`, not `get_`, so the regex won't match. Explicit listing is required.
  - **Alternative naming**: `get_collection_defaults` would match the regex automatically. **Not chosen** because Phase 1 established `collection_*` as the command prefix convention for all collection commands (`collection_list`, `collection_create`, `collection_add_profile`, etc.) and consistency wins over the marginal benefit of auto-matching.
- **VALIDATE**: `pnpm --dir src/crosshook-native type-check` passes; browser dev-mode session does not throw "write blocked: collection_get_defaults" on a read call.

### Task 13: Add `CollectionDefaults` TypeScript type

- **ACTION**: Add the TS interface mirroring `CollectionDefaultsSection` in `src/crosshook-native/src/types/profile.ts` (or a new `collections.ts`).
- **IMPLEMENT**: Append to `types/profile.ts`:

  ```ts
  /**
   * Collection-scoped overrides for LaunchSection fields.
   * Mirrors Rust `CollectionDefaultsSection` (serde-compatible JSON).
   *
   * - `undefined` for Option<T> fields means "inherit from profile"
   * - `custom_env_vars` is an additive merge: collection entries union with
   *   the profile's, collection keys win on collision
   */
  export interface CollectionDefaults {
    method?: string;
    optimizations?: LaunchOptimizationsSection;
    custom_env_vars?: Record<string, string>;
    network_isolation?: boolean;
    gamescope?: GamescopeConfig;
    trainer_gamescope?: GamescopeConfig;
    mangohud?: MangoHudConfig;
  }

  export function isCollectionDefaultsEmpty(d: CollectionDefaults | null | undefined): boolean {
    if (!d) return true;
    return (
      d.method === undefined &&
      d.optimizations === undefined &&
      (d.custom_env_vars === undefined || Object.keys(d.custom_env_vars).length === 0) &&
      d.network_isolation === undefined &&
      d.gamescope === undefined &&
      d.trainer_gamescope === undefined &&
      d.mangohud === undefined
    );
  }
  ```

- **MIRROR**: Existing type exports in `types/profile.ts` — `LaunchOptimizationsSection`, `GamescopeConfig`, `MangoHudConfig` already live here. Reuse them.
- **IMPORTS**: from existing types in the same file.
- **GOTCHA**:
  - **`Record<string, string>` for custom_env_vars** — Tauri serializes Rust `BTreeMap<String, String>` to JSON `{ "k": "v" }`, which TS accepts as `Record<string, string>`. Do NOT use `Map<string, string>`; the JSON.stringify on the wire is an object, not a Map.
  - **Optional vs `null`**: use `?` (undefined when missing) for fields, NOT `| null`. Tauri's `Option<T>` serializes to `undefined` / missing field in JSON, not `null`. The mock layer's `null` return for `collection_get_defaults` is a separate concern — `null` at the top level means "no defaults", while inner fields use `undefined`.
  - **Keep the interface in `types/profile.ts`** (not a new file) because it's tightly coupled to the profile shape. Alternative: create `types/collections.ts` if the collections feature grows a lot more TS types.
- **VALIDATE**: `pnpm --dir src/crosshook-native type-check` passes.

### Task 14: Extend `useProfile.loadProfile` to accept `collectionId`

- **ACTION**: Extend the `loadProfile` function in `src/crosshook-native/src/hooks/useProfile.ts` to accept an optional `collectionId` in `loadOptions` and forward it to `profile_load`.
- **IMPLEMENT**: Replace the current `loadProfile` body (`useProfile.ts:549-598`):

  ```ts
  const loadProfile = useCallback(
    async (
      name: string,
      loadOptions?: {
        collectionId?: string;
        loadErrorContext?: string;
        throwOnFailure?: boolean;
      }
    ) => {
      const trimmed = name.trim();
      if (!trimmed) {
        setSelectedProfile('');
        setProfileName('');
        setProfile(createEmptyProfile());
        setDirty(false);
        lastSavedLaunchOptimizationIdsRef.current = [];
        return;
      }

      setLoading(true);
      setError(null);

      const formatLoadError = (err: unknown) => (err instanceof Error ? err.message : String(err));

      // Normalize collectionId: trim + drop empties so we don't send a
      // degenerate `""` down to Rust which would trigger a collection-missing
      // error. Undefined == no collection context == raw storage profile.
      const collectionId = loadOptions?.collectionId?.trim() || undefined;

      try {
        const loaded = await callCommand<SerializedGameProfile>('profile_load', {
          name: trimmed,
          collectionId,
        });
        const normalized = normalizeProfileForEdit(loaded, optionsById);
        setSelectedProfile(trimmed);
        setProfileName(trimmed);
        setProfile(normalized);
        setDirty(false);
        lastSavedLaunchOptimizationIdsRef.current = normalized.launch.optimizations.enabled_option_ids;
        lastSavedGamescopeJsonRef.current = JSON.stringify(normalized.launch.gamescope ?? null);
        lastSavedTrainerGamescopeJsonRef.current = JSON.stringify(normalized.launch.trainer_gamescope ?? null);
        lastSavedMangoHudJsonRef.current = JSON.stringify(normalized.launch.mangohud ?? null);

        try {
          await syncProfileMetadata(trimmed, normalized);
        } catch (syncErr) {
          console.error('Failed to sync profile metadata (last-used profile, recent files)', syncErr);
          setError(
            `Profile loaded, but preferences sync failed: ${
              syncErr instanceof Error ? syncErr.message : String(syncErr)
            }`
          );
        }
      } catch (err) {
        const msg = formatLoadError(err);
        const fullMsg = loadOptions?.loadErrorContext ? `${loadOptions.loadErrorContext}: ${msg}` : msg;
        setError(fullMsg);
        if (loadOptions?.throwOnFailure) {
          throw fullMsg;
        }
      } finally {
        setLoading(false);
      }
    },
    [optionsById, syncProfileMetadata]
  );
  ```

- **MIRROR**: Existing `loadProfile` body (`useProfile.ts:549-598`). Only change: add `collectionId` to the options type and pass it to `callCommand`.
- **IMPORTS**: none new — `callCommand` already imported.
- **GOTCHA**:
  - **EDITOR SAFETY INVARIANT (critical)**: `ProfilesPage` call sites of `loadProfile` MUST NOT pass `collectionId`. The editor must always see the raw storage profile. If a future contributor accidentally passes `collectionId` from ProfilesPage, the user will see merged data in the editor, edit it, and save — saving the merged view into the profile TOML. **Document this invariant at the ProfilesPage call site with a comment**: `// NEVER pass collectionId — editor must see storage profile`.
  - **Empty-string `collectionId` normalization to `undefined`** — matches the Rust-side `!cid.trim().is_empty()` guard in Task 9. A degenerate empty string would otherwise travel to Rust and be treated as a valid-but-missing collection id, which errors.
  - **`syncProfileMetadata` is still called with the normalized profile** — for a collection-loaded profile, this writes the merged values as the "last used" state. This is acceptable because `syncProfileMetadata` only tracks usage statistics, not the profile body. **Verify by reading `syncProfileMetadata`** — if it persists the profile body anywhere, Task 14 must exclude collection-loaded profiles from the sync.
  - **`lastSavedGamescopeJsonRef` and friends** are dirty-check anchors for the editor. When the profile is loaded with collection defaults, these refs capture the MERGED JSON — which means the editor thinks "this is the initial state", and any edit away from it marks the profile dirty. This is fine for launch-time loads (which don't use the editor) but would be incorrect if a collection-loaded profile were then edited. The editor-safety invariant above keeps this consistent: ProfilesPage never passes collectionId, so the refs always anchor to the storage profile.
  - **Does NOT touch `refreshProfiles`** — that function also calls `loadProfile` as part of auto-selecting the first profile. The auto-select path does not pass `collectionId`, so it loads the raw storage profile — correct for the app-boot flow.
- **VALIDATE**:
  - `pnpm --dir src/crosshook-native type-check` passes
  - Existing call sites in ProfilesPage and the auto-select path continue to pass no `collectionId` and load raw storage profiles

### Task 15: Create `useCollectionDefaults` hook

- **ACTION**: Create `src/crosshook-native/src/hooks/useCollectionDefaults.ts` — a new hook mirroring `useCollectionMembers` that fetches and sets collection defaults.
- **IMPLEMENT**: New file:

  ```ts
  import { useCallback, useEffect, useRef, useState } from 'react';

  import { callCommand } from '../lib/ipc';
  import type { CollectionDefaults } from '../types/profile';

  interface UseCollectionDefaultsReturn {
    defaults: CollectionDefaults | null;
    loading: boolean;
    error: string | null;
    saveDefaults: (next: CollectionDefaults | null) => Promise<void>;
    reload: () => Promise<void>;
  }

  /**
   * Fetches + writes per-collection launch defaults.
   * Mirrors `useCollectionMembers` for race-safety via `requestSeqRef`.
   */
  export function useCollectionDefaults(collectionId: string | null): UseCollectionDefaultsReturn {
    const [defaults, setDefaults] = useState<CollectionDefaults | null>(null);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);
    const requestSeqRef = useRef(0);

    const reload = useCallback(async () => {
      if (collectionId === null) {
        setDefaults(null);
        return;
      }
      const seq = ++requestSeqRef.current;
      setLoading(true);
      setError(null);
      try {
        const result = await callCommand<CollectionDefaults | null>('collection_get_defaults', { collectionId });
        if (seq !== requestSeqRef.current) return;
        setDefaults(result ?? null);
      } catch (err) {
        if (seq !== requestSeqRef.current) return;
        setError(err instanceof Error ? err.message : String(err));
        setDefaults(null);
      } finally {
        if (seq === requestSeqRef.current) {
          setLoading(false);
        }
      }
    }, [collectionId]);

    useEffect(() => {
      reload();
    }, [reload]);

    const saveDefaults = useCallback(
      async (next: CollectionDefaults | null) => {
        if (collectionId === null) {
          throw new Error('cannot save defaults without a collection id');
        }
        await callCommand<null>('collection_set_defaults', {
          collectionId,
          defaults: next,
        });
        await reload();
      },
      [collectionId, reload]
    );

    return { defaults, loading, error, saveDefaults, reload };
  }
  ```

- **MIRROR**: `useCollectionMembers` (`src/crosshook-native/src/hooks/useCollectionMembers.ts`) for race-safety and state shape.
- **IMPORTS**: `callCommand` from `../lib/ipc`, `CollectionDefaults` from `../types/profile`.
- **GOTCHA**:
  - **`requestSeqRef` is essential** — without it, rapid `collectionId` changes cause stale responses to overwrite fresh state.
  - **`reload()` is exposed** so the editor can force-refresh after a save (already done inside `saveDefaults`, but exposed for edge cases like error-recovery refresh).
  - **`saveDefaults(null)` clears** — matches the Rust-side behavior where `None` writes NULL.
  - **Do NOT memoize `defaults`** — it's local state; React handles re-renders.
  - **`collectionId === null` skip is important** — the hook mounts in `<CollectionViewModal>` which only opens when a collection is selected, but defensive null-check prevents accidental IPC calls with empty id.
- **VALIDATE**: `pnpm --dir src/crosshook-native type-check` passes.

### Task 16: Create `<CollectionLaunchDefaultsEditor>` component

- **ACTION**: Create `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx` — the inline editor UI used inside the modal.
- **IMPLEMENT**: New file. Structure:

  ```tsx
  import { useCallback, useMemo, useState } from 'react';

  import { useCollectionDefaults } from '../../hooks/useCollectionDefaults';
  import { isCollectionDefaultsEmpty, type CollectionDefaults } from '../../types/profile';

  interface Props {
    collectionId: string;
    onOpenInProfilesPage: () => void;
  }

  /**
   * Inline editor for per-collection launch defaults. Renders inside the
   * `<CollectionViewModal>` body. Users can toggle/clear each field; saving
   * writes the defaults via `collection_set_defaults`.
   *
   * Excluded fields (per PRD): `presets`, `active_preset`. Use the link-out
   * for those.
   */
  export function CollectionLaunchDefaultsEditor({ collectionId, onOpenInProfilesPage }: Props) {
    const { defaults, loading, error, saveDefaults } = useCollectionDefaults(collectionId);
    const [draft, setDraft] = useState<CollectionDefaults>({});
    const [saving, setSaving] = useState(false);
    const [saveError, setSaveError] = useState<string | null>(null);

    // Reset the draft when fresh defaults arrive from the backend.
    const resetDraftFromLoaded = useCallback(() => {
      setDraft(defaults ?? {});
    }, [defaults]);

    // Initial sync: when the loaded defaults change, reset draft.
    useMemo(() => {
      resetDraftFromLoaded();
    }, [resetDraftFromLoaded]);

    const isDraftEmpty = isCollectionDefaultsEmpty(draft);

    const handleSave = async () => {
      setSaving(true);
      setSaveError(null);
      try {
        await saveDefaults(isDraftEmpty ? null : draft);
      } catch (err) {
        setSaveError(err instanceof Error ? err.message : String(err));
      } finally {
        setSaving(false);
      }
    };

    const handleReset = () => {
      setDraft({});
    };

    // Env-var editor state (inline, minimal — a simple table).
    const addEnvVar = () => {
      const nextVars = { ...(draft.custom_env_vars ?? {}) };
      // Use a placeholder key for new rows; user renames it.
      let i = 1;
      let key = `NEW_VAR_${i}`;
      while (key in nextVars) {
        i += 1;
        key = `NEW_VAR_${i}`;
      }
      nextVars[key] = '';
      setDraft({ ...draft, custom_env_vars: nextVars });
    };

    const updateEnvVar = (oldKey: string, newKey: string, value: string) => {
      const nextVars = { ...(draft.custom_env_vars ?? {}) };
      delete nextVars[oldKey];
      if (newKey.trim() !== '') {
        nextVars[newKey] = value;
      }
      setDraft({ ...draft, custom_env_vars: nextVars });
    };

    const removeEnvVar = (key: string) => {
      const nextVars = { ...(draft.custom_env_vars ?? {}) };
      delete nextVars[key];
      setDraft({ ...draft, custom_env_vars: nextVars });
    };

    return (
      <details className="crosshook-collection-launch-defaults-editor">
        <summary>
          Collection launch defaults
          {!isCollectionDefaultsEmpty(defaults) && (
            <span className="crosshook-collection-launch-defaults-editor__badge">Active</span>
          )}
        </summary>
        {loading && <p>Loading defaults…</p>}
        {error && <p className="crosshook-collection-launch-defaults-editor__error">{error}</p>}
        {!loading && (
          <div className="crosshook-collection-launch-defaults-editor__body">
            <label>
              Method:
              <select
                value={draft.method ?? ''}
                onChange={(e) =>
                  setDraft({
                    ...draft,
                    method: e.target.value === '' ? undefined : e.target.value,
                  })
                }
              >
                <option value="">(inherit)</option>
                <option value="native">native</option>
                <option value="proton_run">proton_run</option>
                <option value="steam_applaunch">steam_applaunch</option>
              </select>
            </label>

            <label>
              Network isolation:
              <select
                value={draft.network_isolation === undefined ? '' : draft.network_isolation ? 'on' : 'off'}
                onChange={(e) => {
                  const v = e.target.value;
                  setDraft({
                    ...draft,
                    network_isolation: v === '' ? undefined : v === 'on' ? true : false,
                  });
                }}
              >
                <option value="">(inherit)</option>
                <option value="on">on</option>
                <option value="off">off</option>
              </select>
            </label>

            <fieldset className="crosshook-collection-launch-defaults-editor__env">
              <legend>Custom env vars (additive)</legend>
              {Object.entries(draft.custom_env_vars ?? {}).map(([k, v]) => (
                <div key={k} className="crosshook-collection-launch-defaults-editor__env-row">
                  <input type="text" value={k} onChange={(e) => updateEnvVar(k, e.target.value, v)} placeholder="KEY" />
                  <input
                    type="text"
                    value={v}
                    onChange={(e) => updateEnvVar(k, k, e.target.value)}
                    placeholder="value"
                  />
                  <button type="button" onClick={() => removeEnvVar(k)} aria-label={`Remove ${k}`}>
                    ×
                  </button>
                </div>
              ))}
              <button type="button" onClick={addEnvVar}>
                + Add env var
              </button>
            </fieldset>

            <p className="crosshook-collection-launch-defaults-editor__hint">
              Gamescope, MangoHUD, and optimizations are managed from the Profiles page.
            </p>

            <div className="crosshook-collection-launch-defaults-editor__actions">
              <button type="button" onClick={onOpenInProfilesPage}>
                Open in Profiles page →
              </button>
              <button type="button" onClick={handleReset}>
                Reset draft
              </button>
              <button type="button" onClick={handleSave} disabled={saving} className="crosshook-button--primary">
                {saving ? 'Saving…' : 'Save'}
              </button>
            </div>
            {saveError && <p className="crosshook-collection-launch-defaults-editor__error">{saveError}</p>}
          </div>
        )}
      </details>
    );
  }
  ```

- **MIRROR**: The `<details>` collapsible pattern is light-weight and accessible; no existing component extraction needed. Form primitives are raw `<input>`/`<select>` — matching the frontend minimalism ethos.
- **IMPORTS**: React hooks, `useCollectionDefaults`, `CollectionDefaults` type + `isCollectionDefaultsEmpty` helper.
- **GOTCHA**:
  - **`<details>` is controller-friendly** — works on Steam Deck with D-pad navigation. Per the PRD's "controller-friendlier UX" constraint.
  - **Env-var editing renames keys in-place** — when a user changes the key in the left input, we delete the old key and insert the new. This can cause React reconciliation weirdness if two rows briefly collide. **Mitigation**: use the key value as the React `key` prop — safe because we delete before insert.
  - **Inline editor v1 only exposes `method`, `network_isolation`, `custom_env_vars`** — `optimizations`, `gamescope`, `trainer_gamescope`, `mangohud` are intentionally NOT editable inline in this initial version. The "Open in Profiles page →" link covers them. **This is a PRD-compatible scope cut** — the PRD's MoSCoW lists "Edit a collection's per-collection launch defaults inline ... with a 'Open in Profiles page →' link for advanced overrides" as **Should**, not **Must**. Per-collection gamescope editing is implicitly captured by "advanced overrides".
  - **No validation on env var keys** — per NOT Building. Users can enter any string; runtime is the enforcer.
  - **`useMemo(() => { resetDraftFromLoaded(); }, [resetDraftFromLoaded])` looks odd** — this is an initialization sync. Alternative: `useEffect` with the same deps; either works. `useEffect` is probably safer here to avoid running during render; **correct this to `useEffect`** in the actual implementation.
  - **Save-success feedback is implicit** — after save, `saveDefaults` triggers `reload`, which updates `defaults`, which triggers the `useEffect`-synced draft refresh. If the editor was dirty, this re-anchors it. If the save returns an error, the error state shows. **No toast** — per the project's existing minimalism (the modal's outer error handling is enough).
  - **`crosshook-button--primary` class** — verify this class exists in the project's existing CSS; if not, use whatever primary-button class `<CollectionViewModal>` already uses for consistency.
  - **New scroll container**: if the env var list grows long (>10 rows), it may become a scroll area. If you add `overflow-y: auto` anywhere in the editor's CSS, **YOU MUST add the selector to `src/crosshook-native/src/hooks/useScrollEnhance.ts:9` `SCROLLABLE`** and set `overscroll-behavior: contain` per CLAUDE.md's WebKitGTK scroll rule. **Initial implementation should NOT add a scroll container** — let the modal body scroll instead.
- **VALIDATE**:
  - `pnpm --dir src/crosshook-native type-check` passes
  - Component renders in a Storybook-less smoke: mount inside `<CollectionViewModal>` and verify it displays "Collection launch defaults" <details>
  - Saving a draft with `method: "proton_run"` and one env var round-trips: close modal, reopen, see the same values

### Task 17: Wire the editor into `<CollectionViewModal>` body

- **ACTION**: Add the `<CollectionLaunchDefaultsEditor>` to the existing `<CollectionViewModal>` component's body, above the search input. Pass the new `onOpenInProfilesPage` callback prop through.
- **IMPLEMENT**: In `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`:
  1. Add `onOpenInProfilesPage` to the props interface:

     ```ts
     interface CollectionViewModalProps {
       // ... existing props ...
       onOpenInProfilesPage: () => void;
     }
     ```

  2. Inside the modal body (look for `<div className="crosshook-modal__body crosshook-collection-modal__body">` around `CollectionViewModal.tsx:293`), add the editor **before** the search input:

     ```tsx
     <div className="crosshook-modal__body crosshook-collection-modal__body">
       <CollectionLaunchDefaultsEditor
         collectionId={collection.collection_id}
         onOpenInProfilesPage={onOpenInProfilesPage}
       />

       {/* existing search input and member list stay here */}
     </div>
     ```

  3. Add the import at the top of `CollectionViewModal.tsx`:

     ```ts
     import { CollectionLaunchDefaultsEditor } from './CollectionLaunchDefaultsEditor';
     ```

- **MIRROR**: The existing prop-passing pattern (`onEdit`, `onLaunch`, `onRemove` — whatever callback pattern the modal uses in Phase 2).
- **IMPORTS**: `CollectionLaunchDefaultsEditor` from the new file.
- **GOTCHA**:
  - **Do not wrap the editor in a new `overflow-y: auto` container** — it stays inside `.crosshook-modal__body` which is already in the `SCROLLABLE` selector (`useScrollEnhance.ts:9`).
  - **The collapsed `<details>` does not affect layout** — it's non-intrusive on first open for users who aren't using defaults yet.
  - **`collection.collection_id` comes from the existing `collection` prop** — verify this prop name in the Phase 2 modal (it may be `collection: CollectionRow` or destructured `{ collectionId, ... }`). Adjust the spread accordingly.
- **VALIDATE**: `pnpm --dir src/crosshook-native type-check` passes; the modal renders with the editor visible.

### Task 18: Wire `onOpenInProfilesPage` from `App.tsx`

- **ACTION**: In `src/crosshook-native/src/App.tsx` where `<CollectionViewModal>` is instantiated (Phase 2), add the `onOpenInProfilesPage` callback that navigates to the Profiles page without unsetting the active collection filter.
- **IMPLEMENT**: Find the existing `<CollectionViewModal ... />` render in App.tsx (Phase 2 work). Add the callback:

  ```tsx
  <CollectionViewModal
    // ... existing props ...
    onOpenInProfilesPage={() => {
      // activeCollectionId is already set in ProfileContext; just navigate.
      setOpenCollectionId(null); // close the modal
      setRoute('profiles');
    }}
  />
  ```

- **MIRROR**: Existing `handleEditFromCollection` pattern (`App.tsx:113-119`) — same shape, different route wiring.
- **IMPORTS**: none new — `setRoute`, `setOpenCollectionId` are already in scope.
- **GOTCHA**:
  - **`activeCollectionId` is not cleared** — the link preserves the filter so ProfilesPage opens "inside" the collection. Phase 2's ProfilesPage filter logic handles the rest.
  - **Close the modal before navigating** — matches the existing `handleEditFromCollection` behavior, avoids leaving a stale modal visible during route transition.
- **VALIDATE**: Clicking "Open in Profiles page →" in the defaults editor navigates to the Profiles page with the collection filter intact.

### Task 19: Wire `loadProfile(collectionId)` from `LaunchPage`

- **ACTION**: At the Active-Profile dropdown onChange handler in `LaunchPage.tsx`, pass `activeCollectionId ?? undefined` as the `collectionId` option to `loadProfile`.
- **IMPLEMENT**: Find the `ThemedSelect` onChange handler in `LaunchPage.tsx` (around line 295-306, where the active profile is selected). Currently:

  ```tsx
  onChange={async (value) => {
    await loadProfile(value);
  }}
  ```

  Change to:

  ```tsx
  onChange={async (value) => {
    await loadProfile(value, {
      collectionId: activeCollectionId ?? undefined,
    });
  }}
  ```

- **MIRROR**: Existing `loadProfile(value)` call shape + the new Task 14 options parameter.
- **IMPORTS**: `activeCollectionId` is already read from `profileState` at the top of `LaunchPage.tsx:24` (Phase 2).
- **GOTCHA**:
  - **EDITOR SAFETY (critical)**: `ProfilesPage.tsx` must NOT receive this change. ProfilesPage's profile selector stays `await loadProfile(value)` with no options — the editor always loads storage profiles. Add a comment: `// NEVER pass collectionId — editor requires storage profile`.
  - **`activeCollectionId` can be `null`** — the `?? undefined` conversion ensures we send `undefined` (which becomes missing field in JSON) rather than `null` (which would be deserialized as `Some(None)` — wait, no, `null` deserializes to `None` for `Option<String>`, so both work. But `undefined` is semantically cleaner and matches the Task 14 normalization).
  - **If Phase 2 lands a collection-clear chip on LaunchPage** that sets `activeCollectionId = null`, a subsequent profile selection via the dropdown will correctly load the raw storage profile. No additional logic needed.
- **VALIDATE**:
  - With `activeCollectionId = null`: selecting a profile in LaunchPage loads raw storage profile (existing behavior)
  - With `activeCollectionId = "some-id"`: selecting a profile loads the merged profile; the dropdown's other profiles also load merged when selected
  - Verified via browser dev-mode + Rust end-to-end manual test in Task 20

### Task 20: Manual end-to-end validation + printenv test fixture

- **ACTION**: Manual validation that a profile launched from a collection context receives the collection's env var overrides.
- **IMPLEMENT**:
  1. Create a minimal test fixture in a dev profile. In the app:
     - Create a profile "printenv-test" with `game.executable_path = /usr/bin/printenv` and `trainer.path = /usr/bin/true` (or similar).
     - Create a collection "EnvTest" and add the profile to it.
     - Open the collection in the view modal, expand the defaults editor.
     - Add env var `CROSSHOOK_PROBE=hello`.
     - Save.
  2. Launch the profile **from the LaunchPage while `activeCollectionId === "EnvTest"`** (i.e., the filter is active). Capture stdout.
     - **Expected**: `CROSSHOOK_PROBE=hello` appears in the printenv output.
  3. Clear the collection filter (`activeCollectionId = null`). Launch the same profile from the flat Active-Profile dropdown (no collection context).
     - **Expected**: `CROSSHOOK_PROBE` is NOT in the printenv output.
  4. Also verify corrupt-JSON recovery: manually SET `defaults_json = 'bad-json'` via `sqlite3` on the metadata DB; reopen the app; call `collection_get_defaults` via the dev-mode console and confirm it returns a friendly error string.
- **MIRROR**: No existing precedent — this is a new manual validation class.
- **GOTCHA**:
  - **`printenv` is a native Linux binary** — launching it via `proton_run` will wrap it; use `native` launch method for the test.
  - **`CROSSHOOK_PROBE` is a safe env var name** (no system collision). Do NOT use `PATH`, `HOME`, etc.
  - **Clean up after**: delete the "EnvTest" collection and "printenv-test" profile once validated.
- **VALIDATE**:
  - Launch-with-collection shows the env var in printenv output
  - Launch-without-collection does NOT show the env var
  - Corrupt JSON surfaces as a toast / error, not a crash

---

## Testing Strategy

### Unit Tests (Rust)

| Test                                                                                | Input                                                                                | Expected Output                                                                                                     | Edge Case?                    |
| ----------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------- | ----------------------------- |
| `effective_profile_with_none_equals_shim`                                           | profile with env vars + local_override                                               | `effective_profile()` == `effective_profile_with(None)`                                                             | ✓ (backward-compat invariant) |
| `effective_profile_with_merges_collection_defaults_between_base_and_local_override` | profile + collection defaults (env vars, method, network isolation) + local_override | base → collection defaults → local_override precedence verified; env vars merged additively with collection winning | ✓ (precedence invariant)      |
| `effective_profile_with_none_fields_do_not_overwrite_profile`                       | profile with env vars, empty collection defaults                                     | profile fields fully preserved                                                                                      | ✓ (empty-defaults no-op)      |
| `effective_profile_with_ignores_whitespace_only_method`                             | defaults with `method = Some("   ")`                                                 | profile method not clobbered                                                                                        | ✓ (whitespace guard)          |
| `migration_19_to_20_adds_defaults_json_column`                                      | fresh in-memory DB                                                                   | version == 20, `defaults_json` col exists as nullable TEXT, round-trips JSON and NULL                               | ✓ (migration correctness)     |
| `test_collection_defaults_set_and_get_roundtrip`                                    | set then get; set then clear then get                                                | round-trip equals; cleared equals None                                                                              | happy path + clear path       |
| `test_collection_defaults_clear_writes_null`                                        | `set(None)` and `set(Some(empty))`                                                   | both normalize to NULL via `is_empty` guard                                                                         | ✓ (idempotent clear)          |
| `test_collection_defaults_unknown_id_errors_on_set`                                 | set defaults on nonexistent collection                                               | `Err(Validation("collection not found: ..."))`                                                                      | ✓ (missing collection)        |
| `test_collection_defaults_corrupt_json_returns_corrupt_error`                       | raw-SQL inject corrupt JSON, then `get`                                              | `Err(Corrupt(_))`                                                                                                   | ✓ (corrupt-read fallback)     |
| `test_collection_defaults_cascades_on_collection_delete`                            | set defaults, delete collection, try to get                                          | error (collection row gone)                                                                                         | ✓ (delete cleanup)            |

**Total: 10 new Rust tests.** All use `MetadataStore::open_in_memory()` — zero filesystem I/O.

### Integration Coverage (manual, post-implementation)

- [ ] `./scripts/dev-native.sh --browser` starts without crashing on `collection_get_defaults` / `collection_set_defaults` calls
- [ ] Devtools console: `invoke('collection_set_defaults', { collectionId: 'mock-collection-1', defaults: { method: 'proton_run', custom_env_vars: { DXVK_HUD: '1' } } })` → `null`
- [ ] Devtools console: `invoke('collection_get_defaults', { collectionId: 'mock-collection-1' })` → returns the just-set defaults
- [ ] Devtools console: `invoke('profile_load', { name: '<mock-profile>', collectionId: 'mock-collection-1' })` → returns a profile with `launch.custom_env_vars.DXVK_HUD === '1'`
- [ ] Devtools console: `invoke('profile_load', { name: '<mock-profile>' })` (no collectionId) → returns the raw storage profile (no `DXVK_HUD`)
- [ ] Task 20 end-to-end `printenv` manual test passes
- [ ] `./scripts/build-native.sh --binary-only` builds successfully; production bundle does NOT contain any `[dev-mock]` strings

### Edge Cases Checklist

- [x] **Empty defaults (all-None / empty-map)** — normalized to NULL column via `is_empty()` guard
- [x] **Corrupt JSON in `defaults_json`** — returns `MetadataStoreError::Corrupt`, frontend shows error
- [x] **Unknown `collection_id`** — `set` errors with Validation; `profile_load` errors with Database; mock layer errors with `[dev-mock] collection not found`
- [x] **`collection_id = ""` (empty string)** — treated as no collection context (Rust + frontend normalization)
- [x] **Whitespace-only `method`** — does not clobber profile's method (guarded)
- [x] **Env var key collision between profile and collection** — collection wins (additive merge semantics)
- [x] **`profile_load` with `collection_id = None`** — backward-compat: returns raw storage profile unchanged
- [x] **`effective_profile()` shim called from 13 existing call sites** — unchanged behavior (None merge layer is no-op)
- [x] **`storage_profile()` / `portable_profile()` must NOT bake collection defaults** — ensured because they call `effective_profile()` (shim) which passes `None`
- [x] **Editor safety**: ProfilesPage never passes `collectionId` to loadProfile — documented invariant
- [x] **Disabled `MetadataStore`** — `get_collection_defaults` returns `Ok(None)` (via `with_conn` + `Option<T>::default()`); `set` silently succeeds as no-op
- [ ] **Concurrent access** — N/A at this layer; `MetadataStore` uses `Mutex<Connection>`, serialized
- [ ] **Permission denied / disk full** — surfaces as `Database { source: ... }`; caller shows error

---

## Validation Commands

### Static Analysis (Rust)

```bash
cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cargo check --manifest-path src/crosshook-native/src-tauri/Cargo.toml
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings
cargo clippy --manifest-path src/crosshook-native/src-tauri/Cargo.toml -- -D warnings
```

**EXPECT**: Zero errors, zero new warnings.

### Rust Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

**EXPECT**: All tests pass. Focus outputs:

```
test profile::models::tests::effective_profile_with_none_equals_shim ... ok
test profile::models::tests::effective_profile_with_merges_collection_defaults_between_base_and_local_override ... ok
test profile::models::tests::effective_profile_with_none_fields_do_not_overwrite_profile ... ok
test profile::models::tests::effective_profile_with_ignores_whitespace_only_method ... ok
test metadata::migrations::tests::migration_19_to_20_adds_defaults_json_column ... ok
test metadata::tests::test_collection_defaults_set_and_get_roundtrip ... ok
test metadata::tests::test_collection_defaults_clear_writes_null ... ok
test metadata::tests::test_collection_defaults_unknown_id_errors_on_set ... ok
test metadata::tests::test_collection_defaults_corrupt_json_returns_corrupt_error ... ok
test metadata::tests::test_collection_defaults_cascades_on_collection_delete ... ok
```

All existing tests pass (`cargo test -p crosshook-core`).

### Full Tauri Build (link check)

```bash
cargo check --manifest-path src/crosshook-native/src-tauri/Cargo.toml --all-targets
```

**EXPECT**: `tauri::generate_handler!` expansion succeeds with the 2 new commands.

### Frontend Static Analysis

```bash
pnpm --dir src/crosshook-native type-check
```

**EXPECT**: Zero type errors in `useProfile.ts`, `useCollectionDefaults.ts`, `CollectionLaunchDefaultsEditor.tsx`, `CollectionViewModal.tsx`, `LaunchPage.tsx`, mock handlers, and `types/profile.ts`.

### Browser Dev-Mode Smoke

```bash
./scripts/dev-native.sh --browser
# Open http://localhost:<port>/, open devtools. In console:
# 1. Set defaults on the seed collection
await callCommand('collection_set_defaults', {
  collectionId: 'mock-collection-1',
  defaults: { method: 'proton_run', custom_env_vars: { DXVK_HUD: '1' } }
})
# 2. Read them back
await callCommand('collection_get_defaults', { collectionId: 'mock-collection-1' })
# 3. Load profile with collection context
await callCommand('profile_load', { name: '<existing-mock-profile>', collectionId: 'mock-collection-1' })
# Verify .launch.custom_env_vars.DXVK_HUD === '1'
# 4. Load without collection context
await callCommand('profile_load', { name: '<existing-mock-profile>' })
# Verify .launch.custom_env_vars.DXVK_HUD is absent
```

**EXPECT**: All calls resolve without `[dev-mock] Unhandled command: ...` errors. Merged vs raw outputs match Rust-side expectations.

### Production Bundle Sentinel (matches CI)

```bash
./scripts/build-native.sh --binary-only
grep -l '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' \
  src/crosshook-native/dist/assets/*.js 2>/dev/null \
  && echo "❌ mock code leaked into production bundle" \
  || echo "✅ no mock code in production bundle"
```

**EXPECT**: `✅ no mock code in production bundle`.

### Database Validation

```bash
# After a fresh launch with the migration:
sqlite3 ~/.local/share/crosshook/metadata.db 'PRAGMA user_version;'
# expect: 20

sqlite3 ~/.local/share/crosshook/metadata.db '.schema collections'
# expect the output to include:
#   defaults_json TEXT
```

### Manual Validation (JTBD-level)

- [ ] Task 20 printenv end-to-end test passes — launch-with-collection shows `CROSSHOOK_PROBE=hello`
- [ ] Launch-without-collection does NOT show the env var
- [ ] Saving empty defaults writes NULL in `defaults_json` (`sqlite3 ... "SELECT defaults_json FROM collections WHERE collection_id = ?"`)
- [ ] Corrupt `defaults_json` surfaces as an error toast in the modal without crashing the app
- [ ] Clicking "Open in Profiles page →" closes the modal and navigates to Profiles with the collection filter intact
- [ ] Editor safety: navigating from LaunchPage (collection-filtered) to ProfilesPage displays raw storage profile fields (no merged env vars)

---

## Acceptance Criteria

- [ ] `CollectionDefaultsSection` serde type exists, derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default`, contains `method`, `optimizations`, `custom_env_vars`, `network_isolation`, `gamescope`, `trainer_gamescope`, `mangohud` as the overrideable subset
- [ ] `effective_profile_with(&self, Option<&CollectionDefaultsSection>)` implemented with base → collection defaults → local_override precedence
- [ ] `effective_profile(&self)` backward-compat shim calls `effective_profile_with(None)`
- [ ] All 13 existing non-test call sites of `effective_profile()` continue to compile and behave identically
- [ ] Schema v19 → v20 migration adds `collections.defaults_json TEXT NULL`
- [ ] `MetadataStore::get_collection_defaults` / `set_collection_defaults` implemented, with corrupt-JSON → `Corrupt` and missing-collection → `Validation` error semantics
- [ ] `empty defaults` (via `is_empty`) normalized to NULL on write
- [ ] 2 new Tauri commands `collection_get_defaults`, `collection_set_defaults` registered in `tauri::generate_handler!`
- [ ] `profile_load` extended to accept `collection_id: Option<String>`; backward-compat when not passed
- [ ] Browser dev-mode mocks for 2 new commands + `profile_load` mock extended to merge mock defaults
- [ ] `collection_get_defaults` added to `wrapHandler.ts` `EXPLICIT_READ_COMMANDS`
- [ ] TypeScript `CollectionDefaults` interface + `isCollectionDefaultsEmpty` helper exported from `types/profile.ts`
- [ ] `useProfile.loadProfile` accepts `collectionId?: string` in options and forwards to IPC (empty string normalized to undefined)
- [ ] `useCollectionDefaults(collectionId)` hook created, mirrors `useCollectionMembers` race-safety
- [ ] `<CollectionLaunchDefaultsEditor>` component created: method dropdown, network_isolation dropdown, env var table, Save/Reset/Open in Profiles page buttons
- [ ] `<CollectionLaunchDefaultsEditor>` wired into `<CollectionViewModal>` body above the search input
- [ ] `onOpenInProfilesPage` wired in `App.tsx` — closes modal + navigates to profiles, activeCollectionId preserved
- [ ] `LaunchPage`'s Active-Profile dropdown onChange passes `activeCollectionId` to `loadProfile`
- [ ] `ProfilesPage`'s `loadProfile` call sites do NOT pass `collectionId` (editor safety)
- [ ] **10 new Rust tests** green (4 merge layer, 1 migration, 5 metadata store)
- [ ] All existing tests continue to pass (no regressions on `effective_profile` precedence, no regressions on `storage_profile` round-trip, no regressions on any Phase 1 collection test)
- [ ] `cargo test -p crosshook-core` zero failures
- [ ] `cargo check` on both crates zero warnings
- [ ] `pnpm --dir src/crosshook-native type-check` passes
- [ ] Browser dev-mode: defaults round-trip end-to-end via mock layer
- [ ] Manual printenv end-to-end test (Task 20) passes: collection-launched profile gets collection env vars; Library-launched profile does not
- [ ] No `[dev-mock]` strings in the production bundle
- [ ] Documentation: collection edit modal displays precedence hint "Local machine paths always win — collection defaults apply on top of profile config but below your local overrides"

## Completion Checklist

- [ ] Code follows discovered patterns: free function + `with_conn` wrapper + `#[tauri::command]` handler; React hook mirrors `useCollectionMembers`
- [ ] Error handling uses `MetadataStoreError::Validation(String)` and `::Corrupt(String)` tuple variants — never struct syntax
- [ ] All mock error messages start with `[dev-mock]`
- [ ] Tests mirror `effective_profile_prefers_local_override_paths` and `test_rename_collection_updates_name` structures
- [ ] No hardcoded schema version constants introduced
- [ ] No new npm / Cargo dependencies added
- [ ] `#[allow(dead_code)]` NOT re-introduced anywhere
- [ ] `storage_profile` / `portable_profile` not touched; still call `effective_profile()` shim (passing None)
- [ ] All 13 existing non-test call sites of `effective_profile()` unchanged
- [ ] `ProfilesPage.tsx` loadProfile call sites unchanged (editor safety)
- [ ] `remove_profile_from_collection` unchanged (Phase 1 behavior preserved)
- [ ] Commit follows Conventional Commits:
  - `feat(core): collection-defaults serde type + effective_profile_with merge layer`
  - `feat(core): schema v20 migration — collections.defaults_json column`
  - `feat(core): get/set collection defaults metadata store methods`
  - `feat(core): collection_get_defaults / collection_set_defaults IPC + profile_load collection_id`
  - `feat(ui): CollectionLaunchDefaultsEditor inline editor in CollectionViewModal`
  - `feat(ui): LaunchPage threads activeCollectionId into loadProfile`
  - `feat(ui): browser dev-mode mocks for collection defaults`
  - Alternatively, a single `feat(ui): per-collection launch defaults` grouped commit linking `#179` is acceptable
- [ ] Label the PR: `type:feature`, `area:profiles`, `priority:high`. Link with `Closes #179`.
- [ ] Self-contained — no questions needed during implementation

## Risks

| Risk                                                                                                                                                                                                                                                                                 | Likelihood | Impact     | Mitigation                                                                                                                                                                                                                                                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------- | ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Editor safety**: ProfilesPage inadvertently loads a merged profile and the user saves merged fields into the storage TOML, corrupting the profile                                                                                                                                  | **Medium** | **High**   | Task 19 GOTCHA documents the invariant. Task 14 normalizes empty-string collectionId. Add a code-level comment at ProfilesPage's loadProfile call. Consider adding a runtime assertion in `loadProfile` that logs a warning when `collectionId` is passed and `route === 'profiles'` (defer to Phase 5 polish).                           |
| **`storage_profile()` inadvertently materializes collection defaults**                                                                                                                                                                                                               | **Low**    | **High**   | `storage_profile()` calls `effective_profile()` (shim) → `effective_profile_with(None)`. `None` merge layer is a no-op. Existing test `storage_profile_roundtrip_is_idempotent` acts as a regression guard and will fail if this invariant breaks.                                                                                        |
| **Precedence order accidentally swapped** (collection defaults applied after `local_override`)                                                                                                                                                                                       | **Medium** | **High**   | Task 2 IMPLEMENT block explicitly orders the layers; Task 3 adds `effective_profile_with_merges_collection_defaults_between_base_and_local_override` regression test. Existing `effective_profile_prefers_local_override_paths` test also catches this.                                                                                   |
| **Phase 2 lands an intermediate migration before Phase 3 merges** (v19→v19.5), shifting the Phase 3 target version                                                                                                                                                                   | **Low**    | **Medium** | Task 4 GOTCHA requires verifying the latest migration via `grep` before writing `migrate_19_to_20`. If Phase 2 adds something, rename the function + dispatch block. No hardcoded `20_u32` anywhere except in `pragma_update` and the test.                                                                                               |
| **`custom_env_vars` additive merge regression** — implementer uses `merged.launch.custom_env_vars = d.custom_env_vars.clone()` (replacement, not merge)                                                                                                                              | **Medium** | **High**   | Task 2 GOTCHA spells this out; Task 3 `effective_profile_with_merges_collection_defaults_between_base_and_local_override` test asserts `PROFILE_ONLY` key retention AND `COLLECTION_ONLY` injection — a replacement-only impl would fail the `PROFILE_ONLY` assertion.                                                                    |
| **`collection_get_defaults` omitted from `wrapHandler.ts` read allow-list**, causing writes to be blocked when the debug-toggle is active                                                                                                                                            | **Low**    | **Low**    | Task 12 explicitly adds it. Alternative: rename to `get_collection_defaults` to match the regex, but this breaks Phase 1's `collection_*` prefix convention.                                                                                                                                                                              |
| **Mock `profile_load` drift**: the browser mock does NOT merge defaults, and dev-mode behavior diverges from real Tauri behavior                                                                                                                                                     | **Medium** | **Medium** | Task 11 explicitly extends the `profile_load` mock handler to apply `applyMockCollectionDefaults`. Manual smoke test in Validation verifies round-trip.                                                                                                                                                                                   |
| **Corrupt `defaults_json`** blocks all collection-context loads of the profile, creating a "stuck collection" UX                                                                                                                                                                     | **Low**    | **Medium** | `Corrupt` error surfaces to the frontend; user can clear the defaults via the editor (which writes `null` → NULL). **Additionally**, the editor should catch the `Corrupt` on initial load and offer a "reset defaults" affordance. Task 15 initial hook does NOT do this; add it as a follow-up in Phase 5 polish.                       |
| **`MetadataStoreError::Corrupt` is not yet used for JSON deserialization errors elsewhere** — this is a NEW usage pattern                                                                                                                                                            | **Low**    | **Low**    | The variant already exists (`metadata/models.rs:1-60`). Task 6 uses it; if code review prefers a different variant, `Database { action: "parse collection defaults", source: ... }` is an acceptable fallback — serde_json errors don't cleanly map to `rusqlite::Error` though, so a wrapper conversion is needed. `Corrupt` is cleaner. |
| **`tauri::generate_handler!` macro error** on missing comma after Phase 3 commands                                                                                                                                                                                                   | **Low**    | **Low**    | Task 10 snippet includes trailing commas; `cargo check` catches it immediately.                                                                                                                                                                                                                                                           |
| **Env var key renaming in the editor** triggers React re-render churn because the `key` prop changes                                                                                                                                                                                 | **Low**    | **Low**    | Use the stable initial key as the React `key` prop; accept the re-mount on rename. For power users editing large env var lists, this may feel laggy — defer performance tuning to Phase 5 polish.                                                                                                                                         |
| **Deep-link to Profiles page with collection filter active but profile not a member** — user clicks "Open in Profiles page →" from a collection defaults editor, ProfilesPage filters by the collection but the currently-selected profile is not a member; user sees an empty state | **Low**    | **Low**    | Phase 2's ProfilesPage filter logic handles this; the fallback behavior is documented in Phase 2 Task 17 ("`memberNames.length === 0` falls back to unfiltered"). Phase 3 inherits this safety.                                                                                                                                           |
| **Opening the modal to a collection with corrupt JSON** causes the editor to render in an error state without a clear recovery path                                                                                                                                                  | **Low**    | **Medium** | Task 15's `useCollectionDefaults` surfaces the error via `error` state; Task 16's editor displays it. **Improvement**: add a "reset defaults" button in the error case that calls `saveDefaults(null)` — defer to Phase 5 polish.                                                                                                         |
| **`profile_load` new `collection_id` parameter breaks existing browser mock signature**, causing type errors in mock consumers                                                                                                                                                       | **Low**    | **Medium** | Task 11 updates the mock signature explicitly; the mock accepts both old-shape (no `collectionId`) and new-shape (`{ name, collectionId }`) calls via optional field.                                                                                                                                                                     |
| **`profile_load` signature change ripples to useUpdateGame / useGameDetailsProfile** (other callers of `profile_load` in the frontend)                                                                                                                                               | **Low**    | **Low**    | The Tauri command accepts the `collection_id` field as `Option`. Callers that don't pass it continue to work unchanged. No explicit update needed for `useUpdateGame.ts` or `useGameDetailsProfile.ts`. Task 14's `loadProfile` is the only TS path that gains the option, and only LaunchPage uses it.                                   |
| **`tauri::Builder::manage(metadata_store)`** not cloneable into the new `profile_load` handler                                                                                                                                                                                       | **Low**    | **Low**    | Phase 1 already uses `MetadataStore` in multiple command handlers; the `State<'_, MetadataStore>` injection works. Verified at `lib.rs:201`.                                                                                                                                                                                              |
| **Mock fixture profiles** don't match any real profile name, so the end-to-end test in browser dev-mode can't exercise the merge against a real profile                                                                                                                              | **Low**    | **Low**    | Add a `mock-profile-for-defaults` fixture to the `profile.ts` mock handler if needed; reuse an existing fixture profile otherwise. Task 11 documents this.                                                                                                                                                                                |

---

## Notes

### Key divergences from the issue body / PRD

- **PRD says "extend `profile_load`"**; Phase 3 does exactly that — `profile_load` gains optional `collection_id`. Frontend callers that don't pass the ID get identical behavior to pre-Phase 3.
- **Issue #179 says "schema v20 (or v21 if Phase 2 adds any intermediate migrations — TBD)"**. Phase 2 is frontend-only and adds NO migration, so Phase 3 targets v20 exactly. Task 4 requires verifying this assumption before starting.
- **PRD lists `CollectionDefaultsSection` fields as `custom_env_vars`, `optimizations`, `gamescope`, `mangohud`, `method`**. Phase 3 expands this to also include `trainer_gamescope` (mirror of `gamescope` for trainer) and `network_isolation` — both are `LaunchSection` fields and both should be overrideable for symmetry. Excluding `trainer_gamescope` while including `gamescope` would be surprising. `network_isolation` was added because a power user testing in an isolated-network collection ("offline-only profiles") needs this knob.
- **PRD lists `collection_launch_defaults` table "OR" an inline column**. Phase 3 picks **inline `defaults_json TEXT`** for the reasons documented in Storage / Persistence. A future phase can migrate to a normalized table if per-field querying becomes necessary without breaking the column.
- **PRD says "`profile_load_with_collection(name, collection_id)` IPC OR extension of `profile_load`"**. Phase 3 picks the **extension** path, per the PRD's own explicit recommendation.

### Things that look concerning but are actually fine

- **The shim `effective_profile(&self) -> Self { self.effective_profile_with(None) }` looks like a hack** — it's not. This is the idiomatic Rust pattern for extending a public API without breaking callers. `None` is a zero-cost no-op at the merge layer; the monomorphized version compiles to the same assembly as the original function.
- **`profile_load` returning a merged profile when `collection_id` is passed looks like hidden state** — it's explicit: the caller chose to pass the collection context and knows they're getting a merged view. Editor-safety is enforced at the call site, not in the IPC layer.
- **Extending `profile_load` instead of a new command has no backward-compat risk** — Tauri deserializes missing `Option<T>` as `None`, so every existing caller continues to work with zero changes.
- **`BTreeMap` additive merge vs replacement could confuse users** — they'll see "my profile's env var `X=1` is still there after I add `X=2` to the collection defaults". Mitigation: the editor could show a "merged preview" feature in Phase 5 polish. v1 documents the behavior inline ("Custom env vars (additive)") in the editor's fieldset legend.

### Things that actually are concerning

- **No existing test in the Rust crate exercises `effective_profile` with `custom_env_vars` non-empty** — so a regression in the `custom_env_vars` merge path wouldn't have been caught pre-Phase 3. Task 3's `effective_profile_with_merges_collection_defaults_between_base_and_local_override` test is the first such assertion. If implementers break the `custom_env_vars` merge semantics during subsequent refactors, this test is the only guard. Consider promoting it to the "don't delete" class of regression tests.
- **The "Open in Profiles page →" deep-link preserves `activeCollectionId`** which means the user lands in a collection-filtered ProfilesPage view. If the currently-loaded profile is NOT a member of the collection, the filter logic from Phase 2 (`memberNames.length === 0 → fallback to unfiltered`) kicks in. This is the right behavior but worth validating manually.

### Conventional Commit suggestions

```text
feat(core): collection launch defaults serde type and effective_profile merge layer
feat(core): schema v20 — collections.defaults_json inline JSON column
feat(core): metadata get_collection_defaults / set_collection_defaults
feat(core): collection_get_defaults / collection_set_defaults IPC + profile_load collection_id
feat(ui): CollectionLaunchDefaultsEditor inline editor inside CollectionViewModal
feat(ui): LaunchPage threads activeCollectionId into loadProfile
feat(ui): browser dev-mode mocks for collection defaults and extended profile_load
```

One grouped PR with all commits + `Closes #179` is also acceptable. Tag with `type:feature`, `area:profiles`, `priority:high`.

### Future phases that depend on Phase 3

- **Phase 4 (TOML export/import)** — requires: `CollectionDefaultsSection` serde type (serialized into the export TOML), `get_collection_defaults`/`set_collection_defaults` for the import path, and the `defaults_json` column to hold imported defaults. Phase 3's JSON-in-SQLite storage and Phase 4's TOML-on-disk wire format are orthogonal — Phase 4 serializes `CollectionDefaultsSection` differently (TOML with `schema_version = "1"` tag) but the Rust type is the same.
- **Phase 5 (polish + Steam Deck validation)** — requires: end-to-end printenv test from Task 20 as a reproducible fixture; keyboard/D-pad navigation audit of the new inline editor; empty-state copy for "no defaults set"; corrupt-JSON recovery affordance in the editor.

### PRD updates required after this plan is written

1. Mark Phase 3 status as `in-progress` in `docs/prps/prds/profile-collections.prd.md` Implementation Phases table.
2. Set the PRP column to `[profile-collections-phase-3-launch-defaults.plan.md](../plans/profile-collections-phase-3-launch-defaults.plan.md)`.
3. No other PRD changes — the Decisions Log and Storage / Persistence sections already match Phase 3's approach.
