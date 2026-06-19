# PR Review #20 — feat(profiles): import Lutris game configs as profiles

**Reviewed**: 2026-06-18
**Mode**: PR (Forgejo, `--parallel --no-worktree`)
**Author**: yandy
**Branch**: feat/lutris-profile-import → main
**Decision**: REQUEST CHANGES

## Summary

Solid, well-structured one-way Lutris → CrossHook importer with good test coverage and clean compilation/lint/typecheck. Review (3 parallel reviewers: correctness, security, quality) found 4 HIGH issues that should be fixed before merge: two correctness bugs in pga.db handling that degrade real-world imports, one path-traversal in runner-path resolution, and a config-history gap where Lutris-imported profiles silently get no config revision (diverging from every other import path).

## Findings

### CRITICAL

- None

### HIGH

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/import.rs:59-64` — Runner-type filter is bypassed whenever a YAML config has no matching `pga.db` row (`pga` is `None`). Non-Wine/Proton Lutris games (dosbox, scummvm, mame, …) that lack a pga entry pass through as importable, and the resulting `runner` field shows the `"wine"` fallback (`map.rs:42`). This is compounded by F002: if the pga read fails entirely, `pga` is `None` for _every_ entry and the filter becomes a no-op across the board. [Correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: When `pga` is `None`, gate importability on the YAML itself — only pass through configs that contain a `wine:` section (Wine/Proton-specific), or read a top-level `runner` key from the YAML and apply `is_wine_or_proton_runner` to it.

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/paths.rs:84-103` — `read_pga_games` deserializes `directory` and `configpath` into non-`Option` `String` fields via `row.get(3)?` / `row.get(4)?`. In real Lutris installs these columns are nullable (games without a configured directory or YAML config). A single NULL row makes `query_map`/row iteration return an error, which propagates out of `read_pga_games`; `preview_lutris_import` (import.rs:34-40) then swallows it into diagnostics and falls back to an **empty** pga map — losing the runner filter and `pga.directory` prefix fallback for _all_ games. [Correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Make `PgaGame.directory` and `PgaGame.configpath` `Option<String>` and read them with `row.get::<_, Option<String>>()`; handle/skip per-row errors inside the loop rather than failing the whole query, and adjust the configpath HashMap key and prefix-fallback to handle `None`.

- **[F003]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/runner.rs:24` — Path traversal via the untrusted YAML `wine.version` string. `lutris_root.join("runners").join("wine").join(version)` does not sanitize `..`/separators, so a crafted version (e.g. `"../../../../etc"`) probes arbitrary paths; on a hit, the traversed absolute path is serialized into `runtime.proton_path` and persisted in the saved profile TOML. No code execution at import time (launch-time validation rejects non-executables), but it persists an attacker-influenced path and leaks filesystem layout via Installed/Missing classification. [Security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Reject versions containing `/`, `\`, or `..` before constructing the path (return `RunnerResolution::Missing`), or `canonicalize` the result and confirm it stays under `lutris_root/runners/wine`.

- **[F004]** `src/crosshook-native/src-tauri/src/commands/lutris.rs:34-59` — `lutris_import_profiles` calls `observe_profile_write` per imported profile but never calls `capture_config_revision`. Every other profile-write path (`profile_save`, `profile_import_legacy` at `lifecycle.rs:358`, `profile_duplicate`, command-arguments save) captures a config revision after the metadata write. Lutris-imported profiles will silently have no config-history entry, breaking diff/rollback for any profile first created via this importer. `ConfigRevisionSource::Import` already exists. [Completeness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: After each successful `observe_profile_write`, call `capture_config_revision(profile_name, &entry_result.entry.mapped, ConfigRevisionSource::Import, None, &metadata_store, max_revisions)`, mirroring `profile_import_legacy`. Add `settings_store: State<'_, SettingsStore>` to the command and resolve `max_revisions` via `resolve_config_history_max_revisions`.

### MEDIUM

- **[F005]** `src/crosshook-native/crates/crosshook-core/Cargo.toml:21` — `serde_yaml_ng = "0.10"` added without a justification comment. It pulls in `unsafe-libyaml` (a community fork's transitive unsafe dependency); the choice of this fork over alternatives is undocumented. [Maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Add a comment documenting why this maintained fork of the abandoned `serde_yaml` was chosen for Lutris YAML parsing.

- **[F006]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/import.rs:102-106` — `apply_lutris_import` takes `profiles_dir: &Path` and internally rebuilds a `ProfileStore` via `with_base_path`; the Tauri command passes `&profile_store.base_path`, bypassing the injected `ProfileStore` state and diverging from every other write path. [Pattern Compliance]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Change the signature to `apply_lutris_import(store: &ProfileStore, entries: Vec<LutrisImportEntry>)` and pass the injected store directly.

- **[F007]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/import.rs:199` — Magic number `1000` as the unique-name attempt ceiling in `derive_unique_profile_name`. [Maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Extract `const MAX_UNIQUE_NAME_ATTEMPTS: u32 = 1000;` and reference it.

- **[F008]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/map.rs:73-86` — Dead/confusing double resolution: the first `resolve_runner_path` call's `Installed` arm discards the path (`let _ = path;` / "proton*path set below") and exists only for the Missing-warning side effect, then `resolve_runner_path` is called again at lines 88-96 to actually compute `proton_path`. Confusing and fragile if the two calls diverge. *(Concurred by correctness + quality reviewers.)\_ [Dead Code]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Remove the first call; derive the not-installed warning from the result of the single second call.

- **[F009]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/map.rs:115` — `"proton_run"` string literal used instead of the existing `METHOD_PROTON_RUN` constant (`launch/request/models.rs`), which every other production site uses. [Pattern Compliance]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: `use crate::launch::request::METHOD_PROTON_RUN;` and replace the literal with `METHOD_PROTON_RUN.to_string()`.

- **[F010]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/map.rs:263-325` — `map_gamescope_config` (resolution parsing, FSR sharpness, FPS limiter, window mode, extra-args split) has no unit tests despite parsing external YAML with implicit format assumptions. [Completeness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add tests for `parse_resolution` (`"1920x1080"`, empty, `"1920"`, `"axb"`) and for `map_gamescope_config` with gamescope enabled.

- **[F011]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/map.rs:334-336` — `shell_split_args` splits on whitespace only, so quoted Lutris `args`/`gamescope_flags` containing spaces (e.g. `--config "/path with spaces/foo.cfg"`) are silently broken into wrong tokens. The name implies shell semantics it does not provide. _(Concurred by all three reviewers.)_ [Correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Use a real tokenizer (`shell-words` crate), or rename to `split_whitespace_args`, document the limitation, and warn when the raw string contains quote characters.

- **[F012]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/parse.rs:57` — `fs::read_to_string` feeds `serde_yaml_ng` with no file-size cap. The directory picker makes a crafted/oversized YAML (or large `system.env` map) reachable with one user action → unbounded allocation (local DoS within the desktop trust model). [Performance/Security]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Gate on `fs::metadata(path).len()` against a constant (e.g. 512 KiB) before deserializing.

- **[F013]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/paths.rs:79` — `read_pga_games` query has no `LIMIT`, and `list_game_configs` has no file-count cap; both run synchronously on the Tauri IPC thread. Unbounded for a pathological library. _(Concurred by security + quality reviewers.)_ [Performance]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Add a `LIMIT` constant to the SQL and cap `list_game_configs` at the same value; log when the cap is hit.

- **[F014]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/tests/paths_tests.rs:37-40` — `set_home_env` mutates process-global env (`HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`) without restoring it. The `ENV_LOCK` mutex only serializes these three tests, not other tests in the binary that read those vars; parallel scheduling could leak the poisoned env. [Completeness]
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Restore originals via a RAII guard, or refactor `discover_lutris_root` to accept an injectable `BaseDirs`.

- **[F015]** `src/crosshook-native/src/hooks/useLutrisImport.ts:49-51,97` — `error` state and `clearLutrisError` are exported from the hook but the only caller (`CommunityBrowser`) consumes neither (it handles the prepare-phase error via its own try/catch and destructures only `importError`/`importResult`). Dead public API surface. [Maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Remove `error`/`clearLutrisError` from the hook return, or document the intended consumer.

### LOW

- **[F016]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/import.rs:171` — `sanitize_profile_name` uses `ch.to_ascii_lowercase()` on chars that passed `is_alphanumeric()`; non-ASCII alphanumerics (e.g. `Ü`, `é`) keep their case, producing inconsistent slugs for international titles. [Correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Use `ch.to_lowercase()` (iterator), or replace non-ASCII alphanumerics with `-` for strict ASCII slugs.

- **[F017]** `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/import.rs:199-206` — After 999 collisions `derive_unique_profile_name` falls back to `{base}-copy` without checking it is free; `store.save` would then overwrite an existing `{base}-copy`. Degenerate but undocumented. [Correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Verify `{base}-copy` is not in `taken` before returning, or document the overflow behavior.

- **[F018]** `src/crosshook-native/src/components/LutrisImportModal.tsx:136,344` — Inline `style={{ gridRow: 3 }}` / `{{ gridRow: 4 }}` are undocumented magic numbers (copied from `MigrationReviewModal`). [Maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Add a comment or CSS class encoding the modal grid-row slots.

## Validation Results

| Check      | Result                                                                      |
| ---------- | --------------------------------------------------------------------------- |
| Type check | Pass (`tsc --noEmit` + test config)                                         |
| Lint       | Pass (`cargo clippy -p crosshook-core -- -D warnings`; biome on changed TS) |
| Tests      | Pass (`cargo test -p crosshook-core`)                                       |
| Build      | Skipped (full release/Flatpak build not run; clippy covers compilation)     |

## Files Reviewed

- `docs/prps/plans/completed/lutris-profile-import.plan.md` (Added)
- `docs/prps/reports/lutris-profile-import-report.md` (Added)
- `src/crosshook-native/Cargo.lock` (Modified)
- `src/crosshook-native/crates/crosshook-core/Cargo.toml` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/error.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/import.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/map.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/mod.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/parse.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/paths.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/runner.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/tests/*.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/types.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/export.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/lutris.rs` (Added)
- `src/crosshook-native/src-tauri/src/commands/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/profile/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/profile/shared.rs` (Modified)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/components/CommunityBrowser.tsx` (Modified)
- `src/crosshook-native/src/components/LutrisImportModal.tsx` (Added)
- `src/crosshook-native/src/components/community/CommunityProfilesSection.tsx` (Modified)
- `src/crosshook-native/src/hooks/useLutrisImport.ts` (Added)
- `src/crosshook-native/src/lib/mocks/handlers/lutris.ts` (Added)
- `src/crosshook-native/src/lib/mocks/index.ts` (Modified)
