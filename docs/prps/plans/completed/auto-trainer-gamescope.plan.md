# Plan: Auto-create independent gamescope config for trainer

**Spec:** [`docs/prps/specs/auto-trainer-gamescope.spec.md`](../specs/auto-trainer-gamescope.spec.md)
**Issue:** [#229](https://github.com/yandy-r/crosshook/issues/229)
**Branch:** `feat/229-auto-trainer-gamescope`
**Generated:** 2026-04-13

---

## Summary

Replace the current `effective_trainer_gamescope()` fallback (which clones the game's gamescope config verbatim, including `fullscreen: true`) with a new `resolved_trainer_gamescope()` method that auto-generates a windowed trainer config when no explicit override exists. Remove the `lockedFullscreen` UI lock so users can edit the trainer fullscreen flag. Update the frontend fallback to derive from the game config instead of a hard-coded empty default.

---

## Tasks

### Task 1 — Add `resolved_trainer_gamescope()` to `LaunchRequest`

**File:** `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
**Lines:** 112–118 (current `effective_trainer_gamescope`)

**What to do:**

1. Add a new method `resolved_trainer_gamescope(&self) -> GamescopeConfig` on `impl LaunchRequest` (after the existing `effective_trainer_gamescope`, around line 118):

```rust
/// Returns the trainer gamescope config to use at launch time.
///
/// Priority:
/// 1. Explicit trainer_gamescope (Some + enabled) → use as-is
/// 2. Game gamescope enabled + no trainer override → clone game config with fullscreen/borderless off
/// 3. Game gamescope disabled → return default (disabled)
pub fn resolved_trainer_gamescope(&self) -> GamescopeConfig {
    if let Some(ref tgs) = self.trainer_gamescope {
        if tgs.enabled {
            return tgs.clone();
        }
    }
    if self.gamescope.enabled {
        let mut auto = self.gamescope.clone();
        auto.fullscreen = false;
        auto.borderless = false;
        return auto;
    }
    GamescopeConfig::default()
}
```

2. Deprecate `effective_trainer_gamescope()` by marking it with `#[deprecated]` and a note pointing to `resolved_trainer_gamescope()`. Do **not** remove it yet — we'll clean up after all call sites are migrated.

**Gotchas:**

- The new method returns **owned** `GamescopeConfig` (not `&GamescopeConfig`) because it may construct a new value. All call sites already clone or consume the config, so this is compatible.
- `effective_gamescope_config()` at line 120 delegates to `effective_trainer_gamescope()` for the `launch_trainer_only` path. This must also be updated (see Task 3).

**Verify:** `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` compiles.

---

### Task 2 — Add `resolved_trainer_gamescope()` to `LaunchSection`

**File:** `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
**Lines:** 398–409 (current `effective_trainer_gamescope` on `LaunchSection`)

**What to do:**

1. Add a new method on `impl LaunchSection` alongside the existing one:

```rust
/// Returns the trainer gamescope config to use at launch/export time.
///
/// Priority:
/// 1. Explicit trainer_gamescope (enabled) → use as-is
/// 2. Game gamescope enabled + trainer not configured → clone game config with fullscreen/borderless off
/// 3. Game gamescope disabled → return default (disabled)
pub fn resolved_trainer_gamescope(&self) -> GamescopeConfig {
    if self.trainer_gamescope.enabled {
        return self.trainer_gamescope.clone();
    }
    if self.gamescope.enabled {
        let mut auto = self.gamescope.clone();
        auto.fullscreen = false;
        auto.borderless = false;
        return auto;
    }
    GamescopeConfig::default()
}
```

2. Deprecate the old `effective_trainer_gamescope()` with `#[deprecated]`.

**Type note:** `LaunchSection.trainer_gamescope` is `GamescopeConfig` (non-optional), so the check is `self.trainer_gamescope.enabled` (not `Option` unwrapping). This mirrors the existing method's approach.

**Verify:** `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` compiles.

---

### Task 3 — Migrate all Rust call sites from `effective_trainer_gamescope` → `resolved_trainer_gamescope`

**Files and exact call sites:**

| #   | File                                                                      | Line | Context                                                                     |
| --- | ------------------------------------------------------------------------- | ---- | --------------------------------------------------------------------------- |
| 1   | `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`        | 122  | `effective_gamescope_config()` delegates to `effective_trainer_gamescope()` |
| 2   | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`  | 237  | `build_trainer_command()` — Steam trainer                                   |
| 3   | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`  | 330  | `build_flatpak_steam_trainer_command()`                                     |
| 4   | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`  | 523  | `build_proton_trainer_command()`                                            |
| 5   | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`  | 698  | `build_steam_trainer_helper_arguments()`                                    |
| 6   | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`  | 762  | `build_flatpak_steam_trainer_helper_arguments()`                            |
| 7   | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`        | 270  | `build_launch_preview()` via `effective_gamescope_config()` (indirect)      |
| 8   | `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs` | 314  | `profile.launch.effective_trainer_gamescope()`                              |
| 9   | `src/crosshook-native/src-tauri/src/commands/export.rs`                   | 50   | `profile.launch.effective_trainer_gamescope()`                              |

**What to do for each call site:**

- **Call sites 2–6 (script_runner.rs):** Replace `request.effective_trainer_gamescope()` with `request.resolved_trainer_gamescope()`. Since the old method returns `&GamescopeConfig` and the new returns owned `GamescopeConfig`, adapt the binding:
  - Old: `let tgs = request.effective_trainer_gamescope();` (reference)
  - New: `let tgs = request.resolved_trainer_gamescope();` (owned)
  - The downstream usage checks `tgs.enabled` and reads fields, which works identically on owned vs reference.

- **Call site 1 (request.rs:122 — `effective_gamescope_config`):** Update the `launch_trainer_only` branch to call `resolved_trainer_gamescope()`. Since `effective_gamescope_config()` returns `&GamescopeConfig`, and the new method returns owned, change `effective_gamescope_config` to also return owned `GamescopeConfig`:

```rust
pub fn effective_gamescope_config(&self) -> GamescopeConfig {
    if self.launch_trainer_only {
        self.resolved_trainer_gamescope()
    } else {
        self.gamescope.clone()
    }
}
```

Check downstream callers of `effective_gamescope_config()`:

- `preview.rs:270` — uses it to read fields; owned works fine.
- `request.rs:861` (validate_all) — reads fields; owned works fine.

- **Call sites 8–9 (launcher_store.rs:314, export.rs:50):** Replace `profile.launch.effective_trainer_gamescope()` with `profile.launch.resolved_trainer_gamescope()`. Both return owned `GamescopeConfig`, so the struct field assignment is unchanged.

**Verify:** `cargo check --manifest-path src/crosshook-native/Cargo.toml` (workspace) compiles with no deprecation warnings from our own code (suppress external if needed).

---

### Task 4 — Remove deprecated `effective_trainer_gamescope()` methods

**Files:**

- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` — lines 113–118
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — lines 398–409

**What to do:** Delete both deprecated methods. Confirm no remaining call sites with:

```bash
grep -rn "effective_trainer_gamescope" src/crosshook-native/
```

**Verify:** `cargo check --manifest-path src/crosshook-native/Cargo.toml` compiles cleanly.

---

### Task 5 — Frontend: remove `lockedFullscreen` prop

**Files:**

- `src/crosshook-native/src/components/GamescopeConfigPanel.tsx`
- `src/crosshook-native/src/components/ProfileSubTabs.tsx`

**What to do:**

1. **`GamescopeConfigPanel.tsx`:**
   - Remove `lockedFullscreen` from the `GamescopeConfigPanelProps` interface (line 14).
   - Remove the JSDoc comment for `lockedFullscreen` (line 13).
   - At lines 283–289, change the fullscreen `CheckboxFlag`:
     - `label`: always `'Fullscreen'`
     - `checked`: `config.fullscreen` (remove `lockedFullscreen ||` prefix)
     - `disabled`: `isDisabled` only (remove `|| !!lockedFullscreen`)
   - Remove destructuring of `lockedFullscreen` from the component's props (search for `lockedFullscreen` in the function signature or destructuring).

2. **`ProfileSubTabs.tsx`:**
   - Remove the `lockedFullscreen` prop from the trainer `<GamescopeConfigPanel>` at line 249.

**Verify:** `npx biome check src/crosshook-native/src/components/GamescopeConfigPanel.tsx src/crosshook-native/src/components/ProfileSubTabs.tsx` passes lint.

---

### Task 6 — Frontend: compute trainer gamescope fallback from game config

**File:** `src/crosshook-native/src/components/ProfileSubTabs.tsx`
**Lines:** 229–239 (current hard-coded fallback)

**What to do:**

Replace the hard-coded default object with a computed fallback that clones from the game's gamescope config:

```typescript
config={
  profile.launch.trainer_gamescope ?? {
    ...profile.launch.gamescope,
    enabled: profile.launch.gamescope?.enabled ?? false,
    fullscreen: false,
    borderless: false,
  }
}
```

This ensures the UI shows the same auto-generated defaults the Rust backend will use at launch time (matching `resolved_trainer_gamescope()` logic). If `profile.launch.gamescope` is also undefined, the spread produces `undefined` values for optional fields, and the explicit `enabled: false`, `fullscreen: false`, `borderless: false` provide the required boolean defaults — but check that `GamescopeConfigPanel` handles potentially-undefined numeric fields gracefully (it already does per the existing `?? undefined` patterns in the component).

**Edge case:** If the game gamescope config has `enabled: true` but `gamescope` is missing from the profile (unlikely but possible for newly created profiles), the fallback still produces a valid disabled config.

**Verify:** `npx biome check src/crosshook-native/src/components/ProfileSubTabs.tsx` passes.

---

### Task 7 — Frontend: update `buildLaunchRequest()` fallback

**File:** `src/crosshook-native/src/utils/launch.ts`
**Line:** 50

**Current:**

```typescript
trainer_gamescope: profile.launch.trainer_gamescope ?? DEFAULT_GAMESCOPE_CONFIG,
```

**Change:** No change needed. The Rust backend `resolved_trainer_gamescope()` handles the resolution. When `trainer_gamescope` is `None` (from `DEFAULT_GAMESCOPE_CONFIG` with `enabled: false`), the backend will auto-generate. The frontend fallback to `DEFAULT_GAMESCOPE_CONFIG` ensures a valid `GamescopeConfig` is always sent over IPC, which the backend interprets as "no override" because `enabled == false`.

**However**, verify that `DEFAULT_GAMESCOPE_CONFIG` (with `enabled: false`) maps to `Some(GamescopeConfig { enabled: false, ... })` on the Rust side (not `None`). Looking at `LaunchRequest.trainer_gamescope: Option<GamescopeConfig>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`: the frontend sends the value, serde deserializes it as `Some(...)`. The `resolved_trainer_gamescope()` method checks `tgs.enabled` — when `enabled == false`, it falls through to the auto-generation path. **This is correct.**

**No code change required for this task.** Mark as verified.

---

### Task 8 — Unit tests for `resolved_trainer_gamescope()`

**File:** `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` (test module at bottom)
**File:** `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` (test module at bottom)

**Tests to add in `request.rs` test module:**

```rust
#[test]
fn resolved_trainer_gamescope_uses_explicit_when_enabled() {
    let (_td, mut request) = proton_request();
    request.gamescope = GamescopeConfig {
        enabled: true,
        fullscreen: true,
        internal_width: Some(1920),
        internal_height: Some(1080),
        ..Default::default()
    };
    request.trainer_gamescope = Some(GamescopeConfig {
        enabled: true,
        fullscreen: false,
        internal_width: Some(800),
        internal_height: Some(600),
        ..Default::default()
    });
    let resolved = request.resolved_trainer_gamescope();
    assert!(resolved.enabled);
    assert!(!resolved.fullscreen);
    assert_eq!(resolved.internal_width, Some(800));
    assert_eq!(resolved.internal_height, Some(600));
}

#[test]
fn resolved_trainer_gamescope_auto_generates_windowed_from_game() {
    let (_td, mut request) = proton_request();
    request.gamescope = GamescopeConfig {
        enabled: true,
        fullscreen: true,
        borderless: false,
        internal_width: Some(1920),
        internal_height: Some(1080),
        frame_rate_limit: Some(60),
        fsr_sharpness: Some(5),
        hdr_enabled: true,
        grab_cursor: true,
        extra_args: vec!["--custom".to_string()],
        ..Default::default()
    };
    request.trainer_gamescope = None;
    let resolved = request.resolved_trainer_gamescope();
    assert!(resolved.enabled, "auto-generated config should be enabled");
    assert!(!resolved.fullscreen, "auto-generated config must not be fullscreen");
    assert!(!resolved.borderless, "auto-generated config must not be borderless");
    assert_eq!(resolved.internal_width, Some(1920), "resolution copied from game");
    assert_eq!(resolved.internal_height, Some(1080), "resolution copied from game");
    assert_eq!(resolved.frame_rate_limit, Some(60), "frame rate copied from game");
    assert_eq!(resolved.fsr_sharpness, Some(5), "FSR copied from game");
    assert!(resolved.hdr_enabled, "HDR copied from game");
    assert!(resolved.grab_cursor, "grab_cursor copied from game");
    assert_eq!(resolved.extra_args, vec!["--custom"], "extra_args copied from game");
}

#[test]
fn resolved_trainer_gamescope_returns_default_when_game_disabled() {
    let (_td, mut request) = proton_request();
    request.gamescope = GamescopeConfig::default(); // disabled
    request.trainer_gamescope = None;
    let resolved = request.resolved_trainer_gamescope();
    assert_eq!(resolved, GamescopeConfig::default());
}

#[test]
fn resolved_trainer_gamescope_ignores_disabled_explicit_override() {
    let (_td, mut request) = proton_request();
    request.gamescope = GamescopeConfig {
        enabled: true,
        fullscreen: true,
        internal_width: Some(1920),
        ..Default::default()
    };
    request.trainer_gamescope = Some(GamescopeConfig {
        enabled: false, // explicitly disabled
        fullscreen: true,
        internal_width: Some(800),
        ..Default::default()
    });
    let resolved = request.resolved_trainer_gamescope();
    // Should auto-generate from game since trainer is disabled
    assert!(resolved.enabled);
    assert!(!resolved.fullscreen);
    assert_eq!(resolved.internal_width, Some(1920));
}
```

**Tests to add/update in `models.rs` test module:**

Update existing tests to use `resolved_trainer_gamescope()` and add coverage for the auto-generation case:

```rust
#[test]
fn launch_section_resolved_trainer_gamescope_auto_generates_windowed() {
    let mut launch = LaunchSection::default();
    launch.gamescope = GamescopeConfig {
        enabled: true,
        fullscreen: true,
        borderless: true,
        internal_width: Some(1920),
        internal_height: Some(1080),
        ..GamescopeConfig::default()
    };
    launch.trainer_gamescope = GamescopeConfig::default(); // not configured
    let resolved = launch.resolved_trainer_gamescope();
    assert!(resolved.enabled);
    assert!(!resolved.fullscreen);
    assert!(!resolved.borderless);
    assert_eq!(resolved.internal_width, Some(1920));
    assert_eq!(resolved.internal_height, Some(1080));
}

#[test]
fn launch_section_resolved_trainer_gamescope_returns_default_when_game_disabled() {
    let mut launch = LaunchSection::default();
    launch.gamescope = GamescopeConfig::default(); // disabled
    launch.trainer_gamescope = GamescopeConfig::default();
    let resolved = launch.resolved_trainer_gamescope();
    assert_eq!(resolved, GamescopeConfig::default());
}
```

**Verify:** `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` — all tests pass.

---

### Task 9 — Update existing tests referencing `effective_trainer_gamescope`

**Files:**

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — tests at lines 1107–1138
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` — test at lines 1826–1845
- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` — tests at lines 1351–1401

**What to do:**

1. **`models.rs` tests (lines 1107, 1120):** Update both tests to call `resolved_trainer_gamescope()` instead of `effective_trainer_gamescope()`. The assertions should still pass since:
   - `inherits_from_game_when_trainer_disabled` test: now the resolved version returns a modified clone (windowed), not a verbatim clone. **Update the assertion** to check `!resolved.fullscreen` and `!resolved.borderless` instead of `assert_eq!(resolved, launch.gamescope)`.
   - `prefers_trainer_when_enabled` test: unchanged behavior.

2. **`request.rs` test (line 1826):** This tests validation, which uses `effective_gamescope_config()`. Since we updated that method in Task 3, the test should still pass without changes. Verify.

3. **`preview.rs` tests (lines 1351, 1384):** These test preview output. The `falls_back_to_main_gamescope_when_trainer_disabled` test at line 1384 may need adjustment — it currently expects the preview to contain the game's gamescope args verbatim. With the new logic, it will get a modified config (windowed). **Update the assertion** to expect the auto-generated windowed args.

**Verify:** `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` — all tests pass.

---

### Task 10 — Final verification

**Commands:**

```bash
# Rust build + test
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core

# Rust clippy
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -- -D warnings

# TypeScript lint
cd src/crosshook-native && npx biome check src/

# Grep for any remaining references to deprecated method
grep -rn "effective_trainer_gamescope" src/crosshook-native/
```

All must pass. The grep should return zero results.

---

## Persistence & data classification

| Datum                               | Classification               | Notes                                                                                 |
| ----------------------------------- | ---------------------------- | ------------------------------------------------------------------------------------- |
| `trainer_gamescope` in profile TOML | **TOML settings** (existing) | No schema change. Field already exists and defaults when absent.                      |
| Auto-generated trainer config       | **Runtime-only**             | Computed at launch time from game config. Not persisted unless user explicitly saves. |

**Migration:** None required. Profiles without `[launch.trainer_gamescope]` auto-generate at runtime.
**Backward compatibility:** Profiles with explicit `trainer_gamescope` sections are unaffected.
**Offline behavior:** No change — resolution is purely local computation.
**User visibility:** Users see the resolved config in the Gamescope subtab and can edit it.

---

## Risks

| Risk                                                                                      | Likelihood | Mitigation                                                                                 |
| ----------------------------------------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------ |
| `effective_gamescope_config()` return type change (ref → owned) breaks downstream callers | Low        | All callers read fields; owned works identically. Compiler will flag any borrowing issues. |
| Preview tests expect exact gamescope arg strings                                          | Medium     | Update test assertions in Task 9. Run tests after each change.                             |
| Frontend spread of potentially-undefined gamescope config                                 | Low        | `GamescopeConfigPanel` already handles undefined optional fields.                          |

---

## Files changed (summary)

| File                                                 | Change                                                                                                  |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/request.rs`        | Add `resolved_trainer_gamescope()`, update `effective_gamescope_config()`, remove old method, add tests |
| `crates/crosshook-core/src/profile/models.rs`        | Add `resolved_trainer_gamescope()`, remove old method, update/add tests                                 |
| `crates/crosshook-core/src/launch/script_runner.rs`  | Replace 5 call sites                                                                                    |
| `crates/crosshook-core/src/launch/preview.rs`        | Indirect via `effective_gamescope_config()`; update test assertions                                     |
| `crates/crosshook-core/src/export/launcher_store.rs` | Replace 1 call site                                                                                     |
| `src-tauri/src/commands/export.rs`                   | Replace 1 call site                                                                                     |
| `src/components/GamescopeConfigPanel.tsx`            | Remove `lockedFullscreen` prop and all its usages                                                       |
| `src/components/ProfileSubTabs.tsx`                  | Remove `lockedFullscreen`, update fallback to derive from game config                                   |
