# Security Research: proton-migration-tool

## Executive Summary

The proton-migration-tool is a local-only, filesystem-bounded feature with a narrow attack surface. There are **no CRITICAL hard-stops** that would block the feature from shipping; the security risks are addressable with standard write-safety patterns already proven in similar Rust CLI tooling.

The two highest-priority items are:

1. **Non-atomic profile writes** (WARNING) — the existing `toml_store.rs::save()` uses `fs::write()` which truncates before writing; a batch migration touching multiple files is one crash away from data loss. Fix: write-to-tempfile + `rename(2)` (atomic on Linux within the same filesystem).
2. **Wrong field targeting in the local_override model** (WARNING) — since v0.2.4, portable profiles store machine paths in `local_override.steam.proton_path`, not `steam.proton_path`. A migration that patches the wrong field silently fails to fix the stale path.

All other findings are ADVISORY. No new dependencies are required; the feature can be implemented entirely with existing crates.

> **Update — full teammate review complete**: All teammates have reviewed this document. Additions: `sanitize_display_path()` already exists (A-4); `steam_client_install_path` IPC validation (A-5); profile TOML permissions gap (A-6); concurrent write race (A-7); use `try_exists()` not `canonicalize()` for staleness (A-8); file picker validation (A-9); React auto-escaping is sufficient (A-10). **Final count: 0 CRITICAL, 4 WARNING, 10 ADVISORY.**

---

## Findings by Severity

### CRITICAL — Hard Stops

_None._ The feature operates entirely within the user's own `~/.config/crosshook/` and Steam library directories. There is no network I/O, no privilege escalation vector, and no path traversal risk that crosses a meaningful trust boundary.

---

### WARNING — Must Address

#### W-1: Non-Atomic Profile Writes During Batch Migration

**Where**: `toml_store.rs:117` — `fs::write(path, toml::to_string_pretty(&storage_profile)?)`

**Risk**: `fs::write` on Linux uses `open(O_WRONLY | O_CREAT | O_TRUNC)` followed by `write()`. If the process is killed between the truncate and the final write, the profile file is left empty or partially written with no recovery path. For a single save this is tolerable; for a batch migration touching 5–20 profiles, the expected-failure surface grows proportionally.

**Mitigation**: Replace the direct `fs::write` in the migration path with a write-to-tempfile + atomic rename pattern:

```rust
let tmp_path = path.with_extension("toml.tmp");
fs::write(&tmp_path, serialized_content)?;
fs::rename(&tmp_path, &path)?; // atomic on Linux within same filesystem
```

The `tempfile` crate is already a dev-dependency; adding it as a regular dependency is acceptable, or use `std::fs` directly with a `.tmp` suffix. The rename must land on the same filesystem as the target (i.e., within `~/.config/crosshook/profiles/`), which is always true here.

**Confidence**: High — confirmed by reading `toml_store.rs:save()` implementation.

---

#### W-2: Migration Must Target `local_override` Fields, Not Base Profile Fields

**Where**: `profile/models.rs:272–298` — `storage_profile()` and `effective_profile()` logic.

**Risk**: Since v0.2.4, profiles use a portable/local split. When a profile is saved, machine-specific paths are moved to `local_override.steam.proton_path` and `local_override.runtime.proton_path`; the base `steam.proton_path` field is cleared to an empty string (see `storage_profile()` lines 283–286). At runtime, `effective_profile()` prefers `local_override` over base fields.

A migration implementation that calls `profile.steam.proton_path == stale_path` will almost always miss the stale path (the base field is empty), and one that writes the replacement to `profile.steam.proton_path` will have no effect at runtime because `effective_profile()` will continue using the non-empty `local_override` value.

The stale detection and replacement must operate on the **effective profile** (the output of `effective_profile()`) and write the replacement into the correct storage location. The correct fix is to either:

- Detect staleness on the effective path, then update the `local_override` field directly before calling `save()`, or
- Use a dedicated `migrate_proton_path(old_path, new_path)` method on `ProfileStore` that handles the effective-vs-storage logic internally.

**Confidence**: High — confirmed by tracing `storage_profile()` and `effective_profile()` in `models.rs`.

---

#### W-3: No User Consent Gate at the Command Layer

