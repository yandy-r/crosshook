# Task Structure Analysis: dev-web-frontend

## Executive Summary

Phase 1 splits into 18 discrete tasks organized into four sequential waves — foundation config (5 independent tasks), adapter/stubs/mock skeleton/dev indicator (8 parallel tasks gated on wave 1), boot handlers (2 tasks gated on wave 2), and one critical-path serializer (mechanical migration, 1 task) followed by CI + docs (2 leaf tasks). All Phase 2 handler tasks are fully independent of each other and can be opened as concurrent sub-PRs. The non-obvious dependency is `1.5` (ProfileSummary export): it has no upstream dependency itself but is a prerequisite for both boot handler tasks (1.13, 1.14), making it a quiet blocker that should be tackled in wave 1 even though it touches existing files, not new ones.

---

## Recommended Phase Structure

### Phase 1: Foundation (single PR per D2)

**Purpose**: Ship the adapter layer, plugin stubs, mock skeleton, boot-critical handlers, mechanical migration, dev-mode indicator, and CI safety gate so the app boots past the loading screen at `http://localhost:5173`.

**Suggested Tasks**:

1. 1.1 — Vite config, tsconfig, vite-env.d.ts (`__WEB_DEV_MODE__` define + `@/` alias + webdev server config)
2. 1.2 — `package.json` `dev:browser` script
3. 1.3 — `scripts/dev-native.sh` `--browser`/`--web` case branch + help text
4. 1.4 — `lib/runtime.ts` (`isTauri()` probe)
5. 1.5 — Export `ProfileSummary` from `types/library.ts`; update `useLibrarySummaries.ts` import
6. 1.6 — `lib/ipc.ts` (`callCommand<T>` adapter)
7. 1.7 — `lib/events.ts` (`subscribeEvent<T>` + `emitMockEvent` + `browserBus`)
8. 1.8 — `lib/plugin-stubs/dialog.ts`
9. 1.9 — `lib/plugin-stubs/shell.ts`
10. 1.10 — `lib/plugin-stubs/fs.ts`
11. 1.11 — `lib/plugin-stubs/convertFileSrc.ts`
12. 1.12 — `lib/mocks/store.ts` + `lib/mocks/index.ts` + `lib/mocks/eventBus.ts` + `lib/mocks/README.md`
13. 1.13 — `lib/mocks/handlers/settings.ts` (boot-blocking settings commands)
14. 1.14 — `lib/mocks/handlers/profile.ts` (boot-blocking profile commands)
15. 1.15 — `lib/DevModeBanner.tsx` + `lib/dev-indicator.css`
16. 1.16 — Mechanical migration (all `invoke`/`listen`/plugin call sites + App.tsx chip wire + `main.tsx` eager import)
17. 1.17 — `verify:no-mocks` grep step in `.github/workflows/release.yml`
18. 1.18 — `AGENTS.md` Commands block update

**Parallelization**: After the initial 5 foundation tasks land (1.1–1.5, all independent), tasks 1.6–1.12 and 1.15 can all run in parallel (8 concurrent). Boot handlers 1.13 and 1.14 can run in parallel after 1.5 + 1.12. Only 1.16 (mechanical migration) serializes everything. 1.17 and 1.18 are concurrent leaf tasks. **At least 10 of 18 tasks have no inter-task blocking constraint within their wave.**

---

### Phase 2: Handler Fan-out (multiple PRs)

**Purpose**: Expand mock handler coverage across all ~18 domain groups so every major route renders meaningful data.

**Dependencies**: Phase 1 merged

**Suggested Tasks**:

- 2.1 — Launch handlers + events
- 2.2 — Profiles mutation handlers + `profiles-changed` event broadcast
- 2.3 — Health Dashboard handlers + events
- 2.4 — Onboarding events + commands
- 2.5 — Install flow handlers + events
- 2.6 — Update flow handlers + events
- 2.7 — Proton stack management handlers
- 2.8 — ProtonUp handlers
- 2.9 — ProtonDB handlers
- 2.10 — Community handlers
- 2.11 — Launcher export handlers
- 2.12 — Library (cover art, metadata, steam) handlers
- 2.13 — Discovery / run-executable / prefix storage / diagnostics / catalog (remaining `system.ts` additions)

