# Research: Classic (React) integration for `[ui].library_view_mode` (YAN-869 / M1-48)

Paths are relative to `src/crosshook-native/` unless noted. Worktree: `crosshook-yan-869`.

## 1. Current state (as-is)

### LibraryPage view mode is in `localStorage` only
- `src/components/pages/LibraryPage.tsx:30` `VIEW_MODE_KEY = 'crosshook.library.viewMode'`.
- `:32-35` `loadViewMode()` reads `localStorage` synchronously; anything other than `'list'` → `'grid'`.
- `:82` `useState<LibraryViewMode>(loadViewMode)` (lazy initializer, runs once per mount).
- `:131-135` `handleViewModeChange` → `setViewMode(mode)` + `localStorage.setItem(VIEW_MODE_KEY, mode)`.
- `:397-398` passed to `LibraryToolbar`; `:408` `viewMode === 'grid' ? <LibraryGrid> : <LibraryList>`.
- Toolbar buttons: `src/components/library/LibraryToolbar.tsx:85-101` (`aria-label="Grid view"` / `"List view"`, `aria-pressed`).
- Type: `src/types/library.ts:1` `LibraryViewMode = 'grid' | 'list'`.
- **This is the only `localStorage` consumer in `src/`** (grep). Every other web-storage use is `sessionStorage`
  (`useProfileNotifications.ts`, `useFlatpakMigrationToast.ts`, `useMetadataStoreStatus.ts`), which M1-48 leaves alone in Classic.
- Mount behaviour: `AppShell.tsx:79` default route is `'library'`; `ContentArea.tsx:76` renders `<LibraryPage>` inside
  `Tabs.Content key={route}`, so LibraryPage **remounts on every route switch** (re-reads state from wherever it lives).

### How React loads/saves settings today
- Single provider: `src/context/PreferencesContext.tsx`, mounted once in `AppShell.tsx:427`.
  - `:43-47` `loadPreferences()` = `settings_load` + `recent_files_load` + `default_steam_client_install_path`, **once at mount** (`:79-101`) or via `refreshPreferences`.
  - `:57` state starts as `DEFAULT_APP_SETTINGS` until the load resolves (async; there is no "loaded" flag in the context value `:12-23`).
  - `:103-112` `persistSettings(patch)` = `{ ...settings, ...patch }` from the **closure snapshot** → `settings_save { data: toSettingsSaveRequest(merged) }` → `settings_load` → `setSettings(loaded)`.
- Full-snapshot writers that bypass the context (read-modify-write from the frontend):
  - `src/hooks/profile/profileMetadataSync.ts:11-15,37-45` (`last_used_profile`; called from `useProfileCrud.ts:165,234,327`).
  - `src/hooks/useCommunityProfiles.ts:238-243` (`community_taps`).
- `toSettingsSaveRequest` (`src/types/settings.ts:85-127`) is an **explicit field list** → any key not listed is never sent.
- Rust side: `src-tauri/src/commands/settings.rs:126-131` `settings_load` → `AppSettingsIpcData::from_snapshot` (`:69-120`, calls `data.clear_extra()` `:76`);
  `:136-146` `settings_save` → `SettingsService::save_request`; `:150-160` `settings_save_steamgriddb_key` (narrow, write-only precedent).
  Registered in `src-tauri/src/lib.rs:131-135`; name-pinning test `settings.rs:183`.
- Service: `crates/crosshook-app/src/services/settings.rs`
  - `:45-90` `SettingsSaveRequest`: most fields **required** (overwrite), newer ones `Option` (absent = preserve).
  - `:109-174` `merge_save_request` builds `AppSettingsData { … }` field by field (no `..rest`), then `preserve_extra_from(&disk)` + `normalize`.
  - `:244-260` `update(mutate)` → `SettingsStore::update` (load-mutate-save under process-local `io_lock`, `crates/crosshook-core/src/settings/store.rs:139-153`, atomic write `:91-97`).
- Other narrow settings mutators already exist as dedicated commands: `dismiss_onboarding`, `dismiss_umu_install_nag`, `dismiss_steam_deck_caveats` (`src-tauri/src/commands/onboarding.rs:245-290`, `store.update(|s| s.field = …)`). Good precedent for a narrow typed command.

## 2. Lost-update analysis (why NOT to route view mode through `settings_save`)

