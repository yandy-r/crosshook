# Plan: Profile Collections - Phase 4 (TOML Export / Import Preset)

## Summary

Make profile collections shareable across machines by adding a `*.crosshook-collection.toml` wire format, a Rust export/import-preview pipeline, thin Tauri commands, and a frontend review flow that lets users resolve ambiguous matches before committing anything to SQLite. This phase builds directly on the current branch state: Phase 2's collection UI already exists, Phase 3's `CollectionDefaultsSection` and `collection_get_defaults` / `collection_set_defaults` are live, and the remaining work is to turn that local state into a human-editable preset file and a safe import experience.

The implementation should stay deliberately bounded: no schema migration, no new SQLite tables, no new dependencies, and no "apply import" backend transaction. Export writes a TOML preset to disk. Import parses TOML into a preview with `matched` / `ambiguous` / `unmatched` buckets, and the review modal commits through the existing collection CRUD/defaults IPC surface with best-effort rollback if any later write fails.

## User Story

As a power user who already organizes profiles into collections, I want to export a collection from one machine and import it on another, so that my grouping and collection-level launch defaults travel with me without manually rebuilding the collection.

## Problem -> Solution

Current state: collections and collection defaults are now local-only. `CollectionViewModal` can edit a collection and its defaults, but there is no export action, no sidebar import entrypoint, no collection preset schema, and no preview/review flow for matching imported descriptors back to local profiles. The only existing share/import precedent is the community profile pipeline.

Desired state: a user can click `Export preset...` from the collection modal, save a `.crosshook-collection.toml` file containing collection metadata + defaults + profile descriptors, then use `Import preset...` from the Collections sidebar to preview matches, resolve ambiguous entries, skip unmatched ones, and persist the imported collection through the existing Phase 1/3 commands.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/profile-collections.prd.md`
- **PRD Phase**: Phase 4 - TOML export / import preset
- **Source Issue**: GitHub `yandy-r/crosshook#180`
- **Depends on**: Phase 1 (collection CRUD IPC + mocks), Phase 3 (collection defaults persistence + `profile_load(collectionId)`)
- **Current branch reality**: `CollectionsSidebar`, `CollectionViewModal`, `CollectionLaunchDefaultsEditor`, `CollectionDefaultsSection`, `useCollectionDefaults`, and the current collection mock domain already exist on this branch
- **Estimated Files**: 14

## Storage / Persistence

Phase 4 adds no new persistent application storage. It introduces a wire format and an import-review runtime state only.

| Datum / behavior                                             | Classification             | Where it lives                                                                                                                       | Migration / compatibility                          |
| ------------------------------------------------------------ | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------- |
| Exported `*.crosshook-collection.toml` preset                | Wire format on disk        | User-chosen filesystem path via `chooseSaveFile(...)`                                                                                | New v1 schema with explicit `schema_version = "1"` |
| Import preview buckets (`matched`, `ambiguous`, `unmatched`) | Runtime-only               | React state inside the import review flow                                                                                            | Reset on modal close                               |
| Imported collection row, membership, and defaults            | SQLite metadata (existing) | Persisted through existing `collection_create`, `collection_update_description`, `collection_add_profile`, `collection_set_defaults` | No schema change in Phase 4                        |
| Ambiguous-entry user resolutions                             | Runtime-only               | Local component state in the review modal                                                                                            | Reset on modal close                               |

**Migration**: None. Phase 4 must not add or change SQLite schema.

**Backward compatibility**: Presets are versioned with a string `schema_version`. Future versions must reject unsupported schema strings with an actionable error instead of partially importing.

**Offline behavior**: Fully offline. Export writes to the local filesystem; import reads from the local filesystem; all persistence uses existing local Tauri/SQLite commands.

**Degraded behavior**:

- If the file dialog is canceled, do nothing.
- If the TOML file is unreadable or malformed, surface a friendly error and do not write any state.
- If some imported descriptors do not match local profiles, show them as `unmatched` and let the user skip them.
- If commit fails after a collection has been created, attempt a best-effort `collection_delete(createdId)` rollback and show the original error.

**User visibility / editability**: Exported TOML is intentionally human-editable. Collection data remains SQLite-backed in-app; the preset file is the user-facing sharing/editing escape hatch.

---

## UX Design

### Before

```
+------------------------------------------------------+
| Collections sidebar                                  |
|   Action / Adventure                                 |
|   Stable                                             |
|   WIP                                                |
|   [+] New Collection                                 |
+------------------------------------------------------+
| CollectionViewModal                                  |
|   Edit | Close                                       |
|   [Collection launch defaults editor]                |
|   Search this collection                             |
|   ... member cards ...                               |
|   Delete collection | Done                           |
+------------------------------------------------------+
| There is no export action, no import entrypoint,     |
| and no way to share or recreate a collection on      |
| another machine.                                     |
+------------------------------------------------------+
```

### After

