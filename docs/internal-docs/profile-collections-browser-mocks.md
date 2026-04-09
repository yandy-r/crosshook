# Profile Collections — Browser Dev Mode Mocks

CrossHook ships a browser-only dev mode (`--browser` flag) that replaces Tauri IPC with an in-memory mock layer. This document covers the mock architecture as it relates to collection features, the CI safeguards that prevent mock code from leaking into production, and known testing gotchas.

Mock handler files for collections:

- `src/crosshook-native/src/lib/mocks/handlers/collections.ts` — collection CRUD, defaults, import/export mocks.
- `src/crosshook-native/src/lib/mocks/handlers/profile.ts` — extended `profile_load` mock that applies collection defaults via `applyMockCollectionDefaults` (mirrors Rust `effective_profile_with` semantics).

## Command classification

`src/crosshook-native/src/lib/mocks/wrapHandler.ts` wraps every registered handler with orthogonal debug middleware (`?delay=<ms>` latency, `?errors=true` forced failures). The `?errors=true` toggle only rejects **mutating** commands; reads always succeed so the app shell can render.

Read classification uses two layers:

1. **`EXPLICIT_READ_COMMANDS`** — a `ReadonlySet<string>` that explicitly lists known read commands (e.g., `profile_load`, `collection_get_defaults`, `collection_import_from_toml`). Collection commands that do not match the verb/noun regex heuristic are listed here explicitly.
2. **`READ_VERB_RE` / `READ_NOUN_RE`** — regex heuristics that catch commands starting with common read prefixes (`get_`, `list_`, `load_`, `check_`, `preview_`, `validate_`, `build_`, `verify_`) or ending with common read suffixes (`_load`, `_list`, `_get`, `_status`). This is a deliberate allow-list bias: false positives only weaken `?errors=true`, while false negatives would crash shell reads.

The function `isReadCommand(name)` returns `true` if the command is in the explicit set or matches either regex.

## Mock registry

All handlers register into a single `Map<string, Handler>` via `registerMocks()` in `src/crosshook-native/src/lib/mocks/index.ts`. The flow is:

1. Each domain module (`registerSettings`, `registerProfile`, `registerCollections`, etc.) populates the shared map.
2. `wrapAllHandlers(map)` applies the debug middleware to every entry.
3. The wrapped map is returned and cached by `ipc.dev.ts`.

At runtime, unhandled commands throw with a `[dev-mock]` prefix:

```
[dev-mock] Unhandled command: <name>. Add a handler in src/lib/mocks/handlers/<area>.ts
```

The `console.debug('[mock] callCommand', name, args)` log line fires for every mock invocation in dev mode.

Sentinel strings checked at build time: `[dev-mock]`, `getMockRegistry`, `registerMocks`, `MOCK MODE`.

## CI sentinel

The `verify:no-mocks` step in `.github/workflows/release.yml:105-120` scans the production JS bundle for mock-mode sentinel strings after the AppImage build:

```yaml
- name: Verify no mock code in production bundle
  run: |
    if grep -rl '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' \
        src/crosshook-native/dist/assets/*.js 2>/dev/null; then
      echo "::error::Mock code found in production bundle — refusing to ship"
      exit 1
    fi
```

If any sentinel is found, the release workflow fails with a `CRITICAL security failure` error. This guards against the `__WEB_DEV_MODE__` Vite define or the dynamic-import dead-branch failing to eliminate mock code from the production bundle.

## Mock coverage drift

`scripts/check-mock-coverage.sh` is a contributor convenience tool that diffs `#[tauri::command]` handlers in Rust source against the mock handler registry. It reports commands that exist in Rust but have no mock handler, and vice versa.

The script always exits `0` — it is advisory, not a CI gate. Run it with:

```bash
./scripts/check-mock-coverage.sh
# or, from src/crosshook-native/:
npm run dev:browser:check
```

## BrowserDevPresetExplainerModal

File-picker-dependent flows (import and export of collection presets) cannot touch disk in browser dev mode because `chooseFile` and `chooseSaveFile` rely on Tauri's native dialog APIs. The `BrowserDevPresetExplainerModal` component (at `src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx`) is the established pattern for handling this:

1. When the user triggers import or export in browser mode, the explainer modal opens instead of the file picker.
2. The modal explains what the desktop app does (file picker, save dialog) and offers a **Continue** button.
3. **Continue** runs the same preview/review step against mock data so contributors can iterate on the UI without a Tauri runtime.

This pattern should be followed for any future flow that depends on native file dialogs.

## MockStore singleton gotcha

The `MockStore` interface and its module-scoped singleton are defined in `src/crosshook-native/src/lib/mocks/store.ts`. The singleton is created lazily on first `getStore()` call and persists for the lifetime of the JS module.

**Key testing implication**: `MockStore` does not reset between tests. Because Vite's module cache keeps the singleton alive across test cases within a single `test.describe` block, state mutations from one test leak into subsequent tests. To avoid flaky assertions:

- Use a full page reload between test cases that depend on a clean store.
- Alternatively, structure tests as separate `test.describe` blocks that each get a fresh page via Playwright's `page` fixture.

The `resetStore()` function exists but only works when called explicitly; it is not wired to any automatic test lifecycle hook.