**Parallelization**: Fully parallel; all 13 tasks are independent of each other; multiple sub-PRs can be in flight simultaneously.

---

### Phase 3: Polish (multiple small PRs)

**Purpose**: Fixture variants, orthogonal debug toggles, coverage tooling, docs, optional test automation.

**Dependencies**: Phase 2 Launch (2.1) + Profiles (2.2) merged so fixture variants have enough surface area to be meaningful.

**Suggested Tasks**:

- 3.1 — Fixture state switcher (`?fixture=populated|empty|error|loading`)
- 3.2 — Orthogonal debug toggles (`?errors=true`, `?delay=<ms>`, `?onboarding=show`)
- 3.3 — Handler-coverage check script (`dev:browser:check`)
- 3.4 — Project README update (dev-browser flow + link to `lib/mocks/README.md`)
- 3.5 — Optional Playwright smoke test (boot dev server + screenshot every route)
- 3.6 — `refactor:` follow-up issue for 13 components calling `invoke()` directly
- 3.7 — Fixture-content CI lint (SteamID64 + home-path pattern grep scoped to `lib/mocks/fixtures/`)

**Parallelization**: All 7 tasks are independent; can ship as separate small PRs simultaneously.

---

## Task Granularity Recommendations

### Appropriate Task Sizes

- Example: "Create `lib/runtime.ts` with `isTauri()` probe" (1 file, no deps, ~6 lines of code) — this is the right floor
- Example: "Add `__WEB_DEV_MODE__` define + `@/` alias + webdev host/strictPort to `vite.config.ts`" (1 file, but touches 3 sibling files `tsconfig.json` + `vite-env.d.ts` that are all pure config — group them as one task because they fail together)
- Example: "Migrate all 42 `invoke()` call sites to `callCommand()`" (1 meta-task touching many files — mechanical, batched; a reviewer can skim in ~15 minutes because every change has the same shape)
- Example: "Create `lib/mocks/handlers/settings.ts`" (1 file, ~40 lines using `DEFAULT_APP_SETTINGS`)

### Tasks to Split

- **1.16 (mechanical migration)** could technically split into: (a) migrate `invoke` sites (42 files), (b) migrate `listen` sites (13 files), (c) migrate plugin imports (6 files), (d) wire `<DevModeChip />` into `App.tsx`. Per D2, all land in one PR, but commits can be structured as four logical commits for reviewability — the split is a commit strategy, not a PR split.
- **1.12 (mock registry skeleton)** groups 4 files. Could be split to 1.12a (`store.ts`), 1.12b (`eventBus.ts`), 1.12c (`index.ts`), 1.12d (`README.md`). Not recommended — these 4 files are tiny and their only purpose is as a group; splitting adds no value.

### Tasks to Combine

- `1.8`, `1.9`, `1.10`, `1.11` (all four plugin stubs) could be one task "Create all plugin stub modules". They are genuinely independent of each other, but grouping them respects that they follow identical patterns and a contributor would naturally do all four at once. If assigning to separate contributors, split them; if one contributor owns stubs, group them.
- `1.2` (package.json script) and `1.3` (dev-native.sh flag) are both script-layer changes with no deps and no shared files — keep them separate to minimize merge conflicts if multiple people work in parallel.

---

## Dependency Analysis

### Phase 1 Dependency Graph

**Wave 1 — Foundation (all independent, start immediately in parallel):**

