# Implementation Report: Profile Collections — Phase 4 (TOML export / import preset)

## Summary

Implemented `*.crosshook-collection.toml` collection presets: Rust schema + export/preview pipeline in `crosshook-core`, Tauri commands `collection_export_to_toml` and `collection_import_from_toml`, frontend types and `CollectionsContext` helpers (`prepareCollectionImportPreview`, `applyImportedCollection`, `exportCollectionPreset`), sidebar **Import preset…** and collection modal **Export preset…**, `CollectionImportReviewModal` for matched/ambiguous/unmatched review with rollback on failed apply, and browser mocks plus `collection_import_from_toml` on the explicit read-command allow-list for `?errors=true`.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual        |
| ------------- | ---------------- | ------------- |
| Complexity    | Large            | Large         |
| Confidence    | (not stated)     | High          |
| Files Changed | ~14              | 16+ (incl. CSS) |

## Tasks Completed

| #   | Task                                      | Status          | Notes |
| --- | ----------------------------------------- | --------------- | ----- |
| 1   | Collection preset schema contract         | Complete        | `collection_schema.rs` |
| 2   | Export-to-TOML pipeline                   | Complete        | `collection_exchange.rs` |
| 3   | Import-preview + matcher                  | Complete        | Same module + unit tests |
| 4   | Tauri commands + registration             | Complete        | `commands/collections.rs`, `lib.rs` |
| 5   | Types + context APIs                      | Complete        | `types/collections.ts`, `CollectionsContext.tsx` |
| 6   | Sidebar + modal entrypoints               | Complete        | `CollectionsSidebar.tsx`, `CollectionViewModal.tsx` |
| 7   | Collection import review modal            | Complete        | `CollectionImportReviewModal.tsx` + CSS |
| 8   | Browser mocks + read allow-list           | Complete        | `handlers/collections.ts`, `wrapHandler.ts` |
| 9   | Focused Rust tests                        | Complete        | `collection_exchange` tests |

## Validation Results

| Level           | Status | Notes |
| --------------- | ------ | ----- |
| Static Analysis | Pass   | `npm run build` (tsc + vite) |
| Unit Tests      | Pass   | `cargo test -p crosshook-core` |
| Build           | Pass   | Frontend production build |
| Integration     | N/A    | No automated E2E in plan |
| Edge Cases      | Manual | Plan checklist partially covered by Rust tests + UI |

## Files Changed

| File | Action |
| ---- | ------ |
| `src/crosshook-native/crates/crosshook-core/src/profile/collection_schema.rs` | CREATED |
| `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange.rs` | CREATED |
| `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs` | UPDATED |
| `src/crosshook-native/src-tauri/src/commands/collections.rs` | UPDATED |
| `src/crosshook-native/src-tauri/src/lib.rs` | UPDATED |
| `src/crosshook-native/src/types/collections.ts` | UPDATED |
| `src/crosshook-native/src/context/CollectionsContext.tsx` | UPDATED |
| `src/crosshook-native/src/hooks/useCollections.ts` | UPDATED |
| `src/crosshook-native/src/components/collections/CollectionsSidebar.tsx` | UPDATED |
| `src/crosshook-native/src/components/collections/CollectionViewModal.tsx` | UPDATED |
| `src/crosshook-native/src/components/collections/CollectionViewModal.css` | UPDATED |
| `src/crosshook-native/src/components/collections/CollectionImportReviewModal.tsx` | CREATED |
| `src/crosshook-native/src/components/collections/CollectionImportReviewModal.css` | CREATED |
| `src/crosshook-native/src/lib/mocks/handlers/collections.ts` | UPDATED |
| `src/crosshook-native/src/lib/mocks/wrapHandler.ts` | UPDATED |

## Deviations from Plan

None material. Modal shell follows the same portal/body lock pattern as `CollectionViewModal` rather than `ProfileReviewModal` (which is profile-field-specific), matching the plan’s “reuse contract” without forcing dummy profile props.

## Issues Encountered

- Mock `CollectionExportResult` needed `CollectionDefaults` assertion for `method` typing (`MockCollectionDefaults` vs `LaunchMethod`).
- Removed unused `CollectionDefaultsSection` import in `collection_exchange.rs` (test-only import under `cfg(test)`).

## Tests Written

| Test file / area | Coverage |
| ---------------- | -------- |
| `collection_schema.rs` | Schema string serialization, empty name validation |
| `collection_exchange.rs` | Future schema rejection, malformed TOML, roundtrip + effective app id, ambiguity, pair fallback, unmatched, missing member file on export |

## Next Steps

- [ ] Code review (`/ycc:code-review` or PR review)
- [ ] Open PR with `Closes #180` and conventional commit
- [ ] Native smoke: export/import on a real collection (manual checklist in plan)
