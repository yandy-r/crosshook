# Plan: Lutris Profile Import for Migration

## Summary

Add a one-way importer that reads a user's existing Lutris game library (YAML
configs under `~/.config/lutris/` or `~/.local/share/lutris/`, plus `pga.db`) and
converts each Wine/Proton game into a native CrossHook `GameProfile` TOML. The
importer mirrors the existing community-JSON import pipeline: a read-only
**preview/prepare** pass followed by a **write** pass that saves profiles via
`ProfileStore::save` and syncs SQLite metadata with a new `lutris_import`
provenance source. Unmappable entries are skipped with a warning and never abort
the batch.

## User Story

As a Linux gamer migrating from Lutris to CrossHook, I want to import my existing
Lutris game launch configurations in bulk, so that I do not have to recreate every
profile (executable, prefix, runner, env vars, performance flags) by hand.

## Problem → Solution

**Current state**: Lutris holds the largest existing library of Linux game launch
configs, but CrossHook users must recreate each profile manually — no importer
exists (`grep -i lutris` over `src/crosshook-native/` returns nothing).

**Desired state**: A "Import from Lutris" entry point discovers the Lutris library
(with a file-picker fallback), previews each convertible game, maps runner/system
settings onto CrossHook profile fields, and writes standard TOML profiles that
appear in the normal profile list immediately and pass validation.

## Metadata

