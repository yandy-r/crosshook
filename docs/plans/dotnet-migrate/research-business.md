# Business Logic Research: dotnet-migrate

## Executive Summary

The migration must preserve ChooChoo Loader as a Windows-targeted launcher and DLL injector running under WINE/Proton. The business-critical behavior is not the old project format; it is the current launch, attach, profile, and injection behavior. The main planning correction is that the docs must distinguish between:

- behavior the code already implements
- behavior the README claims but the code does not implement yet

## Preserve As-Is

- single-instance enforcement via named `Mutex`
- current game/trainer launch workflows
- current profile and settings file formats
- current `LaunchMethod` persistence by enum name
- current remote injection flow using `LoadLibraryA`
- current DLL architecture validation model

## Known Mismatches That Must Be Resolved Before Release

### CLI Contract

Implemented today:

- `-p`
- `-autolaunch`

Claimed in docs but not implemented:

- `-dllinject`

The migration release must either implement `-dllinject` or correct the docs.

### Launch Method Contract

The UI and profile format currently expose six launch-method enum values, but two are placeholders that fall back to `CreateProcess`.

Implication:

- do not remove `CreateThreadInjection` or `RemoteThreadInjection` during the migration without a compatibility strategy for existing `.profile` files

## Business-Critical Technical Path

The most important path to preserve is still:

1. launch or attach to a process
2. validate the DLL
3. allocate memory in the target process
4. write the DLL path
5. resolve `"LoadLibraryA"`
6. create the remote thread
7. wait and inspect the result

That path must remain stable through the migration.

## Important Edge Cases

- existing `.profile` files may contain launch-method values that correspond to placeholder enum members
- recent-files loading intentionally ignores paths that no longer exist
- `ValidateDll()` loads candidate DLLs into the ChooChoo process for validation, which is behaviorally meaningful and should not be casually redesigned during the migration
- the current injection path assumes ASCII-compatible DLL paths

## Count Corrections

- The codebase uses 19 unique Win32 APIs across `kernel32.dll` and `Dbghelp.dll`.
- The migration plan should use that number consistently.
