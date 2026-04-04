# Protontricks Integration — Research Recommendations

## Executive Summary

CrossHook's architecture supports prefix dependency management cleanly via the health enrichment hook, lazy binary discovery pattern, `tokio::process::Command` subprocess model, and SQLite migration infrastructure. External API research has revised the primary tool recommendation: **winetricks-direct (WINEPREFIX-based) is the recommended primary approach**, with protontricks as an optional user-configured alternative. Because CrossHook already stores prefix paths in its SQLite metadata DB, protontricks' main value-add (Steam App ID → prefix resolution) is redundant. Winetricks requires no Steam process, works for non-Steam prefixes, supports `list-installed` directly, and is a widely available bash script. protontricks remains supported for users who prefer it or already have it installed.

The feature name `required_protontricks` in the profile schema should be generalized to `required_wine_deps` or kept as `required_protontricks` with documentation that winetricks verbs are the canonical naming convention used regardless of which tool runs them. The core implementation module is `crosshook-core/src/prefix_deps/` (three files: `mod.rs`, `runner.rs`, `store.rs`). No new crates are required — all dependencies (`tokio`, `rusqlite`, `serde`, `toml`) are already in `crosshook-core`'s `Cargo.toml`.

**Blocking architectural decision before implementation begins**: whether to use a static hard-coded allowlist or a dynamic allowlist derived from `winetricks list`. This choice affects startup behavior, security posture, and maintenance burden and must be resolved first.

---

## Implementation Recommendations

### Approach

Follow the existing "lazy, per-profile, on-demand" model that governs health checks and offline readiness. Do not introduce a background scanner or startup preflight. Dependency installation should trigger only on explicit user action (launch or manual "Fix now" trigger in the profile health panel).

**Primary tool: winetricks-direct.** Invoke as:

```
WINEPREFIX=/path/to/pfx winetricks -q <verb1> <verb2> ...
```

CrossHook already stores the prefix path (`runtime.prefix_path` / `steam.compatdata_path`) — this is all winetricks needs. No Steam process required. Works for both `steam_applaunch` and `proton_run` launch methods.

**Secondary tool: protontricks (user-configured optional).** When the user has protontricks installed and prefers it, allow configuring its path in settings. Invoke as:

```
protontricks <steam_app_id> -q <verb1> <verb2> ...
```

Protontricks requires a Steam App ID and Steam running. Limit this path to profiles with a valid `steam.app_id` or `runtime.steam_app_id`.

**Execution model**: `tokio::process::Command` + `env_clear()` + minimal env restoration, identical to `script_runner.rs`. Winetricks needs: `HOME`, `PATH` (minimal), `STEAM_ROOT`, `STEAM_COMPAT_DATA_PATH`, `WINEPREFIX`. Optionally `WINE` pointing to the Proton wine binary for some verb categories. Reuse `apply_host_environment()` + `attach_log_stdio()` + `is_executable_file()` from `launch/runtime_helpers.rs` directly — no new utility code needed. For synchronous execution contexts, follow the `install/service.rs` pattern: `Handle::try_current().block_on(child.wait())` inside `spawn_blocking`.

**Security-critical invocation constraints**: Always insert `--` between the Steam App ID argument and verb arguments to prevent flag injection (S-06). Never pass `--force` or any checksum-bypass flag — checksum failures must surface as explicit errors (S-08). Enforce a 300-second hard timeout via `tokio::time::timeout` on the entire install operation.

**User confirmation gate**: Every install operation must receive explicit user confirmation before the subprocess is spawned — display the list of verbs to be installed in the confirmation dialog. This is both a security transparency requirement and a UX necessity.

**Binary resolution**: Mirror `resolve_umu_run_path()` — scan `PATH` + known install locations (`/usr/bin/winetricks`, `~/.local/bin/winetricks`). Return `Option<PathBuf>`. Store user-configured override in `AppSettingsData.winetricks_path: Option<String>` and `AppSettingsData.protontricks_path: Option<String>` (same pattern as `steamgriddb_api_key`).

