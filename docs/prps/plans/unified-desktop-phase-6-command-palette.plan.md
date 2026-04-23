# Plan: Unified Desktop Phase 6 Command Palette

## Summary

Implement Phase 6 of the Unified Desktop Redesign from GitHub issue #445 / #418: replace the Library-local `console.debug` placeholder with a real hand-rolled command palette that opens from `Cmd/Ctrl+K` anywhere in the shell. The palette stays dependency-free, uses a static command catalog plus substring filtering, reuses the existing modal focus-trap contract, and executes route/profile actions without adding new Tauri commands or persistence.

## User Story

As a CrossHook power user, I want a global command palette for navigation and profile actions, so that I can jump to common destinations and actions without changing routes manually or reaching for the mouse.

## Problem -> Solution

Today the only palette affordance is the optional `LibraryToolbar` button, and `LibraryPage` still handles it with `console.debug('Command palette (Phase 6)')`. Phase 6 moves ownership into `AppShell`: `AppShell` registers the global shortcut, builds the runtime command list from current route/profile state, renders a portaled `CommandPalette`, and passes an `onOpenCommandPalette` callback down to the Library toolbar so both entry points use the same overlay and execution path.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md`
- **PRD Phase**: Phase 6 - ⌘K command palette
- **Estimated Files**: 13
- **GitHub Issues**: #445 tracking, #418 deliverable
- **Persistence Classification**: runtime-only palette state (`open`, `query`, `activeIndex`, filtered command ids); no new TOML settings; no new SQLite tables or migrations.

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | -          | 3              |
| B2    | 2.1, 2.2      | B1         | 2              |
| B3    | 3.1, 3.2, 3.3 | B2         | 3              |

- **Total tasks**: 8
- **Total batches**: 3
- **Max parallel width**: 3

---

## UX Design

### Before

```text
AppShell
├─ route state + sidebar + content
├─ LibraryPage
│  └─ LibraryToolbar button: "⌘K"
│     └─ LibraryPage callback -> console.debug(...)
└─ No global shortcut, no overlay, no command execution
```

### After

```text
AppShell
├─ route state + sidebar + content
├─ global Cmd/Ctrl+K listener
├─ useCommandPalette state
├─ CommandPalette portal
│  ├─ search input
│  ├─ icon + label + optional hint rows
│  ├─ arrow-key selection + Enter execute
│  └─ Esc / close button restores focus
└─ LibraryToolbar button delegates to the same open() path
```

### Interaction Changes

| Touchpoint              | Before                              | After                                                                                               | Notes                                                              |
| ----------------------- | ----------------------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `Cmd/Ctrl+K`            | No-op everywhere                    | Opens palette over the current shell mode                                                           | Implemented in `AppShell`, not per-page                            |
| Library toolbar trigger | Calls page-local placeholder logger | Opens the shared palette                                                                            | Keeps the existing button contract but changes the target behavior |
| Querying commands       | Not available                       | Case-insensitive substring filter over static labels and keywords                                   | No fuzzy scoring or recency                                        |
| Keyboard navigation     | Not available                       | `ArrowDown` / `ArrowUp` move active option; `Enter` executes; `Esc` closes                          | Must coexist with `useFocusTrap` and `useScrollEnhance`            |
| Closing palette         | Not available                       | Escape, backdrop click, or Close button restores focus to the invoking element when still connected | Reuse modal focus-trap behavior                                    |
| Executing commands      | Not available                       | Route commands call `setRoute`; profile actions call `selectProfile(...)` then navigate             | No new backend command surface                                     |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority | File                                                                       | Lines            | Why                                                                                                      |
| -------- | -------------------------------------------------------------------------- | ---------------- | -------------------------------------------------------------------------------------------------------- |
| P0       | `docs/prps/prds/unified-desktop-redesign.prd.md`                           | 253-257          | Phase 6 goal, scope, and success signal define the acceptance contract.                                  |
| P0       | `src/crosshook-native/src/App.tsx`                                         | 15-27, 29-48     | Global app-level hooks already handle scroll enhancement and gamepad-back modal close behavior.          |
| P0       | `src/crosshook-native/src/components/layout/AppShell.tsx`                  | 50-64, 172-240   | Owns route state, shell layout, and the only correct place for the global shortcut and shared overlay.   |
| P0       | `src/crosshook-native/src/components/layout/ContentArea.tsx`               | 16-19, 37-60     | Current route-to-page dispatch contract for threading `onOpenCommandPalette` down to Library.            |
| P0       | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                | 121-170, 236-302 | Existing launch/edit/detail handlers plus the current palette placeholder entry point.                   |
| P0       | `src/crosshook-native/src/hooks/useFocusTrap.ts`                           | 172-307          | Shared modal contract: body lock, inert siblings, Escape ownership, Tab trapping, and focus restoration. |
| P1       | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                       | 8-10, 96-126     | Scrollable selector and keyboard-scroll logic that the palette list must integrate with.                 |
| P1       | `src/crosshook-native/src/components/library/LibraryToolbar.tsx`           | 15-25, 117-125   | Existing optional toolbar trigger contract that should remain additive.                                  |
| P1       | `src/crosshook-native/src/components/layout/routeMetadata.ts`              | 31-139           | Current route labels and route inventory for navigation commands.                                        |
| P1       | `src/crosshook-native/src/lib/validAppRoutes.ts`                           | 1-20             | Runtime route validation pattern for any string-to-route execution path.                                 |
| P1       | `src/crosshook-native/src/main.tsx`                                        | 4-15             | Global stylesheet import contract if a dedicated `palette.css` file is added.                            |
| P1       | `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx` | 54-188           | Existing Library harness and expectations around the toolbar trigger and detail persistence.             |
| P1       | `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`   | 79-188           | Full-shell integration harness for keyboard shortcuts and sidebar/inspector assertions.                  |
| P1       | `src/crosshook-native/tests/smoke.spec.ts`                                 | 51-174           | Browser-dev smoke style and zero-console-error enforcement to extend for the palette flow.               |

## External Documentation

| Topic         | Source | Key Takeaway                                                                                                    |
| ------------- | ------ | --------------------------------------------------------------------------------------------------------------- |
| External docs | none   | No external library/API research is needed; the Phase 6 contract is fully covered by repo patterns and the PRD. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### STATIC_RECORD_PATTERN

```ts
// SOURCE: src/crosshook-native/src/lib/validAppRoutes.ts:3-19
const VALID_APP_ROUTES: Record<AppRoute, true> = {
  library: true,
  profiles: true,
  launch: true,
};
```

Keep the command catalog static and typed with `Record<Union, ...>` or readonly arrays plus narrow helpers. Do not build the v1 palette around free-form strings or ad hoc object literals scattered across components.

### HOOK_CONTRACT_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/useFocusTrap.ts:10-37
export interface UseFocusTrapOptions {
  open: boolean;
  panelRef: RefObject<HTMLElement | null>;
  onClose: () => void;
}
```

