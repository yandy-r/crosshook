# Task Checklist

## 2026-03-19 - PR #8 IM-9 through IM-12 validation

- [x] Read `docs/pr-reviews/pr-8-review.md` and extract IM-9 through IM-12.
- [x] Validate IM-9 through IM-12 against the current source before implementing.
- [x] Implement confirmed fixes for IM-9 through IM-12 with minimal code impact.
- [x] Add targeted automated tests for the confirmed fixes.
- [x] Run targeted build and test verification for the touched areas.
- [x] Update `docs/pr-reviews/pr-8-review.md` with validation notes, fix status, and verification commands.
- [x] Commit confirmed progress.

## 2026-03-19 - PR #8 IM-13 through IM-14 validation

- [x] Read `docs/pr-reviews/pr-8-review.md` and extract IM-13 through IM-14.
- [x] Validate IM-13 through IM-14 against the current source before implementing.
- [x] Implement confirmed fixes for IM-13 through IM-14 with minimal code impact.
- [x] Add targeted automated tests for the confirmed fixes.
- [x] Run targeted build and test verification for the touched areas.
- [x] Update `docs/pr-reviews/pr-8-review.md` with validation notes, fix status, and verification commands.
- [ ] Commit confirmed progress.

## Review

- IM-9 validated as real: `CreateMiniDump(...)` ignored the `MiniDumpWriteDump(...)` return value and returned `true` on silent failure. Fixed by checking the result, logging the Win32 error, and deleting the incomplete dump file.
- IM-10 validated as real: ProcessManager and InjectionManager still depended on `Debug.WriteLine(...)`. Fixed by adding `AppDiagnostics` with file-backed trace logging and routing the affected error paths through it.
- IM-11 validated as real: `Program.Main(...)` had no global exception wiring. Fixed by initializing diagnostics logging at startup and subscribing both WinForms and AppDomain unhandled-exception hooks.
- IM-12 validated as real: `CurrentProcess` exposed the manager-owned `Process` instance directly. Fixed by returning a detached process snapshot.
- IM-13 validated as real: `InjectionManager` accepted a null `ProcessManager` and deferred failure until first use. Fixed by throwing `ArgumentNullException` in the constructor.
- IM-14 validated as real: `ProfileService.GetProfilePath(...)` accepted traversal and path-separator input verbatim. Fixed by centralizing `ValidateProfileName(...)` and rejecting rooted, relative, or invalid path-like names before file access.
- Verification passed:
  `dotnet build src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Debug`
  `dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~AppDiagnosticsTests|FullyQualifiedName~ProcessManagerDiagnosticsTests|FullyQualifiedName~ProcessManagerLaunchMethodTests|FullyQualifiedName~InjectionManagerTests"`
  `dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~InjectionManagerUnsupportedMethodTests|FullyQualifiedName~ProfileServiceTests"`