```
+------------------------------------------------------+
| Collections sidebar                                  |
|   Action / Adventure                                 |
|   Stable                                             |
|   WIP                                                |
|   [+] New Collection                                 |
|   [>] Import preset...                               |
+------------------------------------------------------+
| CollectionViewModal                                  |
|   Edit | Close                                       |
|   [Collection launch defaults editor]                |
|   Search this collection                             |
|   ... member cards ...                               |
|   Export preset... | Delete collection | Done        |
+------------------------------------------------------+
| CollectionImportReviewModal                          |
|   Imported collection: Action / Adventure            |
|   Matched: 6                                         |
|   Ambiguous: 2  -> choose local profile per entry    |
|   Unmatched: 1  -> skip                              |
|   Defaults: included                                 |
|   Cancel | Import collection                         |
+------------------------------------------------------+
```

### Interaction Changes

| Touchpoint                   | Before                               | After                                                                                 | Notes                                                           |
| ---------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `CollectionsSidebar`         | Only create/open collection          | Adds `Import preset...` entrypoint                                                    | Available even when no collection is open                       |
| `CollectionViewModal` footer | Delete + Done only                   | Adds `Export preset...` action                                                        | Export always targets the currently opened collection           |
| Import flow                  | n/a                                  | `chooseFile(...)` -> preview command -> review modal -> existing CRUD/defaults commit | No writes happen before review confirmation                     |
| Ambiguous profile matches    | n/a                                  | User must resolve or skip each ambiguous descriptor                                   | Confirm button stays disabled until resolved                    |
| Unmatched profiles           | n/a                                  | Displayed explicitly and can be skipped                                               | No stub profiles are created                                    |
| Browser dev mode             | No import/export collection handlers | Import/export preview works through mocked handlers                                   | `collection_import_from_toml` must be treated as a read command |

---

## Implementation Strategy

- **Approach**: Add a dedicated collection-preset exchange layer in `crosshook-core` as a sibling to the existing community profile exchange pipeline, then expose it through two new collection commands and extend the current collections frontend/context to drive file dialogs, preview, review, and commit.
- **Why this approach**: It mirrors the repo's strongest existing precedent (`community_schema.rs` + `exchange.rs`) while keeping collection preset logic isolated from community profile logic and from SQLite persistence helpers.
- **Scope**: Export a collection preset, preview an import from TOML, review/resolve descriptors in the UI, commit via existing collection/defaults commands, add browser mocks, and add focused Rust tests.
- **Not in this phase**: No new migration, no transactional backend `apply import` command, no import into an existing collection, no auto-created stub profiles, no dynamic/smart matching beyond the documented descriptor precedence, no extra metadata beyond what the issue requires.

### Alternatives Considered

| Alternative                                                            | Rejected because                                                                                                                                                                        |
| ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Extend `src/profile/exchange.rs` directly with collection preset logic | It would mix two unrelated wire formats into one file and make Phase 4 harder to reason about than a sibling `collection_exchange.rs` / `collection_schema.rs` pair                     |
| Match imported descriptors through metadata DB only                    | `lookup_profile_id(...)` in `metadata/profile_sync.rs` only resolves filename -> profile id; it does not expose `steam.app_id`, `game.name`, or `trainer.community_trainer_sha256`      |
| Add a new backend `collection_apply_import` transaction command        | The issue/PRD explicitly scope persistence through existing Phase 1/3 IPC commands; adding a transactional backend command would widen the phase and duplicate already-shipped surfaces |
| Silent best-effort import with no review modal                         | Explicitly rejected by the issue; ambiguous/unmatched matches must be user-visible                                                                                                      |

---

## Mandatory Reading

Read these files before implementation starts. The plan assumes this context is loaded and does not expect any extra codebase spelunking mid-implementation.

