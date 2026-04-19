---
pr: 355
url: https://github.com/yandy-r/crosshook/pull/355
head: e5e01b69985d6b88618348e2b44274a339fc6dd4
base: main
title: 'test: Evaluate ts-rs export of Rust arg/return shapes to TypeScript'
author: app/openai-code-agent
reviewed_at: 2026-04-19
decision: REQUEST CHANGES
---

# PR #355 review — ts-rs shape-contract evaluation (Phase 5)

## Scope

Phase 5 spike: evaluate `ts-rs` for generating TS types from Rust arg/return DTOs. Adds an opt-in `ts-rs` Cargo feature on `crosshook-core`, `#[derive(TS)]` annotations on 11 onboarding/health structs, a `ts_rs_export` binary, generated files in `src/types/generated/`, a one-file migration of `src/types/onboarding.ts` to re-export generated types, an evaluation spec, a `copilot-setup-steps.yml` workflow, CLAUDE.md/AGENTS.md firewalled-agent guidance, plus two UI fixes (`useFocusTrap` Escape-at-document + mock handler `docs_url: ''`). 20 files, +498 / −103.

## Validation

| Check                          | Command                                                                        | Result                                    |
| ------------------------------ | ------------------------------------------------------------------------------ | ----------------------------------------- |
| Rust fmt                       | `cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check`   | pass                                      |
| Clippy (default)               | `cargo clippy -p crosshook-core --all-targets -- -D warnings`                  | pass                                      |
| Clippy (`--features ts-rs`)    | `cargo clippy -p crosshook-core --features ts-rs --all-targets -- -D warnings` | **FAIL** (see F002)                       |
| Rust tests                     | `cargo test -p crosshook-core`                                                 | pass                                      |
| TypeScript typecheck           | `npm run typecheck`                                                            | pass                                      |
| Biome                          | `npx @biomejs/biome ci src/`                                                   | pass (2 pre-existing warnings, unrelated) |
| Vitest                         | `npm run test:coverage`                                                        | pass (36/36)                              |
| Host-gateway                   | `./scripts/check-host-gateway.sh`                                              | pass                                      |
| Exporter output matches commit | `cargo run --features ts-rs --bin ts_rs_export` + `git diff`                   | **diverges** (see F003)                   |

## Findings

### F001 — `useFocusTrap` effect thrash + broken focus restoration (Medium, correctness) — Open

**File:** `src/crosshook-native/src/hooks/useFocusTrap.ts:271`

Adding `onClose` to the effect deps fixes the stale-closure risk for the new document-level `handleDocumentKeyDown`, but at least one consumer passes an inline arrow. `src/App.tsx:274` renders `<CollectionEditModal onClose={() => setEditingCollectionId(null)} …>`, so `onClose` identity changes on every `App` re-render. `CollectionEditModal` wraps it in `useCallback(..., [busy, onClose])`, so `guardedOnClose` flips identity in lockstep, and the `useFocusTrap` effect tears down and re-runs on every `App` render while the modal is open.

Concrete consequences per re-run:

- `registerModalPanel`/`unregisterModalPanel` stack churn (benign but noisy).
- `unregisterInertElement` + `registerInertElement` for every body sibling (writes `inert` / `aria-hidden` back and forth).
- `modalBodyLockDepth` decrements to 0 then re-increments to 1 — `body.style.overflow` gets briefly restored then re-hidden inside the same commit.
- `previouslyFocusedRef.current = document.activeElement` overwrites the saved target. After the first run focus lives inside the modal, so subsequent runs save an element inside the soon-to-unmount modal. When the modal finally closes, focus restoration targets a stale element (`.isConnected` check partially saves it, but the original invoker is lost).
- `document.addEventListener('keydown', handleDocumentKeyDown, true)` + `removeEventListener` pair churns on every render.

**Fix:** park `onClose` in a ref and read `onCloseRef.current()` inside the document handler, then drop `onClose` from deps. Pattern:

```ts
const onCloseRef = useRef(onClose);
useEffect(() => {
  onCloseRef.current = onClose;
}, [onClose]);
// inside handleDocumentKeyDown: onCloseRef.current();
// useEffect deps: [open, panelRef, initialFocusRef]
```

This preserves the document-level Escape fix (which is the stated goal of the commit) without making the trap's setup effect depend on the caller's callback identity.

### F002 — ts-rs feature fails clippy `-D warnings` (Medium, build) — Open

**File:** `src/crosshook-native/crates/crosshook-core/src/ts_rs_exports.rs:51-58`

`cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --features ts-rs --all-targets -- -D warnings` fails with:

