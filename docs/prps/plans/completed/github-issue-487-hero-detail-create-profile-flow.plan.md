# Plan: Hero Detail Create-Profile Wizard and Creation Flow (Phase 5c)

**Issue**: [#487](https://github.com/yandy-r/crosshook/issues/487) — `Phase 5c: Hero Detail create-profile wizard and creation flow`
**Tracker**: Part of #478 (Hero Detail consolidation). Depends on #469 (done). Gates #473/#474/#475.
**Mode**: `--parallel` (dependency-batched tasks for `/ycc:prp-implement --parallel`), `--no-worktree`, `--enhanced` (7-researcher fan-out: api / business / tech / ux / security / practices / recommendations).
**Branch suggestion**: `feat/487-hero-detail-create-profile`

## Goal

Make the Hero Detail Profiles tab a complete, self-sufficient profile-creation surface so the legacy `/profiles` route can be deleted in Phase 10 without losing the creation path. Reuse the existing `OnboardingWizard` + `persistProfileDraft → profile_save` persistence path — no parallel save path, no schema change.

## Why

- `/profiles` currently owns the only _fully wired_ create flow. Phase 10 (#475) deletes that route; #487 must land first.
- **Critical framing (verified by all 7 researchers and confirmed first-hand)**: a `+ New` → `OnboardingWizard mode="create"` mount **already exists** in Hero Detail (`HeroProfileCardList.tsx:89-107`, landed in #469, kept as-is by #486 which explicitly deferred "create-flow hardening (prefill, post-create selection)" to #487). This issue is **hardening, not greenfield**: prefill, post-create refresh/selection, cancel-state restore, duplicate-name handling, empty-state CTA, and tests.

## Scope decisions

- **Reuse strategy = Option A** (consensus of practices + recommendations researchers, matching the repo's own #469 precedent): extend the existing `OnboardingWizard` with _optional, additive_ props (`createSeed`, widened `onComplete`) and keep the existing local mount in `HeroProfileCardList`. **No** extracted `CreateProfileFlow` component, **no** second form, **no** new hook layer.
- Legacy call sites (`AppShell.tsx:498` first-run, `ProfilesOverlays.tsx:223-230`) remain byte-for-byte unchanged — new props are optional and the widened `onComplete?: (createdName?: string) => void` signature is backward compatible with existing `() => void` callbacks.
- Executable pre-fill: `LibraryCardData` does **not** carry `executable_path` (`types/library.ts`). Seed it from the singleton context profile **only when** the currently selected profile is one of this game's cards (`singletonOwnsGame` — same-game guarantee); otherwise leave blank (degraded behavior per issue).
- Duplicate-name guard is **frontend-side, create-mode-only**: backend `profile_save` silently overwrites by name (verified: `store.rs:90-107` has no collision check; `AlreadyExists` only fires from `profile_rename`). Adding a backend create-time guard would change edit-save semantics — out of scope. The wizard pre-checks `profiles.includes(trimmedName)` before persisting in create mode.

## What we're NOT building (deferred ownership — do not double-plan)

| Not building                                                                       | Owner                                                                                                |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Removing/deactivating the legacy `/profiles` route or its files                    | #475 (Phase 10)                                                                                      |
| Rewiring `onNavigate('profiles'\|'launch')` callers / command palette              | #474 (Phase 9)                                                                                       |
| Shrinking `AppRoute` / `ROUTE_METADATA`                                            | #473 (Phase 8)                                                                                       |
| `onSetActiveTab` deep-link wiring (still TODO-stubbed at `GameDetail.tsx:195-196`) | #472 (Phase 7)                                                                                       |
| Profile TOML schema changes                                                        | Forbidden by issue                                                                                   |
| Backend duplicate-name rejection in `profile_save`                                 | Behavior change to all edit saves — explicitly preserved as-is (security report §1)                  |
| New global toast/notification system                                               | None exists (grep-verified); success feedback = selected-card visualization + wizard completed stage |
| New npm dependencies                                                               | Forbidden (issue + #486 precedent)                                                                   |

## Architecture (all patterns verified by research, with citations)

All frontend paths relative to `src/crosshook-native/src/`.

### The single persistence path (issue hard requirement)

`OnboardingWizard.handleComplete()` (`components/OnboardingWizard.tsx:326-336`) → `persistProfileDraft(trimmedName, profile)` (`hooks/profile/useProfileCrud.ts:266-301`) → `validateProfileForSave` → `normalizeProfileForSave` → `callCommand('profile_save', { name, data })` → `syncProfileMetadata` → `refreshProfiles()` → `loadProfile(trimmedName)`.

Key facts:

- `persistProfileDraft` **already** refreshes `ctx.profiles` and selects the new profile (`loadProfile` sets `selectedProfile`) on success. Post-create selection at the context level is free.
- Rust `profile_save` (`src-tauri/src/commands/profile/lifecycle.rs:99-212`) validates `steam_app_id` (`resolve.rs:44-58`), re-validates the profile name via `validate_name` (`crates/crosshook-core/src/profile/toml_store/utils.rs:50-78` — traversal-proof), applies creation defaults for new profiles, auto-imports art, and **emits `profiles-changed`** (`shared.rs:66-68`).
- **Review criterion**: zero `invoke('profile_save')`/`callCommand('profile_save'` outside `useProfileCrud.ts`. `grep -rn "profile_save" src/crosshook-native/src/components/library/profiles/` must stay empty.

### The load-bearing refresh gap (highest-risk finding)

`useProfileSummaries` (`hooks/useProfileSummaries.ts:12-38`) **ignores its `_profiles` argument and only fetches on mount / `collectionId` change**. It does not subscribe to `profiles-changed`. Hero Detail's card list derives from it (`GameDetail.tsx:49-54`: `summaries.filter(s => s.gameName === summary.gameName)`), so **after a successful create the new card does not appear** without a fix. Mirror the subscription pattern at `hooks/useProfile.ts:197-214` (`subscribeEvent('profiles-changed', …)` from `@/lib/events`).

### Wizard contract today (what changes, what must not break)

- Props (`OnboardingWizard.tsx:19-25`): `{ open, mode?: 'create'|'edit', onComplete: () => void, onDismiss: () => void, onOpenHostToolDashboard? }`. No prefill prop; `onComplete` carries no name.
- **Create-mode blank reset** (`OnboardingWizard.tsx:138-143`): `useEffect` on `[open, mode, selectProfile]` calls `void selectProfile('')` → `createEmptyProfile()`. Any seed must be applied **after** this reset inside the same effect, and the effect deps must NOT include the seed object identity (re-run would wipe user edits — memoize or ref-guard).
- Wizard state source is the **global singleton `ProfileContext`** (`OnboardingWizard.tsx:122-136`): `profileName`, `profile`, `saving`, `error`, `setProfileName`, `updateProfile`, `persistProfileDraft`, `selectProfile`. Context also exposes `profiles: string[]` and `profileExists` (`hooks/useProfile.ts:217,229`) — usable for the duplicate pre-check.
- Error surface: `profileError` banner `crosshook-error-banner crosshook-error-banner--section` with `role="alert"` (`OnboardingWizard.tsx:404-408`). On `!result.ok`, `handleComplete` early-returns and the wizard stays open — failed-save recovery already works.
- Save gating: `evaluateWizardRequiredFields` (`components/wizard/wizardValidation.ts:46-112`) — always requires profile name, game name, executable path, runner method; Steam App ID required only for `steam_applaunch`. Media/art **never** required (degraded-creation guarantee).
- Skip/ESC → `handleSkip()` → `dismiss()` (sets `onboarding_completed`, no profile write — BR-9, `hooks/useOnboarding.ts:119-126`) → `onDismiss()`. Backdrop click intentionally ignored — keep.
- Completed-stage copy (`OnboardingWizard.tsx:468-474`): "Head to the Launch page…" — wrong for Hero Detail context; make context-aware when a seed is present.
- Modal/portal/focus-trap/inert handling (`OnboardingWizard.tsx:157-223, 281-314`) is layout-independent (portal to `document.body`) — no changes needed.

### Cancel-state restore (shared-draft contamination)

Opening the wizard blanks the **shared** context draft. On dismiss, Hero Detail must restore the prior selection or the tab's editor reads an abandoned blank draft. `HeroProfileCardList` already receives `selectedTrimmed` — on dismiss, call `selectProfile(selectedTrimmed)` when non-empty (`loadProfile` clears `dirty`, `useProfileCrud.ts`). The tab's alignment effect (`HeroDetailProfilesTab.tsx:123-129`) is a safety net, not a guarantee.

### Pre-fill seed contract

New type (new file `components/wizard/profileCreateSeed.ts`):

```ts
export interface ProfileCreateSeed {
  suggestedName?: string; // → setProfileName (NOT a GameProfile field)
  gameName?: string; // → game.name
  steamAppId?: string; // → steam.app_id + runtime.steam_app_id (numeric only)
  executablePath?: string; // → game.executable_path
  coverArtPath?: string; // → game.custom_cover_art_path
  portraitArtPath?: string; // → game.custom_portrait_art_path
}
export function applyCreateSeed(profile: GameProfile, seed: ProfileCreateSeed): GameProfile;
```

| Seed field                         | Source in Hero Detail                                                                                               | Guard / fallback                                                                                                                                                                |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `gameName`                         | `summary.gameName` (`types/library.ts`)                                                                             | omit if blank                                                                                                                                                                   |
| `steamAppId`                       | `summary.steamAppId`                                                                                                | only if `/^\d+$/` (mirrors `GameDetail.tsx` numeric check; `validate_steam_app_id` accepts ≤12 ASCII digits); set `steam.enabled = true` when seeded                            |
| `coverArtPath` / `portraitArtPath` | `summary.customCoverArtPath` / `summary.customPortraitArtPath`                                                      | omit if undefined — **local managed-media paths only, never a remote URL** (security constraint §3)                                                                             |
| `executablePath`                   | `ctx.profile.game.executable_path` **iff** `cards.some(c => c.name === selectedTrimmed)` (singleton owns this game) | else blank — user browses manually                                                                                                                                              |
| `suggestedName`                    | none by default                                                                                                     | leave blank; pre-filling `gameName` as the profile name risks colliding with the existing same-game profile (silent-overwrite hazard) — duplicate guard covers typed collisions |

Security constraints to preserve (security report): seed only `GameProfile` string fields sourced from persisted profile data; never seed remote art URLs (`coverArtUrl`/`header_image` live in the render layer only); do not add executable existence checks to the shared save path; do not bypass `validate_name`/`validate_steam_app_id` with a parallel command.

### UI / empty-state / scroll rules

- Empty state today (`HeroProfileCardList.tsx:42-45`): bare `<p role="status">No profiles found for this game.</p>` + a generic secondary `+ New` at the bottom — the weakest UX moment. Add a primary "Create profile" CTA in the empty-state branch (precedent: `crosshook-panel role="status"` + `crosshook-button--primary` pattern, `ProfilesPage.tsx:178-214`). Both CTAs call the same open handler (DRY).
- Keep the persistent `+ New` as `crosshook-button--secondary`. Text `+ ` glyph convention — no SVG plus icon exists in `components/icons/` and none should be added.
- **No `SCROLL_ENHANCE_SELECTORS` change needed**: the wizard body reuses `.crosshook-modal__body` and the editor pane `.crosshook-hero-detail__profiles-editor` — both already registered (`hooks/useScrollEnhance.ts:8-11`). Only a _new_ `overflow-y:auto` class would require registration + `overscroll-behavior: contain`.
- a11y contract (enforced by `__tests__/a11y/modals.a11y.test.tsx:111-115` which already mounts `<OnboardingWizard open mode="create">`): `role="dialog"`, `aria-modal="true"`, resolvable `aria-labelledby` — structural changes must keep this green.

### File-size budget (~500-line soft cap)

| File                                                  | Now              | Action                                                                                                                                                                                           |
| ----------------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `components/OnboardingWizard.tsx`                     | **500 (at cap)** | Extract step helpers (`STAGE_TITLES`, `getVisibleStepNumber`, `getTotalVisibleSteps`, lines 49-79) to `components/wizard/wizardSteps.ts` **before** adding seed logic — pure move, ~30 lines out |
| `hooks/profile/useProfileCrud.ts`                     | 440              | Do not touch — seed logic lives in `profileCreateSeed.ts`                                                                                                                                        |
| `components/library/profiles/HeroProfileCardList.tsx` | 112              | +~50 lines OK                                                                                                                                                                                    |
| `components/library/HeroDetailProfilesTab.tsx`        | 248              | minimal/no change                                                                                                                                                                                |

### Test patterns (cookbook)

- Runner: Vitest + happy-dom; co-located `__tests__/` dirs. Fixtures: `@/test/fixtures` (`makeProfileDraft`, `makeLibraryCardData`); integration: `@/test/render` `renderWithMocks` with IPC handler map.
- Exemplars: `components/__tests__/OnboardingWizard.test.tsx` (mock idiom: `vi.mock` ProfileContext/useOnboarding/stage bodies; `buildOnboardingState()` factory), `components/library/__tests__/HeroDetailProfilesTab.test.tsx` (mocks `OnboardingWizard` as a stub dialog at lines 31-33 — extend the stub to capture props), `components/library/__tests__/HeroDetailLaunchTab.test.tsx`.
- **No test exists for `HeroProfileCardList`** — this plan adds one.

## Storage Boundary & Persistence (issue-mandated)

| Datum                                  | Classification                                                                                                                                    |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| New profile data                       | User-editable **TOML** via existing `profile_save` (identical shape to legacy-created profiles — guaranteed by reusing the same wizard + command) |
| Profile list metadata/cache            | Existing `profile_list` / `profile_list_summaries` mechanisms. **No new SQLite table**; metadata DB stays at schema v23, no migration             |
| Wizard open/step/error/selection state | Runtime-only React state (`showWizard` local; `useOnboarding` stage; `ProfileContext` singleton)                                                  |
| App settings                           | None new (`syncProfileMetadata` last-used writes are pre-existing side effects)                                                                   |

- **Backward compatibility**: no schema/shape change; profiles created here are indistinguishable from `/profiles`-created ones.
- **Offline**: creation is fully local (TOML write + local art import); Steam metadata/art is optional enrichment — `wizardValidation.ts` requires no art and Steam App ID only for `steam_applaunch`.
- **Degraded**: missing seed fields leave wizard inputs blank and manually editable; art source falls back to "None" (`GameSection.tsx`).
- **Visibility/editability**: on success the new profile appears as a selected card and is immediately editable in the tab's editor pane.

## Tasks

### Batch 1 — independent foundations (parallel, disjoint files)

#### Task 1: Extract wizard step helpers to keep `OnboardingWizard.tsx` under the soft cap

**Depends on**: []
**Files**: `src/crosshook-native/src/components/wizard/wizardSteps.ts` (new), `src/crosshook-native/src/components/OnboardingWizard.tsx`

1. Move `STAGE_TITLES`, `getVisibleStepNumber`, `getTotalVisibleSteps` (`OnboardingWizard.tsx:49-79`) into `components/wizard/wizardSteps.ts` (sibling of `wizardValidation.ts`). Export with unchanged signatures; import back into `OnboardingWizard.tsx`. Pure move — zero behavior change.
2. If any of these helpers are referenced by existing tests, update imports only.

**Validation**: `npm run typecheck`; `npm test -- OnboardingWizard` (all existing tests pass unchanged); `wc -l src/components/OnboardingWizard.tsx` ≤ ~475.

#### Task 2: Make `useProfileSummaries` react to `profiles-changed` (load-bearing refresh fix)

**Depends on**: []
**Files**: `src/crosshook-native/src/hooks/useProfileSummaries.ts`, `src/crosshook-native/src/hooks/__tests__/useProfileSummaries.test.ts` (new; create the `__tests__` dir if absent — follow the repo's co-located convention)

1. Subscribe to the `profiles-changed` Tauri event and re-run `fetchSummaries()`, mirroring the exact pattern at `hooks/useProfile.ts:197-214` (`subscribeEvent<string>('profiles-changed', …)` from `@/lib/events`, unlisten-on-cleanup with the same cancellation guard).
2. Keep the function signature `(profiles: string[], collectionId?)` intact (callers: `GameDetail.tsx:49`, legacy page) — do not rename or re-key on `_profiles`; the event subscription is the refresh mechanism.
3. Test: mock `@/lib/events` `subscribeEvent` + `@/lib/ipc` `callCommand`; assert (a) initial fetch, (b) emitting `profiles-changed` triggers a refetch and updated `summaries`, (c) unsubscribe on unmount.

**Validation**: `npm run typecheck`; `npm test -- useProfileSummaries`.

### Batch 2 — wizard contract (single file owner; after Task 1 frees the line budget)

#### Task 3: `ProfileCreateSeed` + seeded create-mode + name-carrying `onComplete` + create-mode duplicate guard

**Depends on**: [Task 1]
**Files**: `src/crosshook-native/src/components/wizard/profileCreateSeed.ts` (new), `src/crosshook-native/src/components/OnboardingWizard.tsx`, `src/crosshook-native/src/components/__tests__/OnboardingWizard.test.tsx` (extend), `src/crosshook-native/src/components/wizard/__tests__/profileCreateSeed.test.ts` (new)

1. **New module `profileCreateSeed.ts`**: `ProfileCreateSeed` interface (shape per Architecture §Pre-fill) and pure `applyCreateSeed(profile, seed): GameProfile` — shallow-merge over the blank draft: `game.name`, `game.executable_path`, `game.custom_cover_art_path`, `game.custom_portrait_art_path`, `steam.app_id` + `steam.enabled: true` + `runtime.steam_app_id` (only when `/^\d{1,12}$/`). Omit empty/undefined fields. Unit-test the mapping + numeric guard + empty-seed no-op.
2. **Wizard props** (`OnboardingWizard.tsx:19-25`): add `createSeed?: ProfileCreateSeed`; widen `onComplete: () => void` → `onComplete: (createdName?: string) => void` (assignment-compatible with all three existing call sites — verify `AppShell.tsx:498` and `ProfilesOverlays.tsx:223-230` compile **unchanged**).
3. **Seed application** in the create-mode reset effect (`OnboardingWizard.tsx:138-143`): after `selectProfile('')` resolves, apply `setProfileName(seed.suggestedName)` (if set) and `updateProfile((c) => applyCreateSeed(c, seed))`. Hold the seed in a `useRef` captured when `open` flips true; keep effect deps `[open, mode, selectProfile]` — the seed object's identity must NOT re-trigger the reset (would wipe user edits mid-wizard).
4. **Duplicate-name guard (create mode only)** in `handleComplete` (`OnboardingWizard.tsx:326-336`): read `profiles` from `useProfileContext()`; if `mode === 'create' && profiles.includes(trimmedName)`, set a local `nameCollisionError` state (`A profile named "{name}" already exists. Choose a different name.`) and return without persisting. Render it through the existing banner slot: `{(nameCollisionError ?? profileError) && <div className="crosshook-error-banner …" role="alert">…}`. Clear `nameCollisionError` when `profileName` changes. Do NOT touch edit mode (same-name save is a legitimate update) and do NOT change backend semantics.
5. **Name-carrying completion**: after successful persist, call `onComplete(trimmedName)`. The completed-stage footer's `onComplete` pass-through (`OnboardingWizard.tsx:492`) calls it with no arg — fine (param optional).
6. **Context-aware completed copy** (`OnboardingWizard.tsx:468-474`): when `createSeed` is present, replace "Head to the Launch page to start your game." with "It's now selected in this game's profile list." (keep "Profile saved successfully." lead).
7. **Tests** (extend `OnboardingWizard.test.tsx` using its existing `buildOnboardingState`/context-mock idiom): (a) seed applied after create-mode reset — `setProfileName`/`updateProfile` called with seeded values; (b) seed identity change while open does NOT re-reset; (c) create-mode duplicate name → banner shown, `persistProfileDraft` NOT called, recoverable after rename; (d) successful complete calls `onComplete('Trimmed Name')`; (e) `persistProfileDraft → {ok:false}` keeps wizard open, `onComplete` not called (existing behavior, now asserted); (f) legacy no-seed mount behaves exactly as before.

**Validation**: `npm run typecheck`; `npm test -- OnboardingWizard profileCreateSeed`; `wc -l src/components/OnboardingWizard.tsx` ≤ ~510 (soft cap; extract further only if a clean seam exists); a11y suite `npm test -- modals.a11y` stays green.

### Batch 3 — Hero Detail wiring (consumes Tasks 2+3)

#### Task 4: Seed, post-create selection, cancel restore, and empty-state CTA in `HeroProfileCardList`

**Depends on**: [Task 2, Task 3]
**Files**: `src/crosshook-native/src/components/library/profiles/HeroProfileCardList.tsx`, `src/crosshook-native/src/components/library/profiles/__tests__/HeroProfileCardList.test.tsx` (new), `src/crosshook-native/src/styles/theme.css` (only if the empty-state CTA needs a class not yet styled)

1. **Build the seed** in `HeroProfileCardList` from existing props + context: `useProfileContext()` for `profile` and `selectProfile`. Seed = `{ gameName: summary.gameName, steamAppId: summary.steamAppId (numeric only), coverArtPath: summary.customCoverArtPath, portraitArtPath: summary.customPortraitArtPath, executablePath: cards.some(c => c.name === selectedTrimmed) ? ctx.profile.game.executable_path : undefined }`. Memoize with `useMemo` keyed on those inputs. Keep the builder as a small exported pure function (e.g. `buildHeroCreateSeed(summary, cards, selectedTrimmed, contextProfile)`) co-located in this file or `profiles/` for direct unit testing — do not put it in `useProfileCrud.ts`.
2. **Wire callbacks** on the existing wizard mount (`HeroProfileCardList.tsx:100-107`):
   - `createSeed={seed}`
   - `onComplete={(createdName) => { setShowWizard(false); /* persistProfileDraft already selected it; belt-and-suspenders: */ if (createdName) void selectProfile(createdName); }}`
   - `onDismiss={() => { setShowWizard(false); if (selectedTrimmed) void selectProfile(selectedTrimmed); }}` — restores the prior selection and clears the abandoned blank draft (`loadProfile` resets `dirty`). When `selectedTrimmed` is empty (zero-profile game), do nothing extra — the tab's alignment effect handles it.
3. **Empty-state CTA**: in the `cards.length === 0` branch (`:42-45`), render a panel with the existing muted message plus a `crosshook-button crosshook-button--primary` button labeled `Create profile` that calls the same `setShowWizard(true)` handler. Keep the persistent secondary `+ New` button as-is. Native `<button type="button">` (a11y).
4. **No new scroll containers** — verify no new `overflow-y: auto` class is introduced; if one is, add it to `SCROLL_ENHANCE_SELECTORS` (`hooks/useScrollEnhance.ts`) + `overscroll-behavior: contain` (CLAUDE.md rule).
5. **Tests** (new `HeroProfileCardList.test.tsx`; mock `OnboardingWizard` as a prop-capturing stub like `HeroDetailProfilesTab.test.tsx:31-33`, mock `ProfileContext`, use `makeLibraryCardData`):
   - `+ New` opens wizard with `mode="create"` and the seed derived from `summary` (gameName, numeric appId, art paths);
   - executable seeded **only** when `selectedTrimmed` is one of this game's cards;
   - non-numeric `steamAppId` omitted from seed;
   - `onComplete('NewName')` → wizard closed + `selectProfile('NewName')`;
   - `onDismiss` → wizard closed + `selectProfile(selectedTrimmed)` restore; no restore call when `selectedTrimmed` is empty;
   - empty-state renders the primary `Create profile` CTA and it opens the wizard.

**Validation**: `npm run typecheck`; `npm test -- HeroProfileCardList`; `grep -rn "profile_save" src/crosshook-native/src/components/library/profiles/` → empty (no parallel save path).

### Batch 4 — integration proof + finalization

#### Task 5: Tab-level integration tests (create-without-navigation is the headline AC)

**Depends on**: [Task 4]
**Files**: `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` (extend)

1. Extend the existing `OnboardingWizard` stub (`:31-33`) to capture `createSeed`/`onComplete`/`onDismiss` props and expose trigger helpers.
2. Add cases using the existing `buildContextState`/`renderProfilesTab` harness:
   - **Creation without navigation**: click `+ New`, simulate wizard completion with a new name → wizard closes, no navigation callback fired, `selectProfile(newName)` invoked, tab still mounted (AC2/AC5-proxy at tab level; full card-list refresh is covered by Task 2's hook test + the context refresh in `persistProfileDraft`).
   - **Cancellation**: open + dismiss → prior selection restored, editor pane unchanged, no persist call (AC6).
   - **Seed correctness at tab level**: wizard received seed matching the tab's `summary` fixture (AC4).
3. Confirm the duplicate-name and save-failure flows are asserted at the wizard level (Task 3 tests) — do not duplicate them here beyond one smoke assertion that the wizard stub stays open on failure.

**Validation**: `npm run typecheck`; `npm test -- HeroDetailProfilesTab`.

#### Task 6: Full validation gate + docs touch-up

**Depends on**: [Task 1, Task 2, Task 3, Task 4, Task 5]
**Files**: none beyond fixes surfaced by validation

1. Run the full validation suite (below). Fix any regressions in touched files only.
2. Verify the three untouched wizard call sites still compile and behave: `AppShell.tsx:498`, `ProfilesOverlays.tsx:223-230` (legacy page keeps working until Phase 10), a11y modal suite.
3. Dependency guard: `git diff package.json package-lock.json` → empty.
4. PR: title `feat(library): hero detail create-profile wizard flow (#487)`-style Conventional Commit (squash-merge lands verbatim in CHANGELOG), body per `.github/pull_request_template.md`, link `Part of #478` + `Closes #487`, labels `type:feature`, `area:profiles`, `area:ui`, `area:onboarding`, `priority:high`.

**Validation**: full suite below.

## Batches

| Batch | Tasks           | Parallelism | Rationale                                                                                                 |
| ----- | --------------- | ----------- | --------------------------------------------------------------------------------------------------------- |
| 1     | Task 1, Task 2  | parallel    | Disjoint files (`OnboardingWizard.tsx`+new vs `useProfileSummaries.ts`+new); both independent foundations |
| 2     | Task 3          | solo        | Sole owner of `OnboardingWizard.tsx`; needs Task 1's line-budget extraction first                         |
| 3     | Task 4          | solo        | Consumes Task 3's new props and Task 2's refresh guarantee                                                |
| 4     | Task 5 → Task 6 | sequential  | Integration tests need Task 4; final gate needs everything                                                |

## Validation Commands

```bash
cd src/crosshook-native

# Static analysis
npm run typecheck
# EXPECT: zero errors (app + test tsconfigs)

# Focused tests
npm test -- OnboardingWizard profileCreateSeed useProfileSummaries HeroProfileCardList HeroDetailProfilesTab
# EXPECT: all pass, zero console.error

# a11y modal contract
npm test -- modals.a11y
# EXPECT: pass (wizard dialog contract unchanged)

# Full frontend suite
npm test
# EXPECT: all pass, no regressions

# Lint (repo root)
cd ../.. && ./scripts/lint.sh
# EXPECT: exit 0 on touched files

# No parallel save path (repo root)
grep -rn "profile_save" src/crosshook-native/src/components/library/profiles/
# EXPECT: no matches

# Dependency guard (repo root)
git diff --stat package.json src/crosshook-native/package.json src/crosshook-native/package-lock.json
# EXPECT: empty
```

No Rust changes are planned → `cargo test` not required; if any `crates/`/`src-tauri/` file is touched, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

## Acceptance Criteria (from #487) → coverage map

| AC                                                         | Covered by                                                                                                 |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Create-profile affordance on Hero Detail Profiles tab      | Pre-existing `+ New` + Task 4 empty-state primary CTA                                                      |
| Create flow works without navigating to `/profiles`        | Task 5 integration test (headline)                                                                         |
| Existing wizard/save path reused, not duplicated           | Option A architecture; Task 4/6 grep guard; zero new save calls                                            |
| Game context prefilled where available                     | Task 3 seed mechanism + Task 4 seed builder (name, appId, art always; exe when singleton owns game)        |
| New profile appears in list and becomes selected/editable  | Task 2 (`profiles-changed` refresh) + `persistProfileDraft`'s built-in `loadProfile` + Task 4 `onComplete` |
| Cancellation returns to same state without dirtying        | Task 4 `onDismiss` restore + Task 5 test                                                                   |
| Duplicate name + validation errors visible and recoverable | Task 3 create-mode guard + existing `profileError` banner + tests                                          |
| Tests cover success, cancellation, save/validation failure | Tasks 2-5 test additions                                                                                   |
| `npm run typecheck`, focused + full tests pass             | Task 6 gate                                                                                                |

## Risks & Mitigations

| Risk                                                                                                          | Mitigation                                                                                                                                                                                       |
| ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **`useProfileSummaries` staleness** — new card never appears (would fail the headline AC)                     | Task 2 is a standalone, first-batch fix with its own test; mirrors the proven `useProfile.ts:197-214` subscription pattern                                                                       |
| **Seed wiped by create-mode blank reset** (`OnboardingWizard.tsx:138-143`)                                    | Apply seed inside the reset effect after `selectProfile('')`; `useRef`-captured seed; effect deps unchanged; Task 3 test (b) asserts no re-reset                                                 |
| **Silent overwrite on duplicate name** (backend `profile_save` has no create-time collision guard — verified) | Frontend create-mode pre-check against `ctx.profiles` (Task 3); never seed `suggestedName` from `gameName`                                                                                       |
| **Cancel leaves shared context draft dirty/blank**                                                            | Task 4 `onDismiss` restore via `selectProfile(selectedTrimmed)`; tab alignment effect as backstop; Task 5 cancellation test                                                                      |
| **`OnboardingWizard.tsx` breaches 500-line cap**                                                              | Task 1 extracts ~30 lines first; seed logic lives in `profileCreateSeed.ts`, not the component; cap is soft per repo lessons if marginally over                                                  |
| **Legacy `/profiles` or first-run onboarding regression**                                                     | All new props optional; widened callback assignment-compatible; Task 6 verifies both call sites; `dismiss_onboarding` re-set is idempotent (existing behavior on every wizard close — unchanged) |
| **Remote art URL leaking into TOML via seed**                                                                 | Seed sources only `summary.custom*ArtPath` (managed-media local paths); render-layer `coverArtUrl` is explicitly excluded (security report §3)                                                   |

## References

- Issue: https://github.com/yandy-r/crosshook/issues/487 (tracker #478)
- Prior plans: `docs/prps/plans/completed/github-issue-469-hero-detail-profiles-tab.plan.md` (wizard-mount precedent, lines 81/320), `docs/prps/plans/completed/github-issue-486-hero-detail-launch-profile-parity.plan.md` (deferral table line 35; single-persistence-path contract lines 127-135)
- Key contracts verified first-hand during planning: `OnboardingWizard.tsx:19-25,138-143,326-336,404-408,468-474` (500 lines exactly); `useProfileSummaries.ts:12-38` (ignores `_profiles`, no event subscription); `HeroProfileCardList.tsx:35,42-45,89-107` (existing CTA + unwired mount); `useProfileCrud.ts:266-301` (`persistProfileDraft` refresh+select); `useProfile.ts:197-214` (`profiles-changed` pattern), `:217,229` (`profiles`/`profileExists` exposure)
- Rust (read-only for this plan): `src-tauri/src/commands/profile/lifecycle.rs:99-212` (`profile_save`), `crates/crosshook-core/src/profile/toml_store/{store.rs,utils.rs,error.rs}`, `crates/crosshook-core/src/profile/creation_defaults.rs`, `crates/crosshook-core/src/profile/models/resolve.rs:44-58`