- **1.1** — `vite.config.ts` + `tsconfig.json` + `vite-env.d.ts`: mode-conditional `define: { __WEB_DEV_MODE__: mode === 'webdev' }`, `resolve.alias: { '@': path.resolve(__dirname, './src') }`, webdev-mode `server.host = '127.0.0.1'` + `strictPort: true`, tsconfig `paths: { "@/*": ["./src/*"] }`, `declare const __WEB_DEV_MODE__: boolean`
- **1.2** — `package.json`: add `"dev:browser": "vite --mode webdev"` to `scripts`
- **1.3** — `scripts/dev-native.sh`: add `--browser|--web)` case branch that `exec npm run dev:browser`; update `usage()` heredoc
- **1.4** — `lib/runtime.ts`: `export function isTauri(): boolean { return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window; }`
- **1.5** — `src/types/library.ts`: move/export `ProfileSummary` interface (currently local to `useLibrarySummaries.ts`); update `useLibrarySummaries.ts` import

**Wave 2 — Adapter + stubs + mock skeleton + indicator (all parallel, gate on relevant wave-1 tasks):**

- **1.6** — `lib/ipc.ts`: `callCommand<T>` with `ensureMocks()` dynamic import + `if (!__WEB_DEV_MODE__) throw` inside `ensureMocks()`. Depends on `[1.1, 1.4]`
- **1.7** — `lib/events.ts`: `subscribeEvent<T>` + `emitMockEvent` + module-scope `browserBus`. Depends on `[1.1, 1.4]`
- **1.8** — `lib/plugin-stubs/dialog.ts`: re-export real plugin in Tauri mode; `null` + `console.warn('[dev-mock] dialog suppressed')` in browser. Depends on `[1.4]`
- **1.9** — `lib/plugin-stubs/shell.ts`: re-export in Tauri; `open` no-op + warn; `execute` / `Command.spawn` throw. Depends on `[1.4]`
- **1.10** — `lib/plugin-stubs/fs.ts`: re-export in Tauri; read stubs resolve with synthetic data; write/delete/rename/createDir throw. Depends on `[1.4]`
- **1.11** — `lib/plugin-stubs/convertFileSrc.ts`: re-export in Tauri; synchronous passthrough in browser (returns path unchanged). Depends on `[1.4]`
- **1.12** — `lib/mocks/store.ts` + `lib/mocks/index.ts` + `lib/mocks/eventBus.ts` + `lib/mocks/README.md`: `MockStore` singleton, `registerMocks()` orchestrator, event-bus glue, contributor guide. Depends on `[1.1]`
- **1.15** — `lib/DevModeBanner.tsx` + `lib/dev-indicator.css`: fixed corner chip + `.crosshook-app--webdev` CSS inset outline. Depends on `[1.1]`

**Wave 3 — Boot handlers (parallel pair, gate on 1.5 + 1.12):**

- **1.13** — `lib/mocks/handlers/settings.ts`: handlers for `settings_load`, `recent_files_load`, `recent_files_save`, `default_steam_client_install_path` using `DEFAULT_APP_SETTINGS`. Depends on `[1.5, 1.12]`
- **1.14** — `lib/mocks/handlers/profile.ts`: handlers for `profile_list`, `profile_list_favorites`, `profile_list_summaries`, `check_readiness`, `get_optimization_catalog`, `batch_validate_profiles` using `createDefaultProfile()` and `ProfileSummary`. Depends on `[1.5, 1.12]`

**Wave 4 — Mechanical migration (serial, gates on all wave-2 + wave-3 tasks):**

- **1.16** — Migrate all `invoke`/`listen`/plugin call sites; wire `<DevModeChip />` into `App.tsx`; add eager `convertFileSrc` import in `main.tsx`. Depends on `[1.6, 1.7, 1.8, 1.9, 1.10, 1.11, 1.13, 1.14, 1.15]`

**Wave 5 — CI + docs (parallel leaf tasks, gate on 1.16):**

- **1.17** — `.github/workflows/release.yml`: add `verify:no-mocks` step after Build native AppImage, grep for `[dev-mock]`, `getMockRegistry`, `registerMocks`, `MOCK MODE`. Depends on `[1.16]`
- **1.18** — `AGENTS.md`: add `./scripts/dev-native.sh --browser` to Commands block; add 1-paragraph "Browser Dev Mode" section pointing to `lib/mocks/README.md`. Depends on `[1.16]`

---

### Independent Tasks (Can Run in Parallel) — Phase 1

