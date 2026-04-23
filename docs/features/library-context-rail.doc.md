# Library context rail (ultrawide)

On very wide displays, the Library gains a **fourth pane** on the right: the **context rail**. It surfaces host-tool readiness, pinned profiles, a short launch-activity chart, and recent successful sessions for the focused profile.

## When it appears

- Only on the **Library** route, in **grid** (not game detail) mode.
- Only when the viewport width is at or above the rail gate (**3400px**). This is separate from the generic `uw` breakpoint so common ultrawide widths (for example 2560×1440) stay a three-column layout.

## User flow

1. Open **Library** on a display wide enough to meet the gate.
2. The rail shows to the right of the inspector; the main grid and inspector behave as before.
3. Open **game detail** (hero/detail mode): the rail hides until you go **Back** to the grid.
4. Navigate away from Library: the rail stays hidden until you return to Library grid.

## Implementation files

| Area                       | Path                                                                                |
| -------------------------- | ----------------------------------------------------------------------------------- |
| Width gate + layout helper | `src/crosshook-native/src/components/layout/contextRailVariants.ts`                 |
| Rail UI                    | `src/crosshook-native/src/components/layout/ContextRail.tsx`                        |
| Shell layout               | `src/crosshook-native/src/components/layout/AppShell.tsx`                           |
| Library vs detail mode     | `src/crosshook-native/src/context/InspectorSelectionContext.tsx`, `LibraryPage.tsx` |
| Scroll polish              | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                                |
| Styles                     | `src/crosshook-native/src/styles/theme.css`                                         |

## Limitations

- Requires host-tool / IPC data where noted in the rail; empty states are handled in-component.
- Playwright smoke may still flag unrelated Library console noise; unit tests cover visibility rules.