- **Complexity**: Large (10+ files across core + IPC + frontend)
- **Source PRD**: N/A (Forgejo issue #2 — `feat(profiles): Lutris profile import for migration`)
- **Forgejo Issue**: #2 (`Closes #2`)
- **PRD Phase**: N/A
- **Estimated Files**: ~16 (10 core, 2 IPC, 1 capability/registration, 3 frontend) + tests
- **Labels**: `type:feature`, `area:profiles`, `platform:linux`, `priority:low` (P3)

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run
concurrently; batches run in order. **No two tasks in the same batch touch the same
file.**

| Batch | Tasks              | Depends On | Parallel Width |
| ----- | ------------------ | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3, 1.4 | —          | 4              |
| B2    | 2.1, 2.2, 2.3      | B1         | 3              |
| B3    | 3.1                | B2         | 1              |
| B4    | 4.1                | B3         | 1              |
| B5    | 5.1, 5.2           | B4         | 2              |
| B6    | 6.1, 6.2, 6.3      | B5         | 3              |
| B7    | 7.1                | B6         | 1              |

- **Total tasks**: 15
- **Total batches**: 7
- **Max parallel width**: 4

---

## UX Design

### Before

```
Lutris user → CrossHook → (no migration path) → recreate every profile by hand
```

### After

```
Community route ─► "Import from Lutris" button
        │
        ▼
  lutris_prepare_import (auto-detect dir; file picker fallback)
        │
        ▼
  Preview list:  ☑ Game A (proton_run)   mapped: exe, prefix, env, dxvk
                 ☑ Game B (wine→proton)   ⚠ runner build not installed
                 ☐ Game C                 ⚠ skipped: no exe
        │  (user deselects / confirms)
        ▼
  lutris_import_profiles → writes <name>.toml ×N → profiles-changed event
        │
        ▼
  Result: 2 imported, 1 skipped (warnings shown), profiles appear in list
```

### Interaction Changes

| Touchpoint                  | Before        | After                                                          | Notes                                                          |
| --------------------------- | ------------- | -------------------------------------------------------------- | -------------------------------------------------------------- |
| Community Profiles section  | "Import JSON" | "Import JSON" + "Import from Lutris"                           | Sibling button mirroring the JSON import flow                  |
| Import wizard/review modal  | n/a           | Multi-entry review list with per-game mapped fields + warnings | Reuse `ProfileReviewModal`/`MigrationReviewModal` BEM + layout |
| Lutris dir not found        | n/a           | Native folder picker fallback (`chooseDirectory`)              | `dialog:default` capability already granted                    |
| Imported profile provenance | n/a           | SQLite `profiles.source = "lutris_import"`                     | Not prominently surfaced in UI; available in metadata/audit    |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                                       | Lines                    | Why                                                                              |
| -------------- | ------------------------------------------------------------------------------------------ | ------------------------ | -------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/profile/exchange/import.rs`                | 11-73                    | Closest analog: parse external file → hydrate → `save`; preview/result split     |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs`                 | 10-32, 201-291           | `GameProfile` shape (import target) + `storage_profile()` local_override split   |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/profile/models/legacy.rs`                  | 9-94                     | `impl From<external> for GameProfile` mapping idiom + `derive_*` helpers         |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/profile/migration/types.rs`                | 30-99                    | Per-entry outcome + tally pattern (`Applied/Failed/Skipped`, counts)             |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs`               | 97-126, 382-396, 443-473 | `save`, `import_legacy`, `validate_name`, unique-name derivation                 |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`                  | 7-130                    | `LaunchSection` fields: method, optimizations IDs, custom_env_vars, command_args |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/profile/models/runtime.rs`                 | 5-52                     | `RuntimeSection`: prefix_path, proton_path, working_directory, umu fields        |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/profile/exchange/error.rs`                 | 7-65                     | Hand-rolled IPC error enum (Serialize/Deserialize, `From<ProfileStoreError>`)    |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/profile/exchange/utils.rs`                 | 55-122, 188-223          | `derive_import_name`, `sanitize_profile_name`, hydration idiom                   |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`                        | 74-100                   | `SyncSource` closed enum + `as_str()` (add `LutrisImport` variant here)          |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`                  | 11-70                    | `observe_profile_write` upsert into `profiles` table                             |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/settings/paths.rs`                         | 7-16                     | Tilde/home path resolution helpers to reuse for Lutris dir discovery             |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/community.rs`                                 | 95-182                   | Two-command import (prepare + import) + fail-soft metadata sync at IPC layer     |
| P1 (important) | `src/crosshook-native/src/components/CommunityBrowser.tsx`                                 | 60-164                   | Picker → prepare → wizard wiring to mirror for the Lutris button                 |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange/import.rs`     | 20-62                    | Multi-entry classify into matched/ambiguous/unmatched buckets                    |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/profile/migration/scan.rs`                 | 17-46                    | `&mut Vec<String> diagnostics` accumulator + `Err(_) => continue` skip idiom     |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/import_legacy.rs` | 5-27                     | Tempdir import test pattern                                                      |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/tests/fixtures.rs`      | 3-67                     | `sample_profile()` fixture builder                                               |
| P2 (reference) | `src/crosshook-native/assets/default_optimization_catalog.toml`                            | 51-273                   | Optimization IDs (`disable_esync`, `enable_dxvk_async`, `use_gamemode`, …)       |
| P2 (reference) | `src/crosshook-native/src-tauri/src/commands/export.rs`                                    | 455-527                  | IPC contract test (`command_names_match_expected_ipc_contract`) to extend        |
| P2 (reference) | `src/crosshook-native/src/utils/dialog.ts`                                                 | 8-107                    | `chooseFile`/`chooseDirectory` portal-safe pickers                               |

## External Documentation

| Topic                         | Source                                                                                                                                  | Key Takeaway                                                                                                            |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Lutris config layout          | https://deepwiki.com/lutris/lutris/2.3-configuration-system                                                                             | Per-game config at `games/<configpath>.yml`; sections `game:`, `system:`, runner-named (`wine:`)                        |
| Lutris wine runner options    | https://github.com/lutris/lutris/blob/master/lutris/runners/wine.py                                                                     | `game.exe/args/working_dir/prefix/arch`; `wine.version/dxvk/vkd3d/dxvk_nvapi/esync/fsync` (defaults mostly `true`)      |
| Lutris system options         | https://github.com/lutris/lutris/blob/master/lutris/sysoptions.py                                                                       | `system.env` (dict), `terminal`, `gamemode`, `mangohud`, `prefer_system_libs`, gamescope family                         |
| Lutris paths / pga.db         | https://github.com/lutris/lutris/blob/master/lutris/settings.py , https://github.com/lutris/lutris/blob/master/lutris/database/games.py | Config-dir deprecation drift; `pga.db.games(configpath, runner, directory, name, slug)`; Flatpak + balanced-dir layouts |
| Proton convergence (v0.5.20+) | https://alternativeto.net/news/2026/2/lutris-v0-5-20-sets-proton-ge-via-umu-by-default-adds-new-sources                                 | For Proton runners, Lutris ignores vkd3d/nvapi (Proton-internal); only esync/fsync/dxvk passthrough                     |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### MODULE_LAYOUT (module-as-directory with `mod.rs` re-exports)

```rust
// SOURCE: profile/exchange/mod.rs:8-11
pub use import::{import_community_profile, preview_community_profile_import};
// New module mirrors this: profile/lutris_import/mod.rs re-exports public fns + types.
```

### EXTERNAL*TO_PROFILE_MAPPING (From impl + derive*\* helpers)

```rust
// SOURCE: profile/models/legacy.rs:37-66
impl From<LegacyProfileData> for GameProfile {
    fn from(value: LegacyProfileData) -> Self { /* field-by-field; derive_* for fuzzy */ }
}
// Mirror: impl TryFrom<LutrisGameConfig> for GameProfile (TryFrom — mapping can fail per-entry).
```

### PREVIEW_RESULT_SPLIT (dry-run DTO + post-write DTO, both Serde)

```rust
// SOURCE: profile/exchange/types.rs:6-29
pub struct CommunityImportPreview { /* parsed, not written */ }
pub struct CommunityImportResult { pub profile_name: String, pub profile: GameProfile, /* ... */ }
```

### PER_ENTRY_OUTCOME (batch never aborts; tally counts)

```rust
// SOURCE: profile/migration/types.rs:59-99
pub enum MigrationOutcome { Applied, AlreadyValid, Failed }
pub struct BatchMigrationResult { pub results: Vec<MigrationApplyResult>, pub applied_count, failed_count, skipped_count }
// Mirror: LutrisImportOutcome { Imported, Skipped, Failed } + LutrisImportResult { imported_count, skipped_count, ... }.
```

### SKIP_WITH_WARNING (diagnostics accumulator + continue)

```rust
// SOURCE: profile/migration/scan.rs:21,46
diagnostics.push(format!("Could not list profiles: {err}"));
// ...
Err(_) => continue,   // unmappable entry skipped, batch continues
```

### ERROR_ENUM (hand-rolled, Serde for IPC; NOT anyhow/thiserror in profile path)

```rust
// SOURCE: profile/exchange/error.rs:7 + collection_exchange/error.rs:12-36
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CommunityExchangeError { Io { action, path, message }, Json { path, message }, ProfileStore(...) }
// Mirror: LutrisImportError { Io{...}, Yaml{path,message}, Pga{...}, ProfileStore(...) } with manual Display/Error + From<ProfileStoreError>.
```

### SAVE_THEN_SYNC (core saves TOML; IPC layer syncs metadata, fail-soft)

```rust
// SOURCE: src-tauri/src/commands/community.rs:108-119
// after store.save:
if let Err(e) = metadata_store.observe_profile_write(&name, &profile, &path, SyncSource::Import, None) {
    tracing::warn!(%e, profile_name = %name, "metadata sync after import failed");
}
// Mirror with SyncSource::LutrisImport.
```

### NAME_DERIVATION (slug + validate + uniqueness)

```rust
// SOURCE: profile/exchange/utils.rs:188-223 + toml_store/store.rs:443,470-473
fn sanitize_profile_name(name) { /* lowercase, dash, strip invalid */ }
// validate_name rejects / \ : < > " | ? * and . / .. ; generate_unique_copy_name for collisions.
```

### TAURI*COMMAND (snake_case, thin, State<'*, Store>, Result<DTO, String>)

```rust
// SOURCE: src-tauri/src/commands/migration.rs:24-25
#[tauri::command]
pub fn check_proton_migrations(steam_client_install_path: Option<String>, store: State<'_, ProfileStore>) -> Result<..., String>
// Frontend invoke arg keys MUST be camelCase (Rust snake_case params).
```

### TEST_STRUCTURE (tempdir + fixture builder)

```rust
// SOURCE: profile/toml_store/tests/import_legacy.rs:5-27
let temp_dir = tempdir().unwrap();
// write fixture YAML, run import, assert <name>.toml exists on disk.
```

### FRONTEND_PICKER_TO_WIZARD (picker → prepare → review modal)

```ts
// SOURCE: src/components/CommunityBrowser.tsx:150-164
const path = await chooseCommunityProfileImport();
const draft = await prepareCommunityImport(path);
setImportDraft(draft); // opens wizard/review modal
```

---

## Files to Change

| File                                                                                | Action | Justification                                                                        |
| ----------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------ |
| `src/crosshook-native/crates/crosshook-core/Cargo.toml`                             | UPDATE | Add YAML parser dependency (`serde_yaml_ng`)                                         |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/mod.rs`       | CREATE | Module root: declare submodules, re-export public fns/types                          |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/error.rs`     | CREATE | `LutrisImportError` enum (Serde, IPC-safe)                                           |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/types.rs`     | CREATE | DTOs: `LutrisImportPreview`, `LutrisImportEntry`, `LutrisImportResult`, outcome enum |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/paths.rs`     | CREATE | Discover Lutris dirs (config/data/Flatpak/balanced), read `pga.db` games             |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/parse.rs`     | CREATE | Lutris YAML deserialization structs (`game`, `system`, `wine`) — all `Option`        |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/runner.rs`    | CREATE | Classify/resolve `wine.version` → Proton path + installed-vs-missing state           |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/map.rs`       | CREATE | `TryFrom<LutrisGameConfig> for GameProfile` field mapping + optimization IDs         |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/import.rs`    | CREATE | Orchestration: scan → preview (read-only) → apply (save + per-entry outcome)         |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/tests/mod.rs` | CREATE | Test module wiring                                                                   |
| `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/tests/*.rs`   | CREATE | Parse, map, paths, import unit tests with fixtures                                   |
| `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`                     | UPDATE | Register `pub mod lutris_import;` + re-export                                        |
| `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`                 | UPDATE | Add `SyncSource::LutrisImport => "lutris_import"`                                    |
| `src/crosshook-native/src-tauri/src/commands/lutris.rs`                             | CREATE | `lutris_prepare_import` + `lutris_import_profiles` Tauri commands                    |
| `src/crosshook-native/src-tauri/src/commands/mod.rs`                                | UPDATE | `pub mod lutris;`                                                                    |
| `src/crosshook-native/src-tauri/src/lib.rs`                                         | UPDATE | Register both commands in `generate_handler!`                                        |
| `src/crosshook-native/src-tauri/src/commands/export.rs` (or test module)            | UPDATE | Extend IPC contract test with the two new fn signatures                              |
| `src/crosshook-native/src/hooks/useLutrisImport.ts`                                 | CREATE | Hook wrapping `invoke()` for prepare/import (camelCase arg keys)                     |
| `src/crosshook-native/src/components/LutrisImportModal.tsx`                         | CREATE | Multi-entry review modal (mirror `MigrationReviewModal`/`ProfileReviewModal`)        |
| `src/crosshook-native/src/components/community/CommunityProfilesSection.tsx`        | UPDATE | Add "Import from Lutris" button + picker→prepare→modal wiring                        |

## NOT Building

- **Export to Lutris / round-trip / two-way sync** — import is one-directional only.
- **Live Lutris watching or incremental re-import** — single on-demand pass.
- **Importing non-Wine/non-Proton runners** (scummvm, libretro, flatpak, native Linux Lutris entries) — out of CrossHook's Proton/Wine orchestration scope; filter via `pga.db.runner`.
- **Perfectly reproducing Lutris's runtime-computed defaults** (`is_fsync_supported`, `gamemode_available`) — record absent-vs-explicit; do not host-probe.
- **Resolving/installing missing Proton/Wine builds** referenced by `wine.version` — surface an "unresolved runner" warning; leave `runtime.proton_path` to creation defaults.
- **New SQLite schema migration** — `source` is free-form TEXT; only a Rust `SyncSource` enum variant is added (schema stays v24).
- **Importing Lutris cover art / banners / box art** — only launch config is mapped.
- **A dedicated onboarding/first-run page** — entry point lives on the existing Community route.

---

## Step-by-Step Tasks

> Module skeleton note: Task 1.2 creates `lutris_import/mod.rs` declaring **all** planned
> submodules and registers `pub mod lutris_import;` in `profile/mod.rs`, so sibling-file
> tasks in later batches never touch `mod.rs` and never conflict. The module tree only
> fully compiles once its batch completes — per-task VALIDATE for foundation tasks may use
> `cargo check` that succeeds after the batch; the full build gate is at Acceptance.

### Task 1.1: Add YAML parser dependency — Depends on [none]

- **BATCH**: B1
- **ACTION**: Add a maintained YAML parser to `crosshook-core`.
- **IMPLEMENT**: In `crates/crosshook-core/Cargo.toml` `[dependencies]`, add `serde_yaml_ng = "0.10"` (actively-maintained `serde_yaml` fork; `serde_yaml` itself is archived). Use a bare version string — the workspace has no `[workspace.dependencies]` table.
- **MIRROR**: Dependency declaration style at `crates/crosshook-core/Cargo.toml:13-49` (bare versions, no `.workspace = true`).
- **GOTCHA**: Do NOT add `anyhow` — the profile path uses hand-rolled error enums. Justify the new dep in the PR (no existing YAML parser; `quick-xml`/`csv`/`serde_json` cannot read YAML).
- **VALIDATE**: `cargo build --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` resolves the new dep.

### Task 1.2: Create module skeleton + register — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `lutris_import/mod.rs` and register the module.
- **IMPLEMENT**: `lutris_import/mod.rs` declares `mod error; mod types; mod paths; mod parse; mod runner; mod map; mod import; #[cfg(test)] mod tests;` and `pub use` the public API (preview/import fns + DTOs + error). Add `pub mod lutris_import;` to `profile/mod.rs` (and re-export per `profile/mod.rs` conventions).
- **MIRROR**: `MODULE_LAYOUT` (`profile/exchange/mod.rs:8-11`); module-as-dir with `mod.rs` like `profile/migration/mod.rs:6-19`.
- **GOTCHA**: Declaring submodules before their files exist won't compile standalone — that's expected; B2/B3 fill them. Keep all `mod`/`pub use` lines here so no later task edits `mod.rs`.
- **VALIDATE**: File compiles after sibling files land; `cargo check -p crosshook-core` clean at end of B4.

### Task 1.3: Error + DTO types — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `lutris_import/error.rs` and `lutris_import/types.rs`.
- **IMPLEMENT**: `error.rs`: `LutrisImportError` enum with `Io { action, path, message }`, `Yaml { path, message }`, `Pga { message }`, `ProfileStore(...)` variants; manual `Display`/`std::error::Error` + `From<ProfileStoreError>`; derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`. `types.rs`: `LutrisImportPreview { entries: Vec<LutrisImportEntry>, lutris_root: PathBuf, diagnostics: Vec<String> }`, `LutrisImportEntry { source_path, suggested_name, game_name, runner, mapped: GameProfile, warnings: Vec<String>, importable: bool }`, `LutrisImportOutcome { Imported, Skipped, Failed }`, `LutrisImportResult { results: Vec<LutrisImportEntryResult>, imported_count, skipped_count, failed_count }`. All DTOs derive `Serialize, Deserialize`.
- **MIRROR**: `ERROR_ENUM` (`exchange/error.rs:7`, `collection_exchange/error.rs:12-36`); `PREVIEW_RESULT_SPLIT` (`exchange/types.rs:6-29`); `PER_ENTRY_OUTCOME` (`migration/types.rs:59-99`).
- **GOTCHA**: Two files but one batch task — both are new sibling files, no conflict. `GameProfile` import comes from `crate::profile::models::GameProfile`.
- **VALIDATE**: Types compile once `mod.rs` (1.2) declares them; `cargo check -p crosshook-core` at end of batch.

### Task 1.4: Add `SyncSource::LutrisImport` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Add the provenance variant.
- **IMPLEMENT**: In `metadata/models.rs`, add `LutrisImport` to the `SyncSource` enum and map it in `as_str()` to `"lutris_import"`. No DB migration — `profiles.source`/`profile_name_history.source` are free-form `TEXT` (schema v24 unchanged).
- **MIRROR**: `metadata/models.rs:74-100` existing variants (`Import => "import"`, snake_case via `#[serde(rename_all)]`).
- **GOTCHA**: It is a **closed** enum mapped by `as_str()`; you must add the Rust variant, not just pass a string. Confirm round-trip (string → variant) helper if one exists.
- **VALIDATE**: `cargo test -p crosshook-core` (existing metadata tests still pass); grep that `as_str` covers all variants.

### Task 2.1: Lutris YAML parse structs — Depends on [1.1, 1.2]

- **BATCH**: B2
- **ACTION**: Create `lutris_import/parse.rs`.
- **IMPLEMENT**: Serde structs for a per-game YAML: top-level `name: Option<String>`, `#[serde(rename = "game-slug")] game_slug: Option<String>`, `game: Option<LutrisGameSection>`, `system: Option<LutrisSystemSection>`, `wine: Option<LutrisWineSection>`. `LutrisGameSection { exe, args, working_dir, prefix, arch }` (all `Option`). `LutrisSystemSection { env: Option<BTreeMap<String,String>>, terminal, gamemode, mangohud, prefer_system_libs, gamescope*, ... }`. `LutrisWineSection { version, dxvk, vkd3d, dxvk_nvapi, esync, fsync }` (all `Option<bool>` except `version: Option<String>`). Provide `fn parse_lutris_yaml(path: &Path) -> Result<LutrisGameConfig, LutrisImportError>` using `serde_yaml_ng::from_str`.
- **MIRROR**: `LegacyProfileData` serde-rename idiom (`models/legacy.rs:9-35`).
- **GOTCHA**: EVERY field `Option` — absent means "inherit Lutris default", not "off". `env` values are quoted strings (`'1'`, `'true'`) — keep as `String`, never coerce to bool/int. `game-slug` is hyphenated. Empty sections serialize as `{}`.
- **VALIDATE**: Unit test parses the representative fixture (see External Docs §2) and a minimal `game: { exe }`-only file.

### Task 2.2: Lutris dir discovery + pga.db reader — Depends on [1.2, 1.3]

- **BATCH**: B2
- **ACTION**: Create `lutris_import/paths.rs`.
- **IMPLEMENT**: `fn discover_lutris_root() -> Option<PathBuf>` checking, in order: `~/.config/lutris/`, `~/.local/share/lutris/`, Flatpak `~/.var/app/net.lutris.Lutris/{config,data}/lutris/`. `fn list_game_configs(root) -> Vec<PathBuf>` globbing both `games/*.yml` and `games/*/*.yml` (balanced dirs). `fn read_pga_games(root) -> Result<Vec<PgaGame>, LutrisImportError>` opening `<root>/pga.db` read-only via `rusqlite` and selecting `name, slug, runner, directory, configpath` from `games`. Use `directories::BaseDirs` + `settings/paths.rs` tilde helpers for home resolution.
- **MIRROR**: `settings/paths.rs:7-16` (home/tilde resolution); `directories::BaseDirs` usage at `metadata/store.rs:18-22`. `rusqlite` open pattern from `metadata/store.rs`.
- **GOTCHA**: `configpath` is the YAML basename WITHOUT `.yml`. pga.db and YAML can drift; use pga.db `runner` to FILTER to wine/proton and to RECOVER `name`/`slug` when the YAML omits them. Open pga.db read-only (do not write to the user's Lutris DB). Lutris may not be installed → return `None`/empty cleanly.
- **VALIDATE**: Unit test with a temp dir containing `games/*.yml` and a tiny `pga.db` fixture (or skip pga.db test when feature-gated); discovery returns the temp root.

### Task 2.3: Runner version classification/resolution — Depends on [1.2, 1.3]

- **BATCH**: B2
- **ACTION**: Create `lutris_import/runner.rs`.
- **IMPLEMENT**: `fn classify_runner(wine_version: &str) -> RunnerKind` where substring `proton` (case-insensitive) ⇒ Proton, else Wine. `fn resolve_runner_path(root, version) -> RunnerResolution { Installed(PathBuf) | Missing(String) }` checking `<root>/runners/wine/<version>/`. Provide a mapping decision: all in-scope wine/proton games map to CrossHook `launch.method = "proton_run"` (no first-class bare-Wine method exists).
- **MIRROR**: Path-join + existence checks like `paths.rs`/`settings/paths.rs`.
- **GOTCHA**: A referenced build may be uninstalled → return `Missing` and let the caller add a warning; do NOT block import. CrossHook has no Proton/Wine _version-name_ field — only `runtime.proton_path` (a filesystem path). If `Missing`, leave `proton_path` empty so creation defaults fill the user's default Proton.
- **VALIDATE**: Unit test: `lutris-GE-Proton8-14-x86_64` ⇒ Proton; missing dir ⇒ `Missing`.

### Task 3.1: Map Lutris config → GameProfile — Depends on [2.1, 2.2, 2.3]

- **BATCH**: B3
- **ACTION**: Create `lutris_import/map.rs`.
- **IMPLEMENT**: `impl TryFrom<MappedLutrisInput> for GameProfile` (input bundles parsed YAML + pga row + runner resolution). Map: `game.name`←`name`/pga; `game.executable_path`←`game.exe`; `runtime.prefix_path`←`game.prefix` (resolve `~`/relative, fall back to pga `directory`); `runtime.working_directory`←`game.working_dir` (fall back to dir of exe); `runtime.proton_path`←resolved Proton path (empty if Missing); `launch.method="proton_run"`; `launch.custom_env_vars`←`system.env` (verbatim strings); `launch.command_arguments.custom_args`←split `game.args`. Optimization IDs into `launch.optimizations.enabled_option_ids`: `esync == Some(false)` ⇒ `disable_esync`; `fsync == Some(false)` ⇒ `disable_fsync`; `gamemode == Some(true)` ⇒ `use_gamemode`; `mangohud == Some(true)` ⇒ `show_mangohud_overlay`. Gamescope `system.gamescope == Some(true)` ⇒ populate `launch.gamescope` (`GamescopeConfig`). Collect a `warnings: Vec<String>` for unresolved runner, no-exe (→ not importable), Proton-ignored vkd3d/nvapi.
- **MIRROR**: `EXTERNAL_TO_PROFILE_MAPPING` (`models/legacy.rs:37-94`); optimization IDs from `assets/default_optimization_catalog.toml:51-273`.
- **GOTCHA**: esync/fsync are **negative** toggles — Lutris `esync: true` (the default) maps to NO option; only `false` adds `disable_esync`. Optimizations are valid ONLY for `proton_run` — fine here. No exe ⇒ mark entry `importable = false`, skip on apply. `arch: "auto"` ⇒ leave bitness to defaults. Do not set `local_override` (machine paths handled by `save()`'s `storage_profile()`).
- **VALIDATE**: Unit tests covering: full mapping, esync-false adds option, missing exe ⇒ not importable, env strings preserved verbatim.

### Task 4.1: Import orchestration (scan/preview/apply) — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Create `lutris_import/import.rs`.
- **IMPLEMENT**: `pub fn preview_lutris_import(root: Option<PathBuf>) -> Result<LutrisImportPreview, LutrisImportError>`: discover root (or use supplied dir), read pga.db, list game YAMLs, parse each, map to `GameProfile`, build `LutrisImportEntry` (suggested unique name via `sanitize_profile_name` + collision check against `ProfileStore`), accumulate per-entry `warnings` and top-level `diagnostics` for unreadable files (`Err(_) => continue`). `pub fn apply_lutris_import(profiles_dir, entries: Vec<LutrisImportEntry>) -> LutrisImportResult`: for each importable entry, `ProfileStore::with_base_path(profiles_dir).save(name, &profile)`, build per-entry outcome (`Imported`/`Skipped`/`Failed`), never abort the batch; tally counts.
- **MIRROR**: `profile/exchange/import.rs:11-73` (parse→hydrate→save); `collection_exchange/import.rs:20-62` (multi-entry buckets); `migration/scan.rs:17-46` (diagnostics + continue); `SKIP_WITH_WARNING`; `NAME_DERIVATION` (`exchange/utils.rs:188-223`, `store.rs:443`).
- **GOTCHA**: `preview` is READ-ONLY — never writes. `apply` returns a result struct (not `Result`) so one bad entry doesn't fail the rest. Core `save` does NOT sync SQLite — metadata sync happens in the IPC layer (Task 5.1). Resolve the merged Lutris value (game > runner > system) only if cheaply available; otherwise read `games/*.yml` and note the limitation in diagnostics.
- **VALIDATE**: `cargo test -p crosshook-core` — preview over a temp Lutris tree returns N entries; apply writes the importable `.toml` files (tempdir assert exists).

### Task 5.1: Tauri commands — Depends on [4.1, 1.4]

- **BATCH**: B5
- **ACTION**: Create `src-tauri/src/commands/lutris.rs`.
- **IMPLEMENT**: `#[tauri::command] pub fn lutris_prepare_import(directory: Option<String>, profile_store: State<'_, ProfileStore>) -> Result<LutrisImportPreview, String>` (delegates to `preview_lutris_import`; needs the store for name-collision checks). `#[tauri::command] pub fn lutris_import_profiles(entries: Vec<LutrisImportEntry>, profile_store: State<'_, ProfileStore>, metadata_store: State<'_, MetadataStore>, app: AppHandle) -> Result<LutrisImportResult, String>`: call `apply_lutris_import`, then for each imported profile fail-soft `metadata_store.observe_profile_write(&name, &profile, &path, SyncSource::LutrisImport, None)` with `tracing::warn!` on error, then `emit_profiles_changed(&app, ...)`. Map core errors via the existing IPC error-mapping helper.
- **MIRROR**: `SAVE_THEN_SYNC` + two-command split (`commands/community.rs:95-182`); `TAURI_COMMAND` shape (`commands/migration.rs:24-25`); `emit_profiles_changed` (`commands/profile/shared.rs:68-70`).
- **GOTCHA**: Thin handlers only — business logic stays in `crosshook-core`. Serde on all DTOs (already done in 1.3). Frontend will pass camelCase keys (`directory`, `entries`).
- **VALIDATE**: `cargo build -p crosshook` (src-tauri); commands compile with correct signatures.

### Task 5.2: Register commands + contract test — Depends on [4.1, 1.4]

- **BATCH**: B5
- **ACTION**: Wire the commands into the app.
- **IMPLEMENT**: Add `pub mod lutris;` to `src-tauri/src/commands/mod.rs`. Add `commands::lutris::lutris_prepare_import,` and `commands::lutris::lutris_import_profiles,` to `tauri::generate_handler!` in `src-tauri/src/lib.rs`. Extend the IPC contract test (`command_names_match_expected_ipc_contract`, `commands/export.rs:455-527`) with both `as fn(...) -> Result<...>` casts.
- **MIRROR**: `lib.rs:401,416-420` handler registration; `export.rs:455-527` contract test.
- **GOTCHA**: Different files from 5.1 (mod.rs, lib.rs, export.rs) — safe to run alongside 5.1 only if 5.1's `lutris.rs` exists first. **Therefore 5.2 depends on 5.1's file existing**; to stay batch-safe, treat 5.2 as same-batch-after or move to its own batch if executor cannot order intra-batch. Conservative: keep 5.1 and 5.2 in B5 but note 5.2 references symbols from 5.1.
- **VALIDATE**: `cargo test -p crosshook` — contract test passes; app builds.

### Task 6.1: Frontend IPC hook — Depends on [5.1, 5.2]

- **BATCH**: B6
- **ACTION**: Create `src/hooks/useLutrisImport.ts`.
- **IMPLEMENT**: Hook exposing `prepare(directory?: string)` → `callCommand<LutrisImportPreview>('lutris_prepare_import', { directory: directory ?? null })` and `importProfiles(entries)` → `callCommand<LutrisImportResult>('lutris_import_profiles', { entries })`, with loading/error state via `normalizeError`. Add the TS DTO types (`LutrisImportPreview`, `LutrisImportEntry`, `LutrisImportResult`) mirroring the Serde structs.
- **MIRROR**: `useProtonMigration.ts:7-72` (normalizeError, camelCase keys, `{ request }`-style wrapping); `useCommunityProfiles.ts` prepare/import.
- **GOTCHA**: invoke arg keys MUST be camelCase (`directory`, `entries`) — Rust params are snake_case. `Option<String>` → pass `?? null`.
- **VALIDATE**: `npm run typecheck` (in `src/crosshook-native/`); hook types align with Rust DTOs.

### Task 6.2: Lutris import modal — Depends on [5.1, 5.2]

- **BATCH**: B6
- **ACTION**: Create `src/components/LutrisImportModal.tsx`.
- **IMPLEMENT**: Portal modal listing preview entries with per-game checkbox (default-on for importable), mapped-field summary, and warning badges; "Import selected" calls `importProfiles`; result phase shows imported/skipped/failed counts. Reuse `crosshook-modal__*` BEM + `var(--crosshook-color-*)`.
- **MIRROR**: `MigrationReviewModal.tsx:21-339` (checkbox multi-select + result phase); `CommunityImportWizardModal.tsx` step layout.
- **GOTCHA**: Reuse `ProfileReviewModal`/`.crosshook-modal__body` so NO new `overflow-y:auto` container is introduced — otherwise it must be added to the `SCROLLABLE` selector in `hooks/useScrollEnhance.ts:9`. Add `overscroll-behavior: contain` to any inner scroll area.
- **VALIDATE**: `npm run typecheck`; `npm test` (Vitest) for any added component logic; manual render in browser dev mode.

### Task 6.3: Community section entry point — Depends on [5.1, 5.2]

- **BATCH**: B6
- **ACTION**: Update `src/components/community/CommunityProfilesSection.tsx`.
- **IMPLEMENT**: Add an "Import from Lutris" button beside "Import JSON". On click: try auto-detect via `prepare()`; if the preview's `lutris_root` is empty/none, call `chooseDirectory('Select Lutris config directory')` and re-`prepare(dir)`. Open `LutrisImportModal` with the preview.
- **MIRROR**: `CommunityProfilesSection.tsx:72-73` import button; `CommunityBrowser.tsx:150-164` picker→prepare→modal wiring; `utils/dialog.ts` `chooseDirectory`.
- **GOTCHA**: `dialog:default` capability already granted (`src-tauri/capabilities/default.json:8`). Keep the button disabled while a prepare/import is in flight.
- **VALIDATE**: `npm run typecheck`; `npm run test:smoke` (browser dev mode) renders the button; manual click flow.

### Task 7.1: Tests, lint, docs, full validation — Depends on [6.1, 6.2, 6.3]

- **BATCH**: B7
- **ACTION**: Close out: complete test coverage, run all validators, update docs.
- **IMPLEMENT**: Ensure `lutris_import/tests/` covers parse, map (incl. esync-false, missing-exe, env-verbatim), paths discovery, preview/apply (tempdir). Add the new `metadata.db` schema note is NOT needed (no migration). Update `AGENTS.md`/`CLAUDE.md` only if a new persisted datum or convention warrants it (the `lutris_import` SyncSource value). Run host-gateway check (no host-tool spawns added).
- **MIRROR**: `TEST_STRUCTURE` (`tests/import_legacy.rs:5-27`, `tests/fixtures.rs:3-67`).
- **GOTCHA**: The importer reads files only — it must NOT spawn `proton`/`umu-run`/etc., so `scripts/check-host-gateway.sh` stays green without changes. Do not write to the user's `pga.db`.
- **VALIDATE**: Full validation command block below; all green.

---

## Testing Strategy

### Unit Tests

| Test                             | Input                                                | Expected Output                                           | Edge Case?            |
| -------------------------------- | ---------------------------------------------------- | --------------------------------------------------------- | --------------------- |
| `parse_full_yaml`                | Representative wine game YAML (External Docs §2)     | Struct with exe/prefix/env/wine.version populated         | No                    |
| `parse_minimal_yaml`             | `game: { exe: ... }` only                            | All other fields `None`                                   | Yes (sparse)          |
| `parse_env_strings_verbatim`     | `system.env: { WINEDLLOVERRIDES: 'd3d11=', X: '1' }` | env values kept as `String` `"1"`, not bool/int           | Yes                   |
| `map_esync_false_adds_disable`   | `wine.esync: false`                                  | `enabled_option_ids` contains `disable_esync`             | Yes (negative toggle) |
| `map_esync_true_no_option`       | `wine.esync: true`/absent                            | NO `disable_esync` option                                 | Yes (default)         |
| `map_missing_exe_not_importable` | no `game.exe`                                        | entry `importable = false`                                | Yes                   |
| `map_working_dir_fallback`       | `working_dir` absent, `exe` set                      | `runtime.working_directory` = dir of exe                  | Yes                   |
| `runner_classify_proton`         | `lutris-GE-Proton8-14-x86_64`                        | `RunnerKind::Proton`, method `proton_run`                 | No                    |
| `runner_missing_build`           | version dir not on disk                              | `Missing`, warning added, `proton_path` empty             | Yes                   |
| `discover_lutris_root`           | temp `~/.config/lutris/games/*.yml`                  | returns temp root                                         | No                    |
| `discover_balanced_dirs`         | `games/c/foo-123.yml`                                | YAML enumerated                                           | Yes                   |
| `preview_readonly`               | temp Lutris tree                                     | N entries, NO files written                               | Yes                   |
| `apply_writes_importable`        | preview entries                                      | `.toml` exists; skipped entry not written; counts correct | Yes                   |
| `apply_one_bad_entry`            | one entry with save failure                          | batch continues; `failed_count == 1`                      | Yes (resilience)      |
| `sync_source_lutris_import`      | `SyncSource::LutrisImport`                           | `as_str() == "lutris_import"`                             | No                    |

### Edge Cases Checklist

- [ ] Lutris not installed → discovery returns `None`/empty, no panic
- [ ] Empty `system: {}` / `wine: {}` maps → defaults, no panic
- [ ] YAML with only `game.exe` (locally-added game, name from pga.db)
- [ ] Duplicate profile names across multiple Lutris games → unique-name collision handling
- [ ] Unreadable / malformed YAML file → diagnostic + `continue` (other entries still import)
- [ ] Proton runner with `vkd3d: false` → advisory warning (Proton ignores it), not applied
- [ ] Relative / `~` prefix path → resolved; absent prefix → pga.db `directory` fallback
- [ ] Read-only `pga.db` (never written by import)

---

## Validation Commands

### Static Analysis

```bash
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -p crosshook --all-targets -- -D warnings
cd src/crosshook-native && npm run typecheck
```

EXPECT: Zero clippy warnings, zero type errors

### Unit Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cd src/crosshook-native && npm test
```

EXPECT: All tests pass (new `lutris_import` suite + existing)

### Full Build

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml
./scripts/build-release-binary.sh   # optional: production Tauri binary
```

EXPECT: Workspace builds; IPC contract test passes

### Gateway / Lint / Format

```bash
./scripts/check-host-gateway.sh     # importer adds no host-tool spawns
./scripts/lint.sh
./scripts/format.sh
```

EXPECT: Gateway green (no new denylisted `Command::new`), lint/format clean

### Browser Validation

```bash
./scripts/dev-native.sh --browser
```

EXPECT: "Import from Lutris" button renders; modal opens (mock layer in browser dev mode)

### Manual Validation

- [ ] On a host with Lutris installed: button auto-detects the library and lists games
- [ ] Deselect one game → only selected games are imported
- [ ] No Lutris dir → folder picker fallback appears and a chosen dir is parsed
- [ ] Imported profiles appear in the normal profile list and open/edit like any profile
- [ ] Imported profile launches via Proton (acceptance: launch methods/WINE settings mapped)
- [ ] SQLite `profiles.source = "lutris_import"` for imported rows

---

## Acceptance Criteria

- [ ] Lutris YAML game configs can be imported as CrossHook profiles (issue AC #1)
- [ ] Launch methods and WINE settings are correctly mapped (issue AC #2)
- [ ] Imported profiles pass validation (`validate_name` + deserialize to `GameProfile`; method resolves) (issue AC #3)
- [ ] Unmappable entries skipped with a warning; batch completes
- [ ] All tasks completed and all validation commands pass
- [ ] No type errors, no clippy warnings, no lint errors
- [ ] Matches UX design (Community-route button → preview modal → result)
- [ ] `Closes #2` referenced in the PR

## Completion Checklist

- [ ] Code follows discovered patterns (From/TryFrom mapper, preview/result split, per-entry outcome)
- [ ] Error handling matches codebase style (hand-rolled Serde enum, `From<ProfileStoreError>`)
- [ ] Logging follows conventions (`tracing::warn!` with structured fields, fail-soft metadata)
- [ ] Tests follow test patterns (tempdir + fixtures)
- [ ] No hardcoded paths (XDG/BaseDirs + tilde helpers)
- [ ] Business logic in `crosshook-core`; `src-tauri` handlers thin; DTOs Serde
- [ ] No unnecessary scope additions (see NOT Building)
- [ ] Self-contained — no questions needed during implementation
- [ ] New scroll containers (if any) registered in `useScrollEnhance` SCROLLABLE selector

## Risks

| Risk                                                           | Likelihood | Impact | Mitigation                                                                                                                        |
| -------------------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------------- |
| `serde_yaml` is archived; choosing the wrong fork              | Medium     | Medium | Use maintained `serde_yaml_ng`; justify in PR; fork is drop-in API-compatible                                                     |
| Lutris path drift (config vs data dir, Flatpak, balanced dirs) | High       | Medium | Check all known roots in `paths.rs`; glob `games/*.yml` AND `games/*/*.yml`; file-picker fallback                                 |
| Bare-Wine runners have no CrossHook equivalent method          | Medium     | Medium | Map all in-scope wine/proton games to `proton_run`; warn on unresolved runner; filter via `pga.db.runner`                         |
| Missing/uninstalled Proton build referenced by `wine.version`  | High       | Low    | Resolve against `runners/wine/<version>/`; if missing, leave `proton_path` empty (creation defaults fill) + warning               |
| esync/fsync inverse-toggle mapping error                       | Medium     | Low    | Explicit tests: `false` ⇒ add `disable_*`, `true`/absent ⇒ no option                                                              |
| pga.db schema differs across Lutris versions                   | Low        | Low    | Select only the stable columns (`name, slug, runner, directory, configpath`); read-only; tolerate missing rows via YAML-only path |
| Concurrent intra-batch dependency (5.2 needs 5.1 symbols)      | Low        | Low    | Plan notes 5.2 references 5.1; executor orders within B5 or runs sequentially                                                     |

## Notes

- **Storage boundary (per CLAUDE.md)**: Imported profiles → **TOML files** in the profiles dir (same as any profile, auto-synced to SQLite via `observe_profile_write`). Import provenance → **SQLite metadata** (`profiles.source = "lutris_import"`, free-form TEXT — **no schema migration**, schema stays v24). Lutris YAML/pga.db source, path mapping, and validation warnings → **runtime-only** (read during import; not persisted by CrossHook).
- **Persistence & usability**: Migration — none (standard TOML profiles + existing `source` column). Offline — fully offline (reads local files only). Degraded fallback — Lutris not found ⇒ folder picker; unmappable entry ⇒ skip with warning, continue; missing Proton build ⇒ default Proton + warning. User visibility — imported profiles appear immediately in the normal list and are fully editable; provenance lives in metadata, not prominently surfaced.
- **Dependency justification**: `serde_yaml_ng` — no existing YAML parser in the workspace; `serde_json`/`toml`/`quick-xml`/`csv` cannot read Lutris YAML. Maintained fork of the archived `serde_yaml`.
- **Module placement**: New module under `crosshook-core/src/profile/lutris_import/` to keep parse/map/paths/orchestration as separate single-responsibility files (each well under the ~500-line soft cap) and reusable for future importer sources (issue's "Code maintainability" note).
- **Forgejo**: Open the PR on `origin` (Forgejo), Conventional-Commits title (e.g. `feat(profiles): import Lutris game configs as profiles`), `Closes #2`, labels `type:feature` + `area:profiles` + `platform:linux` + `priority:low`. The issue already contains the required Storage boundary + Persistence & usability subsections.

---

## Self-Containment Confidence

**Score: 9/10** — Greenfield feature with a near-perfect existing analog (community-JSON
import: same preview/result split, same save+sync flow, same picker→modal UX). Every
target field, optimization ID, error idiom, and IPC pattern is captured with `file:line`
citations. The one residual unknown is the exact `pga.db` `games` DDL across Lutris
versions (mitigated by selecting only stable columns and tolerating YAML-only fallback).
