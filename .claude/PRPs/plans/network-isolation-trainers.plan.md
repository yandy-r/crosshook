# Plan: Network Isolation for Trainers via `unshare --net`

## Summary

Add a per-profile `network_isolation` toggle (default: `true`) to the `[launch]` section of profile TOML. When enabled, prepend `unshare --net` to the trainer launch command, creating an isolated network namespace with no interfaces. Skip isolation automatically for trainer types that `require_network` (e.g. WeMod) or when `unshare --net` is unavailable (kernel policy), surfacing a non-blocking warning.

## User Story

As a CrossHook user,
I want trainer processes to be isolated from the network by default,
So that third-party Windows executables cannot make outbound connections (telemetry, update checks) without my knowledge.

## Problem -> Solution

Trainers are third-party Windows executables that may include telemetry, update checks, or other network activity. Currently, trainers run with full network access. -> Per-profile toggle prepends `unshare --net` to the trainer command chain, creating an isolated network namespace. Default on. Smart fallback for trainers that need network (Aurora/WeMod) and kernels that block user namespaces.

## Metadata

- **Complexity**: Medium
- **Source PRD**: N/A
- **PRD Phase**: N/A
- **Estimated Files**: 10-12
- **Issue**: #62

---

## UX Design

### Before

```
Profile Editor > Launch section:
  Method: [proton_run]
  Optimizations: [gamemode, mangohud]
  Gamescope: [disabled]
  MangoHud: [disabled]
  (no network isolation toggle)
```

### After

```
Profile Editor > Launch section:
  Method: [proton_run]
  Optimizations: [gamemode, mangohud]
  Network isolation: [ON]        <-- new toggle
  Gamescope: [disabled]
  MangoHud: [disabled]

When trainer_type requires_network=true (e.g. WeMod):
  Network isolation: [OFF] (auto-disabled)
  Helper text: "Disabled — WeMod requires network access"

When unshare --net is unavailable:
  Network isolation: [ON] (grayed out / annotated)
  Helper text: "Unavailable — unprivileged user namespaces disabled"
```

### Interaction Changes

| Touchpoint              | Before                                        | After                                                       | Notes                                                                      |
| ----------------------- | --------------------------------------------- | ----------------------------------------------------------- | -------------------------------------------------------------------------- |
| Profile TOML `[launch]` | No field                                      | `network_isolation = true`                                  | Default true; backward-compatible via `#[serde(default = "default_true")]` |
| Launch preview          | No mention                                    | Shows `unshare --net` in wrapper chain                      | Visible in effective_command string                                        |
| Trainer launch command  | `mangohud gamemoderun proton run trainer.exe` | `unshare --net mangohud gamemoderun proton run trainer.exe` | Prepended as first wrapper                                                 |
| Game launch command     | Unchanged                                     | Unchanged                                                   | Game processes NOT affected                                                |
| Validation              | N/A                                           | Warning when `unshare --net` unavailable                    | Non-blocking warning severity                                              |

---

## Mandatory Reading

| Priority       | File                                                                       | Lines   | Why                                                                                |
| -------------- | -------------------------------------------------------------------------- | ------- | ---------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`         | 307-341 | `LaunchSection` struct — add `network_isolation` field here                        |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | 206-262 | `build_proton_trainer_command` — prepend `unshare --net` to wrappers here          |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | 25-41   | `new_direct_proton_command_with_wrappers` — how wrapper chain is assembled         |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`         | 23-59   | `LaunchRequest` struct — add `network_isolation` field                             |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/offline/trainer_type.rs`   | 36-48   | `TrainerTypeEntry.requires_network` field — used to auto-skip isolation            |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`         | 614-676 | `build_effective_command_string` — include `unshare --net` in preview              |
| P1 (important) | `src/crosshook-native/src/types/profile.ts`                                | 92-161  | Frontend `GameProfile` type — add `network_isolation`                              |
| P2 (reference) | `src/crosshook-native/src-tauri/src/commands/launch.rs`                    | 44-394  | Tauri launch commands — no changes needed, passes `LaunchRequest` through          |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | 126-183 | `AppSettingsData` — reference for how global defaults are structured (not changed) |
| P2 (reference) | `src/crosshook-native/assets/default_trainer_type_catalog.toml`            | all     | Trainer type entries — WeMod has `requires_network = true`                         |