**Wave 1 (5 tasks, all start immediately in parallel):**

- 1.1, 1.2, 1.3, 1.4, 1.5

**Wave 2 (8 tasks, all start after their minimal prerequisite):**

- 1.6, 1.7 — both need only `[1.1, 1.4]`, independent of each other
- 1.8, 1.9, 1.10, 1.11 — all need only `[1.4]`, independent of each other
- 1.12 — needs only `[1.1]`, independent of 1.6–1.11
- 1.15 — needs only `[1.1]`, independent of everything else in wave 2

**Wave 3 (2 tasks, parallel after 1.5 + 1.12):**

- 1.13, 1.14 — independent of each other

**Note on 1.5 (ProfileSummary export):** Although `1.5` depends on `[none]`, it needs to complete before wave 3 can start. Assign it to wave 1 and treat its completion as a release condition for wave 3. This is the non-obvious critical-path element — it has no upstream deps but is a downstream blocker.

**Phase 1 parallel task count: 13 of 18 tasks (72%) have no intra-wave blocking constraint.**

---

### Sequential Dependencies (Phase 1)

```
[none] → 1.1, 1.2, 1.3, 1.4, 1.5   (wave 1, all parallel)
    1.1 → 1.6, 1.7, 1.12, 1.15
    1.4 → 1.6, 1.7, 1.8, 1.9, 1.10, 1.11
    1.1 + 1.4 → 1.6 (ipc.ts), 1.7 (events.ts)
    1.5 + 1.12 → 1.13 (settings handlers), 1.14 (profile handlers)
    1.6 + 1.7 + 1.8 + 1.9 + 1.10 + 1.11 + 1.13 + 1.14 + 1.15 → 1.16 (migration)
    1.16 → 1.17 (CI gate), 1.18 (docs)
```

Critical path: 1.1/1.4 → 1.6/1.7/1.12 → 1.13/1.14 → 1.16 → 1.17/1.18

Minimum sequential depth: **4 waves** (foundation → adapter layer → boot handlers → migration → leaf tasks = 5 steps, but steps 1-3 overlap heavily via parallelism)

---

### Potential Bottlenecks

- **1.16 (mechanical migration)** is the single biggest task — touches 42 `invoke` files + 13 `listen` files + 6 plugin files + `App.tsx` + `main.tsx`. This is the critical-path serializer and the highest-risk task for merge conflicts. All preceding wave-2 and wave-3 tasks must be complete before 1.16 can start. Recommendation: do not attempt 1.16 until 1.6, 1.7, 1.8–1.11, 1.12, 1.13, 1.14, and 1.15 are all reviewed and ready to land as a unit.
- **`App.tsx` is touched by both 1.15 and 1.16.** 1.15 creates `DevModeBanner.tsx` but does NOT touch `App.tsx`. Only 1.16 modifies `App.tsx` (listen migration + chip wire). This ordering is correct and safe — wire the chip in 1.16 after the component file from 1.15 exists.
- **`vite.config.ts` is touched only by 1.1.** No coordination needed with any other task.
- **`tsconfig.json` is touched only by 1.1.** Same note.
- **1.5 (ProfileSummary export)** touches `types/library.ts` and `hooks/useLibrarySummaries.ts`. These files are also touched by 1.16 (migration). Ensure 1.5 lands before 1.16 begins to avoid a three-way merge on `useLibrarySummaries.ts`.
- **`main.tsx` is touched only by 1.16** (eager convertFileSrc import). No coordination needed.

---

## File-to-Task Mapping

### Files to Create (Phase 1)

