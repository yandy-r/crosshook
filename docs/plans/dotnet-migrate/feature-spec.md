# Feature Spec: dotnet-migrate

## Executive Summary

CrossHook Loader can be migrated from `.NET Framework 4.8` to modern `.NET` without changing its core product model: it remains a WinForms Windows executable running under WINE/Proton, and it continues to launch games, trainers, and DLL injections using Win32 APIs. The migration should be scoped as a tooling/runtime modernization with targeted cleanup, not as a combined UI rewrite, architecture rewrite, and config-format rewrite.

The first hard success criterion is not `dotnet build`; it is a published build that starts and behaves correctly under WINE/Proton. Only after that gate passes should the plan move into refactors or `LibraryImport` modernization.

## Decision Freeze

These choices are treated as in-scope decisions for this plan:

- Target framework: `net9.0-windows`.
- UI framework: WinForms stays.
- Project topology: keep the existing single app project for the migration.
- Config formats: preserve `.profile`, `settings.ini`, and `Settings/AppSettings.ini`.
- P/Invoke approach: manual `[LibraryImport]` conversion; CsWin32 is optional follow-up work.
- Publish packaging: architecture is intentionally not frozen to `win-x64` yet.

These items must be resolved before implementation starts:

1. Release architecture policy:
   Decide whether the migration ships `win-x64`, `win-x86`, or dual artifacts. The current project is `AnyCPU`, and DLL validation depends on the loader process bitness.
2. CLI contract:
   Decide whether the migration release implements `-dllinject` or corrects the README and docs to match the current code.
3. Assembly metadata:
   Decide which `AssemblyInfo.cs` attributes move into the SDK project and whether `Guid` and `ComVisible(false)` are intentionally dropped or retained.

## Goals

- Move the app to an SDK-style project that builds with `dotnet build`.
- Preserve current runtime behavior under WINE/Proton.
- Preserve current file formats and profile compatibility.
- Fix known lifecycle/resource issues that would make the migrated build brittle.
- Modernize the existing P/Invoke declarations with `[LibraryImport]`.
- Reduce the `MainForm` monolith only where the seams are low-risk and behavior-preserving.

## Non-Goals

- Avalonia or other UI framework migration.
- New `CrossHookkEngine.Core` project or full interface-based architecture split.
- JSON/TOML/profile format migration.
- Native Linux support.
- NativeAOT.
- Removing placeholder launch-method enum values during the migration.
- Broad style-only cleanup across the whole codebase.

## Verified Facts From The Current Codebase

- The project is currently a classic `.csproj` targeting `.NET Framework 4.8` and `AnyCPU`.
- The solution contains one application project: `src/CrossHookkEngine.App`.
- There are 19 unique Win32 APIs across `kernel32.dll` and `Dbghelp.dll`.
- `MainForm` currently:
  - initializes managers and subscribes to their events in `InitializeManagers()`
  - subscribes to the same events again in `RegisterEventHandlers()`
  - hooks `FormClosing += (s, e) => OnFormClosing(e)` even though it already overrides `OnFormClosing`
- `ProcessManager` owns an unmanaged process handle and does not implement `IDisposable`.
- `InjectionManager` owns a long-lived `System.Timers.Timer` that is stopped on shutdown but not disposed.
- Profiles persist `LaunchMethod` by enum name and reload it with `Enum.TryParse`.
- The current injection path resolves `"LoadLibraryA"` and writes `Encoding.ASCII` bytes into the remote process.
- The imported `LoadLibrary` method is only used for local DLL validation, not the remote injection path.

## Business Constraints

- The app must remain a Windows binary hosted under WINE/Proton.
- The core injection path must stay behaviorally equivalent:
  `VirtualAllocEx -> WriteProcessMemory -> CreateRemoteThread -> LoadLibraryA`