| Priority | File                                                                         | Lines                         | Why                                                                                                       |
| -------- | ---------------------------------------------------------------------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------- |
| P0       | `docs/prps/prds/profile-collections.prd.md`                                  | `261-272`, `317-322`          | Exact Phase 4 scope, storage boundary, and persistence constraints                                        |
| P0       | `docs/prps/archived/profile-collections-phase-3-launch-defaults.plan.md`     | `1-60`                        | Confirms the current defaults model and the already-shipped Phase 3 boundary                              |
| P0       | `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`              | all                           | Shows where new collection preset modules must be exported                                                |
| P0       | `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` | `5-83`                        | Best precedent for schema constants + manifest struct design                                              |
| P0       | `src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`         | `20-41`, `132-238`, `582-659` | Error enum, preview/import/export split, and roundtrip test structure to mirror                           |
| P0       | `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`       | `503-524`                     | Existing shareable TOML writer pattern                                                                    |
| P0       | `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`           | `768-778`                     | `resolve_art_app_id(...)` defines the effective app-id semantics Phase 4 must serialize and match against |
| P0       | `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs`     | `338-412`                     | Current collection defaults read/write surface that Phase 4 must roundtrip through unchanged              |
| P0       | `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`    | `72-86`                       | Confirms metadata lookup is filename-only and cannot drive descriptor matching                            |
| P0       | `src/crosshook-native/src-tauri/src/commands/collections.rs`                 | all                           | Canonical placement and naming for new collection IPC commands                                            |
| P0       | `src/crosshook-native/src-tauri/src/lib.rs`                                  | `281-291`                     | Command registration block that must include the new Phase 4 commands                                     |
| P0       | `src/crosshook-native/src/context/CollectionsContext.tsx`                    | all                           | Existing collection domain context that should own export/import preview/apply helpers                    |
| P0       | `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx`     | all                           | Import entrypoint location                                                                                |
| P0       | `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`    | `255-385`                     | Export button insertion point and modal state integration                                                 |
| P0       | `src/crosshook-native/src/components/ProfileReviewModal.tsx`                 | `112-382`                     | Modal shell, focus trap, body lock, and footer layout to reuse                                            |
| P1       | `src/crosshook-native/src/components/CommunityImportWizardModal.tsx`         | `168-203`                     | Pattern for resetting review/import state when a modal opens                                              |
| P1       | `src/crosshook-native/src/utils/dialog.ts`                                   | all                           | Canonical file dialog helpers; do not copy the direct `open(...)` pattern from `CommunityBrowser.tsx`     |
| P1       | `src/crosshook-native/src/components/pages/ProfilesPage.tsx`                 | `458-494`                     | Concrete save-dialog -> export command flow                                                               |
| P1       | `src/crosshook-native/src/hooks/useCommunityProfiles.ts`                     | `350-383`                     | Existing preview helper pattern and error propagation style                                               |
| P1       | `src/crosshook-native/src/lib/mocks/handlers/collections.ts`                 | `98-267`                      | Existing collection mock style and `[dev-mock]` error convention                                          |
| P1       | `src/crosshook-native/src/lib/mocks/wrapHandler.ts`                          | `42-80`                       | `collection_import_from_toml` must be allow-listed as a read command                                      |
| P1       | `src/crosshook-native/src/types/collections.ts`                              | all                           | Existing collection type home; extend it instead of creating parallel type files                          |
| P1       | `src/crosshook-native/src-tauri/src/commands/profile.rs`                     | `314-346`                     | Existing aggregate-read behavior for corrupt profiles: warn and skip instead of failing the whole list    |
| P2       | `src/crosshook-native/src/components/CommunityBrowser.tsx`                   | `75-88`                       | Demonstrates the older direct `open(...)` pattern that Phase 4 should NOT copy                            |
| P2       | `src/crosshook-native/src/hooks/useProfile.ts`                               | `598-607`                     | Confirms current `collectionId` normalization conventions remain untouched by Phase 4                     |

## External Documentation

No external research needed. Phase 4 uses established internal patterns plus already-present Rust/TypeScript/Tauri/TOML dependencies.

---

## Patterns to Mirror

These snippets are real codebase patterns. Follow the structure, naming, and error boundaries exactly.

### TYPE_CONTRACT

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs:47-58
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommunityProfileManifest {
    #[serde(
        default = "default_schema_version",
        rename = "schema_version",
        skip_serializing_if = "is_default_schema_version"
    )]
    pub schema_version: u32,
    #[serde(default)]
    pub metadata: CommunityProfileMetadata,
    #[serde(default)]
    pub profile: GameProfile,
}
```

Phase 4 should mirror the dedicated manifest-type pattern, but use a string `schema_version` because issue `#180` explicitly requires `schema_version = "1"` in TOML.

### ERROR_HANDLING

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs:20-41
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommunityExchangeError {
    Io { action: String, path: PathBuf, message: String },
    Json { path: PathBuf, message: String },
    InvalidManifest { message: String },
    UnsupportedSchemaVersion { version: u32, supported: u32 },
    ProfileStore { message: String },
}
```

Use a dedicated collection-preset error enum instead of `String`ly-typed core errors. Tauri can still flatten it at the command boundary.

### PREVIEW_THEN_COMMIT

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs:105-166
pub fn import_community_profile(
    json_path: &Path,
    profiles_dir: &Path,
) -> Result<CommunityImportResult, CommunityExchangeError> {
    let preview = preview_community_profile_import(json_path)?;
    let profile_name = preview.profile_name.clone();
    let manifest = preview.manifest.clone();
    let mut profile = preview.profile.clone();
    // ...
    let store = ProfileStore::with_base_path(profiles_dir.to_path_buf());
    store.save(&profile_name, &profile)?;
    Ok(CommunityImportResult { profile_name, source_path: json_path.to_path_buf(), profile_path: profiles_dir.join(format!("{profile_name}.toml")), profile, manifest })
}

pub fn preview_community_profile_import(
    json_path: &Path,
) -> Result<CommunityImportPreview, CommunityExchangeError> {
    let content = fs::read_to_string(json_path).map_err(|error| CommunityExchangeError::Io {
        action: "read the community profile JSON".to_string(),
        path: json_path.to_path_buf(),
        message: error.to_string(),
    })?;
    // ...
    Ok(CommunityImportPreview { profile_name, source_path: json_path.to_path_buf(), profile: hydrated_profile, manifest, required_prefix_deps })
}
```