`useCommandPalette` should follow the same `UseXOptions` / `UseXReturn` naming style and return a stable, explicit contract instead of leaking raw implementation state across `AppShell` and `CommandPalette`.

### OPTIONAL_CALLBACK_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/library/LibraryToolbar.tsx:15-25
interface LibraryToolbarProps {
  searchQuery: string;
  onSearchChange: (query: string) => void;
  onOpenCommandPalette?: () => void;
}
```

Keep palette wiring additive. Thread `onOpenCommandPalette?: () => void` through `ContentArea` / `LibraryPage` without making the Library route depend on the palette implementation details.

### ROUTE_AND_ACTION_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx:121-135
setLaunchingName(name);
try {
  await selectProfile(name, { collectionId: collectionIdForLoad });
  onNavigate?.('launch');
} finally {
  setLaunchingName(undefined);
}
```

Command execution should reuse the existing action flow: call `selectProfile(...)` first for profile commands, then navigate. Do not introduce duplicate launch/edit state machines inside the palette component.

### FOCUS_TRAP_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/hooks/useFocusTrap.ts:215-223, 258-274
const frame = window.requestAnimationFrame(() => {
  const focusable = panel ? getFocusableElements(panel) : [];
  if (focusable.length > 0) focusElement(focusable[0]);
});
queueMicrotask(() => {
  if (restoreTarget?.isConnected) focusElement(restoreTarget);
});
```

The palette must use the shared trap instead of inventing its own modal stack. Preserve the open-focus / close-restore semantics and keep `data-crosshook-focus-root="modal"` on the dialog surface.

### KEYBOARD_LIST_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/collections/CollectionAssignMenu.tsx:148-161
if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
  event.preventDefault();
  const idx = focusable.indexOf(document.activeElement as HTMLElement);
  next = event.key === 'ArrowDown' ? idx + 1 : idx - 1;
}
```

