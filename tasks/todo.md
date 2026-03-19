# merge conflict fix for pr 8

# validate and fix important issues im-5 through im-8 for pr 8

## Scope

- Pull request: `#8`
- Review doc: `docs/pr-reviews/pr-8-review.md`
- Goal: validate IM-5 through IM-8 against the current branch, implement only the confirmed fixes, run focused verification, update the review doc with final status/evidence, and commit the progress

## Plan

- [x] Inspect the current branch and confirm whether IM-5 through IM-8 are still present before editing code.
- [x] Add focused reproducer tests that prove the reported failures on the current code paths.
- [x] Fix INI boolean parsing so malformed user-edited values no longer throw during settings/profile load.
- [x] Remove silent fallback behavior from manual-mapping and undisclosed launch-method stubs, and make unsupported selections fail explicitly.
- [x] Handle `Process.Start()` null returns so process-launch flows fail cleanly instead of storing a null process.
- [x] Run focused validation for the touched project files.
- [x] Update `docs/pr-reviews/pr-8-review.md` with final statuses and validation notes.
- [x] Commit the confirmed progress without touching unrelated worktree changes.

## Review

- Validation confirmed IM-5 through IM-8 are real on the current branch before implementation: the settings/profile loaders still used `bool.Parse`, manual mapping still delegated to standard injection, both thread-injection launch modes still delegated to `CreateProcess`, and the `Process.Start()` call sites still assigned or dereferenced the result without a null guard.
- `src/ChooChooEngine.App/Services/AppSettingsService.cs` and `src/ChooChooEngine.App/Services/ProfileService.cs` now use `bool.TryParse`, so malformed user-edited boolean values no longer crash file loading and unrelated keys still load.
- `src/ChooChooEngine.App/Injection/InjectionManager.cs` now fails manual mapping explicitly and surfaces a clear unsupported-method message instead of silently taking the standard `LoadLibraryA` path.
- `src/ChooChooEngine.App/Core/ProcessManager.cs` now fails the two unimplemented thread-injection launch modes explicitly and routes every `Process.Start()` call through `TryRequireStartedProcess(...)`, so null returns fail cleanly instead of leaving `_process` unusable.
- Added `tests/ChooChooEngine.App.Tests/InjectionManagerUnsupportedMethodTests.cs` and `tests/ChooChooEngine.App.Tests/ProcessManagerLaunchMethodTests.cs`, and extended the service tests so the malformed-INI and unsupported-method/null-start regressions are covered directly.
- Focused verification passed with `PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Debug`.
- Focused verification passed with `PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~AppSettingsServiceTests|FullyQualifiedName~ProfileServiceTests|FullyQualifiedName~InjectionManagerTests|FullyQualifiedName~InjectionManagerUnsupportedMethodTests|FullyQualifiedName~ProcessManagerThreadOperationTests|FullyQualifiedName~ProcessManagerLaunchMethodTests"`.

# validate and fix important issues im-1 through im-4 for pr 8

## Scope

- Pull request: `#8`
- Review doc: `docs/pr-reviews/pr-8-review.md`
- Goal: validate IM-1 through IM-4 against the current branch, implement only the confirmed fixes, run focused verification, update the review doc with final status/evidence, and commit the progress

## Plan

- [x] Inspect the current branch and confirm whether IM-1 through IM-4 are still present before editing code.
- [x] Fix startup initialization so app settings are applied before profile auto-load decisions, and populate controls exactly once.
- [x] Restore the auto-load last profile feature with command-line profile precedence.
- [x] Wire the resize handlers and unify the resize debounce timer strategy.
- [x] Add focused regression tests for startup profile selection logic and run targeted validation.
- [x] Update `docs/pr-reviews/pr-8-review.md` with final statuses and validation notes.
- [x] Commit the confirmed progress without touching unrelated worktree changes.

## Review