Mirror the split between parse/preview and commit, but keep Phase 4's commit in the frontend through existing collection commands.

### TOML_EXPORT_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs:508-524
pub fn profile_to_shareable_toml(
    name: &str,
    profile: &GameProfile,
) -> Result<String, toml::ser::Error> {
    let toml_body = toml::to_string_pretty(profile)?;
    Ok(format!(
        "# CrossHook Profile: {name}\n\
         # https://github.com/yandy-r/crosshook\n\
         #\n\
         # To use this profile, save this file as:\n\
         #   ~/.config/crosshook/profiles/{name}.toml\n\
         #\n\
         # Then select the profile in CrossHook.\n\
         \n\
         {toml_body}"
    ))
}
```

Collection export should use `toml::to_string_pretty(...)` and can add a short comment header, but the serialized body must remain valid TOML.

### APP_ID_RESOLUTION

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models.rs:768-778
pub fn resolve_art_app_id(profile: &GameProfile) -> &str {
    let steam = profile.steam.app_id.trim();
    if !steam.is_empty() {
        return steam;
    }
    profile.runtime.steam_app_id.trim()
}
```

Phase 4 should serialize and match the preset field named `steam_app_id` using this effective-app-id resolution rule, not just `profile.steam.app_id`.

### TAURI_COMMAND_PATTERN

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/collections.rs:98-117
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

Keep collection commands thin: trim/validate inputs at the edge, call the core helper, then `map_err(map_error)`.

### LOGGING_PATTERN

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/community.rs:108-119
if let Err(e) = metadata_store.observe_profile_write(
    &result.profile_name,
    &result.profile,
    &result.profile_path,
    SyncSource::Import,
    None,
) {
    tracing::warn!(
        %e,
        profile_name = %result.profile_name,
        "metadata sync after community_import_profile failed"
    );
}
```

Use `tracing::warn!` with structured fields when a non-fatal side effect fails.

### REVIEW_MODAL_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/ProfileReviewModal.tsx:143-187,300-381
useEffect(() => {
  const host = document.createElement('div');
  host.className = 'crosshook-modal-portal';
  portalHostRef.current = host;
  document.body.appendChild(host);
  setIsMounted(true);
  return () => {
    host.remove();
    portalHostRef.current = null;
    setIsMounted(false);
  };
}, []);

return createPortal(
  <div className="crosshook-modal" role="presentation">
    <div className="crosshook-modal__backdrop" aria-hidden="true" onMouseDown={handleBackdropMouseDown} />
    <div
      ref={surfaceRef}
      className={['crosshook-modal__surface', 'crosshook-panel', 'crosshook-focus-scope', className]
        .filter(Boolean)
        .join(' ')}
      role="dialog"
      aria-modal="true"
      aria-labelledby={titleId}
      aria-describedby={description ? descriptionId : undefined}
      data-crosshook-focus-root={confirmation ? undefined : 'modal'}
      onKeyDown={handleKeyDown}
    >
      <div className="crosshook-modal__body">{children}</div>
      <footer className="crosshook-modal__footer">
        {footer ? <div className="crosshook-modal__footer-actions">{footer}</div> : null}
      </footer>
    </div>
  </div>,
  portalHostRef.current
);
```

Do not invent a new modal shell for Phase 4. Reuse the focus trap / portal / footer contract that already exists.

### MOCK_HANDLER_PATTERN

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/collections.ts:234-266
map.set('collection_get_defaults', async (args): Promise<MockCollectionDefaults | null> => {
  const { collectionId } = args as { collectionId: string };
  if (!findById(collectionId)) {
    throw new Error(`[dev-mock] collection_get_defaults: collection not found: ${collectionId}`);
  }
  const d = mockDefaults.get(collectionId);
  return d && !isDefaultsEmpty(d) ? cloneMockDefaults(d) : null;
});
```

Mirror the current collection mock shape and preserve the `[dev-mock]` prefix exactly.

### READ_COMMAND_ALLOWLIST

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/wrapHandler.ts:42-63
const EXPLICIT_READ_COMMANDS: ReadonlySet<string> = new Set<string>([
  ...SHELL_CRITICAL_READS,
  'profile_load',
  'profile_export_toml',
  'collection_get_defaults',
]);
```

`collection_import_from_toml` must be added here because it is a preview/read command with a mutation-sounding name.

