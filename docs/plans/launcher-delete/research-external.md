# External API Research: launcher-delete

## Executive Summary

The launcher-delete feature requires managing two file types on the Linux filesystem: `.desktop` entries (in `~/.local/share/applications/`) and shell scripts (in `~/.local/share/crosshook/launchers/`). The freedesktop Desktop Entry Specification and XDG Base Directory Specification govern file locations and format. CrossHook already generates these files with `std::fs` and the `directories` crate (v5, `BaseDirs`), so no new heavyweight dependencies are required. The delete and rename operations map directly to `std::fs::remove_file` and `std::fs::rename`, with an optional `update-desktop-database` call to refresh the desktop environment's MIME cache. The existing naming convention (`crosshook-{slug}-trainer.desktop` and `{slug}-trainer.sh`) provides a deterministic mapping from profile state to file paths, making lifecycle management straightforward.

## Primary APIs

### Freedesktop Desktop Entry Specification (v1.5)

- **Documentation**: <https://specifications.freedesktop.org/desktop-entry/latest-single/>
- **Practical guide**: <https://wiki.archlinux.org/title/Desktop_entries>
- **Authentication**: N/A (local filesystem specification)

**Confidence**: High -- specification is authoritative and stable; last updated 2024.

#### Required Fields for .desktop Files

| Field  | Type         | Requirement                   | CrossHook Usage            |
| ------ | ------------ | ----------------------------- | -------------------------- |
| `Type` | string       | Always required               | `Application`              |
| `Name` | localestring | Always required               | `{display_name} - Trainer` |
| `Exec` | string       | Required for Type=Application | `/bin/bash {script_path}`  |

#### Optional Fields CrossHook Uses

| Field           | Type         | Purpose                                     |
| --------------- | ------------ | ------------------------------------------- |
| `Version`       | string       | Spec version compliance (`1.0`)             |
| `Comment`       | localestring | Tooltip text                                |
| `Icon`          | iconstring   | Absolute path to PNG/JPG or theme icon name |
| `Terminal`      | boolean      | `false` -- no terminal window               |
| `Categories`    | string(s)    | `Game;`                                     |
| `StartupNotify` | boolean      | `false`                                     |

#### Key Specification Constraints

