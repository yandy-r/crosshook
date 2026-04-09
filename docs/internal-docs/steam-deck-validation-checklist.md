# Steam Deck / Gamescope Validation Checklist

This checklist covers input accessibility and scroll behavior validation for CrossHook's Profile Collections features on Steam Deck (Game Mode) and gamescope sessions. All checks must pass before the Phase 5 polish milestone can be considered complete.

## Environment setup

- **Hardware**: Steam Deck running in Game Mode (the default gamescope-embedded session).
- **Desktop alternative**: A standard Linux desktop with gamescope launched manually, e.g.:

```bash
gamescope -W 1280 -H 800 -r 60 -- ./CrossHook_amd64.AppImage
```

- **Input**: All checks must be completable using only the Steam Deck's built-in controls (D-pad, A/B/X/Y buttons, triggers). No mouse, touchpad, or external keyboard should be required for the core flow (check 7).

## Validation checklist

| #  | Check                                                                          | Pass/Fail | Notes |
| -- | ------------------------------------------------------------------------------ | --------- | ----- |
| 1  | Sidebar Collections section reachable via D-pad up/down                        |           |       |
| 2  | A button opens CollectionViewModal; B button closes it                         |           |       |
| 3  | Right-click/Shift+F10/ContextMenu key reaches CollectionAssignMenu on library card |       |       |
| 4  | ArrowUp/Down inside assign menu walks checkboxes                               |           |       |
| 5  | Space on focused checkbox toggles membership                                   |           |       |
| 6  | Escape/B closes assign menu and restores focus to card                         |           |       |
| 7  | Full JTBD flow requires no mouse or touchpad                                   |           |       |
| 8  | D-pad inside CollectionViewModal walks library cards without scroll-jank        |           |       |
| 9  | Right panel scroll in CollectionLaunchDefaultsEditor works via D-pad           |           |       |
| 10 | Collection import review modal reachable and navigable via D-pad               |           |       |

## Gotchas

WebKitGTK (Tauri's webview) has sluggish native scroll velocity. CrossHook compensates with the `useScrollEnhance` hook at `src/crosshook-native/src/hooks/useScrollEnhance.ts`. The `SCROLLABLE` selector constant at line 8 determines which containers receive the enhanced scroll behavior:

```
.crosshook-route-card-scroll, .crosshook-page-scroll-body,
.crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body,
.crosshook-modal__body, .crosshook-prefix-deps__log-output,
.crosshook-discovery-results, .crosshook-collections-sidebar__list,
.crosshook-collection-assign-menu__list
```

Any new `overflow-y: auto` container introduced by collection modals **must** be added to this selector. If omitted, the enhanced scroll targets a parent container instead, causing dual-scroll jank. Inner scroll containers should also apply `overscroll-behavior: contain` to prevent scroll chaining.

## Expected env var assertion

When launching a game with collection defaults active (e.g., a collection that sets `DXVK_HUD = "fps"`), verify that the environment variable propagates to the launched process:

```bash
printenv | grep DXVK_HUD
# Expected output: DXVK_HUD=fps
```

This validates that the Phase 3 merge layer correctly applies collection-level `custom_env_vars` through to the launch command.

## Pass/fail criteria

All 10 checks in the validation checklist must pass. Any failure blocks Phase 5 completion. Failures should be filed as issues with the `platform:steam-deck` and `area:ui` labels, referencing the specific check number and observed behavior.
