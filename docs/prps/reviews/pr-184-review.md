# PR Review: #184 — feat(ui): per-collection launch defaults — Phase 3 (#179)

**Reviewed**: 2026-04-08
**Author**: yandy-r
**Branch**: feat/profile-collections-phase-3-launch-defaults → main
**Head OID**: 0572c3ca3b8963666acd4a08589ff75e9917146c
**Changed**: 21 files (+1625 / −21)
**Decision**: REQUEST CHANGES

## Summary

Phase 3 delivers a well-architected merge layer for per-collection launch defaults: the new `CollectionDefaultsSection` serde type, the `effective_profile_with` refactor with 10 green tests, the additive schema v20 migration (with a defensive column-existence check and round-trip test), corrupt-JSON fallback to `Corrupt` error, and browser-mode mocks that mirror Rust semantics. Code quality, test coverage, and documentation are high. However the **frontend plumbing is incomplete**: multiple navigation flows leading into LaunchPage call `selectProfile(name)` without threading `activeCollectionId`, so a user launching a profile from inside a collection context ends up running the raw storage profile — the collection's env vars and method override never apply. The backend does the right thing; the feature is simply not wired into every launch entrypoint.

## Findings

### CRITICAL

_None._

### HIGH

**H1. Collection defaults bypassed on the "Launch from collection modal" flow** — `src/crosshook-native/src/App.tsx:105-111`

`handleLaunchFromCollection` is what fires when a user clicks the **Launch** button on a `LibraryCard` rendered inside `CollectionViewModal`. It currently calls `selectProfile(name)` without a `collectionId`, then navigates to the launch route:

```tsx
const handleLaunchFromCollection = useCallback(
  async (name: string) => {
    await selectProfile(name); // ← no collectionId
    setRoute('launch');
  },
  [selectProfile]
);
```

Trace of the user flow:

1. User opens a collection modal (`activeCollectionId` is now set via `handleOpenCollection`).
2. User clicks **Launch** on a library card inside the modal.
3. `gameDetailsLaunchThenNavigate` closes the modal, then calls `onLaunch(name)` → `handleLaunchFromCollection`.
4. Profile is loaded via `profile_load(name, /* collectionId */ undefined)` → Rust hits the `_ => Ok(profile)` branch in `src-tauri/src/commands/profile.rs:256` → **no merge layer runs**.
5. User lands on `LaunchPage` with `activeCollectionId` still set.
6. `LaunchPage`'s auto-select effect (`LaunchPage.tsx:47-73`) only re-selects when `sel` is **not** in `filteredProfiles`. In this flow the profile IS in `filteredProfiles` (that's why it was shown in the collection modal). The effect is a no-op.
7. User presses the actual **Launch** button. `LaunchStateProvider` (`context/LaunchStateContext.tsx:22-27`) builds the launch request from `profileState.profile`, which is the un-merged storage profile.
8. The game launches without the collection's `custom_env_vars`, `method`, `network_isolation`, etc.

The PR's own editor-safety invariant ("only the LaunchPage profile-selector path passes a collectionId") unintentionally enforces this bug: the collection-modal launch jump is a second launch entrypoint that wasn't updated. The feature silently does nothing for what is likely the most natural user flow (browse collection → click Launch).

Fix:

```tsx
const handleLaunchFromCollection = useCallback(
  async (name: string) => {
    await selectProfile(name, { collectionId: activeCollectionId ?? undefined });
    setRoute('launch');
  },
  [selectProfile, activeCollectionId]
);
```

Note `handleEditFromCollection` in the same file MUST stay as `selectProfile(name)` — passing `collectionId` would violate the editor-safety invariant documented on `loadProfile`.

**H2. Collection defaults also bypassed on "Launch from Library card" flow** — `src/crosshook-native/src/components/pages/LibraryPage.tsx:65-76`

Same class of bug, different entrypoint. `LibraryPage.handleLaunch` calls `selectProfile(name)` with no options, then navigates to the launch route. `LibraryPage` does not currently pull `activeCollectionId` from the context, so:

- User sets `activeCollectionId=X` via the sidebar / a collection modal.
- Navigates to Library.
- Clicks Launch on a card for a profile that belongs to collection X (or any profile).
- Lands on `LaunchPage`. `activeCollectionId` is still `X`. The LaunchPage auto-select effect only re-selects when the current profile isn't in `filteredProfiles`, so if the profile IS in `X`'s filtered list, it stays loaded without defaults. If it ISN'T, the profile vanishes (set to `''` or swapped) and the user's intended target is no longer selected.
- Either way, the intended merge layer does not run for the originally clicked profile.

Fix: pull `activeCollectionId` from `useProfileContext` and thread it:

```tsx
const { ..., activeCollectionId } = useProfileContext();
// ...
await selectProfile(name, { collectionId: activeCollectionId ?? undefined });
```

Both H1 and H2 need to be resolved (or an explicit scope-cut decision made and documented) before merge — otherwise the feature only works when a user clicks the LaunchPage dropdown itself, which is not how most users will launch from a collection.

### MEDIUM

**M1. `get_collection_defaults` conflates "row not found" with a database error** — `crates/crosshook-core/src/metadata/collections.rs:336-364`

`conn.query_row(...)` surfaces `rusqlite::Error::QueryReturnedNoRows` through the `map_err` closure as `MetadataStoreError::Database { action: "read collection defaults", source: ... }`. The doc-comment on the function acknowledges this ("caller surfaces 'collection not found' via the `QueryReturnedNoRows` source") but it has two bad consequences:

1. Error-shape inconsistency with `set_collection_defaults`, which uses `affected == 0` to detect the same condition and returns `MetadataStoreError::Validation(format!("collection not found: {collection_id}"))`. Frontend code now sees two completely different error surfaces for the same semantic condition.
2. `profile_load`'s fallback (`src-tauri/src/commands/profile.rs:243-255`) logs `tracing::warn!(...)` with the raw `Display` impl of the error, which in the missing-row case becomes `"failed to read collection defaults: Query returned no rows"` — unhelpful, and indistinguishable from an actual IO / connection failure.

Recommended fix — branch on `QueryReturnedNoRows` in `get_collection_defaults` and convert to `Validation` for parity with `set_collection_defaults`:

```rust
let json: Option<String> = match conn.query_row(
    "SELECT defaults_json FROM collections WHERE collection_id = ?1",
    params![collection_id],
    |row| row.get(0),
) {
    Ok(v) => v,
    Err(rusqlite::Error::QueryReturnedNoRows) => {
        return Err(MetadataStoreError::Validation(format!(
            "collection not found: {collection_id}"
        )));
    }
    Err(source) => {
        return Err(MetadataStoreError::Database {
            action: "read collection defaults",
            source,
        });
    }
};
```

`test_collection_defaults_cascades_on_collection_delete` currently only asserts `result.is_err()` — update it to assert `matches!(result, Err(MetadataStoreError::Validation(_)))` so the contract is regression-guarded.

**M2. `effective_profile_with` precedence doc is misleading in the `profile_load` context** — `crates/crosshook-core/src/profile/models.rs:534-545`

The doc comment claims:

```
Precedence (lowest → highest):
  1. Base profile (`self`)
  2. Collection defaults (if `Some`)
  3. `local_override.*` — machine-specific paths always win last …
```

This is true when `effective_profile_with` is called directly on a raw-storage profile. But in practice, the only production caller is `profile_load`, which calls `store.load(&name)` first — and `ProfileStore::load` (`crates/crosshook-core/src/profile/toml_store.rs:153-165`) ALREADY applies `effective_profile()` internally and then clears `local_override = LocalOverrideSection::default()`. By the time `profile.effective_profile_with(Some(&defaults))` runs in `profile_load`, the layer-3 branch is a no-op because `self.local_override` is empty.

The actual runtime precedence is therefore:

```
(base ⊕ local_override, baked into `self`)  →  collection defaults  →  ∅
```

Today this has no user-visible effect because `CollectionDefaultsSection` and `LocalOverrideSection` have **zero field overlap** (collection = launch subset; local_override = machine paths). But if a future contributor adds a new field to both — e.g. extending `local_override` with an env-var bucket to sync with `custom_env_vars` — the "local_override always wins" guarantee will silently break, and the two regression tests (`effective_profile_with_merges_collection_defaults_between_base_and_local_override`, `effective_profile_prefers_local_override_paths`) both build their fixtures directly and won't catch it because they bypass `store.load()`.

Recommendations (pick one or both):