1. **Encoding**: Files MUST be encoded in UTF-8.
2. **Escape sequences**: Values of type `string` and `localestring` support `\s`, `\n`, `\t`, `\r`, and `\\`. The `Exec` key has additional quoting rules where `"`, `` ` ``, `$`, and `\` inside double quotes must be backslash-escaped.
3. **Field code processing**: Only the `Exec` key processes `%f`, `%F`, `%u`, `%U`, `%i`, `%c`, `%k` field codes. The `Name` field does NOT process field codes, so special characters like `%` are safe there.
4. **Case sensitivity**: Case is significant everywhere in the file.
5. **Preservation rule**: Implementations MUST NOT remove any fields from the file, even unsupported ones, during rewrites. This means when renaming a launcher, we should rewrite the entire file rather than doing partial edits.
6. **Hidden flag**: Setting `Hidden=true` effectively removes an entry without deleting the file. This is an alternative to deletion but CrossHook should prefer actual deletion for cleanliness.
7. **TryExec behavior**: When the executable named by `Exec=` or `TryExec=` cannot be found on the filesystem, desktop environments SHOULD automatically hide that entry from menus. This means orphaned `.desktop` files (pointing to deleted scripts) degrade gracefully but should still be cleaned up.

#### File Naming Rules

- **Extension**: Must be `.desktop`
- **Recommended format**: Reverse DNS notation (e.g., `org.example.FooViewer.desktop`)
- **D-Bus well-known name**: Filename (minus extension) should be a sequence of non-empty elements separated by dots, containing only `[A-Za-z0-9-_]`, none starting with a digit
- **CrossHook convention**: `crosshook-{slug}-trainer.desktop` -- uses the `crosshook-` prefix for easy identification and glob-based discovery
- **Desktop File ID**: Derived by making the path relative to `$XDG_DATA_DIRS`, removing `applications/`, and converting `/` to `-`

### XDG Base Directory Specification

- **Documentation**: <https://specifications.freedesktop.org/basedir/latest/>
- **Practical guide**: <https://wiki.archlinux.org/title/XDG_Base_Directory>

**Confidence**: High -- specification is authoritative and stable.

#### Relevant Environment Variables

| Variable        | Default                       | CrossHook Usage                                                                  |
| --------------- | ----------------------------- | -------------------------------------------------------------------------------- |
| `XDG_DATA_HOME` | `$HOME/.local/share`          | Parent of `applications/` (desktop entries) and `crosshook/launchers/` (scripts) |
| `XDG_DATA_DIRS` | `/usr/local/share:/usr/share` | System-wide search path (read-only for CrossHook)                                |

#### Directory Creation Rules

- When writing files, applications should create non-existent destination directories with permissions `0700`.
- CrossHook already does this via `fs::create_dir_all()` in the export code.

#### File Location Summary for CrossHook

| File Type          | Path Pattern                                                             |
| ------------------ | ------------------------------------------------------------------------ |
| Desktop entry      | `$XDG_DATA_HOME/applications/crosshook-{slug}-trainer.desktop`           |
| Shell script       | `$XDG_DATA_HOME/crosshook/launchers/{slug}-trainer.sh`                   |
| Fallback (current) | `~/.local/share/applications/` and `~/.local/share/crosshook/launchers/` |

### Desktop Database Update Mechanism

- **Documentation**: <https://man.archlinux.org/man/update-desktop-database.1.en>
- **Package**: `desktop-file-utils`

**Confidence**: High -- widely used system tool.

#### Purpose

`update-desktop-database` builds a cache database (`mimeinfo.cache`) of MIME types handled by desktop files. It is relevant when desktop entries declare `MimeType=` associations.

#### CrossHook Relevance

CrossHook's desktop entries do NOT declare `MimeType=`, so running `update-desktop-database` is **optional** and primarily for correctness. Desktop environments (GNOME, KDE, etc.) use inotify or similar filesystem monitoring to detect changes to `~/.local/share/applications/` and typically update their menus automatically without requiring explicit cache invalidation.

#### Usage Pattern

```bash
# After creating, deleting, or renaming .desktop files
update-desktop-database "$HOME/.local/share/applications"
```

- The `--quiet` flag suppresses output for programmatic use
- Safe to call even if no changes occurred
- Deleting the `mimeinfo.cache` file is safe; it regenerates on next package update
- The command may not be present on all systems; its absence should not be treated as an error

### xdg-desktop-menu Tool

- **Documentation**: <https://manpages.ubuntu.com/manpages/focal/man1/xdg-desktop-menu.1.html>

**Confidence**: Medium -- available on most Linux desktops but not universally present (e.g., minimal Steam Deck installs may lack it).

#### Relevant Commands

```bash
# Uninstall a desktop entry
xdg-desktop-menu uninstall crosshook-elden-ring-deluxe-trainer.desktop

# Install a desktop entry
xdg-desktop-menu install crosshook-elden-ring-deluxe-trainer.desktop
```

#### CrossHook Recommendation

Direct filesystem operations (`std::fs::remove_file`, `std::fs::rename`) are preferred over `xdg-desktop-menu` because:

1. CrossHook already manages the exact file paths deterministically
2. `xdg-desktop-menu` may not be installed on all targets (especially Steam Deck in gaming mode)
3. Direct operations avoid spawning a subprocess for simple file operations
4. The tool primarily adds value for system-level installations, not user-local ones

### desktop-file-validate Tool

- **Documentation**: <https://manpages.ubuntu.com/manpages/bionic/man1/desktop-file-validate.1.html>
- **Package**: `desktop-file-utils`

**Confidence**: Medium -- useful for development/testing, not critical for runtime.

#### Purpose

Validates `.desktop` files against the Desktop Entry Specification. Returns exit code 0 if valid.

#### CrossHook Recommendation

Use during development and CI to validate generated `.desktop` files. Not needed at runtime since CrossHook controls the exact content format.

## Libraries and SDKs

### Currently Used (No New Dependencies Needed)

#### `directories` crate (v5) -- Already in Cargo.toml

- **Docs**: <https://docs.rs/directories/latest/directories/struct.BaseDirs.html>
- **License**: MIT / Apache-2.0
- **Usage in CrossHook**: `BaseDirs::new()` in logging, profiles, settings, community taps, and install service

**Confidence**: High -- already a project dependency, well-maintained, 22M+ downloads.

Key methods for launcher-delete:

```rust
use directories::BaseDirs;