Mirror the existing arrow-key list navigation pattern for palette results. Whether the active option is focused directly or scrolled into view via refs, wrap at the ends and keep the behavior deterministic.

### SCROLL_SELECTOR_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/useScrollEnhance.ts:8-10
const SCROLLABLE = '.crosshook-route-card-scroll, ... , .crosshook-inspector__body, .crosshook-hero-detail__body';
```

Any new scrollable palette result list must be added to `SCROLLABLE` and styled with `overscroll-behavior: contain`; otherwise WebKitGTK will scroll the parent container and introduce dual-scroll jank.

### TEST_TAB_ORDER_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx:54-78
await user.tab();
expect(screen.getByRole('searchbox', { name: 'Search games' })).toHaveFocus();
await user.tab();
expect(screen.getByRole('button', { name: 'Recent' })).toHaveFocus();
```

Use role-based, sequential keyboard assertions for palette focus order. Avoid brittle class-based selectors in RTL tests.

### SMOKE_ERROR_CAPTURE_PATTERN

```ts
// SOURCE: src/crosshook-native/tests/helpers.ts:7-17
page.on('pageerror', (err) => {
  capture.errors.push(`pageerror: ${err.message}`);
});
```

Palette smoke coverage must remain zero-error. If the new overlay emits `console.error` or unhandled exceptions in browser dev mode, treat that as a regression even when the UI still appears to work.

---

## Files to Change

| File                                                                            | Action | Justification                                                                                                               |
| ------------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/lib/commands.ts`                                      | CREATE | Static, typed command catalog and substring-match helpers for route and profile actions.                                    |
| `src/crosshook-native/src/hooks/useCommandPalette.ts`                           | CREATE | Shared open/query/selection/filter state and keyboard action helpers owned by `AppShell`.                                   |
| `src/crosshook-native/src/components/palette/CommandPalette.tsx`                | CREATE | Portaled focus-trapped overlay surface with search input, result list, icons, and execution affordances.                    |
| `src/crosshook-native/src/components/palette/__tests__/CommandPalette.test.tsx` | CREATE | Unit coverage for filtering, arrow navigation, execute-on-enter, close paths, and empty state rendering.                    |
| `src/crosshook-native/src/styles/palette.css`                                   | CREATE | Dedicated BEM-style overlay, list, row, and state styles to keep palette chrome out of `theme.css`.                         |
| `src/crosshook-native/src/main.tsx`                                             | UPDATE | Import the new stylesheet into the frontend bundle.                                                                         |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                            | UPDATE | Register `.crosshook-palette__list` in `SCROLLABLE` so wheel/keyboard scroll targets the result list.                       |
| `src/crosshook-native/src/components/layout/AppShell.tsx`                       | UPDATE | Own palette state, register the global shortcut, execute commands, and render the shared overlay.                           |
| `src/crosshook-native/src/components/layout/ContentArea.tsx`                    | UPDATE | Thread an optional `onOpenCommandPalette` callback through to `LibraryPage`.                                                |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`                     | UPDATE | Replace the placeholder logger with a page-to-shell callback and keep the toolbar trigger visible after detail transitions. |
| `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`        | UPDATE | Full-shell assertions for `Ctrl/Cmd+K`, route execution, and focus restoration.                                             |
| `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`      | UPDATE | Library route assertions that the toolbar trigger delegates upward and still survives detail/back flows.                    |
| `src/crosshook-native/tests/smoke.spec.ts`                                      | UPDATE | Browser-dev smoke for open/close, keyboard navigation, command execution, and zero-console-error behavior.                  |

