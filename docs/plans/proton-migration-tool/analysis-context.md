# Context Analysis: proton-migration-tool

## Executive Summary

The Proton migration tool detects stale Proton paths in CrossHook profiles and suggests same-family replacements using a family-based fuzzy matching algorithm. It builds on existing `discover_compat_tools()` and `normalize_alias()` infrastructure in `steam/proton.rs`, surfaces actions in the existing Health Dashboard, and requires one new Rust module, one Tauri command file, one hook, and one React hook — zero new crate dependencies.

## Architecture Context

- **System Structure**: New `profile/migration.rs` (core logic) consumes `steam/proton.rs` for discovery/matching and `profile/toml_store.rs` for profile CRUD. `commands/migration.rs` exposes two Tauri IPC commands. Health Dashboard gets inline "Suggest Fix" per stale-Proton row.
- **Data Flow**: `discover_compat_tools()` → `extract_proton_family()` + integer-tuple ranking → `MigrationPlan` (dry-run) → user confirms → atomic write via `.toml.tmp` + `fs::rename()` → `observe_profile_write(SyncSource::AppMigration)` → health snapshot invalidated → frontend triggers `revalidateSingle()`.
- **Integration Points**: (1) `steam/proton.rs` — promote two private fns to `pub(crate)`; (2) `metadata/models.rs` — add `AppMigration` to `SyncSource` enum + `as_str()` arm (exhaustive match); (3) `HealthDashboardPage.tsx` — extend `TableToolbar` and issue rows inline; (4) Tauri `invoke_handler` in `lib.rs` — register migration commands; no new `State<>` types needed (both `ProfileStore` and `MetadataStore` already managed).

## Critical Files Reference

- `steam/proton.rs`: `discover_compat_tools()`, `normalize_alias()` (must be promoted to `pub(crate)`), `resolve_compat_tool_by_name()` (must be promoted) — the entire suggestion engine depends on these
- `profile/models.rs`: `effective_profile()` / `storage_profile()` roundtrip (lines 243–298) — migration operates on effective form; `save()` handles local_override routing automatically
- `profile/toml_store.rs`: `ProfileStore::load()` / `save()` — migration must NOT call `save()` directly; must use write-to-tmp + `fs::rename()` in migration write path (non-atomic write risk W-1)
- `profile/health.rs`: `batch_check_health()` — pattern to follow for batch iteration; also seeds the "which profiles need migration" list via `HealthIssue.field == "steam.proton_path"`
- `metadata/models.rs`: `SyncSource` enum — add `AppMigration` variant here; `as_str()` uses exhaustive match so BOTH the variant and its `as_str()` arm must be added together
- `metadata/profile_sync.rs`: `observe_profile_write()` — call after every migration write with `SyncSource::AppMigration`
- `metadata/health_store.rs`: Health snapshots — call `upsert_health_snapshot()` to invalidate cached badges after migration writes
- `commands/profile.rs`: Standard Tauri command pattern with `State<'_, ProfileStore>` + `State<'_, MetadataStore>` injection; `observe_profile_write()` call at line ~110
- `commands/shared.rs`: `sanitize_display_path()` — mandatory for all path strings in IPC results (A-4)
- `components/pages/HealthDashboardPage.tsx`: `categorizeIssue()` (lines 39–48) maps `field === 'steam.proton_path'` → `'missing_proton'`; `TableToolbar` is file-local (not importable — modify in place)
- `components/LauncherPreviewModal.tsx`: Full modal shell (portal, focus trap, ARIA, Tab cycling, Escape, `inert` management) — copy shell for migration review modal body

## Patterns to Follow

- **Tauri Command Pattern**: `#[tauri::command]` with `State<'_, ProfileStore>` + `State<'_, MetadataStore>`; return `Result<T, String>` with stringified errors. See `commands/profile.rs`.
- **Profile Load/Save Roundtrip**: `load()` returns effective profile (local_override merged in); `save()` calls `storage_profile()` to re-split. Migration only needs to update the effective path — no manual local_override handling. Validated by `storage_profile_roundtrip_is_idempotent` test.
- **Batch Health Pattern**: Best-effort iteration — collect per-profile results, never abort on individual failure. See `batch_check_health()` in `profile/health.rs`.
- **Metadata Sync**: Every profile mutation calls `observe_profile_write()` as fail-soft (logged, not fatal). See `commands/profile.rs:~110`.
- **Modal Shell Pattern**: Copy `LauncherPreviewModal.tsx:51–303` for any new modal — NOT `ProfileReviewModal` (wrong layout for migration table).
- **Focus/Controller Classes**: All interactive elements need `crosshook-focus-ring`, `crosshook-nav-target`, `crosshook-focus-target` CSS classes. Minimum touch target: `var(--crosshook-touch-target-min)`.
- **Atomic Writes (migration-specific)**: Write to `.toml.tmp` then `fs::rename()` — do NOT reuse `ProfileStore::save()` for migration writes due to truncate-then-write risk during batch.
- **Staleness Detection**: Use `path.try_exists()` not `exists()` — only flag as stale on `Ok(false)`. Do NOT canonicalize stored symlinked paths before checking.
- **Version Comparison**: Parse dash/dot-separated segments as integer tuples (`[9, 10]` > `[9, 9]`) — never lexicographic. Handles GE-Proton naming correctly.

## Cross-Cutting Concerns

