# YAN-831 research: business and security invariants

Scope: `src-tauri/src/commands/{onboarding,offline}.rs`, `launch/queries.rs`. Paths are relative to `src/crosshook-native/`. Read-only research; no source edited.

## 1. Command inventory (current behaviour is the contract)

| Command                                                                                                | File:line               | Sync/async today                   | Notes                                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------ | ----------------------- | ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `check_readiness`                                                                                      | onboarding.rs:146       | **sync**                           | `SettingsStore.load` -> `check_system_readiness()` (Steam/Proton discovery, host fs, in Flatpak `flatpak-spawn` for distro) -> overlays -> sanitize `checks[].path` |
| `check_generalized_readiness`                                                                          | :167                    | **sync**                           | same + `evaluate_host_tool_checks` (`host_command_exists` per catalog tool, a `which` spawn under Flatpak) + **snapshot write**                                     |
| `probe_host_tool_details(tool_id)`                                                                     | :201                    | **sync**                           | `core::onboarding::probe_host_tool_details`; up to ~4 child spawns, each capped at 1500 ms (`details.rs:23`)                                                        |
| `get_cached_host_readiness_snapshot`                                                                   | :208                    | sync                               | DB read; disabled -> `Ok(None)`                                                                                                                                     |
| `get_capabilities`                                                                                     | :254                    | sync                               | cached snapshot else **live probe + snapshot write** (blocking on main thread)                                                                                      |
| `dismiss_onboarding`                                                                                   | :242                    | sync                               | settings only                                                                                                                                                       |
| `dismiss_umu_install_nag` / `dismiss_steam_deck_caveats`                                               | :279 / :302             | sync                               | dual write                                                                                                                                                          |
| `dismiss_readiness_nag(tool_id, ttl_days)`                                                             | :215                    | sync                               | SQLite only                                                                                                                                                         |
| `get_trainer_guidance`                                                                                 | :320                    | sync, pure static                  | no I/O                                                                                                                                                              |
| `check_offline_readiness` / `batch_offline_readiness` / `verify_trainer_hash` / `check_network_status` | offline.rs              | already `async` + `spawn_blocking` | `get_trainer_type_catalog` sync pure                                                                                                                                |
| `check_gamescope_session` / `launch_platform_status`                                                   | launch/queries.rs:57/63 | sync                               | env read / `OnceLock` cached `unshare --net true` spawn (first call blocks, `runtime_helpers/platform.rs:171`)                                                      |

## 2. Exact error strings (IPC frozen, D12)

All commands return `Result<_, String>`. Tauri rejects the JS promise with the **raw string, not an `Error`**.

### 2.1 What the UI really matches on

Grep of non-test `src/**/*.ts(x)` for every string below returned **zero** matches (`startsWith`/`includes` hits are unrelated: `useProfileMods` `metadata_unavailable:` belongs to mods; not in this scope). No frontend code branches on these strings. They are **display-only**:

- `HostToolsPage.tsx:114-117`: `nextError instanceof Error ? message : String(nextError)` -> a Tauri string rejection is shown **verbatim** in `role="alert"` (`:139-141`). So `readiness nag dismissals require the SQLite metadata store; …` (DB disabled) and `unknown readiness tool_id: …` are user-visible here.
- `useOnboarding.ts:179-219`: `error instanceof Error ? error.message : '<fixed fallback>'` -> with a real Tauri string rejection the **fixed fallback** shows; the backend message is not shown (mock tests throw `Error`, `useOnboarding.test.ts:57`). Do not "fix" this in this ticket.
- `useHostReadiness.ts:28-30` `normalizeError` -> `String(error)`: refresh/probe errors shown verbatim (`HostToolsPage.tsx:130-132,176`).
- `useLaunchState.ts:191,197-200,396-400,464-476`: offline/hash errors go through `normalizeRuntimeError` into `offlineReadinessError` and `Could not update stored trainer hash: ${message}`: display-only.
- `useOfflineReadiness.ts:92,109-113`: `normalizeError` -> display only.
- `useLaunchPlatformStatus.ts:17-25` and `useLaunchPrefixDependencyGate.ts:26-32` swallow errors.