## External Documentation

| Topic           | Source                  | Key Takeaway                                                                                                                                                              |
| --------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unshare --net` | `man 1 unshare` (Linux) | Creates isolated network namespace with no interfaces. Works without root when unprivileged user namespaces are enabled (`kernel.unprivileged_userns_clone=1` or sysctl). |
| User namespaces | kernel docs             | Some distros/kernels disable unprivileged user namespaces for security. The `unshare --net` call will fail with EPERM. Must detect and degrade gracefully.                |

---

## Patterns to Mirror

### NAMING_CONVENTION

// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models.rs:331-337

```rust
/// Per-profile gamescope compositor wrapper configuration.
#[serde(default, skip_serializing_if = "GamescopeConfig::is_default")]
pub gamescope: GamescopeConfig,
```

New field follows the same pattern: `snake_case`, `#[serde(default)]`, doc comment.

### SERDE_DEFAULT_TRUE

// SOURCE: Used for boolean fields that default true (new pattern, justified by issue spec)

```rust
fn default_true() -> bool {
    true
}

fn is_true(v: &bool) -> bool {
    *v
}

#[serde(default = "default_true", skip_serializing_if = "is_true")]
pub network_isolation: bool,
```

Default `true` with `skip_serializing_if = "is_true"` so existing profiles without the field get isolation enabled. The field is omitted from TOML when `true` (the default), keeping profile files clean.

### WRAPPER_CHAIN

// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs:25-41

```rust
pub fn new_direct_proton_command_with_wrappers(proton_path: &str, wrappers: &[String]) -> Command {
    if wrappers.is_empty() {
        let mut command = Command::new(proton_path.trim());
        command.arg("run");
        command.env_clear();
        return command;
    }
    let mut command = Command::new(wrappers[0].trim());
    for wrapper in wrappers.iter().skip(1) {
        command.arg(wrapper.trim());
    }
    command.arg(proton_path.trim());
    command.arg("run");
    command.env_clear();
    command
}
```

`unshare --net` must be prepended BEFORE other wrappers (it wraps the entire sub-process tree).

### LAUNCH_VALIDATION_PATTERN

// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/request.rs:264-275

```rust
/// Available disk space at the launch prefix mount is below warning threshold.
LowDiskSpaceAdvisory {
    available_mb: u64,
    threshold_mb: u64,
    mount_path: String,
},
```

Warning-severity validation errors follow this pattern with message/help/severity.

### TEST_STRUCTURE

// SOURCE: src/crosshook-native/crates/crosshook-core/src/profile/models.rs:658-680

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_profile() -> GameProfile {
        GameProfile { ... }
    }

    #[test]
    fn test_name_describes_behavior() {
        let mut profile = sample_profile();
        // arrange, act, assert
    }
}
```

---

## Files to Change

| File                                                                       | Action | Justification                                                                                          |
| -------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------ |
| `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`         | UPDATE | Add `network_isolation: bool` to `LaunchSection`                                                       |
| `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`         | UPDATE | Add `network_isolation: bool` to `LaunchRequest`; add `UnshareNetUnavailable` validation warning       |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | UPDATE | Prepend `unshare --net` to trainer wrappers when `network_isolation` is true                           |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | UPDATE | Add `is_unshare_net_available()` capability check function                                             |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`         | UPDATE | Include `unshare --net` in effective command preview                                                   |
| `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`   | UPDATE | Add `unshare` to wrapper chain in `resolve_launch_directives` if needed (or handle in `script_runner`) |
| `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs`             | UPDATE | Re-export `is_unshare_net_available` if needed                                                         |
| `src/crosshook-native/src/types/profile.ts`                                | UPDATE | Add `network_isolation?: boolean` to `GameProfile.launch`                                              |
| `src/crosshook-native/src/types/profile.ts`                                | UPDATE | Add default value in `DEFAULT_LAUNCH_SECTION` and `normalizeSerializedGameProfile`                     |

