# Implementation Report: Profile Collections — Phase 3 (Per-Collection Launch Defaults)

**Date**: 2026-04-08
**Branch**: `main` (working tree, pre-PR)
**Source Plan**: `docs/prps/plans/profile-collections-phase-3-launch-defaults.plan.md`
**Source PRD**: `docs/prps/prds/profile-collections.prd.md` (Phase 3)
**Source Issue**: [`yandy-r/crosshook#179`](https://github.com/yandy-r/crosshook/issues/179)
**Status**: Complete — ready for `/ycc:prp-pr`

## Overview

Delivered the "behavior" leg of profile collections: each collection can now carry
its own `LaunchSection` subset (`method`, `optimizations`, `custom_env_vars`,
`network_isolation`, `gamescope`, `trainer_gamescope`, `mangohud`), and those
overrides merge into a profile at load time **only when the profile is loaded
inside a collection context**. All 19 plan tasks completed without deviation.
Zero new clippy warnings, zero test regressions, and all 10 new Rust tests green.

Key outcomes:

- New serde type `CollectionDefaultsSection` (7 optional fields + additive env-var
  bucket) added to `crosshook-core`.
- `GameProfile::effective_profile` refactored into a `effective_profile_with(Option<&CollectionDefaultsSection>)`
  function that introduces a NEW middle merge layer (`base → collection defaults → local_override`),
  with a thin `effective_profile()` shim that forwards `None`. **All 13 existing
  call sites are unchanged**.
- Schema advanced from **v19 → v20** via `ALTER TABLE collections ADD COLUMN defaults_json TEXT`
  (additive, non-destructive).
- 2 new IPC commands: `collection_get_defaults`, `collection_set_defaults`. The
  existing `profile_load` was extended to accept `collection_id: Option<String>`
  and returns the merged profile when present (backward-compat: missing/empty
  collection_id behaves identically to pre-Phase 3).
- New React hook `useCollectionDefaults(collectionId)` and a new
  `<CollectionLaunchDefaultsEditor>` inline editor wired into the existing
  `<CollectionViewModal>` body.
- LaunchPage now threads `activeCollectionId` into `selectProfile` (which is
  `loadProfile`) so launching a profile from a collection-filtered LaunchPage
  applies the collection's defaults. **ProfilesPage call sites are unchanged
  (editor-safety invariant)**: the editor always sees the raw storage profile.
- Browser dev-mode mocks added for both new commands plus `profile_load`'s new
  `collectionId` field, so `--browser` mode parity matches Tauri behavior.
- **10 new Rust tests** (4 merge-layer, 1 migration, 5 metadata store) all green.

## Files Changed

| #   | File                                                                                 | Action | Notes                                                                                                                                                                                                                                |
| --- | ------------------------------------------------------------------------------------ | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`                   | UPDATE | Added `CollectionDefaultsSection` struct + `is_empty()`; refactored `effective_profile` into `effective_profile_with(Option<&CollectionDefaultsSection>) -> Self` + thin shim; added 4 unit tests for the new merge layer            |
| 2   | `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`                      | UPDATE | Re-exported `CollectionDefaultsSection` from `models`                                                                                                                                                                                |
| 3   | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`              | UPDATE | Added `migrate_19_to_20` + dispatch (single `ALTER TABLE` adding nullable `defaults_json TEXT`); retargeted `migration_18_to_19_adds_sort_order_and_cascade` assertion from `version == 19` to `version >= 19`; added migration test |
| 4   | `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs`             | UPDATE | Added `get_collection_defaults` / `set_collection_defaults` free functions with corrupt-JSON → `Corrupt` and missing-collection → `Validation` semantics                                                                             |
| 5   | `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`                     | UPDATE | Added `MetadataStore::get_collection_defaults` / `set_collection_defaults` wrappers; added 5 integration tests (round-trip, clear-NULL, unknown-id error, corrupt-JSON Corrupt error, delete cascade)                                |
| 6   | `src/crosshook-native/src-tauri/src/commands/collections.rs`                         | UPDATE | Added `collection_get_defaults` / `collection_set_defaults` Tauri command handlers                                                                                                                                                   |
| 7   | `src/crosshook-native/src-tauri/src/commands/profile.rs`                             | UPDATE | Extended `profile_load` to accept `collection_id: Option<String>` + `metadata_store: State` and call `effective_profile_with(defaults.as_ref())` when present                                                                        |
| 8   | `src/crosshook-native/src-tauri/src/lib.rs`                                          | UPDATE | Registered 2 new commands in `tauri::generate_handler!`                                                                                                                                                                              |
| 9   | `src/crosshook-native/src/lib/mocks/handlers/collections.ts`                         | UPDATE | Added `MockCollectionDefaults` interface, `mockDefaults` map, `getMockCollectionDefaults` getter, and 2 new handlers (`collection_get_defaults`, `collection_set_defaults`)                                                          |
| 10  | `src/crosshook-native/src/lib/mocks/handlers/profile.ts`                             | UPDATE | Extended `profile_load` mock to accept `collectionId` and apply mock defaults via new `applyMockCollectionDefaults` helper (mirrors Rust `effective_profile_with` semantics)                                                         |
| 11  | `src/crosshook-native/src/lib/mocks/wrapHandler.ts`                                  | UPDATE | Added `collection_get_defaults` to `EXPLICIT_READ_COMMANDS` (the `collection_*` prefix doesn't match `READ_VERB_RE`)                                                                                                                 |
| 12  | `src/crosshook-native/src/types/profile.ts`                                          | UPDATE | Added `CollectionDefaults` interface and `isCollectionDefaultsEmpty` helper mirroring the Rust serde type                                                                                                                            |
| 13  | `src/crosshook-native/src/hooks/useProfile.ts`                                       | UPDATE | Extended `loadProfile` to accept `loadOptions.collectionId`; widened `selectProfile` type signature to mirror `loadProfile`; documented editor-safety invariant in JSDoc                                                             |
| 14  | `src/crosshook-native/src/hooks/useCollectionDefaults.ts`                            | CREATE | New hook with race-safe `requestSeqRef`, `defaults` state, `loading` / `error`, `reload`, `saveDefaults` (mirrors `useCollectionMembers`)                                                                                            |
| 15  | `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx` | CREATE | New collapsible `<details>` editor with method dropdown, network isolation dropdown, custom env-var table, Save / Reset draft / Clear all / Open in Profiles page actions                                                            |
| 16  | `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.css` | CREATE | Bem-like `crosshook-collection-launch-defaults-editor__*` classes; no new scroll containers (lives inside `.crosshook-modal__body` which is already enhanced)                                                                        |
| 17  | `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`            | UPDATE | Added required `onOpenInProfilesPage` prop; rendered `<CollectionLaunchDefaultsEditor>` above the search input                                                                                                                       |
| 18  | `src/crosshook-native/src/App.tsx`                                                   | UPDATE | Wired `onOpenInProfilesPage` callback on `<CollectionViewModal>` — closes the modal then `setRoute('profiles')`; `activeCollectionId` is preserved by Phase 2 plumbing                                                               |
| 19  | `src/crosshook-native/src/components/pages/LaunchPage.tsx`                           | UPDATE | Active-Profile dropdown onChange now passes `{ collectionId: activeCollectionId ?? undefined }`. Auto-select effect and post-suggestion reload also forward the collection context. ProfilesPage call sites untouched.               |

**Total**: 19 files (3 CREATE — `useCollectionDefaults.ts`, `CollectionLaunchDefaultsEditor.tsx`, `CollectionLaunchDefaultsEditor.css`; 16 UPDATE).

## Features Delivered

### Schema v20

- `collections.defaults_json TEXT` — nullable, no DEFAULT. Existing rows backfill
  to `NULL` automatically. Single `ALTER TABLE` (no transaction needed).
- Round-trip JSON storage; reads via `serde_json::from_str` into
  `CollectionDefaultsSection`.

### `CollectionDefaultsSection` serde type

Fields (all optional, `skip_serializing_if = "Option::is_none"` so empty payloads
serialize to `{}`):

- `method: Option<String>` — replacement; whitespace-only is ignored
- `optimizations: Option<LaunchOptimizationsSection>` — replacement
- `custom_env_vars: BTreeMap<String, String>` — **additive merge**, collection
  keys win on collision; profile keys without a collision are preserved
- `network_isolation: Option<bool>` — replacement
- `gamescope: Option<GamescopeConfig>` — replacement
- `trainer_gamescope: Option<GamescopeConfig>` — replacement
- `mangohud: Option<MangoHudConfig>` — replacement

`is_empty()` returns true when no field would influence a merge — this is the
guard used by the metadata store's `set_collection_defaults` write to normalize
empty payloads to a NULL column.

### `effective_profile_with` merge layer

Precedence (lowest → highest):

1. Base profile (`self`)
2. Collection defaults (when `Some`)
3. `local_override.*` — machine-specific paths always win last

The original `effective_profile()` is a one-liner shim:

```rust
pub fn effective_profile(&self) -> Self {
    self.effective_profile_with(None)
}
```

So all 13 existing non-test call sites compile and behave identically. The
existing `effective_profile_prefers_local_override_paths` and
`storage_profile_roundtrip_is_idempotent` regression guards both still pass.

### IPC surface

| Command                   | Args                                                   | Returns                             | Mock                                                                              |
| ------------------------- | ------------------------------------------------------ | ----------------------------------- | --------------------------------------------------------------------------------- |
| `collection_get_defaults` | `{ collectionId }`                                     | `CollectionDefaultsSection \| null` | new                                                                               |
| `collection_set_defaults` | `{ collectionId, defaults: CollectionDefaults\|null }` | `null`                              | new                                                                               |
| `profile_load` (extended) | `{ name, collectionId? }`                              | `GameProfile`                       | extended — applies `applyMockCollectionDefaults` when `collectionId` is non-empty |

`profile_load` is **strictly additive at the IPC boundary**: existing callers
that don't pass `collectionId` continue to receive the raw storage profile.

### Editor safety invariant

The new `loadProfile.loadOptions.collectionId` is documented as:

> EDITOR SAFETY INVARIANT: `ProfilesPage` callers MUST NOT pass this — the
> editor must always see the raw storage profile, otherwise edits would persist
> the merged view back to the profile TOML. Only the LaunchPage profile-selector
> path passes a collectionId.

ProfilesPage's three `selectProfile(name)` call sites were left untouched.
LaunchPage's three `selectProfile(name)` call sites all pass
`{ collectionId: activeCollectionId ?? undefined }`.

### Inline editor UX

The `<CollectionLaunchDefaultsEditor>` component renders inside
`<CollectionViewModal>` as a collapsible `<details>` block above the search
input. It exposes:

- `method` dropdown (`(inherit) / native / proton_run / steam_applaunch`)
- `network_isolation` dropdown (`(inherit) / on / off`)
- `custom_env_vars` table with key/value editing, add, remove
- "Open in Profiles page →" link-out — closes the modal and navigates to the
  Profiles route while preserving `activeCollectionId`
- "Clear all" — wipes the local draft
- "Reset draft" — re-anchors to the persisted defaults
- "Save" — calls `saveDefaults(...)`. Saving an effectively-empty draft
  normalizes to `null` (writes NULL on the column).

A small `Active` badge appears in the summary when the persisted defaults are
non-empty so users know at a glance whether the collection currently has
overrides set.

The editor only exposes the simple inline-editable subset (`method`,
`network_isolation`, `custom_env_vars`). Per PRD scope, `optimizations`,
`gamescope`, `trainer_gamescope`, `mangohud` are persisted by the backend but
edited via the "Open in Profiles page →" link-out. This is a deliberate v1
scope cut documented in `NOT Building` and Task 16.

## Tests

10 new Rust tests, all green. See `cargo test -p crosshook-core` output.

| Test                                                                                | Layer          | Covers                                                                                                           |
| ----------------------------------------------------------------------------------- | -------------- | ---------------------------------------------------------------------------------------------------------------- |
| `effective_profile_with_none_equals_shim`                                           | profile/models | `effective_profile()` is byte-equal to `effective_profile_with(None)` (backward-compat invariant)                |
| `effective_profile_with_merges_collection_defaults_between_base_and_local_override` | profile/models | Precedence base → collection defaults → local_override; env-var additive merge; collection key wins on collision |
| `effective_profile_with_none_fields_do_not_overwrite_profile`                       | profile/models | Empty defaults is a no-op; profile env vars are never dropped                                                    |
| `effective_profile_with_ignores_whitespace_only_method`                             | profile/models | Whitespace-only `method` does NOT clobber the profile's method                                                   |
| `migration_19_to_20_adds_defaults_json_column`                                      | metadata/migr  | Column type, nullability, JSON round-trip, NULL round-trip, `user_version == 20`                                 |
| `test_collection_defaults_set_and_get_roundtrip`                                    | metadata store | Happy-path write + read                                                                                          |
| `test_collection_defaults_clear_writes_null`                                        | metadata store | `set(None)` and `set(Some(empty))` both normalize to NULL                                                        |
| `test_collection_defaults_unknown_id_errors_on_set`                                 | metadata store | `set` on missing collection returns `Validation`                                                                 |
| `test_collection_defaults_corrupt_json_returns_corrupt_error`                       | metadata store | Corrupt JSON in `defaults_json` surfaces as `MetadataStoreError::Corrupt`                                        |
| `test_collection_defaults_cascades_on_collection_delete`                            | metadata store | After `delete_collection`, reading defaults errors (row gone)                                                    |

All existing 766 unit tests continue to pass; 3 integration tests pass.

## Validation Results

| Level             | Command                                                                         | Status                                                                                             |
| ----------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| 1 — static (Rust) | `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` | ✅ Zero errors, zero new warnings                                                                  |
| 1 — static (Rust) | `cargo check --manifest-path src/crosshook-native/src-tauri/Cargo.toml`         | ✅ Zero errors                                                                                     |
| 1 — clippy        | `cargo clippy ... -p crosshook-core`                                            | ✅ Same baseline (31 pre-existing repo warnings, zero introduced by Phase 3)                       |
| 2 — unit (Rust)   | `cargo test ... -p crosshook-core`                                              | ✅ 766 unit + 3 integration tests pass; 10 new tests green                                         |
| 3 — frontend (TS) | `tsc --noEmit`                                                                  | ✅ Zero type errors                                                                                |
| 4 — link / build  | `cargo check ... src-tauri/Cargo.toml --release`                                | ✅ Release profile compiles; `tauri::generate_handler!` expansion succeeds with the 2 new commands |
| 5 — sentinels     | grep `[dev-mock]` outside `lib/mocks/`                                          | ✅ No new leaked markers; pre-existing `plugin-stubs/` markers are tree-shaken in production       |

### Pre-existing tech debt unrelated to Phase 3

- `cargo check ... --all-targets` on the `src-tauri` crate fails with a missing
  `community_trainer_sha256` and `required_protontricks` field initializer in
  `commands/profile.rs:~1326` (test code). This baseline error exists on
  unmodified `main` and is **not introduced by this PRP**. Verified via
  `git stash && cargo check ... --all-targets`.
- 31 clippy warnings in `crosshook-core` (`derivable_impls`, etc.) pre-date
  Phase 3 — see `git stash && cargo clippy ... -p crosshook-core 2>&1 | grep -c warning`.

## Manual end-to-end validation (Task 20 — operator-driven)

Phase 3 includes a manual end-to-end test fixture that the implementer should
run before merging. The fixture lives in the plan; reproduce it on a real Linux
desktop with the Tauri shell:

1. **Build & launch** the dev shell: `./scripts/dev-native.sh`.
2. **Create a "printenv-test" profile** with `game.executable_path =
/usr/bin/printenv` and `launch.method = native`. Save.
3. **Create a "EnvTest" collection** and add `printenv-test` to it.
4. **Open the "EnvTest" collection view modal** → expand the "Collection launch
   defaults" `<details>` → click "+ Add env var" → set `KEY=CROSSHOOK_PROBE`,
   `VALUE=hello` → click **Save**. Verify the `Active` badge appears.
5. **Set `activeCollectionId` to "EnvTest"** (click the collection in the
   sidebar / activate the filter), then **launch printenv-test** from the
   LaunchPage Active-Profile dropdown. **EXPECTED**: stdout/console should show
   `CROSSHOOK_PROBE=hello`.
6. **Clear the collection filter** so `activeCollectionId === null`. Launch
   printenv-test again. **EXPECTED**: stdout should NOT contain `CROSSHOOK_PROBE`.
7. **Corrupt-JSON recovery**: stop the dev shell, run
   `sqlite3 ~/.local/share/crosshook/metadata.db "UPDATE collections SET defaults_json = '{not-valid' WHERE name = 'EnvTest';"`,
   restart the shell, open the EnvTest modal. **EXPECTED**: the editor shows a
   friendly error, the modal still opens, and saving fresh defaults via the
   editor clears the error (overwrite path).
8. **Cleanup**: delete the `EnvTest` collection and the `printenv-test` profile.

### Acceptance against PRD

All 28 acceptance-criteria checkboxes from the plan are met by the
implementation. Items requiring manual validation (printenv end-to-end,
file-system corrupt-JSON recovery, browser-mode dev smoke) are documented
above and need to be performed by an operator before merging the PR.

## Risks Materialized

None of the listed risks materialized during implementation. The two notable
deviations from initial plan assumptions:

- The plan's Task 14 GOTCHA warned about `selectProfile`'s typing being too
  narrow. The implementation widened the type signature
  (`(name, loadOptions?) => Promise<void>`) so it mirrors `loadProfile` exactly.
  This required no changes to `ProfilesPage`, `LibraryPage`, or
  `HealthDashboardPage` because their existing single-arg call sites remain
  type-correct.
- Task 5's migration test required changing the existing
  `migration_18_to_19_adds_sort_order_and_cascade` assertion from
  `assert_eq!(version, 19)` to `assert!(version >= 19, ...)` to permit the new
  v20 head version. This pattern matches what the older migration tests
  (`migration_14_to_15_*`, etc.) already do.

## Conventional commit suggestions

```text
feat(core): collection launch defaults serde type and effective_profile merge layer
feat(core): schema v20 — collections.defaults_json inline JSON column
feat(core): metadata get/set collection defaults
feat(core): collection_get_defaults / collection_set_defaults IPC + profile_load collection_id
feat(ui): CollectionLaunchDefaultsEditor inline editor inside CollectionViewModal
feat(ui): LaunchPage threads activeCollectionId into loadProfile
feat(ui): browser dev-mode mocks for collection defaults and extended profile_load
```

A single grouped PR linking `Closes #179` is acceptable per the plan.

## Next steps

1. **Manual end-to-end** (Task 20 above) on a real Linux desktop.
2. **`/ycc:prp-pr`** to open the PR with the suggested commit grouping and
   `Closes #179` body. Apply labels: `type:feature`, `area:profiles`,
   `priority:high`.
3. **Update PRD** (per Notes section of plan): mark Phase 3 status as
   `complete` in `docs/prps/prds/profile-collections.prd.md` and link this report.
4. **Phase 4 prerequisites**: this report is the foundation for Phase 4 (TOML
   export/import of collection defaults), which can serialize the same
   `CollectionDefaultsSection` Rust type into a TOML wire format.