**Where**: Future `src-tauri/src/commands/` migration command (not yet implemented).

**Risk**: The migration feature could be designed such that auto-detection on startup triggers a migration without a visible confirmation prompt — either through a coding oversight or a feature-creep path where "stale detection" gets coupled to "auto-fix". User configuration data must not be modified without explicit per-session approval.

**Mitigation**: The Tauri command for applying migration must accept an explicit `confirm: bool` or `dry_run: bool` parameter. The command should return a preview `MigrationPlan` struct on the first call (no writes), and only apply changes when called with `confirm: true`. This is an architectural constraint that should be captured in the technical spec before implementation begins.

**Confidence**: High — design constraint, not a code defect in existing code.

---

#### W-4: No Rollback for Partial Batch Migration Failure

**Where**: Batch migration logic (not yet implemented).

**Risk**: If a batch of 10 profile updates fails at profile 6 (e.g., filesystem full, permission error, unexpected profile format), profiles 1–5 are permanently modified with no undo path. The user has no way to know which profiles were migrated and which were not.

**Mitigation**: Two acceptable approaches:

- **Pre-flight backup**: Before any writes, copy all affected `.toml` files to a timestamped backup directory (e.g., `~/.config/crosshook/backups/migration-YYYYMMDD-HHMMSS/`). This is robust but adds complexity.
- **Pre-flight validation**: Validate all target paths and serialize all new TOML content before writing any file. If any step fails, abort with zero writes. Only begin writing after all pre-flight checks pass. This is simpler and preferred for this feature's scope.

The pre-flight validation approach fits naturally with the dry-run design from W-3.

**Confidence**: High — follows from the batch write design.

---

### ADVISORY — Best Practices

#### A-1: Symlink Following in Steam Compatibility Tool Discovery

**Where**: `proton.rs:338` — `proton_path.is_file()` and `safe_enumerate_directories`.

**Detail**: `Path::is_file()` follows symlinks. A symlink at `~/.steam/root/compatibilitytools.d/evil-tool/proton` pointing to a non-Proton file would pass the `is_file()` guard and get registered as a discovered Proton installation. The migration tool would then potentially offer this as a replacement.

**Risk level**: Low. Requires a user or another process to have placed a malicious symlink in the user's own Steam directory. The path ends up written to a profile field that CrossHook later uses to launch games — it won't escalate privileges, but it will produce a broken launch.

**Mitigation**: Before registering a discovered path as a valid Proton replacement, verify that it is executable: `fs::metadata(&proton_path).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)`. This is a cheap additional check.

**Confidence**: Medium — Linux `is_file()` behavior confirmed; practical exploitability is low.

---

#### A-2: `seen_proton_paths` Deduplication Does Not Canonicalize

**Where**: `proton.rs:338` — `seen_proton_paths.insert(proton_path.clone())`.

**Detail**: Two symlinks pointing to the same physical Proton binary will both pass the `seen_proton_paths` HashSet check (different path strings, same inode). This means the same Proton installation could appear twice in the discovered list, surfacing as an "ambiguous" resolution when it shouldn't be.

**Risk level**: Low. Produces confusing UX (duplicate suggestions) rather than a security issue. Only manifests with unusual symlink layouts.

**Mitigation**: Use `fs::canonicalize()` before inserting into `seen_proton_paths`. Note that `canonicalize()` returns an error if the path doesn't exist, so wrap in `unwrap_or_else(|_| proton_path.clone())` to preserve the original on failure.

**Confidence**: High — behavior confirmed from code; practical impact is rare.

---

#### A-3: Migration Audit Log

**Where**: No existing migration infrastructure.

**Detail**: There is no audit trail for profile mutations. A batch migration that changes 15 profiles leaves no record of what changed, making it hard for users or future developers to diagnose issues.

**Mitigation**: Write a simple migration record to SQLite (the metadata layer already exists). A `migration_log` table with `(id, migrated_at, profile_id, old_path, new_path, status)` rows would be sufficient. This is deferrable to a follow-up issue.

**Confidence**: High — feature gap, not a bug.

---

#### A-4: Apply `sanitize_display_path()` to Migration IPC Results

**Where**: Future migration Tauri command in `src-tauri/src/commands/`.

