# Lessons

## 2026-03-18

- When creating GitHub issues for this repo, do not assume `gh issue create --template ...` can be combined with `--body` or `--body-file`; the CLI rejects that combination.
- Do not assume YAML issue forms are discoverable by `gh issue create --template` in this repo. Validate first. If the CLI reports `no templates found`, use API/tooling to create a fully structured issue body that mirrors the intended form fields.
- When debugging missing `gh` tab completion for commands like `gh pr merge`, verify the shell and the CLI separately. If `_gh` is loaded but `gh __complete ...` returns no candidates, the problem is the CLI’s completion output, not shell wiring.
- In this Linux workspace, run `dotnet` restore/build/test commands with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home`, `NUGET_PACKAGES=/tmp/nuget-packages`, and `NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache` because the default home-cache paths are read-only.
- On SDK 10 in this workspace, `dotnet new xunit` defaults to `net10.0` and does not accept `--framework net9.0`; create the test project file manually or retarget it explicitly for `net9.0`.
- Do not describe the current ChooChoo `dotnet publish --self-contained true` output as a standalone or single-file exe. The published `choochoo.exe` is an apphost that requires the rest of the `publish/` directory beside it, including `choochoo.dll`.
- When a user points out a legacy packaging UX requirement, do not stop at documenting the regression. First check whether the modern publish pipeline can restore an equally clean ship artifact before accepting the rougher workflow.
