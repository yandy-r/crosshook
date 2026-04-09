# PR Review: #185 — feat(collections): preset TOML import/export and dev modals

**Reviewed**: 2026-04-08
**Author**: yandy-r
**Branch**: `feat/profile-collections-phase-4-toml-export-import-preset` → `main`
**Head (on origin)**: `741dd67`
**Local tip**: `163ba12` (**not yet pushed** — contains material fixes, see MEDIUM-1)
**Closes**: #180
**Decision**: **APPROVE with comments** (contingent on pushing the local commit)

## Summary

Phase 4 of Profile Collections adds `*.crosshook-collection.toml` preset
export/import with multi-field profile matching, a review modal, browser-dev
explainer modals, and full Rust unit tests. The split between `crosshook-core`
(schema + exchange pipeline) and the thin Tauri command layer is clean,
error types are explicit and `Serde`-safe, and the frontend follows the
existing portal/focus-trap pattern used by `CollectionViewModal`. There are
no critical or high findings; the items below are tightening suggestions
plus one merge-blocking mechanical issue (un-pushed commit).

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

**M1. Un-pushed local commit carries material fixes — push before merge.**
`163ba12 feat(profiles): preset TOML path handling, import UX, modal tokens`
exists only on `HEAD`; `origin/feat/profile-collections-phase-4-toml-export-import-preset`
is at `741dd67`. The un-pushed commit contains non-cosmetic changes:

- `commands/collections.rs:135–150` — removes the incorrect `.trim()` on
  `output_path` / `path` (paths with legitimate leading/trailing whitespace
  would have been silently mangled) and adds explicit empty-string rejection.
- `collection_exchange.rs:411–425` — centralizes validation through
  `CollectionPresetManifest::validate()` while preserving the
  `UnsupportedSchemaVersion` error variant.
- `BrowserDevPresetExplainerModal.tsx` — adds scroll lock + `inert` on
  background nodes so the background isn't interactive while the modal is
  open. Without this, the first review mode of the modal is partially
  broken in WebKitGTK.
- `CollectionImportReviewModal.tsx` — surfaces `importSessionError` inside
  the modal so failed applies are visible to the user rather than hidden in
  the sidebar's collapsed error slot.

  **Fix**: `git push` before merging, or squash into `741dd67` and
  force-push the branch. Without this commit, the PR ships a half-fixed
  explainer modal and path-trimming bug.