## NOT Building

- No new npm dependency such as `cmdk`, `kbar`, `react-router-dom`, or a hotkey package.
- No fuzzy matching, ranking, recency memory, or persisted “recent commands” list.
- No new Tauri command, Rust IPC handler, SQLite migration, or TOML setting.
- No URL routing or route-history integration; palette navigation still mutates `AppRoute` state in `AppShell`.
- No gamepad-nav zone refactor; the palette participates as a modal via the existing `data-crosshook-focus-root="modal"` contract.
- No command authoring UI or user-editable command registry; v1 ships a hand-authored static catalog only.

---

## Step-by-Step Tasks

### Task 1.1: Add a typed static command catalog module — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/lib/commands.ts`.
- **IMPLEMENT**: Define the static v1 command shape, action union, icon ids, keyword aliases, and helpers for case-insensitive substring matching. Keep route commands hand-authored and typed against `AppRoute`; model active-profile actions as separate typed entries so `AppShell` can enable/disable or inject them without stringly logic.
- **MIRROR**: `STATIC_RECORD_PATTERN`, `HOOK_CONTRACT_PATTERN`, `Type Definitions` findings from the discovery tables.
- **IMPORTS**: `AppRoute` from `@/components/layout/Sidebar`; no component imports.
- **GOTCHA**: Do not import `ROUTE_METADATA` into `src/lib`; that would drag component-level dependencies into the catalog. Keep the module UI-agnostic and use icon ids rather than JSX.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck` passes with no implicit-`any` or unused export errors in the new module.

### Task 1.2: Create `useCommandPalette` state and selection helpers — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/hooks/useCommandPalette.ts`.
- **IMPLEMENT**: Expose explicit hook types for open/close state, query text, filtered command list, active index/id, `setQuery`, `moveActive(delta)`, `reset`, and `executeActive`. Reset the query and selection when closing; when the filtered list changes, clamp or reset the active index so Enter never executes a stale command.
- **MIRROR**: `HOOK_CONTRACT_PATTERN`, `KEYBOARD_LIST_PATTERN`.
- **IMPORTS**: React hooks, command types/helpers from `@/lib/commands`.
- **GOTCHA**: The active selection must remain stable across rerenders but must also recover cleanly when the query yields zero results. Avoid storing a bare array index without clamping it against the filtered list.
- **VALIDATE**: Typecheck passes and the hook can be consumed from `AppShell` without additional wrapper state.

### Task 1.3: Add palette styles and scroll registration — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/styles/palette.css`, import it from `src/crosshook-native/src/main.tsx`, and update `src/crosshook-native/src/hooks/useScrollEnhance.ts`.
- **IMPLEMENT**: Add `crosshook-palette*` classes for the shell, header, search input, results list, row states, empty state, and hint chip. Reuse the shared `.crosshook-modal` / `.crosshook-modal__surface` base styles via a palette-specific modifier class instead of duplicating the full modal chrome. Append `.crosshook-palette__list` to `SCROLLABLE` and set `overflow-y: auto; overscroll-behavior: contain;` on the list container.
- **MIRROR**: `SCROLL_SELECTOR_PATTERN`, existing modal surface styling in `theme.css`.
- **IMPORTS**: stylesheet import in `main.tsx`; no runtime code imports.
- **GOTCHA**: The palette list is the scroll target, not the whole modal body. If the list is not whitelisted in `SCROLLABLE`, ArrowUp/ArrowDown will scroll the parent route body instead of the overlay.
- **VALIDATE**: Browser build still includes all stylesheets, and `useScrollEnhance.ts` contains `.crosshook-palette__list` exactly once.