### TEST_STRUCTURE

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs:582-610
#[test]
fn export_and_import_round_trip_profile() {
    let temp_dir = tempdir().unwrap();
    let profiles_dir = temp_dir.path().join("profiles");
    let export_path = temp_dir.path().join("exports").join("elden-ring.json");
    let store = ProfileStore::with_base_path(profiles_dir.clone());
    let profile = sample_profile();

    store.save("elden-ring", &profile).unwrap();

    let exported = export_community_profile(&profiles_dir, "elden-ring", &export_path).unwrap();
    let imported_profiles_dir = temp_dir.path().join("imported-profiles");
    let imported = import_community_profile(&export_path, &imported_profiles_dir).unwrap();
    assert_eq!(imported.profile_name, "elden-ring");
}
```

Use `tempdir()` + real file writes + reloads for the collection preset roundtrip tests.

---

## Files to Change

| File                                                                              | Action | Justification                                                                   |
| --------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`                   | UPDATE | Export new collection preset modules and public types                           |
| `src/crosshook-native/crates/crosshook-core/src/profile/collection_schema.rs`     | CREATE | Dedicated TOML manifest contract + schema constant                              |
| `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange.rs`   | CREATE | Export/import-preview pipeline and focused Rust tests                           |
| `src/crosshook-native/src-tauri/src/commands/collections.rs`                      | UPDATE | Add export and import-preview Tauri commands                                    |
| `src/crosshook-native/src-tauri/src/lib.rs`                                       | UPDATE | Register the new collection commands                                            |
| `src/crosshook-native/src/types/collections.ts`                                   | UPDATE | TS mirrors for export result and import preview bucket types                    |
| `src/crosshook-native/src/context/CollectionsContext.tsx`                         | UPDATE | Domain-level export/import preview/apply helpers and rollback logic             |
| `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx`          | UPDATE | Add `Import preset...` entrypoint                                               |
| `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`         | UPDATE | Add `Export preset...` action and import review modal state wiring              |
| `src/crosshook-native/src/components/collections/CollectionImportReviewModal.tsx` | CREATE | Review/resolve imported matches before commit                                   |
| `src/crosshook-native/src/components/collections/CollectionImportReviewModal.css` | CREATE | Bucket list / resolution layout styles if shared modal classes are insufficient |
| `src/crosshook-native/src/lib/mocks/handlers/collections.ts`                      | UPDATE | Mock export/import preview handlers for browser dev mode                        |
| `src/crosshook-native/src/lib/mocks/wrapHandler.ts`                               | UPDATE | Allow-list `collection_import_from_toml` as a read command                      |

## NOT Building

- No new SQLite migration or new tables
- No backend `collection_apply_import` transaction command
- No import into an existing collection
- No auto-created stub profiles for unmatched descriptors
- No collection merge/overwrite flow
- No smart matching beyond the documented precedence (`steam_app_id`, then exact `game_name + trainer_community_trainer_sha256`)
- No changes to `CollectionDefaultsSection` shape or `profile_load(collectionId)` semantics

---

## Step-by-Step Tasks

### Task 1: Add the collection preset schema contract

- **ACTION**: Create dedicated Rust schema types for the TOML wire format and export them from `profile/mod.rs`.
- **IMPLEMENT**: Add `COLLECTION_PRESET_SCHEMA_VERSION: &str = "1"`, `CollectionPresetManifest`, `CollectionPresetProfileDescriptor`, preview bucket structs, and `CollectionExchangeError`. Keep `defaults: Option<CollectionDefaultsSection>`.
- **MIRROR**: `community_schema.rs` manifest layout and `exchange.rs` error enum pattern.
- **IMPORTS**: `serde::{Serialize, Deserialize}`, `std::path::{Path, PathBuf}`, `crate::profile::CollectionDefaultsSection`.
- **GOTCHA**: Issue `#180` explicitly calls for `schema_version = "1"` in TOML. Do not silently switch this to an integer schema field.
- **VALIDATE**: Unit tests prove default manifest serialization, required-field validation, and unsupported schema rejection.

### Task 2: Implement the export-to-TOML pipeline

- **ACTION**: Add a core helper that exports one collection preset to disk.
- **IMPLEMENT**: Read collection metadata + defaults from `MetadataStore`; read members from `list_profiles_in_collection`; load each local profile via `ProfileStore`; derive descriptor fields; serialize the manifest with `toml::to_string_pretty(...)`; write the final string to `output_path`; return a lightweight export result. Populate the preset field named `steam_app_id` with the effective value from `resolve_art_app_id(profile)` so profiles that only carry `runtime.steam_app_id` still roundtrip with the strongest match key.
- **MIRROR**: `export_community_profile(...)` and `profile_to_shareable_toml(...)`.
- **IMPORTS**: `ProfileStore`, `MetadataStore`, `std::fs`, `toml`.
- **GOTCHA**: Do not silently skip a member whose profile file cannot be loaded. Export must fail loudly instead of producing an incomplete preset.
- **VALIDATE**: Exported file contains collection name, optional description, optional defaults, and one descriptor per member; parse it back successfully in tests, including a profile whose `steam.app_id` is empty but `runtime.steam_app_id` is populated.

