# Research: docs & UX for M1-48 `[ui]` settings (YAN-869 / #537)

Paths are relative to the worktree root `crosshook-yan-869/`. `native/` = `src/crosshook-native/`.

## 1. Docs: where the "settings reference" lives today

**There is no settings reference doc yet.** Nothing in `docs/` lists the `settings.toml` keys:

- I searched for `settings reference`, `configuration reference` and `## Settings`. The only hits are in plans and research.
- The user-facing docs only say "set X in Settings":
  - `docs/getting-started/quickstart.md:121,321,388`
  - `docs/features/steam-proton-trainer-launch.doc.md:148,353,385,419-420`
- Key names only appear in internal plans, for example `docs/plans/profile-writer/feature-spec.md:140,191` and `docs/prps/...`.

So the acceptance item "The docs settings reference is updated" means creating the reference page in this PR.

Existing user-doc layout and conventions:

| Area | Files | Format |
| -- | -- | -- |
| Getting started | `docs/getting-started/quickstart.md` (402 lines; numbered TOC at 7-43; "Related Guides" at 398-402) | `#` title, `##` sections, tables, fenced `toml` blocks (e.g. the profile example at 133-157) |
| Feature docs | `docs/features/*.doc.md` (`library-context-rail.doc.md`, `console-status-bar.doc.md`, ...) | `# Title` → `## When it appears` / `## User flow` / `## Implementation files` (table) / `## Limitations` |
| Index links | `README.md:49-52` and `README.md:151-154` ("Docs" list) | Bulleted relative links |
| Data paths (authoritative wording) | `quickstart.md:106-110,133` | Flatpak live config: `~/.var/app/dev.crosshook.CrossHook/config/crosshook/`. The legacy/AppImage import source is `~/.config/crosshook/`. |

Formatting:
- `.prettierrc` sets markdown `printWidth: 120` and `proseWrap: "preserve"`.
- `.prettierignore` only excludes `src/crosshook-native/src/`, so `docs/**` is formatted by `scripts/format.sh` (prettier). CI autofix runs this in `.github/workflows/lint-autofix.yml:43-44`.
- Tables must be prettier-aligned.

### Recommendation: where to document `[ui]`

1. **Create `docs/features/settings-reference.doc.md`.** It is user-facing, so it does not go under `docs/plans|research|internal` and does not need the `docs(internal)` prefix (`CLAUDE.md:42`). The `feat(app): …` PR title covers it. Contents:
   - **File location:**
     - Flatpak: `~/.var/app/dev.crosshook.CrossHook/config/crosshook/settings.toml`
     - Legacy/AppImage: `~/.config/crosshook/settings.toml`
     - Use the same wording as `quickstart.md:108`.
   - **Editing rules:**
     - Edit the file while CrossHook is closed.
     - Unknown keys are preserved (M1-11).
     - Invalid values are clamped or reset to the default, with a warning in the log.
   - **`## [ui]` table** with columns Key | Type / allowed values | Default | Range / clamp | Written by | Read by:
     - `library_view_mode`
     - `window_width`, `window_height`
     - `ui_scale`
     - `density`
     - `renderer` (read at bootstrap phase A, M1-13)
     - `controller_mode`
     - `sidebar_collapsed`
     - `inspector_width`, `console_drawer_height`
     - `palette_recent` (≤20, deduped, command ids only, never paths)
   - **Example block** (`toml`) with every `[ui]` key.
   - **Import note:** if `library_view_mode` is absent, it is imported once, read-only, from the old WebKit localStorage. That file is under `~/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/localstorage/` in Flatpak. Failures silently fall back to `grid`.
   - **Degraded note:** if the file is unwritable, values still apply for the session and one error toast is shown.
   - **Runtime-only list:** toast and banner dismissals are not persisted.
2. Add links to the new page:
   - in `quickstart.md` "Related Guides" (398-402);
   - in `README.md` docs list (151-154).
   - Optionally add a TOC entry if you add a short quickstart section. Not required.
3. **Scope:** the issue only requires the `[ui]` keys. Documenting all ~30 existing top-level keys (`native/src/types/settings.ts:24-70`) is extra scope. Keep the page structured so other sections can be added later, but write only `[ui]` plus file location and editing rules.

