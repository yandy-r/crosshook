# Code Analysis: dotnet-migrate

## Executive Summary

The live code supports a conservative migration plan. The highest-signal findings are not exotic interop blockers; they are planning details the earlier docs got wrong:

- `MainForm.cs` is the real sequencing bottleneck
- the shutdown path already has a bug
- architecture packaging is not frozen
- profile compatibility constrains enum cleanup

## Source-Level Findings

### Project And Packaging

- `CrossHookEngine.App.csproj` targets `.NET Framework 4.8`
- platform is `AnyCPU`
- `packages.config` contains `SharpDX 4.2.0`
- `AssemblyInfo.cs` still contains explicit assembly attributes beyond name/description

### MainForm

- constructor calls `InitializeManagers()` and `RegisterEventHandlers()`
- both methods subscribe to manager events
- `RegisterEventHandlers()` also hooks `FormClosing` back into `OnFormClosing`
- `btnRefreshProcesses.Click` is subscribed in two places

### ProcessManager

- owns unmanaged process handle state
- does not implement `IDisposable`
- placeholder launch methods still exist and are referenced by the enum

### InjectionManager

- owns a `System.Timers.Timer`
- uses imported `LoadLibrary` for local validation
- uses `"LoadLibraryA"` plus `Encoding.ASCII` for remote injection

### MemoryManager

- cleanest interop surface for mechanical modernization

## Planning Implications

- fix lifetime bugs before larger refactors
- keep `MainForm`-touching tasks sequential
- keep placeholder launch-method enum values unless compatibility handling is added
- do not assume `win-x64` as the only valid publish output
- document the ASCII-path limitation honestly

## Recommended Scope Boundaries

### In Scope

- SDK-style conversion
- assembly metadata mapping
- contributor-doc updates
- WINE/Proton smoke gate
- lifecycle cleanup
- persistence and CLI extraction
- interop conversion

### Out Of Scope

- new core project split
- broad product cleanup
- config-format migration
- launch-method removal
- large style-only modernization