| File                                                          | Task | Phase | Dependencies |
| ------------------------------------------------------------- | ---- | ----- | ------------ |
| `src/crosshook-native/src/lib/runtime.ts`                     | 1.4  | 1     | none         |
| `src/crosshook-native/src/lib/ipc.ts`                         | 1.6  | 1     | 1.1, 1.4     |
| `src/crosshook-native/src/lib/events.ts`                      | 1.7  | 1     | 1.1, 1.4     |
| `src/crosshook-native/src/lib/plugin-stubs/dialog.ts`         | 1.8  | 1     | 1.4          |
| `src/crosshook-native/src/lib/plugin-stubs/shell.ts`          | 1.9  | 1     | 1.4          |
| `src/crosshook-native/src/lib/plugin-stubs/fs.ts`             | 1.10 | 1     | 1.4          |
| `src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts` | 1.11 | 1     | 1.4          |
| `src/crosshook-native/src/lib/mocks/store.ts`                 | 1.12 | 1     | 1.1          |
| `src/crosshook-native/src/lib/mocks/index.ts`                 | 1.12 | 1     | 1.1          |
| `src/crosshook-native/src/lib/mocks/eventBus.ts`              | 1.12 | 1     | 1.1, 1.7     |
| `src/crosshook-native/src/lib/mocks/README.md`                | 1.12 | 1     | none         |
| `src/crosshook-native/src/lib/mocks/handlers/settings.ts`     | 1.13 | 1     | 1.5, 1.12    |
| `src/crosshook-native/src/lib/mocks/handlers/profile.ts`      | 1.14 | 1     | 1.5, 1.12    |
| `src/crosshook-native/src/lib/DevModeBanner.tsx`              | 1.15 | 1     | 1.1          |
| `src/crosshook-native/src/lib/dev-indicator.css`              | 1.15 | 1     | 1.1          |

### Files to Modify (Phase 1)

| File                                                             | Task | Phase | Dependencies         |
| ---------------------------------------------------------------- | ---- | ----- | -------------------- |
| `src/crosshook-native/vite.config.ts`                            | 1.1  | 1     | none                 |
| `src/crosshook-native/tsconfig.json`                             | 1.1  | 1     | none                 |
| `src/crosshook-native/src/vite-env.d.ts`                         | 1.1  | 1     | none                 |
| `src/crosshook-native/package.json`                              | 1.2  | 1     | none                 |
| `scripts/dev-native.sh`                                          | 1.3  | 1     | none                 |
| `src/crosshook-native/src/types/library.ts`                      | 1.5  | 1     | none                 |
| `src/crosshook-native/src/hooks/useLibrarySummaries.ts`          | 1.5  | 1     | none                 |
| `src/crosshook-native/src/App.tsx`                               | 1.16 | 1     | 1.7, 1.15            |
| `src/crosshook-native/src/main.tsx`                              | 1.16 | 1     | 1.11                 |
| `src/crosshook-native/src/context/PreferencesContext.tsx`        | 1.16 | 1     | 1.6                  |
| `src/crosshook-native/src/utils/optimization-catalog.ts`         | 1.16 | 1     | 1.6                  |
| ~39 other `invoke`-using files under `src/crosshook-native/src/` | 1.16 | 1     | 1.6                  |
| ~12 other `listen`-using files under `src/crosshook-native/src/` | 1.16 | 1     | 1.7                  |
| ~6 plugin-import files under `src/crosshook-native/src/`         | 1.16 | 1     | 1.8, 1.9, 1.10, 1.11 |
| `.github/workflows/release.yml`                                  | 1.17 | 1     | 1.16                 |
| `AGENTS.md`                                                      | 1.18 | 1     | 1.16                 |

---

## Phase 2 Detailed Task Breakdown

Ordered by iteration value (highest first). Each is an independent sub-PR. All depend on Phase 1 being merged.

### 2.1 — Launch handlers + events

**Target file**: `src/crosshook-native/src/lib/mocks/launch.ts`

**Commands to mock**: `launch_game`, `launch_trainer`, `preview_launch`, `validate_launch`, `check_game_running`, `verify_trainer_hash`, `check_gamescope_session`, `run_executable` family

**Events to emit via `emitMockEvent`**: `launch-log` (streaming text lines), `launch-diagnostic`, `launch-complete`, `profiles-changed`

**Notes**: Mutating launch handlers must emit `launch-log` events with realistic streaming behavior (multiple calls with a delay, or a batch). Use `setTimeout` inside the handler to simulate async streaming. Round-trip via `store.ts` is not needed for launch (stateless simulation).