if let Some(base_dirs) = BaseDirs::new() {
    // ~/.local/share (respects XDG_DATA_HOME)
    let data_dir = base_dirs.data_dir();

    // Desktop entries: data_dir/applications/crosshook-{slug}-trainer.desktop
    let desktop_dir = data_dir.join("applications");

    // Scripts: data_dir/crosshook/launchers/{slug}-trainer.sh
    let scripts_dir = data_dir.join("crosshook").join("launchers");
}
```

#### `std::fs` -- Rust standard library

- **Docs**: <https://doc.rust-lang.org/std/fs/index.html>

All necessary filesystem operations are in the standard library:

| Operation       | Function                        | Notes                                         |
| --------------- | ------------------------------- | --------------------------------------------- |
| Delete file     | `std::fs::remove_file(path)`    | Returns `Err` if file doesn't exist           |
| Rename file     | `std::fs::rename(from, to)`     | Atomic on same filesystem (Linux)             |
| Read file       | `std::fs::read_to_string(path)` | For reading `.desktop` content before rewrite |
| Write file      | `std::fs::write(path, content)` | For rewriting `.desktop` with updated Name    |
| Check existence | `Path::exists()`                | Subject to TOCTOU races; use with care        |
| Create dirs     | `std::fs::create_dir_all(path)` | Already used by CrossHook                     |
| Set permissions | `std::fs::set_permissions()`    | Already used by CrossHook                     |
| List directory  | `std::fs::read_dir(path)`       | For discovering existing launchers            |

#### `tempfile` crate (v3) -- Already in dev-dependencies

- **Docs**: <https://docs.rs/tempfile/latest/tempfile/>
- **License**: MIT / Apache-2.0

Already used for tests. Could be promoted to a regular dependency if atomic writes are desired, but for the launcher-delete use case, direct `std::fs` operations are sufficient since:

1. Both files are always on the same filesystem (`$HOME`)
2. Failure during rename/delete is recoverable (user can retry)
3. The files are not critical system files

### Evaluated but NOT Recommended

#### `freedesktop-desktop-entry` crate (v0.8.1)

- **Docs**: <https://docs.rs/freedesktop-desktop-entry/latest/freedesktop_desktop_entry/>
- **License**: MPL-2.0 (copyleft -- requires source availability for modifications)
- **Maintained by**: Pop!\_OS (System76)
- **Downloads**: ~22K/month
- **Last updated**: January 2026

**Confidence**: Medium

**Why NOT recommended**:

1. **Read-only focus**: The crate is designed for parsing/reading desktop entries, not writing or modifying them. It has no write API.
2. **License concern**: MPL-2.0 is copyleft. While it allows linking from non-MPL code, any modifications to the crate itself must be shared. This adds legal complexity compared to the existing MIT/Apache-2.0 stack.
3. **Unnecessary complexity**: CrossHook already generates `.desktop` files with simple string formatting. The rename operation only needs to update the `Name=` line and rewrite the file -- no parser needed.
4. **Heavy dependency**: Pulls in `bstr`, `thiserror`, `unicase`, `xdg`, and `gettext-rs`.

#### `deentry` crate

- **Docs**: <https://github.com/coastalwhite/deentry>
- **License**: MIT / Apache-2.0
- **Status**: 10 commits, 2 stars, last updated 2023

**Confidence**: Low

**Why NOT recommended**:

1. **Low maturity**: Very few commits and stars; essentially a single-author hobby project
2. **Stale**: No updates since 2023
3. **Unnecessary**: CrossHook's `.desktop` files are simple enough to manage with string operations

#### `xdg` crate (v3.0)

- **Docs**: <https://docs.rs/xdg/latest/xdg/struct.BaseDirectories.html>
- **License**: Apache-2.0 / MIT

**Why NOT recommended**: CrossHook already uses `directories` (v5) which provides the same functionality via `BaseDirs`. Adding `xdg` would be redundant. The `directories` crate is a superset that also works on Windows/macOS.

#### `notify` crate (filesystem watcher)

- **Docs**: <https://docs.rs/notify/latest/notify/>

**Why NOT recommended**: Filesystem watching is unnecessary for the launcher-delete feature. CrossHook is the sole manager of its launcher files and knows when changes occur (profile delete triggers launcher delete; profile rename triggers launcher rename). Polling or watching for external changes adds complexity with no benefit.

## Integration Patterns

### Recommended Approach: Direct Filesystem Operations

**Confidence**: High -- aligns with existing CrossHook patterns and uses well-understood Rust stdlib operations.

CrossHook should manage launcher lifecycle with direct `std::fs` calls, mirroring the existing `export_launchers()` pattern. No new dependencies are needed.

#### Architecture: Path Resolution

The key insight is that CrossHook's existing naming convention creates a **deterministic mapping** from profile state to file paths:

```
Profile name  ->  sanitize_launcher_slug()  ->  slug
slug  ->  "{slug}-trainer.sh"                  (script filename)
slug  ->  "crosshook-{slug}-trainer.desktop"   (desktop entry filename)
```

This means given a profile name (old or new), we can always compute the exact file paths for both the script and desktop entry. The `SteamExternalLauncherExportResult` already returns both paths after export.

#### Operation: Delete Launcher

When a profile is deleted or when the user manually deletes a launcher:

```rust
use std::fs;
use std::io;
use std::path::Path;

