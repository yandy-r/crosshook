# Integration Research: dotnet-migrate

## Executive Summary

The migration touches four integration surfaces:

1. the legacy project system
2. assembly metadata and packaging
3. file-system rooted configuration data
4. Win32 interop under WINE/Proton

The previous draft overstated some assumptions. This version treats architecture packaging and startup-path behavior as things to validate, not things to assume.

## Project-System Integration

- current project format: classic .NET Framework `.csproj`
- current solution shape: one app project
- current build target: `AnyCPU`

Migration implication:

- moving to SDK-style is straightforward
- release packaging must not silently change supported architecture without an explicit decision

## Assembly Metadata Integration

`AssemblyInfo.cs` currently carries more than marketing metadata. The migration must explicitly decide how to handle:

- version attributes
- `ComVisible(false)`
- assembly `Guid`

Do not document `AssemblyInfo.cs` as fully disposable until those choices are made.

## File-System Integration

The app stores everything relative to `Application.StartupPath`:

- `settings.ini`
- `Settings/AppSettings.ini`
- `Profiles/*.profile`

Migration implication:

- this rooting behavior must be validated on the published build under WINE/Proton
- keep a fallback plan ready, but do not rewrite file-root logic preemptively

## Interop Integration

There are two distinct DLL-loading cases in the current code:

### Local Validation Import

- imported `LoadLibrary`
- used to validate a DLL inside the CrossHook process
- appropriate place for Unicode-aware marshalling decisions

### Remote Injection Path

- resolves `"LoadLibraryA"` with `GetProcAddress`
- writes `Encoding.ASCII` bytes into the remote process
- this is the real injection contract that must remain stable during the migration

Planning implication:

- do not conflate these two cases in migration docs
- converting the validation import to UTF-16 does not change the remote injection path
- non-ASCII DLL paths remain a known limitation unless the remote injection strategy itself changes

## WINE/Proton Integration

The modernized build must be validated under the published artifact, not just from local builds. The plan should check:

- executable startup
- UI rendering
- settings/profile path resolution
- process listing
- launch/attach behavior
- injection smoke behavior

The plan should stop after Phase 1 if this integration point fails.
