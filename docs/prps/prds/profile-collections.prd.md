# Profile Collections

> **Source**: [GitHub issue #73](https://github.com/yandy-r/crosshook/issues/73) — promoted from P3 to ambitious scope
> **Status**: DRAFT — needs validation
> **Generated**: 2026-04-08

---

## Problem Statement

Power users with 50+ CrossHook profiles cannot organize, find, or jump-start a session without scrolling a flat list. The Active-Profile dropdown (`ThemedSelect`) has no search and no grouping, and the sidebar today does not surface profiles at all — leaving users to scroll-hunt through dozens of cards on the Library page every time they sit down to play. The cost of _not_ solving this is a slow, friction-filled launch experience that gets monotonically worse as a user invests more in CrossHook — i.e., the most engaged users feel the most pain.

## Evidence

- **GitHub issue #73** explicitly identifies the flat-list pain and proposes user-created collections; storage boundary is already classified to SQLite schema v4 in the issue body.
- **Codebase confirms the gap**: the sidebar (`src/crosshook-native/src/components/layout/Sidebar.tsx:36-61`) renders only fixed routes — zero per-profile content. The Active-Profile picker (`LaunchPage.tsx:295-306`, `ProfilesPage.tsx:603-614`) is a `ThemedSelect` with no built-in search.
- **User confirmation (Phase 2)**: target users are power users with many profiles, Steam Deck users juggling loadouts, and testers maintaining experimental profile forks. Current workaround: none beyond the flat library card view.
- **Market evidence (Phase 3)**: Steam, Apple Music/iTunes, and Playnite all converged independently on collections + smart playlists as the gold standard for libraries that grow past ~50 items. Lutris shipped basic categories only in Feb 2025 (0.5.18); Heroic ships static categories in 2.11. **There is a real gap in the Linux launcher space** for Playnite-grade organization.
- **Schema and IPC are pre-built but unused**: schema v4 already creates `collections` and `collection_profiles` tables (`crates/crosshook-core/src/metadata/migrations.rs:229-298`); all six Tauri IPC commands exist and are registered (`src-tauri/src/commands/collections.rs:1-64`); `CollectionRow` is `#[allow(dead_code)]` (`metadata/models.rs:294-303`). The backend foundation has been waiting for a UI consumer.

## Proposed Solution

Build a **first-class collections layer** that surfaces in three places: a new sidebar **Collections** section, a **collection view modal** that lets users search/filter/launch from inside, and an **Active-Profile dropdown** that respects the current collection filter. Collections carry both **identity** (a named, persistent group of profiles) and **behavior** (per-collection launch defaults that override profile config at launch time). Collections export as **shareable TOML preset files** for cross-machine portability — runtime storage in SQLite, wire format in TOML. The implementation strictly composes existing primitives (`useLibraryProfiles`, `effective_profile()`, `GameDetailsModal` portal pattern, `chooseSaveFile`/`chooseFile`) and introduces **zero new JS dependencies**.

This approach beats the alternatives because:

- **vs. tags-only on profiles**: collections are user-named, persistent, multi-membership, and can carry behavior. Tags can't.
- **vs. dropdown-only grouping**: a sidebar surface is constant-visibility and matches the user's actual mental model ("I want to jump to my collection, then filter inside it").
- **vs. dynamic-only smart collections**: requires a query language and metadata reliability we don't yet have. Static collections deliver immediate value; dynamic collections are a v2 follow-up that reuses the same schema.
- **vs. file-system folders**: profiles already have stable filenames in `~/.config/crosshook/profiles/` and re-foldering them would break existing portable share flows. Collections are an _orthogonal_ axis on top of the flat profile namespace.

## Key Hypothesis

We believe **named collections with per-collection launch defaults and a search-inside view modal** will eliminate the _scroll-the-80-profile-dropdown_ pain for **power users with 50+ profiles**. We'll know we're right when **users create ≥3 collections within the first week of upgrading and qualitative complaints about the Active-Profile dropdown drop**.

## What We're NOT Building

| Out of Scope                                                                                                                                   | Why                                                                                                                                                                                                   |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Bulk launch** (launch all profiles in a collection sequentially)                                                                             | v2 — niche; useful only for testers; high blast radius if mis-clicked                                                                                                                                 |
| **Bulk env-var apply** (apply Proton env vars to every profile in a collection at once)                                                        | v2 — powerful but high risk of accidentally breaking many profiles. Per-collection _launch defaults_ (which apply non-destructively at launch time) cover the safe subset of this need                |
| **Dynamic / smart collections** (rule-based membership, e.g. "all profiles last launched > 30 days")                                           | v2 — requires query model + Boolean logic; v1 ships static collections only                                                                                                                           |
| **Drag-and-drop reordering / drag-to-assign**                                                                                                  | v1 — no DnD library installed; Steam Deck (controller) is a stated target where DnD is hostile. Right-click menu + multi-select chip patterns are controller-friendlier and match existing precedents |
| **Per-collection cover art / icons / colors**                                                                                                  | v2 polish — Spotify/iTunes have it but it's pure decoration on top of the same data model                                                                                                             |
| **Generic `Collection<T>` schema with `entity_kind` discriminator** (so collections could later hold env-var presets or optimization profiles) | v1 — coupling cost is low; future generalization can be added via sibling tables (`collection_optimizations`, etc.) **without** a destructive migration. Decision documented in §Decisions Log        |
| **Generalized `useLaunch` hook** (one-click launch from any UI)                                                                                | v2 or v3 — v1 reuses the existing select-then-navigate indirection that `GameDetailsModal` already uses                                                                                               |
| **Soft-delete of collections**                                                                                                                 | v1 — profiles are soft-deleted but collections are simpler; hard-delete is acceptable until users complain                                                                                            |
| **Import-side profile auto-creation** (importing a preset that references a profile you don't have should NOT create stub profiles)            | v1 — the import review modal will surface unmatched profiles for the user to resolve manually                                                                                                         |

## Success Metrics

| Metric                                                                                         | Target                                                                                                | How Measured                                                                                                                          |
| ---------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Adoption** — users with ≥1 collection                                                        | ≥40% of users with 20+ profiles within 2 weeks of release                                             | Count of distinct users with `SELECT COUNT(*) FROM collections > 0` (local-only — no telemetry; rely on issue/Discord/Reddit reports) |
| **Engaged adoption** — collections per power user                                              | ≥3 collections / user for users with 50+ profiles within 4 weeks                                      | Anecdotal: GitHub Discussions thread + community feedback                                                                             |
| **Complaint drop** — qualitative reduction in dropdown / sidebar / "can't find profile" issues | Visible drop in dropdown-related issues filed in the 4 weeks post-release vs. the 4 weeks pre-release | GitHub issue search: `is:issue label:area:ui dropdown OR sidebar OR scroll`                                                           |
| **Feature reachability** — time to launch a profile in a 50-profile library                    | ≤2 clicks from any context (sidebar → collection → profile → launch)                                  | Manual test on a fixture profile set; defined by user flow below                                                                      |
| **Per-collection defaults adoption** — users actively setting per-collection launch defaults   | ≥1 collection with non-default launch overrides per power-user adopter within 4 weeks                 | Local-only; gauge via community feedback                                                                                              |
| **TOML preset sharing** — at least one community-shared collection preset within 2 weeks       | 1 example preset shared via GitHub Discussions / Discord                                              | Direct observation                                                                                                                    |

## Open Questions

- [ ] **Assign-to-multiple ergonomics**: should v1 support assigning a profile to multiple collections in a single interaction (multi-select checkbox dialog), or is one-at-a-time acceptable for v1? _User said "assign to multiple can be deferred if complex for MVP" — needs a complexity assessment in plan phase._
- [ ] **Profile ID matching disambiguation UX** for TOML import — exact wireframe/copy for the review modal when matches are ambiguous (e.g., 6/8 matched, 2 ambiguous). The pattern exists in `CommunityImportWizardModal` but the copy needs to be specific to collections.
- [ ] **Exact TOML schema for per-collection defaults**: which `LaunchSection` fields are user-editable in the collection-edit modal vs. read-only? The user agreed _"LaunchSection only, with redirect to specific pages for more advanced work"_ — the cut line between "edit inline" and "redirect to ProfilesPage" needs to be drawn explicitly.
- [ ] **Should "Favorites" become Collection #0**? `is_favorite` is a boolean column on profiles today (`PinnedProfilesStrip` is its UI). Consolidating into the collections model is conceptually clean but increases v1 scope. **Recommendation**: keep separate for v1, file as v1.1 cleanup.
- [ ] **Empty-state copy and CTA** for "you have no collections yet" — small but matters for first-launch impression. Defer to plan phase.
- [ ] **Collection name uniqueness UX**: backend already enforces `UNIQUE` on `collections.name`. What's the failure UX in the create-modal? Inline error or post-submit toast?

---

## Users & Context

### Primary User — Power User with 50+ Profiles

- **Who**: A long-time CrossHook user who has invested in building 50–200 profiles across multiple Proton/Wine variants, trainer types, and game generations. Often runs CrossHook on a Steam Deck or a desktop Linux workstation. May be a tester / modder who maintains 3–5 _variants_ of the same game's profile.
- **Current behavior**: Opens CrossHook → scrolls the Library card grid OR opens the Active-Profile dropdown → squints → mis-clicks the wrong fork → corrects → finally launches. Repeats this dance every session.
- **Trigger**: "I sit down for a play session" or "I want to test my last config tweak on the Elden Ring [WIP] fork specifically."
- **Success state**: Two clicks from app open to "the right profile is launching" — even when the library is large.

### Secondary Users (optimized-for _only_ when not in conflict with primary)

- **Steam Deck users** juggling handheld vs. docked profile variants — controller-friendly UX is a hard constraint, which is _why_ DnD is excluded.
- **Testers** maintaining stable / WIP / experimental forks of the same profile — collections like "Stable", "WIP", "Broken — needs investigation" are the canonical use case.

### Job to Be Done

> When **I sit down to play tonight**, I want to **jump to my collections, select one (e.g., "Action/Adventure"), and filter/search inside it without scrolling**, so I can **launch the profile I want in two clicks even when the collection has 50+ profiles**.

### Non-Users (Explicitly Out of Target)

- **Casual users** with fewer than ~10 profiles — the flat list works fine for them; they should not be forced into a collections workflow.
- **Non-heavy gamers** who launch CrossHook once a week — they will not benefit from organizational features and should not see significant UI churn.
- These non-users are protected by making the collections sidebar **opt-in** (renders only when ≥1 collection exists) and by preserving the existing flat library card view exactly as it is today.

---

## Solution Detail

### Core Capabilities (MoSCoW)

| Priority       | Capability                                                                                                                                                                                                                                                                      | Rationale                                                                                                                   |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| **Must**       | Create / rename / delete a collection                                                                                                                                                                                                                                           | Foundational CRUD; rename is _new_ IPC (not in the existing 6 commands)                                                     |
| **Must**       | Add / remove a profile from a collection (via right-click menu and via the collection view modal)                                                                                                                                                                               | Foundational membership; mirrors `is_favorite` toggle UX                                                                    |
| **Must**       | Sidebar **Collections** section showing all user-created collections                                                                                                                                                                                                            | Constant-visibility surface; matches Steam/Spotify pattern                                                                  |
| **Must**       | **Collection view modal** — opens when a sidebar collection is clicked, shows the collection's profiles, supports inline search + filter, allows launching and editing profiles via select-then-navigate indirection                                                            | The primary interaction loop. Mirrors `GameDetailsModal` portal pattern                                                     |
| **Must**       | **Per-collection launch defaults** — collection-level overrides for `LaunchSection` fields (`custom_env_vars`, `optimizations`, `gamescope`, `mangohud`, `method`); merged into the launch chain at `effective_profile()` time when launched-from-collection context is present | The "behavior" leg of "both visual + behavior". Where most engineering risk lives                                           |
| **Must**       | **Filter the Active-Profile dropdown to the active collection** (ephemeral runtime state, resets on app restart)                                                                                                                                                                | Reuses the dropdown without rebuilding it; closes the loop on the user's "scroll a long dropdown" complaint                 |
| **Must**       | **Export collection as TOML preset** (`.crosshook-collection.toml`) via `chooseSaveFile`; preset includes name, description, per-collection defaults, and multi-field profile descriptors (`steam.app_id`, `game.name`, `trainer.community_trainer_sha256`)                     | Sharing is the wire format constraint; multi-field IDs are required because there's no stable cross-install profile UUID    |
| **Must**       | **Import collection preset** with a review modal that surfaces matched / ambiguous / unmatched profiles for user resolution                                                                                                                                                     | Mirrors `CommunityImportWizardModal` / `ProfileReviewModal` pattern                                                         |
| **Must**       | **Empty state** for the sidebar Collections section ("Create your first collection")                                                                                                                                                                                            | First-launch UX; small but visible                                                                                          |
| **Must**       | **Browser dev-mode mocks** for all collection IPC commands (existing 6 + new ones)                                                                                                                                                                                              | `pnpm dev:browser` will crash on collection calls until added — non-negotiable per CLAUDE.md `verify:no-mocks` CI sentinel  |
| **Should**     | Assign a profile to multiple collections in one interaction (multi-select dialog)                                                                                                                                                                                               | Power-user ergonomic. _User said this can be deferred if complex._ If it slips: one-at-a-time fallback is acceptable for v1 |
| **Should**     | Reverse lookup — show "this profile belongs to: X, Y, Z" as chips on the Profile detail page                                                                                                                                                                                    | Discoverability; needs new IPC `collections_for_profile`                                                                    |
| **Should**     | Edit a collection's per-collection launch defaults _inline_ in the collection edit modal for the simple `LaunchSection` fields, with a **"Open in Profiles page →" link** for advanced overrides                                                                                | User said "redirect to specific pages for more advanced work" — this is the cut line                                        |
| **Should**     | Extract a shared `<Modal>` primitive (focus trap + body lock + portal) and migrate at least the new collection modals to it                                                                                                                                                     | ≥8 modals duplicate this logic today; not strictly required for v1 but every new modal otherwise adds debt                  |
| **Could**      | Inline collection name editing (double-click to rename)                                                                                                                                                                                                                         | Discoverability over modal-based rename                                                                                     |
| **Could**      | Show profile count badge on each sidebar collection entry                                                                                                                                                                                                                       | Already trivial — `CollectionRow.profile_count` is computed by `MetadataStore::list_collections`                            |
| **Won't (v1)** | Drag-and-drop, dynamic/smart collections, bulk launch, bulk env-var apply, cover art, generic `Collection<T>`, soft-delete, generalized `useLaunch`                                                                                                                             | See "What We're NOT Building"                                                                                               |

### MVP Scope (irreducible core if runway runs out)

If we ship only one thing from v1: **the sidebar Collections section + the collection view modal with filter + add/remove/rename collections + add/remove profiles + launch via select-then-navigate indirection**. This validates the "I can find and launch a profile in 2 clicks from a 50-profile library" hypothesis without the more complex per-collection-defaults and TOML export work.

The "Must" items above all ship in v1 — the MVP-scope clause exists only as a fallback if Phase 3 (per-collection defaults) or Phase 4 (TOML export/import) proves more expensive than the plan estimates.

### User Flow — Critical Path to Value

**First-time creation:**

1. User opens CrossHook → sidebar shows Library/Profiles/Launch as today + new empty Collections section with "Create collection" CTA
2. Click "Create collection" → modal: name + optional description + (optional) initial profiles via multi-select
3. Submit → collection appears in sidebar → success toast

**Return-session usage (the JTBD path):**

1. User opens CrossHook
2. Click sidebar collection "Action/Adventure" → collection view modal opens with filterable list of member profiles
3. Type `elden` in the modal's search → list narrows to one entry
4. Click profile → modal closes → Launch page opens with profile pre-selected (existing select-then-navigate pattern)
5. Click Launch → game launches with collection-level overrides applied (because launch context carried `activeCollectionId`)

**Total clicks**: 4 (sidebar → modal → profile → launch). Zero scrolling. Zero typing beyond a 5-character search.

**Sharing:**

1. From collection view modal → "Export…" → file save dialog → write `my-action-collection.crosshook-collection.toml` to disk
2. Send file to friend
3. Friend → Settings or Collections sidebar → "Import collection…" → file open dialog → review modal shows 6 matches / 1 ambiguous / 1 unmatched → user resolves → import completes

---

## Technical Approach

**Overall Feasibility**: 🟡 **MEDIUM** — high on UI/composition/export, medium on per-collection launch defaults (the architectural crux). All required infrastructure is already installed; **zero new dependencies**.

### Architecture Notes

- **Storage**: SQLite metadata (schema v4 already in place); v5 migration adds (a) `ON DELETE CASCADE` to `collection_profiles.profile_id` FK, (b) optional `collection_launch_defaults` table or inline JSON column on `collections` for per-collection overrides, (c) `sort_order INTEGER` column on `collections` for stable ordering.
- **Wire format for sharing**: TOML, mirroring `profile_to_shareable_toml` (`crates/crosshook-core/src/profile/toml_store.rs:508-524`). Uses `schema_version` field per the `COMMUNITY_PROFILE_SCHEMA_VERSION` precedent (`profile/community_schema.rs:5`).
- **Merge layer**: Rust-side. New `CollectionDefaultsSection` serde type holding the overrideable subset of `LaunchSection`. `effective_profile()` extended to accept an optional `Option<&CollectionDefaultsSection>` and apply it as a layer **between** base and `local_override` (precedence: base → collection defaults → local_override; local always wins last because local paths are machine-specific truths).
- **Launch context plumbing**: New `activeCollectionId?: string` carried through `LaunchStateContext` (`src/context/LaunchStateContext.tsx:18-37`); `profile_load_with_collection(name, collection_id)` IPC OR extension of `profile_load` to accept optional `collection_id`. Decision: _extend `profile_load`_ to avoid IPC duplication.
- **Sidebar surface**: New `<CollectionsSection>` component below the existing fixed routes in `Sidebar.tsx`. Renders only when `collections.length > 0` (opt-in for non-power-users). Uses the existing `crosshook-sidebar__nav-item` class pattern. **MUST** add new scrollable inner panel (if any) to `useScrollEnhance.ts:9` `SCROLLABLE` selector or WebKitGTK wheel scroll will misroute.
- **Modal surface**: New `<CollectionViewModal>` mirrors `GameDetailsModal` (`src/components/library/GameDetailsModal.tsx:1-398`) — `createPortal` + manual focus trap + body scroll lock. **Should** extract a shared `<Modal>` primitive and migrate the new modals to it. The body uses `.crosshook-modal__body` (already in `SCROLLABLE`); a search input + filtered list compose with `useLibraryProfiles(profilesInCollection, searchQuery)`.
- **Hooks**: New `useCollections` hook wraps the IPC commands; new `useCollectionMembers(collectionId)` returns the filtered profile list. State container = React Context only (no Redux/Zustand — matching repo convention).
- **Modularity (constraint)**: collection primitives are profile-coupled in v1; future entity types added via sibling tables (`collection_optimizations`, `collection_envvars`) when needed. Decision documented in §Decisions Log.

### Backend gap closure (preconditions in Phase 1)

Critical foot-guns that must be fixed _before_ the UI consumes the IPC:

1. **`add_profile_to_collection` silent no-op** (`metadata/collections.rs:88-96`) — currently logs a warning and returns `Ok(())` when the profile name doesn't resolve. Frontend cannot distinguish "added" from "skipped". **Fix**: return `Err(MetadataStoreError::Validation { ... })` and surface in the Tauri command.
2. **`collection_profiles.profile_id` FK lacks `ON DELETE CASCADE`** (`migrations.rs:284-289`) — deleting a profile leaves orphan membership rows. **Fix**: schema v5 migration to add the cascade.
3. **Missing IPC commands**: `collection_rename`, `collection_update_description`, `collections_for_profile` (reverse lookup). **Fix**: add to `commands/collections.rs` with corresponding `MetadataStore` methods.
4. **No browser dev-mode mocks** for any of the 6 existing or 3 new commands. **Fix**: add to `src/lib/ipc.dev` mock layer; required by `verify:no-mocks` CI sentinel and for `pnpm dev:browser` to not crash.

### Technical Risks

| Risk                                                                                                                                          | Likelihood | Mitigation                                                                                                                                                                                                                                          |
| --------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Per-collection defaults merge requires hand-wiring every overrideable field in `effective_profile()` (`profile/models.rs:486-545`)            | **High**   | Define `CollectionDefaultsSection` to mirror only the `LaunchSection` overrideable subset (~6 fields, not all 30+). Keep the merge tight                                                                                                            |
| Threading `activeCollectionId` through `LaunchStateContext` adds new state plumbing across context + provider + multiple consumers            | **High**   | Add as a single optional context value; default `undefined`; only the new collection-launch path reads it. Existing flows are unaffected                                                                                                            |
| Profile identification on TOML import is unreliable (no stable cross-install UUID)                                                            | **High**   | Multi-field matching (`steam.app_id`, then `(game.name, trainer.community_trainer_sha256)`, then user disambiguation review modal). Mirrors `CommunityImportWizardModal` flow. Document explicitly that non-Steam profiles may need manual matching |
| Sidebar collections panel introduces a new scrollable container that breaks WebKitGTK enhanced scroll if not added to `useScrollEnhance.ts:9` | **Medium** | Phase 2 acceptance criterion: any new `overflow-y: auto` container added to `SCROLLABLE` selector + uses `overscroll-behavior: contain`                                                                                                             |
| `add_profile_to_collection` silent no-op corrupts the new feature's UX if not fixed first                                                     | **Medium** | Phase 1 precondition; ~10-line fix                                                                                                                                                                                                                  |
| Orphan FK rows on profile delete corrupt collection counts                                                                                    | **Medium** | Phase 1 precondition; schema v5 migration                                                                                                                                                                                                           |
| `CollectionRow` and 6 IPC commands are dead-code today — first real use surfaces latent bugs                                                  | **Medium** | Phase 1 includes integration tests for every CRUD path against a real SQLite store                                                                                                                                                                  |
| Steam Deck (controller) ergonomics for "add profile to collection" are not validated by any existing pattern                                  | **Medium** | Use right-click menu + button, **not** DnD. Test on a Deck or `gamescope` session before Phase 5 ships                                                                                                                                              |
| Shared `<Modal>` extraction (Should-have) leaks scope into v1                                                                                 | **Low**    | Defer to a follow-up if Phase 2 runs long; new collection modals can copy `GameDetailsModal` directly as a fallback                                                                                                                                 |
| Collection name uniqueness collision UX is undefined                                                                                          | **Low**    | Inline error in create modal — `MetadataStore::create_collection` already enforces `UNIQUE`; surface as friendly error                                                                                                                              |
| Per-collection defaults precedence (vs `local_override`) confuses users                                                                       | **Low**    | Document in collection edit modal: "Local machine paths always win — collection defaults apply on top of profile config but below your local overrides"                                                                                             |

---

## Implementation Phases

<!--
  STATUS: pending | in-progress | complete
  PARALLEL: phases that can run concurrently (e.g., "with 3" or "-")
  DEPENDS: phases that must complete first (e.g., "1, 2" or "-")
  PRP: link to generated plan file once created
-->

| #   | Phase                                            | Issue | Description                                                                                                                                                                                                        | Status      | Parallel | Depends | PRP Plan |
| --- | ------------------------------------------------ | ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------- | -------- | ------- | -------- |
| 1   | Backend foundation                               | #177  | Fix `add_profile_to_collection` no-op, schema v19 migration (FK cascade + sort_order), missing IPC commands, browser dev-mode mocks, integration tests                                                             | in-progress | -        | -       | [profile-collections-phase-1-backend-foundation.plan.md](../archived/profile-collections-phase-1-backend-foundation.plan.md) |
| 2   | Sidebar + view modal                             | #178  | `useCollections` hook, sidebar Collections section, view modal with filter/search, add/remove from collection, launch+edit via select-then-navigate, empty state                                                   | pending     | with 3   | 1       | -        |
| 3   | Per-collection launch defaults                   | #179  | `CollectionDefaultsSection` serde type, `effective_profile()` extension, `profile_load` collection-context param, `activeCollectionId` plumbing through `LaunchStateContext`, edit-defaults UI in collection modal | pending     | with 2   | 1       | -        |
| 4   | TOML export / import preset                      | #180  | TOML schema v1 with `schema_version`, `collection_export_to_toml`, `collection_import_from_toml`, multi-field profile matching, import review modal                                                                | pending     | -        | 1, 3    | -        |
| 5   | Polish, integration tests, Steam Deck validation | #181  | End-to-end test fixtures, controller-friendliness validation, empty-state copy, keyboard nav, docs                                                                                                                 | pending     | -        | 2, 3, 4 | -        |

### Phase Details

**Phase 1: Backend foundation**

- **Goal**: Make the existing dead-code IPC surface production-ready and add missing primitives so the frontend can consume it safely.
- **Scope**:
  - Fix `add_profile_to_collection` silent no-op → return typed error
  - Add `ON DELETE CASCADE` to `collection_profiles.profile_id` FK (schema v5 migration)
  - Add `sort_order INTEGER` column to `collections` (schema v5)
  - Implement `MetadataStore::rename_collection`, `update_collection_description`, `collections_for_profile`
  - Add Tauri commands `collection_rename`, `collection_update_description`, `collections_for_profile`
  - Add browser dev-mode mocks for all 6 existing + 3 new collection IPC commands
  - Backend integration tests for every CRUD + edge cases (rename, multi-membership, reverse lookup, FK cascade)
  - Remove `#[allow(dead_code)]` from `CollectionRow`
- **Success signal**: `cargo test -p crosshook-core` passes, all 9 commands callable from frontend, `pnpm dev:browser` doesn't crash on collection calls.

**Phase 2: Sidebar + view modal**

- **Goal**: Deliver the irreducible MVP — users can create, populate, view, filter, search, and launch profiles via collections.
- **Scope**:
  - `useCollections` React hook (CRUD + list)
  - `useCollectionMembers(collectionId)` hook
  - `<CollectionsSection>` in `Sidebar.tsx` (renders only when ≥1 collection exists)
  - `<CollectionViewModal>` mirroring `GameDetailsModal` pattern, with inner search input + `useLibraryProfiles` composition
  - Add/remove profile from collection via right-click context menu and modal action
  - Multi-select assign-to-multiple dialog _(or one-at-a-time fallback if complex)_
  - Launch + edit from modal via existing select-then-navigate indirection
  - Active-Profile dropdown filter to active collection (ephemeral runtime state in `ProfileContext`)
  - Empty state "Create your first collection" CTA
  - Update `useScrollEnhance.ts:9` SCROLLABLE selector for any new scroll containers
  - **Should**: extract shared `<Modal>` primitive (deferred if too costly)
- **Success signal**: User can complete the JTBD critical-path flow (sidebar → collection → filter → launch) end-to-end. No regressions in existing Library page or Active-Profile dropdown.

**Phase 3: Per-collection launch defaults**

- **Goal**: Add the "behavior" leg — collections actually affect launches.
- **Scope**:
  - `CollectionDefaultsSection` serde type (subset of `LaunchSection`: `custom_env_vars`, `optimizations`, `gamescope`, `mangohud`, `method`)
  - Schema v5: `collection_launch_defaults` table or inline TEXT column on `collections` (decision in plan phase based on sqlx ergonomics)
  - `MetadataStore` methods to read/write per-collection defaults
  - New IPC: `collection_get_defaults`, `collection_set_defaults`
  - Extend `effective_profile()` (`profile/models.rs:486`) to accept `Option<&CollectionDefaultsSection>` and apply as a layer between base and `local_override` (precedence: base → collection defaults → local_override)
  - Extend `profile_load` IPC to accept optional `collection_id`
  - Thread `activeCollectionId?: string` through `LaunchStateContext` and `useLaunchState`
  - Edit-defaults inline UI in `CollectionViewModal` for the simple `LaunchSection` fields
  - "Open in Profiles page →" link-out for advanced overrides (matches the "redirect to specific pages for more advanced work" decision)
- **Success signal**: A profile launched from a collection with custom env vars receives those env vars merged into its launch chain; the same profile launched from the Library page does **not** receive them. Verified with a fixture profile + `printenv` test.

**Phase 4: TOML export / import preset**

- **Goal**: Make collections shareable across machines.
- **Scope**:
  - Define `crosshook-collection.toml` schema v1 with `schema_version = "1"` field, mirroring `COMMUNITY_PROFILE_SCHEMA_VERSION`
  - Multi-field profile descriptors (`steam_app_id`, `game_name`, `trainer_community_trainer_sha256`)
  - `collection_export_to_toml(collection_id, output_path)` Rust function + Tauri command
  - `collection_import_from_toml(input_path)` with multi-field matching pass + ambiguous/unmatched buckets
  - `<CollectionImportReviewModal>` mirroring `CommunityImportWizardModal` / `ProfileReviewModal`
  - File save / open via existing `chooseSaveFile` / `chooseFile` (`utils/dialog.ts`)
  - Roundtrip tests: export → re-import on the same machine → identical collection (membership + defaults)
- **Success signal**: A collection exported on machine A and imported on machine B reproduces with all its members matched (assuming both have the same source profiles). Unmatched profiles surface in a review modal for the user to resolve or skip.

**Phase 5: Polish, integration tests, Steam Deck validation**

- **Goal**: Ship-ready quality bar.
- **Scope**:
  - End-to-end test fixtures for the JTBD flow
  - Manual Steam Deck / `gamescope` controller validation (no DnD, all interactions reachable via D-pad)
  - Keyboard navigation audit (Tab order, Esc to close modal, Enter to confirm)
  - Empty-state copy review
  - Internal docs update (`docs/internal/` if applicable)
  - User-facing changelog entry (Conventional Commit `feat(ui):` per CLAUDE.md)
- **Success signal**: All AC from issue #73 closed; no regressions on existing flows; Steam Deck flow confirmed end-to-end.

### Parallelism Notes

- **Phase 2 and Phase 3 can run in parallel** with two engineers because they touch disjoint code paths: Phase 2 is frontend-only (modal + sidebar + hook), Phase 3 is mostly backend (`effective_profile()` + serde types + new IPC) plus one new modal section. Both depend on Phase 1's IPC additions but neither depends on the other's deliverables — they integrate via the `activeCollectionId` field which Phase 2 plumbs through context and Phase 3 consumes.
- **Phase 4 depends on Phase 3** because the export schema must include the per-collection defaults shape. It cannot start until `CollectionDefaultsSection` is defined.
- **Phase 5 depends on all** — it's the integration polish gate.
- **Single-engineer fallback**: serial 1 → 2 → 3 → 4 → 5 also works without parallelism risk.

---

## Decisions Log

| Decision                                  | Choice                                                                   | Alternatives                                                                          | Rationale                                                                                       |
| ----------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| **Scope: visual + behavior**              | Both — collections carry per-collection launch defaults                  | Visual grouping only                                                                  | User explicitly chose "both" in Phase 4. Differentiates from Heroic/Lutris                      |
| **Storage classification**                | SQLite for runtime, TOML for sharing                                     | TOML for both / SQLite for both                                                       | Repo convention (CLAUDE.md): SQLite for queryable metadata; TOML for human-editable / shareable |
| **Boolean dynamic collections**           | OUT of v1                                                                | Steam-style hybrid in v1                                                              | Doubles scope; differentiator deferred to v2                                                    |
| **Drag-and-drop**                         | NOT in v1                                                                | `@dnd-kit` integration                                                                | No DnD library installed; Steam Deck (controller) is target — DnD is hostile                    |
| **Merge layer placement**                 | Rust-side via `effective_profile()` extension                            | TypeScript-side in `buildProfileLaunchRequest`                                        | User accepted recommendation; matches existing layering idiom and survives backend refactors    |
| **Override scope per collection**         | `LaunchSection` only, with link-out for advanced fields                  | All overrideable sections / `LaunchSection` + `runtime` / `LaunchSection` + `trainer` | User said "LaunchSection only with redirect to specific pages for more advanced work"           |
| **Precedence order**                      | base → collection defaults → local_override                              | base → local_override → collection defaults                                           | Local machine paths must always win last (machine-specific truths). User confirmed              |
| **Profile ID matching for TOML import**   | Multi-field with user disambiguation review modal                        | Best-effort silent match / export-only v1                                             | User chose (b); mirrors existing `CommunityImportWizardModal` pattern                           |
| **Launch from modal**                     | Match existing select-then-navigate indirection                          | Generalized one-click `useLaunch` action                                              | User chose "match for now, direct later v2 or v3"; ships faster without architectural risk      |
| **Generic `Collection<T>` primitive**     | Profile-coupled v1; sibling tables for future entity types               | Add `entity_kind` discriminator now                                                   | User chose (b); coupling cost is low; future generalization is non-destructive                  |
| **Favorites consolidation**               | Keep `is_favorite` separate from collections in v1                       | Make Favorites = Collection #0                                                        | Increases v1 scope; clean up in v1.1                                                            |
| **Soft-delete collections**               | Hard-delete v1                                                           | Soft-delete with `deleted_at` like profiles                                           | Simpler; users can re-create; revisit if complaints                                             |
| **Sidebar opt-in rendering**              | Render Collections section only when ≥1 collection exists                | Always render with empty state                                                        | Protects non-power-users from UI churn; matches "non-users" exclusion                           |
| **`add_profile_to_collection` no-op fix** | Return typed error                                                       | Keep silent skip + log                                                                | Foundational correctness fix; pre-existing foot-gun that would corrupt the new feature's UX     |
| **Schema v5 migration scope**             | FK cascade + `sort_order` + collection defaults storage in one migration | Three separate migrations                                                             | Single coherent migration; less migration overhead per CLAUDE.md migration policy               |

---

## Persistence & Usability

- **Migration**: Schema v5 migration adds (a) `ON DELETE CASCADE` to `collection_profiles.profile_id` FK, (b) `sort_order INTEGER` column on `collections`, (c) per-collection launch defaults storage. The `collections` and `collection_profiles` tables themselves already exist in schema v4 and require **no destructive migration**. Backward compatibility: schema v4 → v5 is additive; downgrade not supported per repo policy.
- **Offline**: Collections are 100% local — no network dependency at any point. TOML export writes to local filesystem; import reads from local filesystem. Works fully offline.
- **Degraded fallback**: If `MetadataStore` is unavailable (e.g., DB locked, disk full), the sidebar Collections section is hidden and the existing flat library card view + flat Active-Profile dropdown render unchanged. No data loss; the user simply loses the new organizational layer until the DB recovers. Documented as a no-data-loss fallback in the issue body.
- **User visibility**: Collections are user-created and named via the UI. The collection database row is **not** directly file-editable (unlike profile TOMLs in `~/.config/crosshook/profiles/`), but **collection presets exported as TOML** _are_ fully human-editable and shareable — that's the user-facing escape hatch for power-user editing and sharing. Per-collection launch defaults are inline-editable in the collection edit modal for `LaunchSection` fields, with link-outs to the full Profiles page for advanced overrides.

---

## Research Summary

### Market Context

- **Steam, iTunes/Apple Music, and Playnite** independently converged on hybrid static + dynamic collections as the gold standard for libraries that grow past ~50 items. **Steam's #1 power-user complaint is AND-only Boolean logic** on dynamic collections — no OR, no NOT — which is exactly the differentiator a future v2 dynamic-collection feature could exploit.
- **Lutris** shipped categories only in Feb 2025 (0.5.18). **Heroic** ships static categories in 2.11 (Nov 2023) but they're static-only, with no smart collections. **Bottles** has no grouping at all.
- **There is a real gap in the Linux launcher space** for Playnite-grade organization. v1 closes most of that gap; v2 (dynamic collections + Boolean operators) makes CrossHook genuinely best-in-class on Linux.
- **Right-click context menu + multi-select** is the UX pattern that survives controller-first contexts. Drag-and-drop is hostile to Steam Deck users and is therefore explicitly excluded.

### Technical Context

- **Backend foundation is 80% pre-built**: schema v4 has the tables, all 6 IPC commands exist (registered in `src-tauri/src/lib.rs:281-286`), `CollectionRow` and `MetadataStore` collection methods exist. v1 adds 3 IPC commands, 1 schema v5 migration, 1 new serde type, and 1 line of `effective_profile()`-layer extension on the backend.
- **Frontend foundation is 60% pre-built**: `useLibraryProfiles` is composable, `GameDetailsModal` is the modal precedent, `PinnedProfilesStrip` is the chip-strip pattern, `ProfileContext` + `ThemedSelect` are the dropdown integration points, `chooseSaveFile`/`chooseFile` are wired up. v1 adds 2 hooks (`useCollections`, `useCollectionMembers`), 1 sidebar section, 1–2 new modals, 1 import-review modal, and updates the active-profile dropdown to filter by active collection.
- **Zero new dependencies required**. All required libraries (`@radix-ui/react-select`, `@radix-ui/react-tabs`, `@radix-ui/react-tooltip`, `tauri-plugin-dialog` v2, `tauri-plugin-fs` v2, `toml` v1.1.0, `react-dom` `createPortal`) are already in the tree.
- **Three foot-guns to fix as Phase 1 preconditions**: silent no-op on `add_profile_to_collection`, missing FK cascade on `collection_profiles.profile_id`, and the missing browser dev-mode mocks (`pnpm dev:browser` will crash without them).
- **WebKitGTK scroll trap**: any new `overflow-y: auto` container in the sidebar or modal must be added to `useScrollEnhance.ts:9` `SCROLLABLE` selector or wheel scroll will misroute to a parent. Project rule per CLAUDE.md.
- **Modal tech debt**: ≥8 modals duplicate the focus-trap + body-lock + portal logic. Extracting a shared `<Modal>` primitive is a Should-have, not a Must-have, but every new modal otherwise adds debt.

---

_Generated: 2026-04-08_
_Status: DRAFT — needs validation_
