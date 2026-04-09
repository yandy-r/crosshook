# Profile Collections — TOML Preset Schema v1

CrossHook collection presets use the `*.crosshook-collection.toml` file format to share collections across machines. A preset captures the collection name, optional description, optional per-collection launch defaults, and a list of profile descriptors that identify which games belong to the collection. On import, CrossHook matches descriptors against the local profile library and presents a review modal for any ambiguous or unmatched entries.

## Schema structure

The top-level manifest is defined by `CollectionPresetManifest` in `src/crosshook-native/crates/crosshook-core/src/profile/collection_schema.rs`.

| Field            | Type                                     | Required | Description                                                           |
| ---------------- | ---------------------------------------- | -------- | --------------------------------------------------------------------- |
| `schema_version` | `String`                                 | Yes      | Must be `"1"`. Validated before any other field is read.              |
| `name`           | `String`                                 | Yes      | Collection display name. Must be non-empty after trimming whitespace. |
| `description`    | `Option<String>`                         | No       | Human-readable description of the collection.                         |
| `defaults`       | `Option<CollectionDefaultsSection>`      | No       | Per-collection launch defaults (see merge-layer doc).                 |
| `profiles`       | `Vec<CollectionPresetProfileDescriptor>` | No       | Array of profile identity descriptors for matching.                   |

## Profile descriptor

Each entry in the `profiles` array is a `CollectionPresetProfileDescriptor` with three fields used for matching:

| Field                              | Type     | Description                                                                  |
| ---------------------------------- | -------- | ---------------------------------------------------------------------------- |
| `steam_app_id`                     | `String` | Steam App ID, resolved via `resolve_art_app_id()` at export time.            |
| `game_name`                        | `String` | The profile's `game.name` field.                                             |
| `trainer_community_trainer_sha256` | `String` | SHA-256 hash of the trainer binary, from `trainer.community_trainer_sha256`. |

## Matching order

On import, each descriptor is matched against local profiles in the following order:

1. **`steam_app_id`** via `resolve_art_app_id()` — if exactly one local profile shares the same resolved app ID, the match is confirmed.
2. **`(game_name, trainer_sha256)` pair fallback** — if the app ID match fails or is empty, the importer falls back to matching the game name and trainer hash together.
3. **Ambiguous resolution** — if multiple local profiles match the same descriptor (e.g., two profiles with the same Steam App ID), the entry is flagged as ambiguous and surfaced in the import review modal for the user to resolve manually.

Unmatched descriptors (no local profile found) are reported separately so the user knows which games are missing from their library.

## Forward compatibility

`schema_version` is validated in `CollectionPresetManifest::validate` and in `parse_collection_preset_toml` (`collection_exchange.rs`). The importer rejects **any non-empty** `schema_version` string other than `"1"`: during parsing, that case is surfaced as `CollectionExchangeError::UnsupportedSchemaVersion { version, supported }` (with `supported` set to the current `COLLECTION_PRESET_SCHEMA_VERSION`).

An **empty** `schema_version` (string `""`) is treated differently: `validate()` fails with `"collection preset must include schema_version"`, and the mapper returns `CollectionExchangeError::InvalidManifest` instead of `UnsupportedSchemaVersion`, because the manifest is missing a required version rather than declaring an unsupported future version.

This ensures older versions of CrossHook do not silently misinterpret data from a newer schema.

## Minimal example

```toml
schema_version = "1"
name = "Action RPGs"
description = "Souls-likes and action RPGs"

[defaults]
method = "proton_run"
network_isolation = false

[[profiles]]
steam_app_id = "1245620"
game_name = "ELDEN RING"
trainer_community_trainer_sha256 = ""

[[profiles]]
steam_app_id = "374320"
game_name = "DARK SOULS III"
trainer_community_trainer_sha256 = ""
```

## Roundtrip contract

The export-then-import roundtrip is tested at `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange.rs:492-537` (`export_preview_roundtrip_with_effective_app_id`). The test creates a profile with a runtime-resolved app ID, exports it to a TOML file, then imports and verifies that the manifest name, matched profiles, defaults, and descriptor counts all survive the roundtrip intact.

Run with:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core export_preview_roundtrip
```

## Source files

| File                                                                            | Purpose                                                                     |
| ------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/profile/collection_schema.rs`   | `CollectionPresetManifest`, `CollectionPresetProfileDescriptor`, validation |
| `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange.rs` | Export, import preview, matching logic, `CollectionExchangeError`           |
