# Responsive runtime console

CrossHook now switches the bottom console chrome between a **drawer** and a
compact **status bar** based on the active shell size.

## When the status bar appears

- On **deck** and **narrow** breakpoints.
- On **short desktop windows** where the shell height is at or below
  **720px**, even if the width is still desktop-sized.

## User flow

1. Resize the window to a narrow or short shell.
2. The full runtime console drawer is replaced by a **32px status bar**.
3. The bar shows:
   - current log line count
   - host-readiness summary chips
   - a `⌘K commands` hint
4. Resize back to a larger desktop shell.
5. The full drawer surface returns and can be expanded manually.

## Drawer behavior on larger shells

- The drawer remains available on desktop and ultrawide layouts.
- Incoming log lines update the count, but **do not auto-open** the drawer.
- The existing “start expanded” preference still applies only to the drawer
  mode on larger shells.

## Implementation files

| Area                        | Path                                                                                                                                                                                                                                                                 |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shell breakpoint gate       | `src/crosshook-native/src/components/layout/AppShell.tsx`                                                                                                                                                                                                            |
| Drawer/status UI            | `src/crosshook-native/src/components/layout/ConsoleDrawer.tsx`                                                                                                                                                                                                       |
| Drawer/status styles        | `src/crosshook-native/src/styles/console-drawer.css`                                                                                                                                                                                                                 |
| Compact shell layout        | `src/crosshook-native/src/styles/layout.css`                                                                                                                                                                                                                         |
| User-facing copy updates    | `src/crosshook-native/src/components/settings/LoggingAndUiSection.tsx`, `src/crosshook-native/src/hooks/useRunExecutable.ts`, `src/crosshook-native/src/components/RunExecutablePanel.tsx`                                                                           |
| Breakpoint fixtures + tests | `src/crosshook-native/src/test/breakpoint.ts`, `src/crosshook-native/src/test/__tests__/breakpoint.test.ts`, `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`, `src/crosshook-native/src/components/layout/__tests__/ConsoleDrawer.test.tsx` |

## Limitations

- The browser-dev Playwright smoke path still hits an existing `LibraryPage`
  render-loop warning; targeted Vitest coverage covers the shell mode logic.