### Doc risks: key names disagree across sources (decide once, then document)

| Source | Scale | Console height | Controller | Renderer values | Density default |
| -- | -- | -- | -- | -- | -- |
| Issue spec (`/tmp/YAN-869.md`), authoritative | `ui_scale` 0.75–2.0, default 1.0 | `console_drawer_height` 120–800 | `auto`/`on`/`off` | `auto`/`wgpu`/`gl`/`tiny-skia` | not stated |
| `docs/research/native-ui-migration/README.md:544-548` | `scale` (default `auto`) | — | `auto`/`on`/`off` | `auto`/`wgpu`/`gl`/`software` | `auto` |
| M1-11 fixture `native/crates/crosshook-core/tests/unknown_settings_keys.rs:8-19` | `scale = 1.1` | `console_height = 220` | `controller_mode = true` (bool!) | `"auto"` | `"compact"` |

Effects:
- With the spec names, the fixture's `scale` and `console_height` become unknown keys inside `[ui]`.
- `controller_mode = true` is an invalid value.

The test asserts `saved["ui"] == original["ui"]` (`unknown_settings_keys.rs:56-71`) and byte-stable re-save (`:74-85`). So `UiSettings` must carry its own `#[serde(default, flatten)] extra: toml::Table`, following `ConfigHistorySettings` (`native/crates/crosshook-core/src/settings/types.rs:51-57`). `clear_extra`/`preserve_extra_from` (`types.rs:368-390`) must cover `ui.extra` as well. Otherwise the fixture must be edited and the downgrade promise weakens.

Two more fixtures reference `[ui] scale`:
- `native/crates/crosshook-app/src/services/settings/tests.rs:91`
- `unknown_settings_keys.rs:124,140`

Line 140 must still fail. It fails because of `config_history`, so that is fine.

There is also a policy conflict to note in the decisions doc:
- The spec's "invalid → clamp/reset + warn" for `[ui]` contradicts the M1-11 "malformed known fields fail" rule (`unknown_settings_keys.rs:133-146`).
- So `[ui]` fields need lenient per-field deserialization (the `HighContrastPreference` custom `Deserialize` at `types.rs:182-199` is the in-repo pattern).
- That leniency should not leak to top-level keys.

The README §10 also says "Classic reads `localStorage` once and writes TOML". The issue overrides this: Rust reads the WebKit SQLite file read-only. Follow the issue and mention the difference in the plan.

## 2. CHANGELOG must not be edited

- `CHANGELOG.md:5`: "This file is generated with `git-cliff`…". The footer is `<!-- generated by git-cliff -->`.
- `CLAUDE.md:39-41` and `RELEASING.md:68-75`: `scripts/prepare-release.sh` regenerates it at tag time from the squash-merge PR title.
- The changelog entry is therefore the PR title. Example: `feat(app): add [ui] settings table for layout and display preferences` (type `feat` → minor bump).
- **Do not touch `CHANGELOG.md`.** The PR template's "Documentation" checkbox (`.github/pull_request_template.md:17`) should be ticked.

## 3. UX today: how Classic surfaces settings save errors

### 3.1 Shared save path: `PreferencesContext`

`native/src/context/PreferencesContext.tsx`:
- **`persistSettings` (103-112):** builds `{...settings, ...patch}`, then calls `settings_save` and `settings_load`, then sets `settings` and clears `settingsError`.
  - It has **no try/catch**. A failure rejects and leaves `settings` unchanged.
  - The `setSettings` call after a successful save is the only update, so the UI is *pessimistic*. The control snaps back to the old value on the next render.
- **Only these three wrappers set `settingsError`:** `handleAutoLoadChange` (114-127), `handleSteamGridDbApiKeyChange` (129-145) and `clearRecentFiles` (147-162).
- **Every section call is `void onPersistSettings(...)`:**
  - `components/settings/AccessibilitySection.tsx:62,73`
  - `LoggingAndUiSection.tsx:29,48,78`
  - `RunnerSection.tsx:30,43`
  - `NewProfileDefaultsSection.tsx:35,48,64,83`
  - `PrefixDependenciesSection.tsx:59,69,97`
  - `ProfilesSection.tsx:45,109`
  - `ProtonManagerDefaultsSection.tsx:126,151,174`
  - `TrainerDiscoveryPanel.tsx:93-97` (`.then` without `.catch`)

  On failure these are **unhandled promise rejections**: there is no banner, no toast and no global `unhandledrejection` handler (grep finds none). The failure is **silent**.