- Existing `.profile` and INI files must continue to load.
- Existing serialized `LaunchMethod` values must not silently break.
- Single-instance enforcement via `Mutex` must remain intact.
- `AllowUnsafeBlocks` must remain enabled.

## Technical Constraints

- `MainForm.cs` is the main coordination bottleneck. Tasks that modify it should be sequenced, not treated as freely parallel.
- `Application.StartupPath` should not be assumed to change or remain identical under single-file publish without validation under WINE/Proton.
- The migration docs must distinguish:
  - `LoadLibrary` import for local validation
  - `LoadLibraryA` remote-thread injection for the target process
- Non-ASCII DLL paths are a known limitation of the current injection path. They are not solved by converting the validation import to UTF-16.

## Packaging Strategy

The plan should separate build modernization from release packaging:

- Development target: `net9.0-windows`
- Release artifact decision: `win-x64`, `win-x86`, or both

The docs should not assume a single `win-x64` publish unless the project explicitly drops 32-bit support. If 32-bit support is dropped, that is a release decision that must be documented as a compatibility change.

## Assembly Metadata Strategy

Do not blindly delete `AssemblyInfo.cs`.

Safe moves into the SDK project:

- `AssemblyTitle`
- `AssemblyDescription`
- `AssemblyCompany`
- `AssemblyProduct`
- `AssemblyCopyright`
- `AssemblyVersion`
- `AssemblyFileVersion`

Needs explicit decision before deletion:

- `ComVisible(false)`
- assembly `Guid`

If those are intentionally removed, the plan should say so explicitly instead of assuming they are unnecessary.

## Recommended In-Scope Refactors

- Fix duplicate event subscriptions.
- Remove the recursive `FormClosing` event hookup.
- Add explicit disposal for `ProcessManager`.
- Add explicit disposal/cleanup for `InjectionManager` timer ownership.
- Extract only pure or near-pure services from `MainForm`:
  - `ProfileService`
  - `RecentFilesService`
  - `AppSettingsService`
  - `CommandLineParser`

## Deferred Refactors

- `LaunchOrchestrator`
- `CrossHookkEngine.Core` split
- P/Invoke deduplication into shared interop files
- nullable-wide cleanup across every source file
- README/product cleanup beyond migration-related corrections

These are reasonable follow-up issues, but they should not block the base migration.

## Success Criteria

- [ ] `dotnet build src/CrossHookkEngine.sln -c Release` succeeds
- [ ] The SDK-style project maps assembly metadata intentionally
- [ ] Contributor docs no longer claim `dotnet build` is unsupported
- [ ] A published modern-.NET build launches under WINE/Proton
- [ ] Main UI renders and the main workflows still function
- [ ] Existing profiles and settings load without manual conversion
- [ ] Current launch-method values remain loadable from existing profiles
- [ ] Duplicate event delivery is fixed
- [ ] Form closing no longer routes through the override recursively
- [ ] `ProcessManager` unmanaged handle cleanup is explicit
- [ ] `InjectionManager` timer cleanup is explicit
- [ ] P/Invoke declarations are migrated to `[LibraryImport]`
- [ ] The remote `LoadLibraryA` injection path remains behaviorally unchanged

## Validation Gates

### Gate 1: Foundation

- build succeeds
- publish succeeds
- basic launch under WINE/Proton succeeds
- profile/settings path resolution still works

### Gate 2: Post-Cleanup

- event handlers fire once per action
- closing the form does not recurse or leak obvious resources
- profile/settings/CLI behavior still matches the pre-migration build

### Gate 3: Post-Interop

- DLL validation still works
- DLL injection still works with ASCII paths
- process attach/launch/suspend/resume still works
- regression smoke under WINE/Proton passes

## Release Notes Implications

The release must explicitly call out one of these outcomes:

1. `-dllinject` was implemented as part of the migration.
2. The README and help text were corrected because `-dllinject` remains out of scope.

If the migration narrows supported publish architectures, release notes must also call that out explicitly.