**Schema addition**: Add `required_protontricks: Vec<String>` to `CommunityProfileManifest` in `community_schema.rs`. Keep the field name `required_protontricks` for community naming convention compatibility — document that these are winetricks verb names regardless of which tool installs them. Bump `COMMUNITY_PROFILE_SCHEMA_VERSION` to 2. Add the same field to `GameProfile` as an optional user-declared override (empty by default, `skip_serializing_if = "Vec::is_empty"`).

**SQLite state tracking**: Add `migrate_14_to_15()` (migration 15, current version is 14) in `metadata/migrations.rs` creating a `prefix_dependency_state` table, following the exact `if version < N { migrate(); pragma_update(None, "user_version", N); }` pattern:

```sql
CREATE TABLE prefix_dependency_state (
    profile_id     TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    package_name   TEXT NOT NULL,
    state          TEXT NOT NULL DEFAULT 'unknown',  -- unknown | installed | failed
    checked_at     TEXT NOT NULL,
    PRIMARY KEY (profile_id, package_name)
);
```

This follows the exact pattern of `health_snapshots`, `offline_readiness_snapshots`, and `trainer_hash_cache`. The companion store file is `metadata/prefix_deps_store.rs` (named to match `health_store.rs`, `offline_store.rs`) with functions taking bare `&Connection` — not `&MetadataStore`.

**Prefix initialization guard**: Winetricks can only operate on an initialized WINE prefix. Before invoking, check that the prefix path contains a `pfx/` subdirectory (same check used in `onboarding/readiness.rs` for `game_launched_once`). If the prefix is uninitialized, surface a `HealthIssue` with remediation "Launch the game once with Proton to initialize the prefix before installing dependencies." Do not attempt to auto-initialize.

**Verb installation detection**: Use `WINEPREFIX=/path/to/pfx winetricks list-installed` — this works correctly with the WINEPREFIX model and does not require a Steam App ID. Parse the output (newline-delimited verb names) to check which packages are already installed before deciding whether to run a full install. Cache the result in SQLite as `state = 'installed'`. Fall back to exit-code-only detection if `list-installed` is unavailable for the winetricks version on the system.

### Technology Choices

- **Primary install tool**: winetricks (direct WINEPREFIX invocation)
- **Optional secondary tool**: protontricks (user-configured, requires Steam App ID)
- **Process spawning**: `tokio::process::Command` (async, already used everywhere in launch)
- **Output parsing**: Parse exit code for success/failure. Parse `list-installed` output (newline-delimited) for pre-install state check. Do NOT parse install output for state — use `list-installed` or exit code only.
- **stderr handling (S-11)**: Raw winetricks/protontricks stderr must NEVER reach the UI — it contains full filesystem paths and Wine debug output. Capture stderr to internal `tracing` log only via `attach_log_stdio()`. Surface only templated user-facing error messages to the UI on failure.
- **stdout to UI**: Sanitized stdout lines (filtered for Wine debug noise) may be streamed to `ConsoleView.tsx`. Apply a line-level filter that drops lines matching Wine internal patterns before emitting Tauri events.
- **Dependency state persistence**: SQLite via `metadata/migrations.rs` migration
- **Binary path settings**: `AppSettingsData` fields in `settings/mod.rs`
- **Profile schema**: Extend `CommunityProfileManifest` + `GameProfile` via `profile/models.rs` and `profile/community_schema.rs`
- **Install output display**: Reuse `ConsoleView.tsx` (already used for launch log output) for streaming filtered winetricks stdout during install. Stderr is captured to internal log only — never forwarded to `ConsoleView.tsx`. No new output component needed.

### Phasing Strategy

Build in this order — each phase is independently mergeable:

1. **Binary detection** — `resolve_winetricks_path()` (and optionally `resolve_protontricks_path()`) in `launch/runtime_helpers.rs` or new `prefix_deps/mod.rs` module. Onboarding readiness check. No UI yet.
2. **Schema addition** — `required_protontricks` field in `CommunityProfileManifest` and `GameProfile`. Version bump. Backward-compatible (empty vec = no-op).
3. **SQLite migration** — `migrate_14_to_15()` + new `metadata/prefix_deps_store.rs` module.
4. **Install runner** — async function in `crosshook-core` that invokes winetricks (or protontricks), pre-checks `list-installed`, streams output via Tauri events, writes result to SQLite.
5. **Health integration** — Enrich `ProfileHealthReport` with missing-dependency issues via `batch_check_health_with_enrich` closure.
6. **IPC + UI** — Tauri commands (`check_prefix_dependencies`, `install_prefix_dependencies`, `get_wine_deps_tool_path`); frontend progress modal; profile editor dependency list.

### Quick Wins

- Onboarding check for winetricks binary (adds to existing readiness panel, minimal code)
- `required_protontricks` schema field with `#[serde(default, skip_serializing_if = "Vec::is_empty")]` (pure data, no behavior yet)
- SQLite migration (isolated, testable, zero UI impact)

---

## Improvement Ideas

### Related Feature Enhancements

- **Dependency health badge**: The profile list already shows health status badges. Extend `HealthBadge.tsx` with a "missing dependencies" variant (distinct color from Broken/Stale) for immediate visibility without entering the profile.
- **Per-package status chips**: The dependency section in the profile view must show individual status chips per package — one of: installed / missing / installing / failed. A spinner badge on each chip during active install is required. This is a non-negotiable UX requirement based on competitive analysis showing spinners-only (Bottles pattern) cause user anxiety during 10–20 minute installs.
- **Community profile metadata display**: Show `required_protontricks` packages in the community profile preview UI before import, so users know what will be installed.
- **Dependency history log**: Log install attempts in `launch_operations` table (already exists in SQLite) with a `dependency_install` operation type. Gives users a history trail.
- **Bulk dependency check**: After importing multiple community profiles, offer a "Check all dependencies" sweep that uses `winetricks list-installed` per prefix (deduplicated by prefix path) and batches SQLite writes.
- **Shared-prefix warning**: If two or more profiles share the same `compatdata_path`, warn the user before installing dependencies. Installing packages for one profile's workflow may affect other games using that prefix. Surface this as an `Info`-severity `HealthIssue` at profile view time.

### Optimizations

- **TTL-based cache invalidation**: The `checked_at` column in `prefix_dependency_state` enables staleness detection. After N days (configurable, default 7), mark dependency state as `unknown` to trigger re-verification. Mirrors the `offline_readiness_snapshots.checked_at` pattern.
- **Deduplication across profiles**: If two profiles share the same prefix path, `required_protontricks` checks and installs can be shared. Consider tracking state by `(prefix_path_hash, package_name)` rather than `(profile_id, package_name)` to avoid redundant installs. This is a follow-on optimization; v1 can use `profile_id`.
- **Pre-install `list-installed` check**: Before running the full install, check which verbs are already installed via `winetricks list-installed`. Skip the install entirely if all verbs are already present. This avoids the long install flow for users who already have dependencies from a prior manual install.

### Integration with Existing Flows

- **Launch gate**: Integrate dependency check into `validate()` in `launch/request.rs`. A new `ValidationError::MissingPrefixDependency(String)` variant raises a non-fatal warning (not blocking) unless opt-in strict mode is enabled.
- **Health system**: Use `batch_check_health_with_enrich` to inject missing-dependency issues into `ProfileHealthReport.issues` — these surface in the existing health UI without new infrastructure.
- **Onboarding**: Add winetricks binary check to `check_system_readiness()` in `onboarding/readiness.rs` with `Info` severity (optional, not blocking).

---

## Risk Assessment

### Technical Risks