### Task 3: Implement the import-preview parsing and descriptor matcher

- **ACTION**: Add a read-only preview helper that parses a preset and classifies each descriptor against local profiles.
- **IMPLEMENT**: Parse TOML into `CollectionPresetManifest`; build a local candidate index from `ProfileStore::list()` + `load()`; match with precedence: (1) exact effective app id using `resolve_art_app_id(...)`, serialized into the preset field named `steam_app_id`, (2) exact `(game_name, trainer_community_trainer_sha256)` pair; return `matched`, `ambiguous`, and `unmatched` buckets with enough display info for the UI.
- **MIRROR**: `preview_community_profile_import(...)`.
- **IMPORTS**: `ProfileStore`, `GameProfile`, `HashMap`, `BTreeMap`, `Path`.
- **GOTCHA**: `metadata/profile_sync.rs` only supports filename lookups. Matching must inspect actual loaded profiles, not metadata rows. If loading one local profile fails while building the candidate index, mirror `profile_list_summaries(...)`: `tracing::warn!` and skip that one profile instead of failing the whole import preview.
- **VALIDATE**: Tests cover unique steam match, ambiguous steam match, pair fallback, effective app-id fallback (`runtime.steam_app_id`), unmatched descriptor, malformed TOML, future schema version rejection, and corrupt local profile files being skipped with a warning.

### Task 4: Add thin Tauri collection import/export commands

- **ACTION**: Expose the new core helpers through `src-tauri/src/commands/collections.rs` and register them in `src-tauri/src/lib.rs`.
- **IMPLEMENT**: Add `collection_export_to_toml(collection_id, output_path, ...)` and `collection_import_from_toml(path, ...)` command handlers; map core errors to strings; keep persistence out of the preview command.
- **MIRROR**: Existing `collection_get_defaults` / `collection_set_defaults` handlers and `community_export_profile` / `community_prepare_import`.
- **IMPORTS**: New core exchange types/functions plus the already-managed `ProfileStore` and `MetadataStore`.
- **GOTCHA**: `collection_import_from_toml` is preview-only despite its name; it must not mutate SQLite or profile files.
- **VALIDATE**: Native build compiles and the commands appear in the Tauri handler list.

### Task 5: Extend collection types and context APIs

- **ACTION**: Add Phase 4 types and actions to `src/types/collections.ts` and `CollectionsContext.tsx`.
- **IMPLEMENT**: Define TS mirrors for import preview buckets and export result; add `exportCollectionPreset`, `prepareCollectionImport`, and `applyImportedCollection` context helpers; add dedicated internal no-refresh command wrappers for the apply path so the existing eager-refresh CRUD helpers can stay unchanged for interactive UI while import apply performs one final `refresh()` after success.
- **MIRROR**: `useCommunityProfiles.prepareCommunityImport(...)` and the existing `createCollection(...)`/`addProfile(...)` patterns in `CollectionsContext.tsx`.
- **IMPORTS**: `callCommand`, `chooseFile`, `chooseSaveFile`, existing collection CRUD/defaults helpers.
- **GOTCHA**: Apply is multi-step and non-transactional. If anything fails after `collection_create`, attempt best-effort cleanup with `collection_delete(createdId)` and rethrow the original error. Do not implement apply on top of the existing eager-refresh public helpers or the UI will flash partial state during import.
- **VALIDATE**: Mocked UI flow can preview a file, confirm import, and end with one refreshed collection list and no partial leftover collection on induced failure.

### Task 6: Add user entrypoints for import and export

- **ACTION**: Update the current collection UI to expose the new flow.
- **IMPLEMENT**: Add `Import preset...` to `CollectionsSidebar.tsx`; add `Export preset...` to `CollectionViewModal.tsx`; use `chooseFile(...)` / `chooseSaveFile(...)`; open the review modal only after a successful preview command.
- **MIRROR**: `ProfilesPage` community export flow and current collection modal footer actions.
- **IMPORTS**: `chooseFile`, `chooseSaveFile`, new context helpers.
- **GOTCHA**: Canceling the file dialog is a clean no-op; do not show errors or clear existing modal state on cancel.
- **VALIDATE**: Browser dev mode lets a tester open the import chooser from the sidebar and export from an open collection with no console errors.

### Task 7: Build the collection import review modal

- **ACTION**: Create `CollectionImportReviewModal.tsx` using the existing modal shell.
- **IMPLEMENT**: Show imported collection metadata, defaults summary, matched bucket, ambiguous bucket with required selection UI, unmatched bucket with skip UI, and a confirm action that stays disabled until every ambiguous entry is resolved or explicitly skipped. Seed the modal with the imported collection name/description and allow name edits before commit.
- **MIRROR**: `ProfileReviewModal` portal/focus trap and `CommunityImportWizardModal` state reset behavior.
- **IMPORTS**: `ProfileReviewModal`, React state/effects, new collection preview types.
- **GOTCHA**: Keep any custom scrolling inside `.crosshook-modal__body`. If a new nested scroll container is introduced, update `useScrollEnhance`.
- **VALIDATE**: Keyboard navigation works, `Esc` closes, focus returns to the trigger, and the confirm button enables only when the review state is valid.