### 2.2 Strings to keep byte-identical (cheap, protects docs/tests/support text)

| Origin                                          | String                                                                                                                                                      |
| ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| onboarding.rs:34 (`require_readiness_metadata`) | `readiness nag dismissals require the SQLite metadata store; dismiss_umu_install_nag and dismiss_steam_deck_caveats remain available via settings.toml`     |
| onboarding.rs:312                               | `unknown readiness tool_id: {tool_id}. Use a tool id from the host readiness catalog.`                                                                      |
| onboarding.rs:74,77,83                          | `failed to parse cached host readiness snapshot: {error}`; `invalid cached host readiness snapshot critical_failures value: {n}`; `... warnings value: {n}` |
| offline.rs:16,40,90                             | `metadata store unavailable`                                                                                                                                |
| offline.rs:25,99                                | `profile id not found for {name}`                                                                                                                           |
| offline.rs:102                                  | `trainer path is empty`                                                                                                                                     |
| offline.rs:107                                  | `trainer file missing or unreadable`                                                                                                                        |

Existing unit test asserts `error.contains("SQLite metadata store")` (onboarding.rs:~395); AC test must keep that substring. The only repo-wide hits for these strings are `offline.rs`, `onboarding.rs`, and an unrelated comment in `launch/preview/sections.rs`.

### 2.3 How prior services keep IPC strings (precedent)

- Adapters do `.map_err(|e| e.message().to_string())` (`commands/storage.rs:13`, `version.rs:13`, `mods.rs:17`, `steam.rs:24`). Health uses `error.to_string()` (equal to the message, `#[error("{message}")]`).
- `AppError::new` **always** runs `sanitize_message` (`error.rs:37-42`), so even "verbatim" strings are home-redacted (`$HOME`, `/home/<user>`, `/var/home/<user>`).
- `AppError::new` is `pub(crate)`, so services build exact legacy strings with `AppError::new(kind, "<legacy text>")` (mods did this, `mods/crud.rs:11-34`, D7/D8). `AppError::validation(field, msg)` is public.
- `From<MetadataStoreError>` gives only fixed texts (`metadata database operation failed`, `metadata database is unavailable`, `metadata validation failed`, …). `From<SettingsStoreError>`: `Io` -> kind only (e.g. `permission denied`), `TomlDe` -> `settings file is not valid TOML`. `From<ProfileStoreError>`: `InvalidName`/`NotFound` keep `error.to_string()` text (with `NotFound(path)` home-redacted), `Io` -> kind only, `TomlDe` -> `profile file is not valid TOML`.
- Hence raw `e.to_string()` strings in these commands (rusqlite/IO/TOML Display) become sanitized fixed texts. That is the accepted D8 delta ("Error text").

AppError kinds available (`error.rs:7-25`): `Validation{field}`, `NotFound`, `Conflict`, `Unavailable`, `Io`, `Cancelled`, `Internal`. No new variant is needed.

## 3. Dual-write dismissal ordering (must preserve)

Settings TOML is the source of truth for the two legacy nags; SQLite `readiness_nag_dismissals` is secondary.

