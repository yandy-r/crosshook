# PR Review #264 — feat(launch): add umu opt-in for non-Steam launches (Phase 3)

**Reviewed**: 2026-04-14 **Mode**: PR (parallel — 3 reviewers: correctness, security, quality)
**Author**: yandy-r **Branch**: `feat/umu-migration-phase-3` → `main` **Head**:
`7bff73e38230230ee579b2103f9bd26fcc49c97e` **Decision**: REQUEST CHANGES

## Summary

Phase 3 + Phase 3b of the umu-launcher migration land cleanly on the business-logic boundary and all
validation checks pass (lib tests, integration tests, rustfmt, clippy `-D warnings`, biome, tsc,
shellcheck). The review surfaces one browser-dev-only correctness bug (mock IPC key mismatch), one
missing regression test for the `force_no_umu=true` Steam-trainer invariant that `CLAUDE.md`
explicitly requires, and four security-hardening gaps in the new `umu_database` HTTP cache
(unbounded response body, production-live env-var URL override, predictable `.tmp` path,
shell-script key validation asymmetry) that should be resolved before merge.

## Findings

### CRITICAL

_(none)_

### HIGH

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:219` —
  `umu_preference: UmuPreference` in `AppSettingsData` carries no field-level `#[serde(default)]`.
  Backward compatibility for legacy `settings.toml` files works today only because the struct-level
  `#[serde(default)]` (line 172) delegates to `Default::default()`, which in turn depends on
  `UmuPreference` deriving `#[default] Auto`. This implicit three-step chain silently breaks if a
  future contributor removes the struct-level annotation or changes `AppSettingsData::default()`.
  Every other recently-added field with a non-trivial default (e.g. `log_filter`,
  `recent_files_limit`, `protonup_auto_suggest`) uses an explicit field-level
  `#[serde(default = "…")]`. [quality]
  - **Status**: Fixed
  - **Category**: Type Safety
  - **Suggested fix**: Add `#[serde(default)]` at the field level on `umu_preference` so the default
    is explicit and resilient to struct-level annotation removal, matching the convention used for
    other fields in the struct.

- **[F002]** `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs:89-97` —
  `CROSSHOOK_TEST_UMU_DATABASE_URL` env-var override is checked at every call to `source_url()` and
  is **not** gated behind `#[cfg(test)]`. In production binaries, any process with write access to
  the app's environment can redirect the CSV download to an attacker-controlled server. The inner
  `#[cfg(test)]` block at lines 98–107 confirms the author intended this as test-only scaffolding;
  the outer unconditional `std::env::var` contradicts that intent. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Wrap the `CROSSHOOK_TEST_UMU_DATABASE_URL` block in `#[cfg(test)]` alongside
    the existing `TEST_SOURCE_URL` `OnceLock` block, and consolidate the two test-override
    mechanisms (see F008).

- **[F003]** `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs:255` —
  `response.bytes().await` buffers the entire HTTP body into memory with no cap. The default source
  is GitHub Raw (stable), but a CDN hiccup, a redirect to a pathological server, or a mirror
  configured via the production-live env override (see F002) could serve an arbitrarily large body
  and OOM the process. The umu-database CSV is ~500 KiB today; a 16 MiB cap is more than two orders
  of magnitude of headroom. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Check `response.content_length()` against a constant such as
    `MAX_CSV_BYTES = 16 * 1024 * 1024` before buffering, and re-check the final length after
    `response.bytes().await` returns — error out with `Error::Io(InvalidData)` on either breach.