- **`settingsError` display:** shown only on the Settings route, as an inline `role="alert"` banner (`components/pages/SettingsPage.tsx:41-45`). It is invisible from Library.
- **Error text:** comes from `AppError::message()` via `map_app_error` (`native/src-tauri/src/commands/settings.rs:13-15,131-142`).
  - Store I/O errors are sanitized to the io kind only, e.g. `permission denied` (`native/crates/crosshook-app/src/error.rs:156`).
  - That is safe to show but too terse to show alone, so wrap it in copy.

### 3.2 Library view toggle today

`native/src/components/pages/LibraryPage.tsx`:
- 30-35: `VIEW_MODE_KEY = 'crosshook.library.viewMode'`. `loadViewMode()` reads `localStorage` synchronously; anything but `'list'` becomes `'grid'`.
- 82: `useState(loadViewMode)`.
- 131-135: `handleViewModeChange` calls `setViewMode(mode)` **first** (optimistic, in-memory), then `localStorage.setItem` with no try/catch. A throwing storage (quota or security) surfaces as an uncaught event-handler error. The view still switches, nothing is shown to the user, and the value is lost on restart.
- 397-398 and 408: wired to `LibraryToolbar` (`components/library/LibraryToolbar.tsx:84-101`, `aria-pressed` buttons "Grid view"/"List view").

`LibraryPage.tsx` is the only non-test `localStorage` user in `native/src`.

### 3.3 Existing toast infrastructure

There is no global toast service. Toasts are ad-hoc state in `components/layout/AppShell.tsx`:
- 173-177: `collectionDescriptionToast` and `navigationToast` state.
- 277: Flatpak migration toast hook.
- 585-630: markup. The shared classes are `crosshook-status-toast crosshook-rename-toast` with `role="status" aria-live="polite"`, plus a `crosshook-rename-toast-dismiss` "×" button with `aria-label="Dismiss"`.
- Styles: `native/src/styles/theme.css:4369-4405` (fixed bottom-center, slide-in animation).
- `PreferencesProvider` wraps the whole shell (`AppShell.tsx:427-636`), so a toast can render from context state anywhere.

## 4. UX recommendation for the degraded behaviour

Spec: "settings file unwritable → in-memory values still apply for the session and a single error toast".

1. **Optimistic in-memory first.** The view-mode toggle (and any Classic `[ui]` setter) updates React state immediately, as `LibraryPage.tsx:133` already does. Persistence happens afterwards, and a failure **never reverts** the visible value. This is the reverse of `persistSettings`' pessimistic flow, so do not route `[ui]` writes through `persistSettings` unchanged.
2. **Use a narrow write path, not the full-snapshot `settings_save`.**
   - `persistSettings` sends the whole `AppSettingsData` snapshot from React state (`PreferencesContext.tsx:105`).
   - `merge_save_request` takes every non-`Option` field from the request (`native/crates/crosshook-app/src/services/settings.rs:109-174`).
   - If `settings_load` failed at startup (state is still `DEFAULT_APP_SETTINGS`, `PreferencesContext.tsx:57,86-90`), toggling the view mode would **overwrite `settings.toml` with defaults**.
   - Prefer a dedicated IPC, e.g. `settings_ui_update(patch)` that wraps the service's typed setter, which uses `SettingsStore::update` (`native/crates/crosshook-core/src/settings/store.rs:136-152`).
   - If `ui` is instead added to `SettingsSaveRequest`, make it `Option<UiSettings>` with absent meaning preserve (like `config_history`, `settings.rs:92-93,166`) and **omit** it in `toSettingsSaveRequest` (`native/src/types/settings.ts:85-128`), so other Settings sections never clobber `[ui]`.
   - A new command requires:
     - a mock in `native/src/lib/mocks/handlers/settings.ts` (enforced by `scripts/check-mock-coverage.sh:100-140`, which exits 1 on drift);
     - an update to the IPC-name contract test (`native/src-tauri/src/commands/settings.rs` `command_names_match_expected_ipc_contract`, around line 176).