### Task 2.1: Build the `CommandPalette` overlay component — Depends on [1.1, 1.2, 1.3]

- **BATCH**: B2
- **ACTION**: Create `src/crosshook-native/src/components/palette/CommandPalette.tsx`.
- **IMPLEMENT**: Render a portaled modal dialog using `createPortal`, the shared `useFocusTrap`, a close button marked with `data-crosshook-modal-close`, a search field, and a result list with icon, label, and optional hint. Compose the focus-trap `handleKeyDown` with palette-specific ArrowUp/ArrowDown/Enter handlers, and scroll the active row into view when selection changes. Support empty-state rendering when no commands match.
- **MIRROR**: `FOCUS_TRAP_PATTERN`, `KEYBOARD_LIST_PATTERN`, `OPTIONAL_CALLBACK_PATTERN`.
- **IMPORTS**: `createPortal`, `useRef`, `useFocusTrap`, command types from `@/lib/commands`, and existing icon components from `@/components/icons/SidebarIcons`.
- **GOTCHA**: `App.tsx` gamepad-back closes the topmost modal by querying `[data-crosshook-focus-root="modal"] [data-crosshook-modal-close]`. Without the close button marker, controller back will regress even if keyboard Escape still works.
- **VALIDATE**: The component can render open/closed in isolation, and Escape / backdrop / close button all hit the same `onClose` path.

### Task 2.2: Wire `AppShell`, `ContentArea`, and `LibraryPage` to the shared palette — Depends on [1.1, 1.2, 1.3]

- **BATCH**: B2
- **ACTION**: Update `src/crosshook-native/src/components/layout/AppShell.tsx`, `src/crosshook-native/src/components/layout/ContentArea.tsx`, and `src/crosshook-native/src/components/pages/LibraryPage.tsx`.
- **IMPLEMENT**: Make `AppShell` own the command list and palette state, register a document-level `Cmd/Ctrl+K` listener, render `CommandPalette`, and execute commands by calling `setRoute(...)` or `selectProfile(...)` plus navigation. Thread `onOpenCommandPalette?: () => void` through `ContentArea` to `LibraryPage` so the existing toolbar trigger opens the same overlay, replacing the placeholder logger. Keep all route changes inside the existing `AppRoute` contract and do not add new IPC.
- **MIRROR**: `ROUTE_AND_ACTION_PATTERN`, `OPTIONAL_CALLBACK_PATTERN`, `Data Flow` findings from the discovery tables.
- **IMPORTS**: `useCommandPalette`, `CommandPalette`, `type AppRoute`, `selectProfile` from the existing profile context usage already present in `AppShell`.
- **GOTCHA**: Guard the global key listener against duplicate fires (`event.repeat`) and unsupported modifier combinations. The handler should `preventDefault()` for the shortcut, but it must not hijack unrelated keyboard flow once the palette is already open.
- **VALIDATE**: `Ctrl/Cmd+K` opens from any route, the Library toolbar button opens the same palette, and choosing a route command updates the selected sidebar tab.

### Task 3.1: Add palette unit tests and update LibraryPage delegation coverage — Depends on [2.1, 2.2]

