# Local Build And Publish

This document captures the current local build and publish workflow for ChooChoo before CI/CD is added.

## Prerequisites

- .NET 9 SDK
- Repo root as the current working directory

Optional local SDK path if the SDK is installed under the repo:

```bash
PATH="$PWD/.dotnet:$PATH"
```

## Build

Build the solution:

```bash
dotnet build src/ChooChooEngine.sln -c Release
```

## Publish

Build the release artifacts:

```bash
./scripts/publish-dist.sh
```

By default this produces both supported RIDs:

```bash
dist/choochoo-win-x64/
dist/choochoo-win-x86/
dist/choochoo-win-x64.zip
dist/choochoo-win-x86.zip
```

To publish only one RID:

```bash
./scripts/publish-dist.sh win-x64
./scripts/publish-dist.sh win-x86
```

## Artifact Shape

Each `dist/choochoo-win-*` directory is a cleaned copy of the RID-specific `dotnet publish` output:

- `choochoo.pdb` is removed from the shipped artifact.
- `Profiles/`, `Settings/`, and `settings.ini` are removed from the shipped artifact because they are runtime/user state.
- The runtime host/runtime DLLs remain beside `choochoo.exe`.

The matching `.zip` file is the preferred release artifact to upload or copy into a test area.

Important: these publishes are self-contained, but they are not single-file publishes. The `choochoo.exe` apphost expects `choochoo.dll`, `choochoo.deps.json`, `choochoo.runtimeconfig.json`, the bundled runtime DLLs, and the adjacent `Profiles/`, `Settings/`, and `settings.ini` files to remain in the same published directory layout.

If you copy only `choochoo.exe` into another directory, startup fails with an error like:

```text
The application to execute does not exist: 'D:\...\choochoo.dll'
```

When testing or packaging a publish, copy the entire `dist/choochoo-win-*` directory as a unit, or use the generated zip.

The repo-root `choochoo.exe` is a legacy file already checked into the repository. The raw `src/ChooChooEngine.App/bin/Release/net9.0-windows/<rid>/publish/` directories are intermediate publish outputs; `dist/` is the release packaging output.

## Which Artifact To Use

- Use `win-x64` by default for 64-bit game and trainer combinations.
- Use `win-x86` when you need 32-bit compatibility.
- Keep both for release packaging because the current migration policy is dual artifacts.

## What Was Produced Earlier

The Phase 1 validation work produced both publish variants:

- `/tmp/choochoo-publish/win-x64/choochoo.exe`
- `/tmp/choochoo-publish/win-x86/choochoo.exe`

That means the earlier verification did not standardize on only one architecture. Both were built successfully. The later Steam validation did not explicitly identify which one was loaded, so that result should not be treated as proof of only `win-x64` or only `win-x86`.