3. **Read path.**
   - Add `ui` (at minimum `library_view_mode`) to `AppSettingsIpcData` (`native/src-tauri/src/commands/settings.rs:21-67`, `from_snapshot` 69-113) and to the TS `AppSettingsData` (`types/settings.ts:72-83`) plus `DEFAULT_APP_SETTINGS`.
   - `LibraryPage` reads `settings.ui.library_view_mode` from `usePreferencesContext()`. To avoid a grid→list flash while settings load, keep a local optimistic state that is seeded once settings arrive.
4. **"Single error toast."**
   - Hold a runtime-only `uiSaveErrorShown`/`uiSaveError` in `PreferencesContext`. It is never persisted, which matches the storage table.
   - Render one toast in `AppShell` next to `navigationToast` (`AppShell.tsx:605-617`), reusing `crosshook-status-toast` and the dismiss button.
   - Show it **at most once per session**: later failures (including debounced retries) only log.
   - Suggested copy: *"Couldn't save display preferences to settings.toml ({reason}). Your changes apply until CrossHook closes."*
   - Keep `role="status"` for consistency. An error variant could use `role="alert"`, but that would be a new pattern, so justify it if you choose it.
5. **Mocks.**
   - `handlers/settings.ts:13-26` shallow-merges and special-cases `config_history`. Add the same nested merge for `ui`, or add the new command's handler.
   - `lib/mocks/store.ts:26` seeds from `DEFAULT_APP_SETTINGS`, so add `ui` defaults there.
   - Consider a `handlerOverrides` rejection test for the toast.
6. **Cleanup after the cutover (Classic stops using `localStorage`).** Remove `VIEW_MODE_KEY`/`loadViewMode` and the `localStorage` stubs that existed only for LibraryPage:
   - `native/src/components/pages/__tests__/LibraryPage.test.tsx:111-127`
   - `__tests__/a11y/routes.a11y.test.tsx:106-109,155-158,202-203`
   - `__tests__/a11y/skip-link.a11y.test.tsx:44-45`
   - `components/__tests__/LaunchSubTabs.test.tsx:171-172`
   - `components/layout/__tests__/AppShell.startup.test.tsx:39-40`
   - `components/layout/__tests__/AppShell.test.tsx:75-76`
   - `components/pages/__tests__/DashboardRoutes.test.tsx:60-61`

   Check each for other `localStorage` uses first. Grep shows none in non-test code.

   The Classic `sessionStorage` dismissals (`useFlatpakMigrationToast.ts:4-5`, `profileNotificationConstants.ts:6-7`, `useMetadataStoreStatus.ts:5-8`) stay as they are in Classic. "Runtime-only" applies to the native UI.

## 5. Tests to add or adjust (UX side)

- **LibraryPage view toggle:**
  - persists through the new command;
  - initial mode comes from `settings_load`;
  - on a rejected save, the list view stays active and the toast appears once;
  - a second failure produces no second toast.
- **Cross-UI acceptance ("Classic ↔ native"):** a Rust-level round-trip test (write via the service, read via IPC DTO) plus a Vitest test that `settings_load` returning `ui.library_view_mode = 'list'` renders the list. `npm run test:smoke` covers browser-dev.
- **Unknown-key fixtures:** update `unknown_settings_keys.rs` per §1 risks.

## 6. Risks summary

1. **High:** a full-snapshot `settings_save` from stale or default React state can clobber `settings.toml`. Use a narrow `[ui]` update command.
2. **High:** `persistSettings` failures are silent today. The `[ui]` path must catch the rejection, keep the in-memory value and raise the once-per-session toast. Do not widen this to the other Settings sections; that is out of scope.
3. **Medium:** the spec, migration README and M1-11 fixture disagree on key names and types. Pin them in `decisions.md` and document only the final names.
4. **Medium:** M1-11 strict parsing of known fields versus the spec's clamp/reset for `[ui]`. Use lenient per-field deserialization scoped to `UiSettings`.
5. **Low:** grid→list flicker on first paint while settings load asynchronously, where `localStorage` was synchronous.
6. **Process:** do not edit `CHANGELOG.md`. Create the docs page under `docs/features/` (user-facing). Any plan docs under `docs/plans/` need `docs(internal):` commits.
