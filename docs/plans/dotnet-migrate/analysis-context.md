# Context Analysis: dotnet-migrate

## Executive Summary

The codebase supports a modern-.NET migration, but the original plan mixed migration work with product cleanup and future architecture ideas. The corrected context is:

- modernize the project and runtime first
- prove it under WINE/Proton
- then do targeted cleanup and interop modernization

## Current-State Facts

- Single application project: `src/CrossHookEngine.App`
- Current build target: `.NET Framework 4.8`, `AnyCPU`
- Main code bottleneck: `src/CrossHookkEngine.App/Forms/MainForm.cs`
- Current persistence root: `Application.StartupPath`
- Current profile format stores `LaunchMethod` by enum name
- Current remote injection path uses `"LoadLibraryA"` plus `Encoding.ASCII`

## Key Source Observations

- `MainForm` currently has duplicate manager event subscriptions.
- `MainForm` also wires `FormClosing` back into its own override, which should be removed.
- `ProcessManager` owns unmanaged handle state and needs explicit disposal.
- `InjectionManager` owns a `System.Timers.Timer` and needs explicit cleanup, not just `StopMonitoring()`.
- The project still carries `AssemblyInfo.cs` with more than simple title/description metadata.
- The codebase does not implement `-dllinject` even though README text claims it.

## Corrected Planning Assumptions

- The migration should keep WinForms and the single-project structure.
- The migration should preserve current file formats and serialized launch-method values.
- The migration should not assume `win-x64` only until release architecture is explicitly decided.
- The migration should not assume `Application.StartupPath` behavior; it should validate it under the published build.
- The migration should not treat placeholder launch methods as removable by default.

## High-Risk Areas

### Runtime Gate

The riskiest dependency is WINE/Proton behavior of the published modern-.NET build. That must be validated before refactoring work begins.

### MainForm Bottleneck

`MainForm.cs` is the shared write hotspot for:

- lifecycle bug fixes
- persistence extraction
- command-line extraction
- shutdown behavior changes

This work should be sequenced.

### Interop Semantics

The docs must keep these cases separate:

- `LoadLibrary` import for local validation
- `LoadLibraryA` remote thread used for actual injection

Changing the validation import to UTF-16 does not solve the remote ASCII injection-path limitation.

## Working Decisions For The Plan

- `net9.0-windows`
- WinForms stays
- single project stays
- file formats stay
- `[LibraryImport]` is the modernization target
- publish architecture remains an explicit release decision

## Explicitly Deferred

- Avalonia
- new core library split
- JSON/TOML migration
- launch-method cleanup that breaks persisted values
- broad style-only cleanup