1. **Stale `last_used_profile` clobber (pre-existing, would become frequent).** `profileMetadataSync` writes `last_used_profile` to disk without refreshing `PreferencesContext`. Any later `persistSettings(...)` sends the context's stale snapshot, and `merge_save_request:113` overwrites `last_used_profile` (required field). Today only Settings-page edits trigger this. Routing every Library toggle through `persistSettings` would trigger it on routine browsing.
2. **Stale closure.** `persistSettings` merges over the `settings` captured at render (`:105`); two quick calls each send a full snapshot; last writer wins for *all* required fields.
3. **Native-only `[ui]` keys.** If `ui` were added to `SettingsSaveRequest`, Classic would have to send a whole `ui` object. Classic never knows `window_width`, `renderer`, `palette_recent`, … → serde `#[serde(default)]` would fill defaults and **wipe native preferences** every time Classic saves anything. Even as `Option<UiSettings>` it is a footgun for any future caller that passes `ui`.
4. **Two binaries.** There is no cross-process lock yet (grep for `flock|instance_lock` in `crates/` → none; planned as M1-12, `docs/research/native-ui-migration/README.md:288,554`). `SettingsStore::update` re-reads from disk inside the lock, so a narrow `update` closure only touches its own key; a full-snapshot save does not have that property.

Conclusion: **reads via `settings_load`, writes via a dedicated narrow command that calls `SettingsService::update`.** `settings_save` must *preserve* `ui` from disk.

## 3. Recommended minimal safe contract

### Rust
- `crosshook-core/src/settings/types.rs`: `UiSettings` (owned by the core researcher) with `#[serde(default, flatten)] extra: toml::Table`, like `ConfigHistorySettings` (`types.rs:51-57`).
  - **Must update `clear_extra` (`types.rs:368-378`) and `preserve_extra_from` (`:381-…`) for `ui.extra`.** Otherwise `merge_save_request:171` clears the unknown `[ui]` sub-keys and never restores them (M1-11 regression).
  - `LibraryViewMode { Grid (default), List }`, `#[serde(rename_all = "lowercase")]`. Unknown strings are clamped to the default on load (spec) but **rejected** at the IPC boundary (typed command argument).
- `merge_save_request` (`app/services/settings.rs:111`): add `ui: current.ui` (the compiler forces a decision because the struct literal has no `..`). Do **not** add `ui` to `SettingsSaveRequest`.
- `SettingsService` typed setter, e.g. `pub async fn set_library_view_mode(&self, mode: LibraryViewMode) -> Result<(), AppError>` via `self.update(move |s| s.ui.library_view_mode = mode)`. The native UI (M2-01) calls the same setter, so both UIs share one code path.
  - Debounce interplay (spec: ≤1 write/s): view mode is a discrete click, not a resize stream. Either write it immediately, or make the debouncer **flush on shutdown/drop** and keep the pending value readable by `load()`. If not, a Classic quit within 1 s loses the toggle, and a `settings_load` right after a toggle returns the old value.
- Tauri: `src-tauri/src/commands/settings.rs`
  ```rust
  #[tauri::command]
  pub async fn settings_set_library_view_mode(mode: LibraryViewMode, service: State<'_, SettingsService>) -> Result<(), String>
  ```
  Register in `lib.rs` next to `:133-135`, and add it to the name-pin test `:183`.
  - Name: `settings_set_library_view_mode` (narrow, self-describing). The other names fail for these reasons:
    - A generic `settings_ui_update(patch)` would expose native-only keys to a frozen Classic (feature freeze from alpha.2, README R6) for no consumer.
    - `settings_save_*` collides semantically with the full-form save.
  - The name does not match the mock read heuristics (`wrapHandler.ts:86-87`), so `?errors=true` correctly treats it as a mutation.
- IPC read DTO `AppSettingsIpcData` (`settings.rs:23-66`): add `pub ui: UiSettingsIpcData` with **only** `library_view_mode`. That is all Classic consumes. A hand-written TS mirror of every native key (no ts-rs for settings; ts-rs is opt-in in `crosshook-core/Cargo.toml:10-11,54` and not on settings types) would be dead surface and drift risk. If the team prefers exposing full `UiSettings`, TS must then mirror all keys, and nothing changes on the write side.