- Validation confirmed IM-1 through IM-4 are real on the current branch before implementation: the constructor never called `PopulateControls()`, no startup path auto-loaded `_lastUsedProfile`, the resize handlers were not registered, and two incompatible resize timer setups existed.
- `src/ChooChooEngine.App/Forms/MainForm.cs` now loads app settings before initial population, calls `PopulateControls()` exactly once, auto-loads the saved profile when enabled, and lets command-line `-p` requests override that saved startup behavior.
- `src/ChooChooEngine.App/Forms/MainForm.cs` now wires `SizeChanged` and `ResizeEnd` and uses one shared 100ms debounce timer instead of constructor-time and handler-time timer variants.
- Added `src/ChooChooEngine.App/Forms/MainFormStartupCoordinator.cs` and `tests/ChooChooEngine.App.Tests/MainFormStartupCoordinatorTests.cs` so the startup auto-load decision is covered by focused unit tests without trying to host WinForms in the non-Windows test project.
- Focused verification passed with `PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.sln -c Debug`.
- Focused verification passed with `PATH="$PWD/.dotnet:$PATH" DOTNET_CLI_HOME="$PWD/.dotnet-cli-home" NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~MainFormStartupCoordinatorTests|FullyQualifiedName~AppSettingsServiceTests|FullyQualifiedName~ProfileServiceTests|FullyQualifiedName~CommandLineParserTests"`.

## validate and fix critical issues 9-10 for pr 8

### Scope

- Pull request: `#8`
- Review doc: `docs/pr-reviews/pr-8-review.md`
- Goal: validate CR-9 and CR-10 against the current branch, implement only the confirmed fixes, run focused verification, update the review doc with final status/evidence, and commit the progress

### Plan

- [x] Inspect the current branch and confirm whether CR-9 and CR-10 are still present before editing code.
- [x] Confirm the specific failure paths in `InjectionManager` so the fix stays narrow and testable.
- [x] Make `InjectDllStandard()` handle `WaitForSingleObject` and `GetExitCodeThread` failure conditions explicitly instead of treating them as success.
- [x] Fix `IsDll64Bit()` to read the PE Optional Header magic intentionally rather than relying on a comment/offset mismatch.
- [x] Run focused validation for the touched project files.
- [x] Update `docs/pr-reviews/pr-8-review.md` with final statuses and validation notes.
- [ ] Commit the confirmed progress without touching unrelated worktree changes.

### Review

- Validation confirmed that CR-9 and CR-10 are still present on the current branch before implementation.
- `src/ChooChooEngine.App/Injection/InjectionManager.cs` still discards the return from `WaitForSingleObject(...)` and immediately calls `GetExitCodeThread(...)`, so a timeout/failure path can be misread as a successful injection.
- `src/ChooChooEngine.App/Injection/InjectionManager.cs` still advances past the entire 20-byte COFF header and then reads the Optional Header magic while the code/comment still describes that value as COFF `Characteristics`.
- Added explicit `WAIT_OBJECT_0` / `WAIT_TIMEOUT` / `WAIT_ABANDONED` / `WAIT_FAILED` handling and `GetExitCodeThread` validation in `InjectionManager`, so the remote-thread injection path now fails with a real diagnostic instead of accepting timeout/failure cases as success.
- Refactored the PE architecture check to read the Optional Header magic intentionally through `TryReadIsDll64Bit(Stream)`, which correctly distinguishes PE32 from PE32+ and removes the previous offset/comment mismatch.
- Focused verification passed with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Debug`.
- Focused verification passed with `DOTNET_ROLL_FORWARD=Major DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~InjectionManagerTests"`.

## validate and fix critical issues 5-8 for pr 8

### Scope

- Pull request: `#8`
- Review doc: `docs/pr-reviews/pr-8-review.md`
- Goal: validate CR-5 through CR-8 against the current branch, implement only the confirmed fixes, run focused verification, update the review doc with final status/evidence, and commit the progress

### Plan

- [x] Inspect the current branch and confirm whether CR-5 through CR-8 are still present before editing code.
- [x] Confirm the related failure paths in `Kernel32Interop`, `ProcessManager`, and `MemoryManager` so the fix scope stays narrow.
- [x] Add `SetLastError = true` where missing and surface Win32 error details at the affected failure sites.
- [x] Make suspend/resume report failure when `OpenThread`, `SuspendThread`, or `ResumeThread` fail instead of returning success unconditionally.
- [x] Run focused validation for the touched project files.
- [x] Update `docs/pr-reviews/pr-8-review.md` with final statuses and validation notes.
- [ ] Commit the confirmed progress without touching unrelated worktree changes.

### Review