1. Add a regression test that exercises the `profile_load` code path end-to-end (load a profile with a populated `local_override`, read with a collection context, assert precedence). This locks in the real runtime behavior.
2. Update the doc comment on `effective_profile_with` to note that when called on a profile returned from `ProfileStore::load`, layer 3 is already baked into layer 1.

**M3. `profile_load` error fallback is dead code** — `src-tauri/src/commands/profile.rs:247-254`

On `get_collection_defaults` failure, the handler returns `profile.effective_profile_with(None)`:

```rust
Err(e) => {
    tracing::warn!(...);
    Ok(profile.effective_profile_with(None))
}
```

Because `store.load()` has already cleared `profile.local_override`, calling `effective_profile_with(None)` on it is functionally equivalent to `Ok(profile.clone())`. The extra clone is harmless, but the code reads as though it's doing something defensive (applying local_override layer) when it is not. Simpler and clearer:

```rust
Err(e) => {
    tracing::warn!(collection_id = %cid, error = %e, "failed to load collection defaults; launching with raw profile");
    Ok(profile)
}
```

This also matches the `_ => Ok(profile)` branch below so the two "no merge" paths are identical. Low-impact cleanup, but it'll age better.

**M4. Silent failure when collection defaults cannot be loaded** — `src-tauri/src/commands/profile.rs:247-254`

When `get_collection_defaults` returns an `Err` (corrupt JSON, missing collection, DB error), `profile_load` emits a `tracing::warn!` and silently falls back to the un-merged profile. The user sees no indication in the UI that the requested collection context was dropped — they'll launch the game and wonder why `CROSSHOOK_PROBE` isn't set.

This is a defensible "fail-open" trade-off for launch flows (a partial launch is better than a hard block), but the failure is truly invisible: no toast, no banner, no error state in the editor. For corrupt JSON specifically, the plan's own operator test step 7 verifies the _editor_ surfaces the error — but the launch path is distinct and does not. Recommendation: at minimum, include the corrupt-JSON case in an explicit branch that bubbles the error up so `LaunchStateProvider` / the LaunchPage can show a non-blocking notice.

### LOW

**L1. `addEnvVar` uniqueness loop uses raw keys, not trimmed** — `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx:137-148`

```tsx
const keys = new Set(rows.map((r) => r.key));
let i = 1;
let key = `NEW_VAR_${i}`;
while (keys.has(key)) { ... }
```

If a user manually named a row `NEW_VAR_1` with a leading/trailing space, `keys.has("NEW_VAR_1")` will be false and `addEnvVar` will happily create a duplicate. Low-impact because `envRowsToRecord` trims on save, but the UI momentarily shows two rows that look identical. Use trimmed keys in the check: `const keys = new Set(rows.map((r) => r.key.trim()));`.

**L2. Empty / whitespace-only env var keys are silently dropped on save** — `CollectionLaunchDefaultsEditor.tsx:40-47`

`envRowsToRecord` skips any row whose trimmed key is empty without feedback. If a user adds an env row, types a value, forgets to type a key, and hits Save, the row vanishes with no explanation. Consider either disabling Save when any row has an empty key, or showing an inline validation hint.

**L3. Env var name/value validation is absent** — `CollectionLaunchDefaultsEditor.tsx:150-158`

The inline editor accepts any characters in both fields. POSIX env var names are conventionally `[A-Za-z_][A-Za-z0-9_]*`, and the raw text is passed through to the launch mechanism via the merge. Linux `unshare --net` / `env` will typically tolerate arbitrary names but a key containing `=` would be broken. Not a correctness issue for the merge itself (the map stores whatever you give it), but worth a future validation pass. Not required for this PR.

**L4. `crypto.randomUUID()` availability** — `CollectionLaunchDefaultsEditor.tsx:34-37,146`

`crypto.randomUUID()` is used to generate React keys. It is available in every modern WebKit/Chromium version the Tauri WebView ships with, and the browser dev mode targets modern browsers, so this is fine — but note it WILL throw in a non-secure-context (`http://`) browser. The project's browser dev mode binds loopback-only, so this is only a theoretical concern.

## Validation Results