| Risk                                                              | Severity | Likelihood | Notes                                                                                                                                                                                                     |
| ----------------------------------------------------------------- | -------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Flatpak protontricks sandbox blocks secondary Steam libraries     | High     | High       | Only affects protontricks path; winetricks-direct is unaffected. Mitigated by making winetricks primary.                                                                                                  |
| `winetricks list-installed` output changes between versions       | Medium   | Medium     | Parse is simple (newline-delimited verb names). Fall back to exit-code-only if parse fails.                                                                                                               |
| Verb installation detection unreliable for some verb categories   | High     | High       | Some verbs modify only the registry, some only install files. `list-installed` may not always reflect true state. Mitigation: rely on idempotent install (re-run is safe) + cached SQLite state with TTL. |
| Concurrent installs corrupt WINE prefix                           | High     | Medium     | No locking exists today. A per-prefix async Mutex (keyed by prefix path) in the Tauri app state is needed.                                                                                                |
| winetricks binary not found on minimal installs                   | Medium   | Medium     | Less common than protontricks absence — winetricks is a bash script in most distro repos. Discovery function covers `PATH` + known paths.                                                                 |
| Package name injection via community profile                      | Critical | Low        | Mitigated by allowlist validation against known winetricks verb names before passing to `Command::arg()`. Do not use shell spawning.                                                                      |
| `WINE=` path required for some winetricks verb categories         | Medium   | Medium     | Some verbs (dotnet, vcrun) need `WINE` to point to Proton's wine binary. CrossHook already knows `proton_path` — derive wine binary path as `dirname(proton_path)/files/bin/wine`.                        |
| Uninitialized prefix at install time                              | Medium   | Medium     | Fresh profiles have no prefix until Proton runs once. Must detect and gate with clear remediation message.                                                                                                |
| `auto_install_prefix_deps` triggers silent long install at launch | Medium   | Medium     | Default must be `false`. When `true`, show a blocking progress indicator before the game spawns — never install silently in background.                                                                   |
| Long-running install blocks UI thread                             | Low      | Low        | Using `tokio::process::Command` async keeps UI responsive; progress streamed via Tauri events.                                                                                                            |

### Integration Challenges

- **Gamescope session**: Winetricks should NOT be run inside gamescope. Check `is_inside_gamescope_session()` and spawn outside if needed.
- **Multiple Steam libraries**: When a prefix is in a non-default Steam library, prefix path resolution must use `discover_steam_libraries()` already in `steam/libraries.rs`. Winetricks-direct avoids this for non-Steam prefixes since CrossHook directly stores the path.
- **Wine dialogs during install**: Winetricks install steps may spawn visible Wine dialogs (installers, UAC prompts). These look like crashes to uninitiated users. The UI must show a clear "installation in progress" state with messaging that Wine dialogs may appear and are expected.
- **Install duration warnings**: dotnet48 installations take 10–20 minutes. The install confirmation UI must warn the user before starting when `dotnet48` (or any `dotnet` verb) is in the package list. Other common verbs (vcrun2019, d3dx9, corefonts, xact) complete in seconds to ~2 minutes and do not require a pre-warning.
- **Graceful degradation when binary absent**: When neither winetricks nor protontricks is found, the `required_protontricks` section must render in a degraded state (packages listed, status chips grayed out, "Install winetricks to manage dependencies" call-to-action) rather than hiding the section or blocking the profile from loading. The app must not error or prevent usage of unrelated features.
- **protontricks bwrap failures**: When protontricks path is used, `--no-bwrap` flag may be needed if the bubblewrap sandbox conflicts with the Steam Runtime environment. Surface this as a hint when protontricks exits with a bwrap-related error.
- **Flatpak settings help note**: Settings screen must show a help note with the Flatpak invocation string (`flatpak run com.github.Matoking.protontricks`) to paste into the binary path override field when native protontricks is unavailable.

### Performance Considerations

- **`winetricks list-installed` cost**: This command spawns Wine and reads the registry — it is slow (several seconds). Use only as a pre-install check and for display in the UI detail view, never on every health check iteration. Cache results in SQLite with TTL.
- **Prefix scanning speed**: Dependency state checks against SQLite are fast (single row lookup). Avoid running `list-installed` on every health check; use the SQLite cache as the source of truth between TTL expirations.
- **Package list size**: Profiles should not declare more than ~10 packages. There is no technical enforcement but community guidelines should recommend minimal declarations.

### Security Risks (Critical to Address)