## NOT Building

- Global settings toggle for network isolation (per-profile only, per issue spec)
- `firejail --net=none` alternative (out of scope; `unshare --net` is simpler)
- iptables rules keyed on UID (out of scope; `unshare --net` is simpler)
- Network isolation for game processes (explicitly out of scope per acceptance criteria)
- UI profile editor component changes (no frontend test framework; UI toggle is a separate concern that follows naturally from the type changes)
- New SQLite migration or metadata table (issue confirms existing `launch_operations.diagnostic_json` suffices)

---

## Step-by-Step Tasks

### Task 1: Add `network_isolation` field to `LaunchSection` in profile models

- **ACTION**: Add `network_isolation: bool` field to `LaunchSection` with `default = "default_true"` and `skip_serializing_if = "is_true"` serde attributes.
- **IMPLEMENT**:

  ```rust
  // In profile/models.rs, before LaunchSection:
  fn default_network_isolation() -> bool {
      true
  }

  fn is_default_network_isolation(v: &bool) -> bool {
      *v
  }

  // In LaunchSection struct:
  /// When true, trainer processes are launched in an isolated network namespace
  /// via `unshare --net`, preventing outbound connections.
  #[serde(default = "default_network_isolation", skip_serializing_if = "is_default_network_isolation")]
  pub network_isolation: bool,
  ```

  Also update `Default` for `LaunchSection` (it derives Default, so the `serde(default)` handles it, but verify the `Default` impl sets it to `true` via the serde default function).

  **Important**: `LaunchSection` derives `Default`, which gives `bool` a value of `false`. Since we want `true` as default, we must NOT rely on `#[derive(Default)]` for this field. Instead, implement `Default` manually for `LaunchSection`, OR use the serde default function which handles deserialization. For the `Default` trait (used in code, not just TOML parsing), we need a manual impl or to verify that all code paths that create `LaunchSection` go through TOML deserialization. Since `GameProfile` derives `Default` and `LaunchSection` derives `Default`, we need a **manual `Default` impl** for `LaunchSection` that sets `network_isolation = true`.

  Actually, examining the code: `LaunchSection` currently uses `#[derive(Default)]`. We need to replace this with a manual `Default` impl that sets `network_isolation: true` and all other fields to their current defaults.

- **MIRROR**: SERDE_DEFAULT_TRUE pattern, NAMING_CONVENTION pattern
- **IMPORTS**: None needed
- **GOTCHA**: `LaunchSection` derives `Default` which would set `network_isolation` to `false`. Must switch to manual `Default` impl OR accept that `Default::default()` gives `false` and rely on serde for TOML parsing (which uses `default_network_isolation()`). The safest approach: manual `Default` impl for `LaunchSection`.
- **VALIDATE**:
  - `cargo test -p crosshook-core -- launch_section` passes
  - Add test: profile TOML without `network_isolation` field deserializes to `true`
  - Add test: profile TOML with `network_isolation = false` deserializes correctly
  - Add test: profile TOML with `network_isolation = true` omits the field (skip_serializing_if)

### Task 2: Add `network_isolation` field to `LaunchRequest`

- **ACTION**: Add `network_isolation: bool` to `LaunchRequest` struct with `#[serde(default)]` (default false for requests is fine; the caller sets it from the profile).
- **IMPLEMENT**:

  ```rust
  // In launch/request.rs, LaunchRequest struct:
  #[serde(default)]
  pub network_isolation: bool,
  ```

- **MIRROR**: Same pattern as other bool fields in `LaunchRequest` (e.g., `launch_trainer_only`)
- **IMPORTS**: None
- **GOTCHA**: `LaunchRequest` derives `Default` which gives `false` for `network_isolation`. This is correct for the request — the frontend explicitly sets it from the profile value.
- **VALIDATE**: Existing tests still pass; `LaunchRequest::default()` has `network_isolation: false`.

### Task 3: Add `is_unshare_net_available()` capability check