- **[F004]** `src/crosshook-native/crates/crosshook-core/tests/umu_concurrent_pids.rs` — No
  dedicated test exercises the `build_flatpak_steam_trainer_command` + `force_no_umu=true` invariant
  that `CLAUDE.md` explicitly requires (_"Steam trainer `force_no_umu=true` invariant must hold with
  a dedicated test"_). The existing integration test only covers the generic
  `build_proton_trainer_command` path with `force_no_umu=false`, so a future refactor that drops or
  inverts the `force_no_umu=true` argument on the Steam-trainer branch has no regression guard. The
  PR body claims `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred` exists — verify
  this test is actually present in the committed suite and, if not, add it. [correctness]
  - **Status**: Failed
  - **Category**: Completeness
  - **Suggested fix**: Add a test named
    `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred` (or similar) that stages a
    fake `umu-run` on PATH, constructs a request with `UmuPreference::Umu`, calls
    `build_flatpak_steam_trainer_command`, and asserts the resulting command's program does NOT
    contain `umu-run`.

- **[F005]** `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh:84,89` —
  `restore_preserved_trainer_env` calls `export "${key}=${value}"` for both the builtin and custom
  preserved-env arrays without invoking `is_valid_shell_env_key`. The write path
  (`write_preserved_custom_env_file`, line 117) correctly validates keys before persisting; the
  restore path does not. The builtin key list is Rust-generated and safe in practice, but the custom
  key list is user-controlled via `CROSSHOOK_TRAINER_CUSTOM_ENV_KEYS` — a key containing `=`, a
  newline, or glob characters would produce a malformed `export` statement and silently corrupt the
  child process environment. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Mirror the validation guard from `write_preserved_custom_env_file` into
    `restore_preserved_trainer_env`: skip-and-log any key where `is_valid_shell_env_key` returns
    false before calling `export`.

- **[F006]** `src/crosshook-native/src/lib/mocks/handlers/umu_database.ts:25` — The mock
  destructures `app_id` (snake_case) from the args object, but `RuntimeSection.tsx:73` calls
  `callCommand<UmuCsvCoverage>('check_umu_coverage', { appId: umuAppId })` with camelCase — matching
  Tauri v2's own JS→Rust key transformation and the convention used by every other mock handler in
  the codebase (e.g. `protondb.ts:130` uses `appId`). In browser dev mode the args object never
  crosses the native IPC boundary, so the handler receives `{ appId: "..." }`, destructures
  `undefined` for `app_id`, short-circuits on the empty-string guard, and returns `'unknown'` for
  every input. Consequence: the amber coverage-warning UI path (`umuCoverage === 'missing'`) cannot
  be exercised in dev mode for any app id, including the documented allow-list entries (`546590`,
  `2050650`). [correctness] [quality]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Rename the destructured key to `appId` on line 25 to match the calling
    convention: `const { appId } = (args ?? {}) as { appId?: string };` and use `appId` for the rest
    of the handler.

### MEDIUM

- **[F007]** `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs:19-21` — Three
  magic constants (`CACHE_TTL_HOURS = 24`, `REQUEST_TIMEOUT_SECS = 6`, and the hardcoded
  `"crosshook/umu-database.csv"` sub-path inside `csv_target_path`) lack doc comments explaining the
  rationale (e.g. why 24h vs 6h vs 72h, why 6s vs 30s, why this exact sub-path). The CSV sub-path is
  also duplicated between `client.rs` and `paths.rs` (tier-1 lookup of the HTTP cache). [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Add a one-line doc comment to each constant explaining the rationale. Extract
    the CSV sub-path into a shared
    `pub(crate) const CROSSHOOK_UMU_DATABASE_CSV_SUBPATH: &str = "crosshook/umu-database.csv";`
    imported by both `client.rs` and `paths.rs`.

- **[F008]** `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs:88-109` —
  `source_url()` has two URL-override mechanisms: a process-global `OnceLock<Mutex<String>>`
  (`TEST_SOURCE_URL` + `set_source_url_for_test`, gated on `#[cfg(test)]`) and the un-gated
  `CROSSHOOK_TEST_UMU_DATABASE_URL` env var (see F002). The env-var path always wins when set; the
  `OnceLock` path is never used by any test in this PR. Two mechanisms for the same concern is
  confusing and invites drift. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Delete the unused `OnceLock`/`set_source_url_for_test` scaffolding in favour
    of the env-var override (which must be wrapped in `#[cfg(test)]`, per F002), or pick the
    `OnceLock` approach and delete the env-var code. Do not keep both.

- **[F009]** `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs:271-273` — The
  temp path is fixed and predictable: `~/.local/share/crosshook/umu-database.tmp`. `std::fs::write`
  on Linux follows symlinks, so a local adversary (or a stale run that left a dangling link) at
  `umu-database.tmp` could redirect the CSV write to an arbitrary path. The target dir is
  user-private, so exploitability is narrow, but the gap is trivial to close. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Use `tempfile::NamedTempFile::new_in(parent)?` + `persist(&target_path)` to
    create the temp file with `O_CREAT | O_EXCL` and an unpredictable name. `tempfile` is already a
    dev-dependency — promote it to a regular dep or use its equivalent in the `tempfile-fast` /
    `std::fs::File::options().create_new(true)` pattern.

- **[F010]** `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs:271-273` — After
  the atomic write, the CSV file's permissions are whatever the process umask allows (typically
  `0644`). No sensitive data is at stake, but setting `0600` explicitly prevents a narrow race where
  another same-user process reads a partially-written temp file before rename, and documents intent.
  [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Apply `fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))`
    before the rename, or use the `tempfile` crate fix in F009 which defaults to `0600`.

- **[F011]** `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs:15` —
  `#[allow(dead_code)]` on `CsvRow` suppresses dead-code lints for fields (`title`, `umu_id`,
  `common_acronym`, `note`, `exe_strings`) that are parsed by serde but never read downstream. The
  attribute masks future lints if more fields become unused. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: If the extra fields are retained for forward-compat (future coverage reasons
    or debugging), replace the blanket `#[allow(dead_code)]` with a comment explaining intent;
    otherwise trim `CsvRow` down to only the fields the index keys on and drop the attribute
    entirely.

- **[F012]** `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs:50-55` — Race
  window between reading `fs::metadata(&path).modified()` and acquiring the `COVERAGE_CACHE` mutex.
  A concurrent writer advancing the CSV mtime during the gap would cause the mtime comparison inside
  the lock to see `current == cached` (both reflect the pre-write snapshot), and the stale index
  would be served. Exploitability is near-zero in a single-user desktop app, but the fix is a
  two-line reorder. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Acquire the mutex first, then read the mtime inside the critical section so
    invalidation check and cache update are atomic with respect to other callers.

- **[F013]** `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs:56` —
  `mutex.lock().expect("umu_database cache mutex poisoned")` in the hot `check_coverage` path turns
  any prior panic while holding the lock into a cascading panic in every subsequent Tauri IPC call
  that touches coverage. The tokio worker running `check_umu_coverage` would die and the UI would
  stop receiving updates. [quality]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Use `mutex.lock().unwrap_or_else(|e| e.into_inner())` to recover the guard on
    poison, consistent with `umu_database_http_cache.rs`'s `ENV_LOCK` handling.

- **[F014]** `src/crosshook-native/crates/crosshook-core/src/umu_database/paths.rs:20-26` — Each
  `XDG_DATA_DIRS` colon-split entry is joined with `umu-protonfixes/umu-database.csv` and used
  verbatim — no `canonicalize()`, no `..` stripping, no symlink check. `XDG_DATA_DIRS` is
  user-controlled; an entry like `../../etc` produces a readable path outside the expected scope.
  Impact is limited because tiers 3–5 only feed the advisory coverage check (not execution), but
  attacker-controlled CSV can still manipulate the amber warning and influence user decisions.
  [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Reject any entry whose path components contain
    `std::path::Component::ParentDir`, and log-skip non-absolute entries. Optionally canonicalize
    once per entry to resolve symlinks deterministically.

- **[F015]** `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx:64-83` — The
  `useEffect` that fires `check_umu_coverage` is inlined directly in the component body. `CLAUDE.md`
  asks IPC state hooks to be wrapped in dedicated hooks (the pattern `usePreviewState.ts` already
  follows). Leaving this in the component makes the coverage IPC hard to memoize, hard to unit-test
  in isolation, and awkward to reuse if a second coverage surface is added (e.g. in
  `PinnedProfilesStrip`, which this PR already prepares a `umuCoverageWarnByProfile` prop for).
  [quality]
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Extract a
    `useUmuCoverage(effectivePreference: UmuPreference, appId: string): UmuCsvCoverage` hook in
    `src/crosshook-native/src/hooks/` and delegate `PinnedProfilesStrip` to it so the
    Runner-dropdown and pinned-strip warnings share a single cache/debounce surface.

- **[F016]** `src/crosshook-native/src/types/launch.ts:152` —
  `requested_preference: 'auto' | 'umu' | 'proton'` duplicates the `UmuPreference` literal union
  already exported from `src/types/settings.ts`. If the Rust enum ever grows a variant, TypeScript
  will not flag the divergence at compile time. [correctness]
  - **Status**: Fixed
  - **Category**: Type Safety
  - **Suggested fix**: `import type { UmuPreference } from './settings';` and use
    `requested_preference: UmuPreference;` so the single source of truth lives in one place.

### LOW

- **[F017]** `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs` (module-wide) —
  No backend-side coalescing of concurrent `refresh_umu_database` calls. The startup-spawn path and
  the Settings button share the same tokio runtime; if the startup refresh is in progress when the
  user clicks the Settings button, two concurrent HTTP fetches race on writing the same `.tmp` path
  and then both call `fs::rename`. Last-writer wins for the file; the metadata DB upserts also race.
  The frontend `isRefreshing` flag blocks double-clicks within one UI session but not across entry
  points. [security]
  - **Status**: Open
  - **Category**: Performance
  - **Suggested fix**: Wrap the HTTP-fetch-and-write body in a `tokio::sync::Mutex<()>` stored in a
    `OnceLock`; either serialize or short-circuit the second caller when the mutex is held.

- **[F018]** `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs:42` —
  `pub fn check_coverage` is part of the module's public surface (re-exported from `mod.rs`) but
  carries no doc comment. Sister functions `refresh_umu_database` and `resolve_umu_database_path`
  are documented. [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Add a `///` comment documenting arguments (`app_id` = numeric Steam-store
    string; `store` defaults to `"steam"` when `None`), return semantics
    (`Found`/`Missing`/`Unknown`), and that the call is synchronous and file-cached.

- **[F019]** `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs:55-77` — The
  `COVERAGE_CACHE` `Mutex` is held for the entire duration of `load_index` (full-file CSV read +
  parse) on cold start or mtime-change. At current CSV size (~500 KiB, ~13k rows) this is fast, but
  a future CSV growth or a slow filesystem would stall every coverage check behind the parse.
  [security]
  - **Status**: Open
  - **Category**: Performance
  - **Suggested fix**: Read + parse the index without holding the mutex, then re-acquire to compare
    mtime and publish the new index (double-check). Accept the rare case where two cold-start
    callers parse in parallel.

- **[F020]** `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs:91-106` —
  `flexible(true)` on the CSV reader silently tolerates malformed rows. Combined with F003 (no
  download size limit), a corrupted or adversarial CSV could be parsed in full with a quietly
  incomplete index. [security]
  - **Status**: Open
  - **Category**: Performance
  - **Suggested fix**: Either error out on malformed rows (drop `flexible(true)`) or increment a
    `rows_skipped` counter and log a `tracing::warn!` when any rows are skipped, so silent
    degradation is observable.

- **[F021]** `src/crosshook-native/crates/crosshook-core/src/umu_database/mod.rs:21` — The doc
  comment for `CsvCoverage::Missing` references `umu/umu_run.py:515 verified 2026-04-14`. The line
  number is fragile — a one-line commit upstream invalidates it. [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Replace the file:line reference with the upstream commit SHA at the time of
    verification, or link to the specific function name (stable identifier).

- **[F022]** `src/crosshook-native/crates/crosshook-core/tests/umu_database_coverage.rs:163` — The
  mtime-invalidation test sleeps `Duration::from_secs(1)` to guarantee a distinct mtime. On
  filesystems with sub-second mtime precision (some tmpfs configs, future ext4 with nanosecond-mtime
  on) this is fine, but the unconditional 1s sleep is a 1s tax on every CI run. [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Use the `filetime` crate (already transitive) to set the new mtime explicitly
    via
    `filetime::set_file_mtime(&path, FileTime::from_system_time(SystemTime::now() + Duration::from_secs(2)))`
    and drop the sleep.

- **[F023]** `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx:72` — Inline
  comment reads _"Tauri auto-converts snake_case Rust params to camelCase on the JS boundary."_ The
  direction is inverted — Tauri v2 converts **camelCase JS argument keys → snake_case Rust parameter
  names**. This inverted comment is what misled the mock-handler author (F006). [quality]
  - **Status**: Open
  - **Category**: Maintainability
  - **Suggested fix**: Rewrite as _"Tauri converts camelCase JS argument keys to snake_case Rust
    parameter names at the IPC boundary."_

- **[F024]** `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx:272-279` — The
  amber `⚠` glyph is rendered inline. Emoji-as-UI is acceptable for this project, but the
  `role="img"` + `aria-label` pattern should be verified in one place (accessibility consistency
  with `PinnedProfilesStrip` which uses the same glyph). [quality]
  - **Status**: Open
  - **Category**: Pattern Compliance
  - **Suggested fix**: Centralise the warning badge in a small shared component
    (`<CoverageWarnBadge />`) so `RuntimeSection` and `PinnedProfilesStrip` render identical
    markup + a11y attributes.

- **[F025]** `src/crosshook-native/src/types/launch.ts:185` —
  `umu_decision?: UmuDecisionPreview | null;` uses both `?` (key-may-be-absent) and `| null`
  (value-may-be-null). Rust `Option<T>` under serde's default serialization always emits the key
  with value `null` for `None`, so the key is never actually absent on the wire. The double-nullable
  encourages sloppy `=== undefined` checks that silently pass when the value is `null`.
  [correctness]
  - **Status**: Open
  - **Category**: Type Safety
  - **Suggested fix**: Pick one encoding: either `umu_decision: UmuDecisionPreview | null;`
    (preferred — matches the wire format) or `#[serde(skip_serializing_if = "Option::is_none")]` on
    the Rust side plus `umu_decision?: UmuDecisionPreview` in TS.

## Validation Results

| Check      | Result                                                                                                                    |
| ---------- | ------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (tsc clean, part of `scripts/lint.sh`)                                                                               |
| Lint       | Pass (rustfmt clean, `cargo clippy -D warnings` clean, biome clean on 230 files, shellcheck clean)                        |
| Tests      | Pass (crosshook-core lib tests + `umu_database_coverage` 4/4 + `umu_database_http_cache` 4/4 + `umu_concurrent_pids` 1/1) |
| Build      | Pass (implicit via `cargo clippy` and `cargo test` artifacts)                                                             |

## Files Reviewed

- `docs/prps/plans/umu-migration-phase-3-umu-opt-in.plan.md` (Modified)
- `docs/prps/plans/umu-migration-phase-3b-umu-opt-in.plan.md` (Added)
- `docs/prps/prds/umu-launcher-migration.prd.md` (Modified)
- `docs/prps/reports/umu-migration-phase-3-umu-opt-in-report.md` (Added)
- `docs/prps/reports/umu-migration-phase-3b-umu-opt-in-report.md` (Added)
- `src/crosshook-native/Cargo.lock` (Modified)
- `src/crosshook-native/crates/crosshook-cli/src/main.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/Cargo.toml` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/install/models.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/install/service.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/env.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/lib.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/run_executable/service.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/umu_database/mod.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/umu_database/paths.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/src/update/service.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/tests/umu_concurrent_pids.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/tests/umu_database_coverage.rs` (Added)
- `src/crosshook-native/crates/crosshook-core/tests/umu_database_http_cache.rs` (Added)
- `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh` (Modified)
- `src/crosshook-native/src-tauri/src/commands/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/settings.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/umu_database.rs` (Added)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/components/LaunchPanel.tsx` (Modified)
- `src/crosshook-native/src/components/PinnedProfilesStrip.tsx` (Modified)
- `src/crosshook-native/src/components/SettingsPanel.tsx` (Modified)
- `src/crosshook-native/src/components/pages/LaunchPage.tsx` (Modified)
- `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx` (Modified)
- `src/crosshook-native/src/context/LaunchStateContext.tsx` (Modified)
- `src/crosshook-native/src/hooks/usePreviewState.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/umu_database.ts` (Added)
- `src/crosshook-native/src/lib/mocks/index.ts` (Modified)
- `src/crosshook-native/src/styles/preview.css` (Modified)
- `src/crosshook-native/src/styles/theme.css` (Modified)
- `src/crosshook-native/src/types/launch.ts` (Modified)
- `src/crosshook-native/src/types/profile.ts` (Modified)
- `src/crosshook-native/src/types/settings.ts` (Modified)
- `src/crosshook-native/src/utils/launch.ts` (Modified)
