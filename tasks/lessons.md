# Lessons

## 2026-03-18

- After changing a manifest that becomes a new source of truth for CI or release workflows, explicitly verify the committed file contents and byte size before pushing. Do not assume an edited `Cargo.toml` is intact just because local commands succeeded earlier in the turn.

- When replacing a manual path field with a detected-install dropdown, do not make the text input effectively dead once a dropdown value is chosen. Prefer a dropdown that fills the editable path field, so detection accelerates entry without blocking manual correction.

- For exported standalone Steam trainer launchers, do not point Proton at the original host/Linux trainer path. Generate the script so it stages the trainer into the deterministic `C:\CrossHook\StagedTrainers\...` prefix path first, matching the in-app trainer launch behavior and allowing staged state to persist across runs.
- In native Steam mode, do not derive `STEAM_COMPAT_CLIENT_INSTALL_PATH` from the selected game's compatdata or library root. That value must point at the real Steam client install (for example `~/.local/share/Steam` or `~/.steam/root`), or Proton-side trainer launches can misbehave even when the game launch itself works.
- For trainer staging in Steam mode, do not copy only the selected `.exe` by default. Directory-based trainers such as Aurora require adjacent DLLs, config files, and support directories. Stage a minimal bundle closure around the selected exe, and keep the staged Windows launch path deterministic under `C:\CrossHook\StagedTrainers\...`.
- When checking whether a Steam-launched game is already running from a helper shell script, do not use `pgrep -af` on the executable name. That matches the helper script’s own command line arguments like `--game-exe-name GoWR.exe` and produces false positives that skip the real Steam launch. Use exact process-name matching instead.
- In stateful diagnostic UI, do not map “no fresh scan result yet” to an error-like state such as `NotFound`. If persisted values are already loaded, show a neutral or saved state instead so reopening the app does not falsely imply missing data.
- When moving a UI section to a new layout container, remove the old render site in the same change. Do not rely on visual inspection later to catch duplicated panels like Launcher Export.
- In the native Tauri UI, do not let controller/gamepad key handlers capture typing keys while focus is inside editable controls. Before handling `Space`, `Enter`, arrows, or `Escape` globally, explicitly skip `input`, `textarea`, `select`, and `contenteditable` targets.
- In the Tauri v2 native app, do not assume registering a plugin is enough for frontend access. If a plugin API such as `@tauri-apps/plugin-dialog` appears to no-op, first verify that `src-tauri/capabilities/*.json` exists and grants the corresponding plugin permission set to the `main` window.

- When creating GitHub issues for this repo, do not assume `gh issue create --template ...` can be combined with `--body` or `--body-file`; the CLI rejects that combination.
- Do not assume YAML issue forms are discoverable by `gh issue create --template` in this repo. Validate first. If the CLI reports `no templates found`, use API/tooling to create a fully structured issue body that mirrors the intended form fields.
- When debugging missing `gh` tab completion for commands like `gh pr merge`, verify the shell and the CLI separately. If `_gh` is loaded but `gh __complete ...` returns no candidates, the problem is the CLI’s completion output, not shell wiring.
- When a user provides direct target-environment WINE/Proton validation, treat that as higher-confidence runtime evidence than a synthetic local harness and update the verification approach instead of continuing to optimize the proxy.
- When live-debugging CrossHook’s Steam helper from runtime logs, do not dismiss shell-level errors like `ps: command not found` as harmless just because later log lines appear. First remove the noisy failure and add explicit path/exit logging so the next run produces a clean causal signal.
- When auto-detecting Proton tools for Steam mode, do not assume every compat tool lives under the user’s Steam root. Also scan system-wide Steam compatibility tool directories such as `/usr/share/steam/compatibilitytools.d` before declaring a Proton mapping unresolved.