### TypeScript
- `src/types/settings.ts`: add `ui: { library_view_mode: LibraryViewMode }` to `AppSettingsData` (`:72`), **not** to `SettingsSaveRequest` (`:25`). Add `ui: { library_view_mode: 'grid' }` to `DEFAULT_APP_SETTINGS` (`:139`). `toSettingsSaveRequest` stays as is, because its explicit list already omits `ui`. Add a unit assertion for that (see §6).
- `PreferencesContext.tsx`: add `setLibraryViewMode(mode)` to the value (`:12-23`, `:163-185`):
  - optimistic `setSettings(prev => ({ ...prev, ui: { ...prev.ui, library_view_mode: mode } }))` (functional update, no stale closure);
  - then `callCommand('settings_set_library_view_mode', { mode })`. On failure, keep the in-memory value (spec: "in-memory values still apply for the session") and set `settingsError` once / `console.error`. Classic has no global toast system: `AppShell.tsx:173-177,585-610` holds ad-hoc toasts, and `settingsError` renders only on SettingsPage.
  - No `settings_load` re-read after the write, so it cannot race a debounced backend.
- `LibraryPage.tsx`: delete `VIEW_MODE_KEY`, `loadViewMode`, the `useState` (`:30-35,82`) and the `localStorage.setItem` (`:134`). Derive `const viewMode = settings.ui.library_view_mode` from `usePreferencesContext()`, and make `handleViewModeChange = setLibraryViewMode`. Classic never writes or deletes the localStorage key (spec: "read once for import, never written"). Leaving it intact keeps 0.6.x downgrades sane.

### First-paint flash (risk to decide)
`localStorage` was synchronous; the context starts at `DEFAULT_APP_SETTINGS` (`:57`). A user with `list` will see grid → list on cold start, because Library is the default route. Options:
- (a) Accept it. `settings_load` is a local file read and resolves alongside `profile_list_summaries`, which LibraryPage already waits on (`useLibrarySummaries`, `:65-70`).
- (b) Expose `settingsLoaded: boolean` from the context and hold the grid/list body (not the toolbar) until it is true.

Recommend (b) only if (a) is visibly janky in `?delay=300` browser mode. Don't add a speculative flag.

### localStorage import: who does it
Classic cannot do the import correctly. The IPC DTO always carries a value (default `grid`), so the frontend can't tell "absent" from "grid". Keep the import in Rust, as the spec says.
- The import must run in a **shared path used by both binaries** (the `crosshook-app` settings service/bootstrap) **before Classic's first `settings_load`**. Otherwise a Classic user upgrading to 0.7 alpha silently reverts to grid.
- Absence detection needs the raw TOML. `#[serde(default)]` hides absence; use `Option` internally or check `ui.library_view_mode` in the parsed table.
- Non-Flatpak Tauri path: `[INFERENCE]` `~/.local/share/dev.crosshook.CrossHook/localstorage/tauri_localhost_0.localstorage`. The identifier `dev.crosshook.CrossHook` is confirmed in `src-tauri/tauri.conf.json:4`. The origin-based file name is unverified, so check it on a real install.

## 4. Browser-dev mocks + coverage gate
- `scripts/check-mock-coverage.sh` (repo root):
  - `:67-71` extracts every `#[tauri::command] pub (async )?fn name` from `src-tauri/src` and `crates/crosshook-core/src`.
  - `:89-94` extracts `map.set('name'` from `src/lib/mocks/handlers/`, excluding `_mock_*`.
  - `:101,108,138-139` exits 1 on any missing **or** orphaned handler.
  - So the new command **requires** a mock in the same PR (`npm run dev:browser:check`, `package.json:12`).
- Add to `src/lib/mocks/handlers/settings.ts` (after `:26`):
  `map.set('settings_set_library_view_mode', async (args) => { const { mode } = args as { mode: string }; if (mode !== 'grid' && mode !== 'list') throw new Error('[dev-mock] invalid library view mode: ' + mode); const s = getStore().settings; getStore().settings = { ...s, ui: { ...s.ui, library_view_mode: mode } }; return null; });`
  - Replace the object immutably. `store.ts:27` seeds with a **shallow** `{ ...DEFAULT_APP_SETTINGS }`, so mutating `store.settings.ui.x = …` in place would mutate the shared `DEFAULT_APP_SETTINGS.ui` and leak across `resetStore()`/tests. Alternatively, switch `store.ts:27` to `structuredClone(DEFAULT_APP_SETTINGS)`; `config_history` has the same latent hazard.
  - The `settings_save` mock (`:13-26`) merges `{ ...store.settings, ...next }`. Because `toSettingsSaveRequest` omits `ui`, the mock preserves it like the backend does, so no change is needed.