**CRITICAL — Must block implementation (S-01/S-02/S-06/S-03)**

1. **No shell interpolation ever** (S-01/S-02, Critical): Use `Command::new("winetricks")` with individual `.arg()` calls only. Never `Command::new("sh").arg("-c").arg(format!(...))`. CrossHook's existing `runtime_helpers.rs` already uses the correct pattern.
2. **Flag injection via `--` separator** (S-06, Critical): Always insert `cmd.arg("--")` between the Steam App ID and verb arguments. A verb name beginning with `-` (e.g. `-c`) triggers a winetricks flag without the separator. The structural regex `^[a-z0-9][a-z0-9_\-]{0,63}$` also naturally rejects `-`-prefixed strings.
3. **Structural verb validation** (S-03, Critical): `validate_protontricks_verbs()` in `crosshook-core/src/prefix_deps/validation.rs` — max 50 verbs per profile, structural regex required (`^[a-z0-9][a-z0-9_\-]{0,63}$`), dual-layer (structural check + known-verb warning for unknown-but-structurally-valid names). Must run at tap sync time AND at install time.

**WARNING — Must address before shipping (S-04/S-05/S-07/S-08/S-10/S-11)**

4. **Steam App ID never from community TOML** (S-04, High): `app_id` must be typed `u32`, nonzero, and sourced from the internal game record only — never from the community profile TOML. Protontricks is used only when a valid nonzero Steam App ID exists; non-Steam prefixes must use winetricks-direct.
5. **Prefix path never from community TOML** (S-05, High): Prefix path must come from CrossHook's Steam discovery layer or stored profile data. If any boundary accepts path input, canonicalize and assert `starts_with(expected_root)`.
6. **env_clear() + minimal restoration** (S-07, High): `cmd.env_clear()` then restore only: `HOME`, `PATH` (minimal), `STEAM_ROOT`, `STEAM_COMPAT_DATA_PATH`. Never pass `DISPLAY`/`WAYLAND_DISPLAY` env vars that could leak session context unnecessarily.
7. **No --force / no checksum bypass** (S-08, High): Never pass `--force` or equivalent to winetricks/protontricks. Checksum failures surface as explicit errors — never silently bypassed.
8. **Concurrent install lock** (S-10, High): Per-prefix async Mutex in core layer (not just Tauri app state) to prevent registry corruption from concurrent winetricks runs against the same prefix.
9. **Stderr never to UI** (S-11, High): Raw stderr contains full filesystem paths and Wine debug output. Capture to internal `tracing` log only. Surface only templated error messages to the UI on failure.

**ADVISORY — Best practice, deferrable**

10. **Executable path validation** (Medium): Validate `winetricks_path` / `protontricks_path` from settings before invoking. Note: `check_required_executable()` at `profile/health.rs:209` is a **private `fn`** — it cannot be called from `prefix_deps/`. Two options: (a) promote it to `pub(crate)` in `health.rs` (small, targeted change); (b) duplicate the 30-line check pattern inline in `prefix_deps/mod.rs`. Option (a) is preferred for DRY.
11. **Symlink check on prefix path** (S-14, Medium): Apply `symlink_metadata()` check (same as `db.rs`) when resolving the WINE prefix path. Low practical risk given paths come from Steam discovery.
12. **Audit log** (S-12, Advisory): Log which community profiles triggered which installs (verb, timestamp, success/failure, tap source) in the SQLite metadata DB. Reuse or extend `launch_operations` table.
13. **Outdated winetricks detection** (S-09, Advisory): Distribution-packaged winetricks often has stale checksums. Detect and warn when winetricks version is outdated. Prefer Flatpak winetricks for this reason as well.

---

## Alternative Approaches

### Option A: Winetricks-direct / WINEPREFIX-based (Recommended)

**Approach**: Invoke `winetricks -q <verbs>` with `WINEPREFIX` set from the profile's stored prefix path.

**Pros**: No Steam process required; works for both `steam_applaunch` and `proton_run` launch methods; CrossHook already stores the prefix path; `winetricks list-installed` works consistently; widely available (bash script in most distro repos); no Flatpak sandbox complications.

