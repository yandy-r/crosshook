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

Publish the 64-bit artifact:

```bash
dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Release -r win-x64 --self-contained true
```

Publish the 32-bit artifact:

```bash
dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Release -r win-x86 --self-contained true
```

## Output Paths

The published executables are written to:

- `src/ChooChooEngine.App/bin/Release/net9.0-windows/win-x64/publish/choochoo.exe`
- `src/ChooChooEngine.App/bin/Release/net9.0-windows/win-x86/publish/choochoo.exe`

The repo-root `choochoo.exe` is a legacy file already checked into the repository. `dotnet publish` writes the new build outputs only under the `src/ChooChooEngine.App/bin/Release/net9.0-windows/<rid>/publish/` directories.

## Which Artifact To Use

- Use `win-x64` by default for 64-bit game and trainer combinations.
- Use `win-x86` when you need 32-bit compatibility.
- Keep both for release packaging because the current migration policy is dual artifacts.

## What Was Produced Earlier

The Phase 1 validation work produced both publish variants:

- `/tmp/choochoo-publish/win-x64/choochoo.exe`
- `/tmp/choochoo-publish/win-x86/choochoo.exe`

That means the earlier verification did not standardize on only one architecture. Both were built successfully. The later Steam validation did not explicitly identify which one was loaded, so that result should not be treated as proof of only `win-x64` or only `win-x86`.