```
error: cannot test inner items
  --> crates/crosshook-core/src/ts_rs_exports.rs:51:14
   | #[derive(TS)]
   = note: `-D unnameable-test-items` implied by `-D warnings`

error: fields `id`, `generated_at`, `payload`, and `note` are never read
  --> crates/crosshook-core/src/ts_rs_exports.rs:54:9
   = note: `-D dead-code` implied by `-D warnings`
```

`.github/workflows/lint.yml` runs clippy with default features only, so this is invisible to CI. Anyone running the full matrix (or a future CI step that enables the feature — the spec itself suggests one as a next step) will see it fail.

**Fix:** hoist `TsRsEdgeCases` out of the function body to module scope with `#[allow(dead_code)]` (or make the struct `pub` and document it as part of the evaluation sample). Example:

```rust
#[cfg(feature = "ts-rs")]
#[allow(dead_code)]
#[derive(TS)]
#[ts(export, export_to = "generated/ts_rs_edge_cases.ts")]
struct TsRsEdgeCases {
    id: uuid::Uuid,
    generated_at: chrono::DateTime<chrono::Utc>,
    payload: Vec<u8>,
    note: Option<String>,
}
```

…and call `TsRsEdgeCases::export()` directly from `export_ts_types()`.

### F003 — Exporter output diverges from checked-in generated files (Low, maintainability) — Open

Running `cargo run -p crosshook-core --features ts-rs --bin ts_rs_export` against this branch rewrites `src/crosshook-native/src/types/generated/{onboarding,health,ts_rs_edge_cases}.ts` to compact single-line, double-quoted ts-rs output. The checked-in files are multi-line single-quoted (Prettier/Biome post-pass). There is no script or CI sentinel that formats + diffs them, so:

- Regenerating on a fresh checkout produces a dirty tree.
- A maintainer touching a Rust struct without remembering to re-format will leave `src/types/onboarding.ts` and the IPC boundary silently mismatched.

The evaluation spec already names this as suggested next step #2. For a spike PR this is fine to defer, but it deserves a tracked follow-up issue before any wider migration.

**Suggested fix:** add a `scripts/regenerate-ts-types.sh` that runs the exporter + `npx @biomejs/biome format --write src/types/generated/`, and a future CI job (feature-gated) that runs it and asserts a clean diff.

### F004 — `event.stopImmediatePropagation()` on capture-phase document handler (Low, maintainability) — Open

**File:** `src/crosshook-native/src/hooks/useFocusTrap.ts:230`

The new document-level Escape handler registers on `keydown` with `capture: true` and calls `event.stopImmediatePropagation()`. For the topmost-modal case this is the correct behavior, but it will also silently preempt any other capture-phase Escape listener registered by the app (console drawer, overlay panels, search popovers). The `isTopmostModalPanel` guard scopes it to when a modal is active, so this is low risk today, but a short inline comment explaining the intent ("swallow Escape so it never bubbles to non-modal overlays when a modal is open") would help the next reader avoid footguns.

### F005 — Cosmetic doc parity (Info) — Open

`AGENTS.md:54-56` and `CLAUDE.md:32-34` add the same three firewalled-agent bullets (firewalled env, `gh` quoting, GitHub API fallback). Content is good and consistent between the two files. Unrelated pre-existing typo on `CLAUDE.md:15` ("ceratin" → "certain") is not introduced by this PR; flag only as a future cleanup.

## Pattern compliance

- **Feature gating:** `ts-rs` is `optional = true` in `Cargo.toml`, the module and all derives are `#[cfg(feature = "ts-rs")]` / `#[cfg_attr(...)]`. Default builds do not pull `ts-rs`, `chrono-impl`, or `uuid-impl` — good.
- **serde parity:** `CapabilityState`, `HealthStatus`, `HealthIssueSeverity` keep their `rename_all` casing through ts-rs. `HostDistroFamily` correctly exports `PascalCase`. Good.
- **Generated-file marker:** files carry the `// This file was generated by [ts-rs]…` banner. Good.
- **Re-export shape:** `src/types/onboarding.ts` now re-exports generated types with `export type { … }` — TS-only, no runtime cost. Good.
- **Business logic untouched:** the 7 mock-handler and 1 capability/health changes are purely type-compatibility. The `resolveCapabilityToolCheck` fallback `docs_url: undefined → ''` matches the generated `docs_url: string` field.
- **Host-gateway:** no new host-tool calls; `check-host-gateway.sh` passes.

## Security / performance

- No new secrets, network calls, or privileged ops.
- Performance: F001 is effectively a perf foot-gun on parent re-renders for consumers that don't stably memoize `onClose`.

## Recommendation

**REQUEST CHANGES.** The ts-rs scaffolding is clean and the evaluation artifact is thoughtful — that part is mergeable as-is. The two blockers are the non-ts-rs UI hook regression (F001) and the clippy failure under the new feature (F002). F003 is acceptable to defer but deserves a tracked follow-up before any broader migration.