- Validation confirmed that CR-5 through CR-8 are still present on the current branch before implementation.
- `src/ChooChooEngine.App/Interop/Kernel32Interop.cs` still omits `SetLastError = true` on `OpenProcess`, `CreateRemoteThread`, `WriteProcessMemory`, `VirtualAllocEx`, and `VirtualFreeEx`.
- `src/ChooChooEngine.App/Core/ProcessManager.cs` still omits `SetLastError = true` on `OpenThread`, and both `SuspendProcess()` and `ResumeProcess()` currently ignore `OpenThread` and `SuspendThread`/`ResumeThread` failure conditions.
- `src/ChooChooEngine.App/Memory/MemoryManager.cs` still omits `SetLastError = true` on `ReadProcessMemory`, `WriteProcessMemory`, and `VirtualQueryEx`, so memory-operation failure messages currently lose the underlying Win32 error.
- Added `SetLastError = true` to the affected shared interop, process-thread, and memory-manager `LibraryImport` declarations and introduced `Win32ErrorHelper` so failure messages consistently include the Win32 code and description.
- Updated `InjectionManager.InjectDllStandard()` to emit explicit `VirtualAllocEx`, `WriteProcessMemory`, and `CreateRemoteThread` error messages and to log `VirtualFreeEx` cleanup failures.
- Refactored `ProcessManager` thread suspend/resume handling through `TryExecuteThreadOperation(...)`, so `OpenThread`, `SuspendThread`, and `ResumeThread` failures now return `false` instead of silently reporting success.
- Updated `MemoryManager` to distinguish true API failures from short read/write transfers and to surface `VirtualQueryEx` errors instead of collapsing them into generic messages.
- Focused verification passed with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Debug`.
- Focused verification passed with `DOTNET_ROLL_FORWARD=Major DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj --filter "FullyQualifiedName~InteropLibraryImportTests|FullyQualifiedName~ProcessManagerThreadOperationTests|FullyQualifiedName~Win32ErrorHelperTests"`.

## Scope

- Pull request: `#8`
- Branch: `feat/dotnet-migrate`
- Goal: merge current `main` into the migration branch, resolve the documentation conflicts, and restore PR mergeability without regressing the migrated project guidance

## Plan

- [x] Confirm the actual conflict set against current `origin/main`.
- [x] Merge `origin/main` into `feat/dotnet-migrate` and inspect the conflict markers.
- [x] Resolve `.github/pull_request_template.md` to keep the post-migration `dotnet build` / `dotnet publish` checklist.
- [x] Resolve `CLAUDE.md` to keep the migrated `.NET 9` guidance and retain the newer GitHub workflow notes.
- [x] Run focused validation and confirm the branch is ready to push.
- [ ] Add the review summary with the conflict cause and resolution.

## Review

- The merge conflict was limited to `.github/pull_request_template.md` and `CLAUDE.md`.
- `main` had newer GitHub workflow text, but it still referenced the pre-migration `msbuild` flow.
- The correct resolution kept the migration branch’s `dotnet build` / `dotnet publish` guidance and preserved the newer practical workflow notes about issue-template CLI limitations and `gh` completion behavior.
- Focused validation passed with `git diff --check`, and the branch was left ready for the final merge commit and push.

## Previous Task History

### dotnet-migrate issue 6

## Scope

- GitHub issue: `#6`
- Plan tasks: `4.1`, `4.2`
- Goal: apply small, reviewable post-migration cleanup after the proven Phase 3 regression gate without changing runtime behavior or reopening migration decisions

## Plan

- [x] Confirm issue `#6` scope and map it to the optional Phase 4 tasks in `docs/plans/dotnet-migrate/parallel-plan.md`.
- [x] Validate prerequisites and confirm the post-Phase-3 build/regression gate is already green before starting optional cleanup.
- [x] Task `4.1` Deduplicate only the duplicated `kernel32` interop used by the process/injection managers into a narrow shared interop file.
- [x] Task `4.2` Apply targeted nullable/style cleanup in low-risk service files without broad churn.
- [x] Integrate the task changes and run targeted build/test validation.
- [x] Add the implementation review with files changed, validation results, and any intentionally deferred cleanup.

## Review