**Cons**: Requires `WINE=` env var for some verb categories (derivable from `proton_path`); requires winetricks installed.

**Effort**: Medium.

### Option B: Protontricks-primary / App ID-based

**Approach**: Invoke `protontricks <steam_app_id> -q <verbs>`.

**Pros**: Familiar to users who already use protontricks manually; auto-resolves prefix from App ID; handles Proton version detection.

**Cons**: Requires Steam running; requires a valid Steam App ID (not available for all CrossHook profiles); Flatpak protontricks has sandbox issues with secondary Steam libraries; `bwrap` failures require `--no-bwrap` workaround; `list-installed` still requires invoking winetricks separately; redundant given CrossHook already stores prefix paths.

**Effort**: Medium — but more edge cases than Option A.

**Verdict**: Support as a user-configured secondary tool for users who prefer it, not as the default. Primary path is winetricks-direct.

### Option C: Both (protontricks for detection, winetricks for install)

**Approach**: Use protontricks to resolve the prefix path via Steam App ID, then invoke winetricks directly against that path.

**Pros**: Leverages protontricks' Steam discovery.

**Cons**: Added complexity for no benefit — CrossHook already stores prefix paths so protontricks' discovery adds nothing.

**Verdict**: Reject. The hybrid adds complexity without benefit given CrossHook's data model.

### Option D: Eager Pre-flight Scanner

**Approach**: Scan all prefix dependency states on app startup using `winetricks list-installed`.

**Pros**: Health status always current when user views profile list.

**Cons**: `list-installed` spawns Wine per prefix — would be extremely slow at startup across multiple profiles.

**Verdict**: Reject. Use lazy/on-demand pattern consistent with existing health checks.

---

## Task Breakdown Preview

### Phase 1 — Foundation (Low complexity, no UI)

- `resolve_winetricks_path()` and `resolve_protontricks_path()` in `crosshook-core/src/prefix_deps/mod.rs` — follow `resolve_umu_run_path()` template in `launch/runtime_helpers.rs` exactly. Import from `launch::runtime_helpers` directly; `onboarding/readiness.rs` re-imports it from there but should not be used as the import source.
- Add winetricks check to `check_system_readiness()` in `onboarding/readiness.rs` with `Info` severity
- `PrefixDepsSection` struct in `profile/models.rs` with `required_protontricks: Vec<String>`; add to `GameProfile`; backward-compatible (empty = skip serialization)
- `required_protontricks` field in `CommunityProfileManifest` (`community_schema.rs`); bump `COMMUNITY_PROFILE_SCHEMA_VERSION` to 2
- Unit tests for schema round-trip and backward-compatibility (missing field deserializes to empty vec)

### Phase 2 — Storage (Low complexity, isolated)

- `migrate_14_to_15()` in `metadata/migrations.rs` — `prefix_dependency_state` table
- New `metadata/prefix_deps_store.rs` with `upsert_dependency_state()`, `load_dependency_states()`, `check_dependency_state()` — functions take bare `&Connection`, following `health_store.rs` and `offline_store.rs` conventions; use `open_in_memory()` from `metadata/db.rs` in all tests
- Unit tests for migration and store

### Phase 3 — Install Runner (Medium complexity, async)

