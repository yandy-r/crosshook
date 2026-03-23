## Executive Summary

The cleanest implementation is a dedicated install orchestration path that reuses existing Proton discovery, runtime environment assembly, profile persistence, and dialog patterns, but does not overload the current launch commands. Installing a game is a multi-stage workflow with preflight validation, long-running process execution, post-install executable selection, and profile finalization, which is materially different from the current "launch a known executable" path.

The final saved artifact can remain a standard `GameProfile` with `launch.method = proton_run`, so the existing main-tab launch experience does not need to change. The main technical design question is whether draft install state should live only in the frontend or gain a small persisted backend model; the safer recommendation is a lightweight persisted draft so retries and interrupted installs do not throw away user input.

### Architecture Approach

- Introduce a new install domain rather than bolting install behavior into `launch_game`:
  - `crosshook-core/src/install/` for request models, validation, prefix defaults, executable scanning, and installer command building
  - `src-tauri/src/commands/install.rs` for IPC entry points
  - a frontend install sub-view under the existing profile editor for the guided workflow
- Reuse existing components instead of duplicating them:
  - reuse `list_proton_installs` and the existing detected-Proton dropdown pattern
  - reuse `ProfileStore` for final profile save
  - reuse launch-environment logic from `launch/script_runner.rs` where possible, but expose shared helpers rather than calling launch-only functions indirectly
- Keep the final runtime model aligned with existing launch behavior:
  - installation and later launches should both use the same selected Proton executable path
  - the generated profile should populate `runtime.prefix_path`, `runtime.proton_path`, optional `trainer.path`, and `game.executable_path`
- Add a distinct state machine for install:
  - `draft` -> `validated` -> `installing` -> `awaiting_executable_confirmation` -> `ready` or `failed`

### Data Model Implications

- Final profile model:
  - No breaking `GameProfile` schema change is required for v1
  - Existing fields are sufficient for the saved output:
    - `game.name`
    - `game.executable_path`
    - `trainer.path`
    - `runtime.prefix_path`
    - `runtime.proton_path`
    - `launch.method = proton_run`
- New install-specific model:
  - Add a separate `InstallDraft` model instead of polluting `GameProfile` with installer-only fields

| Field             | Type     | Constraints                  | Description                         |
| ----------------- | -------- | ---------------------------- | ----------------------------------- |
| `profile_name`    | `String` | required, valid profile name | User-facing target profile name     |
| `proton_path`     | `String` | required, executable         | Selected Proton executable          |
| `prefix_path`     | `String` | required, absolute path      | Durable Wine prefix root            |
| `installer_path`  | `String` | required, `.exe` file        | Installation media chosen by user   |
| `trainer_path`    | `String` | optional, `.exe` file        | Optional trainer attachment         |
| `final_game_path` | `String` | optional until finalize      | Confirmed installed game executable |
| `status`          | enum     | required                     | Draft/install lifecycle state       |
| `last_error`      | `String` | optional                     | Last failure summary for retry UX   |

- Persistence recommendation:
  - v1 can persist drafts in a small TOML or JSON store under `~/.config/crosshook/install-drafts/`
  - if that feels too heavy for first implementation, persist at least the active draft in settings so interrupted sessions can resume

### API Design Considerations

- Add focused Tauri commands rather than overloading profile or launch commands:

#### `install_validate`

- Purpose: validate install inputs before execution
- Request shape:

```json
{
  "profile_name": "god-of-war-ragnarok",
  "proton_path": "/home/user/.steam/root/steamapps/common/Proton - Experimental/proton",
  "prefix_path": "/home/user/.config/crosshook/prefixes/god-of-war-ragnarok",
  "installer_path": "/mnt/iso/setup.exe",
  "trainer_path": "/games/trainers/gowr-trainer.exe"
}
```

- Response:

```json
{
  "ok": true,
  "default_prefix_used": false,
  "warnings": []
}
```

#### `install_launch_media`

- Purpose: create the prefix if needed and run the installer under Proton
- Behavior:
  - validate inputs
  - create prefix directory if missing
  - spawn installer process with log capture
  - return handle/log metadata for the UI

#### `install_scan_prefix_executables`

- Purpose: propose likely final game executables after installation
- Behavior:
  - scan the prefix for `.exe` files
  - rank results away from obvious uninstallers, crash reporters, and the original installer filename where possible
  - return candidate list for confirmation

#### `install_finalize_profile`

- Purpose: convert a completed draft into a saved `GameProfile`
- Behavior:
  - require confirmed final executable
  - write normal TOML profile via `ProfileStore`
  - optionally delete or mark the draft complete

- Error model:
  - continue the current Tauri convention of `Result<T, String>` for IPC
  - map validation failures to specific field-level messages where feasible so the frontend can anchor errors

### System Constraints

- Process model:
  - installers may exit quickly after spawning child processes or background launchers
  - a simple "process exited means install complete" assumption is unsafe
- Path semantics:
  - current validation requires runtime Proton paths to be executable files and prefix paths to exist as directories
  - install flow needs one extra capability: creating the prefix directory before the installer launches
- Compatibility:
  - some installers may depend on runtime/container behavior closer to `umu-run` than direct `proton run`
  - v1 should document that it targets the common case and retain room for optional `umu-run` support later
- Performance:
  - prefix scans can become expensive if done recursively without filtering
  - scan logic should prefer common install roots first, cap result counts, and avoid crawling obviously irrelevant directories forever
- Testing:
  - frontend has no formal test harness in this repo
  - unit tests should live in `crosshook-core` for default prefix derivation, validation, executable ranking, and profile generation

### File-Level Impact Preview

- Likely files to create:
  - `src/crosshook-native/crates/crosshook-core/src/install/mod.rs`
  - `src/crosshook-native/crates/crosshook-core/src/install/models.rs`
  - `src/crosshook-native/crates/crosshook-core/src/install/runner.rs`
  - `src/crosshook-native/crates/crosshook-core/src/install/scan.rs`
  - `src/crosshook-native/src-tauri/src/commands/install.rs`
  - `src/crosshook-native/src/components/ProfileInstall.tsx`
  - `src/crosshook-native/src/types/install.ts`
- Likely files to modify:
  - `src/crosshook-native/src/components/ProfileEditor.tsx`
  - `src/crosshook-native/src/App.tsx`
  - `src/crosshook-native/src-tauri/src/lib.rs`
  - `src/crosshook-native/crates/crosshook-core/src/lib.rs`
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
  - `src/crosshook-native/src/hooks/useProfile.ts`
- Refactor target:
  - extract shared Proton-environment helpers from `launch/script_runner.rs` so install and launch do not duplicate runtime variable assembly