- Added [Kernel32Interop.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Interop/Kernel32Interop.cs) to hold the shared `kernel32` source-generated imports used by both process and injection management.
- Removed the duplicated `OpenProcess`, `CloseHandle`, `VirtualAllocEx`, `VirtualFreeEx`, `WriteProcessMemory`, and `CreateRemoteThread` declarations from [ProcessManager.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Core/ProcessManager.cs) and [InjectionManager.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Injection/InjectionManager.cs), and updated call sites to use the shared interop class.
- Kept the remote `LoadLibraryA` injection flow, the ASCII path conversion, and the rest of the manager-specific imports unchanged.
- Implemented the optional Phase 4 style cleanup in [ProfileService.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Services/ProfileService.cs), [RecentFilesService.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Services/RecentFilesService.cs), [AppSettingsService.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Services/AppSettingsService.cs), and [CommandLineParser.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Services/CommandLineParser.cs).
- Converted the four service files to file-scoped namespaces and tightened constructor/null-argument guards with `ArgumentNullException.ThrowIfNull`.
- Kept runtime behavior and persisted formats unchanged.
- Verification passed with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.sln -c Release`.
- Verification passed with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache DOTNET_ROLL_FORWARD=Major dotnet test tests/ChooChooEngine.App.Tests/ChooChooEngine.App.Tests.csproj -c Release`.
- Verification passed with `DOTNET_ROLL_FORWARD=Major DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet test src/ChooChooEngine.sln -c Release --no-build`.

## Previous Task History

### dotnet-migrate issue 5

#### Scope

- GitHub issue: `#5`
- Plan tasks: `3.1`, `3.2`, `3.3`, `3.4`
- Goal: convert the three interop-heavy managers to `[LibraryImport]` and run the post-conversion regression gate as far as the local environment allows

#### Plan

- [x] Confirm issue `#5` scope and map it to `docs/plans/dotnet-migrate/parallel-plan.md`.
- [x] Validate implementation prerequisites and read the plan/research context for the interop conversion rules.
- [x] Execute batch 1:
- [x] Task `3.1` Convert `ProcessManager` to `[LibraryImport]`, including intentional `CreateProcess` marshalling.
- [x] Task `3.2` Convert `InjectionManager` to `[LibraryImport]`, keeping local validation import behavior separate from the remote `LoadLibraryA` injection path.
- [x] Task `3.3` Convert `MemoryManager` to `[LibraryImport]`.
- [x] Integrate the batch 1 changes, resolve conflicts, and run targeted build validation.
- [x] Execute batch 2:
- [x] Task `3.4` Run the WINE/Proton regression gate or document the environment gap plus any substitute local verification.
- [x] Add the implementation review with files changed, validation results, and unresolved gaps.

#### Review

- Converted the Win32 declarations in [ProcessManager.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Core/ProcessManager.cs) from `[DllImport]` to `[LibraryImport]`.
- Made the `CreateProcess` import explicit with `CreateProcessW` and UTF-16 string marshalling, and adjusted `STARTUPINFO` to be source-generator friendly.
- Verification passed with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.sln -c Release`.
- Converted the Win32 declarations in [InjectionManager.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Injection/InjectionManager.cs) from `[DllImport]` to `[LibraryImport]`.
- Preserved the local validation `LoadLibrary` import separately from the remote `LoadLibraryA` injection path and kept the remote path ASCII-based.
- Converted the Win32 declarations in [MemoryManager.cs](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/src/ChooChooEngine.App/Memory/MemoryManager.cs) from `[DllImport]` to `[LibraryImport]` and marked the class `partial` for source-generated interop.
- Integrated verification passed with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet build src/ChooChooEngine.sln -c Release`.
- Sequential publish verification passed for both RIDs:
- `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Release -r win-x64 --self-contained true -o /tmp/choochoo-regression/win-x64`
- `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Release -r win-x86 --self-contained true -o /tmp/choochoo-regression/win-x86`
- Local WINE smoke verification improved versus the earlier Phase 1 workspace result: both published binaries stayed alive until `timeout` terminated them, and the x64 run registered a `choochoo.exe` window class instead of failing immediately with `System.Runtime.dll` load errors.
- Supplemental ad hoc harness validation under local WINE confirmed `launch` and `attach` for both x64 and x86. The harness then failed at `ValidateDll` when probing `C:\\windows\\system32\\kernel32.dll`, so the local validation/import path remains inconclusive in this workspace.
- Primary runtime evidence for task `3.4`: the user reported that the x64 artifact produced by `scripts/publish-dist.sh` runs fine when launched manually. That target-style validation is treated as the strongest regression-gate signal for this task.

### dist packaging cleanup

#### Plan