| Check                                              | Result                                  |
| -------------------------------------------------- | --------------------------------------- |
| `cargo test -p crosshook-core` (lib + integration) | **Pass** — 766 unit + 3 integration     |
| `cargo clippy -p crosshook-core`                   | **Pass** — 30 warnings (baseline)       |
| `cargo check --manifest-path src-tauri/Cargo.toml` | **Pass**                                |
| `tsc --noEmit` (frontend)                          | **Pass** — zero errors                  |
| `./scripts/build-native.sh`                        | **Skipped** — operator-run before merge |

Clippy came in at 30 warnings, one below the 31 noted in the PR body. Not a concern; none introduced.

## Files Reviewed

| File                                                                      | Action | Notes                                                                              |
| ------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/models.rs`                             | M      | `CollectionDefaultsSection` type + `effective_profile_with` refactor + 4 new tests |
| `crates/crosshook-core/src/profile/mod.rs`                                | M      | re-export of `CollectionDefaultsSection`                                           |
| `crates/crosshook-core/src/metadata/migrations.rs`                        | M      | v19→v20 migration with idempotent guard + test                                     |
| `crates/crosshook-core/src/metadata/collections.rs`                       | M      | `get_/set_collection_defaults` free functions; **M1 error-shape concern**          |
| `crates/crosshook-core/src/metadata/mod.rs`                               | M      | `MetadataStore` wrappers + 5 new tests                                             |
| `src-tauri/src/commands/collections.rs`                                   | M      | 2 new Tauri commands — thin passthroughs                                           |
| `src-tauri/src/commands/profile.rs`                                       | M      | `profile_load` extended; **M3/M4** on the error branch                             |
| `src-tauri/src/lib.rs`                                                    | M      | command registration                                                               |
| `src/App.tsx`                                                             | M      | **H1 bug** in `handleLaunchFromCollection`                                         |
| `src/components/collections/CollectionLaunchDefaultsEditor.tsx`           | A      | inline editor component; **L1/L2/L3** nits                                         |
| `src/components/collections/CollectionLaunchDefaultsEditor.css`           | A      | BEM-like classes, no new scroll containers                                         |
| `src/components/collections/CollectionViewModal.tsx`                      | M      | renders editor; new required `onOpenInProfilesPage` prop                           |
| `src/components/pages/LaunchPage.tsx`                                     | M      | threads `activeCollectionId` on onChange + auto-select — correct                   |
| `src/hooks/useCollectionDefaults.ts`                                      | A      | race-safe hook mirroring `useCollectionMembers`                                    |
| `src/hooks/useProfile.ts`                                                 | M      | `loadProfile` signature widened with documented editor-safety invariant            |
| `src/lib/mocks/handlers/collections.ts`                                   | M      | mock state + 2 new handlers + `MockCollectionDefaults` interface                   |
| `src/lib/mocks/handlers/profile.ts`                                       | M      | mock `profile_load` applies merge via `applyMockCollectionDefaults`                |
| `src/lib/mocks/wrapHandler.ts`                                            | M      | `collection_get_defaults` added to `EXPLICIT_READ_COMMANDS`                        |
| `src/types/profile.ts`                                                    | M      | `CollectionDefaults` interface + `isCollectionDefaultsEmpty` helper                |
| `docs/prps/archived/profile-collections-phase-3-launch-defaults.plan.md`  | A      | archived plan (docs-only)                                                          |
| `docs/prps/reports/profile-collections-phase-3-launch-defaults.report.md` | A      | implementation report (docs-only)                                                  |

Not reviewed in depth: `LibraryPage.tsx` is not part of this PR's diff, but **H2** observes it has the same class of bug. It should either be addressed in this PR or explicitly scoped out in the report with a follow-up issue.

## Required Before Merge

1. **H1** — pass `activeCollectionId` from `handleLaunchFromCollection` (App.tsx).
2. **H2** — either pass `activeCollectionId` from `LibraryPage.handleLaunch`, or file a follow-up issue and update the implementation report's "manual end-to-end" fixture to explicitly call out that the LaunchPage dropdown is the only merge-aware path today.
3. **M1** — convert `QueryReturnedNoRows` to `Validation` in `get_collection_defaults` for error-shape parity with `set_collection_defaults` and update the `cascades_on_collection_delete` test to assert the variant.

## Nice to Have (post-merge ok)

- M2 (precedence doc + end-to-end regression test)
- M3 (dead-code simplification in `profile_load` error branch)
- M4 (surface corrupt-defaults failure in launch path)
- L1/L2/L3 polish on the inline editor
