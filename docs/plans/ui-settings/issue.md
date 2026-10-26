_Paths are relative to_ `src/crosshook-native/`_._

## Context

This folds in the closed legacy requirement for persisted layout preferences: panel and inspector widths, sidebar collapsed state, and command-palette recency.

Today:

- The library view mode lives in WebKit `localStorage` (`crosshook.library.viewMode`, `src/components/pages/LibraryPage.tsx:29-32/127`). It would be lost on the port.
- Session dismissals live in `sessionStorage` (`useProfileNotifications.ts`, `useFlatpakMigrationToast.ts`).
- The native UI also needs renderer, scale, density and controller-mode preferences.

## Scope

- Add a `UiSettings` struct as a `[ui]` table in `crates/crosshook-core/src/settings/types.rs`, with `#[serde(default)]` on every field. The keys are validated and clamped on load:
  - `library_view_mode`: `grid` or `list`, default `grid`.
  - `window_width` and `window_height`: from 800x600 up to 16384, saved on resize end.
  - `ui_scale`: 0.75 to 2.0, default 1.0.
  - `density`: `compact`, `comfortable` or `spacious`.
  - `renderer`: `auto`, `wgpu`, `gl` or `tiny-skia` (read by M1-13 in bootstrap phase A).
  - `controller_mode`: `auto`, `on` or `off`.
  - `sidebar_collapsed`: bool.
  - `inspector_width` (240 to 640) and `console_drawer_height` (120 to 800).
  - `palette_recent`: a list of command ids, never paths, holding at most 20 and de-duplicated.
- The `crosshook-app` settings service (M1-07) exposes typed getters and setters. Writes are debounced to at most one per second and rely on M1-11's settings-store guarantees (atomic save and preserving unknown keys).
- **Best-effort import:** if `[ui].library_view_mode` is absent, read the WebKit localStorage SQLite file read-only. In Flatpak it is under `~/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/localstorage/`. Any failure falls back to the default without an error dialog.
- Classic switches `LibraryPage` from `localStorage` to `[ui].library_view_mode`, so both UIs stay consistent. The mocks are updated (M1-56).
- The old `sessionStorage` dismissals become **runtime-only** in-memory state in the native UI.

## Storage boundary

| Datum                                                       | Class                                          |
| ----------------------------------------------------------- | ---------------------------------------------- |
| all `[ui]` keys above                                       | TOML settings (`settings.toml`, user-editable) |
| WebKit `localStorage` view mode                             | read once for import, never written            |
| profile-notification and Flatpak-migration toast dismissals | runtime-only (memory)                          |
| SQLite metadata                                             | no change; schema version unchanged            |

## Persistence & usability

- **Migration and backward compatibility:** the keys are additive with defaults. Older builds ignore `[ui]`, and M1-11 ensures their saves don't drop it. The view mode is imported once.
- **Offline:** fully local.
- **Degraded behaviour:** an invalid value is clamped or reset to its default with a logged warning. If the settings file is unwritable, the in-memory values still apply for the session and a single error toast is shown.
- **User visibility and editing:** every key can be edited by hand in `settings.toml` and is documented in `docs/` settings reference. The renderer, scale, density and controller mode are also exposed in Settings when that screen is ported.

## Acceptance criteria

- [ ] Round-trip and clamp tests pass for every key, including `palette_recent` holding at most 20 with dedupe.
- [ ] The import test covers a fixture localStorage DB, a missing file and a corrupt file.
- [ ] Debounce test: 100 resize events produce at most 2 writes.
- [ ] Changing the view mode in Classic is visible in native and vice versa.
- [ ] The docs settings reference is updated.

## Test plan

- `cargo test -p crosshook-core settings`
- `cargo test -p crosshook-app settings`
- `npm run test:smoke`

---

**Plan key:** `M1-48` · **Release:** v0.7.0-alpha.1 · **Estimate:** 3 pts · **GitHub labels:** `type:feature`, `area:profiles`, `priority:high` · **Blocked by:** M1-07, M1-11

REL:{"blocks": [{"id": "YAN-853", "title": "M3-06 \u00b7 feat(ui): port Settings page with enumerated sections, UI scale and renderer options"}, {"id": "YAN-841", "title": "M1-44 \u00b7 feat(widgets): add selectable text, skeleton loaders and a reduced-motion animation layer"}, {"id": "YAN-805", "title": "M1-19 \u00b7 feat(ui): build navigation shell with routes, sidebar, breakpoints, inspector and console drawer"}, {"id": "YAN-840", "title": "M4-10 \u00b7 feat(ui): port command palette, global shortcuts and app-wide notifications"}, {"id": "YAN-903", "title": "M6-03 \u00b7 test(migration): verify settings and metadata compatibility across Classic, native and downgrades"}, {"id": "YAN-854", "title": "M2-01 \u00b7 feat(ui): port Library page with grid/list, search, sort, filter chips and favorites"}], "blockedBy": [{"id": "YAN-825", "title": "M1-07 \u00b7 refactor(app): move settings, catalog, recent files, diagnostics and background portal services"}, {"id": "YAN-868", "title": "M1-11 \u00b7 fix(core): preserve unknown settings keys and write settings atomically"}], "relatedTo": [], "duplicateOf": null}
ATT:[]
BR:yramirez/yan-869-m1-48-featapp-add-ui-settings-table-for-layout-and-display
URL:https://linear.app/yandy-r/issue/YAN-869/m1-48-featapp-add-ui-settings-table-for-layout-and-display-preferences