- `crosshook-core/src/prefix_deps/runner.rs` — define `ProtontricksRunner` trait with `RealRunner` (actual CLI) and `FakeRunner` (canned results for tests); `run_deps_install(request, log_path)` function
- `crosshook-core/src/prefix_deps/store.rs` — thin wrapper re-exporting from `metadata/prefix_deps_store.rs` or inline state helpers
- `crosshook-core/src/prefix_deps/validation.rs` — `validate_protontricks_verbs(verbs: &[String]) -> Result<(), ValidationError>`: structural regex `^[a-z0-9][a-z0-9_\-]{0,63}$`, max 50 verbs, dual-layer (hard reject structurally invalid + warn unknown-but-valid); run at tap sync AND at install time
- `cmd.arg("--")` inserted between Steam App ID and verb arguments in runner — flag injection prevention (S-06)
- `winetricks list-installed` pre-check — parse newline-delimited output; skip packages already installed
- Prefix initialization guard (check `pfx/` subdirectory exists before invoking)
- Per-prefix async Mutex in Tauri app state to prevent concurrent installs
- For protontricks invocation: require a valid nonzero Steam App ID and invoke only in that case; for non-Steam prefixes always use winetricks-direct with `WINEPREFIX`; derive `WINE=` as `dirname(proton_path)/files/bin/wine` only when needed.
- `attach_log_stdio()` from `runtime_helpers.rs` for log file; capture stderr to tracing log only — never forward to UI (S-11)
- 300-second `tokio::time::timeout` wrapping the full install `child.wait()` call
- User confirmation payload must reach the runner before subprocess is spawned (IPC layer enforces this)
- Output streaming via `tokio` channels + Tauri event emitter
- Integration with `MetadataStore` to persist results via `prefix_deps_store.rs`
- Unit tests using `FakeRunner` — no protontricks binary required on test hosts

### Phase 4 — Health Integration (Low complexity)

- Add dependency enrichment closure to `batch_check_health_with_enrich` call in Tauri `health` command. The closure signature is `FnMut(&str, &GameProfile, &mut ProfileHealthReport)` — it does NOT receive a SQLite connection. Dependency state for all profiles in the batch must be pre-loaded from `prefix_deps_store.rs` before the closure is constructed, then captured by reference in the closure body.
- Reuse existing `Error`/`Warning` severity levels for missing dependencies
- Update `ProfileHealthReport` with dependency status
- Shared-prefix detection and `Info`-severity warning when multiple profiles share a prefix

### Phase 5 — IPC + UI (Medium complexity)

- New `src-tauri/src/commands/prefix_deps.rs` with commands: `check_prefix_dependencies`, `install_prefix_dependencies`, `get_wine_deps_tool_paths` — thin async → `spawn_blocking` → core fn → `.map_err(|e| e.to_string())` following `install.rs` IPC pattern
- Add `winetricks_path: Option<String>` and `protontricks_path: Option<String>` to `AppSettingsData` and `AppSettingsIpcData`
- Settings UI: winetricks + protontricks binary path fields with Flatpak help note
- Profile health UI: "Missing dependencies" issue with "Install now" action
- Progress modal for install operations (stream via Tauri events) with "Wine dialogs may appear" messaging
- Community profile preview: list `required_protontricks` packages before import
- Per-package status chips (`installed` / `missing` / `installing` / `failed`) in dependency section — reuse `HealthBadge.tsx` styling conventions
- Streaming install output via `ConsoleView.tsx` (same component used for launch logs) — not a modal spinner alone
- User confirmation dialog before every install: show verb list, warn on `dotnet` verbs (10–20 min), require explicit "Install" action — never trigger install without this step (S-08 security transparency + UX requirement)
- dotnet48 duration warning message shown within the confirmation dialog when `dotnet` verbs are present
- Graceful degraded state when winetricks binary is absent: show packages, grayed chips, install-winetricks CTA

---

## Key Decisions Needed

1. **Static vs. dynamic verb allowlist** (Blocking — resolve before Phase 3): Hard-coded list of known winetricks verb names in source, or derived at runtime from `winetricks list`? Static is simpler and more auditable but requires maintenance. Dynamic reduces maintenance but adds startup cost and a subprocess dependency. **Recommendation**: static for v1 with a refresh plan each release cycle. Note: S-03 security requirement mandates structural validation regardless of which allowlist approach is chosen — structural regex is the hard gate, known-verb check is advisory.
2. **Field name: `required_protontricks` vs. `required_wine_deps`**: Keep `required_protontricks` for community naming convention compatibility (many users know the term), or rename to `required_wine_deps` to be tool-agnostic? Renaming is cleaner but breaks any existing community profiles using the field. Since this is a new field (schema v2, no existing usage), renaming is viable before v1 ships. Either way, the `GameProfile` section struct is named `PrefixDepsSection` — this is an implementation detail not exposed in the TOML surface.
3. **Blocking vs. warning for missing dependencies at launch**: Should CrossHook prevent launch if required dependencies are uninstalled, or just warn? Default-to-warn with an opt-in strict mode preserves user agency.
4. **Flatpak protontricks support scope**: Is Flatpak protontricks supported in v1 or documented-unsupported? Since winetricks is now primary, this only affects the optional protontricks path. At minimum, the settings UI should document the Flatpak override.
5. **Dependency state TTL**: How long before cached `installed` state is considered stale and re-verified? 7 days is a reasonable default but this is a product decision.
6. **`auto_install_prefix_deps` setting**: Should this exist? If yes, default must be `false` and the launch flow must show a blocking progress indicator before spawning the game — never install silently.

