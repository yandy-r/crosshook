# Lessons

## 2026-04-15

- When extending a Rust struct used in tests, do not leave a trailing `..Default::default()` once every field has been specified explicitly. In this repo, Clippy runs with `-D warnings`, so a needless struct update in test code is still a build blocker.

## 2026-04-14

- When a shell script stores state that is only consumed indirectly by a sourced helper, do not assume ShellCheck will follow that usage across files. In this repo, either refactor to an explicit argument flow or add a targeted `SC2034` suppression at the declaration so `.github/workflows/lint.yml` stays green.
- When ShellCheck still flags the assignment site after a suppression on a shared shell variable, stop patching around the warning. In this repo, prefer a stateless helper API that returns filtered paths directly instead of hiding cross-file mutable state behind sourced functions.

## 2026-04-13

- When a frontend settings panel is meant to reflect backend-resolved behavior, do not key its fallback only off a missing optional field. In this repo, `launch.trainer_gamescope.enabled == false` still means “auto-generate from the game gamescope config” when the game gamescope wrapper is enabled, so the Profiles -> Gamescope tab must render the resolved trainer config, not the raw disabled storage value.
- When a settings screen shows values that are derived rather than explicitly saved, do not leave that state implicit. In this repo, if trainer gamescope is being auto-generated from the game config, the Profiles -> Gamescope tab should show a visible notice that the values are generated and that editing them creates a trainer-specific saved override.
- When a launch-state fix relies on process detection, do not stop at verifying the visible status label. In this repo, also verify that the underlying `LaunchPhase` returns to `Idle` when the detected game exits, so `Launch Game` becomes actionable again without requiring a manual Reset.
- When a Steam trainer launch diverges from a working `proton_run` trainer launch, do not keep patching the shell helper in the abstract. Treat the trainer subprocess by its actual runtime path: use `resolved_trainer_gamescope()`, resolve trainer launch optimizations as `METHOD_PROTON_RUN`, pass through `runtime.working_directory`, and preserve only the explicit trainer env keys across helper cleanup.
- In Flatpak, if the Steam trainer helper path still differs from the working `proton_run` trainer path after env and gamescope parity work, prefer routing Steam trainer launches through the same direct Proton trainer builder and record/analyze the execution as `proton_run` instead of maintaining a second helper-only launch contract.
- After a platform-wide trainer-launch fix appears to work once and then fails on a specific title, verify the fix on at least one other Steam game before continuing to mutate the general launch path. In this repo, Hitman 2’s launcher-to-game transition produced a separate game-specific failure after the Flatpak Steam/proton parity bug was already fixed.

## 2026-04-11

- When manually reproducing a `proton_run` failure, verify the target game is fully closed before trusting the exit code. A duplicate launch against an already-running Windows game can exit immediately and mimic the original failure, which makes the repro useless for root-cause analysis.
- When a direct host Proton repro succeeds under `env -i ... proton run ...` but the same launch fails through `flatpak-spawn --host`, suspect sandbox env contamination before blaming paths again. For Flatpak host launches, prefer `--clear-env` plus an explicit allowlist of env vars, rather than inheriting the full sandbox environment into the host child.
- When a Flatpak `proton_run` failure survives after helper-side host path validation is fixed, stop assuming the helper scripts are still on the critical path. For direct `proton_run`, inspect the actual `flatpak-spawn --host` child command and its env, and do not treat the lack of a host-visible `/tmp/crosshook-logs/...` file as “no logs” until you account for Flatpak’s sandbox-private `/tmp`.
- When a Flatpak launch failure reproduces only with a system compat tool under `/usr/share/steam/compatibilitytools.d` and disappears when the same profile uses a user-local Proton install under `~/.local/share/Steam/compatibilitytools.d`, stop debugging the launch method in the abstract. Treat the compat-tool location as the primary axis, prefer the user-local tool in Flatpak launch resolution, and do not preserve the `/usr` path just because discovery found it.
- When normalizing Flatpak-selected host paths, do not stop at `/run/host/...`. Tauri/portal file pickers can return document-portal mounts under `/run/user/<uid>/doc/...`; resolve those through the `user.document-portal.host-path` xattr before persisting or host-executing the path, or Proton/tool directories will launch from incomplete portal mirrors and fail in non-obvious ways.
- When adding a new Tauri command module under `src-tauri/src/commands/`, do not forget the module visibility contract used by `src-tauri/src/lib.rs`. If `generate_handler!` references `commands::foo::bar`, the `foo` module must be publicly reachable from `commands/mod.rs`, or the native build will fail with a private-module error even though the command function itself is public.

## 2026-04-04

- When rendering profile data loaded over IPC, do not trust frontend-required nested objects like `profile.runtime` to exist on every saved profile. Guard nested reads in the UI, because legacy or sparse TOML profiles can deserialize without those sections and crash render paths that dereference them directly.

## 2026-03-31

- When porting a JavaScript hash or checksum routine that relies on 32-bit overflow semantics, do not translate it with normal Rust arithmetic in debug builds. Use explicit `wrapping_*` operations so the code matches the upstream behavior and does not panic under real UI-triggered inputs.

## 2026-03-26

