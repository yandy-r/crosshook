# Local Build And Publish

This document captures the current local build and publish workflow for CrossHook and how it feeds the standard GitHub Releases packaging flow.

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
dotnet build src/CrossHookEngine.sln -c Release
```

## Publish

Build the release artifacts:

```bash
./scripts/publish-dist.sh
```

By default this produces both supported RIDs:

```bash
dist/crosshook-win-x64/
dist/crosshook-win-x86/
dist/crosshook-win-x64.zip
dist/crosshook-win-x86.zip
```

The standard distribution path is the GitHub Releases page. `.github/workflows/release.yml` runs `./scripts/publish-dist.sh` and uploads these two zip files when a `v*` tag is pushed or the workflow is dispatched manually.

## Prepare A Release

Use the repo-local release prep script to generate `CHANGELOG.md`, commit it, create the annotated release tag, and optionally push in the correct order.

Prerequisites:

- `git-cliff` installed locally, for example with `cargo install git-cliff --locked`
- A clean git worktree

Examples:

```bash
./scripts/prepare-release.sh --version 5.1.0
./scripts/prepare-release.sh --tag v5.1.0 --push
```

The script sequence is:

1. Regenerate `CHANGELOG.md` from git history using `.git-cliff.toml`
2. Commit the changelog update as `chore(release): prepare vX.Y.Z`
3. Create the annotated tag `vX.Y.Z`
4. If `--push` is used, push the branch first and the tag second

That keeps the tag-triggered GitHub Release workflow pointed at the commit that already contains the matching changelog update.

To publish only one RID:

```bash
./scripts/publish-dist.sh win-x64
./scripts/publish-dist.sh win-x86
```

## Artifact Shape

Each `dist/crosshook-win-*` directory is a cleaned copy of the RID-specific `dotnet publish` output:

- `crosshook.pdb` is removed from the shipped artifact.
- `Profiles/`, `Settings/`, and `settings.ini` are removed from the shipped artifact because they are runtime/user state.
- The runtime host/runtime DLLs remain beside `crosshook.exe`.

The matching `.zip` file is the preferred release artifact to upload, attach to a release, or copy into a test area.

Important: these publishes are self-contained, but they are not single-file publishes. The `crosshook.exe` apphost expects `crosshook.dll`, `crosshook.deps.json`, `crosshook.runtimeconfig.json`, and the bundled runtime DLLs to remain in the same published directory layout.

If you copy only `crosshook.exe` into another directory, startup fails with an error like:

```text
The application to execute does not exist: 'D:\...\crosshook.dll'
```

When testing or packaging a publish, copy the entire `dist/crosshook-win-*` directory as a unit, or use the generated zip. End-user guidance should tell users to download a zip from GitHub Releases, extract it into a directory of their choice, and run `crosshook.exe` from the extracted folder.

The repo-root `crosshook.exe` is a legacy file already checked into the repository. The raw `src/CrossHookEngine.App/bin/Release/net9.0-windows/<rid>/publish/` directories are intermediate publish outputs; `dist/` is the release packaging output.

## Which Artifact To Use

- Use `win-x64` by default for 64-bit game and trainer combinations.
- Use `win-x86` when you need 32-bit compatibility.
- Keep both for release packaging because the current migration policy is dual artifacts.

## What Was Produced Earlier

The Phase 1 validation work produced both publish variants:

- `/tmp/crosshook-publish/win-x64/crosshook.exe`
- `/tmp/crosshook-publish/win-x86/crosshook.exe`

That means the earlier verification did not standardize on only one architecture. Both were built successfully. The later Steam validation did not explicitly identify which one was loaded, so that result should not be treated as proof of only `win-x64` or only `win-x86`.
