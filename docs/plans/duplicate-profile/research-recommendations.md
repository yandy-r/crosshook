# Duplicate Profile -- Recommendations & Risk Assessment

## Executive Summary

Profile duplication (#56) is a genuine quick win: the existing `ProfileStore` already provides `load()`, `save()`, and `list()` -- all the primitives needed. The recommended approach is a thin Tauri command (`profile_duplicate`) that composes these primitives with a name-generation helper and conflict guard, plus a "Duplicate" button in `ProfileActions`. The main risks are silent overwrite (ProfileStore has no existence check on save) and launcher slug collision (two profiles pointing at the same game/trainer produce the same launcher slug). Both are solvable with small, contained changes.

---

## Implementation Recommendations

### Recommended Approach: Compose in Tauri Command Layer (Option B)

Add a new `profile_duplicate` Tauri command that:

1. Calls `store.load(source_name)` to get the `GameProfile`
2. Calls `store.list()` to get existing profile names
3. Generates a unique target name via a helper (see naming strategy below)
4. Calls `store.save(target_name, &profile)` to persist the copy
5. Returns the new profile name to the frontend

**Rationale**: This mirrors the existing pattern in `profile_delete` (which composes load + launcher cleanup + delete). It requires zero changes to `crosshook-core` and keeps the quick-win scope tight. The load + save round-trip through `toml::from_str` + `toml::to_string_pretty` validates the profile data and ensures format consistency, which is preferable to raw file copy (see Option C analysis).

### Name Generation Strategy

GNOME-style `(Copy)` naming is recommended over alternatives:

```
"Profile Name" -> "Profile Name (Copy)"
"Profile Name (Copy)" -> "Profile Name (Copy 2)"
"Profile Name (Copy 2)" -> "Profile Name (Copy 3)"
```

**Naming convention comparison** (from external research):

| Method        | Pattern       | UX Quality                                                | Recommendation  |
| ------------- | ------------- | --------------------------------------------------------- | --------------- |
| GNOME-style   | `Name (Copy)` | Good, clear intent                                        | **Recommended** |
| Windows-style | `Name - Copy` | Good but dash conflicts with game names containing dashes | Not recommended |
| Numeric-only  | `Name (2)`    | Ambiguous -- copy vs version?                             | Not recommended |
| UUID suffix   | `Name-a1b2c3` | Bad UX, not human-readable                                | Not recommended |

The name generation helper should:

- Accept the source name and the list of existing names
- Append `(Copy)` as the first attempt
- If that exists, try `(Copy 2)`, `(Copy 3)`, etc.
- Cap the iteration at a reasonable limit (e.g., 100) and return an error if exhausted
- Live in the Tauri command module (not in crosshook-core) since it is presentation-level logic

### Where Name Generation Should Live

In the Tauri command layer (`src-tauri/src/commands/profile.rs`), not in `crosshook-core`. Reasons:

- The naming pattern (`(Copy)`, `(Copy N)`) is a UI/UX convention, not business logic
- The CLI (`crosshook-cli`) would likely want a different UX (explicit `--name` flag)
- Keeping it in the command layer matches how `profile_delete` handles launcher cleanup logic

### Conflict Detection Strategy

**Recommended**: Use `store.list()` to get all existing names, then check the candidate name against the list. This is preferred over `path.exists()` because the name generation loop needs the full list anyway to find a unique name.

**Conflict detection comparison** (from external research):

| Method                          | Race-Safe               | Complexity | Recommended                               |
| ------------------------------- | ----------------------- | ---------- | ----------------------------------------- |
| `store.list()` pre-check        | No (theoretical TOCTOU) | Low        | **Yes**                                   |
| `path.exists()` pre-check       | No (same TOCTOU)        | Low        | Viable for single-name checks             |
| `OpenOptions::create_new(true)` | Yes (kernel-level)      | Medium     | No (overkill for desktop app)             |
| Atomic write crate              | Yes                     | Higher     | No (new dependency for trivial operation) |

The TOCTOU race is theoretical only -- this is a single-user desktop app where concurrent profile operations are not a realistic scenario.

### Quick Wins to Ship Alongside

1. **Conflict detection helper**: A `fn profile_exists(&self, name: &str) -> Result<bool, ProfileStoreError>` on `ProfileStore` would be useful beyond duplicate (e.g., save-as, import). This is the one small addition to crosshook-core worth considering.
2. **Select duplicated profile after creation**: The frontend should call `selectProfile(newName)` after the duplicate command succeeds, so the user lands in the editor for the new copy.

---

## Improvement Ideas

### Ship Alongside (Low Effort)

- **Auto-select duplicated profile**: After duplication, immediately load the new profile in the editor so users can start modifying it. This is just a `selectProfile()` call in the frontend handler.
- **Toast/notification on success**: Show a brief "Duplicated as {newName}" message rather than requiring the user to notice the profile list changed.

### Near-Term Follow-ups

- **Duplicate with modifications**: A "Duplicate and Edit" flow that opens the duplicated profile in a dirty state, pre-focused on a specific section (e.g., launch method). Useful for #50 optimization presets.
- **Keyboard shortcut**: `Ctrl+D` / gamepad button mapping for quick duplication on Steam Deck.
- **Bulk duplicate**: Low priority. No clear user story yet. Could be added later if template/preset workflows demand it.

### Future Enhancements

- **Template profiles**: Read-only base profiles that can only be duplicated, not edited. This would require a new `is_template` field on `GameProfile` and UI guards. Relevant for #42 (override layers) and #50 (optimization presets).
- **Connection to #50 (Optimization Presets)**: The implementation guide states #56 unblocks #50. The workflow would be: duplicate a profile, then change only the launch optimizations to create a variant. This works naturally with the current design since launch optimizations auto-save independently.

---

## Risk Assessment

### Technical Risks

| Risk                                             | Severity | Likelihood | Mitigation                                                                                                                                                                                                                  |
| ------------------------------------------------ | -------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Silent overwrite on save**                     | High     | Medium     | The duplicate command MUST check `store.list()` before calling `store.save()`. `ProfileStore::save()` has no existence guard -- it silently overwrites (toml_store.rs:93-98).                                               |
| **Launcher slug collision**                      | Medium   | Medium     | Two profiles with identical `display_name`, `app_id`, and `trainer_path` generate the same launcher slug. Exporting launchers from both would overwrite each other. Document this; do not block duplication.                |
| **Name validation failure**                      | Low      | Low        | The generated name must pass `validate_name()` (toml_store.rs:189-214). The `(Copy)` suffix uses parentheses which are NOT in the reserved character list, so this is safe.                                                 |
| **TOCTOU race on conflict check**                | Low      | Very Low   | Between `list()` and `save()`, another operation could create a profile with the same name. Acceptable for a single-user desktop app.                                                                                       |
| **Large profile TOML**                           | Low      | Very Low   | `load()` + `save()` round-trips through `toml::from_str` + `toml::to_string_pretty`. This normalizes formatting but is functionally identical. No data loss risk.                                                           |
| **Format migration gap (if raw copy were used)** | Medium   | N/A        | Raw file copy would propagate old TOML formats without migration. The load + save approach avoids this by deserializing and re-serializing through current struct definitions. This reinforces the Option B recommendation. |

### Edge Cases

- **Source profile does not exist**: `store.load()` returns `ProfileStoreError::NotFound`. The Tauri command should propagate this as-is.
- **Source name is empty or invalid**: `store.profile_path()` calls `validate_name()` which rejects empty, `.`, `..`, and path-separator-containing names.
- **Generated name exceeds filesystem limits**: Extremely unlikely since profile names are short, but the helper should cap generated name length.
- **Duplicate of a profile with active launch optimizations autosave**: The autosave timer in `useProfile.ts` (line 639) writes to the _selected_ profile. If the user duplicates and immediately selects the new profile, autosave correctly targets the new profile name.
- **Community-imported profiles**: No special handling needed. Community profiles are stored as regular TOML files with no provenance metadata. The `GameProfile` struct has no `source` or `community_tap_id` field (confirmed in models.rs). Duplicating them is identical to duplicating any other profile.

---

## Alternative Approaches

### Option A: Dedicated `ProfileStore::duplicate()` Method

```rust
pub fn duplicate(&self, source: &str, target: Option<&str>) -> Result<String, ProfileStoreError> {
    let profile = self.load(source)?;
    let target_name = target.map(String::from).unwrap_or_else(|| generate_unique_name(source, &self.list().unwrap_or_default()));
    if self.list()?.contains(&target_name) {
        return Err(ProfileStoreError::AlreadyExists(target_name));
    }
    self.save(&target_name, &profile)?;
    Ok(target_name)
}
```

| Aspect             | Assessment                                                                                                                     |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------ |
| **Pros**           | Encapsulated; single unit-testable method; forces conflict check                                                               |
| **Cons**           | Adds API surface to crosshook-core; name generation is UI logic not business logic; requires new `AlreadyExists` error variant |
| **Effort**         | Low-Medium (new method + new error variant + tests)                                                                            |
| **Recommendation** | Acceptable but over-engineered for a quick win                                                                                 |

### Option B: Compose `load()` + `save()` in Tauri Command (RECOMMENDED)

```rust
#[tauri::command]
pub fn profile_duplicate(name: String, store: State<'_, ProfileStore>) -> Result<String, String> {
    let profile = store.load(&name).map_err(map_error)?;
    let existing = store.list().map_err(map_error)?;
    let new_name = generate_copy_name(&name, &existing);
    store.save(&new_name, &profile).map_err(map_error)?;
    Ok(new_name)
}
```

| Aspect             | Assessment                                                                                                                                                                      |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**           | Zero crosshook-core changes; matches existing command patterns; name generation stays in presentation layer; load+save round-trip validates data and ensures format consistency |
| **Cons**           | Conflict check is manual (no type-system enforcement)                                                                                                                           |
| **Effort**         | Low (~30 lines of Rust + frontend wiring)                                                                                                                                       |
| **Recommendation** | Best fit for quick-win scope                                                                                                                                                    |

### Option C: Filesystem-level TOML Copy

```rust
fs::copy(source_path, target_path)?;
```

| Aspect             | Assessment                                                                                                                                                                              |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**           | Fastest; preserves exact TOML formatting including comments                                                                                                                             |
| **Cons**           | Bypasses all validation; no deserialization check; must manually construct paths and check conflicts; if profile format evolves, raw copy could propagate old formats without migration |
| **Effort**         | Low                                                                                                                                                                                     |
| **Recommendation** | Not recommended. Bypassing the store abstraction is fragile and inconsistent with the rest of the codebase                                                                              |

### Option D: Atomic Write with `OpenOptions::create_new(true)` or External Crate

| Aspect             | Assessment                                                                                                                                                                 |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**           | Kernel-level conflict detection; no TOCTOU race; battle-tested if using `atomicwrites` crate                                                                               |
| **Cons**           | Over-engineered for single-user desktop app; inconsistent with existing `fs::write` usage throughout codebase; adds complexity or a new dependency for a trivial operation |
| **Effort**         | Medium                                                                                                                                                                     |
| **Recommendation** | Not recommended. The theoretical TOCTOU race does not justify the added complexity                                                                                         |

---

## Task Breakdown Preview

### Phase 1: Backend (Rust)

- Add `profile_duplicate` Tauri command in `src-tauri/src/commands/profile.rs`
- Implement `generate_copy_name()` helper (name + existing list -> unique name)
- Register command in `src-tauri/src/lib.rs` invoke handler
- Add unit tests for name generation (edge cases: existing copies, long names, special characters)
- Add integration test for the full duplicate flow in commands/profile.rs

### Phase 2: Frontend Wiring

- Add `duplicateProfile` function to `useProfile.ts` hook (invoke `profile_duplicate`, then `refreshProfiles` + `selectProfile`)
- Expose `duplicateProfile` through `ProfileContext`
- Add `canDuplicate` guard (profile must exist and not be in loading/saving/deleting state)

### Phase 3: UI

- Add "Duplicate" button to `ProfileActions.tsx` (between Save and Delete)
- Wire button to `duplicateProfile` from context
- Add duplicating state indicator (optional: "Duplicating..." button text)

### Phase 4: Testing & Polish

- Run `cargo test -p crosshook-core` (no changes expected, but verify)
- Manual test: duplicate a profile, verify the copy loads correctly
- Manual test: duplicate a profile with launch optimizations, verify they carry over
- Manual test: duplicate when a "(Copy)" profile already exists, verify "(Copy 2)" is generated
- Manual test: gamepad navigation reaches the new Duplicate button

---

## Key Decisions Needed

1. **Conflict resolution strategy**: Auto-increment name (recommended) vs. prompt user for a new name vs. fail with error?
2. **Button placement**: Next to Delete in `ProfileActions` (recommended) vs. in a context menu vs. in the profile selector dropdown?
3. **Post-duplicate behavior**: Auto-select the new profile (recommended) vs. stay on the original?

## Open Questions

1. Should the duplicate command accept an optional `target_name` parameter for cases where the caller wants to specify the name (e.g., future "Save As" feature)? This would be a low-cost generalization.
2. Should we add a `ProfileStore::exists(name)` method to crosshook-core as part of this work? It would benefit other features (import, save-as) but is not strictly required for duplicate.
3. How should the CLI (`crosshook-cli`) expose duplication? Probably `crosshook duplicate <source> [--name <target>]`, but this can be deferred since the CLI is thin.