### Task 8: Extend browser dev-mode mocks

- **ACTION**: Add Phase 4 handlers to `src/lib/mocks/handlers/collections.ts` and classify the preview command correctly in `wrapHandler.ts`.
- **IMPLEMENT**: Mock `collection_export_to_toml` and `collection_import_from_toml` with deterministic preset fixture data; keep the existing collection state model intact; add `collection_import_from_toml` to `EXPLICIT_READ_COMMANDS`.
- **MIRROR**: Current collection defaults mock handlers and the explicit read-allow-list pattern in `wrapHandler.ts`.
- **IMPORTS**: Existing collection mock helpers/state plus simple fixture builders.
- **GOTCHA**: Without the explicit read allow-list, `?errors=true` will treat `collection_import_from_toml` as a mutation and break the browser-only review flow.
- **VALIDATE**: `pnpm --dir src/crosshook-native dev:browser:check` passes and the browser-only import review path can be exercised end-to-end.

### Task 9: Add focused roundtrip and failure tests

- **ACTION**: Put the strongest regression protection in backend Rust tests.
- **IMPLEMENT**: Add tests for export -> preview -> apply -> re-export parity, schema rejection, malformed TOML rejection, ambiguous matching, unmatched handling, defaults losslessness, and export failure on missing member profile files.
- **MIRROR**: `exchange.rs` roundtrip tests and `MetadataStore::open_in_memory()` collection test style in `metadata/mod.rs`.
- **IMPORTS**: `tempfile::tempdir`, `ProfileStore`, `MetadataStore::open_in_memory`, sample profile helpers.
- **GOTCHA**: There is no frontend component-test harness in this repo. Do not sink time into ad hoc test infrastructure here; backend tests + build + browser validation are the right balance.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes and covers the new exchange module thoroughly.

---

## Testing Strategy

### Unit / integration tests

| Test                                  | Input                                                  | Expected Output                                                    | Edge Case? |
| ------------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------ | ---------- |
| Schema v1 roundtrip                   | Minimal valid collection preset                        | Parse succeeds and serializes back with `schema_version = "1"`     | No         |
| Unsupported schema                    | `schema_version = "2"`                                 | Actionable unsupported-schema error                                | Yes        |
| Malformed TOML                        | Missing `profiles` array or broken syntax              | Parse/validation error, no writes                                  | Yes        |
| Export/import roundtrip               | Collection with members + defaults                     | Re-import preview resolves back to the same member/default set     | No         |
| Exact `steam_app_id` match            | One local profile with matching app id                 | Entry lands in `matched` bucket                                    | No         |
| Ambiguous `steam_app_id` match        | Two local profiles share the same app id               | Entry lands in `ambiguous` bucket with both candidates             | Yes        |
| Effective app-id fallback             | Empty `steam.app_id`, populated `runtime.steam_app_id` | Export and preview both use the fallback value for matching        | Yes        |
| Pair fallback match                   | Empty app id, exact `(game_name, trainer_sha)` present | Entry lands in `matched` bucket                                    | Yes        |
| Unmatched descriptor                  | No local profile satisfies either rule                 | Entry lands in `unmatched` bucket                                  | Yes        |
| Corrupt local profile during preview  | One local profile cannot be loaded                     | Preview logs a warning, skips that profile, and continues matching | Yes        |
| Empty defaults export                 | Collection with no defaults                            | Export omits defaults block or serializes it as absent             | Yes        |
| Missing member profile file on export | Metadata member exists but on-disk load fails          | Export errors loudly, no partial file written                      | Yes        |

### Edge cases checklist

- [ ] File dialog cancel from export does nothing
- [ ] File dialog cancel from import does nothing
- [ ] Human-edited TOML missing required sections is rejected cleanly
- [ ] Future schema string is rejected cleanly
- [ ] Profiles that only expose `runtime.steam_app_id` still export/import with the correct `steam_app_id` descriptor
- [ ] Ambiguous entries cannot be confirmed without user choice or explicit skip
- [ ] Unmatched entries never auto-create stub profiles
- [ ] A corrupt local profile file does not block preview for every other profile
- [ ] Collection name collision on import surfaces a clear error
- [ ] Defaults roundtrip preserves `custom_env_vars`, `gamescope`, `trainer_gamescope`, `mangohud`, `method`, `network_isolation`, and `optimizations`
- [ ] Browser dev mode still works with `?errors=true`

---

## Validation Commands

### Static analysis / frontend build

```bash
pnpm --dir src/crosshook-native build
```

EXPECT: TypeScript compilation and Vite build both succeed.

### Mock coverage / browser dev safeguards

```bash
pnpm --dir src/crosshook-native dev:browser:check
```

EXPECT: No missing mock coverage for the new collection commands.