- **ACTION**: Add a function to `launch/runtime_helpers.rs` that probes whether `unshare --net` is available by attempting a dry-run.
- **IMPLEMENT**:

  ```rust
  /// Returns `true` if `unshare --net` is available for the current user.
  ///
  /// Probes by running `unshare --net true` (which immediately exits).
  /// Returns `false` if the binary is missing or unprivileged user namespaces
  /// are disabled by kernel policy.
  pub fn is_unshare_net_available() -> bool {
      std::process::Command::new("unshare")
          .args(["--net", "true"])
          .stdin(std::process::Stdio::null())
          .stdout(std::process::Stdio::null())
          .stderr(std::process::Stdio::null())
          .status()
          .map(|s| s.success())
          .unwrap_or(false)
  }
  ```

- **MIRROR**: Similar to `is_command_available` in `optimizations.rs`
- **IMPORTS**: `std::process::{Command, Stdio}`
- **GOTCHA**: This spawns a real process. Cache the result if called frequently (e.g., use `OnceLock<bool>`). For now, called once at validation time, so no cache needed. If preview + validate both call it in the same session, consider caching.
- **VALIDATE**: Test on a system with user namespaces enabled — should return `true`. Unit test can mock or be `#[ignore]` for CI.

### Task 4: Add `UnshareNetUnavailable` validation warning

- **ACTION**: Add a new `ValidationError` variant for when `unshare --net` is requested but unavailable. Add it to the validation pipeline for trainer launches.
- **IMPLEMENT**:

  ```rust
  // In ValidationError enum:
  /// `unshare --net` was requested but is not available on this system.
  UnshareNetUnavailable,

  // In message():
  Self::UnshareNetUnavailable => {
      "Network isolation (unshare --net) is not available on this system.".to_string()
  }

  // In help():
  Self::UnshareNetUnavailable => {
      "Unprivileged user namespaces may be disabled by kernel policy. The trainer will launch without network isolation.".to_string()
  }

  // In severity():
  Self::UnshareNetUnavailable => ValidationSeverity::Warning,
  ```

  Add validation check in `collect_proton_issues` and `collect_steam_issues`:

  ```rust
  if request.network_isolation && !request.launch_game_only {
      if !crate::launch::runtime_helpers::is_unshare_net_available() {
          issues.push(ValidationError::UnshareNetUnavailable.issue());
      }
  }
  ```

- **MIRROR**: LAUNCH_VALIDATION_PATTERN (like `LowDiskSpaceAdvisory`, `GamescopeNestedSession`)
- **IMPORTS**: `crate::launch::runtime_helpers::is_unshare_net_available`
- **GOTCHA**: Only check when `network_isolation` is true AND we're not in game-only mode (trainer isolation only). Don't add to `validate()` (fail-fast) — only to `validate_all()` (advisory collection).
- **VALIDATE**: Add test that `UnshareNetUnavailable` has `Warning` severity.

### Task 5: Prepend `unshare --net` to trainer wrapper chain

- **ACTION**: In `build_proton_trainer_command`, prepend `["unshare", "--net"]` to the wrappers list when `request.network_isolation` is true and `unshare --net` is available.
- **IMPLEMENT**:
  In `script_runner.rs`, modify `build_proton_trainer_command`:

  ```rust
  // After resolving directives, before building the command:
  let effective_wrappers = if request.network_isolation
      && crate::launch::runtime_helpers::is_unshare_net_available()
  {
      let mut w = vec!["unshare".to_string(), "--net".to_string()];
      w.extend(directives.wrappers.iter().cloned());
      w
  } else {
      directives.wrappers.clone()
  };
  ```

  Then use `effective_wrappers` instead of `directives.wrappers` in the gamescope/direct command construction.

  **Do NOT modify `build_proton_game_command`** — game processes must not be affected (per acceptance criteria).