- **Security W-1 (Prerequisite)**: Migration write path MUST use `.toml.tmp` + `fs::rename()` — applies to all write logic in `profile/migration.rs` and `commands/migration.rs`.
- **Security W-3 (Architectural)**: Scan and apply are ALWAYS separate IPC calls; preview returns `MigrationPlan` with no writes; no auto-migration from startup path.
- **Security W-4 (Batch)**: Pre-flight validation pass required before any batch writes — serialize all profiles + verify all replacement paths; abort if any fail.
- **Local Override Correctness**: Confirmed safe — standard `load()` → modify → `save()` works for portable profiles. But a migration-specific round-trip test is required for confidence.
- **Health Snapshot Invalidation**: After every successful write, call `upsert_health_snapshot()` for migrated profile IDs — otherwise Health Dashboard shows stale badges for fixed profiles.
- **`sanitize_display_path()` Mandatory**: All `old_path` / `new_path` strings in migration IPC responses must pass through `sanitize_display_path()` from `commands/shared.rs`.
- **TKG / Hash-Versioned Builds**: Detect via `normalized_aliases.iter().any(|a| a.starts_with("protontkg"))` — exclude from numeric ranking but include as unranked manual options.
- **TOCTOU Mitigation**: Re-check replacement path existence immediately before each write (not just at scan time).
- **Testing patterns**: Use `tempfile::tempdir()` + `ProfileStore::with_base_path(tmp)`; do NOT mock `ProfileStore` or `ProtonInstall` — codebase deliberately avoids mocking filesystem.

## Parallelization Opportunities

- **Group 1.0 (Prerequisite visibility changes)** — `normalize_alias` / `resolve_compat_tool_by_name` promotion + `AppMigration` variant — trivial, ~10 lines total; can be done by any implementor before other work starts.
- **Group 1.1 (Backend suggestion engine)** — `profile/migration.rs` with `extract_proton_family()`, version ranking, `MigrationPlan` structs — can be written and tested in full isolation; no Tauri or frontend dependency.
- **TypeScript type definitions** — `src/types/migration.ts` and `src/hooks/useProtonMigration.ts` stubs can be drafted in parallel with Group 1.1 backend work using the spec's type contracts.
- **Group 1.2 (Tauri commands)** — strictly depends on 1.0 + 1.1; cannot parallelize with suggestion engine.
- **Group 1.3 (Frontend UX)** — depends on 1.2 for actual invoke calls, but component skeleton and modal shell can be built against mock data.
- **Phase 2 batch work** — entirely sequential after Phase 1 is validated.

## Implementation Constraints

- **Zero new crate dependencies** — all logic uses existing `serde`, `toml`, `rusqlite`, stdlib `std::fs`. `tempfile` is dev-dep only; use stdlib rename pattern instead.
- **Module placement is `profile/migration.rs`** (per team consensus) — not `steam/migration.rs`; discovery stays read-only in `steam/`.
- **Cross-family suggestions excluded from batch defaults** — cross-family requires per-profile opt-in; never included in batch "Fix All" pre-selection.
- **No new page or sidebar entry** — feature integrates into Health Dashboard only; sidebar already has 7 items.
- **`TableToolbar` is file-local** — cannot import; extend `HealthDashboardPage.tsx` in place.
- **`ProfileReviewModal` is the wrong base** — it has summary-item layout, not checkbox table; use `LauncherPreviewModal` shell.
- **`effective_profile()` / `storage_profile()` roundtrip is safe** — confirmed by existing test `storage_profile_roundtrip_is_idempotent`; migration does NOT need to directly manipulate `local_override` fields.
- **`ProtonInstall.path` is the executable** — it ends in `.../proton` (the binary), not the parent directory. When comparing profile stored paths to discovered tools, the stored path is also the executable path; comparison is direct string/path equality after normalization.
- **Proton Experimental is not version-comparable** — only suggest another Experimental install; no digit comparison.
- **`steam_applaunch` vs `proton_run` field targeting** — use `resolve_launch_method()` to select correct field; do not migrate both fields for the same profile unless both are independently stale.
- **Command naming — use `preview_proton_migration`** — feature-spec uses `check_proton_migrations` but `preview_proton_migration` has clearer read-only semantics and aligns with the dry-run/confirm split. Resolve before implementation starts; all IPC call sites must use the same name.
- **`HealthDashboardPage.tsx` is modified in two separate tasks** — Phase 1 adds per-row inline fix button; Phase 2 adds toolbar "Fix All" button. Changes are additive and non-conflicting, but should be assigned to the same implementor or sequenced to avoid merge conflicts.
- **Phase 1 must ship before Phase 2** — batch commands have hard dependency on validated single-profile algorithm.

## Key Recommendations

- **Start with Group 1.0 prerequisites** (visibility changes + enum variant) — unblocks all other work with <30 minutes effort; assign to any implementor.
- **Write `extract_proton_family()` and version ranking tests first** — algorithm correctness is the highest-risk logic; test-driven approach catches edge cases (TKG, Experimental, "9-10" ordering) before wiring.
- **Build migration-specific round-trip test** alongside Group 1.1 — tests `load()` → modify `steam.proton_path` → `save()` → re-`load()` confirms new path in effective profile; prevents regression if `storage_profile()` logic changes.
- **Keep scan and apply as strictly separate IPC commands** — prevents accidental auto-write; matches security W-3 consent gate requirement.
- **Frontend types can start from the spec contracts** — `MigrationSuggestion`, `MigrationScanResult`, `MigrationApplyResult` TypeScript types are fully specified in `feature-spec.md`; implement without waiting for Rust side.
- **Phase 2 batch pre-flight is non-negotiable** — serialize + path-validate all targets before the first write; security W-4 hard requirement.
