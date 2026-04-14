# Spec: Auto-create independent gamescope config for trainer

**Issue:** [#229](https://github.com/yandy-r/crosshook/issues/229)
**Component:** Process (launch/attach), Profiles
**Priority:** Medium
**Labels:** `type:feature`, `area:process`, `area:profiles`

---

## Problem statement

When a game profile enables gamescope, the trainer inherits the game's gamescope config via `effective_trainer_gamescope()`. Because both the game and the trainer run as separate fullscreen gamescope compositor instances on the same desktop, they compete for display resources. Switching focus to the game can cause the trainer's gamescope session to lose its display and exit, killing the trainer.

Trainers still _need_ a gamescope session to function under Proton (they crash without one when the parent game is under gamescope), but they should not compete with the game for fullscreen.

### Current behavior

1. **Profile storage** (`models.rs`): `LaunchSection.trainer_gamescope` is a `GamescopeConfig` (not `Option`), defaulting to `GamescopeConfig::default()` (all disabled/zeroed). Skipped from TOML serialization when it equals default.
2. **Launch request** (`request.rs`): `LaunchRequest.trainer_gamescope` is `Option<GamescopeConfig>`. `effective_trainer_gamescope()` returns `trainer_gamescope` if `Some` and `enabled`, otherwise falls back to the game's `gamescope`.
3. **Frontend** (`ProfileSubTabs.tsx`): The trainer gamescope panel uses `lockedFullscreen`, forcing the fullscreen checkbox on and disabling user edits for that field.
4. **Result**: When the game has gamescope enabled and the trainer has no override, the trainer inherits a clone of the game's config -- including `fullscreen: true` -- creating two competing fullscreen compositors.

---

## Requirements

### Functional

| #   | Requirement                                                                                                                                                                                | Notes                                                                                                                           |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| R1  | When a game profile has gamescope enabled and the trainer has **no** configured gamescope override, auto-generate an independent trainer gamescope config cloned from the game's settings. | "No configured override" = `trainer_gamescope` is default/unset in the profile.                                                 |
| R2  | The auto-generated trainer gamescope config defaults to **windowed** mode (`fullscreen: false`, `borderless: false`).                                                                      | Avoids fullscreen competition with the game's gamescope.                                                                        |
| R3  | The trainer gamescope config must be **visible and editable** in the UI (profile "Gamescope" subtab). Remove the `lockedFullscreen` prop.                                                  | Users can still manually set fullscreen if they want.                                                                           |
| R4  | If the trainer profile already has an explicit gamescope config (enabled or specifically configured by the user), use that -- do **not** overwrite it.                                     | "Explicitly configured" means the user has toggled the trainer gamescope panel at least once, saving a non-default config.      |
| R5  | Auto-generation must work for both `proton_run` and `steam_applaunch` launch methods.                                                                                                      | Both methods flow through `effective_trainer_gamescope()`.                                                                      |
| R6  | Auto-generation must work for both native (host) and Flatpak runtimes.                                                                                                                     | The gamescope argument-building and command construction already abstract over runtime; this requirement ensures no regression. |
| R7  | All other game gamescope settings (resolution, frame rate, FSR, HDR, grab cursor, extra args) are copied from the game config into the auto-generated trainer config as sensible defaults. | Users can tune from there.                                                                                                      |

### Non-functional

| #   | Requirement                                                                                                                                                                                        |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| NF1 | No user-visible data migration required -- profiles without an explicit `trainer_gamescope` section will auto-generate at launch time. Existing profiles with an explicit section are not touched. |
| NF2 | The auto-generation logic must be pure (no I/O side effects) and unit-testable.                                                                                                                    |
| NF3 | Launch preview (`build_launch_preview`) must reflect the auto-generated config so the user sees the actual trainer gamescope args before launching.                                                |

---

## Technical approach

### 1. Core: new helper `auto_trainer_gamescope()`

Add a method on `LaunchRequest` (or a free function in `launch/request.rs`) alongside `effective_trainer_gamescope()`:

```rust
/// Returns the trainer gamescope config to use at launch time.
///
/// Priority:
/// 1. Explicit trainer_gamescope (Some + enabled) -> use as-is
/// 2. Game gamescope enabled + no trainer override -> clone game config with fullscreen/borderless off
/// 3. Game gamescope disabled -> return default (disabled)
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

**Replace** all call-sites of `effective_trainer_gamescope()` in the launch pipeline (`script_runner.rs`, `launcher_store.rs`, `preview.rs`) with `resolved_trainer_gamescope()`. Deprecate or remove the old method.

Mirror the same logic on `LaunchSection` in `profile/models.rs` for the profile-level counterpart.

### 2. Frontend: remove `lockedFullscreen`

- **`GamescopeConfigPanel.tsx`**: Remove the `lockedFullscreen` prop entirely. The fullscreen checkbox becomes a normal user-controlled toggle.
- **`ProfileSubTabs.tsx`**: Remove `lockedFullscreen` from the trainer gamescope panel instantiation. The `enableHint` text stays (explains why the trainer needs gamescope).

### 3. Frontend: surface auto-generated defaults

When the user opens the trainer gamescope tab and `trainer_gamescope` is unset in the profile, the UI should display the auto-generated defaults (cloned from game config with windowed mode) as the initial state. This already partially works because the frontend falls back to a hard-coded default object in `ProfileSubTabs.tsx:230`. Change this fallback to compute from the game's gamescope config:

```typescript
config={
  profile.launch.trainer_gamescope ?? {
    ...profile.launch.gamescope,
    fullscreen: false,
    borderless: false,
  }
}
```

This way the user sees what will actually be used at launch time, and can enable/edit it to persist an explicit override.

### 4. Launch preview alignment

`build_launch_preview()` in `preview.rs` must use `resolved_trainer_gamescope()` so the command preview reflects the auto-generated config. This is automatic if all call-sites are updated per step 1.

---

## Integration points

| System                     | Impact                                                                            |
| -------------------------- | --------------------------------------------------------------------------------- |
| `launch/request.rs`        | New `resolved_trainer_gamescope()`, update `effective_gamescope_config()`         |
| `launch/script_runner.rs`  | Replace `effective_trainer_gamescope()` calls with `resolved_trainer_gamescope()` |
| `launch/preview.rs`        | Same replacement for preview accuracy                                             |
| `export/launcher_store.rs` | Same replacement for exported launcher scripts                                    |
| `profile/models.rs`        | Mirror `resolved_trainer_gamescope()` on `LaunchSection`; update existing tests   |
| `GamescopeConfigPanel.tsx` | Remove `lockedFullscreen` prop                                                    |
| `ProfileSubTabs.tsx`       | Remove `lockedFullscreen`, update fallback default computation                    |
| `LauncherExport.tsx`       | No change needed (reads from profile, which is correct)                           |
| `useProfile.ts`            | No change needed (saves what the user sets)                                       |
| Watchdog (`watchdog.rs`)   | No change needed (watches gamescope PID, agnostic to config)                      |

---

## Persistence & data classification

| Datum                               | Classification               | Notes                                                                                           |
| ----------------------------------- | ---------------------------- | ----------------------------------------------------------------------------------------------- |
| `trainer_gamescope` in profile TOML | **TOML settings** (existing) | Already persisted; no schema change. Profiles without this section auto-generate at runtime.    |
| Auto-generated config               | **Runtime-only**             | Computed at launch time from game config. Not persisted unless user explicitly edits and saves. |

**Migration:** None. The `trainer_gamescope` field already exists and defaults to `GamescopeConfig::default()` when absent. The behavioral change is in how an unset trainer config is resolved at launch time -- from "inherit game config verbatim" to "clone game config with windowed defaults".

**Backward compatibility:** Profiles that already have an explicit `[launch.trainer_gamescope]` section with `enabled = true` are not affected. Profiles without one get improved default behavior.

---

## Risks

| Risk                                                         | Likelihood | Mitigation                                                                                                                                 |
| ------------------------------------------------------------ | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| Windowed gamescope may not work on all compositors/WMs       | Low        | Gamescope windowed is well-tested; user can override to borderless or fullscreen.                                                          |
| Users confused by auto-generated config appearing in UI      | Low        | Enable hint text explains the behavior; config only appears when game gamescope is on.                                                     |
| Breaking change for users who relied on inherited fullscreen | Low        | Only affects cases where the old behavior was broken (competing fullscreen sessions). Users who want fullscreen for both can re-enable it. |

---

## Testing strategy

### Unit tests (Rust)

1. `resolved_trainer_gamescope()` returns explicit trainer config when present and enabled.
2. `resolved_trainer_gamescope()` clones game config with `fullscreen: false` when trainer is unset and game gamescope is enabled.
3. `resolved_trainer_gamescope()` returns default (disabled) when game gamescope is also disabled.
4. Copied fields (resolution, FSR, HDR, extra_args) match game config in auto-generated case.
5. `build_launch_preview()` reflects auto-generated trainer gamescope args.

### Manual/integration tests

1. Launch game (proton_run) with gamescope fullscreen + no trainer gamescope override -> trainer runs in windowed gamescope, game in fullscreen.
2. Same test with `steam_applaunch` method.
3. Same tests under Flatpak runtime.
4. Profile with explicit trainer gamescope config -> auto-generation does not interfere.
5. UI shows editable (not locked) fullscreen checkbox in trainer gamescope panel.
6. Editing trainer gamescope in UI persists to profile TOML.

---

## Success criteria

- Two gamescope sessions (game + trainer) coexist without display resource competition in the default configuration.
- Trainer gamescope is fully editable in the profile UI.
- No regression for profiles that already have explicit trainer gamescope configs.
- All unit tests pass; `cargo test -p crosshook-core` green.
