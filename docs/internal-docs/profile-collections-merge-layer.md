# Profile Collections — Merge Layer

CrossHook resolves launch configuration for a game through a 3-layer precedence model. When a profile is viewed or launched from within a collection, the backend merges fields from three sources in order: the **base profile** (layer 1), optional **collection defaults** (layer 2), and the **local override** (layer 3). Each subsequent layer wins on collision, and fields without a value in a higher layer inherit from the layer below.

## CollectionDefaultsSection

Defined at `src/crosshook-native/crates/crosshook-core/src/profile/models.rs:409-424`.

| Field                | Type                               | Semantics                                                                                          |
| -------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------- |
| `method`             | `Option<String>`                   | Launch method override (`"proton_run"`, `"native"`, etc.). Whitespace-only values are ignored.     |
| `optimizations`      | `Option<LaunchOptimizationsSection>` | Replaces the profile's launch optimizations wholesale when `Some`.                                |
| `custom_env_vars`    | `BTreeMap<String, String>`         | Additive merge with profile env vars. Collection keys win on collision; profile-only keys survive. |
| `network_isolation`  | `Option<bool>`                     | Overrides the profile's network isolation toggle when `Some`.                                      |
| `gamescope`          | `Option<GamescopeConfig>`          | Overrides the profile's gamescope config when `Some`.                                              |
| `trainer_gamescope`  | `Option<GamescopeConfig>`          | Overrides the profile's trainer gamescope config when `Some`.                                      |
| `mangohud`           | `Option<MangoHudConfig>`           | Overrides the profile's MangoHUD config when `Some`.                                               |

Every `Option<T>` field means "inherit from profile when `None`, replace when `Some`". Fields excluded from collection-level override by design: `presets` and `active_preset` (preset coupling is too complex to override at the collection level).

## Effective Profile Merge

The merge is performed by `GameProfile::effective_profile_with(defaults: Option<&CollectionDefaultsSection>)` at `models.rs:573`. The 3-layer precedence is:

```
base profile (TOML on disk)
  -> collection defaults (SQLite defaults_json column)
    -> local_override (machine-specific paths, always wins last)
```

1. **Layer 1 — Base profile**: The portable profile as stored on disk in TOML (`ProfileStore` / `GameProfile` fields such as `game`, `launch`, etc.) **before** any `local_override` paths are applied. `local_override` is **not** part of layer 1; it is merged in **layer 3** inside `GameProfile::effective_profile_with` (and `effective_profile()`), so collection defaults are applied **between** the base profile and `local_override`.
2. **Layer 2 — Collection defaults**: Each non-`None` field in `CollectionDefaultsSection` replaces the corresponding base profile field. `custom_env_vars` uses an additive merge: collection entries are unioned with the profile's `launch.custom_env_vars`, and collection keys win on collision. Profile keys without a collision are preserved.
3. **Layer 3 — Local override**: Machine-specific overrides from `local_override.*` (paths, etc.) always win last, after collection defaults.

When `defaults` is `None`, `effective_profile_with(None)` skips the collection-defaults layer and **otherwise matches** the pre-Phase 3 `effective_profile()` shim (still `base + local_override` via the same method).

## Editor-safety invariant

`useProfile.loadProfile(name, { collectionId })` **MUST NOT** be called from `ProfilesPage`. `ProfilesPage` calls `selectProfile(name)` without a `collectionId`. This prevents the editor from accidentally writing collection-merged values back to the base profile on disk.

The collection-aware load path is used only by `CollectionViewModal` and similar read-only views where the merged profile is displayed but never persisted. The hook alias `selectProfile` is defined as `loadProfile` at `useProfile.ts:666`, and the `collectionId` parameter is threaded through to the Rust `profile_load` IPC command only when explicitly provided.

## Merge-layer tests

Four tests in `src/crosshook-native/crates/crosshook-core/src/profile/models.rs:1288-1380` cover the merge layer:

| Test name                                                                 | Lines       | Validates                                                                 |
| ------------------------------------------------------------------------- | ----------- | ------------------------------------------------------------------------- |
| `effective_profile_with_none_equals_shim`                                 | 1288-1299   | Passing `None` produces the same result as the existing shim              |
| `effective_profile_with_merges_collection_defaults_between_base_and_local_override` | 1302-1339   | Collection defaults apply between base and local override; env var collision semantics |
| `effective_profile_with_none_fields_do_not_overwrite_profile`             | 1342-1365   | Empty defaults (all `None`) leave the profile unchanged                   |
| `effective_profile_with_ignores_whitespace_only_method`                   | 1368-1380   | Whitespace-only `method` does not clobber the profile's launch method     |

Run with:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core effective_profile_with
```

## Schema v20 migration

Schema v20 added the `defaults_json TEXT` column to the `collections` table via an `ALTER TABLE` migration in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:865-891`. The column is nullable; `NULL` means "no collection defaults". The migration is additive and non-destructive (no data loss on upgrade from v19).

The migration test `migration_19_to_20_adds_defaults_json_column` at `migrations.rs:1259` verifies column type, nullability, JSON round-trip, `NULL` round-trip, and `user_version == 20`.

## Extended IPC signature

The `profile_load` Tauri command accepts an optional `collection_id: Option<String>` parameter. When provided, the backend:

1. Loads the base profile from disk via `ProfileStore`.
2. Retrieves collection defaults from SQLite via `MetadataStore::get_collection_defaults`.
3. Calls `profile.effective_profile_with(defaults.as_ref())` to produce the merged view.
4. Returns the merged profile to the frontend.

When `collection_id` is missing or empty, behavior is identical to pre-Phase 3 (backward compatible). The extended signature is defined at `src/crosshook-native/src-tauri/src/commands/profile.rs`.
