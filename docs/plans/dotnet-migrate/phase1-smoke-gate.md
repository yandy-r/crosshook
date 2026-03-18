# Phase 1 Smoke Gate

## Scope

This report captures the Phase 1 WINE/Proton validation attempt for issue `#2` of `dotnet-migrate`.

## Verified

- `dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Release -r win-x64 --self-contained true -o /tmp/choochoo-publish/win-x64`
  Result: passed
- `dotnet publish src/ChooChooEngine.App/ChooChooEngine.App.csproj -c Release -r win-x86 --self-contained true -o /tmp/choochoo-publish/win-x86`
  Result: passed
- Both publish directories contain the expected runtime payloads, including `choochoo.exe`, `choochoo.dll`, and framework assemblies such as `System.Runtime.dll`.

## Runtime Findings

### Attempt 1: clean-prefix WINE launch from `Z:`

Command shape:

```bash
DISPLAY=:1 \
WINEPREFIX=/tmp/choochoo-smoke-prefix \
XDG_CACHE_HOME=/tmp/choochoo-xdg-cache \
WINEDEBUG=-all \
wine /tmp/choochoo-publish/win-x64/choochoo.exe
```

Observed result:

- WINE created windows, but X11 enumeration showed `Wine` and `Wine Mono Installer`, not the ChooChoo UI.

### Attempt 2: disable `mscoree/mshtml`

Command shape:

```bash
DISPLAY=:1 \
WINEPREFIX=/tmp/choochoo-smoke-prefix-override \
XDG_CACHE_HOME=/tmp/choochoo-xdg-cache \
WINEDLLOVERRIDES="mscoree,mshtml=" \
WINEDEBUG=-all \
wine /tmp/choochoo-publish/win-x64/choochoo.exe
```

Observed result:

- X11 enumeration showed `Wine Debugger` and `Program Error`.
- WINE reported:
  `System.IO.FileNotFoundException: Could not load file or assembly 'Z:\tmp\choochoo-publish\win-x64\System.Runtime.dll'. Module not found.`

### Attempt 3: copy the publish output into `C:\choochoo`

Command shape:

```bash
DISPLAY=:1 \
WINEPREFIX=/tmp/choochoo-smoke-prefix-cdrive \
XDG_CACHE_HOME=/tmp/choochoo-xdg-cache-cdrive \
WINEDLLOVERRIDES="mscoree,mshtml=" \
WINEDEBUG=-all \
wine C:\\choochoo\\choochoo.exe
```

Observed result:

- X11 enumeration again showed `Wine Debugger` and `Program Error`.
- WINE reported:
  `System.IO.FileNotFoundException: Could not load file or assembly 'C:\choochoo\System.Runtime.dll'. Module not found.`

## Gate Status

Phase 1 is blocked in this environment.

The publish step is proven for both `win-x64` and `win-x86`, but the WINE/Proton smoke gate is not proven because the modern published app does not reach an observable ChooChoo UI in the available runtime setup.

## Unproven Checklist Items

- main WinForms UI renders
- profile list loads
- settings load/save path still resolves correctly
- process list refresh works
- single-instance `Mutex` behavior still works

## Recommended Follow-Up

- Re-run the x64 smoke gate in a known-good Proton/WINE environment that is closer to the target deployment setup.
- Compare the failing self-contained publish against a framework-dependent publish under the same prefix to isolate whether the blocker is packaging-specific or a broader .NET 9 desktop-runtime issue under this WINE build.
- If the failure reproduces outside this workspace, split a focused follow-up issue before Phase 2 begins.
