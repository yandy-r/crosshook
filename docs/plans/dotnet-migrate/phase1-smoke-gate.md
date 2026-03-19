# Phase 1 Smoke Gate

## Scope

This report captures the Phase 1 WINE/Proton validation attempt for issue `#2` of `dotnet-migrate`.

## Verified

- `dotnet publish src/CrossHookEngine.App/CrossHookEngine.App.csproj -c Release -r win-x64 --self-contained true -o /tmp/crosshook-publish/win-x64`
  Result: passed
- `dotnet publish src/CrossHookEngine.App/CrossHookEngine.App.csproj -c Release -r win-x86 --self-contained true -o /tmp/crosshook-publish/win-x86`
  Result: passed
- Both publish directories contain the expected runtime payloads, including `crosshook.exe`, `crosshook.dll`, and framework assemblies such as `System.Runtime.dll`.

## External Validation

User-provided target-environment validation for March 18, 2026:

- The executable was loaded into Steam and launched against The Witcher 3 with the trainer.
- The user reported that everything worked with no problems and behavior matched the pre-migration build.
- This validation is recorded under the assumption that the executable launched through Steam was the migrated `crosshook.exe`.

## Runtime Findings

### Attempt 1: clean-prefix WINE launch from `Z:`

Command shape:

```bash
DISPLAY=:1 \
WINEPREFIX=/tmp/crosshook-smoke-prefix \
XDG_CACHE_HOME=/tmp/crosshook-xdg-cache \
WINEDEBUG=-all \
wine /tmp/crosshook-publish/win-x64/crosshook.exe
```

Observed result:

- WINE created windows, but X11 enumeration showed `Wine` and `Wine Mono Installer`, not the CrossHook UI.

### Attempt 2: disable `mscoree/mshtml`

Command shape:

```bash
DISPLAY=:1 \
WINEPREFIX=/tmp/crosshook-smoke-prefix-override \
XDG_CACHE_HOME=/tmp/crosshook-xdg-cache \
WINEDLLOVERRIDES="mscoree,mshtml=" \
WINEDEBUG=-all \
wine /tmp/crosshook-publish/win-x64/crosshook.exe
```

Observed result:

- X11 enumeration showed `Wine Debugger` and `Program Error`.
- WINE reported:
  `System.IO.FileNotFoundException: Could not load file or assembly 'Z:\tmp\crosshook-publish\win-x64\System.Runtime.dll'. Module not found.`

### Attempt 3: copy the publish output into `C:\crosshook`

Command shape:

```bash
DISPLAY=:1 \
WINEPREFIX=/tmp/crosshook-smoke-prefix-cdrive \
XDG_CACHE_HOME=/tmp/crosshook-xdg-cache-cdrive \
WINEDLLOVERRIDES="mscoree,mshtml=" \
WINEDEBUG=-all \
wine C:\\crosshook\\crosshook.exe
```

Observed result:

- X11 enumeration again showed `Wine Debugger` and `Program Error`.
- WINE reported:
  `System.IO.FileNotFoundException: Could not load file or assembly 'C:\crosshook\System.Runtime.dll'. Module not found.`

## Gate Status

Phase 1 is satisfied for issue `#2`.

The local WINE-only validation remains blocked in this workspace, but the release gate is now covered by a real target-environment Steam validation in addition to successful `win-x64` and `win-x86` publish results.

## Unproven Checklist Items

- local standalone WINE startup in this workspace still does not reach an observable CrossHook UI
- the exact local root cause for the `Wine Mono Installer` and `System.Runtime.dll` failures is still unresolved

## Recommended Follow-Up

- Re-run the x64 smoke gate in a known-good Proton/WINE environment that is closer to the target deployment setup.
- Compare the failing self-contained publish against a framework-dependent publish under the same prefix to isolate whether the blocker is packaging-specific or a broader .NET 9 desktop-runtime issue under this WINE build.
- If the failure reproduces outside this workspace, split a focused follow-up issue as a local-environment/runtime investigation rather than treating Phase 1 as blocked.