| Command                                    | Order                                                                                                                                       | Partial failure                                                                                                                                                                                                                           | DB disabled                                                                                                                            |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| `dismiss_umu_install_nag`                  | 1) `SettingsStore.update(install_nag_dismissed_at = now RFC3339)`; 2) if `metadata.is_available()` `dismiss_readiness_nag("umu_run", 3650)` | settings fail -> `Err(to_string)`, **SQLite not touched** (comment :284-285 "never record metadata dismissal without the legacy settings timestamp"). SQLite fail -> **`tracing::warn!` only, returns `Ok(())`** (settings already saved) | settings only, `Ok(())`, **silent** (no warning today)                                                                                 |
| `dismiss_steam_deck_caveats`               | same with `steam_deck_caveats_dismissed_at`, tool id `"steam_deck_caveats"`, TTL **3650 days**                                              | same                                                                                                                                                                                                                                      | same                                                                                                                                   |
| `dismiss_readiness_nag(tool_id, ttl_days)` | **SQLite only**: `require_readiness_metadata` -> catalog lookup (`find_by_id`) -> `dismiss_readiness_nag(tool_id, ttl_days.unwrap_or(90))`  | each step `Err`                                                                                                                                                                                                                           | **hard `Err`** with the §2.2 string (the UI is not allowed to think it succeeded). Order: disabled check **before** catalog validation |
| `dismiss_onboarding`                       | settings only: `onboarding_completed = true`                                                                                                | `Err`                                                                                                                                                                                                                                     | n/a                                                                                                                                    |

Note: `dismiss_readiness_nag("umu_run")` from the Host Tools card writes SQLite only; no settings timestamp. Consequently `check_readiness` overlays both sources (settings first, then `apply_readiness_nag_dismissals` from SQLite). Keep that.

The issue says "dismissals persist to TOML only with a warning" when the DB is disabled. Today there is **no** warning on the disabled path; add a `tracing::warn!` there (behaviour-neutral, log only).

Settings writes use `SettingsStore::update` (process-local `io_lock`, atomic write via `write_atomic`); the service must hold the same `SettingsStore` clone as `SettingsService` (shared `Arc<Mutex>` lock) to keep atomicity. No `.unwrap()` on the `Infallible` inner result in the service; use `?`/`match` on the `Result<Result<_, Infallible>, _>`.

SQLite semantics: `get_dismissed_readiness_nags` filters `expires_at >= now` (RFC3339 string compare); `dismiss_readiness_nag` upserts by `tool_id`; TTL math `now + ttl_days`. `with_conn_mut` on a disabled/`!available` store returns `Ok(default)` silently (store.rs:202-204), so the `is_available()` guard in the command is the only thing that distinguishes disabled from success. A `read_only` store (NewerSchema) with `available=true` returns `Err(readonly_error)`; the legacy path for `dismiss_umu/steam` turns that into a warn (soft); for `dismiss_readiness_nag` it is a hard `Err`.

## 4. Snapshot persistence (`host_readiness_snapshots`, single row id=1)

- `check_generalized_readiness` **does write** the snapshot (onboarding.rs:177-188) before overlays: `upsert_host_readiness_snapshot(&result.tool_checks, distro, all_passed, critical_failures, warnings)`, only if `is_available()`. A write failure is a **hard error** for the whole command (`map_err(to_string)?`); the probe result is lost. The stored payload is the **raw probe** (unsanitized `resolved_path`, un-dismissed `install_guidance`). Sanitizing and dismissals apply on read (`load_cached_host_readiness_snapshot`: `get_dismissed_readiness_nags` -> `snapshot_row_to_payload` (decode + `sanitize_tool_checks`) -> `apply_tool_guidance_dismissals` clears `install_guidance`).
- `get_capabilities` also writes when no cached snapshot exists (live probe fallback, :263-275), hard error on write failure; it ignores `_store`.
- `check_readiness` (basic) and `probe_host_tool_details` do **not** write.
- Mock comment `src/lib/mocks/wrapHandler.ts:58-60` documents that `check_generalized_readiness` is not a pure read; keep that classification.
- Invariant for the move: persist **raw** probe output; sanitize at the presenter boundary; never persist home-redacted data (re-sanitizing is idempotent but persisting `~/` would change the cache format).
- Recommended: keep write failure fatal for `check_generalized_readiness` (existing, IPC-visible) but make `get_capabilities` honour the same. Do not silently downgrade without an explicit decision (see D-O6).
- Cached snapshot row validity: negative `critical_failures`/`warnings` -> error strings in §2.2. Disabled store -> `Ok(None)` (maps to "cached snapshots reported unavailable").