**Detail**: `sanitize_display_path()` already exists in `commands/shared.rs` and is used by `launch.rs` and `health.rs` to replace the home directory prefix with `~` before sending paths over the IPC boundary. Migration confirmation results (before/after paths in `MigrationPlan` and `MigrationResult` structs) must also pass through `sanitize_display_path()` before being returned to the frontend.

**Risk level**: Low — the main concern is consistency with the rest of the codebase, not a security hard-stop. Full paths are acceptable in local UI; the function mainly ensures paths don't embed raw usernames in places that could leak (logs, community-facing surfaces).

**Mitigation**: Apply `sanitize_display_path()` to all path strings in the migration result structs, consistent with the pattern in `launch.rs:373–389`.

**Confidence**: High — function confirmed in `commands/shared.rs:20`; usage pattern confirmed in `launch.rs` and `health.rs`.

---

#### A-5: `steam_client_install_path` IPC Input Should Be Treated as Advisory

**Where**: `commands/steam.rs:36` — `list_proton_installs(steam_client_install_path: Option<String>)`.

**Detail**: The migration tool will call `list_proton_installs` (or equivalent) to discover replacement Proton candidates. The frontend passes `steam_client_install_path` as an IPC argument. If an attacker could inject an arbitrary path here, they could point discovery at a crafted directory containing a fake `proton` executable, causing the migration to offer it as a replacement.

**Risk level**: Low. This is a local desktop app with no multi-user or network attack surface; the "attacker" would be the user themselves. Additionally, discovery only reads paths — it doesn't execute them. The fake path would end up in a profile suggestion, which the user must still confirm. A broken launch would result, not privilege escalation.

**Mitigation**: Treat `steam_client_install_path` as advisory — if the provided path doesn't contain a `steamapps` subdirectory, fall back to `default_steam_client_install_path()` rather than scanning an arbitrary directory. The existing `default_steam_client_install_path()` in `steam.rs:9` already validates the path by checking for `candidate.join("steamapps").is_dir()` before returning it. Apply the same validation to the incoming IPC argument.

**Confidence**: Medium — confirmed by reading `steam.rs`; practical exploitability is very low in a local desktop context.

---

#### A-6: Profile TOML Files Have No Explicit Permission Mode

**Where**: `toml_store.rs:117` — `fs::write(path, ...)`.

**Detail**: The SQLite metadata database is created with explicit permissions (mode 0o600 for the file, 0o700 for the directory). Profile TOML files are written with `fs::write()` which inherits the process umask — typically 0o644 on most Linux systems, meaning the files are world-readable.

Profile files may contain full filesystem paths (`local_override.steam.proton_path`, etc.) that reveal the user's home directory layout. This is not a secret per se, but the inconsistency with the DB permissions is worth noting.

**Risk level**: Very low. Profile files are in `~/.config/crosshook/` which is user-owned. Any process running as the same user can already read all files there.

**Mitigation**: Optionally, open the file with explicit 0o600 mode using `OpenOptions`. Not required for the migration feature specifically, but worth tracking as a consistency improvement.

**Confidence**: High — confirmed by comparing `health_store.rs` DB creation (0o600) against `toml_store.rs:save()` which uses plain `fs::write()`.

---

#### A-7: Concurrent Write Race Between User Saves and Batch Migration

**Where**: Batch migration logic + `ProfileStore::save()` (no file locking).

**Detail**: `ProfileStore::save()` has no file locking. If the user edits and saves a profile while a batch migration is processing it concurrently, one write will silently overwrite the other (last-writer-wins). This is the same race that exists for `save_launch_optimizations()` in the existing code — it is consistent with the codebase's current concurrency model.

**Risk level**: Low. Tauri IPC calls are serialized through the async runtime; a concurrent user save during a batch migration would require the user to trigger a save from the UI while the migration dialog's apply operation is in-flight, which is an unusual interaction. No security boundary is crossed — the race only affects which write survives.

**Mitigation**: The batch migration design should hold the migration as a single async operation from the Tauri command layer, reducing the window. No file locking is required for this feature's risk level.

**Confidence**: High — confirmed by reading `toml_store.rs:save()` and the existing `save_launch_optimizations` note in the file.

---

