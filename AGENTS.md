# ChooChoo Loader - Project Guidelines

## Project Overview

ChooChoo is a Proton/WINE Trainer & DLL Loader — a Windows Forms application that launches games alongside trainers, mods (FLiNG, WeMod, etc.), patches, and DLL injections. It targets Steam Deck, Linux, and macOS users running games through Proton/WINE.

## Tech Stack

- **Language**: C# (.NET Framework 4.8)
- **UI**: Windows Forms (WinForms)
- **Build System**: MSBuild (classic `.csproj`, NOT SDK-style)
- **Solution**: `src/ChooChooEngine.sln`
- **Project**: `src/ChooChooEngine.App/ChooChooEngine.App.csproj`
- **Output**: `choochoo.exe` (WinExe)

## Build Commands

```bash
# Build with MSBuild (NOT dotnet CLI — this is .NET Framework 4.8)
msbuild src/ChooChooEngine.sln /p:Configuration=Release
msbuild src/ChooChooEngine.sln /p:Configuration=Debug
```

> **Important**: `dotnet build` will NOT work. This project uses classic .NET Framework 4.8, not .NET Core/5+.

## Architecture

```
src/ChooChooEngine.App/
  Program.cs              # Entry point (single-instance via Mutex)
  Core/ProcessManager.cs  # Process lifecycle (launch, attach, suspend, resume, kill)
  Injection/InjectionManager.cs  # DLL injection (LoadLibraryA via CreateRemoteThread)
  Memory/MemoryManager.cs # Process memory read/write/save/restore
  Forms/MainForm.cs       # Main WinForms UI (large file)
  UI/ResumePanel.cs       # Overlay panel for pause/resume
```

### Key Patterns

- **Win32 P/Invoke**: Extensive use of `kernel32.dll` imports (`DllImport`) for process/memory/thread operations
- **Event-driven**: Components communicate via C# events (`EventHandler<T>`)
- **AllowUnsafeBlocks**: Enabled for low-level memory operations
- **Single-instance**: Enforced via named `Mutex` in `Program.cs`

## Code Conventions

- Namespace pattern: `ChooChooEngine.App.{Layer}` (Core, Injection, Memory, Forms, UI)
- Private fields: `_camelCase` prefix
- Win32 constants: `UPPER_SNAKE_CASE`
- P/Invoke declarations grouped in `#region Win32 API` blocks
- Event args: dedicated `{Feature}EventArgs` classes per component

## Important Notes

- This is a **Windows-only binary** designed to run under Proton/WINE on Linux/macOS
- The `AllowUnsafeBlocks` and P/Invoke usage is intentional for process manipulation
- `MainForm.cs` is the largest file — it contains the full WinForms UI with designer-generated code
- No test framework is currently configured
- Environment management uses `direnv` with `.envrc` and `dotenvx` for encrypted env vars
- Never commit `.env`, `.env.encrypted`, or `.env.keys` files

## GitHub Workflow

### Issue Templates

All issues MUST use the YAML form templates in `.github/ISSUE_TEMPLATE/`:

- **Bug Report** (`bug_report.yml`): Use `gh issue create --template bug_report.yml`
- **Feature Request** (`feature_request.yml`): Use `gh issue create --template feature_request.yml`
- **Compatibility Report** (`compatibility_report.yml`): Use `gh issue create --template compatibility_report.yml`

Blank issues are disabled via `config.yml`. Never bypass templates with `--title`-only issue creation.

Practical CLI limitation:

- `gh issue create` does not support combining `--template` with `--body` or `--body-file`.
- In this repo, `gh issue create --template ...` currently reports `no templates found` for the YAML issue forms, so the CLI is not discovering these form templates reliably.
- If this limitation blocks issue creation, use the GitHub API/tooling to create a fully structured issue body that mirrors the intended form fields, then apply the correct labels. Do not fall back to a vague or title-only issue.

### Pull Requests

PRs auto-populate from `.github/pull_request_template.md`. The template includes:

- `Closes #` issue linkage (always link the related issue)
- Type of Change checkboxes
- MSBuild verification checklist (`msbuild`, NOT `dotnet`)
- Conditional checks for Injection/, Memory/, Core/, and UI/ changes

CLI completion note:

- Zsh completion for `gh` may be loaded correctly while `gh` itself still returns no positional completions for PR or issue numbers.
- If `gh pr merge <TAB>` does not fill in PR identifiers, verify with `gh __complete pr merge \"\"`. If it returns only `:0`, that is a `gh` completion limitation, not necessarily a shell setup problem.

### Labels

Use the colon-prefixed label taxonomy — never create ad-hoc labels:

- `type:` bug, feature, docs, refactor, compatibility, build, migration
- `area:` injection, memory, process, ui, build, profiles, cli
- `platform:` steam-deck, linux, macos, wine, proton
- `priority:` critical, high, medium, low
- `status:` needs-triage, in-progress, blocked, needs-info
- Standalone: `good first issue`, `help wanted`, `duplicate`, `wontfix`