---

### 2.2 — Profiles mutation handlers + event broadcast

**Target file**: `src/crosshook-native/src/lib/mocks/handlers/profile.ts` (extend existing)

**Commands to mock**: `profile_save`, `profile_duplicate`, `profile_rename`, `profile_delete`, `profile_set_favorite`, `profile_export_toml`, `config_history_*`, `config_diff`, `config_rollback`, optimization preset commands

**Events to emit**: `profiles-changed` on every mutating handler

**Notes**: Use `store.ts` for round-trip: `profile_save` updates `store.profiles[id]` and returns the saved payload; `profile_list` reads from `store.profiles`. `profile_delete` must remove the entry and emit `profiles-changed`. Per BR-6, mutating commands must return the stored object, not void.

---

### 2.3 — Health Dashboard handlers + events

**Target file**: `src/crosshook-native/src/lib/mocks/health.ts`

**Commands to mock**: `batch_validate_profiles` (already in Phase 1 minimal boot set — extend here with richer payload), `get_profile_health`, `get_cached_health_snapshots`, `check_offline_readiness` family, `check_version_status`, `acknowledge_version_change`, `protondb_lookup`, `protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion`

**Events to emit**: `profile-health-batch-complete`, `version-scan-complete`

**Notes**: Health batch results should include a mix of healthy, warning, and error entries for realistic fixture data. The `batch_validate_profiles` mock from Phase 1 returns minimal data; expand it here without breaking the boot path.

---

### 2.4 — Onboarding events + commands

**Target file**: `src/crosshook-native/src/lib/mocks/handlers/` (extend `system.ts` or new `onboarding.ts`)

**Commands to mock**: `dismiss_onboarding`, `check_version_status`

**Events to emit**: `onboarding-check` (synthesized on mount when `?onboarding=show` query param is present), `auto-load-profile`

**Notes**: `check_readiness` is already mocked in Phase 1 (boot-blocking). This PR focuses on the event-driven onboarding wizard flow. The `?onboarding=show` URL param gates the event emission — document this in `lib/mocks/README.md`.

---

### 2.5 — Install flow handlers + events

**Target file**: `src/crosshook-native/src/lib/mocks/` (new `install.ts` or extend `system.ts`)

**Commands to mock**: `install_game`, `validate_install_request`, `install_default_prefix_path`

**Events to emit**: `install-progress`, `install-complete`, `install-error`

**Notes**: Streaming install events require multiple `emitMockEvent` calls with sequential payloads. Use `setTimeout` chaining to simulate download/install progress steps. Return final install status from the command handler.

---

### 2.6 — Update flow handlers + events

**Target file**: Extend `system.ts` or new `update.ts`

**Commands to mock**: `update_game`, `validate_update_request`, `cancel_update`

**Events to emit**: `update-log` (streaming), `update-complete`

**Notes**: `cancel_update` must update the store to reflect cancelled state and should suppress any pending `setTimeout` event emissions (a simple "cancelled" flag in the module is sufficient). Mirrors the install flow shape.

---

### 2.7 — Proton stack management

**Target file**: Extend `system.ts` or new `proton.ts`

**Commands to mock**: `list_proton_installs`, `check_proton_migrations`, `apply_proton_migration`, `apply_batch_migration`

**Notes**: `list_proton_installs` should return a realistic array of `ProtonInstallOption[]` from `types/proton.ts` including a mix of GE-Proton, Valve Proton, and SteamTinkerLaunch entries. Migration commands return success payloads.

---

### 2.8 — ProtonUp handlers

**Target file**: Extend `system.ts` or new `protonup.ts`

**Commands to mock**: `protonup_list_available_versions`, `protonup_install_version`, `protonup_get_suggestion`

**Notes**: `protonup_install_version` should emit progress events and return a success result. Use obviously fake version strings (`GE-Proton99-1`, `GE-Proton99-2`).

---

### 2.9 — ProtonDB handlers