- `settings_load` is shell-critical (`wrapHandler.ts:30-37`). Adding `ui` to it changes nothing there.
- Update `src/lib/mocks/README.md` only if it enumerates settings commands. Its tree line `handlers/settings.ts — settings_* and recent_files_*` still holds.

## 5. Tests that touch this area today

**Vitest**
- `src/components/pages/__tests__/LibraryPage.test.tsx:111-126`: stubs `localStorage` for LibraryPage. `:146-155` clicks "List view" (still valid; it now goes through the mock command).
- `localStorage` stubs that exist *only* because LibraryPage read it. After cutover these become dead scaffolding; remove them in the same PR (clean cutover), then run the suite to confirm nothing else needs them:
  - `src/__tests__/a11y/routes.a11y.test.tsx:106-127,155-176,200-220`
  - `src/__tests__/a11y/skip-link.a11y.test.tsx:44-60`
  - `src/components/__tests__/LaunchSubTabs.test.tsx:171-186`
  - `src/components/layout/__tests__/AppShell.test.tsx:75-90` and `AppShell.startup.test.tsx:39-54`
  - `src/components/pages/__tests__/DashboardRoutes.test.tsx:60-75`
  - `LibraryPage.test.tsx:111-126`
- **Breakage risk: partial `settings_load` overrides without `ui`.**
  - `routes.a11y.test.tsx:56-64` (`EMPTY_PROFILE_OVERRIDES`) renders LibraryPage (`:92`, `:185`). `settings.ui.library_view_mode` would throw `TypeError`, so add `ui: { library_view_mode: 'grid' }` there.
  - `src/context/__tests__/ProfileContext.test.tsx:14,29` (`baseSettings`) does not render LibraryPage. Update it anyway for shape fidelity.
  - Do **not** paper over this with `settings.ui?.… ?? 'grid'` in production code.
- `src/types/__tests__/settings.test.ts:1-9` is the natural home for the "save request never carries `ui`" assertion.
- `src/components/library/__tests__/LibraryToolbar.test.tsx:10-11,84-87` is prop-driven and unaffected.

**Playwright** (`tests/smoke.spec.ts`; `npm run test:smoke`, `package.json:19`)
- No current test touches grid/list. Library tests: `:141-231` (inspector/hero), `:233-269` (quick filters), `:504-524`, `:712-746`.
- Helpers call real/`_mock_` commands via `window.__CROSSHOOK_DEV__.callCommand` (`tests/navigation-helpers.ts:46-77`).
- The mock store is a per-page-load singleton, so cross-reload persistence can't be smoke-tested in browser mode. The persistence direction belongs in Vitest/Rust.

## 6. Recommended test additions (Classic side)

**Vitest: `LibraryPage.test.tsx`**
1. "renders list when settings carry `ui.library_view_mode = 'list'`". Use a `handlerOverrides.settings_load` that returns `{ ...DEFAULT_APP_SETTINGS, ui: { library_view_mode: 'list' } }`, which simulates a native-written value. Assert that "List view" has `aria-pressed="true"` and list rows render.
2. "toggling List persists via `settings_set_library_view_mode` and does not touch localStorage". Spy on the handler, or use `seed` to wrap it. Assert `{ mode: 'list' }`, mock store updated, and `localStorage.getItem('crosshook.library.viewMode') === null`.
3. Optional: when the command rejects, the view stays `list` for the session (spec degraded behaviour) and an error surfaces once.

**Vitest: `types/__tests__/settings.test.ts`**
- `'ui' in toSettingsSaveRequest({...DEFAULT_APP_SETTINGS, ui:{library_view_mode:'list'}})` is `false`.

**Playwright smoke** (new `test.describe('library view mode')`), at 1920×1080 with `?fixture=populated`:
1. Click "List view" and assert `aria-pressed`.
2. Go to the Settings tab and back to Library (remount); "List view" is still pressed.
3. `page.evaluate(() => localStorage.getItem('crosshook.library.viewMode'))` returns `null`.
4. No console errors (`attachConsoleCapture`).

**Rust: the "both directions" AC, the test that makes it real today**
- `crosshook-app` settings tests (`services/settings/tests.rs`):
  - (a) `set_library_view_mode(List)`, then a fresh `SettingsService` on the same dir `load()` returns `List` (Classic write → native read path).
  - (b) After `set_library_view_mode(List)`, a `save_request` with an ordinary form payload keeps `ui.library_view_mode == List` and keeps unknown `[ui]` keys (lost-update guard).
