# dotnet-migrate

This plan now assumes a behavior-preserving migration, not a broad product redesign. The core objective is to move CrossHook Loader from classic `.NET Framework 4.8` project tooling to modern `.NET` while keeping the app a Windows binary that runs inside WINE/Proton and preserving the current profile/settings formats and injection flow.

## Working Decisions

- Target framework for the migration plan: `net9.0-windows`.
- UI framework: keep WinForms.
- Project structure: keep a single app project for the migration. Large structural moves such as a new `CrossHookEngine.Core` project or Avalonia rewrite are follow-up work.
- File formats: keep `.profile`, `settings.ini`, and `Settings/AppSettings.ini` unchanged.
- Publish architecture: dual artifacts (`win-x64` and `win-x86`). The codebase is currently `AnyCPU`, and `InjectionManager.ValidateDll()` depends on loader-process bitness, so this migration release must preserve 32-bit compatibility rather than collapsing to `win-x64`.
- Launch method compatibility: preserve serialized `LaunchMethod` values during the migration. Do not remove `CreateThreadInjection` or `RemoteThreadInjection` as part of the base migration without a compatibility story.
- CLI contract: the repo currently implements `-p` and `-autolaunch`; the README also claims `-dllinject`, which is not implemented. The migration release must either implement it or correct the docs.

## Key Risks

- WINE/Proton viability is the first hard gate. Refactoring should not proceed until the SDK-style build and published executable have been smoke-tested under WINE/Proton.
- `MainForm` has two real lifecycle issues today: duplicate manager event subscriptions and `FormClosing += (s, e) => OnFormClosing(e)`, which should be fixed before deeper refactors.
- `InjectionManager` uses two different DLL-loading paths:
  - Local validation uses imported `LoadLibrary`.
  - Remote injection resolves `"LoadLibraryA"` and writes `Encoding.ASCII` bytes.
    These must be documented separately. The current injection path does not safely support non-ASCII DLL paths.
- `AssemblyInfo.cs` cannot be treated as disposable metadata only. Version attributes can move into the SDK project, but `Guid` and `ComVisible` need an explicit keep/drop decision.

## Execution Priorities

1. Convert the project to SDK-style and map assembly metadata intentionally.
2. Update contributor docs and publish instructions.
3. Prove the new build starts under WINE/Proton before Phase 2.
4. Fix lifecycle/resource issues in `MainForm`, `ProcessManager`, and `InjectionManager`.
5. Extract only low-risk pure services from `MainForm`.
6. Convert P/Invoke declarations to `[LibraryImport]`.
7. Run a second WINE/Proton regression gate before optional cleanup.

## Critical Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook-loader/src/CrossHookEngine.App/CrossHookEngine.App.csproj`
- `/home/yandy/Projects/github.com/yandy-r/crosshook-loader/src/CrossHookEngine.App/Properties/AssemblyInfo.cs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook-loader/src/CrossHookEngine.App/Core/ProcessManager.cs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook-loader/src/CrossHookEngine.App/Injection/InjectionManager.cs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook-loader/src/CrossHookEngine.App/Memory/MemoryManager.cs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook-loader/src/CrossHookEngine.App/Forms/MainForm.cs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook-loader/README.md`
- `/home/yandy/Projects/github.com/yandy-r/crosshook-loader/CLAUDE.md`