## 5. Display-path sanitizing

Two different helpers exist; they are **not** interchangeable.

| Helper                                                                      | Output                                                                                                                      | Used by                                                                                                                                                                                                                        |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crosshook_app::presenter::display_path_tilde(&str)` (`presenter/mod.rs:7`) | `~/rel` or `~/` for HOME exact; other paths **verbatim** (lexical `strip_prefix`, so `/home/user2` is safe); no HOME -> raw | the Tauri `commands::shared::sanitize_display_path` wrapper (`shared.rs:6-8`) -> onboarding.rs (`checks[].path`, `resolved_path`, capabilities) and migration.rs; health `types.rs:187`; launch diagnostics; profile migration |
| `presenter::sanitize_display_path(&Path)` (`:49`)                           | `$HOME/…` placeholder + `/var/home/<user>` alias + generic `/home/<user>` scrub                                             | `services/diagnostics.rs` only                                                                                                                                                                                                 |
| `presenter::sanitize_message` (`pub(crate)`)                                | message scrub, applied automatically in `AppError::new`                                                                     | all errors                                                                                                                                                                                                                     |
| core `home_to_tilde` (`onboarding/readiness/system.rs:17`)                  | **string** `starts_with(home)` -> `~…`; bug: `/home/user2/x` -> `~2/x`                                                      | core readiness `path` for steam root / proton / umu; output is re-run through `display_path_tilde` (no-op on `~`)                                                                                                              |

Binding fact: the onboarding IPC contract is the **tilde** form (`~/.local/bin/gamescope`; asserted by `snapshot_row_to_payload_decodes_and_sanitizes_resolved_paths`, onboarding.rs:~543). Switching to `$HOME` would break that test and any UI/mocks showing `~/`. The moved service must use `display_path_tilde`, applied to: `ReadinessCheckResult.checks[].path`, `HostToolCheckResult.resolved_path` (live and cached), `Capability.missing_required/optional[].resolved_path`, `HostToolDetails.resolved_path`. Not currently sanitized (and none of these are paths by contract): `docs_url`, `install_guidance`, `umu_install_guidance` command text (static), `steam_deck_caveats`.

Gaps to document, **not** fix here (IPC frozen): (a) `display_path_tilde` has no `/var/home` alias or generic `/home/<other>` scrub; (b) under Flatpak, `resolved_path` is a _host_ path (`normalize_flatpak_host_path`) while `$HOME` is the sandbox `~/.var/app/…`, so host usernames can pass through unredacted; (c) core `home_to_tilde` prefix bug. Offline reports (`OfflineReadinessReport`, `HashVerifyResult`) carry no paths; `HashVerifyResult{hash, from_cache, file_size}` has none.

## 6. Host process invocation and UI-supplied input

### 6.1 `probe_host_tool_details(tool_id)` (the only UI string reaching host probing)

Data flow (`core/onboarding/details.rs:57-205`): `global_readiness_catalog().find_by_id(tool_id)` **exact match** against catalog `tool_id` -> unknown id returns `HostToolDetails::unavailable(tool_id)` (no probe, **no error**, echoes the input id in `tool_id`). All downstream values (`binary_name`, version arg sets) come from the **catalog**, never from the UI string:

- `resolve_tool_binary_path`: empty binary -> None; `umu_run` special-case `resolve_umu_run_path`; binary containing `/` (catalog-provided absolute path) -> `normalize_and_validate_resolved_path`; else `is_safe_host_path_lookup_name` (`[A-Za-z0-9._+-]`, no `/`, no `..`, `gateway.rs:425`) AND `host_command_exists` AND `host_std_command("which").arg(binary)` (args passed as argv, no shell).
- `run_version_probe(resolved_path, &[version args])` -> `host_std_command(program)` (`flatpak-spawn --host program args…` in Flatpak, `Command::new` otherwise); stdin null; 1500 ms deadline with kill+wait; args are static (`--version`, `-V`, `-v`, `version`).
- No shell involved anywhere. `flatpak-spawn` receives `--host`, program, args as separate argv entries.

Conclusion: **no command-injection path from UI input**; the tool_id is validated by catalog membership by construction. Residual risk lives in the **catalog contents**: the user override `~/.config/crosshook/host_readiness_catalog.toml` (`load_readiness_catalog`, catalog.rs:172-211) can set arbitrary `binary_name` including absolute paths that are then executed with `--version`. That is the user's own config (trust boundary unchanged) and is existing behaviour; do not widen it.

Binding for the move: keep `probe_host_tool_details` calling core `find_by_id` and keep the soft failure contract (unknown id -> empty details, `Ok`). Do **not** convert it to an error (UI `useHostReadiness.probeTool` merges the result; an error would surface a banner). Validate length/emptiness only (UI already trims and rejects empty, `useHostReadiness.ts:161-165`); no extra denylist needed.

### 6.2 `dismiss_readiness_nag(tool_id)`

Validated against `global_readiness_catalog().find_by_id` before touching SQLite (SQL uses bound params anyway). Keep the catalog check **after** the `is_available` check (order matters for the error message in DB-disabled mode). `ttl_days: Option<u32>`: unbounded by value; `Duration::days(i64::from(u32))` cannot overflow `i64`, but `now + Duration` can panic on chrono overflow for huge values (>~ 2.4e8 days?). `[INFERENCE]` chrono's `DateTime + Duration` panics on out-of-range. Recommend clamping `ttl_days` (e.g. `min(.., 3650)`) — an additive hardening that does not change normal results (UI passes `null` -> 90).

### 6.3 Other host spawns

- `check_generalized_readiness`/`check_readiness`: `host_command_exists` (`which` under Flatpak), distro detection `cat /etc/os-release` via `host_std_command`. No UI input. These are the "sync host probes on main thread" the issue wants off the caller thread: wrap each in one `run_blocking`.
- `launch_platform_status`: `unshare --net true` via `host_std_command("unshare")` once, cached in a `OnceLock`; blocking on first use. Move into `run_blocking`; `check_gamescope_session` is a pure env read (`GAMESCOPE_WAYLAND_DISPLAY`) and cheap; either may stay sync but both should be `async` commands per the AC ("Tauri commands async"). The issue says reuse `platform::is_steam_deck()` / `is_inside_gamescope_session()` (the latter lives in `crosshook_core::launch::request::models:13`, re-exported as `crosshook_core::launch::is_inside_gamescope_session`).
- `LaunchPlatformCapabilities` serializes **camelCase** (`isFlatpak`, `unshareNetAvailable`); keep (mock returns `{unshare_net_available:false}` only in tests).
- `check_network_status`: TCP connects to fixed endpoints with 300 ms timeout; no input.

### 6.4 `verify_trainer_hash(name)` path handling

- `name` -> `ProfileStore::load(name)` first (`profile_path` -> `validate_name`: rejects empty, `.`, `..`, absolute, `/`, `\`, `:`, control chars, Windows-reserved chars, the anonymous key). So traversal through `name` is blocked before any metadata call. `lookup_profile_id(name)` runs only after load succeeds.
- The **trainer path comes from the saved profile** (`profile.effective_profile().trainer.path.trim()`), not from the UI. Empty -> `trainer path is empty`. The path is passed as `Path` to `verify_trainer_hash_for_profile_path`, which does `fs::metadata` (follows symlinks; must be a regular file else `Ok(None)` -> `trainer file missing or unreadable`), caches by `(profile_id, file_path)` keyed on size + mtime, hashes with `fs::read` of the **whole file into memory** (`hash_trainer_file`, `version_store.rs:212`, unbounded).
- No `flatpak-spawn` is used, so under Flatpak the sandbox view of the path applies (`normalize_flatpak_host_path` is NOT applied here; unchanged behaviour). The profile path is a user-trusted value; the result discloses only a hash and file size.
- Invariants to keep: validate profile via `ProfileStore` before using the id; do not accept a path from the caller; keep the `Ok(None)` -> fixed error mapping; DB-disabled check **first** (before `load`).
- Hardening (optional, additive): streaming hash would remove the unbounded memory read; belongs in core, not required by AC. Record as follow-up (like D15).

### 6.5 Offline commands order of checks (preserve)

`check_offline_readiness` / `verify_trainer_hash`: (1) `!is_available` -> `metadata store unavailable`; (2) load profile; (3) `lookup_profile_id` -> missing -> `profile id not found for {name}`; (4) core call. `batch_offline_readiness`: (1) availability; (2) `store.list()` error is **hard**; (3) per-profile load/lookup/check failures are **skipped with `tracing::warn!`** (never abort); output order = `list()` order. `check_offline_readiness_for_profile` itself can `Err(readonly_error)` when read-only; `check_network_status`/`get_trainer_type_catalog` have no failure path (the latter is a sync pure catalog clone).

## 7. Disabled-store behaviour matrix (target after move)

| Operation                                               | Disabled metadata                                                                                                                                |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `check_readiness`, `check_generalized_readiness`        | succeed; no snapshot write; dismissal overlays from settings only                                                                                |
| `get_cached_host_readiness_snapshot`                    | `Ok(None)`                                                                                                                                       |
| `get_capabilities`                                      | live probe, **no** write, `Ok`                                                                                                                   |
| `dismiss_umu_install_nag`, `dismiss_steam_deck_caveats` | settings write, SQLite skipped, `Ok`, **add warn log**                                                                                           |
| `dismiss_readiness_nag`                                 | `Err` with legacy string (§2.2)                                                                                                                  |
| offline commands                                        | `Err("metadata store unavailable")`                                                                                                              |
| startup offline-readiness scan                          | scan still runs health; per the issue cached snapshots reported unavailable (see health `snapshots.rs` which already returns `Unavailable` text) |

## 8. Recommended binding decisions (numbered D-O\*, to mirror D1–D16)

- **D-O1 Error text.** Adapters map `AppError` -> `error.message().to_string()`. Services build the legacy strings of §2.2 with `AppError::new(kind, "<exact legacy text>")` using kinds: availability -> `Unavailable`; unknown tool_id / empty trainer path -> `Validation{field: "tool_id"|"trainer"}`; missing profile id / missing trainer file -> `NotFound`. Underlying store errors use the existing `From` impls (fixed texts, accepted D8 delta); the legacy-string constants live in one `const` block per service and are covered by tests that assert the exact text (substring `SQLite metadata store` for the readiness one).
- **D-O2 Dual-write order preserved, in one function.** `OnboardingService::dismiss_nag_legacy(DismissKind)` does: settings `update` first (hard error, SQLite untouched on failure); then if `metadata.is_available()` `dismiss_readiness_nag(id, 3650)`; SQLite failure -> `tracing::warn!` with sanitized error, still `Ok`. Disabled -> settings only + warn (new log line only). Test matrix: DB enabled / disabled / SQLite write forced failing (read-only store or dropped table) -> settings persisted in all three, `Ok` in all three, SQLite row present only in the first. Share a single `SettingsStore` clone with `SettingsService` (same `io_lock`).
- **D-O3 `dismiss_readiness_nag` stays SQLite-only**, availability check **before** catalog validation, TTL default 90, clamp `ttl_days` to a sane maximum (3650) to avoid chrono overflow; UI behaviour unchanged for `null`.
- **D-O4 Snapshot persistence.** `OnboardingService::check_generalized` runs probe -> upsert raw snapshot (if available, failure fatal as today) -> overlays -> sanitize paths, all in one `run_blocking` closure (AC1). Cached snapshot decode keeps the sanitize-then-dismiss order and the §2.2 decode error strings. `get_capabilities` keeps cached-else-live-with-write semantics.
- **D-O5 Path sanitizing uses `presenter::display_path_tilde`** everywhere in these services (the tilde contract is IPC-frozen and test-pinned); delete the `commands/shared.rs` use for onboarding but keep `shared::sanitize_display_path` for `migration.rs`. Do not introduce `$HOME` output. Document the `/var/home` + Flatpak-host gaps as follow-ups, not fixes. Services apply sanitization once, at the DTO boundary, never before persisting.
- **D-O6 Host probes.** `probe_host_tool_details` keeps catalog-membership validation (unknown id -> empty details, `Ok`, no error); no UI-controlled value reaches argv. All host-spawning work (`check_system_readiness`, `evaluate_host_tool_checks`, `probe_host_tool_details`, `launch_platform_capabilities` first call, `check_network_status`) runs inside `run_blocking`; commands become `async`. Probes are injectable for tests through a small `HostProbe` trait or fn pointers on `OnboardingService` (AC2 "readiness with fake host probe"); production impl calls core. No new `flatpak-spawn` call sites.
- **D-O7 Offline services keep check order** (availability -> load -> id -> core) and batch skip-and-warn semantics; `verify_trainer_hash` accepts only a profile name; the trainer path is always read from the validated profile. Streaming hash is a core follow-up (record next to D15-type items), not part of this ticket.
- **D-O8 Metadata disabled** is detected with `MetadataStore::is_available()` (not by catching errors) because `with_conn*` returns `Ok(default)` silently on a disabled store; the legacy asymmetry (hard error for offline + `dismiss_readiness_nag`, soft for cached reads and legacy dismissals) is preserved and tested.
- **D-O9 IPC frozen** (same as D12): command names, camelCase arg keys (`toolId`, `ttlDays`, `name`), serde shapes (`CachedHostReadinessSnapshot` fields, `LaunchPlatformCapabilities` camelCase), the 10 + 5 + 2 command names and `check-mock-coverage.sh` unchanged. `get_trainer_guidance` and `get_trainer_type_catalog` may stay sync adapters (pure static) unless the contract test requires otherwise; the 3 commands the AC names MUST become `async`.
- **D-O10 Do-not-change list (review checklist).** (1) settings-before-SQLite ordering; (2) 3650-day TTL for the two legacy nags, 90-day default for per-tool; (3) raw snapshot persisted, sanitized/dismissed on read; (4) soft unknown tool_id in probe; (5) hard error for dismiss with disabled DB; (6) the exact strings in §2.2; (7) tilde path format; (8) no new host command invocations or shells; (9) `AppError` never carries raw rusqlite/TOML text.

## 9. Test hooks implied by the invariants (for the plan)

1. Dual-write: enabled / disabled / SQLite-write-fails (settings still saved) for both legacy nags; assert TOML timestamp, SQLite active set, `Ok`, and a `warn` on the disabled path.
2. `dismiss_readiness_nag`: disabled -> legacy text (contains `SQLite metadata store`); unknown id -> legacy text; known id + `ttl None` -> row with ~90-day expiry; huge ttl clamped, no panic.
3. Snapshot: after `check_generalized` with a fake probe, `get_host_readiness_snapshot` has the **raw** (unsanitized) `resolved_path`; the IPC result has `~/` paths; dismissal clears `install_guidance` for cached read.
4. Cached decode errors: invalid JSON, negative counts -> legacy strings; disabled -> `None`.
5. Probe: unknown id -> empty details `Ok`; known id with fake probe -> sanitized path.
6. Offline: disabled -> `metadata store unavailable` for all three; empty trainer path; missing profile id; batch skips failing profiles without aborting; name traversal (`../x`, `a/b`) rejected before metadata access.
7. Command contract tests (`command_signatures_match_expected_ipc_contract` in onboarding.rs) must be rewritten for async (name pinning like `mods.rs:87`).