**Target file**: Extend `health.ts` or new `protondb.ts`

**Commands to mock**: `protondb_lookup`, `protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion`

**Notes**: May overlap with 2.3 if `protondb_lookup` was already added there. Coordinate or consolidate into `health.ts` to avoid duplication. `protondb_get_suggestions` should return a non-empty array so the suggestion-accept UI is reachable.

---

### 2.10 — Community handlers

**Target file**: `src/crosshook-native/src/lib/mocks/community.ts`

**Commands to mock**: `community_list_profiles`, `community_list_indexed_profiles`, `community_sync`, `community_add_tap`, `community_prepare_import`, `community_export_profile`, `discovery_search_trainers`

**Notes**: Community fixtures must use obviously fake trainer names (`Test Trainer Alpha`, `Dev Trainer Beta`) and URLs (`https://example.com/tap/test`). No real Steam App IDs. `community_sync` should emit a completion event.

---

### 2.11 — Launcher export handlers

**Target file**: Extend `system.ts` or new `launcher.ts`

**Commands to mock**: `list_launchers`, `check_launcher_exists`, `validate_launcher_export`, `export_launchers`, `preview_launcher_*`, `delete_launcher*`

**Notes**: `list_launchers` should return entries for common launchers (Steam, Lutris, Heroic) with obviously fake paths. Export validation commands return success with a preview payload.

---

### 2.12 — Library (cover art, metadata, steam) handlers

**Target file**: `src/crosshook-native/src/lib/mocks/library.ts`

**Commands to mock**: `fetch_game_cover_art`, `import_custom_art`, `fetch_game_metadata`, `auto_populate_steam`, `build_steam_launch_options_command`

**Notes**: `fetch_game_cover_art` should return a data-URI placeholder or empty string (not a real `tauri://` path) — remember `convertFileSrc` is a passthrough in browser mode, so the resulting `<img src>` will be a plain path that fails to load. Return an empty string or a static placeholder asset path that exists under `public/`. `auto_populate_steam` returns a mock metadata payload for a fake Steam App ID `9999001`.

---

### 2.13 — Remaining system.ts additions (prefix storage, diagnostics, catalog, art, steam)

**Target file**: Extend `system.ts`

**Commands to mock**: `export_diagnostics`, `detect_protontricks_binary`, `get_dependency_status`, `prefix_storage_*` family, `get_trainer_type_catalog`, `get_mangohud_presets`, remaining `get_optimization_catalog` depth (if Phase 1 boot version was minimal)

**Notes**: These are the tail-end commands that Phase 1 and earlier Phase 2 PRs left as `[dev-mock] Unhandled command: ...` errors. Claim this PR only after opening a specific page that hits one of these commands and encountering the unhandled-command error.

---

## Optimization Opportunities

### Maximize Parallelism

**Wave 1** (5 tasks, all start immediately in parallel, target: complete within one sitting):

- 1.1 (config files), 1.2 (package.json), 1.3 (dev-native.sh), 1.4 (runtime.ts), 1.5 (ProfileSummary export)
- Total: ~1–2 hours of focused work; simple files with clear specs