### Rust test suite

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: All `crosshook-core` tests pass, including the new collection preset tests.

### Browser validation

```bash
./scripts/dev-native.sh --browser
```

EXPECT: Browser-only dev mode can open the sidebar import flow, show preview buckets, and export a collection without unhandled command errors.

### Native validation

```bash
./scripts/dev-native.sh
```

EXPECT: Native mode can export a real collection, import it back, and preserve collection defaults when reopening the imported collection.

### Manual validation

- [ ] Open an existing collection and export it to `sample.crosshook-collection.toml`
- [ ] Inspect the file and confirm it contains `schema_version = "1"`, collection metadata, defaults, and profile descriptors
- [ ] Import that same preset through the sidebar and confirm preview buckets are sensible
- [ ] Resolve ambiguous entries, skip unmatched ones, and complete the import
- [ ] Confirm the imported collection appears in the sidebar with the expected member count
- [ ] Open the imported collection and verify its defaults editor shows the expected values
- [ ] Re-export the imported collection and confirm the descriptor/default payload is stable

---

## Acceptance Criteria

- [ ] TOML schema v1 is defined with `schema_version = "1"` and a dedicated manifest type
- [ ] Multi-field profile descriptors include at minimum `steam_app_id`, `game_name`, and `trainer_community_trainer_sha256`
- [ ] `collection_export_to_toml` exists in Rust, is exposed through Tauri, and is mocked in browser dev mode
- [ ] `collection_import_from_toml` exists in Rust, is exposed through Tauri, returns a preview with `matched` / `ambiguous` / `unmatched`, and is mocked in browser dev mode
- [ ] `CollectionImportReviewModal` lets the user resolve ambiguous matches and skip unmatched entries before any state is written
- [ ] Imported collections persist through existing Phase 1/3 commands only
- [ ] Per-collection defaults roundtrip losslessly through export/import
- [ ] Malformed or unsupported-version TOML is rejected with actionable errors
- [ ] No stub profiles are created for unmatched entries
- [ ] Browser dev mode can exercise the full preview/review flow

## Completion Checklist

- [ ] Code follows the existing community exchange and collection command patterns
- [ ] No new migration or persistence layer was added
- [ ] Error handling stays explicit at the core layer and friendly at the UI layer
- [ ] Logging uses `tracing::warn!` for non-fatal side effects only
- [ ] Mock handlers keep the `[dev-mock]` prefix and preview command is allow-listed as read
- [ ] Tests cover roundtrip, ambiguity, malformed TOML, and schema rejection
- [ ] The plan remains self-contained; no extra codebase search should be required during implementation

## Risks

| Risk                                                                                                   | Likelihood | Impact | Mitigation                                                                                                       |
| ------------------------------------------------------------------------------------------------------ | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------- |
| Descriptor matching requires loading many local profiles from disk                                     | Medium     | Medium | Build the candidate index once per preview, not per descriptor, and keep matching rules simple and deterministic |
| Multi-step frontend commit can leave partial state                                                     | Medium     | High   | Centralize apply logic in `CollectionsContext`, attempt best-effort rollback, and surface the original failure   |
| Human-edited TOML omits fields or uses future schema strings                                           | High       | Medium | Validate early with dedicated manifest parsing and explicit schema rejection                                     |
| Exporting a collection whose member profile file is missing/corrupt                                    | Medium     | Medium | Fail export loudly rather than silently emitting an incomplete preset                                            |
| Browser-only dev mode breaks because `collection_import_from_toml` is misclassified as a write command | High       | Medium | Add it to `EXPLICIT_READ_COMMANDS` and cover it in `dev:browser:check`                                           |

## Notes

Recommended wire format skeleton:

```toml
schema_version = "1"
name = "Action / Adventure"
description = "Steam Deck-ready action profiles"

[defaults]
method = "proton"
network_isolation = true

[defaults.custom_env_vars]
PROTON_LOG = "1"

[[profiles]]
steam_app_id = "1245620"
game_name = "Elden Ring"
trainer_community_trainer_sha256 = "..."
```

Matching precedence is intentionally strict and deterministic:

1. Use exact `steam_app_id` when present and non-empty.
2. If that yields no candidate, fall back to exact `(game_name, trainer_community_trainer_sha256)` when both fields are present and non-empty.
3. One candidate -> `matched`.
4. Multiple candidates -> `ambiguous`.
5. Zero candidates -> `unmatched`.

For both export and preview matching, the preset field named `steam_app_id` should carry the effective app-id semantics from `resolve_art_app_id(...)`: `steam.app_id` first, then `runtime.steam_app_id` when the primary field is empty.

When building the local preview index, unreadable local profile TOMLs should be handled like `profile_list_summaries(...)`: log with `tracing::warn!`, skip the broken profile, and continue building preview results for everything else.

This keeps Phase 4 aligned with the issue text, avoids schema changes, and prevents the review modal from making undocumented guesses.