- [x] Confirm the modern .NET publish layout and why moving runtime DLLs out of the apphost directory is not safe.
- [x] Verify whether single-file publish is technically possible for the migrated project.
- [x] Add a repeatable packaging script that publishes per-RID release artifacts into clean `dist/` directories and creates the exact zip files to ship.
- [x] Update repo ignore rules for generated `dist/` artifacts.
- [x] Update public and internal docs so `dist/` outputs, not raw `bin/.../publish`, are the release deliverables.
- [x] Run focused validation on the new packaging workflow and record the exact commands/results.

#### Review

- Added [publish-dist.sh](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/scripts/publish-dist.sh) to publish `win-x64` and `win-x86` into canonical `dist/choochoo-win-*` directories and matching `dist/choochoo-win-*.zip` artifacts.
- The packaging workflow keeps the required runtime/apphost payload flat beside `choochoo.exe`, but removes `choochoo.pdb` from the shipped artifact.
- Updated [README.md](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/README.md) and [local-build-publish.md](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/docs/internal-docs/local-build-publish.md) so `dist/` is the thing to ship, not the raw `bin/.../publish` directory.
- Added `dist/` to [.gitignore](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/.gitignore).
- Validation passed with `DOTNET_CLI_HOME=/tmp/dotnet-cli-home NUGET_PACKAGES=/tmp/nuget-packages NUGET_HTTP_CACHE_PATH=/tmp/nuget-http-cache ./scripts/publish-dist.sh`, which produced:
- `dist/choochoo-win-x64/`
- `dist/choochoo-win-x86/`
- `dist/choochoo-win-x64.zip`
- `dist/choochoo-win-x86.zip`

### release docs refresh

#### Plan

- [x] Find repo documentation that still points users to `releases/latest` or implies ChooChoo ships as a single executable.
- [x] Update user-facing download and install guidance to send users to the GitHub Releases page and explain the zip extraction flow.
- [x] Update internal packaging docs to reference `.github/workflows/release.yml` as the standard release path.
- [x] Run focused verification on the updated documentation.

#### Review

- Updated [README.md](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/README.md) so the download badges and install guidance point to the GitHub Releases page instead of `releases/latest`.
- Added explicit README guidance that users should download `choochoo-win-x64.zip` or `choochoo-win-x86.zip`, extract the full archive into a directory of their choice, and run the extracted `choochoo.exe`.
- Updated the Steam Deck and Whisky quick-start sections in [README.md](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/README.md) so setup starts from the extracted release folder rather than treating `choochoo.exe` as a standalone artifact.
- Updated [release_notes.md](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/release_notes.md) with the same Releases-page and extraction guidance, and removed an unrelated corrupted shell-snippet fragment that had leaked into the touch-interface bullet list.
- Updated [local-build-publish.md](/home/yandy/Projects/github.com/yandy-r/choochoo-loader/docs/internal-docs/local-build-publish.md) to describe `.github/workflows/release.yml` as the standard release packaging path and to keep end-user instructions zip-based.
- Focused verification passed with `rg -n "releases/latest|latest release" README.md release_notes.md docs/internal-docs/local-build-publish.md`, which returned no matches.
- Focused verification passed with `git diff --check -- README.md release_notes.md docs/internal-docs/local-build-publish.md tasks/todo.md`.

### changelog automation

#### Plan

- [x] Add repo-local `git-cliff` configuration for changelog generation from tags and conventional commits.
- [x] Add a release-prep script that regenerates `CHANGELOG.md`, commits it, creates the tag, and optionally pushes branch then tag.
- [x] Document the release-prep flow alongside the existing publish workflow.
- [x] Validate the generated changelog output with the actual `git-cliff` binary.

#### Review

- Added `.git-cliff.toml` to group conventional commits into a deterministic `CHANGELOG.md`.
- Added `scripts/prepare-release.sh` with `--version` or `--tag`, optional `--push`, clean-worktree checks, changelog generation, release commit creation, and annotated tag creation.
- Added a generated `CHANGELOG.md` to the repo so the changelog exists before the next release prep run.
- Updated `docs/internal-docs/local-build-publish.md` with a release-prep section that explains the changelog-commit-before-tag flow.
- Validation passed with `/tmp/git-cliff/bin/git-cliff --config .git-cliff.toml`, which successfully generated both an unreleased changelog and a versioned preview.
- End-to-end validation passed in a disposable temp clone with `PATH="/tmp/git-cliff/bin:$PATH" ./scripts/prepare-release.sh --version 9.9.9`, which created `CHANGELOG.md`, committed `chore(release): prepare v9.9.9`, and created the annotated tag `v9.9.9` without pushing.