/// Remove a launcher's .desktop entry and shell script.
/// Silently succeeds if either file is already absent.
fn delete_launcher(
    desktop_entry_path: &Path,
    script_path: &Path,
) -> Result<(), io::Error> {
    // Delete .desktop entry first (user-visible artifact)
    match fs::remove_file(desktop_entry_path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    // Then delete the shell script
    match fs::remove_file(script_path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    Ok(())
}
```

**Key design decisions**:

- Ignore `NotFound` errors to make the operation idempotent (safe to call multiple times)
- Delete the `.desktop` entry first because it is the user-visible artifact; if the script deletion fails, the desktop entry is already gone and the user won't see a broken launcher
- No need to delete parent directories (`~/.local/share/applications/` is shared; `~/.local/share/crosshook/launchers/` may contain other launchers)

#### Operation: Rename Launcher

When a profile is renamed:

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

struct LauncherRenamePaths {
    old_desktop: PathBuf,
    new_desktop: PathBuf,
    old_script: PathBuf,
    new_script: PathBuf,
}

/// Rename a launcher's files and update internal references.
fn rename_launcher(
    paths: &LauncherRenamePaths,
    new_display_name: &str,
    old_script_path_str: &str,
    new_script_path_str: &str,
) -> Result<(), io::Error> {
    // 1. Rename the shell script file
    fs::rename(&paths.old_script, &paths.new_script)?;

    // 2. Read the existing .desktop entry
    let desktop_content = fs::read_to_string(&paths.old_desktop)?;

    // 3. Update Name= line and Exec= line (which references the script path)
    let updated_content = desktop_content
        .lines()
        .map(|line| {
            if line.starts_with("Name=") {
                format!("Name={new_display_name} - Trainer")
            } else if line.starts_with("Exec=") {
                // Replace old script path with new script path
                line.replace(old_script_path_str, new_script_path_str)
            } else if line.starts_with("Comment=") {
                format!(
                    "Comment=Trainer launcher for {new_display_name}. \
                     Generated by CrossHook: https://github.com/yandy-r/crosshook"
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    // 4. Write updated content to new .desktop path
    fs::write(&paths.new_desktop, updated_content)?;

    // 5. Remove old .desktop file (if the path actually changed)
    if paths.old_desktop != paths.new_desktop {
        match fs::remove_file(&paths.old_desktop) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }

    Ok(())
}
```

**Key design decisions**:

- Rename the script first (via atomic `fs::rename`) so the Exec= path is valid
- Rewrite the `.desktop` file content rather than renaming, because the `Name=`, `Exec=`, and `Comment=` fields must be updated to reflect the new display name and script path
- Write the new `.desktop` file before deleting the old one to avoid a window where no entry exists
- Handle the case where old and new paths are identical (e.g., display name changed but slug didn't)

#### Operation: Update Script Internal Content on Rename

The generated shell script contains a comment line with the display name:

```bash
# {display_name} - Trainer launcher
```

This is cosmetic and does not affect execution. For completeness, a rename could also update this comment by reading, modifying, and rewriting the script content. However, since the comment has no functional impact, this is low priority.

#### Operation: Discover Existing Launchers

To populate the UI with a list of existing launchers:

```rust
use std::fs;
use std::path::Path;

struct DiscoveredLauncher {
    slug: String,
    display_name: String,
    desktop_entry_path: String,
    script_path: String,
}

fn discover_launchers(data_dir: &Path) -> Vec<DiscoveredLauncher> {
    let desktop_dir = data_dir.join("applications");
    let scripts_dir = data_dir.join("crosshook").join("launchers");
    let mut launchers = Vec::new();

    let entries = match fs::read_dir(&desktop_dir) {
        Ok(entries) => entries,
        Err(_) => return launchers,
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Match CrossHook's naming convention
        if let Some(slug) = name
            .strip_prefix("crosshook-")
            .and_then(|s| s.strip_suffix("-trainer.desktop"))
        {
            let desktop_path = entry.path();
            let script_path = scripts_dir.join(format!("{slug}-trainer.sh"));

            // Extract display name from Name= line
            let display_name = fs::read_to_string(&desktop_path)
                .ok()
                .and_then(|content| {
                    content.lines()
                        .find(|l| l.starts_with("Name="))
                        .map(|l| l.trim_start_matches("Name=").to_string())
                })
                .unwrap_or_else(|| slug.to_string());

            launchers.push(DiscoveredLauncher {
                slug: slug.to_string(),
                display_name,
                desktop_entry_path: desktop_path.to_string_lossy().into_owned(),
                script_path: script_path.to_string_lossy().into_owned(),
            });
        }
    }

    launchers.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    launchers
}
```

### File Management Safety

**Confidence**: High

#### Atomic Rename Guarantees

On Linux, `std::fs::rename()` calls the POSIX `rename()` syscall, which is **atomic when both paths are on the same filesystem**. Since both `~/.local/share/applications/` and `~/.local/share/crosshook/launchers/` are under `$HOME`, they are guaranteed to be on the same filesystem. This means:

- The rename either fully succeeds or fully fails
- No partial state is visible to other processes
- The old filename disappears and the new filename appears in a single operation

**Source**: <https://doc.rust-lang.org/std/fs/fn.rename.html>

#### Idempotent Deletion

All deletion operations should ignore `ErrorKind::NotFound` to be safely retriable. This handles:

- User deletes a profile whose launcher was already manually removed
- Concurrent operations (unlikely but possible on multi-session systems)
- Crash recovery where partial cleanup occurred

#### No TOCTOU Concerns for CrossHook

The typical TOCTOU (Time of Check to Time of Use) race condition -- where you check if a file exists before operating on it -- is not a significant concern for CrossHook because:

1. CrossHook is the sole creator/manager of its launcher files
2. The `crosshook-` prefix in filenames makes collisions with other applications extremely unlikely
3. Operations are user-initiated and sequential within the application

### Desktop Cache Invalidation

**Confidence**: High

#### How Desktop Environments Detect Changes

| Desktop Environment      | Detection Mechanism                       | Manual Refresh Needed? |
| ------------------------ | ----------------------------------------- | ---------------------- |
| GNOME                    | inotify on `~/.local/share/applications/` | No                     |
| KDE Plasma               | inotify (via KDirWatch)                   | No                     |
| Xfce                     | Periodic polling + inotify                | Rarely                 |
| Steam Deck (Gaming Mode) | N/A (no desktop menu)                     | N/A                    |

Modern desktop environments use inotify to watch the `applications/` directory and automatically update their menus when `.desktop` files are added, removed, or modified. No explicit notification is needed.

#### Optional: update-desktop-database

For maximum compatibility, CrossHook can optionally run `update-desktop-database` after changes:

```rust
use std::process::Command;

fn refresh_desktop_database(applications_dir: &str) {
    // Best-effort: don't fail if the command is missing
    let _ = Command::new("update-desktop-database")
        .arg(applications_dir)
        .arg("--quiet")
        .status();
}
```

This is a low-priority enhancement since CrossHook's `.desktop` entries do not declare `MimeType=` and desktop environments already use inotify.

### Permissions Considerations

**Confidence**: High

| File Type                  | Mode               | Rationale                                                                                                 |
| -------------------------- | ------------------ | --------------------------------------------------------------------------------------------------------- |
| `.desktop` entry           | `0644` (rw-r--r--) | Desktop entries should be readable but not executable (the spec does not require +x for Type=Application) |
| Shell script               | `0755` (rwxr-xr-x) | Scripts must be executable                                                                                |
| `applications/` dir        | `0755`             | Standard for XDG directories                                                                              |
| `crosshook/launchers/` dir | `0755`             | Standard for user data directories                                                                        |

CrossHook already sets these permissions correctly in `write_host_text_file()`. The delete and rename operations do not need to change permissions -- `std::fs::rename()` preserves the original file's permissions.

## Constraints and Gotchas

### 1. Slug Collision on Rename

**Impact**: Medium | **Confidence**: High

If a profile is renamed to a name that produces the same slug as an existing launcher (from a different profile), the rename operation would overwrite the other launcher's files.

**Workaround**: Before renaming, check if the target filenames already exist and belong to a different profile. If so, either reject the rename or append a numeric suffix to the slug.

### 2. Script Content References Display Name

**Impact**: Low | **Confidence**: High

The generated shell script contains the display name in a comment: `# {display_name} - Trainer launcher`. On rename, this comment becomes stale. Since it has no functional impact, updating it is optional. If desired, the rename operation can read-modify-write the script file.

### 3. Desktop Entry Exec Path Contains Embedded Script Path

**Impact**: High | **Confidence**: High

The `.desktop` file's `Exec=` line contains the absolute path to the shell script: `Exec=/bin/bash /home/user/.local/share/crosshook/launchers/{slug}-trainer.sh`. When renaming, this path MUST be updated to match the new script filename. Failure to update this line will result in a broken launcher that points to a non-existent script.

**Mitigation**: The rename operation rewrites the entire `.desktop` file with updated content (see Rename pattern above).

### 4. Orphaned Desktop Entries Point to Missing Scripts

**Impact**: Low | **Confidence**: High

Per the specification: "When the executable named by a desktop entry's `Exec=` or `TryExec=` cannot be found on the filesystem, that entry should be hidden from main menus." This means if CrossHook fails to clean up a `.desktop` entry after deleting its script, the entry will be automatically hidden by compliant desktop environments. However, the file still exists on disk and clutters the applications directory.

**Mitigation**: Always delete both files together, `.desktop` entry first.

### 5. File Path Encoding

**Impact**: Low | **Confidence**: High

Both `.desktop` files and shell scripts are UTF-8. CrossHook's `sanitize_launcher_slug()` function already strips non-alphanumeric characters and normalizes to lowercase ASCII with hyphens, so generated filenames will never contain problematic characters (spaces, quotes, unicode, etc.).

The `Exec=` line in `.desktop` files requires escaping of spaces, double quotes, backticks, dollar signs, and backslashes. CrossHook's `escape_desktop_exec_argument()` already handles this correctly.

### 6. XDG_DATA_HOME Override

**Impact**: Low | **Confidence**: High

If the user has `XDG_DATA_HOME` set to a non-default value, `.desktop` files must be placed in `$XDG_DATA_HOME/applications/` rather than `~/.local/share/applications/`. The `directories` crate's `BaseDirs::data_dir()` already respects this environment variable, so using it consistently will handle this case.

**Current gap**: The existing `export_launchers()` function hardcodes `~/.local/share/` paths using `combine_host_unix_path()` rather than using `BaseDirs::data_dir()`. The new delete/rename functions should use `BaseDirs` for consistency and correctness. Consider refactoring the export function in a future pass as well.

### 7. Cross-Device Rename Failure

**Impact**: Very Low | **Confidence**: High

`std::fs::rename()` fails with `EXDEV` (error 18) if source and destination are on different filesystems. This is extremely unlikely for CrossHook since both the `applications/` and `crosshook/launchers/` directories are under `$HOME`. However, exotic setups (e.g., bind mounts, overlayfs) could trigger this.

**Mitigation**: If `rename()` returns `EXDEV`, fall back to copy + delete. This is a low-priority edge case.

### 8. Concurrent Access from Multiple CrossHook Instances

**Impact**: Very Low | **Confidence**: Medium

If two CrossHook instances (e.g., one in desktop mode, one via CLI) attempt to delete/rename the same launcher simultaneously, race conditions could occur. Since CrossHook is a single-user desktop application, this scenario is extremely unlikely.

**Mitigation**: No locking mechanism is needed for the initial implementation. If this becomes a concern, a simple `.lock` file or advisory flock could be added.

## Code Examples

### Basic Launcher Delete (Minimal Working Example)

```rust
use std::fs;
use std::io;
use std::path::Path;

/// Delete a CrossHook launcher's desktop entry and shell script.
///
/// Both paths are derived from the launcher slug:
///   desktop: $XDG_DATA_HOME/applications/crosshook-{slug}-trainer.desktop
///   script:  $XDG_DATA_HOME/crosshook/launchers/{slug}-trainer.sh
///
/// Idempotent: silently succeeds if files are already absent.
pub fn delete_launcher_files(
    desktop_entry_path: &Path,
    script_path: &Path,
) -> Result<(), io::Error> {
    remove_if_exists(desktop_entry_path)?;
    remove_if_exists(script_path)?;
    Ok(())
}

fn remove_if_exists(path: &Path) -> Result<(), io::Error> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
```

### Launcher Path Resolution from Slug

```rust
use directories::BaseDirs;
use std::path::PathBuf;

pub struct LauncherPaths {
    pub desktop_entry: PathBuf,
    pub script: PathBuf,
}

pub fn resolve_launcher_paths(slug: &str) -> Option<LauncherPaths> {
    let base_dirs = BaseDirs::new()?;
    let data_dir = base_dirs.data_dir();

    Some(LauncherPaths {
        desktop_entry: data_dir
            .join("applications")
            .join(format!("crosshook-{slug}-trainer.desktop")),
        script: data_dir
            .join("crosshook")
            .join("launchers")
            .join(format!("{slug}-trainer.sh")),
    })
}
```

### Desktop Entry Content Update on Rename

```rust
use std::fs;
use std::io;
use std::path::Path;

/// Rewrite a .desktop file's Name=, Exec=, and Comment= fields
/// to reflect a renamed launcher.
pub fn rewrite_desktop_entry(
    desktop_path: &Path,
    new_display_name: &str,
    old_script_path: &str,
    new_script_path: &str,
) -> Result<String, io::Error> {
    let content = fs::read_to_string(desktop_path)?;

    let updated = content
        .lines()
        .map(|line| {
            if line.starts_with("Name=") {
                format!("Name={new_display_name} - Trainer")
            } else if line.starts_with("Exec=") {
                line.replace(old_script_path, new_script_path)
            } else if line.starts_with("Comment=") {
                format!(
                    "Comment=Trainer launcher for {new_display_name}. \
                     Generated by CrossHook: https://github.com/yandy-r/crosshook"
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(updated)
}
```

### Tauri Command Pattern for Delete

```rust
use crosshook_core::export::launcher;

#[tauri::command]
pub fn delete_launcher(slug: String) -> Result<(), String> {
    let paths = launcher::resolve_launcher_paths(&slug)
        .ok_or_else(|| "Could not resolve home directory".to_string())?;

    launcher::delete_launcher_files(&paths.desktop_entry, &paths.script)
        .map_err(|e| format!("Failed to delete launcher: {e}"))
}
```

## Open Questions

1. **Profile-to-launcher tracking**: Should CrossHook store the exported launcher slug/paths in the profile TOML file? This would enable automatic launcher cleanup on profile delete without needing to recompute the slug. The alternative is to always recompute paths from the profile name using `sanitize_launcher_slug()`.

2. **Multiple launchers per profile**: Can a single profile have multiple exported launchers (e.g., different configurations)? The current export model is 1:1 (one profile -> one launcher). If multiple launchers are needed in the future, the slug-based path resolution needs a disambiguation mechanism.

3. **Confirmation UI**: Should the delete operation show a confirmation dialog? For manual deletion from the Launcher panel, a confirmation is recommended. For automatic deletion on profile delete, it could be a setting (auto-delete launchers vs. orphan them).

4. **Bulk operations**: Should the "discover existing launchers" feature allow bulk delete/rename? This is a UI concern but affects the Tauri command design (single vs. batch operations).

5. **Existing export path migration**: The current `export_launchers()` hardcodes `~/.local/share/` paths rather than using `BaseDirs::data_dir()`. Should the delete/rename implementation use `BaseDirs` (correct) even though export doesn't? This could cause a mismatch where delete looks in the XDG-correct location but the file was written to the hardcoded location. Consider refactoring export to use `BaseDirs` as part of this feature.

## Sources

### Specifications

- [Desktop Entry Specification (freedesktop.org)](https://specifications.freedesktop.org/desktop-entry/latest-single/)
- [Desktop Entry File Naming](https://specifications.freedesktop.org/desktop-entry/latest/file-naming.html)
- [Desktop Entry Basic Format](https://specifications.freedesktop.org/desktop-entry/latest/basic-format.html)
- [XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/latest/)

### Practical Guides

- [Desktop Entries -- ArchWiki](https://wiki.archlinux.org/title/Desktop_entries)
- [XDG Base Directory -- ArchWiki](https://wiki.archlinux.org/title/XDG_Base_Directory)

### Rust Standard Library

- [std::fs::rename](https://doc.rust-lang.org/std/fs/fn.rename.html)
- [std::fs::remove_file](https://doc.rust-lang.org/std/fs/fn.remove_file.html)
- [std::fs module](https://doc.rust-lang.org/std/fs/index.html)

### Rust Crates

- [directories crate (v5)](https://docs.rs/directories/latest/directories/struct.BaseDirs.html)
- [freedesktop-desktop-entry crate (evaluated, not recommended)](https://docs.rs/freedesktop-desktop-entry/latest/freedesktop_desktop_entry/)
- [deentry crate (evaluated, not recommended)](https://github.com/coastalwhite/deentry)
- [xdg crate (evaluated, not recommended)](https://docs.rs/xdg/latest/xdg/struct.BaseDirectories.html)
- [tempfile crate](https://docs.rs/tempfile/latest/tempfile/)

### Tools

- [update-desktop-database manpage](https://man.archlinux.org/man/update-desktop-database.1.en)
- [xdg-desktop-menu manpage](https://manpages.ubuntu.com/manpages/focal/man1/xdg-desktop-menu.1.html)
- [desktop-file-validate manpage](https://manpages.ubuntu.com/manpages/bionic/man1/desktop-file-validate.1.html)
- [desktop-file-utils project](https://www.freedesktop.org/wiki/Software/desktop-file-utils/)

### Atomic File Operations

- [Rust forum: Atomic file writes](https://users.rust-lang.org/t/how-to-write-replace-files-atomically/42821)
- [atomicwrites crate](https://github.com/untitaker/rust-atomicwrites)
- [tempfile::NamedTempFile::persist](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html)

---

_Research conducted: 2026-03-24_
_CrossHook version: v0.2.0_
_Existing export module: `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`_