#### A-8: Use `path.try_exists()` for Staleness Detection — Do Not Canonicalize the Stored Path

**Where**: Staleness detection logic (not yet implemented).

**Detail**: The stored Proton path is a literal string that may take the symlinked form (e.g., `~/.steam/steam/steamapps/common/GE-Proton9-4/proton`). Two concerns:

1. **Use `try_exists()` not `exists()`**: `Path::exists()` silently returns `false` on permission errors, making a temporarily inaccessible path look stale. `Path::try_exists()` (stable since Rust 1.63) distinguishes "file not found" (`Ok(false)`) from "I/O error" (`Err`). The migration tool should only suggest a replacement when staleness is definitively confirmed (`Ok(false)`), not when a permission error fires. The rest of the codebase uses `path.exists()` consistently — this is an improvement specific to the migration staleness check where a false positive has real consequences (overwriting a valid profile path).

2. **Do NOT canonicalize before the staleness check**: Resolving the stored symlinked path before checking (e.g., calling `fs::canonicalize()`) would normalize it to the physical path. If the user stored the symlinked form and the symlink still exists, canonicalization would succeed and the path would not appear stale — but if the symlink is later changed (e.g., after ProtonUp-Qt updates GE-Proton), comparing the canonicalized form against the stored literal would produce incorrect results. Check the stored literal path as-is with `try_exists()`.

**Risk level**: Low for security; Medium for correctness. A false positive stale detection could prompt the user to migrate a profile whose Proton installation is actually still valid.

**Mitigation**: `stored_path.try_exists().map(|exists| !exists).unwrap_or(false)` — only flag as stale when existence is definitively `false`.

**Confidence**: High — api-researcher confirmed `~/.steam/steam` is a symlink; `try_exists()` behavior confirmed from Rust stdlib docs.

---

#### A-9: Validate User-Picked Paths from the File Picker Before Writing to Profile

**Where**: "Browse for Proton…" file picker (future UI feature per UX research).

**Detail**: If the migration UI includes a manual "Browse for Proton…" file picker, the user-selected path bypasses the `discover_compat_tools()` validation chain entirely. The backend must apply the same executable check that exists for discovered paths before writing the selection to a profile.

The pattern already exists in the codebase at `update/service.rs:137` and `launch/optimizations.rs:408`:

```rust
metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
```

Apply this same check to any path returned from the Tauri file dialog before accepting it as a valid Proton binary.

**Risk level**: Low. The user is picking their own file on their own filesystem — no privilege escalation vector. The main risk is writing a non-executable path to a profile, which would produce a broken launch. Consistent with the existing behavior where users can type arbitrary paths into the profile editor.

**Mitigation**: Run the file-picker result through the same `check_required_executable()` validation used elsewhere in the codebase before accepting it.

**Confidence**: High — file picker design confirmed by UX research; executable check pattern confirmed in `update/service.rs:137`.

---

#### A-10: WebView Path Display — React Auto-Escaping Is Sufficient, No `dangerouslySetInnerHTML`

**Where**: Future React components for migration confirmation modal.

**Detail**: UX research raised the concern that displaying raw filesystem paths in the Tauri WebView could be an XSS-equivalent vector if paths contain HTML-like characters (e.g., `<script>`). For React: JSX string rendering (`{path}`) automatically HTML-escapes content — there is no XSS risk from string values unless `dangerouslySetInnerHTML` is used.

Confirmed: no `dangerouslySetInnerHTML` or raw `innerHTML` assignments exist anywhere in the current TypeScript codebase. The pattern is safe to follow for the migration modal.

**Risk level**: Very low. Mentioned as a reminder for implementation — do not use `dangerouslySetInnerHTML` for path display.

**Mitigation**: Use standard JSX string rendering (`{path}`) for displaying paths in the migration modal. Do not use raw HTML rendering APIs.

**Confidence**: High — zero `dangerouslySetInnerHTML` occurrences confirmed by codebase grep.

---

## File System Security

### Path Traversal Analysis

