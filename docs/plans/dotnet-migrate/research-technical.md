# Technical Specifications: dotnet-migrate

## Executive Summary

The migration target for this plan is `net9.0-windows` with WinForms retained. The technical work is straightforward, but the docs need to stay honest about three things:

- release architecture is a decision, not a default
- `AssemblyInfo.cs` needs intentional handling
- `LoadLibrary` validation import and remote `LoadLibraryA` injection are different code paths

## Framework And Packaging

- target framework: `net9.0-windows`
- UI: WinForms
- project style: SDK-style
- publish architecture: dual artifacts (`win-x64` and `win-x86`)

Do not hard-code `win-x64` into the plan because the current codebase is `AnyCPU` and the injection validation path depends on loader-process bitness.

## SDK-Style Conversion Requirements

Required project properties:

- `TargetFramework`
- `OutputType`
- `UseWindowsForms`
- `AllowUnsafeBlocks`
- `Nullable`
- `ImplicitUsings`

Project-conversion notes:

- move agreed version/identity attributes into the project file
- keep or drop `ComVisible(false)` and assembly `Guid` intentionally
- remove `packages.config` only after confirming SharpDX remains unused

## Interop Modernization Rules

### General

- convert `[DllImport]` to `[LibraryImport]`
- add `partial` to containing classes as needed
- keep behavior equivalent before cleanup refactors

### ProcessManager

- treat `CreateProcess` as a deliberate marshalling decision
- preserve launch behavior while modernizing the signature

### InjectionManager

Document these separately:

1. imported `LoadLibrary` used for local validation
2. remote `LoadLibraryA` path used for actual injection

The migration may modernize the imported validation signature, but it must not silently rewrite the remote injection flow.

### MemoryManager

- low-risk mechanical conversion once cleanup is stable

## Known Technical Risks

### Architecture Drift

The existing project is `AnyCPU`. A move to fixed-architecture publish output changes runtime assumptions and DLL compatibility behavior.

### Shutdown Semantics

Before interop changes, the code should first fix:

- duplicate event subscriptions
- recursive `FormClosing` wiring
- missing `ProcessManager` disposal
- missing `InjectionManager` timer disposal

### ASCII Injection Path

The current remote injection path assumes ASCII-compatible DLL paths. This is a real limitation and should be documented as such instead of being hand-waved away by the `LoadLibrary` validation import changes.

## Support-Lifecycle Note

As of March 18, 2026, Microsoft’s support guidance lists:

- `.NET 10` support through November 2028
- `.NET 9` support through November 2026
- `.NET 8` support through November 2026

That makes WINE/Proton runtime behavior a more important planning variable than `.NET 8` versus `.NET 9` lifecycle alone for this package.