**Wave 2** (8 tasks, parallel after wave 1's minimal deps):

- 1.6 (ipc.ts) + 1.7 (events.ts) need [1.1, 1.4] — start as soon as wave 1 is done
- 1.8, 1.9, 1.10, 1.11 need [1.4] only — can start the moment `runtime.ts` exists
- 1.12 (mock skeleton) needs [1.1] only — can start the moment vite.config.ts is done
- 1.15 (dev indicator) needs [1.1] only — same
- Theoretical parallelism: assign plugin stubs (4 tasks) to one contributor, adapter core (1.6, 1.7) to another, mock skeleton (1.12) + indicator (1.15) to a third

**Wave 3** (2 tasks, parallel after 1.5 + 1.12):

- 1.13 and 1.14 are independent; one contributor can own both, or assign one each

**Wave 4** (1 task, serial):

- 1.16 (migration) must be done as a focused session; recommend `sed`/`rg` for the find-replace portion, then manual review of the App.tsx chip wire and main.tsx import

**Wave 5** (2 tasks, parallel after 1.16):

- 1.17 (CI) and 1.18 (docs) are independent 15-minute tasks

**Phase 1 critical path length: 4 logical waves.** In practice, a single contributor doing all 18 tasks sequentially takes the full wave sequence. With 3 contributors, waves 1–3 collapse into roughly 2 sessions.

### Minimize Risk

- Land the CI `verify:no-mocks` gate (1.17) as the first commit in the single Phase 1 PR so the sentinel is active during code review, not just after merge. Even though it depends on 1.16 (migration) to be meaningful, inserting it early in the commit history costs nothing and documents intent.
- Before opening the Phase 1 PR for review, run `rg 'from ['"'"'"'"'"']@tauri-apps/api/core['"'"'"'"'"']' src/crosshook-native/src/ --glob '!lib/'` and verify zero hits (confirms migration completeness per SC-14).
- Test `./scripts/build-native.sh --binary-only` locally before relying on CI for the `verify:no-mocks` check. One manual grep confirms Rollup eliminated the mock subtree.
- The `checkReadiness` + `get_optimization_catalog` + `batch_validate_profiles` mocks (in `profile.ts` or a new `system.ts`) are the ones most likely to be missing from Phase 1 boot handlers if the implementation is rushed — verify the app shell renders before declaring 1.16 complete.

---

## Implementation Strategy Recommendations

### Commit Structure for the Single Phase 1 PR

Per D2, all Phase 1 work ships in one PR. Recommended commit structure (preserves reviewability):

```
feat(build): add --browser flag to dev-native.sh and dev:browser npm script
feat(build): add __WEB_DEV_MODE__ define, @/ alias, webdev server config to vite.config.ts
feat(ui): create lib/runtime.ts isTauri probe
feat(ui): create lib/ipc.ts callCommand adapter and lib/events.ts subscribeEvent adapter
feat(ui): create lib/plugin-stubs/ (dialog, shell, fs, convertFileSrc)
feat(ui): create lib/mocks/ skeleton (store, index, eventBus, README)
feat(ui): export ProfileSummary from types/library.ts
feat(ui): create boot-critical mock handlers (settings, profile, system)
feat(ui): create DevModeBanner component and dev-indicator.css
feat(ui): migrate all invoke/listen/plugin call sites to adapter layer
feat(build): add verify:no-mocks CI sentinel to release.yml
docs(internal): update AGENTS.md Commands block with --browser flag
```

### Testing Sequence

1. After wave 2 tasks: run `npm run dev:browser` manually inside `src/crosshook-native/`; verify no import errors; verify dev indicator renders
2. After 1.14: boot with all 10 boot-blocking handlers in place; verify Library page renders without `[dev-mock] Unhandled command:` errors in console
3. After 1.16 (migration): `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`; manual smoke of all 9 routes; check Tauri mode still works
4. After 1.17: `./scripts/build-native.sh --binary-only` locally; run `grep -r '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' src/crosshook-native/dist/assets/*.js` — should return no matches

### Label Taxonomy

Phase 1 PR labels: `type:feature`, `area:ui`, `area:build`, `priority:medium`

Phase 2 PR labels (per handler PR): `type:feature`, `area:ui`, `priority:low`

Phase 3 PR labels: `type:feature` or `type:docs`, `area:ui`, `area:build`, `priority:low`

### Documentation Commit Policy

Per `CLAUDE.md`, commits that change files under `./docs/plans`, `./docs/research`, or `./docs/internal` must use `docs(internal):` prefix. Implementation commits in Phase 1 use `feat(ui):` (for lib/ files) or `feat(build):` (for vite.config.ts, package.json, release.yml, dev-native.sh). The AGENTS.md update is `docs(internal):` even though AGENTS.md is not under `docs/`. Clarification: CLAUDE.md states the prefix applies to `docs/plans/**` changes — the AGENTS.md change may use `docs:` or `chore:` per team convention; confirm with team-lead before the Phase 1 PR.