- `src-tauri` `settings.rs` tests: a raw TOML `[ui]\nlibrary_view_mode = "list"` → `AppSettingsIpcData::from_snapshot(...).ui.library_view_mode == List` (native write → Classic read). Also check that the serialized JSON contains `"library_view_mode":"list"` and **not** `[ui]` unknown keys.

## 7. "Changing the view mode in Classic is visible in native and vice versa": what it can mean today
- **Verified:** the native UI does not exist.
  - `Cargo.toml` workspace members are `crosshook-app`, `crosshook-core`, `crosshook-cli` and `src-tauri` only.
  - No `crates/crosshook-ui` directory.
  - `crosshook-ui` appears only in plans/ADR (`docs/research/native-ui-migration/README.md:378`, `docs/architecture/adr-0001…:117`) and in forward-looking gateway scan lists (`scripts/check-host-gateway.sh:15`, `scripts/tests/check-host-gateway/run.sh:13`).
  - The native Library port is M2-01 (YAN-854), which this issue blocks.
- So the AC can only be proven at the **shared contract** level: both UIs read and write `settings.toml [ui].library_view_mode` through the same `crosshook-app` `SettingsService` getter/setter. Classic goes through Tauri delegations; native will call the service directly (README:388 "crosshook-ui never calls crosshook-core directly").
  - Evidence = the Rust round-trip tests in §6 plus the Vitest test #1 (externally written value → Classic renders it).
  - State in the PR that the end-to-end native half is verified by M2-01, and M6-03 (YAN-903) covers cross-UI compat.
- "Visible" means **on next launch / next `settings_load`**, not live. Classic loads preferences once at mount (`PreferencesContext.tsx:79-101`), there is no `settings-changed` event (grep → none), and concurrent runs will be prevented by the M1-12 instance lock. Live sync is out of scope; don't add a file watcher.

## 8. Risks / gotchas checklist
- `ui.extra` must be wired into `clear_extra` and `preserve_extra_from`, or M1-11 preservation regresses for unknown `[ui]` keys.
- `merge_save_request` must take `ui` from `current`. Never put `ui` in `SettingsSaveRequest`.
- Pre-existing `last_used_profile` stale-snapshot clobber via `persistSettings` exists today; the narrow command avoids amplifying it. Flag it, but don't fix it here (out of scope).
- Mock store shallow-copy aliasing of nested `ui` (`store.ts:27`).
- Partial `settings_load` fixtures in tests lack `ui` → crash. Update the fixtures; no optional chaining in prod.
- Debounced service writes vs. Classic quit and read-after-write. Flush on shutdown, or write the view mode immediately.
- The import must run for Classic too, before the first `settings_load`. Otherwise upgraders lose `list`.
- Docs AC: no user-facing settings reference exists under `docs/` (no `settings.toml` key reference found in `docs/features` or `docs/getting-started`), so this PR likely has to **create** one. Coordinate with the docs owner.
- File-size bar (~500 lines, `CLAUDE.md:50`): `LibraryPage.tsx` is 485 lines and shrinks; `src-tauri/.../settings.rs` is 358 + ~25; `app/services/settings.rs` is ~300 + ~15. All OK.

## Key files

| File | Role |
| --- | --- |
| `src/components/pages/LibraryPage.tsx:30-35,82,131-135` | localStorage read/write to remove |
| `src/context/PreferencesContext.tsx:57,103-112,163-185` | add `setLibraryViewMode`, the read source |
| `src/types/settings.ts:25,72,85,139` | `AppSettingsData.ui`, defaults; save request unchanged |
| `src/lib/mocks/handlers/settings.ts:10-26`, `store.ts:27` | new mock handler, aliasing fix |
| `scripts/check-mock-coverage.sh:67-139` | gate: mock must exist for the new command |
| `src-tauri/src/commands/settings.rs:23-66,136-160,183`, `lib.rs:131-135` | DTO `ui`, new command, registration, name pin |
| `crates/crosshook-app/src/services/settings.rs:45-90,109-174,244-260` | no `ui` in request; `ui: current.ui`; typed setter via `update` |
| `crates/crosshook-core/src/settings/types.rs:51-57,368-…` | `UiSettings.extra` + extra helpers |
| `src/components/pages/__tests__/LibraryPage.test.tsx`, `src/__tests__/a11y/routes.a11y.test.tsx:56`, `tests/smoke.spec.ts` | test updates/additions |
