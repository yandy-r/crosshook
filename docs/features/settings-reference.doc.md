# Settings reference

CrossHook stores user settings as TOML in `settings.toml`. This page documents
where the file lives, how to edit it by hand, and every key of the `[ui]`
table (layout and display preferences).

## File location

- **Flatpak (current release), live config:**
  `~/.var/app/dev.crosshook.CrossHook/config/crosshook/settings.toml`
- **Legacy/AppImage:** `~/.config/crosshook/settings.toml` (imported into the
  Flatpak sandbox on first launch; not the live path after migration).

## Editing rules

- Edit the file **while CrossHook is closed**. The application writes settings
  on change and on exit, so edits made while it is running can be overwritten.
- Unknown keys are preserved verbatim, including unknown keys inside `[ui]`,
  so newer or older builds do not lose each other's settings.
- Loading `[ui]` never fails: a key with the wrong type or an invalid value
  is reset to its default, and a number outside its range is clamped. Each
  fix-up logs one warning naming the key and the reason (never the value).
- Saves are atomic (temporary file plus rename).

## [ui]

The `[ui]` table holds layout and display preferences shared by the Classic
(Tauri/WebKit) UI and the upcoming native UI.

| Key                     | Type or values                       | Default            | Range          | Used by                                                 |
| ----------------------- | ------------------------------------ | ------------------ | -------------- | ------------------------------------------------------- |
| `library_view_mode`     | `grid`, `list`                       | `grid` when absent | –              | Classic Library toggle; native UI library               |
| `window_width`          | integer (logical pixels)             | absent = centred   | 800–16384      | Native shell                                            |
| `window_height`         | integer (logical pixels)             | absent = centred   | 600–16384      | Native shell                                            |
| `ui_scale`              | float                                | `1.0`              | 0.75–2.0       | Native UI; covers gamescope at scale 1.0 and docked TVs |
| `density`               | `compact`, `comfortable`, `spacious` | `comfortable`      | –              | Native UI spacing                                       |
| `renderer`              | `auto`, `wgpu`, `gl`, `tiny-skia`    | `auto`             | –              | Native UI backend; `tiny-skia` is the software fallback |
| `controller_mode`       | `auto`, `on`, `off`                  | `auto`             | –              | Native UI controller navigation                         |
| `sidebar_collapsed`     | boolean                              | `false`            | –              | Native UI sidebar                                       |
| `inspector_width`       | integer (logical pixels)             | `320`              | 240–640        | Native UI inspector pane                                |
| `console_drawer_height` | integer (logical pixels)             | `280`              | 120–800        | Native UI console drawer                                |
| `palette_recent`        | array of command ids                 | `[]`               | at most 20 ids | Native UI command palette; move-to-front, deduplicated  |

Notes:

- `library_view_mode`, `window_width` and `window_height` are optional: an
  absent key means “not set” and is not written back until a value exists.
  The window size is saved on resize end by the native shell, not by Classic.
- `palette_recent` ids must match
  `^[a-z][a-z0-9-]{0,31}(:[a-z0-9][a-z0-9-]{0,31}){0,2}$` (at most 98 bytes);
  invalid entries are dropped with a warning that gives only the count.

### Example

```toml
[ui]
library_view_mode = "list"
window_width = 1280
window_height = 800
ui_scale = 1.25
density = "compact"
renderer = "auto"
controller_mode = "auto"
sidebar_collapsed = false
inspector_width = 320
console_drawer_height = 280
palette_recent = ["route:library", "settings:ui:density"]
```

## Import from the Classic WebKit storage

On every launch that finds no `library_view_mode` in `[ui]`, CrossHook
attempts to read the old Classic preference from the WebKit `localStorage`
database (`tauri_localhost_0.localstorage`, key `crosshook.library.viewMode`):

- **Flatpak:** `~/.var/app/dev.crosshook.CrossHook/data/dev.crosshook.CrossHook/localstorage/`
- **Legacy/AppImage:** `~/.local/share/dev.crosshook.CrossHook/localstorage/`

The read is hardened and read-only (the file is opened immutable, without
locks or journal sidecars, and rejected if it is not a regular file or is
larger than 16 MiB). Any failure silently falls back to `grid`; the import is
best-effort and never user-facing. A missing or unreadable database writes
nothing, so the import is retried on each launch until `[ui]` stores a view
mode. After a successful import, the Library toggle reads and writes
`[ui].library_view_mode` in `settings.toml` and no longer touches
`localStorage`.

## Degraded behaviour

If the settings file is not writable, `[ui]` changes still apply in memory for
the rest of the session but are not persisted; the first failed write is
logged and surfaced internally as an event, later failures are only logged.
Writes are spaced at least one second apart: the first change after an idle
window is written immediately (leading write), and every change made inside
the following one-second window coalesces into a single trailing write. A
burst of rapid changes — for example a window resize — produces one leading
write plus at most one trailing write.

## Runtime-only state

Toast and banner dismissals (rename toasts, the Flatpak migration toast) are
per-session and are **not** saved anywhere.

## Known limitations

- Builds before 0.6.1 do not preserve unknown tables and drop `[ui]` when
  they save settings. Builds from 0.6.1 until the release that reads it keep
  the table verbatim but unread; `[ui]` itself is new in 0.7.0.
- A `renderer` value written by a newer build (for example a future backend)
  reads as `auto` on an older build, and the unknown value is lost on the
  next save from that older build.
- `window_width` / `window_height` are only written by the native shell on
  resize end; Classic never sets them.
- The exit-time flush of pending writes only happens on a graceful exit.
  Changes made in the last second before a non-graceful exit (SIGTERM/`kill`,
  session logout) can be lost with their trailing write.
- The WebKit import is skipped when any directory on the localStorage path is
  a symlink (for example a symlinked `~/.local/share` from a dotfile
  manager); the hardened read refuses to follow symlinked path components.