- **BATCH**: B3
- **ACTION**: Create `src/crosshook-native/src/components/palette/__tests__/CommandPalette.test.tsx` and update `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`.
- **IMPLEMENT**: Cover render/open state, substring filtering, empty results, ArrowUp/ArrowDown wrap, Enter execution, Escape close, and click execution for `CommandPalette`. In `LibraryPage.test.tsx`, stop asserting `console.debug` and instead assert that clicking the toolbar trigger calls the delegated `onOpenCommandPalette` callback while existing detail/back assertions still pass.
- **MIRROR**: `TEST_TAB_ORDER_PATTERN`, existing `LibraryPage` harness/provider setup.
- **IMPORTS**: `render`, `screen`, `waitFor`, `userEvent`, `vi`, and the existing test helpers already used by page tests.
- **GOTCHA**: Keep tests role-driven. Avoid asserting implementation details such as internal active-index state when the visible selection/focus contract is enough.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/palette/__tests__/CommandPalette.test.tsx src/components/pages/__tests__/LibraryPage.test.tsx`

### Task 3.2: Extend full-shell `AppShell` integration coverage — Depends on [2.1, 2.2]

- **BATCH**: B3
- **ACTION**: Update `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`.
- **IMPLEMENT**: Add integration cases for opening the palette with `Ctrl+K`, executing a route command, and restoring focus to the Library toolbar trigger after closing from a button-opened palette. Keep the existing viewport mocking and provider harness patterns intact.
- **MIRROR**: existing `AppShell` integration setup and the focus expectations used by current keyboard tests.
- **IMPORTS**: existing test harness utilities plus `fireEvent` only if keyboard shortcut dispatch is cleaner than `user.keyboard`.
- **GOTCHA**: Use `Ctrl+K` in tests unless a specific environment requires `Meta`; the implementation should support both, but CI/test environments are Linux-first.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/layout/__tests__/AppShell.test.tsx`

### Task 3.3: Add browser-dev smoke coverage for the palette flow — Depends on [2.1, 2.2]

- **BATCH**: B3
- **ACTION**: Update `src/crosshook-native/tests/smoke.spec.ts`.
- **IMPLEMENT**: Add a smoke case that opens the palette with the keyboard, filters to a known route command such as Settings or Proton Manager, executes it with Enter, and asserts the destination route is active. Add an explicit open/close case from the Library toolbar trigger if needed to protect the button path. Keep zero-console-error enforcement via `attachConsoleCapture`.
- **MIRROR**: `SMOKE_ERROR_CAPTURE_PATTERN`, existing role-based route assertions in `smoke.spec.ts`.
- **IMPORTS**: no new helpers unless a tiny local helper reduces repeated keypress boilerplate.
- **GOTCHA**: Browser-dev mode is mock-backed; do not add assertions that require new backend commands or real filesystem/network behavior.
- **VALIDATE**: `cd src/crosshook-native && npm run test:smoke`

---

## Testing Strategy

### Unit Tests

| Test               | Input                                               | Expected Output                                                         | Edge Case? |
| ------------------ | --------------------------------------------------- | ----------------------------------------------------------------------- | ---------- |
| Command filtering  | query `set` against route + profile commands        | Only commands whose label/keywords contain `set` remain                 | No         |
| Selection wrap     | active index at first/last item + ArrowUp/ArrowDown | Selection wraps to the opposite end                                     | Yes        |
| Enter execution    | active command selected + Enter                     | `onExecute` receives the active command exactly once and palette closes | No         |
| Empty results      | query with no matches                               | Empty-state copy renders; Enter is a no-op                              | Yes        |
| Toolbar delegation | click `Open command palette` in `LibraryPage`       | delegated `onOpenCommandPalette` callback fires                         | No         |
| AppShell shortcut  | `Ctrl+K` on an active shell route                   | Palette dialog opens and focus lands inside it                          | No         |
| Focus restoration  | open from toolbar trigger, then Escape/Close        | Trigger regains focus if still connected                                | Yes        |

### Edge Cases Checklist

