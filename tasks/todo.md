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
- [x] Commit confirmed progress.

## 2026-03-19 - PR #8 TC-1 through TC-8 validation

- [x] Read `docs/pr-reviews/pr-8-review.md` and extract TC-1 through TC-8.
- [x] Validate TC-1 through TC-8 against the current source and tests before implementing.
- [x] Add targeted automated tests for each still-real coverage gap.
- [x] Run targeted test verification for the touched service/parser suites.
- [x] Update `docs/pr-reviews/pr-8-review.md` with validated status changes and verification commands.
- [x] Commit confirmed progress.

## Review

- IM-9 validated as real: `CreateMiniDump(...)` ignored the `MiniDumpWriteDump(...)` return value and returned `true` on silent failure. Fixed by checking the result, logging the Win32 error, and deleting the incomplete dump file.
- IM-10 validated as real: ProcessManager and InjectionManager still depended on `Debug.WriteLine(...)`. Fixed by adding `AppDiagnostics` with file-backed trace logging and routing the affected error paths through it.
- IM-11 validated as real: `Program.Main(...)` had no global exception wiring. Fixed by initializing diagnostics logging at startup and subscribing both WinForms and AppDomain unhandled-exception hooks.
- IM-12 validated as real: `CurrentProcess` exposed the manager-owned `Process` instance directly. Fixed by returning a detached process snapshot.
- IM-13 validated as real: `InjectionManager` accepted a null `ProcessManager` and deferred failure until first use. Fixed by throwing `ArgumentNullException` in the constructor.
- IM-14 validated as real: `ProfileService.GetProfilePath(...)` accepted traversal and path-separator input verbatim. Fixed by centralizing `ValidateProfileName(...)` and rejecting rooted, relative, or invalid path-like names before file access.
- TC-1/2/3/5/6/7/8 validated as real coverage gaps in the current branch. Fixed by adding focused tests for `ProfileService.DeleteProfile`, missing-profile load failures, `GetProfileNames`, missing recent-files defaults, service round-trips, unknown CLI flags, and constructor null guards.
- TC-4 was stale in the review doc rather than a real gap. The existing `LoadAppSettings_IgnoresInvalidBooleanValues_InsteadOfThrowing` test already covered the corrupt-boolean case, and the review entry was updated to reflect that.
- Verification passed:
  `dotnet build src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Debug`
  `dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~AppDiagnosticsTests|FullyQualifiedName~ProcessManagerDiagnosticsTests|FullyQualifiedName~ProcessManagerLaunchMethodTests|FullyQualifiedName~InjectionManagerTests"`
  `dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~InjectionManagerUnsupportedMethodTests|FullyQualifiedName~ProfileServiceTests"`
  `dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~ProfileServiceTests|FullyQualifiedName~AppSettingsServiceTests|FullyQualifiedName~RecentFilesServiceTests|FullyQualifiedName~CommandLineParserTests"`
