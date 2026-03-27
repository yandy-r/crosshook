# External API & Library Research: Profile Rename

## Executive Summary

Profile rename for CrossHook is primarily a filesystem operation (renaming TOML files on disk) with cascade side effects to exported launchers and app settings. The operation requires no external web APIs or cloud services. The Rust standard library's `std::fs::rename` provides the core primitive, which maps to the POSIX `rename(2)` syscall on Linux — atomic on the same filesystem. The existing codebase already has a `ProfileStore::rename()` method and a `profile_rename` Tauri command registered; the remaining work is frontend integration, cascade updates (settings `last_used_profile`, launcher files), and conflict detection (preventing overwrites of existing profiles).

**Confidence**: High — based on direct codebase analysis and official Rust/Tauri documentation.

## Primary APIs

### 1. Rust `std::fs::rename`

- **Documentation**: [std::fs::rename](https://doc.rust-lang.org/std/fs/fn.rename.html)
- **Status**: Stable since Rust 1.0
- **Already used**: Yes — `toml_store.rs:176` calls `fs::rename(&old_path, &new_path)`

**Signature:**

```rust
pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<()>
```

**Behavior on Linux:**

- Maps to POSIX `rename(2)` syscall
- Atomic on the same filesystem (guaranteed by POSIX — the destination path atomically replaces the old one; at no point does a reader see a missing file)
- If `to` already exists, it is **silently replaced** (this is the overwrite risk the implementation must guard against)
- Fails with `EXDEV` if source and destination are on different mount points (not applicable here — both are under `~/.config/crosshook/profiles/`)
- Fails if `from` does not exist (`ENOENT`)

**Confidence**: High — official Rust stdlib documentation + POSIX standard. [rename(2) man page](https://man7.org/linux/man-pages/man2/rename.2.html)

### 2. Tauri v2 File System Plugin (`@tauri-apps/plugin-fs`)

- **Documentation**: [Tauri v2 File System Plugin](https://v2.tauri.app/plugin/file-system/)
- **JavaScript API Reference**: [@tauri-apps/plugin-fs](https://v2.tauri.app/reference/javascript/fs/)

**Rename API (frontend-side):**

```typescript
import { rename, BaseDirectory } from '@tauri-apps/plugin-fs';

await rename('old-name.toml', 'new-name.toml', {
  fromPathBaseDir: BaseDirectory.AppConfig,
  toPathBaseDir: BaseDirectory.AppConfig,
});
```

**NOT recommended for this feature.** The Tauri docs explicitly state: "Although this plugin has a file manipulation API on the frontend, in the backend it offers only the methods to change permission of some resources." For file operations in Tauri v2, the recommendation is to use `std::fs` on the Rust side and expose operations via Tauri IPC commands — which is exactly the pattern CrossHook already follows.

**Confidence**: High — official Tauri v2 documentation.

### 3. Tauri v2 State Management

- **Documentation**: [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/)

The existing `ProfileStore` is already managed as Tauri state (`State<'_, ProfileStore>`). The `profile_rename` command already receives the store via dependency injection. No changes needed to the state management layer.

**Key detail**: Tauri wraps managed state in `Arc` internally, so `ProfileStore` (which is `Clone` and contains only a `PathBuf`) is thread-safe as-is. Since `ProfileStore` methods use `std::fs` directly (no interior mutable state), no `Mutex` is needed.

**Confidence**: High — verified from both Tauri docs and existing codebase patterns.

### 4. Tauri v2 Event System

- **Documentation**: [Calling Frontend from Rust](https://v2.tauri.app/develop/calling-frontend/)

If the rename command needs to notify the frontend of side effects (e.g., launcher rename warnings), the Tauri event system can emit events:

```rust
use tauri::Emitter;

app_handle.emit("profile-renamed", payload)?;
```

**Not required for initial implementation** — the rename command can return a result struct with all necessary information. Events would only be needed for background/async rename workflows.

**Confidence**: Medium — the event system exists but may not be needed for this feature.

## Libraries and SDKs

### Already In Use (No New Dependencies Needed)

| Crate         | Version   | Purpose                | Relevance               |
| ------------- | --------- | ---------------------- | ----------------------- |
| `serde`       | 1.x       | Serialize/Deserialize  | Profile TOML encoding   |
| `toml`        | 0.8       | TOML parsing/writing   | Profile file format     |
| `directories` | 5.x       | XDG base directories   | Profile path resolution |
| `tempfile`    | 3.x (dev) | Temp files for testing | Test infrastructure     |
| `tracing`     | 0.1       | Structured logging     | Operation logging       |

**Confidence**: High — verified from `crates/crosshook-core/Cargo.toml`.

### Evaluated But Not Recommended

#### `renamore` (Atomic No-Clobber Rename)

- **Documentation**: [renamore on docs.rs](https://docs.rs/renamore)
- **crates.io**: [renamore](https://crates.io/crates/renamore)
- **Version**: 0.3.2
- **License**: MIT/Apache-2.0

**What it provides:**

- `rename_exclusive()` — atomically rename a file, failing if destination exists (no-clobber)
- `rename_exclusive_fallback()` — attempts atomic, falls back to non-atomic
- Uses Linux `renameat2` with `RENAME_NOREPLACE` flag under the hood

**Why not recommended:**
The existing implementation can achieve the same no-clobber behavior with a simple `path.exists()` check before calling `fs::rename()`. The TOCTTOU window is negligible for a single-user desktop app where only one process writes to the profiles directory. Adding a dependency for a single syscall wrapper is overkill.

**Confidence**: High — evaluated API surface against actual requirements.

#### `atomic-write-file` / `atomicwrites`

- **Documentation**: [atomic-write-file](https://docs.rs/atomic-write-file)
- **Pattern**: Write to temp file in same directory, then `rename()` to final path

**Why not recommended:**
These crates solve the "write new content atomically" problem (preventing readers from seeing partial writes). Profile rename doesn't write new content — it just moves the file. The existing `fs::rename()` is already atomic for this use case.

**Confidence**: High — the atomic write pattern solves a different problem than file renaming.

#### `tempfile::NamedTempFile::persist()`

- **Documentation**: [NamedTempFile](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html)

**Provides:**

- `persist(path)` — atomically moves temp file to target, replacing if exists
- `persist_noclobber(path)` — fails if target exists

**Why not recommended:**
Same as above — useful for atomic writes, not for renaming existing files. Already a dev-dependency for tests.

**Confidence**: High.

## Integration Patterns

### Pattern 1: Direct `fs::rename` with Pre-Check (Recommended)

This is the pattern the codebase already uses. The profile rename operation:

1. Validate both old and new names (`validate_name()`)
2. Check old profile exists
3. Check new profile does NOT exist (conflict detection — currently missing)
4. Call `fs::rename(old_path, new_path)`
5. Cascade: update `last_used_profile` in settings if it matched old name
6. Cascade: rename associated launcher files if they exist

```rust
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    validate_name(old_name)?;
    validate_name(new_name)?;
    let old_path = self.profile_path(old_name)?;
    let new_path = self.profile_path(new_name)?;
    if !old_path.exists() {
        return Err(ProfileStoreError::NotFound(old_path));
    }
    if old_name == new_name {
        return Ok(()); // no-op
    }
    // MISSING: conflict detection for existing target
    fs::rename(&old_path, &new_path)?;
    Ok(())
}
```

**Confidence**: High — matches existing codebase patterns.

### Pattern 2: Write-Then-Delete for Content-Bearing Rename (Launcher Files)

The launcher store already implements this pattern in `rename_launcher_files()` — when a launcher slug changes, it:

1. Generates new file content with updated display names/paths
2. Writes new files at new paths
3. Deletes old files (best-effort, with cleanup warnings)

This is necessary because launcher `.sh` and `.desktop` files embed display names and paths as plaintext, so a simple `fs::rename` would leave stale content.

**Confidence**: High — observed directly in `launcher_store.rs:362-449`.

### Pattern 3: Cascade Update Pattern

Profile rename has side effects that must cascade:

| Resource                             | Keyed By                          | Update Needed                                             |
| ------------------------------------ | --------------------------------- | --------------------------------------------------------- |
| Profile TOML file                    | Filename (`{name}.toml`)          | `fs::rename` the file                                     |
| `settings.last_used_profile`         | Profile name string               | Update if matches old name                                |
| Exported launcher `.sh` + `.desktop` | Derived from profile display name | Rename/rewrite if slug changes                            |
| Community tap references             | Not applicable                    | No — taps reference source repos, not local profile names |
| Recent files (`recent.toml`)         | File paths (game, trainer, dll)   | No — tracks executable paths, not profile names           |

**Confidence**: High — verified by reading `startup.rs`, `recent.rs`, `launcher_store.rs`, and `community/` modules.

### Pattern 4: Tauri IPC Command Result Pattern

The existing codebase pattern for operations with side effects returns a result struct:

```rust
// Example from duplicate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateProfileResult {
    pub name: String,
    pub profile: GameProfile,
}
```

For rename, a result struct could communicate cascade outcomes:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameProfileResult {
    pub old_name: String,
    pub new_name: String,
    pub launcher_rename: Option<LauncherRenameResult>,
    pub settings_updated: bool,
}
```

**Confidence**: Medium — recommended pattern based on existing conventions; not yet implemented.

## Constraints and Gotchas

### 1. Silent Overwrite on `fs::rename`

**Risk**: `std::fs::rename` silently replaces the target if it exists. If a user renames profile "A" to "B" and "B" already exists, "B" is permanently lost.

**Mitigation**: Add an existence check before the rename call. The current implementation at `toml_store.rs:163-178` does NOT check for target existence.

**Confidence**: High — verified by reading the `rename()` method and the test `test_rename_overwrites_existing_target_profile` which explicitly tests (and expects) this overwrite behavior.

### 2. `last_used_profile` Settings Stale Reference

**Risk**: If `settings.last_used_profile == old_name`, the auto-load-on-startup feature (`startup.rs:34-64`) will fail to find the renamed profile, falling back to "no auto-load."

**Mitigation**: The Tauri command must update `last_used_profile` in settings when it matches the old name.

**Confidence**: High — verified from `startup.rs:50-63` which matches `last_used_profile` against available profile names.

### 3. Launcher File Cascade Complexity

**Risk**: Launcher files embed profile metadata (display names, paths, slugs). A profile rename may or may not change the launcher slug depending on whether the profile's `steam.launcher.display_name` field changes.

**Key insight**: Profile filename and profile `display_name` are independent. Renaming the profile file (changing `old-name.toml` to `new-name.toml`) does NOT inherently change the `steam.launcher.display_name` field inside the TOML. The launcher cascade is only needed if the display name inside the profile also changes.

**Confidence**: High — verified by reading `launcher_store.rs` and the profile data model.

### 4. Case Sensitivity

**Behavior on Linux (ext4, btrfs, XFS)**: File renames are case-sensitive. Renaming "MyGame" to "mygame" creates a different file. `fs::rename("MyGame.toml", "mygame.toml")` works correctly.

**Behavior on macOS (APFS default)**: Case-insensitive but case-preserving. Renaming "MyGame" to "mygame" preserves the new case but `MyGame.toml` and `mygame.toml` refer to the same file. This is a no-op rename at the filesystem level but the `validate_name` check would see them as different names.

**CrossHook context**: Primary target is Linux (AppImage), so case sensitivity is the expected behavior. macOS is listed as a future target but not yet supported.

**Confidence**: High — well-documented OS behavior.

### 5. Concurrent Access

**Risk**: If two frontend actions race (e.g., rapid double-click on rename), the second rename could fail with `NotFound` if the first already moved the file.

**Mitigation**: The single-user desktop app context makes true concurrency unlikely. The frontend should disable the rename action while a rename is in-flight. No `Mutex` is needed on the Rust side because `fs::rename` is atomic at the syscall level.

**Confidence**: High — single-user desktop app assumption is valid.

### 6. Filename Character Restrictions

The existing `validate_name()` function already rejects: empty/`.`/`..` names, absolute paths, path separators (`/`, `\`), colons, and all Windows reserved characters (`<>:"/\|?*`). This validation applies to both old and new names during rename.

**Confidence**: High — verified from `toml_store.rs:273-298`.

## Code Examples

### Existing Backend Implementation (Already in Codebase)

**Profile store rename** (`crates/crosshook-core/src/profile/toml_store.rs:163-178`):

```rust
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    validate_name(old_name)?;
    validate_name(new_name)?;
    let old_path = self.profile_path(old_name)?;
    let new_path = self.profile_path(new_name)?;
    if !old_path.exists() {
        return Err(ProfileStoreError::NotFound(old_path));
    }
    if old_name == new_name {
        return Ok(()); // no-op
    }
    fs::rename(&old_path, &new_path)?;
    Ok(())
}
```

**Tauri IPC command** (`src-tauri/src/commands/profile.rs:148-154`):

```rust
#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)
}
```

### Suggested: Enhanced Rename with Conflict Detection

```rust
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    validate_name(old_name)?;
    validate_name(new_name)?;
    if old_name == new_name {
        return Ok(());
    }
    let old_path = self.profile_path(old_name)?;
    let new_path = self.profile_path(new_name)?;
    if !old_path.exists() {
        return Err(ProfileStoreError::NotFound(old_path));
    }
    if new_path.exists() {
        return Err(ProfileStoreError::AlreadyExists(new_name.to_string()));
    }
    fs::rename(&old_path, &new_path)?;
    Ok(())
}
```

### Suggested: Cascade-Aware Tauri Command

```rust
#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<(), String> {
    // 1. Rename the profile file
    store.rename(&old_name, &new_name).map_err(map_error)?;

    // 2. Update last_used_profile if it matches old name
    if let Ok(mut settings) = settings_store.load() {
        if settings.last_used_profile.trim() == old_name.trim() {
            settings.last_used_profile = new_name.clone();
            let _ = settings_store.save(&settings); // best-effort
        }
    }

    // 3. Launcher cascade would go here (if profile display_name changed)

    Ok(())
}
```

### Frontend Invocation Pattern

```typescript
import { invoke } from '@tauri-apps/api/core';

async function renameProfile(oldName: string, newName: string): Promise<void> {
  await invoke('profile_rename', { oldName, newName });
}
```

## Open Questions

1. **Should rename prevent overwriting existing profiles?** The current implementation silently overwrites. The test suite explicitly tests this behavior (`test_rename_overwrites_existing_target_profile`). A new `AlreadyExists` error variant would need to be added to `ProfileStoreError` if overwrite prevention is desired.

2. **Should the `game.name` field inside the TOML be updated to match the new filename?** Currently these are independent — `game.name` is a display name inside the profile, while the filename is the profile identifier. Changing one doesn't change the other.

3. **Should launcher files cascade on profile rename?** Only relevant if the profile's `steam.launcher.display_name` changes. If only the profile filename changes (not the display name), launchers are unaffected. The Tauri command needs to decide whether profile rename also updates the display name.

4. **Should the frontend use optimistic updates?** The rename is fast (single syscall), so waiting for the IPC response before updating the UI is acceptable. Optimistic updates add complexity without meaningful UX benefit.

## Search Queries Executed

1. `Tauri v2 file system API rename file Rust 2025 2026`
2. `Rust std::fs::rename atomic operation Linux cross-platform behavior`
3. `Rust TOML crate serialize deserialize rename file best practices`
4. `Rust renamore crate atomic rename no-clobber renameat2`
5. `Tauri v2 IPC command rename file best practices state management 2025`
6. `desktop application rename file-backed entity cascade side effects pattern`
7. `Rust tempfile NamedTempFile persist atomic rename same directory pattern`
8. `Linux rename(2) syscall atomicity POSIX same filesystem guarantees`
9. `Rust toml crate version latest 2025 2026 features changelog`
10. `Tauri v2 event system emit frontend notify state change after rename operation`
11. `"fs::rename" "already exists" conflict detection Rust desktop app profile rename`
12. `Rust atomic-write-file crate tempfile atomic file write rename pattern`

## Sources

- [std::fs::rename — Rust Documentation](https://doc.rust-lang.org/std/fs/fn.rename.html)
- [rename(2) — Linux Manual Page](https://man7.org/linux/man-pages/man2/rename.2.html)
- [Tauri v2 File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [@tauri-apps/plugin-fs Reference](https://v2.tauri.app/reference/javascript/fs/)
- [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/)
- [Tauri v2 Calling Frontend from Rust](https://v2.tauri.app/develop/calling-frontend/)
- [Tauri v2 IPC Concepts](https://v2.tauri.app/concept/inter-process-communication/)
- [renamore — docs.rs](https://docs.rs/renamore)
- [renamore — crates.io](https://crates.io/crates/renamore)
- [atomic-write-file — docs.rs](https://docs.rs/atomic-write-file)
- [atomicwrites — GitHub](https://github.com/untitaker/rust-atomicwrites)
- [tempfile::NamedTempFile — docs.rs](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html)
- [toml — crates.io](https://crates.io/crates/toml)
- [Add std::fs::rename_noreplace — rust-lang/libs-team#131](https://github.com/rust-lang/libs-team/issues/131)
- [std::fs::rename fails on Windows — rust-lang/rust#123985](https://github.com/rust-lang/rust/issues/123985)
- [Rename file without overriding existing target — Rust Internals](https://internals.rust-lang.org/t/rename-file-without-overriding-existing-target/17637)

## Uncertainties & Gaps

- **macOS case-insensitive rename edge case**: Not fully tested in the current codebase. If CrossHook ships on macOS, renaming "MyGame" to "mygame" would be a filesystem no-op on APFS but the profile store would see them as different names. Low priority since macOS is not yet a supported target.
- **Concurrent IPC calls**: No investigation into whether Tauri serializes IPC calls or if two rapid `profile_rename` invocations could race. Practically irrelevant for single-user desktop app.
- **`renameat2` with `RENAME_NOREPLACE`**: Rust stdlib does not expose this. The `renamore` crate does, but it's unnecessary for this use case. If Rust stdlib adds `rename_noreplace` in the future (tracked in [libs-team#131](https://github.com/rust-lang/libs-team/issues/131)), it could replace the check-then-rename pattern.
