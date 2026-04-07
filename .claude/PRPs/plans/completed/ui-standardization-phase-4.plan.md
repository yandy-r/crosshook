# Plan: UI Standardization Phase 4 — Setup Run EXE/MSI Ad-Hoc Flow

## Summary

Add a Lutris-style **Run EXE / MSI** ad-hoc launcher under the existing **Setup**
sidebar group as a third sub-tab on the Install Game route, alongside _Install
Game_ and _Update Game_. The new flow lets a user pick an arbitrary `.exe` or
`.msi` file, choose the Proton runtime to use, optionally point at an existing
prefix (or create a throwaway one), and stream the helper log live — without
ever forcing profile creation. It reuses the **`update_game` orchestration
stack** (Proton-direct command builder, log streaming, child cancellation,
status events) end-to-end so we ship one new Tauri command pair plus a small
core wrapper, not a parallel launcher subsystem.

## User Story

As a CrossHook user on Linux/Steam Deck who has just downloaded a new trainer
or third-party setup utility, I want to run that arbitrary `.exe` (or `.msi`)
through Proton from the Setup section without having to commit it to a profile
first, so I can test it, install supporting media, or run a one-off
maintenance binary while keeping my profile catalog clean.

## Problem → Solution

CrossHook today only knows how to launch Windows binaries that live inside a
saved profile (Launch tab) or as part of the structured `install_game` /
`update_game` flows that _require_ a profile name and treat the result as a
draft profile. There is no path to run an arbitrary `.exe`/`.msi`
ad-hoc — users currently misuse the Update Game tab as a workaround, which is
semantically wrong (it forces selecting a target profile, expects an existing
prefix, and is labelled "update") → introduce a third Install Page sub-tab
**Run EXE/MSI** that wraps a new `RunExecutableRequest` core service. The
service uses the same `new_direct_proton_command` + `apply_host_environment` +
`apply_runtime_proton_environment` + `attach_log_stdio` stack that
`update_game` uses, but treats the prefix as optional (auto-resolves to
`$XDG_DATA_HOME/crosshook/prefixes/_run-adhoc/<slug>` when blank), accepts
`.exe` _or_ `.msi` (via `msiexec /i`), and never persists a profile. The UI
mirrors the `UpdateGamePanel` shell (intro hero, fields, status card, footer
actions) so terminology, layout, and accessibility match the existing
Setup-group flows from Phases 1–3, and reuses the existing `ProtonPathField`,
`InstallField`, log streaming pattern, and confirmation modal.

## Metadata