- [ ] `Ctrl+K` and `Meta+K` both open the palette
- [ ] Repeated shortcut press while the palette is already open does not duplicate overlays or reset unexpectedly
- [ ] Query with zero matches does not execute anything on Enter
- [ ] Active-profile commands are hidden or disabled when no profile is selected
- [ ] Arrow keys do not scroll the underlying route while the palette list is active
- [ ] Closing from backdrop, Escape, and Close button all restore focus consistently
- [ ] Browser dev mode stays green with no `console.error` / `pageerror`
- [ ] Library detail mode still shows the toolbar trigger after Back

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run typecheck
```

EXPECT: Zero TypeScript errors in the new palette files and updated shell/page/test surfaces.

### Unit Tests

```bash
cd src/crosshook-native && npm test -- src/components/palette/__tests__/CommandPalette.test.tsx src/components/layout/__tests__/AppShell.test.tsx src/components/pages/__tests__/LibraryPage.test.tsx
```

EXPECT: Palette unit tests, shell integration tests, and Library delegation tests all pass.

### Full Test Suite

```bash
./scripts/lint.sh
```

EXPECT: Biome, repo sentinels, and other configured checks pass with no new frontend regressions.

### Browser Validation

```bash
cd src/crosshook-native && npm run test:smoke
```

EXPECT: Browser-dev smoke passes, including the new palette case, with zero captured `console.error` / `pageerror` output.

### Manual Validation

- [ ] Start browser dev mode with `./scripts/dev-native.sh --browser` and verify `Ctrl/Cmd+K` opens from Library, Health, and Settings.
- [ ] From Library, click `Open command palette`, press `Escape`, and confirm focus returns to the toolbar trigger.
- [ ] Type a partial query such as `host` and execute the Host Tools command with Enter.
- [ ] Type a query that yields no matches and confirm the empty state is rendered with no accidental navigation.
- [ ] On a long result list, use ArrowDown until scrolling is required and confirm the list scrolls instead of the background route.

---

## Acceptance Criteria

- [ ] `Cmd/Ctrl+K` opens a focus-trapped command palette from any route
- [ ] Library toolbar trigger opens the same palette instead of logging a placeholder
- [ ] Palette uses a static typed command catalog with substring filtering only
- [ ] ArrowUp/ArrowDown move the active result; Enter executes; Escape closes
- [ ] Focus returns to the invoking element when the palette closes and that element is still connected
- [ ] `.crosshook-palette__list` is registered in `useScrollEnhance`
- [ ] No new dependency, Tauri command, SQLite table, or TOML setting is added
- [ ] Typecheck, targeted tests, lint, and smoke coverage all pass

## Completion Checklist

- [ ] Code follows the discovered static-record and hook-contract patterns
- [ ] Overlay behavior reuses `useFocusTrap` rather than inventing a second modal system
- [ ] Route/profile command execution reuses existing `selectProfile(...)` plus navigation flows
- [ ] Palette styling stays modular in a dedicated stylesheet and uses `crosshook-*` BEM naming
- [ ] Tests remain role-based and readable
- [ ] Browser-dev mode works without new mock handlers
- [ ] Scope stays inside Phase 6; no fuzzy ranking or persistence slips in
- [ ] Implementation is self-contained and ready for `prp-implement --parallel`

## Risks

| Risk                                                                                                    | Likelihood | Impact | Mitigation                                                                                                                                      |
| ------------------------------------------------------------------------------------------------------- | ---------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Global shortcut conflicts with existing keyboard handling or repeats excessively                        | M          | M      | Guard on `event.repeat`, support both `ctrlKey` and `metaKey`, and keep the handler centralized in `AppShell`                                   |
| Palette rows drift from route labels maintained elsewhere                                               | M          | M      | Keep route ids typed against `AppRoute`; either reuse existing labels intentionally or document the duplicated strings clearly in `commands.ts` |
| Focus restoration or controller-back behavior regresses because the overlay misses shared modal markers | M          | H      | Use `useFocusTrap`, keep `data-crosshook-focus-root="modal"`, and render a close button with `data-crosshook-modal-close`                       |
| Arrow keys scroll the background route instead of the results list                                      | H          | M      | Add `.crosshook-palette__list` to `SCROLLABLE`, mark the result container as interactive, and cover keyboard navigation in RTL + smoke          |
| Profile actions behave inconsistently when no profile is selected                                       | M          | M      | Model active-profile commands explicitly and hide/disable them when selection state is missing                                                  |

## Notes

- External research was intentionally skipped; the PRD and existing overlay/navigation patterns are sufficient for a self-contained Phase 6 plan.
- Persistence remains runtime-only for Phase 6. If future work adds recent-command memory or palette preferences, that should be planned as a separate issue with an explicit storage-boundary section.