**Risk**: None identified for write operations. Profile files are always written via `ProfileStore::profile_path()` which calls `validate_name()` before constructing the path. `validate_name()` (lines 304–329 of `toml_store.rs`) explicitly rejects names containing `/`, `\`, `:`, `.`, and `..`, preventing path traversal through the profile name.

**Risk for read operations**: `safe_enumerate_directories()` in `proton.rs` follows symlinks in Steam directories (see A-1), but this affects only discovery, not write operations. Discovery results are used as candidate replacement paths displayed to the user — the user still confirms before any write.

**Conclusion**: The existing `validate_name()` gate is sufficient protection for write-path safety. The migration tool should not introduce any new path construction that bypasses `validate_name()`.

### TOCTOU Analysis

**Scenario**: Migration tool scans and finds GE-Proton9-7 as a valid replacement. User is shown the confirmation dialog. Between display and confirmation click, the user or Steam uninstalls GE-Proton9-7. Migration applies with a path that no longer exists.

**Risk level**: Low. Post-migration, the profile will show the same "path does not exist" error it showed before — no worse than the original state. No security boundary is crossed.

**Mitigation**: After the user confirms and before the first write, re-check that the replacement path still exists (`Path::new(replacement).try_exists() == Ok(true)`). If it returns `Ok(false)` or `Err`, abort with a clear error rather than writing a newly-broken path.

**Staleness detection**: Use `try_exists()` (not `exists()`) on the stored literal path to detect staleness — see A-8. Do not canonicalize the stored path before the check; the symlinked form must be checked as-is.

### Directory Permissions

**No new risks**: The migration tool reads from `~/.steam/` (user-owned) and writes to `~/.config/crosshook/profiles/` (user-owned). All operations are within the user's own permission scope. No elevated permissions are required or used.

---

## Data Integrity

### Atomic Write Requirements

See **W-1**. Summary: use write-to-temp + `fs::rename()` for all profile writes in the migration path. This is atomic within the same filesystem partition, which is guaranteed here since both temp and target are in `~/.config/crosshook/`.

### Batch Migration Safety

See **W-4**. Summary: use a pre-flight validation pass (serialize all changes, verify all target paths) before beginning any writes. This provides implicit "all or nothing" behavior for most failure modes, with the exception of filesystem-full or permission errors that occur mid-write.

### Health Snapshot Invalidation After Migration

After a successful batch migration, any existing health snapshots for migrated profiles will reflect the pre-migration state (stale path = health issue). These snapshots must be cleared or invalidated so the next health check reflects the updated paths.

The existing SQLite `health_snapshots` table uses `profile_id` as the key (see `health_store.rs`). The migration service should delete health snapshot rows for all successfully migrated profile IDs after applying writes, or mark them as stale. Alternatively, the migration result can carry a list of migrated profile IDs and the health dashboard can re-check them on next open.

An existing `profile_name_history` table tracks profile renames. A similar `migration_events` table (or entries in `launch_operations`) could serve as the audit log for migrations (see A-3).

### Profile Data Loss Prevention

The migration only modifies the Proton path field(s). All other profile content (game executable, trainer, injection, launch settings, etc.) must be preserved byte-for-byte. The correct implementation is:

1. Load profile via `ProfileStore::load()` (returns effective profile)
2. Replace only the `proton_path` field(s) where they match the stale path
3. Save via `ProfileStore::save()` (which serializes via `storage_profile()`)

This ensures the serde round-trip preserves all other fields. Avoid any string-based find-and-replace on the raw TOML — it would be fragile and could corrupt the file.

---

## Input Validation

### Discovered Path Validation

Before offering a path as a migration target, the migration service should verify:

1. `Path::new(path).is_file()` — target exists and is a regular file (follows symlinks; see A-1 for enhancement)
2. The path ends with `/proton` (or matches the Proton binary naming convention) — protects against offering a non-Proton file as a replacement
3. The path lives within a known Steam library root — this is informational, not a hard block, since user-installed GE-Proton can live in `~/.local/share/` or other non-Steam paths

### Version Similarity Scoring

The migration tool must rank replacement candidates by similarity to the stale version (e.g., GE-Proton9-7 should rank higher than Proton Experimental for a stale GE-Proton9-4 path). This ranking is pure string/number comparison and carries no security implications. The existing `normalize_alias()` and `tool_matches_requested_name_heuristically()` functions in `proton.rs` provide a foundation.

No injection risk exists here — the similarity score is computed internally from filesystem-discovered paths and never passed to a shell or SQL query without parameterization.

---

## Dependency Security

### New Dependencies Required

**None.** The entire feature can be implemented using existing dependencies:

| Need                      | Solution                                           | Current dep?   |
| ------------------------- | -------------------------------------------------- | -------------- |
| Path existence checks     | `std::path::Path`                                  | stdlib         |
| Atomic writes             | `std::fs::rename`                                  | stdlib         |
| TOML read/write           | `toml = "0.8"`                                     | Yes            |
| Profile discovery         | `crosshook-core` internal                          | Yes            |
| Temp file creation        | `tempfile = "3"` (or `std::fs` with `.tmp` suffix) | Dev-dep only   |
| Version string comparison | Custom logic on `normalize_alias` output           | Yes (internal) |
| Migration log             | `rusqlite = "0.38"`                                | Yes            |

If `tempfile` is used in production code (not just tests), it should be promoted from `[dev-dependencies]` to `[dependencies]`. Alternatively, the `.tmp`-suffix + rename pattern can be implemented with `std::fs` alone.

### Existing Dependency Audit

The following currently-used crates are relevant to the migration feature:

| Crate      | Version                 | Notes                                                                |
| ---------- | ----------------------- | -------------------------------------------------------------------- |
| `toml`     | `0.8`                   | No known CVEs; actively maintained                                   |
| `serde`    | `1`                     | No known CVEs; industry standard                                     |
| `rusqlite` | `0.38` (bundled SQLite) | Bundled SQLite 3.x; recent versions have no known high-severity CVEs |
| `tempfile` | `3`                     | No known CVEs; dev-dep currently                                     |
| `std::fs`  | stdlib                  | `rename()` is atomic on Linux within same FS per POSIX `rename(2)`   |

No new crate introductions means no new supply chain risk.

---

## Configuration Security

### Safe Defaults

The migration feature must default to **dry-run / preview mode**. The default behavior on detecting stale Proton paths should be: show a notification in the Health Dashboard or on the Profiles page, not silently fix.

The `apply_migration` Tauri command should be structured to require affirmative action:

```rust
// Safe: caller must explicitly pass confirm=true
#[tauri::command]
async fn migrate_proton_paths(plan: MigrationPlan, confirm: bool) -> Result<MigrationResult, String> {
    if !confirm {
        return Ok(MigrationResult::preview_only(plan));
    }
    // ... apply writes
}
```

### Preventing Auto-Migration Without Consent

The startup flow (`src-tauri/src/startup.rs`) must not trigger migration silently. Profile loading at startup should detect stale paths and surface them as health issues (already the health system's job) — not automatically apply any fixes.

### Audit Trail

See A-3. An SQLite `migration_log` table is the recommended approach. Until that table exists, the Tauri command should return a structured `MigrationResult` that the frontend can display as a one-time confirmation summary.

---

## Secure Coding Guidelines

For the implementation team:

1. **Always use `ProfileStore::save()` for writes** — never write raw TOML to profile paths by hand. This ensures `validate_name()` is always called and the storage/effective model is honored.

2. **Operate on effective profiles, write to storage layout** — call `profile.effective_profile()` to detect stale paths, then update the corresponding `local_override` field and call `store.save()`. Do not patch `steam.proton_path` directly (it will be empty in v0.2.4+ profiles).

3. **Use write-to-temp + rename for batch migrations** — see W-1. The pattern is:

   ```rust
   let tmp = path.with_extension("toml.tmp");
   fs::write(&tmp, content)?;
   fs::rename(&tmp, path)?;
   ```

4. **Pre-flight all writes before any write** — validate replacement path existence and serialize all profiles before beginning the write loop. This provides best-effort atomicity for the batch.

5. **Re-validate replacement path at apply time** — re-check `replacement_path.is_file()` immediately before writing, to reduce the TOCTOU window (see TOCTOU Analysis).

6. **Never pass discovered paths to shell commands without validation** — the migration tool itself doesn't exec anything, but the paths written to profiles are later passed to Proton via the launch system. Ensure the replacement path (a) ends with `/proton`, (b) was discovered via `discover_compat_tools`, and (c) was confirmed by the user.

7. **Return `Result<T, E>` with descriptive errors** — follow the existing `ProfileStoreError` pattern. Do not silently swallow errors during batch migration.

8. **Apply `sanitize_display_path()` to all IPC-bound path strings** — import from `commands/shared.rs` and apply to `old_path`, `new_path`, and any profile name references in `MigrationPlan` and `MigrationResult` before returning them from the Tauri command. This is the established pattern in `launch.rs` and `health.rs`.

9. **Validate `steam_client_install_path` IPC argument before scanning** — apply the same `candidate.join("steamapps").is_dir()` guard used in `default_steam_client_install_path()` to any caller-supplied path. Fall back to the default if the argument fails validation rather than scanning an arbitrary path (see A-5).

10. **Invalidate health snapshots for migrated profiles** — after successful writes, delete or mark stale the `health_snapshots` rows for all migrated profile IDs so the next health check reflects the new paths (see Health Snapshot Invalidation section).

11. **Staleness check: use `try_exists()` on the stored literal path** — `path.try_exists()` returns `Ok(false)` only when the path definitively does not exist (vs. `exists()` which swallows permission errors). Only flag a path as stale on `Ok(false)`. Do not canonicalize the stored path before checking — symlinked paths must be checked as-is (see A-8).

12. **File picker paths must pass executable validation** — if the UI includes a "Browse for Proton…" option, the selected path must be validated with `metadata.is_file() && metadata.permissions().mode() & 0o111 != 0` (the pattern used in `update/service.rs:137`) before accepting it. Do not write an unvalidated picker result to a profile (see A-9).

---

## Trade-off Recommendations

| Trade-off                                           | Recommended choice                                | Rationale                                                                                   |
| --------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| Atomic writes (stdlib rename vs tempfile crate)     | stdlib `rename`                                   | No new dependency; POSIX atomic guarantee is sufficient                                     |
| Backup directory vs pre-flight validation           | Pre-flight validation                             | Simpler, no backup dir management, sufficient for this use case                             |
| Audit log now vs later                              | Defer to follow-up                                | Adds scope; health dashboard already surfaces migration state indirectly                    |
| `is_executable()` check on discovered Proton        | Add it (cheap)                                    | Weeds out symlink targets that aren't actually runnable                                     |
| Path display in UI (full vs truncated)              | Use `sanitize_display_path()`                     | `~`-replaces home dir, consistent with launch.rs/health.rs; full paths go in backend struct |
| Staleness check (`exists()` vs `try_exists()`)      | `try_exists()`                                    | Distinguishes "not found" from permission errors; prevents false-positive stale detection   |
| File picker result validation                       | Validate with `is_file()` + executable mode check | Consistent with `update/service.rs:137`; prevents non-Proton path from being written        |
| Symlink in stored path — canonicalize before check? | Do NOT canonicalize                               | Stored symlinked paths must be checked as-is; canonicalization changes the reference        |

---

## Open Questions

1. **Which profile fields contain stale Proton paths?** The stale path may appear in `steam.proton_path`, `runtime.proton_path`, `local_override.steam.proton_path`, and/or `local_override.runtime.proton_path`. The migration needs a clear spec on which fields it scans and patches. (Recommend: scan all four; patch only those that match the stale path.)

2. **How should the migration handle `proton_run` profiles vs `steam_applaunch` profiles?** The `runtime.proton_path` field is used for `proton_run` launch method; `steam.proton_path` (and its local_override counterpart) is used for `steam_applaunch`. Does the migration tool treat them the same? (Likely yes — stale is stale.)

3. **What is the version similarity algorithm?** The feature description says "closest matching replacement." This needs a concrete definition to prevent the migration from suggesting Proton Experimental when the user had GE-Proton. (Suggest: exact family match first (GE → GE, Official → Official), then highest version number within family.)

4. **Does the migration tool handle the case where no replacement exists?** If all installed Proton versions are stale (e.g., user uninstalled everything), the tool should surface a clear "no replacement found" state, not crash or emit an empty migration plan.

5. ~~**Should `local_override.toml` be in scope?**~~ **Resolved**: practices-researcher confirmed that local overrides are stored in the same `.toml` file under the `[local_override]` section — there is no separate file. The migration operates on a single TOML file per profile via `ProfileStore::save()`, which handles the full storage model including `local_override`.