---

## Open Questions

- Does `winetricks list-installed` produce consistent output across winetricks versions (v20220411, v20230212, etc.)? Is the output format stable enough to parse reliably?
- When deriving `WINE=` from `proton_path`, is the convention `dirname(proton_path)/files/bin/wine` stable across Proton, GE-Proton, and umu-proton versions?
- Does winetricks behave correctly when `WINEPREFIX` points to a prefix created by Proton (which uses a different prefix structure than vanilla Wine)?
- Are Wine dialogs spawned during winetricks install visible on the correct display/Wayland socket when CrossHook passes `DISPLAY`/`WAYLAND_DISPLAY` through `apply_host_environment()`?
- For the protontricks secondary path: what is the exit code behavior when `--no-bwrap` is needed? Is there a reliable way to detect bwrap failure vs. other errors?

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` — CommunityProfileManifest definition; add `required_protontricks` here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — GameProfile definition; optional `required_protontricks` field here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/health.rs` — `batch_check_health_with_enrich`, health issue types; extend with dependency enrichment
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `AppSettingsData`; add `winetricks_path` and `protontricks_path` fields
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — add `migrate_14_to_15()` for `prefix_dependency_state` table (current schema is v14)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/prefix_deps_store.rs` — new file; follow `health_store.rs` and `offline_store.rs` conventions (bare `&Connection` args, no `MetadataStore` wrapper)
- `crosshook-core/src/prefix_deps/` — new module: `mod.rs` (public API + path resolution), `runner.rs` (`ProtontricksRunner` trait + `RealRunner`/`FakeRunner`), `store.rs` (state helpers), `validation.rs` (`validate_protontricks_verbs()` — structural regex + known-verb dual-layer check)
- `src-tauri/src/commands/prefix_deps.rs` — new IPC command file; follow `install.rs` thin-command pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore`; add dependency store methods
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` — `resolve_umu_run_path` (exact template for path resolution), `apply_host_environment`, `attach_log_stdio`, `is_executable_file` — reuse all four directly, no wrappers needed
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` — subprocess spawning pattern to follow
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` — binary availability check pattern; add winetricks check; prefix initialization check pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/db.rs` — symlink protection pattern for path validation
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/settings.rs` — `AppSettingsIpcData` DTO; add winetricks/protontricks path fields
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs` — IPC command pattern to follow for new dependency commands
- `docs/plans/protontricks-integration/research-business.md` — business domain analysis; verb detection details, shared-prefix risks, auto-install UX risks
- `docs/plans/protontricks-integration/research-ux.md` — UX competitive analysis; Bottles/Lutris/Heroic/Steam patterns, per-package chip model, duration warnings, binary-not-found degradation requirements
- `docs/plans/protontricks-integration/research-practices.md` — engineering practices analysis; exact reuse inventory, module structure, `ProtontricksRunner` trait design, testability approach, no-new-crates confirmation
- `docs/plans/protontricks-integration/research-security.md` — full security findings by severity; S-01 through S-14 with code examples, `validate_protontricks_verbs()` spec, env var restrictions, `--` flag separator rationale
- `docs/plans/protontricks-integration/research-external.md` — external API research; protontricks vs winetricks invocation models, dependency matrix