**M2. `applyImportedCollection` rollback does not communicate rollback
failure distinctly.**
[`src/crosshook-native/src/context/CollectionsContext.tsx:286–298`](../../../src/crosshook-native/src/context/CollectionsContext.tsx#L286-L298)
If the sequential `collection_add_profile` loop fails mid-way, the catch
block fires `collection_delete(createdId)` to roll back. But if that
rollback itself fails, the error is only logged to `console.error` and the
user sees the original apply-error message. They are left with a partially
populated "orphan" collection and no way to tell it happened. The current
code:

```ts
} catch (rollbackErr) {
  console.error('collection import rollback failed', rollbackErr);
}
```

**Fix**: Include rollback failure context in the returned error (e.g.
`{ ok: false, error: "${apply error} (rollback also failed: ${rollback error})" }`)
so users know a manual cleanup may be required.

**M3. `BrowserDevPresetExplainerModal` portal pattern diverges from the
other two modals in this PR.**
[`src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx:261`](../../../src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx#L261)
`CollectionViewModal` and `CollectionImportReviewModal` both create a
persistent portal host div (`crosshook-modal-portal`) and add
`body.classList.add('crosshook-modal-open')` while open. The explainer
renders directly with `createPortal(node, document.body)` and does NOT set
the `crosshook-modal-open` body class. If that class drives scrollbar
gutter / pointer-events / background styling in `theme.css`, the
explainer will look subtly different from the other modals.
**Fix**: Mirror the `CollectionImportReviewModal` mount pattern — create
a dedicated host element and toggle `crosshook-modal-open` in the effect.

**M4. `--crosshook-gap` CSS token is too generic for its single
consumer.**
[`src/crosshook-native/src/styles/variables.css:94`](../../../src/crosshook-native/src/styles/variables.css#L94) →
[`BrowserDevPresetExplainerModal.css:16`](../../../src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.css#L16)
The new token lives in the global `:root` scope but is only consumed by
the explainer modal body. A future component that assumes
`--crosshook-gap` is a general-purpose gap will either (a) reuse it and
couple the two, or (b) redefine it and break the explainer.
**Fix**: Rename to `--crosshook-modal-body-gap` (or inline the 12px in
the explainer CSS since it is the only consumer).

### LOW

**L1. rustfmt diffs in new files.** `cargo fmt --check` flags the following
in PR-added Rust code (CI does not currently enforce fmt — the repo has a
pre-existing baseline — but these are trivial to clean up):

- `collection_exchange.rs:12` — import ordering vs `#[cfg(test)]` use.
- `collection_exchange.rs:130` — `build_local_match_index` signature wrap.
- `collection_exchange.rs:153` — collapsible `profile_display.insert(...)`.
- `collection_exchange.rs:526` — chained `.as_ref().and_then(...)` wrap.
- `collection_exchange.rs:628` — collapsible `metadata.add_profile_to_collection(...)`.
- `collection_schema.rs:40` — `format!` args can collapse.
- `profile/mod.rs:1` — `collection_schema` / `collection_exchange` mod
  ordering (alphabetical).
- `profile/mod.rs:12` — `CollectionPresetMatchCandidate` sorts before
  `CollectionPresetMatchedEntry`.

**L2. Clippy `field_reassign_with_default` nit in new unit test.**
[`collection_exchange.rs:513–514`](../../../src/crosshook-native/crates/crosshook-core/src/profile/collection_exchange.rs#L513-L514)

```rust
let mut defaults = CollectionDefaultsSection::default();
defaults.method = Some("proton_run".to_string());
```

Clippy prefers struct-update syntax. Matches pre-existing pattern in
`profile/models.rs` — not blocking.

**L3. Index-based React keys on ambiguous/unmatched lists.**
[`CollectionImportReviewModal.tsx:308, 350`](../../../src/crosshook-native/src/components/collections/CollectionImportReviewModal.tsx#L308-L356)
The matched list now uses a stable composite key (post un-pushed commit),
but ambiguous and unmatched still use `<div key={i}>`/`<li key={i}>`. The
lists are static for a given preview so reconciliation is safe, but for
consistency consider:

```ts
key={`${d.steam_app_id}-${d.trainer_community_trainer_sha256}-${i}`}
```

**L4. Commit scope for un-pushed commit.** `feat(profiles): preset TOML
path handling, import UX, modal tokens` — the commit touches
`collection_*` exclusively (exchange, commands, import review modal,
sidebar pass-through, explainer modal, CSS tokens used only by the
collection explainer). A `feat(collections): ...` prefix would match the
first commit and be more specific for the CHANGELOG. Minor — see CLAUDE.md
Commits / changelog section.

**L5. `CollectionsSidebar` error UI collapses three sources into one
slot.** [`CollectionsSidebar.tsx:174–178`](../../../src/crosshook-native/src/components/collections/CollectionsSidebar.tsx#L174-L178)
`createSessionError ?? importSessionError ?? error` — fine because the
three sources are mutually exclusive in practice (user can only do one
action at a time), but a comment explaining the precedence would help
future maintenance.

**L6. `data-crosshook-modal-close` missing from Cancel buttons.** The
top-right Close button has `data-crosshook-modal-close` but the footer
Cancel button does not (both `CollectionImportReviewModal` and
`BrowserDevPresetExplainerModal`). If that attribute is consumed by global
keyboard / automation tooling, both buttons should have it.

**L7. `BrowserDevPresetExplainerModal` hand-rolls a `**bold**` markdown
parser.** [Lines 47–60](../../../src/crosshook-native/src/components/collections/BrowserDevPresetExplainerModal.tsx#L47-L60).
Safe here because the `COPY` constant is static, but a bare helper like
this tends to attract future callers who _do_ pass user input. Consider
adding a comment that `renderParagraph` MUST only receive static strings,
or inlining the two `<strong>` spans per paragraph.

## Correctness deep-dives

Items I verified in full (not just from diff):

- **Schema version rejection.** `parse_collection_preset_toml` correctly
  emits `UnsupportedSchemaVersion { version, supported }` when
  `schema_version` is present but != "1", and `InvalidManifest` when the
  manifest is well-formed but fails `validate()` (empty name, empty
  schema_version). Unit test `parse_rejects_future_schema_version` covers
  the happy path for this discrimination.
- **Export rollback semantics.** `applyImportedCollection` captures
  `createdId` before the first downstream write — description, defaults,
  then per-member add — so any failure past the initial `collection_create`
  will fire `collection_delete(createdId)`, which relies on the
  `collection_profiles` FK `ON DELETE CASCADE` in the SQLite schema.
  Confirmed via the existing `collection_delete_cascades_memberships`
  test elsewhere in metadata integration.
- **Effective Steam app id resolution.** `descriptor_from_profile` uses
  `resolve_art_app_id(profile)` which prefers `steam.app_id`, falling
  back to `runtime.steam_app_id`, matching existing tests in
  `profile/models.rs`. Verified via
  `export_preview_roundtrip_with_effective_app_id`.
- **Empty defaults filtering.** The `.filter(|d| !d.is_empty())` on
  `metadata_store.get_collection_defaults(...)` ensures an empty
  `[defaults]` table does not appear in exported TOML. Unit-tested via
  the explicit `Some("proton_run")` case in the roundtrip test.
- **Focus trap, inert restoration, scroll lock** in
  `CollectionImportReviewModal` follow the exact structure used by
  `CollectionViewModal` — previous focus restore, Tab cycle, ESC close,
  `body.style.overflow` save/restore, per-node `inert` and `aria-hidden`
  save/restore. Safe.
- **Frontend persistence boundary.** Phase 4 writes go through existing
  Phase 1–3 IPC (create/description/defaults/add_profile) — no new SQLite
  migrations, no new TOML settings keys. Exported TOML files are ephemeral
  disk artifacts chosen by the user, not managed storage. This matches the
  CLAUDE.md Storage boundary classification implicit in the Phase 4 plan.
- **Dialog stub fallback.** `lib/plugin-stubs/dialog.ts` is the single
  source of truth for "Tauri vs browser-dev" picker branching; both new
  call sites (`CollectionsSidebar.handleImportPreset`,
  `CollectionViewModal.handleExportPreset`) correctly short-circuit on
  `isBrowserDevUi()` and open the explainer modal instead.

## Validation Results

| Check            | Result | Notes                                                                                                                                 |
| ---------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| Type check (TS)  | Pass   | `npx tsc --noEmit` → exit 0                                                                                                           |
| Frontend build   | Pass   | `npm run build` (tsc + vite) → clean; only pre-existing dynamic-import warning                                                        |
| Rust build       | Pass   | `cargo build -p crosshook-core` → clean                                                                                               |
| Rust unit tests  | Pass   | `cargo test -p crosshook-core --lib` → 776 passed, 0 failed                                                                           |
| New module tests | Pass   | `profile::collection_exchange` → 7 passed (future-version, malformed, roundtrip, ambiguous, pair fallback, unmatched, missing member) |
| Clippy           | Warn   | 1 `field_reassign_with_default` nit in new unit test (L2); pre-existing baseline                                                      |
| rustfmt          | Warn   | Several diffs in new files (L1); CI does not enforce; pre-existing baseline                                                           |

## Files Reviewed

### Rust (core + Tauri)

| Path                                                       | Change | Verdict |
| ---------------------------------------------------------- | ------ | ------- |
| `crates/crosshook-core/src/profile/collection_schema.rs`   | Added  | Approve |
| `crates/crosshook-core/src/profile/collection_exchange.rs` | Added  | Approve |
| `crates/crosshook-core/src/profile/mod.rs`                 | Edit   | Approve |
| `src-tauri/src/commands/collections.rs`                    | Edit   | Approve |
| `src-tauri/src/lib.rs`                                     | Edit   | Approve |

### Frontend (TypeScript + CSS)

| Path                                                            | Change | Verdict      |
| --------------------------------------------------------------- | ------ | ------------ |
| `src/components/collections/CollectionImportReviewModal.tsx`    | Added  | Approve      |
| `src/components/collections/CollectionImportReviewModal.css`    | Added  | Approve      |
| `src/components/collections/BrowserDevPresetExplainerModal.tsx` | Added  | Approve (M3) |
| `src/components/collections/BrowserDevPresetExplainerModal.css` | Added  | Approve      |
| `src/components/collections/CollectionViewModal.tsx`            | Edit   | Approve      |
| `src/components/collections/CollectionViewModal.css`            | Edit   | Approve      |
| `src/components/collections/CollectionsSidebar.tsx`             | Edit   | Approve      |
| `src/context/CollectionsContext.tsx`                            | Edit   | Approve (M2) |
| `src/hooks/useCollections.ts`                                   | Edit   | Approve      |
| `src/constants/browserDevPresetPaths.ts`                        | Added  | Approve      |
| `src/lib/runtime.ts`                                            | Edit   | Approve      |
| `src/lib/plugin-stubs/dialog.ts`                                | Edit   | Approve      |
| `src/lib/mocks/handlers/collections.ts`                         | Edit   | Approve      |
| `src/lib/mocks/wrapHandler.ts`                                  | Edit   | Approve      |
| `src/types/collections.ts`                                      | Edit   | Approve      |
| `src/styles/variables.css`                                      | Edit   | Approve (M4) |

### Docs

| Path                                                                                                           | Change |
| -------------------------------------------------------------------------------------------------------------- | ------ |
| `docs/prps/plans/profile-collections-phase-4-toml-export-import-preset.plan.md` → `plans/completed/` (renamed) | Move   |
| `docs/prps/reports/profile-collections-phase-4-toml-export-import-preset-report.md`                            | Added  |

## Next steps for the author

1. Push `163ba12` to `origin/feat/profile-collections-phase-4-toml-export-import-preset`
   (M1). Without this, the PR ships without its latest fixes.
2. Address M2 rollback-failure reporting (one-line change in
   `CollectionsContext.tsx`).
3. Optional: M3 portal host alignment, M4 CSS token rename, L1 fmt pass,
   L3 key consistency.
4. Run the native smoke checklist from the plan before merge (real file
   dialogs, real disk round-trip).
