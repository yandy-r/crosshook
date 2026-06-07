# Implementation Report: GitHub Issue 233 - umu GAMEID HTTP Resolver

## Summary

Implemented the optional umu GAMEID HTTP resolver and SQLite cache for GitHub
issue #233. Profiles can now store optional `runtime.umu_store` and
`runtime.umu_codename` hints, settings expose a disabled-by-default
`umu_database_lookup` opt-in, and launch/preview enrichment resolves GAMEID from
explicit override, Steam app id, fresh cache, fresh lookup, stale cache fallback,
cached miss, or `umu-0` fallback without writing derived values back to profile
TOML.

The implementation stayed in the current checkout per `--no-worktree`.

## Completed Work

- Added `UmuDatabaseLookupPreference` to TOML settings and Tauri/frontend
  settings DTOs, defaulting to disabled for backward compatibility.
- Added profile runtime hints for `umu_store` and `umu_codename` across Rust
  models, frontend profile types, empty-profile defaults, and normalization.
- Added schema v24 with `umu_gameid_lookup_cache` and metadata helpers for
  storing fresh hits, cached misses, stale fallbacks, timestamps, and clear-cache
  behavior.
- Added a dedicated `umu_database` API client and resolver that performs
  cache-first lookup, opt-in gating, explicit/Steam precedence, `umu-0` fallback,
  seven-day TTL, stale fallback, and per-key in-process dedupe.
- Enriched Tauri launch and preview command flows before script/preview
  construction so game and trainer launches share the same resolved GAMEID state.
- Updated `proton_run` and trainer environment construction to use the resolved
  GAMEID and optional `STORE` value while preserving existing fallback behavior.
- Extended Launch Preview DTOs and UI with GAMEID resolution source, key, and
  expiry details without changing the copied command text.
- Added the clear-cache IPC command and wired the action into Settings Advanced,
  keeping the lookup preference itself in the Runner settings section.
- Updated browser mocks for preview resolution and clear-cache command coverage.
- Updated `AGENTS.md` and `CLAUDE.md` schema references from v23 to v24.

## Deviations

- No new runtime dependency was added. In-flight dedupe uses existing Tokio
  primitives and a process-local per-key lock map.
- The test suite covers settings defaults/serde, profile runtime fields, schema
  migration/cache operations, script-runner GAMEID propagation, preview DTOs, and
  frontend type/default behavior. A standalone mock HTTP integration test for
  the resolver was not added in this pass.

## Validation

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --no-fail-fast`: PASS
- `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native`: PASS
- `npm run typecheck` from `src/crosshook-native`: PASS
- `npm test` from `src/crosshook-native`: PASS
- `./scripts/check-host-gateway.sh`: PASS
- `cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all`: PASS
- `npx biome format --write src/components/library/launch/HeroLaunchCommandSection.tsx`: PASS
- `./scripts/lint.sh`: PASS

`./scripts/lint.sh` still reports existing Biome warnings in unrelated frontend
files, and one non-fatal semantic suggestion for the touched preview metadata
group. The script exits 0.

## Files Changed

Primary backend areas:

- `src/crosshook-native/crates/crosshook-core/src/settings/*`
- `src/crosshook-native/crates/crosshook-core/src/profile/models/runtime.rs`
- `src/crosshook-native/crates/crosshook-core/src/launch/request/*`
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/*`
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/*`
- `src/crosshook-native/crates/crosshook-core/src/metadata/*`
- `src/crosshook-native/crates/crosshook-core/src/umu_database/*`
- `src/crosshook-native/src-tauri/src/commands/*`

Primary frontend areas:

- `src/crosshook-native/src/types/*`
- `src/crosshook-native/src/hooks/useUmuDatabaseRefresh.ts`
- `src/crosshook-native/src/hooks/profile/*`
- `src/crosshook-native/src/components/settings/AdvancedSettingsSection.tsx`
- `src/crosshook-native/src/components/settings/RunnerSection.tsx`
- `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`
- `src/crosshook-native/src/components/library/launch/HeroLaunchCommandSection.tsx`
- `src/crosshook-native/src/lib/mocks/handlers/*`
- `src/crosshook-native/src/utils/launchPreviewPresentation.ts`

Reference docs:

- `AGENTS.md`
- `CLAUDE.md`

## Follow-Up Risk

The resolver is intentionally non-fatal and opt-in, so the highest remaining
risk is behavioral coverage around live HTTP edge cases. The next useful test
hardening would be a local mock-server resolver test covering fresh hit, empty
array miss, stale fallback after network failure, and concurrent miss dedupe.