- **MIRROR**: WRAPPER_CHAIN pattern
- **IMPORTS**: `crate::launch::runtime_helpers::is_unshare_net_available`
- **GOTCHA**: `unshare --net` must be the OUTERMOST wrapper (before `mangohud`, `gamemoderun`, etc.) so the entire sub-process tree is network-isolated. When gamescope is active, `unshare --net` goes inside the gamescope wrapper (gamescope needs network for Wayland). Actually — `unshare --net` should wrap the proton command, not gamescope. So it should be prepended to the wrappers list that goes between `--` and the proton command. Let me re-examine: in `new_proton_command_with_gamescope`, wrappers go after `--`. So prepending to wrappers is correct — the network namespace isolates the proton+trainer subtree, not gamescope itself.
- **VALIDATE**: Add test that trainer command includes `unshare --net` as first arg when `network_isolation` is true. Add test that game command does NOT include it.

### Task 6: Include network isolation in launch preview

- **ACTION**: When `network_isolation` is true and unshare is available, include `unshare --net` in the effective command string for trainer-only previews.
- **IMPLEMENT**:
  In `preview.rs`, `build_effective_command_string`, for `ProtonRun` method when `request.launch_trainer_only`:

  ```rust
  // Before building parts, if trainer-only and network_isolation:
  if request.network_isolation
      && crate::launch::runtime_helpers::is_unshare_net_available()
  {
      parts.push("unshare".to_string());
      parts.push("--net".to_string());
  }
  ```

  This should be added right after the gamescope args (or at the beginning of the non-gamescope path) and before other wrappers, mirroring what `build_proton_trainer_command` does.

  Also update the `wrappers` field in the preview to include `unshare --net` when applicable.

- **MIRROR**: Preview mirrors runtime command construction
- **IMPORTS**: `crate::launch::runtime_helpers::is_unshare_net_available`
- **GOTCHA**: Preview must match actual runtime behavior exactly. If gamescope is active, `unshare --net` goes after `--` (inside the compositor), same as other wrappers.
- **VALIDATE**: Existing preview tests still pass. Add test that preview for trainer-only with `network_isolation = true` includes `unshare` in effective command.

### Task 7: Update frontend TypeScript types

- **ACTION**: Add `network_isolation?: boolean` to `GameProfile.launch` in the frontend type definitions.
- **IMPLEMENT**:
  In `src/crosshook-native/src/types/profile.ts`:

  ```typescript
  // In GameProfile.launch:
  launch: {
    method: LaunchMethod;
    optimizations: LaunchOptimizations;
    presets?: Record<string, LaunchOptimizations>;
    active_preset?: string;
    custom_env_vars: Record<string, string>;
    network_isolation?: boolean;  // <-- new field
    gamescope?: GamescopeConfig;
    trainer_gamescope?: GamescopeConfig;
    mangohud?: MangoHudConfig;
  };
  ```

  In `DEFAULT_LAUNCH_SECTION`:

  ```typescript
  const DEFAULT_LAUNCH_SECTION: GameProfile['launch'] = {
    method: '',
    optimizations: { enabled_option_ids: [] },
    presets: {},
    active_preset: '',
    custom_env_vars: {},
    network_isolation: true, // <-- default true
  };
  ```

  In `normalizeSerializedGameProfile`, the spread operator handles this naturally since the Rust backend sends the deserialized value.

- **MIRROR**: Frontend type definitions mirror Rust model structs
- **IMPORTS**: None
- **GOTCHA**: Default must be `true` to match Rust side. The `?` makes it optional for backward compat with old serialized profiles.
- **VALIDATE**: TypeScript compilation succeeds (no frontend test framework).

### Task 8: Add unit tests for profile TOML backward compatibility

- **ACTION**: Add tests in `profile/models.rs` verifying backward compatibility and serialization behavior.
- **IMPLEMENT**:

  ```rust
  #[test]
  fn network_isolation_defaults_true_when_absent_from_toml() {
      let toml = r#"
  [game]
  executable_path = "/games/x.exe"
  [trainer]
  path = "/t/y.exe"
  type = "fling"
  [launch]
  method = "proton_run"
  "#;
      let p: GameProfile = toml::from_str(toml).expect("deserialize");
      assert!(p.launch.network_isolation);
  }

  #[test]
  fn network_isolation_false_roundtrips_through_toml() {
      let mut p = sample_profile();
      p.launch.network_isolation = false;
      let s = toml::to_string_pretty(&p).expect("serialize");
      assert!(s.contains("network_isolation = false"));
      let back: GameProfile = toml::from_str(&s).expect("deserialize");
      assert!(!back.launch.network_isolation);
  }

  #[test]
  fn network_isolation_true_omitted_from_toml() {
      let mut p = sample_profile();
      p.launch.network_isolation = true;
      let s = toml::to_string_pretty(&p).expect("serialize");
      assert!(!s.contains("network_isolation"), "true (default) should be omitted: {s}");
  }

  #[test]
  fn launch_section_default_has_network_isolation_true() {
      let launch = LaunchSection::default();
      assert!(launch.network_isolation);
  }
  ```

