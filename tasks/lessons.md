# Lessons

## 2026-03-18

- When creating GitHub issues for this repo, do not assume `gh issue create --template ...` can be combined with `--body` or `--body-file`; the CLI rejects that combination.
- Do not assume YAML issue forms are discoverable by `gh issue create --template` in this repo. Validate first. If the CLI reports `no templates found`, use API/tooling to create a fully structured issue body that mirrors the intended form fields.
- When debugging missing `gh` tab completion for commands like `gh pr merge`, verify the shell and the CLI separately. If `_gh` is loaded but `gh __complete ...` returns no candidates, the problem is the CLI’s completion output, not shell wiring.
- In this Linux workspace, run `dotnet` restore/build/test commands with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home`, `NUGET_PACKAGES=/tmp/nuget-packages`, and `NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache` because the default home-cache paths are read-only.
- On SDK 10 in this workspace, `dotnet new xunit` defaults to `net10.0` and does not accept `--framework net9.0`; create the test project file manually or retarget it explicitly for `net9.0`.
- Do not describe the current CrossHook `dotnet publish --self-contained true` output as a standalone or single-file exe. The published `crosshook.exe` is an apphost that requires the rest of the `publish/` directory beside it, including `crosshook.dll`.
- When a user points out a legacy packaging UX requirement, do not stop at documenting the regression. First check whether the modern publish pipeline can restore an equally clean ship artifact before accepting the rougher workflow.
- Do not run `dotnet publish` for multiple RIDs in parallel against the same project directory. The commands race on `obj/project.assets.json` and can produce false `NETSDK1047` failures.
- When a user provides direct target-environment WINE/Proton validation, treat that as higher-confidence runtime evidence than a synthetic local harness and update the verification approach instead of continuing to optimize the proxy.
- When guiding a user through the CrossHook UI during live debugging, verify which panel is visible before referencing controls like `Running EXE`; the selector is only on the `Target Process, DLL Inject...` panel, not `Trainer Setup`.
- When a trainer works only when both the game and trainer are launched manually inside the Steam game's Proton compatdata, treat that as evidence that CrossHook's direct-EXE launch path is bypassing the Steam/Proton session rather than as a generic trainer timing issue.
- When the user proves that manual `proton run <trainer>` works at the same in-game state where CrossHook's staged trainer launch fails, stop attributing the bug to stale timing or staging theories. Treat the remaining delta as the host launch chain used by CrossHook to enter the shell and invoke Proton.
- When changing CrossHook's Steam helper bridge, do not assume a Windows .NET process under Wine can `Process.Start("/bin/bash")` directly. Validate that launch path first; if it fails, keep Wine's `start.exe /unix` bridge and fix the Proton-side environment instead.
- When live-debugging CrossHook's Steam helper from runtime logs, do not dismiss shell-level errors like `ps: command not found` as harmless just because later log lines appear. First remove the noisy failure and add explicit path/exit logging so the next run produces a clean causal signal.
- When normalizing browse-selected Steam paths from a Wine-hosted Windows app, do not rely on probing the full `.../pfx/dosdevices/d:` path directly. In the real app runtime, the `d:` segment can fail path checks even when the symlink exists. Resolve mapped drives by enumerating the parent `dosdevices/` directory and matching the drive entry by name.
