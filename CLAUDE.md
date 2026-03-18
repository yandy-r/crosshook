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