- **MIRROR**: TEST_STRUCTURE pattern (same as existing TOML roundtrip tests)
- **IMPORTS**: None (within `#[cfg(test)]` module)
- **GOTCHA**: Must verify that `LaunchSection::default()` returns `network_isolation: true` — this confirms the manual `Default` impl is correct.
- **VALIDATE**: `cargo test -p crosshook-core -- network_isolation` all pass.

### Task 9: Add unit tests for trainer command wrapper chain

- **ACTION**: Add tests in `script_runner.rs` verifying that `unshare --net` is prepended to the trainer command and NOT to the game command.
- **IMPLEMENT**:

  ```rust
  #[test]
  fn proton_trainer_command_prepends_unshare_net_when_isolation_enabled() {
      // Setup: create temp dir with proton, trainer, prefix
      // Create request with network_isolation = true
      // Build trainer command
      // Assert first arg is "unshare" with "--net" following
      // (only if is_unshare_net_available() returns true on the test system)
  }

  #[test]
  fn proton_game_command_does_not_include_unshare_net() {
      // Setup: request with network_isolation = true
      // Build game command
      // Assert "unshare" is NOT in the command args
  }

  #[test]
  fn proton_trainer_command_skips_unshare_when_isolation_disabled() {
      // Setup: request with network_isolation = false
      // Build trainer command
      // Assert "unshare" is NOT in the command args
  }
  ```

- **MIRROR**: Existing test pattern in `script_runner.rs` (e.g., `proton_trainer_command_applies_optimization_wrappers_and_env`)
- **IMPORTS**: None (within `#[cfg(test)]` module)
- **GOTCHA**: These tests depend on `is_unshare_net_available()` returning true on the test system. Use `#[cfg_attr(not(feature = "unshare-tests"), ignore)]` or conditionally check inside the test.
- **VALIDATE**: `cargo test -p crosshook-core -- unshare` passes.

---

## Testing Strategy

### Unit Tests

| Test                                   | Input                | Expected Output                      | Edge Case?              |
| -------------------------------------- | -------------------- | ------------------------------------ | ----------------------- |
| TOML without `network_isolation` field | Old profile TOML     | `network_isolation == true`          | Yes - backward compat   |
| TOML with `network_isolation = false`  | Explicit false       | `network_isolation == false`         | No                      |
| TOML roundtrip: true omitted           | Profile with `true`  | Field not in serialized TOML         | No                      |
| TOML roundtrip: false preserved        | Profile with `false` | `network_isolation = false` in TOML  | No                      |
| `LaunchSection::default()`             | None                 | `network_isolation == true`          | Yes - manual Default    |
| Trainer command with isolation=true    | LaunchRequest        | `unshare --net` first in args        | No                      |
| Trainer command with isolation=false   | LaunchRequest        | No `unshare` in args                 | No                      |
| Game command with isolation=true       | LaunchRequest        | No `unshare` in args                 | Yes - game NOT isolated |
| `UnshareNetUnavailable` severity       | Validation error     | `Warning` severity                   | No                      |
| Preview with isolation=true            | LaunchRequest        | `unshare --net` in effective_command | No                      |

### Edge Cases Checklist

- [x] Existing profiles without `network_isolation` field (backward compat via serde default)
- [x] `network_isolation = true` but `unshare` binary missing (degrade gracefully, warning)
- [x] `network_isolation = true` but kernel blocks unprivileged namespaces (degrade gracefully, warning)
- [x] Trainer types with `requires_network = true` (WeMod) — frontend should auto-disable toggle; backend still respects the field value
- [x] Game-only launch with `network_isolation = true` — NO isolation applied to games
- [x] `steam_applaunch` method with trainer launch (bash script path) — `unshare --net` should work in `build_trainer_command` too
- [x] Gamescope active + network isolation — `unshare --net` goes inside gamescope (after `--`)