- **Complexity**: Medium
- **Source PRD**: N/A (GitHub umbrella issue #163, sub-issue #165)
- **PRD Phase**: `#163` Phase 4 (`#165`)
- **Estimated Files**: 11–13
- **Issue**: [#163](https://github.com/yandy-r/crosshook/issues/163), [#165](https://github.com/yandy-r/crosshook/issues/165)

---

## UX Design

### Before

```text
┌──── Sidebar ────┐  ┌──── Install Page (route: install) ──────────────┐
│ Game            │  │  ┌─ RouteBanner: Setup / Install Game ────────┐ │
│  Library        │  │  └────────────────────────────────────────────┘ │
│  Profiles       │  │  ┌─ Sub-tabs ──────────────────────────────┐    │
│  Launch         │  │  │ [Install Game] [Update Game]            │    │
│ Setup           │  │  └─────────────────────────────────────────┘    │
│  Install Game ◀ │  │  (No third tab — no ad-hoc EXE/MSI surface)     │
│ Dashboards      │  │  (Sidebar parent label duplicates first child)  │
│  Health         │  │                                                  │
│ Community ...   │  │  ✗ Users misuse Update Game for one-off EXE      │
│ Settings        │  │  ✗ Trainer "test before save" requires fake      │
│                 │  │    profile creation                              │
└─────────────────┘  └──────────────────────────────────────────────────┘
```

### After

```text
┌──── Sidebar ────┐  ┌──── Install Page (route: install) ──────────────┐
│ Game            │  │  ┌─ RouteBanner: Setup / Install & Run ───────┐ │
│  Library        │  │  │ (eyebrow stays "Setup", title renamed to   │ │
│  Profiles       │  │  │  "Install & Run", copy mentions ad-hoc run)│ │
│  Launch         │  │  └────────────────────────────────────────────┘ │
│ Setup           │  │  ┌─ Sub-tabs ──────────────────────────────────┐│
│  Install & Run◀ │  │  │ [Install Game] [Update Game] [Run EXE/MSI] ││
│ Dashboards      │  │  └─────────────────────────────────────────────┘│
│  Health         │  │  ┌─ Run EXE/MSI shell (mirrors UpdateGamePanel)┐│
│ Community ...   │  │  │ Hero: "Run an arbitrary Windows executable" ││
│ Settings        │  │  │ Section "Executable":                       ││
└─────────────────┘  │  │   Path field (.exe / .msi) + Browse         ││
                     │  │ Section "Runtime":                          ││
                     │  │   ProtonPathField (reuses installs hook)    ││
                     │  │   Prefix Path field (optional + auto-fill)  ││
                     │  │ Section "Working Directory" (optional)      ││
                     │  │ Status card (stage / log path / errors)     ││
                     │  │ Footer: [Run] [Cancel] [Reset]              ││
                     │  └─────────────────────────────────────────────┘│
                     └──────────────────────────────────────────────────┘
                       (Confirmation modal before run for unknown
                        executables; live console drawer log lines)
```

### Interaction Changes

| Touchpoint            | Before                                                              | After                                                                                                      | Notes                                                                                                                                              |
| --------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Sidebar Setup group   | One entry labelled `Install Game`                                   | Same single entry, **renamed to `Install & Run`**, route now hosts a third sub-tab                         | No new sidebar item — keeps Phase 1 nav contract; rename stops the parent label from duplicating its first child and fits all three sub-tab modes  |
| Install Page sub-tabs | `[Install Game] [Update Game]` (2)                                  | `[Install Game] [Update Game] [Run EXE/MSI]` (3)                                                           | Sub-tab type widened; banner title also updated (see banner row below)                                                                             |
| Banner title          | `Install Game`                                                      | `Install & Run`                                                                                            | Mirrors the new sidebar label so the route identity is consistent across nav and banner; banner _structure_ unchanged (Phase 1 contract preserved) |
| Banner copy           | "Install or update games, then review and save generated profiles." | "Install games, apply updates, or run an arbitrary Windows EXE or MSI without committing it to a profile." | Updated `routeMetadata.ts` summary only (no banner contract change)                                                                                |
| File picker filter    | `.exe` only on installer                                            | `.exe` _and_ `.msi` on Run EXE/MSI tab                                                                     | New filter list passed to existing `chooseFile`                                                                                                    |
| Profile commit        | Always (Install) / Always (Update target)                           | **Never** for Run EXE/MSI                                                                                  | No `persistProfileDraft`, no review modal                                                                                                          |
| Confirmation          | Update has confirmation modal                                       | Run EXE/MSI shows confirmation modal before launching                                                      | Mirrors `UpdateGamePanel` modal                                                                                                                    |
| Cancellation          | `cancel_update` Tauri command                                       | New `cancel_run_executable` command (parallel pattern)                                                     | Reuses Mutex<Option<u32>> pid pattern                                                                                                              |
| Log streaming         | `update-log` / `update-complete` events                             | `run-executable-log` / `run-executable-complete` events                                                    | Mirrors helper exactly                                                                                                                             |
| Reset behavior        | Update reset clears form                                            | Same — no draft to discard                                                                                 | Simpler than install reset flow                                                                                                                    |

---

## Mandatory Reading

Files that **MUST** be read before implementing:

| Priority | File                                                                       | Lines   | Why                                                                                                                                                                                                                                                                                     |
| -------- | -------------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0       | `src/crosshook-native/src-tauri/src/commands/update.rs`                    | 1-170   | The exact Tauri command + log streaming pattern to mirror, including `UpdateProcessState` PID tracking and the `spawn_log_stream` / `stream_log_lines` helpers. New code will be a near-copy adapted to a wider executable filter and optional prefix.                                  |
| P0       | `src/crosshook-native/crates/crosshook-core/src/update/service.rs`         | 1-380   | Core service shape (validate → build command → spawn child → return result+child). Run EXE service will follow the same return tuple `(RunExecutableResult, tokio::process::Child)`. Read tests carefully — they show how to script a fake Proton binary in tempdir.                    |
| P0       | `src/crosshook-native/crates/crosshook-core/src/update/models.rs`          | 1-131   | `UpdateGameRequest`, `UpdateGameValidationError`, error → message mapping. Pattern for new `RunExecutableRequest` / `RunExecutableValidationError`.                                                                                                                                     |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | 1-300   | The shared Proton-direct primitives (`new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`). Run EXE service will compose these in the same order.                                                   |
| P0       | `src/crosshook-native/src/components/UpdateGamePanel.tsx`                  | 1-280   | The UI shell to mirror — intro hero, install-section pattern, footer actions, confirmation modal, cover-art-aware hero treatment (we will use a simpler eyebrow/title since there is no profile context).                                                                               |
| P0       | `src/crosshook-native/src/hooks/useUpdateGame.ts`                          | 1-370   | Hook shape: request state, validation state, stage state, listen-before-invoke ordering, cancel/reset semantics. New `useRunExecutable` will follow the same scaffolding.                                                                                                               |
| P0       | `src/crosshook-native/src/components/pages/InstallPage.tsx`                | 1-450   | The host page that now needs a third sub-tab. Shows the Tabs pattern, scroll shell wrapper, banner placement, and how `forceMount` + `display:none` is used to retain hidden tab state.                                                                                                 |
| P0       | `src/crosshook-native/src-tauri/src/lib.rs`                                | 196-329 | Where new commands must be `manage`d and registered in `invoke_handler!`. Copy the pattern used for `UpdateProcessState`.                                                                                                                                                               |
| P1       | `src/crosshook-native/src/components/ui/InstallField.tsx`                  | 1-58    | Existing field control with browse button + filters — reuse as-is.                                                                                                                                                                                                                      |
| P1       | `src/crosshook-native/src/components/ui/ProtonPathField.tsx`               | 1-78    | Reusable Proton picker — accepts the proton installs list and an `idPrefix` for unique IDs. We will pass `idPrefix="run-exec"`.                                                                                                                                                         |
| P1       | `src/crosshook-native/src/utils/dialog.ts`                                 | 1-67    | The `chooseFile` helper — pass `[{ name: 'Windows Executable', extensions: ['exe', 'msi'] }]`.                                                                                                                                                                                          |
| P1       | `src/crosshook-native/src/hooks/useProtonInstalls.ts`                      | all     | Returns `installs` + `error`; already used by `InstallPage` — pass through to the new panel.                                                                                                                                                                                            |
| P1       | `src/crosshook-native/src/components/layout/routeMetadata.ts`              | 26-103  | Update the `install` entry's `navLabel`, `bannerTitle`, and `bannerSummary` so the sidebar entry and route banner read `Install & Run` and the summary advertises ad-hoc EXE/MSI execution. `ROUTE_NAV_LABEL` derives from `ROUTE_METADATA` so it picks up the new label automatically. |
| P1       | `src/crosshook-native/crates/crosshook-core/src/install/service.rs`        | 1-150   | For the _prefix provisioning_ pattern (`provision_prefix`, `resolve_default_prefix_path`, slugify) — Run EXE/MSI will reuse the same approach but under a `_run-adhoc` namespace.                                                                                                       |
| P1       | `src/crosshook-native/src-tauri/src/commands/shared.rs`                    | 1-54    | `create_log_path("run-executable", &slug)` and `slugify_target` are reused.                                                                                                                                                                                                             |
| P1       | `src/crosshook-native/src/styles/theme.css`                                | 540-630 | Existing install-page-tabs CSS is already row-gap-friendly when wrapped — sub-tab CSS does not need changes for the third tab, but verify wrapping behavior on Steam Deck width.                                                                                                        |
| P2       | `src/crosshook-native/src/types/install.ts`                                | all     | Just to confirm naming/serialization conventions for new types.                                                                                                                                                                                                                         |
| P2       | `.claude/PRPs/plans/completed/ui-standardization-phase-3.plan.md`          | 1-200   | Confirms the InstallPage is the consolidated host for all Setup-group flows; Phase 4 builds on that decision.                                                                                                                                                                           |

## External Documentation

| Topic                       | Source                                                            | Key Takeaway                                                                                                                                                                                                                                                                                             |
| --------------------------- | ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `msiexec` Proton invocation | Wine docs (`msiexec`), Proton's `bin/wine` symlink                | Inside a Proton prefix, `msiexec /i installer.msi /qb` runs an MSI installer. Our service must detect `.msi` extension and rewrite the Proton arg list from `[<exe>]` to `["msiexec", "/i", "<msi>", "/qb"]`. Proton's `proton run msiexec ...` resolves `msiexec` from inside the prefix automatically. |
| `tauri::Emitter` events     | Tauri 2 docs                                                      | We already use `app.emit("update-log", line)` etc. — pattern is identical for new `run-executable-log` / `run-executable-complete` events.                                                                                                                                                               |
| Tauri dialog filters        | `@tauri-apps/plugin-dialog`                                       | `filters: [{ name, extensions }]` — extensions are without the dot. Use `['exe', 'msi']`.                                                                                                                                                                                                                |
| Phase 1 banner contract     | `.claude/PRPs/plans/completed/ui-standardization-phase-1.plan.md` | The `RouteBanner` is the only top-level identity surface; sub-tabs render _inside_ the route shell. Phase 4 must not introduce a second banner.                                                                                                                                                          |

---

## Patterns to Mirror

### NAMING_CONVENTION (TypeScript)

```ts
// SOURCE: src/crosshook-native/src/hooks/useUpdateGame.ts:1-50
import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
// camelCase hooks, PascalCase exported types, snake_case fields on Tauri DTOs
export interface UseUpdateGameResult {
  request: UpdateGameRequest;
  validation: UpdateGameValidationState;
  stage: UpdateGameStage;
  // ...
}
```

### NAMING_CONVENTION (Rust)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/update/models.rs:7-19
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameRequest {
    #[serde(default)]
    pub profile_name: String,
    #[serde(default)]
    pub updater_path: String,
    // snake_case all the way; #[serde(default)] on every string so empty
    // payloads from the frontend deserialize cleanly.
}
```

### ERROR_HANDLING (Rust core service)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/update/service.rs:15-45
pub fn validate_update_request(
    request: &UpdateGameRequest,
) -> Result<(), UpdateGameValidationError> {
    validate_updater_path(request.updater_path.trim())?;
    validate_proton_path(request.proton_path.trim())?;
    validate_prefix_path(request.prefix_path.trim())?;
    Ok(())
}

pub fn build_update_command(
    request: &UpdateGameRequest,
    log_path: &Path,
) -> Result<Command, UpdateGameError> {
    let mut command = new_direct_proton_command(request.proton_path.trim());
    command.arg(request.updater_path.trim());
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(
        &mut command,
        request.prefix_path.trim(),
        request.steam_client_install_path.trim(),
    );
    apply_working_directory(&mut command, "", Path::new(request.updater_path.trim()));
    attach_log_stdio(&mut command, log_path).map_err(|error| {
        UpdateGameError::LogAttachmentFailed {
            path: log_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    Ok(command)
}
```

### ERROR_HANDLING (Tauri command)

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/update.rs:25-58
#[tauri::command]
pub async fn validate_update_request(request: UpdateGameRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_update_request_core(&request).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn update_game(
    app: AppHandle,
    state: tauri::State<'_, UpdateProcessState>,
    request: UpdateGameRequest,
) -> Result<UpdateGameResult, String> {
    let slug = slugify_target(&request.profile_name, "update");
    let log_path = create_log_path("update", &slug)?;

    let log_path_clone = log_path.clone();
    let (result, child) = tauri::async_runtime::spawn_blocking(move || {
        update_game_core(&request, &log_path_clone).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())??;

    if let Some(pid) = child.id() {
        *state.pid.lock().unwrap() = Some(pid);
    }

    spawn_log_stream(app, log_path, child, "update-log", "update-complete");
    Ok(result)
}
```

### LOG_STREAMING / CANCELLATION (Rust)

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/update.rs:60-170
#[tauri::command]
pub async fn cancel_update(state: tauri::State<'_, UpdateProcessState>) -> Result<(), String> {
    let pid = state.pid.lock().unwrap().take();
    if let Some(pid) = pid {
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .status();
    }
    Ok(())
}

fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    event_name: &'static str,
    complete_event_name: &'static str,
) { /* polls log file every 500ms, emits each new line, emits exit code on completion */ }
```

**KEY INSIGHT**: `spawn_log_stream` and `stream_log_lines` are _already_
generic over event names. The new module can either (a) re-import them as
public helpers or (b) duplicate the same fn body. Prefer **(a) — promote
`spawn_log_stream` to a shared module** so we have one log streamer powering
both `update` and `run-executable`.

### LISTEN-BEFORE-INVOKE (TypeScript hook)

```ts
// SOURCE: src/crosshook-native/src/hooks/useUpdateGame.ts:225-265
let completedBeforeInvoke = false;
try {
  const unlisten = await listen<number | null>('update-complete', (event) => {
    completedBeforeInvoke = true;
    const exitCode = event.payload;
    if (exitCode === 0) {
      setStage('complete');
    } else if (exitCode === null) {
      setStage('failed');
      setError('Update process was terminated by a signal.');
    } else {
      setStage('failed');
      setError(`Update process exited with code ${exitCode}.`);
    }
    unlistenRef.current = null;
    unlisten();
  });
  unlistenRef.current = unlisten;

  const updateResult = await invoke<UpdateGameResult>('update_game', { request });
  setResult(updateResult);
  if (!completedBeforeInvoke) {
    setStage('running_updater');
  }
} catch (invokeError) {
  // ...
}
```

**GOTCHA**: Always subscribe to the completion event _before_ `invoke()` —
otherwise a fast-finishing process can race past the listener registration.

### CONFIRMATION_MODAL (TypeScript component)

```tsx
// SOURCE: src/crosshook-native/src/components/UpdateGamePanel.tsx:243-273
{
  showConfirmation && (
    <div className="crosshook-modal-overlay" onClick={() => setShowConfirmation(false)}>
      <div className="crosshook-modal-dialog" onClick={(e) => e.stopPropagation()}>
        <h4>Apply update to {selectedProfile}?</h4>
        <p>This will run {fileNameFromPath(request.updater_path)} inside the Proton prefix...</p>
        <div style={{ display: 'flex', gap: 12, justifyContent: 'flex-end', marginTop: 16 }}>
          <button
            type="button"
            className="crosshook-button crosshook-button--secondary"
            onClick={() => setShowConfirmation(false)}
            autoFocus
          >
            Cancel
          </button>
          <button
            type="button"
            className="crosshook-button"
            onClick={() => {
              setShowConfirmation(false);
              void startUpdate();
            }}
          >
            Apply Update
          </button>
        </div>
      </div>
    </div>
  );
}
```

### TAB_HOSTING (TypeScript host page)

```tsx
// SOURCE: src/crosshook-native/src/components/pages/InstallPage.tsx:333-372
<Tabs.Root
  className="crosshook-install-page-tabs__root"
  value={installPageTab}
  onValueChange={(value) => setInstallPageTab(value as InstallPageTab)}
>
  <Tabs.List className="crosshook-subtab-row" aria-label="Install page sections">
    <Tabs.Trigger value="install" className="crosshook-subtab">
      Install Game
    </Tabs.Trigger>
    <Tabs.Trigger value="update" className="crosshook-subtab">
      Update Game
    </Tabs.Trigger>
  </Tabs.List>
  <Tabs.Content
    value="install"
    forceMount
    className="..."
    style={{ display: installPageTab === 'install' ? undefined : 'none' }}
  >
    {/* Install panel */}
  </Tabs.Content>
  <Tabs.Content
    value="update"
    forceMount
    className="..."
    style={{ display: installPageTab === 'update' ? undefined : 'none' }}
  >
    {/* Update panel */}
  </Tabs.Content>
</Tabs.Root>
```

**KEY**: `forceMount` + `display:none` means hidden tabs retain state and any
in-flight log subscriptions. Run EXE/MSI follows the exact same pattern.

### TEST_STRUCTURE (Rust core)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/update/service.rs:148-380
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_executable_script(path: &Path, body: &str) {
        fs::write(path, body).expect("write executable script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(path).expect("script metadata").permissions();
            permissions.set_mode(permissions.mode() | 0o111);
            fs::set_permissions(path, permissions).expect("set executable permissions");
        }
    }

    fn valid_request(temp_dir: &Path) -> UpdateGameRequest { /* ... */ }

    #[test]
    fn validate_update_request_accepts_valid_request() {
        let temp_dir = tempdir().expect("temp dir");
        let request = valid_request(temp_dir.path());
        assert!(validate_update_request(&request).is_ok());
    }

    #[test]
    fn validate_update_request_rejects_empty_updater_path() { /* ... */ }
    // table of negative cases per validation variant
}
```

---

## Files to Change

### Backend (Rust)

| File                                                                       | Action | Justification                                                                                                                                                                                                                                                                                                                       |
| -------------------------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/mod.rs`     | CREATE | New `pub mod` for the ad-hoc executable runner. Mirrors `update/mod.rs` re-export structure.                                                                                                                                                                                                                                        |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/models.rs`  | CREATE | `RunExecutableRequest`, `RunExecutableResult`, `RunExecutableError`, `RunExecutableValidationError`, plus the message mapping. Mirrors `update/models.rs`.                                                                                                                                                                          |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/service.rs` | CREATE | `validate_run_executable_request`, `build_run_executable_command`, `run_executable` (returns `(RunExecutableResult, tokio::process::Child)`), `resolve_default_adhoc_prefix_path`. Validates `.exe` _or_ `.msi`, builds the appropriate Proton arg list, optionally provisions a default ad-hoc prefix, and reuses runtime helpers. |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs`                    | UPDATE | Add `pub mod run_executable;` export so `crosshook_core::run_executable::*` is reachable from `src-tauri`.                                                                                                                                                                                                                          |
| `src/crosshook-native/src-tauri/src/commands/run_executable.rs`            | CREATE | Tauri handlers `validate_run_executable_request`, `run_executable`, `cancel_run_executable`, plus a `RunExecutableProcessState` (Mutex<Option<u32>>). Reuses `spawn_log_stream` (newly promoted to `mod log_stream`) emitting `run-executable-log` and `run-executable-complete`.                                                   |
| `src/crosshook-native/src-tauri/src/commands/log_stream.rs`                | CREATE | Extracts `spawn_log_stream` + `stream_log_lines` from `update.rs` into a shared helper module so `update.rs` and `run_executable.rs` both call it. **Behavior preserved exactly** — only the location changes.                                                                                                                      |
| `src/crosshook-native/src-tauri/src/commands/update.rs`                    | UPDATE | Replace local `spawn_log_stream` / `stream_log_lines` with `use super::log_stream::spawn_log_stream;`. Behavior unchanged; existing tests/runtime stay green.                                                                                                                                                                       |
| `src/crosshook-native/src-tauri/src/commands/mod.rs`                       | UPDATE | Add `pub mod run_executable;` and `mod log_stream;` (private — only `update.rs` and `run_executable.rs` import it).                                                                                                                                                                                                                 |
| `src/crosshook-native/src-tauri/src/lib.rs`                                | UPDATE | (1) `.manage(commands::run_executable::RunExecutableProcessState::new())`; (2) register `commands::run_executable::validate_run_executable_request`, `commands::run_executable::run_executable`, `commands::run_executable::cancel_run_executable` in `invoke_handler!`.                                                            |

### Frontend (TypeScript / React)

| File                                                          | Action | Justification                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/types/run-executable.ts`            | CREATE | `RunExecutableRequest`, `RunExecutableResult`, `RunExecutableStage`, `RunExecutableValidationState`, `RUN_EXECUTABLE_VALIDATION_MESSAGES`, `RUN_EXECUTABLE_VALIDATION_FIELD`. Mirrors `types/install.ts` style and the existing update-game type module.                                                                                                                                                                                                                      |
| `src/crosshook-native/src/types/index.ts`                     | UPDATE | Re-export the new run-executable types if `index.ts` aggregates them (verify during impl).                                                                                                                                                                                                                                                                                                                                                                                    |
| `src/crosshook-native/src/hooks/useRunExecutable.ts`          | CREATE | Hook that owns request state, validation, stage, log path, action label, listen-before-invoke flow, cancel and reset. Mirrors `useUpdateGame` but: no `loadProfiles`, no `populateFromProfile`, no profile-cover logic, optional prefix path.                                                                                                                                                                                                                                 |
| `src/crosshook-native/src/components/RunExecutablePanel.tsx`  | CREATE | The panel UI. Renders the intro hero (no cover art branch — there is no profile context), an `Executable` section (`InstallField` with `extensions: ['exe', 'msi']`), a `Runtime` section (`ProtonPathField` + optional `Prefix Path` `InstallField` + optional `Working Directory` `InstallField`), a status card, and a footer with `Run`, `Cancel`, `Reset` buttons plus the confirmation modal. Uses `crosshook-install-shell` classes for parity with `UpdateGamePanel`. |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`   | UPDATE | (1) Widen `InstallPageTab` to `'install'                                                                                                                                                                                                                                                                                                                                                                                                                                      | 'update' | 'run_executable'`; (2) add a third `Tabs.Trigger`"Run EXE/MSI"; (3) add a third`Tabs.Content`mounting`<RunExecutablePanel protonInstalls={protonInstalls} protonInstallsError={protonInstallsError} />`. |
| `src/crosshook-native/src/components/layout/routeMetadata.ts` | UPDATE | Rename the `install` entry: `navLabel` and `bannerTitle` → `Install & Run`, and `bannerSummary` → copy that mentions installing, updating, and running ad-hoc EXE/MSI. `sectionEyebrow` (`Setup`), the route key, and the `Art` icon are unchanged. No structural change to `RouteMetadataEntry`. `ROUTE_NAV_LABEL` derivation requires no manual update — it reads from `ROUTE_METADATA.install.navLabel`.                                                                   |

### Tests

| File                                                                                                  | Action                | Justification                                                                                                                                                                                                                                                                                            |
| ----------------------------------------------------------------------------------------------------- | --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/service.rs` (`#[cfg(test)] mod tests`) | CREATE (in same file) | Mirrors update tests: valid request accepted, every validation variant rejected, MSI detection switches arg list, default ad-hoc prefix is created when `prefix_path` is empty, `build_run_executable_command` references the proton path, `run_executable` rejects an invalid request without spawning. |
| `src/crosshook-native/crates/crosshook-core/src/run_executable/models.rs` (`#[cfg(test)] mod tests`)  | CREATE                | Round-trip Serde of `RunExecutableRequest` with mixed empty / populated fields; verify each `RunExecutableValidationError` produces the expected human message.                                                                                                                                          |

### NOT Building

- **No new sidebar item.** The Setup group keeps a single entry — relabelled from `Install Game` to `Install & Run` — and the new flow is a sub-tab on the same route. (Issue #165 explicitly says "discoverability level as existing setup actions" — sub-tab parity is the lowest-friction interpretation and avoids re-doing Phase 1 banner contract work. The label rename is a copy change, not a contract change.)
- **No persisted ad-hoc run history.** All run state is runtime-only this phase. Storage boundary stays at runtime memory; no SQLite or TOML changes. (Issue #165 leaves history persistence optional.)
- **No "save as profile" affordance** from the Run EXE panel. If a user decides they want a profile, they switch to the Install Game tab. Keeping these flows separate avoids leaking the profile review draft session (Phase 3) into an ad-hoc surface.
- **No new file picker UI.** We reuse `chooseFile` with the existing dialog plugin and just widen the filter list.
- **No new log streamer.** We promote `spawn_log_stream` to `commands/log_stream.rs` and _reuse_ it. Not duplicated.
- **No CLI parity.** `crosshook-cli` is intentionally untouched; this is a UI-only phase. (CLI ad-hoc support, if ever wanted, is its own issue.)
- **No automatic prefix dependency installation.** The Run EXE flow uses whatever the user's prefix already has. Phase 4 does not invoke `prefix_deps`.
- **No metadata DB row for ad-hoc runs.** Runtime-only by design (issue #165 default).
- **No banner restructure.** Phase 1 banner contract is final.
- **No "Open Log" or in-app log viewer.** The status card surfaces the log path; the existing console drawer already streams `run-executable-log` events.

---

## Step-by-Step Tasks

### Phase A — Core service

#### Task A1: Create `run_executable` core module skeleton

- **ACTION**: Create `crates/crosshook-core/src/run_executable/{mod.rs, models.rs, service.rs}` and add `pub mod run_executable;` to `crates/crosshook-core/src/lib.rs`.
- **IMPLEMENT**: `mod.rs` re-exports the public types (`RunExecutableRequest`, `RunExecutableResult`, `RunExecutableError`, `RunExecutableValidationError`) and functions (`run_executable`, `validate_run_executable_request`, `build_run_executable_command`, `resolve_default_adhoc_prefix_path`). Empty submodules for now — fill in next tasks.
- **MIRROR**: `crates/crosshook-core/src/update/mod.rs:1-7`
- **IMPORTS**: none
- **GOTCHA**: Confirm `crates/crosshook-core/src/lib.rs` actually has the `pub mod update;` line and add `pub mod run_executable;` _next to it_; do not insert it inside an unrelated `cfg`.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` succeeds.

#### Task A2: Define `RunExecutableRequest`, results, and errors

- **ACTION**: Fill `run_executable/models.rs`.
- **IMPLEMENT**:
  - `RunExecutableRequest { executable_path: String, proton_path: String, prefix_path: String /* optional */, working_directory: String, steam_client_install_path: String }` — all `#[serde(default)]`, `Default`, `Clone`, `Eq`, `Serialize`, `Deserialize`.
  - `RunExecutableResult { succeeded: bool, message: String, helper_log_path: String, resolved_prefix_path: String }` — `resolved_prefix_path` echoes the actual prefix used (helpful for the UI when we auto-resolved a default).
  - `RunExecutableError` enum mirroring `UpdateGameError` (`Validation`, `RuntimeUnavailable`, `LogAttachmentFailed`, `RunnerSpawnFailed`, `RunnerWaitFailed`, `RunnerExitedWithFailure`, `PrefixCreationFailed { path, message }`).
  - `RunExecutableValidationError`: `ExecutablePathRequired`, `ExecutablePathMissing`, `ExecutablePathNotFile`, `ExecutablePathNotWindowsExecutable` (accepts `.exe` _or_ `.msi`), `ProtonPathRequired`, `ProtonPathMissing`, `ProtonPathNotExecutable`, `PrefixPathMissing` (only when user provided a non-empty path that does not exist), `PrefixPathNotDirectory`.
  - Implement `Display`, `Error`, `From<RunExecutableValidationError> for RunExecutableError`, and a `message()` method on each error enum that returns user-facing strings.
- **MIRROR**: `crates/crosshook-core/src/update/models.rs` — every section.
- **IMPORTS**: `std::error::Error`, `std::fmt`, `std::path::PathBuf`, `serde::{Deserialize, Serialize}`.
- **GOTCHA**: Note the _new_ `ExecutablePathNotWindowsExecutable` message must say "must point to a Windows .exe or .msi file" (different from the update copy).
- **VALIDATE**: `cargo build --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

#### Task A3: Implement `validate_run_executable_request`

- **ACTION**: Add the validation function to `service.rs`.
- **IMPLEMENT**:
  - Validate executable path: required → exists → is file → has extension `exe` (case-insensitive) **OR** `msi` (case-insensitive). Return matching `RunExecutableValidationError` for each branch.
  - Validate proton path: required → exists → is executable file (reuse the `is_executable_file` helper pattern). Return matching errors.
  - Validate prefix path: **optional**. If non-empty, must exist and be a directory (no "PrefixPathRequired" — empty is allowed and means "auto-resolve").
- **MIRROR**: `crates/crosshook-core/src/update/service.rs:15-122` for structure; reuse the same `validate_proton_path` logic.
- **IMPORTS**: `std::fs`, `std::path::Path`, `super::models::*`.
- **GOTCHA**: The MSI extension matcher must use `eq_ignore_ascii_case` like the EXE matcher — Wine treats the extension case-insensitively too.
- **VALIDATE**: First test cases for A6 will exercise this; for now, `cargo check`.

#### Task A4: Implement `resolve_default_adhoc_prefix_path` + `provision_prefix`

- **ACTION**: Add helpers to `service.rs`.
- **IMPLEMENT**:
  - `resolve_default_adhoc_prefix_path(executable_path: &Path) -> Result<PathBuf, RunExecutableError>`: returns `BaseDirs::new()...data_local_dir().join("crosshook/prefixes/_run-adhoc").join(slugify(file_stem(executable_path)))`. Return `RunExecutableError::PrefixCreationFailed` if `BaseDirs::new()` fails (treat as home-unavailable).
  - `provision_prefix(prefix_path: &Path) -> Result<(), RunExecutableError>`: same as `install/service.rs:125-139` — if directory exists fine, if file → error, otherwise `create_dir_all`.
  - Local `slugify(name: &str) -> String` mirroring the install service slugifier (ASCII alphanumeric → lowercase, others → `-`, trim, fallback `"adhoc"`).
- **MIRROR**: `crates/crosshook-core/src/install/service.rs:21-46, 125-150, 323-341`.
- **IMPORTS**: `directories::BaseDirs`, `std::fs`, `std::path::{Path, PathBuf}`.
- **GOTCHA**: Use `_run-adhoc` (with leading underscore) so the directory sorts to the top of `~/.local/share/crosshook/prefixes/` and is visually distinct from real game prefixes. Document the choice in a brief module-level comment so reviewers do not "fix" it.
- **VALIDATE**: Compile check; A6 unit test will verify the resolved path shape.

#### Task A5: Implement `build_run_executable_command` and `run_executable`

- **ACTION**: Add to `service.rs`.
- **IMPLEMENT**:
  - `build_run_executable_command(request, prefix_path, log_path)`:
    1. `let mut command = new_direct_proton_command(request.proton_path.trim());`
    2. **Branch on file extension**:
       - `.exe` → `command.arg(request.executable_path.trim());`
       - `.msi` → `command.arg("msiexec"); command.arg("/i"); command.arg(request.executable_path.trim()); command.arg("/qb");` (`/qb` = basic UI, no full silence — gives users feedback while still being mostly automated; document the choice).
    3. `apply_host_environment(&mut command);`
    4. `apply_runtime_proton_environment(&mut command, prefix_path.to_string_lossy().as_ref(), request.steam_client_install_path.trim());`
    5. `apply_working_directory(&mut command, request.working_directory.trim(), Path::new(request.executable_path.trim()));`
    6. `attach_log_stdio(&mut command, log_path).map_err(...)?;`
    7. Return `Ok(command)`.
  - `run_executable(request, log_path) -> Result<(RunExecutableResult, tokio::process::Child), RunExecutableError>`:
    1. `validate_run_executable_request(request)?;`
    2. Resolve prefix path: if `request.prefix_path.trim().is_empty()`, call `resolve_default_adhoc_prefix_path(Path::new(request.executable_path.trim()))?`; otherwise `PathBuf::from(request.prefix_path.trim())`.
    3. `provision_prefix(&prefix_path)?;`
    4. `let mut command = build_run_executable_command(request, &prefix_path, log_path)?;`
    5. `let child = command.spawn().map_err(|e| RunExecutableError::RunnerSpawnFailed { message: e.to_string() })?;`
    6. Return `Ok((RunExecutableResult { succeeded: true, message: "Executable launched.".to_string(), helper_log_path: log_path.display().to_string(), resolved_prefix_path: prefix_path.display().to_string() }, child))`.
- **MIRROR**: `crates/crosshook-core/src/update/service.rs:25-70` for command building, `install/service.rs:48-103` for the spawn-and-result shape (minus the synchronous wait — Run EXE returns the child like Update does so the Tauri layer can stream logs and emit completion events).
- **IMPORTS**: `tokio::process::Command`, `crate::launch::runtime_helpers::*`, `super::models::*`, `std::path::{Path, PathBuf}`.
- **GOTCHA**: Do **not** use `runtime_handle.block_on(child.wait())` like `install_game` does — that approach blocks the tokio worker. Mirror `update_game`'s "return the child + spawn an async log streamer in the Tauri layer" pattern instead. This is the whole reason we are mirroring `update.rs`, not `install.rs`.
- **VALIDATE**: `cargo check`; A6 tests will exercise the full path.

#### Task A6: Unit tests for the core service

- **ACTION**: Add `#[cfg(test)] mod tests { ... }` to `service.rs`.
- **IMPLEMENT** (mirroring `update/service.rs:148-380`):
  - `valid_request(temp_dir)` helper that creates a fake `proton` exec script + an `installer.exe` and returns a populated `RunExecutableRequest`.
  - `validate_run_executable_request_accepts_valid_request`.
  - `validate_run_executable_request_accepts_msi_executable` (write `setup.MSI` and assert ok).
  - `validate_run_executable_request_rejects_empty_executable_path`.
  - `validate_run_executable_request_rejects_missing_executable_path`.
  - `validate_run_executable_request_rejects_directory_as_executable_path`.
  - `validate_run_executable_request_rejects_non_exe_or_msi_extension` (e.g. `.txt`).
  - `validate_run_executable_request_accepts_uppercase_exe_and_msi_extensions`.
  - `validate_run_executable_request_rejects_empty_proton_path`.
  - `validate_run_executable_request_rejects_missing_proton_path`.
  - `validate_run_executable_request_rejects_non_executable_proton_path`.
  - `validate_run_executable_request_allows_empty_prefix_path`.
  - `validate_run_executable_request_rejects_missing_prefix_path_when_provided`.
  - `validate_run_executable_request_rejects_file_as_prefix_path`.
  - `build_run_executable_command_uses_msiexec_for_msi` — assert `format!("{:?}", command).contains("msiexec")` and `.contains("/i")`.
  - `build_run_executable_command_uses_direct_arg_for_exe` — assert the debug output references the executable path directly without `msiexec`.
  - `resolve_default_adhoc_prefix_path_slugifies_executable_stem` — guard the chosen `_run-adhoc` namespace and slug shape using `resolve_default_adhoc_prefix_path_from_data_local_dir(temp.path(), Path::new("/x/Setup Wizard.exe"))` (extract a private fn so the test can inject the data dir, mirroring `install/service.rs:148-150`).
  - `run_executable_rejects_invalid_request` — `RunExecutableRequest::default()` → expect `Validation(ExecutablePathRequired)`.
- **MIRROR**: `crates/crosshook-core/src/update/service.rs:148-380` and `install/service.rs:368-547`.
- **IMPORTS**: `tempfile::tempdir`, `std::fs`, `std::path::Path`.
- **GOTCHA**: For `build_run_executable_command_*` tests we must create a real log file before calling, otherwise `attach_log_stdio` fails. See update tests for the `std::fs::File::create(&log_path)` pattern.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core run_executable`.

### Phase B — Tauri layer

#### Task B1: Promote `spawn_log_stream` to a shared `commands/log_stream.rs`

- **ACTION**: Create `src-tauri/src/commands/log_stream.rs` and move `spawn_log_stream` + `stream_log_lines` from `update.rs` into it as `pub(super) fn spawn_log_stream(...)` (or `pub(crate)` if you prefer).
- **IMPLEMENT**: Cut/paste the existing functions (no behavior changes), updated module path. The cancellation-PID clearing block currently references `UpdateProcessState::try_state` — move this responsibility back to the caller via a `clear_pid: Box<dyn Fn() + Send + Sync>` callback so `log_stream` is process-state-agnostic. (Alternative: take a `&'static str` state name — but the callback is cleaner and avoids string indirection.) Add a brief module doc explaining "shared async log poll-and-emit helper used by update + run_executable".
- **MIRROR**: `src-tauri/src/commands/update.rs:73-170`
- **IMPORTS**: `std::path::PathBuf`, `std::time::Duration`, `tauri::{AppHandle, Emitter}`.
- **GOTCHA**: This is a **refactor**. Run the existing update integration manually or at minimum `cargo check` immediately after the move; then update `update.rs` to call `spawn_log_stream(app, log_path, child, "update-log", "update-complete", Box::new(move || { if let Some(state) = app.try_state::<UpdateProcessState>() { *state.pid.lock().unwrap() = None; } }))`. Verify the `update-log` event still fires by running the dev shell at the end of the phase.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml`; confirm no other call sites of the old fn names remain (`grep -rn "spawn_log_stream\|stream_log_lines" src/crosshook-native/src-tauri/src/`).

#### Task B2: Register `log_stream` and `run_executable` modules

- **ACTION**: Update `src-tauri/src/commands/mod.rs`.
- **IMPLEMENT**: Add `mod log_stream;` (private — only siblings call it) and `pub mod run_executable;` next to existing module declarations. Keep alphabetical order roughly consistent with the existing list.
- **MIRROR**: `src-tauri/src/commands/mod.rs:1-23`
- **IMPORTS**: none
- **GOTCHA**: `mod log_stream;` should NOT be `pub` — keep the helper internal.
- **VALIDATE**: `cargo check`.

#### Task B3: Implement `commands/run_executable.rs`

- **ACTION**: Create the Tauri command file.
- **IMPLEMENT**:
  - `pub struct RunExecutableProcessState { pid: Mutex<Option<u32>> }` with `new()`.
  - `#[tauri::command] pub async fn validate_run_executable_request(request: RunExecutableRequest) -> Result<(), String>` — `spawn_blocking` + map to `String`.
  - `#[tauri::command] pub async fn run_executable(app: AppHandle, state: tauri::State<'_, RunExecutableProcessState>, request: RunExecutableRequest) -> Result<RunExecutableResult, String>`:
    1. `let slug = slugify_target(Path::new(&request.executable_path).file_stem().and_then(|s| s.to_str()).unwrap_or(""), "run-executable");`
    2. `let log_path = create_log_path("run-executable", &slug)?;`
    3. `spawn_blocking` to call `crosshook_core::run_executable::run_executable(&request, &log_path)`.
    4. Store PID into `state.pid`.
    5. `super::log_stream::spawn_log_stream(app, log_path, child, "run-executable-log", "run-executable-complete", Box::new(move || { if let Some(s) = app_handle.try_state::<RunExecutableProcessState>() { *s.pid.lock().unwrap() = None; } }));`
    6. Return `Ok(result)`.
  - `#[tauri::command] pub async fn cancel_run_executable(state: tauri::State<'_, RunExecutableProcessState>) -> Result<(), String>` — same `kill <pid>` pattern as `cancel_update`.
- **MIRROR**: `src-tauri/src/commands/update.rs:13-72`
- **IMPORTS**: `std::sync::Mutex`, `std::path::Path`, `crosshook_core::run_executable::{run_executable as run_executable_core, validate_run_executable_request as validate_run_executable_request_core, RunExecutableRequest, RunExecutableResult}`, `tauri::{AppHandle, Manager}`, `super::shared::{create_log_path, slugify_target}`.
- **GOTCHA**: Keep the `app_handle` clone capture for the callback closure — `app: AppHandle` is moved into `spawn_log_stream`, so clone before calling and capture the clone in the closure. Mirrors how the existing update flow would have to change in B1.
- **VALIDATE**: `cargo check`.

#### Task B4: Wire `RunExecutableProcessState` and commands into `lib.rs`

- **ACTION**: Update `src-tauri/src/lib.rs`.
- **IMPLEMENT**:
  - In the `tauri::Builder::default()` chain, after `.manage(commands::update::UpdateProcessState::new())`, add `.manage(commands::run_executable::RunExecutableProcessState::new())`.
  - In `invoke_handler!`, after the existing `commands::update::cancel_update,` line, add:

    ```rust
    commands::run_executable::validate_run_executable_request,
    commands::run_executable::run_executable,
    commands::run_executable::cancel_run_executable,
    ```

- **MIRROR**: `src-tauri/src/lib.rs:200-273` for `manage` and `invoke_handler!` usage.
- **IMPORTS**: none — already importing `commands`.
- **GOTCHA**: `tauri::generate_handler!` is order-sensitive only for readability, but commas are required between every entry — re-check with `cargo build` after editing.
- **VALIDATE**: `cargo build --manifest-path src/crosshook-native/Cargo.toml -p crosshook-tauri` (or `cargo check` on the full workspace) — must compile cleanly.

### Phase C — Frontend types & hook

#### Task C1: Create `types/run-executable.ts`

- **ACTION**: New file mirroring `types/install.ts` and the existing update types.
- **IMPLEMENT**:

  ```ts
  export interface RunExecutableRequest {
    executable_path: string;
    proton_path: string;
    prefix_path: string;
    working_directory: string;
    steam_client_install_path: string;
  }

  export interface RunExecutableResult {
    succeeded: boolean;
    message: string;
    helper_log_path: string;
    resolved_prefix_path: string;
  }

  export type RunExecutableStage = 'idle' | 'preparing' | 'running' | 'complete' | 'failed';

  export type RunExecutableValidationError =
    | 'executable_path_required'
    | 'executable_path_missing'
    | 'executable_path_not_file'
    | 'executable_path_not_windows_executable'
    | 'proton_path_required'
    | 'proton_path_missing'
    | 'proton_path_not_executable'
    | 'prefix_path_missing'
    | 'prefix_path_not_directory';

  export const RUN_EXECUTABLE_VALIDATION_MESSAGES: Record<RunExecutableValidationError, string> = {
    executable_path_required: 'The executable path is required.',
    executable_path_missing: 'The executable path does not exist.',
    executable_path_not_file: 'The executable path must be a file.',
    executable_path_not_windows_executable: 'The executable path must point to a Windows .exe or .msi file.',
    proton_path_required: 'A Proton path is required.',
    proton_path_missing: 'The Proton path does not exist.',
    proton_path_not_executable: 'The Proton path does not point to an executable file.',
    prefix_path_missing: 'The prefix path does not exist.',
    prefix_path_not_directory: 'The prefix path must be a directory.',
  };

  export const RUN_EXECUTABLE_VALIDATION_FIELD: Record<RunExecutableValidationError, keyof RunExecutableRequest> = {
    executable_path_required: 'executable_path',
    executable_path_missing: 'executable_path',
    executable_path_not_file: 'executable_path',
    executable_path_not_windows_executable: 'executable_path',
    proton_path_required: 'proton_path',
    proton_path_missing: 'proton_path',
    proton_path_not_executable: 'proton_path',
    prefix_path_missing: 'prefix_path',
    prefix_path_not_directory: 'prefix_path',
  };

  export interface RunExecutableValidationState {
    fieldErrors: Partial<Record<keyof RunExecutableRequest, string>>;
    generalError: string | null;
  }
  ```

- **MIRROR**: `src/types/install.ts` and the update validation maps in `src/types/update.ts`.
- **IMPORTS**: none.
- **GOTCHA**: Strings on the right of `RUN_EXECUTABLE_VALIDATION_MESSAGES` must **exactly** match the Rust `message()` output — that is how `mapValidationErrorToField` resolves a Rust-emitted string back to a typed variant. Cross-check character-for-character against `models.rs` after writing both.
- **VALIDATE**: `pnpm tsc --noEmit --pretty false` from `src/crosshook-native`.

#### Task C2: Create `hooks/useRunExecutable.ts`

- **ACTION**: New hook file.
- **IMPLEMENT**:
  - State: `request` (default empty `RunExecutableRequest`), `validation`, `stage`, `result`, `error`, `unlistenRef` for the completion event.
  - `updateField<Key extends keyof RunExecutableRequest>(key, value)` — same shape as `useUpdateGame`.
  - `mapValidationErrorToField(message: string)` — first check the message map, then a small heuristic fallback (`includes('executable')`, `includes('proton')`, `includes('prefix')`).
  - `startRun()`:
    1. Cleanup listener.
    2. Reset validation/error/result; set stage `'preparing'`.
    3. `await invoke('validate_run_executable_request', { request })` → on error map to field or generalError, set stage `'idle'`, return.
    4. `let completedBeforeInvoke = false;`
    5. `const unlisten = await listen<number | null>('run-executable-complete', (event) => { /* same exit-code switch as useUpdateGame */ })`.
    6. `const result = await invoke<RunExecutableResult>('run_executable', { request })`.
    7. `setResult(result)`. If `!completedBeforeInvoke` set stage `'running'`.
  - `cancelRun()` → `invoke('cancel_run_executable')` (best effort).
  - `reset()` → if running, cancel; cleanup listener; reset all state.
  - `statusText` / `hintText` / `actionLabel` switch on stage (mirror update).
  - `canStart` = stage idle/complete/failed AND `request.executable_path.trim().length > 0` AND `request.proton_path.trim().length > 0`.
  - Return all of the above plus `isRunning = stage === 'preparing' || stage === 'running'`.
- **MIRROR**: `src/hooks/useUpdateGame.ts:92-367` end-to-end.
- **IMPORTS**: `useCallback`, `useEffect`, `useRef`, `useState` from React; `invoke` from `@tauri-apps/api/core`; `listen` from `@tauri-apps/api/event`; new types from `../types/run-executable`.
- **GOTCHA**: The completion event name **must** be `'run-executable-complete'` — match the Tauri command exactly. Subscribe **before** invoking. Same race condition as the update flow.
- **VALIDATE**: `pnpm tsc --noEmit --pretty false`.

### Phase D — Frontend panel + page integration

#### Task D1: Create `components/RunExecutablePanel.tsx`

- **ACTION**: New component.
- **IMPLEMENT**:
  - Imports: `useState`, `ProtonPathField` (from `./ui/ProtonPathField`), `InstallField` (from `./ui/InstallField`), `useRunExecutable`, `useProtonInstalls` (or pass installs in via props — see D2 for the prop choice), types.
  - Props: `protonInstalls: ProtonInstallOption[]`, `protonInstallsError: string | null`.
  - Local state: `showConfirmation: boolean` for the run confirmation modal.
  - Helper: `fileNameFromPath(path)` — copy from `UpdateGamePanel.tsx:18-21`.
  - Helper: `stageLabel(stage)` — switch on `'preparing' | 'running' | 'complete' | 'failed' | 'idle'`.
  - Layout (mirroring `UpdateGamePanel`):
    1. `<section className="crosshook-install-shell" aria-labelledby="run-executable-heading">`
    2. `crosshook-install-shell__content` containing:
       - `crosshook-install-intro` block with eyebrow `Run EXE/MSI`, title `Run an arbitrary Windows executable`, copy `Run a one-off .exe or .msi through Proton without saving a profile. Useful for trying trainers, running installers, or one-off maintenance binaries.`
       - `crosshook-install-section` "Executable":
         - `<InstallField label="Executable" value={request.executable_path} onChange={(v) => updateField('executable_path', v)} placeholder="/mnt/media/setup.exe or /mnt/media/installer.msi" browseLabel="Browse" browseTitle="Select Executable" browseFilters={[{ name: 'Windows Executable', extensions: ['exe', 'msi'] }]} helpText="Choose any .exe or .msi to run inside the Proton prefix." error={validation.fieldErrors.executable_path} />`
       - `crosshook-install-section` "Runtime":
         - `<ProtonPathField value={request.proton_path} onChange={(v) => updateField('proton_path', v)} error={validation.fieldErrors.proton_path} installs={protonInstalls} installsError={protonInstallsError} idPrefix="run-exec" />`
         - `<InstallField label="Prefix Path (optional)" value={request.prefix_path} onChange={(v) => updateField('prefix_path', v)} placeholder="Auto: ~/.local/share/crosshook/prefixes/_run-adhoc/<slug>" browseLabel="Browse" browseMode="directory" browseTitle="Select Prefix Directory" helpText="Leave empty to auto-create a throwaway prefix under _run-adhoc/." error={validation.fieldErrors.prefix_path} />`
         - `<InstallField label="Working Directory (optional)" value={request.working_directory} onChange={(v) => updateField('working_directory', v)} placeholder="Defaults to the executable's parent directory" browseLabel="Browse" browseMode="directory" browseTitle="Select Working Directory" helpText="Optional override for the process current directory." />` (no error binding — backend treats it as optional and the field has no validation variant.)
       - `crosshook-install-card` status block: stage label, status text, hint text, error banner if any, log path display, _plus_ the resolved prefix path when `result.resolved_prefix_path` is set.
    3. `crosshook-install-shell__footer crosshook-route-footer` with:
       - `<button className="crosshook-button" onClick={() => setShowConfirmation(true)} disabled={isRunning || !canStart}>{actionLabel}</button>`
       - `<button className="crosshook-button crosshook-button--secondary" onClick={() => cancelRun()} disabled={!isRunning}>Cancel</button>`
       - `<button className="crosshook-button crosshook-button--secondary" onClick={() => reset()}>Reset</button>`
    4. Confirmation modal copy: `Run {fileNameFromPath(request.executable_path)} through Proton?` with body `This will spawn the executable inside {prefixHint} and stream its output to the console drawer.` where `prefixHint` is `request.prefix_path.trim() || 'a new throwaway prefix under _run-adhoc/'`.
- **MIRROR**: `src/components/UpdateGamePanel.tsx:39-280` end-to-end. Drop the cover-art branch entirely (no profile context).
- **IMPORTS**: `useState`, `InstallField`, `ProtonPathField`, `useRunExecutable`, types.
- **GOTCHA**: Reuse the existing `crosshook-install-shell*` and `crosshook-install-section*` classes — do not invent new BEM blocks. The Phase 1 design system already styles these, including responsive wrap behavior on Steam Deck width.
- **VALIDATE**: `pnpm tsc --noEmit --pretty false`. Manual smoke test happens after Task D2.

#### Task D2: Wire `RunExecutablePanel` into `InstallPage` as a third sub-tab

- **ACTION**: Update `src/components/pages/InstallPage.tsx`.
- **IMPLEMENT**:
  - Widen `type InstallPageTab = 'install' | 'update' | 'run_executable';`.
  - Import `RunExecutablePanel`.
  - Add a third `Tabs.Trigger value="run_executable" className="crosshook-subtab">Run EXE/MSI</Tabs.Trigger>` after the existing Update Game trigger.
  - Add a third `Tabs.Content value="run_executable" forceMount className="crosshook-subtab-content crosshook-install-page-tabs__content" style={{ display: installPageTab === 'run_executable' ? undefined : 'none' }}>` containing `<div className="crosshook-subtab-content__inner crosshook-install-page-tabs__panel-inner"><RunExecutablePanel protonInstalls={protonInstalls} protonInstallsError={protonInstallsError} /></div>`.
  - **Do not** widen any of the profile-review-session logic — Run EXE/MSI never opens the review modal.
- **MIRROR**: `src/components/pages/InstallPage.tsx:333-372` (the existing two-tab `Tabs.Root`).
- **IMPORTS**: `RunExecutablePanel` from `../RunExecutablePanel`.
- **GOTCHA**: When the third tab is added, the `crosshook-subtab-row` may wrap on narrow widths. CSS already handles this (`row-gap: var(--crosshook-radius-sm)` from `theme.css:603`), but visually verify on the 1280×800 dev breakpoint (Steam Deck).
- **VALIDATE**: `pnpm tsc --noEmit --pretty false`.

#### Task D3: Rename Install route in `routeMetadata.ts`

- **ACTION**: Edit `src/components/layout/routeMetadata.ts`.
- **IMPLEMENT**: In the `install` entry of `ROUTE_METADATA`, update **three** literals (the route key `'install'`, the icon `Art`, and the `sectionEyebrow: 'Setup'` all stay unchanged):
  - `navLabel`: `'Install Game'` → `'Install & Run'` — also propagates into `ROUTE_NAV_LABEL.install` automatically because it is derived (`ROUTE_METADATA.install.navLabel`). Verify the derivation block at the bottom of the file does **not** need a manual update.
  - `bannerTitle`: `'Install Game'` → `'Install & Run'`
  - `bannerSummary`: `'Install or update games, then review and save generated profiles.'` → `'Install games, apply updates, or run an arbitrary Windows EXE or MSI without committing it to a profile.'`
- **MIRROR**: `src/components/layout/routeMetadata.ts:48-54` and the `ROUTE_NAV_LABEL` derivation at lines 93-103.
- **IMPORTS**: none
- **GOTCHA**: Do **not** change `sectionEyebrow` (still `'Setup'`), the `'install'` route key, the `Art` icon, or the `RouteMetadataEntry` type. The Phase 1 banner _contract_ (eyebrow + title + summary + icon layout) is preserved — only the human-facing text changes. Also do **not** rename the inner sub-tab labels: `Install Game`, `Update Game`, and `Run EXE/MSI` remain accurate descriptions of what each sub-tab does, and the renamed parent (`Install & Run`) is now an honest umbrella for all three.
- **CROSS-CHECK**: After saving, grep for the string literal `Install Game` in `src/crosshook-native/src/` to confirm the only remaining occurrences are intentional (the sub-tab trigger label inside `InstallPage.tsx`, and any test fixtures). The sidebar status row (`StatusRow label="Current view"`) reads from `ROUTE_NAV_LABEL[activeRoute]`, so it will pick up the new label automatically.
- **VALIDATE**: `pnpm tsc --noEmit --pretty false`; manually verify the sidebar entry, sidebar status row "Current view", and the route banner all show `Install & Run`.

### Phase E — Verification

#### Task E1: Static analysis pass

- **ACTION**: Run formatters/linters/type-checkers on changed files.
- **IMPLEMENT**: From `src/crosshook-native/`:
  - `cargo fmt`
  - `cargo clippy --workspace -- -D warnings` (or at least `cargo clippy -p crosshook-core -p crosshook-tauri -- -D warnings`)
  - `pnpm tsc --noEmit --pretty false`
- **MIRROR**: project verification commands in CLAUDE.md.
- **VALIDATE**: All three commands exit zero.

#### Task E2: Core unit tests

- **ACTION**: Run the new core tests.
- **IMPLEMENT**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core run_executable`
- **VALIDATE**: All Phase A6 tests pass.

#### Task E3: Full crosshook-core test sweep (regression check for the log_stream refactor)

- **ACTION**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- **VALIDATE**: All existing tests still pass (including update + install service tests).

#### Task E4: End-to-end smoke test in dev shell

- **ACTION**: Run `./scripts/dev-native.sh` and exercise the new tab manually.
- **IMPLEMENT**:
  1. Launch dev shell.
  2. Confirm the Setup-group sidebar entry now reads `Install & Run` (not `Install Game`) and the sidebar status row "Current view" matches when the route is active.
  3. Navigate to the `Install & Run` route and confirm the route banner title is `Install & Run` with the new summary copy.
  4. Confirm the three sub-tabs render in order: `Install Game`, `Update Game`, `Run EXE/MSI`.
  5. Pick a tiny known-safe `.exe` (e.g. a Notepad++ portable installer); leave prefix empty; click Run; confirm the confirmation modal; click Run.
  6. Verify the console drawer streams `run-executable-log` lines, the status card transitions to `Running` then `Complete`, and the resolved prefix path shows under the status card.
  7. Verify clicking Cancel mid-run kills the process and the status transitions to `Failed` (or `Idle` after Reset).
  8. Try a sample `.msi` (e.g. a free MSI like the Visual C++ redistributable) and confirm the Proton command logs `msiexec /i ...` (visible in console drawer).
  9. Switch back to the `Install Game` and `Update Game` sub-tabs, confirm their state is preserved (forceMount working) and their existing flows still run correctly.
- **VALIDATE**: All checks pass; no console errors; no broken layout on Steam Deck width; no remaining `Install Game` references in the sidebar or route banner (only the inner sub-tab still uses that label).

---

## Testing Strategy

### Unit Tests (Rust core)

| Test                                                                        | Input                                        | Expected Output                                             | Edge Case?                  |
| --------------------------------------------------------------------------- | -------------------------------------------- | ----------------------------------------------------------- | --------------------------- |
| `validate_run_executable_request_accepts_valid_request`                     | populated `RunExecutableRequest` with `.exe` | `Ok(())`                                                    | –                           |
| `validate_run_executable_request_accepts_msi_executable`                    | request with `.msi` extension                | `Ok(())`                                                    | yes (new path)              |
| `validate_run_executable_request_accepts_uppercase_exe_and_msi_extensions`  | request with `.EXE` and `.MSI`               | `Ok(())`                                                    | yes (case-insensitive)      |
| `validate_run_executable_request_rejects_empty_executable_path`             | empty `executable_path`                      | `Err(ExecutablePathRequired)`                               | –                           |
| `validate_run_executable_request_rejects_missing_executable_path`           | nonexistent path                             | `Err(ExecutablePathMissing)`                                | –                           |
| `validate_run_executable_request_rejects_directory_as_executable_path`      | path points at a dir                         | `Err(ExecutablePathNotFile)`                                | –                           |
| `validate_run_executable_request_rejects_non_exe_or_msi_extension`          | `.txt` file                                  | `Err(ExecutablePathNotWindowsExecutable)`                   | –                           |
| `validate_run_executable_request_rejects_empty_proton_path`                 | empty proton_path                            | `Err(ProtonPathRequired)`                                   | –                           |
| `validate_run_executable_request_rejects_missing_proton_path`               | nonexistent proton                           | `Err(ProtonPathMissing)`                                    | –                           |
| `validate_run_executable_request_rejects_non_executable_proton_path`        | non-+x file                                  | `Err(ProtonPathNotExecutable)`                              | –                           |
| `validate_run_executable_request_allows_empty_prefix_path`                  | blank `prefix_path`                          | `Ok(())`                                                    | yes (new optional behavior) |
| `validate_run_executable_request_rejects_missing_prefix_path_when_provided` | nonexistent prefix dir                       | `Err(PrefixPathMissing)`                                    | –                           |
| `validate_run_executable_request_rejects_file_as_prefix_path`               | regular file at prefix                       | `Err(PrefixPathNotDirectory)`                               | –                           |
| `build_run_executable_command_uses_msiexec_for_msi`                         | `.msi` request                               | command debug contains `msiexec` and `/i`                   | yes                         |
| `build_run_executable_command_uses_direct_arg_for_exe`                      | `.exe` request                               | command debug contains the exe path, _not_ `msiexec`        | –                           |
| `resolve_default_adhoc_prefix_path_slugifies_executable_stem`               | `Setup Wizard.exe` under `/x`                | path ends with `crosshook/prefixes/_run-adhoc/setup-wizard` | yes                         |
| `run_executable_rejects_invalid_request`                                    | `RunExecutableRequest::default()`            | `Err(Validation(ExecutablePathRequired))`                   | –                           |
| Models round-trip (in `models.rs`)                                          | populated request → JSON → request           | equal                                                       | yes (Serde defaults)        |
| Models error message (in `models.rs`)                                       | each `RunExecutableValidationError` variant  | `.message()` matches the TS map literal                     | yes (TS↔Rust contract)      |

### Edge Cases Checklist

- [x] Empty input — covered by all `*_rejects_empty_*` cases
- [x] Invalid extension (e.g. `.txt`) — `validate_run_executable_request_rejects_non_exe_or_msi_extension`
- [x] Uppercase extensions — covered
- [x] Optional prefix omitted — `validate_run_executable_request_allows_empty_prefix_path` + `resolve_default_adhoc_prefix_path_*`
- [x] Optional working directory omitted — already covered by `apply_working_directory` falling back to executable parent
- [x] Cancellation mid-run — manual E4 verification
- [x] Concurrent runs — Mutex-guarded PID prevents accidental overwrite (only one ad-hoc run at a time)
- [x] Permission denied on prefix creation — `RunExecutableError::PrefixCreationFailed` returned with the underlying io message
- [x] Hidden tab state preserved when switching sub-tabs — forceMount + display:none guarantee it; verified manually in E4

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native
cargo fmt --check
cargo clippy -p crosshook-core -p crosshook-tauri -- -D warnings
pnpm tsc --noEmit --pretty false
```

EXPECT: All zero exit, no warnings.

### Unit Tests (focused)

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core run_executable
```

EXPECT: All Phase A6 tests pass.

### Full Core Test Suite (regression for log_stream refactor)

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: No regressions in any existing test.

### Tauri Compilation

```bash
cargo build --manifest-path src/crosshook-native/Cargo.toml
```

EXPECT: Workspace compiles.

### Frontend Build

```bash
cd src/crosshook-native
pnpm build
```

EXPECT: Vite production bundle succeeds.

### Browser / Dev-Shell Validation

```bash
./scripts/dev-native.sh
```

Then follow the manual E4 checklist above.
EXPECT: Run EXE/MSI tab present; .exe and .msi paths both run; cancel kills the process; existing Install/Update tabs unaffected.

### Manual Validation

- [ ] Sidebar Setup group still has exactly one entry, now labelled `Install & Run` (renamed from `Install Game`, no new entries added)
- [ ] Sidebar status row "Current view" reads `Install & Run` when the route is active
- [ ] Route banner title reads `Install & Run` and the eyebrow still reads `Setup`
- [ ] Install Page now shows three sub-tabs in order: `Install Game`, `Update Game`, `Run EXE/MSI`
- [ ] Banner summary mentions running an arbitrary EXE/MSI
- [ ] `.exe` selection produces a `proton run <exe>` command in `/tmp/crosshook-logs/run-executable-*.log`
- [ ] `.msi` selection produces a `proton run msiexec /i <msi> /qb` command in the same log
- [ ] Empty prefix path auto-creates a directory under `~/.local/share/crosshook/prefixes/_run-adhoc/<slug>`
- [ ] Status card transitions Idle → Preparing → Running → Complete (or Failed)
- [ ] Cancel button kills the process and the next state transition is Failed (with "Run process was terminated by a signal." or similar)
- [ ] Reset clears the form even after a completed run
- [ ] Switching to the Install/Update tabs and back preserves the Run tab's input fields and any in-progress run state
- [ ] No `console.log` statements left behind
- [ ] Steam Deck (1280×800) layout: sub-tab row wraps cleanly when needed, and the status card stays inside the scroll shell

---

## Acceptance Criteria

- [ ] All tasks A1–E4 completed
- [ ] All validation commands pass
- [ ] New core unit tests written and passing
- [ ] No type errors (Rust or TypeScript)
- [ ] No clippy warnings
- [ ] Existing update + install flows unaffected (regression run in E3 + manual smoke in E4)
- [ ] Sidebar entry, route banner title, and banner summary updated to `Install & Run`
- [ ] Issue #163 Phase 4 checklist items can be marked complete:
  - [ ] Add a Setup sidebar entry for one-off EXE/MSI execution → satisfied via the third sub-tab on the renamed `Install & Run` sidebar entry (Phase 1 banner contract preserved; no new sidebar item added — the existing Setup-group entry now broadens to cover all three flows)
  - [ ] Reuse existing launch/update orchestration where feasible → `commands/log_stream.rs` shared between update + run_executable; `runtime_helpers::*` reused intact
  - [ ] Keep execution profile-optional while preserving consistent status/log UX → no profile is created; status card mirrors `UpdateGamePanel`

## Completion Checklist

- [ ] Code follows discovered patterns (camelCase TS, snake_case Rust, BEM-like crosshook-\* classes)
- [ ] Error handling matches codebase style (`Result<T, String>` at the Tauri boundary, typed errors in core)
- [ ] Logging follows codebase conventions (`tracing::warn!` / `tracing::error!`, no println)
- [ ] Tests follow `update/service.rs` patterns (`tempdir`, `write_executable_script`, table of negative cases)
- [ ] No hardcoded values beyond the documented `_run-adhoc` namespace and the `/qb` MSI flag
- [ ] CLAUDE.md not modified (this is a feature plan, not an architectural rule change)
- [ ] No unnecessary scope additions — Run EXE/MSI does **not** introduce profile persistence, history, recent files, MCP changes, or schema migrations
- [ ] Self-contained — no questions needed during implementation
- [ ] Issue #163 phase 4 checkboxes ready to tick after PR merge

## Risks

| Risk                                                                                       | Likelihood | Impact | Mitigation                                                                                                                                                                                                                                                                                            |
| ------------------------------------------------------------------------------------------ | ---------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `log_stream.rs` refactor breaks the existing `update_game` log streaming                   | Medium     | High   | Refactor in B1 _first_, then immediately re-test the existing update flow manually before adding any new code paths. Promote the helper one-for-one (no behavior changes other than the new clear-pid callback).                                                                                      |
| MSI invocation via `proton run msiexec` differs across Proton versions                     | Low        | Medium | `msiexec` is part of every Proton/Wine prefix; no version we ship has dropped it. If a user reports issues, we can fall back to `wine msiexec` later — but that path has its own quirks (`apply_runtime_proton_environment` already sets `WINEPREFIX`, so `proton run msiexec` is the safer default). |
| Auto-creating throwaway prefixes balloons disk usage                                       | Low        | Low    | The `_run-adhoc/<slug>` namespace is reused per executable stem, so repeated runs of the same EXE share a prefix. Document that users can `rm -rf ~/.local/share/crosshook/prefixes/_run-adhoc` to reclaim space.                                                                                     |
| Long-running ad-hoc runs collide with simultaneous game launches and saturate the host CPU | Low        | Low    | Same as the existing update flow — no new resource controls needed.                                                                                                                                                                                                                                   |
| Steam Deck sub-tab row wrap looks ugly at 1280×800 with three triggers                     | Medium     | Low    | Existing CSS already handles wrap with `row-gap: var(--crosshook-radius-sm)`. Verify visually in E4; if it wraps unattractively, tighten the trigger label to `Run EXE`.                                                                                                                              |
| Validation message strings drift between Rust and TS                                       | Medium     | Medium | C1 explicitly cross-checks the strings against `models.rs`. Also add a TODO follow-up to write a Rust integration test that asserts every variant's `.message()` is present in the TS map (out of scope for this phase but worth noting).                                                             |

## Notes

- **Why a sub-tab and not a sidebar item?** Issue #165 says "discoverability level as existing setup actions". The Setup group currently has only one entry, and Phase 1 froze the route banner contract. Adding a sub-tab matches the discoverability tier without re-doing nav infrastructure or creating a new route.
- **Why rename the sidebar entry to `Install & Run`?** With three sub-tabs (Install Game, Update Game, Run EXE/MSI), the parent label `Install Game` would (a) duplicate the first sub-tab and (b) misrepresent the other two — Update Game is not an install, and Run EXE/MSI is profile-less by design. `Install & Run` is short enough for the sidebar column, honest about all three flows, and does not collide with the existing `Setup` group label. Alternatives considered and rejected: `Setup` (collides with the group label), `Game Setup` (still install-centric, doesn't fit ad-hoc Run), `Windows Runner` / `Proton Tools` (too technical for primary nav copy).
- **Why mirror Update Game and not Install Game?** The Install Game flow is intentionally heavyweight: it owns a draft profile, candidate executable discovery, profile review modal, and a `persistProfileDraft` call. Run EXE/MSI is the _opposite_ — single-shot, no persistence. Update Game is a much closer structural match: pick a binary, pick a runtime, run, stream logs, no profile mutation.
- **Why promote `spawn_log_stream` rather than duplicate it?** Because the second use case proves the abstraction is real. Duplication would diverge the moment one flow needs a different polling cadence. Single source of truth fits the project's coding standards (DRY, KISS).
- **Why `_run-adhoc` prefix root?** Underscore prefix sorts above alphanumeric prefixes in `ls`/`tree`, making it visually distinct from real game prefixes; the dash separator matches the existing slug convention; "adhoc" is the nearest English word to the `Lutris "Run EXE"` semantic without leaking the Lutris brand into the user-visible directory tree.
- **Why `/qb` for MSI?** "Basic UI" mode shows progress bars without modal prompts. "Silent" (`/qn`) would hide errors entirely. `/qb` is the right balance for an ad-hoc one-off run.
- **Storage boundary**: confirmed runtime-only. No `settings.toml` change, no metadata DB change, no migration. Issue #165 made these optional and we are intentionally deferring them to a follow-up if user demand emerges.
- **Persistence & usability**: the auto-resolved `_run-adhoc` prefix directory persists on disk (because we have to materialize a Wine prefix to run anything), but we treat it as a cache: it is not tracked in TOML or SQLite, users can rm it freely, and re-running an executable will lazily recreate it.