- When reverse-engineering ProtonDB’s richer report feed, do not guess which inputs affect the hashed report path. In the current frontend chunks, the hash input is `zk(Number(appId), counts.reports, counts.timestamp, pageSelector)`, while `hardwareType` only selects the URL segment (`all-devices`, `pc`, `steam-deck`, `chrome-os`). Keep that distinction explicit before porting the lookup logic into CrossHook.
- When debugging top-level page scroll carryover in the native Tauri shell, first identify the real scroll owner before patching route effects. In this repo, if multiple pages wrap themselves in the same `.crosshook-content-area`, move that scroll container into the shared layout shell and reset it there with a `ref` in `useLayoutEffect`; do not rely on per-page wrappers or `querySelector` plus `requestAnimationFrame` to clean up shared scroll state.
- In React route/layout work, do not assume `key` survives inside a spread props object. If a remount is required, pass `key={...}` directly in JSX or the remount logic will silently fail.

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
- When moving final executable editing from the install hook into a shared profile form, keep `runtime.working_directory` derivation aligned with `game.executable_path`. If the executable changes and the working directory was still derived, update it too, or Proton launches can appear to start while the game never opens.
- When debugging an install-generated profile that logs Proton startup but never opens the game, verify the autodetected final executable candidate before blaming the modal handoff or runtime fields. Candidate selection can still pick the wrong launcher/bootstrap executable even when the rest of the saved profile is valid.

- When creating GitHub issues for this repo, do not assume `gh issue create --template ...` can be combined with `--body` or `--body-file`; the CLI rejects that combination.
- Do not assume YAML issue forms are discoverable by `gh issue create --template` in this repo. Validate first. If the CLI reports `no templates found`, use API/tooling to create a fully structured issue body that mirrors the intended form fields.
- When debugging missing `gh` tab completion for commands like `gh pr merge`, verify the shell and the CLI separately. If `_gh` is loaded but `gh __complete ...` returns no candidates, the problem is the CLI’s completion output, not shell wiring.
- When a user provides direct target-environment WINE/Proton validation, treat that as higher-confidence runtime evidence than a synthetic local harness and update the verification approach instead of continuing to optimize the proxy.
- When live-debugging CrossHook’s Steam helper from runtime logs, do not dismiss shell-level errors like `ps: command not found` as harmless just because later log lines appear. First remove the noisy failure and add explicit path/exit logging so the next run produces a clean causal signal.
- When auto-detecting Proton tools for Steam mode, do not assume every compat tool lives under the user’s Steam root. Also scan system-wide Steam compatibility tool directories such as `/usr/share/steam/compatibilitytools.d` before declaring a Proton mapping unresolved.
- When a repo already treats `CHANGELOG.md` as the release source of truth, do not introduce a parallel `release_notes.md` flow. Publish the tagged `CHANGELOG.md` section directly so the workflow and human edits stay aligned.
- When `CHANGELOG.md` is generated from commit history, treat commit messages as release-note copy. Use descriptive conventional commits for user-facing work, and route internal planning/release churn into skipped forms like `chore(...)` or `docs(internal): ...` so `git-cliff` and release validation stay clean.

## 2026-03-25

- When a user narrows a feature’s required scope, immediately rewrite the plan/spec around the narrowed scope instead of continuing to treat optional stretch goals as core requirements. In this repo, if the user says a launch feature is only required for `proton_run`, do not keep Steam parity as a gating decision in the main spec.
- When backend logic derives behavior from `GameProfile.launch.method`, do not branch on the raw stored string alone. In this repo, legacy or imported profiles can have an empty `launch.method` and still resolve to `steam_applaunch` via fallback rules, so stale checks and export-path selection must use the same resolved launch-method logic as the frontend/profile normalization path.
- When adding a dense new panel to the native app, do not default to stacking it in the right column beneath existing launch cards. First check whether that pushes important actions below the fold; if it does, move the dense panel into a full-width slot or move low-frequency surfaces like logs into their own tab.
- Before tightening a UI layout based on one screenshot, verify whether the apparent imbalance is actually intentional grouping. In this repo, do not replace `auto-fit` card grids with fixed column counts if sparse rows are category-specific sections rather than accidental leftovers.
- When launch options are mutually exclusive, do not rely on backend validation at launch time as the first user-visible feedback. Encode the conflict matrix in the frontend contract too, block incompatible selections immediately, and surface the reason inline in the panel.
- When conflict feedback belongs to a specific option category, do not pin the warning to a single global location at the top of the panel. Render the warning inside the affected section or group so users see the cause next to the conflicting controls.
- When introducing a new app shell, do not layer a CSS grid wrapper around a resizable panel-group shell unless the panel-group element is explicitly made to span the full grid. If the wrapper only has one child, verify that the child fills the entire shell before considering the layout acceptable.
- When introducing resizable panes, do not assume the library separator becomes a usable handle without explicit shell sizing and handle styles. Verify that the separator has visible hit area, that panel min/max constraints match the intended resize range, and that inner content is not fighting panel height with its own fixed max-height.
- When replacing a previously centered app shell, preserve the old width and height contract first before adding new resize behavior. In this repo, do not let the new shell go edge-to-edge or switch from fixed viewport height to unconstrained min-height unless that change is explicitly desired and visually verified.
- When choosing responsive collapse breakpoints for a new shell, do not guess from generic desktop/mobile values. Validate against the actual app window widths the user is working in; if the sidebar is still expected to be visible at a given width, do not collapse it there.
- When the user explicitly changes the product decision from “responsive collapse” to “never auto-collapse,” remove the collapse logic entirely in the same pass. Do not leave a reduced breakpoint behind and assume that is close enough.
- When using `react-resizable-panels`, do not treat numeric panel size props as percentages. `defaultSize`, `minSize`, and `maxSize` numeric values are pixels; use percentage strings like `"20%"` when you want relative shell sizing.
- When refactoring a UI shell into new page components, explicitly audit every previously mounted feature surface before declaring the refactor complete. A component still existing in the repo is not enough; verify that it still has a live render site.