---

## Validation Commands

### Static Analysis

```bash
# Type check / compile
cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: Zero errors

### Unit Tests

```bash
# Run all crosshook-core tests
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: All tests pass, including new network_isolation tests

### Focused Tests

```bash
# Run only network isolation related tests
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- network_isolation
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- unshare
```

EXPECT: All new tests pass

### Build Verification

```bash
# Full native build
./scripts/build-native.sh --binary-only
```

EXPECT: Build succeeds

### Manual Validation

- [ ] Create a new profile — verify `network_isolation` defaults to `true` (not present in TOML file)
- [ ] Set `network_isolation = false` in profile TOML — verify it persists after save/reload
- [ ] Launch trainer with isolation enabled — verify trainer cannot reach the network
- [ ] Launch game with isolation enabled — verify game CAN reach the network
- [ ] Launch preview shows `unshare --net` in the effective command for trainer-only
- [ ] On a system with user namespaces disabled — verify warning appears and trainer launches without isolation

---

## Acceptance Criteria

- [x] Per-profile network isolation toggle exists (`[launch] network_isolation` in TOML)
- [x] Trainer processes cannot make outbound network connections when enabled
- [x] Game processes are NOT affected by the isolation
- [x] Backward compatible — existing profiles without the field default to `true`
- [x] Degrades gracefully when `unshare --net` is unavailable (non-blocking warning)
- [x] Launch preview reflects the isolation wrapper

## Completion Checklist

- [ ] Code follows discovered patterns (serde default, wrapper chain, validation warning)
- [ ] Error handling matches codebase style (Warning severity for degraded behavior)
- [ ] Tests follow test patterns (TOML roundtrip, command assertion)
- [ ] No hardcoded values
- [ ] No unnecessary scope additions (game isolation, firejail, iptables)
- [ ] Self-contained — no questions needed during implementation

## Risks

| Risk                                                            | Likelihood | Impact   | Mitigation                                                                                                                                         |
| --------------------------------------------------------------- | ---------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `unshare --net` not available on some distros                   | Medium     | Low      | Graceful fallback with warning; tested on common distros                                                                                           |
| `unshare --net` breaks Wine/Proton internals                    | Low        | Medium   | Trainers run inside Proton which uses wineserver; wineserver should work in isolated namespace since it's a local socket. Test with real trainers. |
| Performance overhead of network namespace creation              | Very Low   | Very Low | `unshare` is a lightweight kernel operation; negligible overhead                                                                                   |
| `steam_applaunch` bash scripts don't support wrapper prepending | Low        | Medium   | `build_trainer_command` passes args to bash script; verify the script respects the wrapper chain. May need to handle separately for steam path.    |

## Notes

- The `steam_applaunch` trainer launch path uses `build_trainer_command` which invokes a bash script (`steam-launch-trainer.sh`). The `unshare --net` wrapper needs to be applied inside that script or as a wrapper around the entire script invocation. The simplest approach: if `network_isolation` is true, set an env var (e.g., `CROSSHOOK_NET_ISOLATE=1`) that the bash script checks, or prepend `unshare --net` to the script invocation itself. The `build_trainer_command` creates a `Command::new(BASH_EXECUTABLE)` with the script as arg — so we can't easily prepend wrappers. For steam_applaunch trainer launches, consider passing `--net-isolate` as a script argument and having the script prepend `unshare --net` to the proton command internally.
- Alternative for steam_applaunch: modify `build_trainer_command` to wrap the entire bash invocation: `unshare --net /bin/bash script.sh ...`. This is simpler and doesn't require script changes.
- The `requires_network` field on `TrainerTypeEntry` is already available via the global trainer type catalog. The frontend can use this to auto-disable the toggle and show an explanatory message. The backend does not need to check `requires_network` — it simply respects the `network_isolation` field value.
