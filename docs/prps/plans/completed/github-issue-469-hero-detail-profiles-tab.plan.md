# Plan: Hero Detail Profiles Tab — Two-Pane Editor with Card-Click Profile Switch (Issue #469, Phase 4)

## Summary

Build `components/library/HeroDetailProfilesTab.tsx`: a two-pane editor inside Hero Detail's Profiles tab. Left pane lists the game's profile cards (active pill, name, `{filename}.toml · {proton_version}` mono line, last-used + health pill, `+ New` CTA). Right pane flattens `ProfileIdentitySection → RuntimeSection → GameSection → MediaSection` into one scrollable column. Card click calls `useProfileContext().selectProfile(name)`; edits autosave through `persistProfileDraft` → `profile_save` with a 350ms debounce; the read-only `ProfilesPanel` in the panels switch is replaced.

## User Story

As a user, I want per-game profile editing inside Hero Detail — click a profile card, edit any field, autosave — so I don't need to navigate to `/profiles`.

## Problem → Solution

Today `case 'profiles':` in `HeroDetailPanels.tsx:443-444` renders `ProfilesPanel` — a read-only kv summary that short-circuits with "No active profile loaded in the editor for this game" whenever the singleton `ProfileContext` points at a different profile (`HeroDetailPanels.tsx:316-333`). → Replace it with a live two-pane editor: the tab aligns the singleton to the detail card on mount, switches it on card click, binds the existing prop-driven `profile-sections/*` to `useProfileContext().profile`/`.updateProfile`, and persists drafts via a new debounced `persistProfileDraft` autosave effect. `GameDetail` is rewired so hero pills and the command preview follow the singleton when it points at one of this game's profiles (≤200ms ripple).

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 4 — Hero Detail Profiles tab (two-pane editor) — GitHub issue #469
- **Estimated Files**: 9 (3 new, 6 updated)

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | —          | 3              |
| B2    | 2.1           | B1         | 1              |
| B3    | 3.1, 3.2      | B2         | 2              |
| B4    | 4.1           | B3         | 1              |

- **Total tasks**: 7
- **Total batches**: 4
- **Max parallel width**: 3

---

## UX Design

### Before

```
┌─ Hero Detail ── [Overview][Profiles][Launch][Trainer][History][Compat] ─┐
│ Profiles tab:                                                           │
│   ┌─ Active profile ────────────────────────────────────────────────┐  │
│   │ "No active profile loaded in the editor for this game."         │  │
│   │ (or read-only Name / Prefix / Proton kv rows)                   │  │
│   └──────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

### After

```
┌─ Hero Detail ── [Overview][Profiles][Launch][Trainer][History][Compat] ─┐
│ Profiles tab:                                                           │
│ ┌─ Cards (left) ──────┐ ┌─ Editor (right, scrolls) ──────────────────┐ │
│ │ ✓ rdr2-enhanced     │ │ Identity                                   │ │
│ │   [Active] [✓ ok]   │ │   Profile name / Game name                 │ │
│ │   rdr2-enhanced.toml│ │ ───────────────────────────────────────────│ │
│ │   · GE-Proton9-15   │ │ Runtime                                    │ │
│ │   last used 2 days  │ │   Prefix / Proton picker / working dir …   │ │
│ ├─────────────────────┤ │ ───────────────────────────────────────────│ │
│ │   rdr2-stream       │ │ Game                                       │ │
│ │   rdr2-stream.toml… │ │   Executable path / args …                 │ │
│ ├─────────────────────┤ │ ───────────────────────────────────────────│ │
│ │ [+ New]             │ │ Media (cover / portrait / background art)  │ │
│ └─────────────────────┘ └────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────┘
(collapses to one column — cards above editor — below 720px / deck)
```

### Interaction Changes

| Touchpoint           | Before                                     | After                                                                      | Notes                                             |
| -------------------- | ------------------------------------------ | -------------------------------------------------------------------------- | ------------------------------------------------- |
| Profiles tab content | Read-only kv summary or short-circuit text | Two-pane live editor                                                       | Replaces `ProfilesPanel`                          |
| Profile switch       | Navigate to `/profiles`, use dropdown      | Click card in left list → `selectProfile(name)`                            | Hero pills + command preview update ≤200ms        |
| Field edit           | `/profiles` + explicit Save button         | Edit in place → autosave 350ms after draft change                          | Net-new debounced `persistProfileDraft` effect    |
| New profile          | `/profiles` → New Profile button           | `+ New` CTA in left list opens existing `OnboardingWizard` (mode `create`) | Same wizard component, locally mounted            |
| Trainer tab          | Read-only view                             | **Unchanged** — read-only view stays                                       | Q4 resolution: trainer editor deferred (see #478) |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                              | Lines                                              | Why                                                                                                                                                                        |
| -------------- | --------------------------------------------------------------------------------- | -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                | 18-40, 316-360, 405-504                            | Panel contract (Phase 1 channels at 34-39), `ProfilesPanel` to delete, switch case to replace                                                                              |
| P0 (critical)  | `src/crosshook-native/src/components/library/GameDetail.tsx`                      | 42-43, 90-156                                      | `useGameDetailsProfile` read pipeline, preview wiring, `TODO(phase-4)` panelProps stubs                                                                                    |
| P0 (critical)  | `src/crosshook-native/src/components/ProfileSubTabs.tsx`                          | 184-233                                            | How the four sections are composed today (props threaded per section) — flatten WITHOUT Tabs                                                                               |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/ProfileIdentitySection.tsx` | 6-16                                               | Required props incl. `onProfileNameChange`, optional `profiles`/`profileExists`                                                                                            |
| P0 (critical)  | `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx`         | 24-36                                              | Heaviest props payload: `protonInstalls`, `protonInstallsError`, `launchMethod`                                                                                            |
| P0 (critical)  | `src/crosshook-native/src/hooks/useProfile.ts`                                    | 41-120, 231-240                                    | `UseProfileResult`: `selectProfile`, sync `updateProfile`, `persistProfileDraft`, `setProfileName`, `profiles`, editor-safety note (no `collectionId` from editor callers) |
| P0 (critical)  | `src/crosshook-native/src/hooks/profile/useProfileCrud.ts`                        | 123-173, 266-301                                   | `loadProfile` side effects (`syncProfileMetadata`), `persistProfileDraft` `{ok}` result contract                                                                           |
| P1 (important) | `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts`       | 106-153                                            | Canonical debounce-effect shape: timer ref, `cancelled` cleanup, error status `{tone,label,detail}`                                                                        |
| P1 (important) | `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`          | 44-51, 72-87                                       | `latestProfileNameRef` race guard, timer cleanup discards (does not flush) pending writes                                                                                  |
| P1 (important) | `src/crosshook-native/src/hooks/profile/constants.ts`                             | 1-2                                                | `launchOptimizationsAutosaveDelayMs = 350` — reuse this constant for the general-field autosave                                                                            |
| P1 (important) | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                              | 8-9                                                | `SCROLL_ENHANCE_SELECTORS` — new scroll container class MUST be appended here                                                                                              |
| P1 (important) | `src/crosshook-native/src/styles/hero-detail.css`                                 | 68-76, 131-136, 182-191, 223-251, 302-305, 491-498 | Pill / two-col grid / scroll body / card section / mono / deck collapse patterns                                                                                           |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx` | 3-11, 145-169, 245-275                             | Render factory, Phase-1 defaults, the `mode="profiles"` no-op test whose assertions break                                                                                  |
| P1 (important) | `src/crosshook-native/src/components/__tests__/OnboardingWizard.test.tsx`         | 9-30, 99-113                                       | `vi.mock('@/context/ProfileContext')` idiom giving direct `persistProfileDraft`/`selectProfile` spies                                                                      |
| P2 (reference) | `src/crosshook-native/src/components/pages/profiles/ProfilesOverlays.tsx`         | 223-230                                            | `OnboardingWizard` mount: `open`/`mode`/`onComplete`/`onDismiss`                                                                                                           |
| P2 (reference) | `src/crosshook-native/src/components/HealthBadge.tsx`                             | 62-71                                              | Canonical health pill component — reuse, don't re-invent                                                                                                                   |
| P2 (reference) | `src/crosshook-native/src/hooks/useProtonInstalls.ts`                             | 6-45                                               | Standalone hook supplying `protonInstalls`/`error` for `RuntimeSection`                                                                                                    |
| P2 (reference) | `src/crosshook-native/src/hooks/useProfileSummaries.ts`                           | 6-37                                               | `useProfileSummaries(profiles, collectionId?)` → `ProfileSummary[]` via `profile_list_summaries`                                                                           |
| P2 (reference) | `src/crosshook-native/src/hooks/useLaunchHistoryForProfile.ts`                    | all                                                | Per-profile launch history (`started_at` → last-used)                                                                                                                      |
| P2 (reference) | `src/crosshook-native/src/utils/format.ts`                                        | 1-18                                               | `formatRelativeTime` for `last used {duration}`                                                                                                                            |
| P2 (reference) | `src/crosshook-native/src/components/library/LibraryCard.tsx`                     | 74-76, 91-135                                      | Clickable card a11y (tabIndex, Enter guard, `--selected` class composition)                                                                                                |

## External Documentation

No external research needed — purely internal patterns (React composition, existing IPC surfaces, existing hooks).

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION (components + CSS)

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailPanels.tsx:18,405
export interface HeroDetailPanelsProps { ... }
export function HeroDetailPanels({ ... }: HeroDetailPanelsProps) { ... }
// CSS: BEM-like crosshook-hero-detail__* — new classes: crosshook-hero-detail__profiles-*
```

### SYNC_UPDATER_SHAPE (what the four sections need)

```ts
// SOURCE: src/crosshook-native/src/components/profile-sections/ProfileIdentitySection.tsx:10
onUpdateProfile: (updater: (current: GameProfile) => GameProfile) => void;
// Matches useProfileContext().updateProfile (hooks/useProfile.ts:80) — sync, sets draft + dirty.
// Does NOT match HeroDetailPanelsProps.updateProfile (async draft) — do not pass that to sections.
```

### AUTOSAVE_DEBOUNCE_EFFECT (timer + cancelled cleanup)

```ts
// SOURCE: src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts:106-153 (shape)
const trimmedName = profileName.trim();
let cancelled = false;
timerRef.current = setTimeout(() => {
  if (cancelled) return; /* persist */
}, launchOptimizationsAutosaveDelayMs);
return () => {
  cancelled = true;
  clearTimeout(timerRef.current);
};
```

### RACE_GUARD (stale-profile check at fire time)

```ts
// SOURCE: src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts:73-75
if (latestProfileNameRef.current !== scheduledProfileName) {
  return;
}
```

### ERROR_HANDLING (persistProfileDraft result + status tone)

```ts
// SOURCE: src/crosshook-native/src/hooks/profile/useProfileCrud.ts:266-301 (contract)
const result = await persistProfileDraft(name, draft); // never throws
// {ok: true} | {ok: false, error: string}; on failure also sets context error.
// Status display: {tone: 'idle'|'saving'|'success'|'warning'|'error', label, detail}
// rendered as crosshook-launch-autosave-chip--${tone} with aria-live="polite" (LaunchSubTabs.tsx:116-124)
```

### LOAD_STATE_GUARD (three-state panel rendering)

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailPanels.tsx:429-432
{loadState === 'loading' ? <p className="crosshook-hero-detail__muted">Loading profile details…</p> : null}
{loadState === 'error' ? <p className="crosshook-hero-detail__warn">{profileError ?? 'Failed to load profile.'}</p> : null}
{profile && loadState === 'ready' ? ( /* content */ ) : null}
```

### TEST_STRUCTURE (render factory + overrides)

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx:145-169
function renderHeroDetailPanels(overrides: Partial<HeroDetailPanelsProps> = {}) {
  const props = { mode: 'launch-options', /* full defaults */, ...overrides };
  return render(<HeroDetailPanels {...props} />);
}
```

### CONTEXT_MOCK (direct spies for selectProfile/persistProfileDraft)

```tsx
// SOURCE: src/crosshook-native/src/components/__tests__/OnboardingWizard.test.tsx:9,99-113
vi.mock('@/context/ProfileContext', () => ({ useProfileContext: () => useProfileContextMock() }));
useProfileContextMock.mockReturnValue({
  persistProfileDraft: vi.fn().mockResolvedValue({ ok: true }),
  selectProfile: vi.fn().mockResolvedValue(undefined), updateProfile: vi.fn(), ...
});
```

### AUTOSAVE_TIMING_TEST (real timers, not fake)

```ts
// SOURCE: src/crosshook-native/src/components/pages/__tests__/LaunchRoute.test.tsx:223-253
fireEvent.blur(valueInput);
await new Promise((resolve) => setTimeout(resolve, 500)); // clear debounce window
expect(profileSaveSpy).toHaveBeenCalled(); // global afterEach runs vi.useRealTimers()
```

### CARD_A11Y (clickable list item)

```tsx
// SOURCE: src/crosshook-native/src/components/library/LibraryCard.tsx:91-135,74-76
// <li> with tabIndex={0}; Enter guard: !e.shiftKey && e.target === e.currentTarget → preventDefault + open;
// inner buttons stopPropagation; selected class composed via
['crosshook-library-card', isSelected && 'crosshook-library-card--selected'].filter(Boolean).join(' ');
```

### SCROLL_ENHANCE_REGISTRATION (mandatory for new scroll containers)

```ts
// SOURCE: src/crosshook-native/src/hooks/useScrollEnhance.ts:8-9
// Append new class to the comma-separated SCROLL_ENHANCE_SELECTORS string, e.g.:
// '..., .crosshook-install-page-tabs__panel-inner, .crosshook-hero-detail__profiles-editor'
// CSS pair (hero-detail.css:182-191): overflow-y: auto; overscroll-behavior: contain;
```

### TWO_COL_GRID_COLLAPSE (deck responsive)

```css
/* SOURCE: src/crosshook-native/src/styles/hero-detail.css:131-136,491-498 */
.crosshook-hero-detail__layout {
  display: grid;
  grid-template-columns: minmax(120px, 168px) minmax(0, 1fr);
  gap: 20px;
}
@media (max-width: 720px) {
  .crosshook-hero-detail__layout {
    grid-template-columns: 1fr;
  }
}
```

---

## Files to Change

| File                                                                                   | Action | Justification                                                                                   |
| -------------------------------------------------------------------------------------- | ------ | ----------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/hooks/useProfileCardMeta.ts`                                 | CREATE | Per-card proton label + last-used metadata (profile_load + launch history, 3–5 profiles/game)   |
| `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx`                | CREATE | The two-pane editor (~350 lines, under 500-line soft cap)                                       |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` | CREATE | Autosave-timing + card-click tests (separate file — needs `vi.mock` of ProfileContext)          |
| `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                     | UPDATE | Replace `case 'profiles':` with new tab; delete dead `ProfilesPanel`                            |
| `src/crosshook-native/src/components/library/GameDetail.tsx`                           | UPDATE | Wire `TODO(phase-4)` panelProps (`updateProfile`, `profileList`); singleton-aware pills/preview |
| `src/crosshook-native/src/styles/hero-detail.css`                                      | UPDATE | New `crosshook-hero-detail__profiles-*` classes + deck collapse                                 |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                                   | UPDATE | Register the new right-pane scroll container class                                              |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`      | UPDATE | `mode="profiles"` no-op describe asserts old short-circuit text — rewrite for new tab           |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`            | UPDATE | Phase-1 placeholder assertions (`updateProfile: undefined`, `profileList: undefined`) change    |

## NOT Building

- **Trainer section in the flattened editor** — Q4 resolution: the standalone Trainer tab (`HeroDetailPanels.tsx` `case 'trainer':`, currently lines 460-493) stays read-only and untouched. Trainer editor upgrade is a follow-up (tracker #478).
- **`ProfileSubTabs` mounting** — its Radix tab nesting fights Hero Detail's tabs. Sections are stacked flat.
- **Built-in pill** — no `is_builtin` flag exists anywhere (TS types, `ProfileSummary`, Rust backend — confirmed) and no name-prefix convention exists. Deferred per PRD ("derive from name prefix or defer"). Do not invent a heuristic.
- **`description` field** — no `description` on `GameProfile`/`ProfileSummary`. Cards render `summary.gameName` as the secondary line instead. Adding a schema field is backend scope this issue excludes.
- **Environment variables / pre-post hooks sections** — those live in the Launch tab (Phase 5 #470 / Phase 6).
- **Duplicate-profile action in the editor header** — PRD "Could"; out of this phase.
- **`onSetActiveTab` wiring** — `TODO(phase-7)` in GameDetail stays `undefined` (Overview deep-links are Phase 7).
- **Route deletion / nav rewiring / sidebar changes** — Phases 2, 8–10.
- **Card-list virtualization** — real-world profile count is 3–5 per game; don't pre-optimize.
- **New npm dependencies** — none needed (`@radix-ui/react-tabs` already present; everything else is composition).

---

## Step-by-Step Tasks

### Task 1.1: Create `useProfileCardMeta` hook — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/hooks/useProfileCardMeta.ts` — given `profileNames: string[]`, return `Record<string, { protonLabel: string | null; lastUsedLabel: string | null }>` plus `loading`.
- **IMPLEMENT**: For each name (callers pass 3–5), fetch `callCommand<SerializedGameProfile>('profile_load', { name })` and `callCommand<LaunchHistoryEntry[]>('list_launch_history_for_profile', { profileName: name, limit: 1 })` in parallel (`Promise.all`); derive `protonLabel` from the basename of `profile.runtime.proton_path || profile.steam.proton_path` (null when both empty), `lastUsedLabel` from `formatRelativeTime(entries[0]?.started_at)` (null when no history). Guard stale results with a request-counter ref exactly like `useGameDetailsProfile.ts:47-64`. Per-name failures degrade to `null` fields (cards render without the mono/last-used line) — do not fail the whole map.
- **MIRROR**: Request-counter stale guard from `hooks/useGameDetailsProfile.ts:47-64`; hook shape (options/result interfaces, `normalizeLoadError`) from `hooks/useProtonInstalls.ts:6-45`.
- **IMPORTS**: `callCommand` from `@/lib/ipc`; `normalizeSerializedGameProfile` from the same module `useGameDetailsProfile` uses; `formatRelativeTime` from `@/utils/format`; types from `@/types/profile`, `@/types/library`.
- **GOTCHA**: Do NOT use `useProfileContext` here — this hook reads via the read-only `profile_load` path; it must not disturb the singleton. Re-fetch when `profileNames` changes (join names into a stable key for the effect dep).
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck` passes; hook compiles with no `any`.

### Task 1.2: CSS classes + scroll-enhance registration — Depends on [none]

- **BATCH**: B1
- **ACTION**: Append `crosshook-hero-detail__profiles-*` classes to `src/crosshook-native/src/styles/hero-detail.css` (before the existing `@media (max-width: 720px)` block) and register the right-pane scroll class in `src/crosshook-native/src/hooks/useScrollEnhance.ts:9`.
- **IMPLEMENT**: Classes: `__profiles` (two-col grid `minmax(220px, 280px) minmax(0, 1fr)`, gap 20px, mirroring `__layout` at hero-detail.css:131-136), `__profiles-cards` (card list column), `__profiles-card` (+ `--selected` modifier mirroring `crosshook-library-card--selected` accent outline, library.css:145-160), `__profiles-card-meta` (mono line — compose with existing `__mono`), `__profiles-editor` (`overflow-y: auto; overscroll-behavior: contain; min-height: 0;` like `__body` at hero-detail.css:182-191), `__profiles-cta` (the `+ New` button row). Add `@media (max-width: 720px) { .crosshook-hero-detail__profiles { grid-template-columns: 1fr; } }`. In `useScrollEnhance.ts`, append `, .crosshook-hero-detail__profiles-editor` to `SCROLL_ENHANCE_SELECTORS`.
- **MIRROR**: TWO_COL_GRID_COLLAPSE and SCROLL_ENHANCE_REGISTRATION patterns above; pill styling reuses existing `crosshook-hero-detail__pill` (hero-detail.css:68-76) and `crosshook-status-chip--success` — no new pill CSS unless a checkmark spacing rule is needed.
- **IMPORTS**: none (CSS + one string edit).
- **GOTCHA**: Skipping the `SCROLL_ENHANCE_SELECTORS` registration causes WebKitGTK dual-scroll jank (enhanced scroll targets a parent) — this is a CLAUDE.md MUST. Keep CSS variables (`--crosshook-color-border`, `--crosshook-radius-md`, `--crosshook-font-mono`, `--crosshook-divider-color`) — no hardcoded colors.
- **VALIDATE**: `cd src/crosshook-native && npm run lint` (biome) passes; grep confirms the selector: `grep -n "profiles-editor" src/crosshook-native/src/hooks/useScrollEnhance.ts`.

### Task 1.3: GameDetail wiring — panelProps channels + singleton-aware pills/preview — Depends on [none]

- **BATCH**: B1
- **ACTION**: In `src/crosshook-native/src/components/library/GameDetail.tsx`, replace the `TODO(phase-4)` stubs (lines ~133-138) with real wiring and make hero pills + command preview follow the singleton when it points at one of this game's profiles. Update `__tests__/GameDetail.test.tsx` accordingly.
- **IMPLEMENT**: (1) Subscribe `useProfileContext()`. (2) Build `profileList`: `useProfileSummaries(ctx.profiles)` filtered to `s.gameName === summary.gameName` (memoized). (3) Compute `const singletonOwnsGame = gameProfileNames.has(ctx.selectedProfile.trim())`; derive the **display profile** used for pills/`launchRequest`/preview: `singletonOwnsGame && ctx.profile ? { profile: ctx.profile, name: ctx.selectedProfile, loadState: 'ready', error: null } : useGameDetailsProfile result` (fallback unchanged). (4) Wire `updateProfile: async (draft) => { const r = await ctx.persistProfileDraft(displayName, draft); if (!r.ok) throw new Error(r.error); }` (async-draft shape per `HeroDetailPanels.tsx:34-35` — consumed by Phase 5, not by the Profiles tab). (5) Leave `onSetActiveTab: undefined // TODO(phase-7)`. (6) Add all new values to the `panelProps` `useMemo` dep array. (7) Update `GameDetail.test.tsx:76-93` `objectContaining` assertions: `updateProfile: expect.any(Function)`, `profileList: expect.any(Array)`.
- **MIRROR**: Memoized `panelProps` construction already at `GameDetail.tsx:117-156`; the existing preview `useEffect` (`GameDetail.tsx:90-112`) re-runs automatically once `launchRequest` derives from the display profile.
- **IMPORTS**: `useProfileContext` from `@/context/ProfileContext`; `useProfileSummaries` from `@/hooks/useProfileSummaries`.
- **GOTCHA**: `useGameDetailsProfile` stays mounted as the fallback (cold open: singleton may point at an unrelated profile). The display-profile derivation is what delivers the ≤200ms ripple: card click → context state update → synchronous re-render of pills/preview — no IPC wait for the already-loaded `ctx.profile`. `GameDetail.test.tsx` renders without a real provider in places — check its mock setup (`vi.mock('@/lib/ipc', ...)` at lines 12-15) and wrap with `ProfileProvider` where the context subscription now requires it.
- **VALIDATE**: `cd src/crosshook-native && npm test -- GameDetail` passes; `npm run typecheck` passes.

### Task 2.1: Create `HeroDetailProfilesTab` component — Depends on [1.1, 1.2, 1.3]

- **BATCH**: B2
- **ACTION**: Create `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx` (~350 lines; if it drifts meaningfully past the 500-line soft cap, split the card list into `HeroDetailProfileCards.tsx`).
- **IMPLEMENT**:
  - **Props** (`HeroDetailProfilesTabProps`): `summary: LibraryCardData`, `profileList: ProfileSummary[] | undefined`, `loadState`, `profileError`, `healthByName?` (thread from GameDetail via panelProps if not already present). Everything mutable comes from `useProfileContext()` internally: `profile`, `profileName`, `selectedProfile`, `profiles`, `selectProfile`, `updateProfile` (sync), `setProfileName`, `persistProfileDraft`, `dirty`, `saving`, `error`, `launchMethod`, `steamClientInstallPath`.
  - **Alignment effect**: on mount / when `profileList` resolves — if `selectedProfile` is not one of this game's profile names, `void selectProfile(summary.name)` (NEVER pass `collectionId` — editor-safety rule, `useProfile.ts:71-78`).
  - **Left pane**: map `profileList` to cards — active checkmark `✓` + `Active` pill when `card.name === selectedProfile`; bold name; `{card.name}.toml · {protonLabel}` mono line and `last used {lastUsedLabel}` from `useProfileCardMeta(names)` (omit when null); `summary.gameName` secondary line; `<HealthBadge>` from `healthByName?.[card.name]`. Card = `<li tabIndex={0}>` with Enter-guard a11y per CARD_A11Y, `aria-current` on the active card. `+ New` CTA sets local `showWizard` state mounting `<OnboardingWizard open mode="create" onComplete={() => setShowWizard(false)} onDismiss={() => setShowWizard(false)} />` (ProfilesOverlays.tsx:223-230 precedent).
  - **Card click**: if `dirty && hasSavedSelectedProfile`, `void persistProfileDraft(selectedProfile, profile)` FIRST (flush — otherwise the pending debounce is silently discarded on switch), then `void selectProfile(card.name)`.
  - **Right pane** (`__profiles-editor` scroll column): when the singleton owns one of this game's profiles and `profile` is loaded, render heading (`profileName`) + autosave status chip, then `ProfileIdentitySection` (`profileName`, `onProfileNameChange={setProfileName}`, `profiles`, `profileExists`) → `RuntimeSection` (`protonInstalls`/`protonInstallsError` from `useProtonInstalls({ steamClientInstallPath })`, `launchMethod`) → `GameSection` (`launchMethod`) → `MediaSection` (`launchMethod`), all bound to `profile` + `onUpdateProfile={updateProfile}` (SYNC updater — see SYNC_UPDATER_SHAPE), separated by dividers (`border-top: 1px solid var(--crosshook-divider-color)`). Retain the LOAD_STATE_GUARD three-state pattern for the not-yet-aligned/loading/error states.
  - **General-field autosave effect** (net-new — this is the issue's core parity requirement): `hasSavedSelectedProfile = selectedProfile.trim().length > 0 && profiles.includes(selectedProfile.trim()) && profileName.trim() === selectedProfile.trim()`. Effect over `[profile, dirty, ...]`: if `dirty && hasSavedSelectedProfile`, schedule `setTimeout(..., launchOptimizationsAutosaveDelayMs)` (350, from `hooks/profile/constants.ts:2`) that re-checks a `latestProfileNameRef` (RACE_GUARD) then `void persistProfileDraft(scheduledName, latestProfileRef.current)`; cleanup clears the timer (AUTOSAVE_DEBOUNCE_EFFECT). `persistProfileDraft` resets `dirty` via its trailing `loadProfile`, which terminates the loop. Render the result as a `{tone,label,detail}` chip (`crosshook-launch-autosave-chip--${tone}`, `aria-live="polite"`).
- **MIRROR**: NAMING_CONVENTION (named export + `XxxProps` interface); section composition order/props from `ProfileSubTabs.tsx:184-233` (minus Tabs, minus RunnerMethod/Trainer/Gamescope/Export); ERROR_HANDLING for the status chip.
- **IMPORTS**: four sections from `@/components/profile-sections/*`; `useProfileContext`; `useProtonInstalls`; `useProfileCardMeta` (Task 1.1); `HealthBadge`; `OnboardingWizard` from `@/components/OnboardingWizard`; `launchOptimizationsAutosaveDelayMs` from `@/hooks/profile/constants`; types from `@/types/profile`, `@/types/library`.
- **GOTCHA**: (a) Do NOT pass the panel-prop `updateProfile` (async draft) to sections — incompatible shape; sections take the context sync updater. (b) The `profileName.trim() === selectedProfile.trim()` gate pauses autosave while the user is renaming in the Identity section — prevents autosave from creating files under a half-typed name (renames remain explicit, as on `/profiles`). (c) No `console.*` in components (repo policy — zero matches in `components/library`). (d) `selectProfile` side-effects `syncProfileMetadata` (writes last-used-profile settings) — intended singleton-alignment semantics, do not suppress.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm run lint` pass; component renders in `npm test -- HeroDetailProfilesTab` smoke (added in 3.2).

### Task 3.1: Swap the panels switch + fix existing panel tests — Depends on [2.1]

- **BATCH**: B3
- **ACTION**: In `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`: replace `case 'profiles': return <ProfilesPanel profileName={summary.name} />;` (lines 443-444) with `<HeroDetailProfilesTab summary={summary} profileList={profileList} loadState={loadState} profileError={profileError} />`; delete the now-dead `ProfilesPanel` function (lines 316-360) and its unused imports. Update `__tests__/HeroDetailPanels.test.tsx`.
- **IMPLEMENT**: The `no-op defaults` describe (test file lines ~245-275) renders `mode="profiles"` inside a real `ProfileProvider` and asserts `'No active profile loaded in the editor for this game.'` + `heading 'Active profile'` — both texts disappear. Rewrite to assert the new tab's empty-list state (e.g. left-pane empty-state text rendered when `profileList` is `undefined`/empty) and that rendering without `updateProfile` does not throw.
- **MIRROR**: TEST_STRUCTURE factory — extend `renderHeroDetailPanels` overrides rather than hand-rolling props.
- **IMPORTS**: `HeroDetailProfilesTab` from `./HeroDetailProfilesTab`.
- **GOTCHA**: Phase 5 (#470) edits the adjacent `case 'launch-options':` in this same switch — coordinate merge order (PRD: last-to-merge rebases). Do not touch `case 'trainer':` (lines 460-493). Keep this PR scoped to the `profiles` case + `ProfilesPanel` deletion.
- **VALIDATE**: `cd src/crosshook-native && npm test -- HeroDetailPanels` passes; `grep -n "function ProfilesPanel" src/crosshook-native/src/components/library/HeroDetailPanels.tsx` returns nothing.

### Task 3.2: Dedicated test file — autosave timing + card-click switch — Depends on [2.1]

- **BATCH**: B3
- **ACTION**: Create `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` with the issue's two required cases plus guards.
- **IMPLEMENT**: Use CONTEXT_MOCK (`vi.mock('@/context/ProfileContext')` — a module-level mock CANNOT live in `HeroDetailPanels.test.tsx`, whose existing describe needs the real provider; that's why this is a separate file). Also mock `@/lib/ipc` via the `mockCallCommand` idiom (`GameDetail.test.tsx:12-15`) for `useProfileCardMeta`/`useProtonInstalls` fetches. Cases: (1) **autosave**: render with a dirty-capable mock (configure `updateProfile` to flip `dirty: true` via `mockReturnValue` rerender), edit the Proton path field in `RuntimeSection`, assert `persistProfileDraft` NOT called immediately, then `await waitFor(() => expect(persistProfileDraft).toHaveBeenCalledWith(profileName, expect.any(Object)), { timeout: 1000 })` — real timers (AUTOSAVE_TIMING_TEST; global `afterEach` resets timers). (2) **card switch**: render with `profileList` of two summaries (inline `ProfileSummary` literals — no fixture exists), click the second card, assert `selectProfile` called with `'card2'`; rerender with the mock returning `profileName: 'card2'` and assert the editor heading updates. (3) **alignment**: render with singleton pointing at an unrelated profile → assert `selectProfile(summary.name)` fired on mount. (4) **a11y**: keyboard Enter on a card triggers selection.
- **MIRROR**: CONTEXT_MOCK; AUTOSAVE_TIMING_TEST; `userEvent.setup()` + `getByRole('heading', { name })` idioms from `HeroDetailPanels.test.tsx:173,195`.
- **IMPORTS**: `render`/`screen`/`waitFor` from `@testing-library/react`; `userEvent` from `@testing-library/user-event`; component under test.
- **GOTCHA**: No fake timers — repo autosave tests run real timers with `waitFor`/`setTimeout` waits (`vi.useRealTimers()` enforced in global `afterEach`, `test/setup.ts:253`). Spy `console.error` and assert not called (repo test convention).
- **VALIDATE**: `cd src/crosshook-native && npm test -- HeroDetailProfilesTab` — all cases green.

### Task 4.1: Full-suite validation + manual smoke — Depends on [3.1, 3.2]

- **BATCH**: B4
- **ACTION**: Run the repo's full frontend gates and manually verify the tab in dev mode.
- **IMPLEMENT**: `cd src/crosshook-native && npm run typecheck && npm test`; from repo root `./scripts/lint.sh --ts`. Manual: `./scripts/dev-native.sh --browser`, open a game's Hero Detail → Profiles tab → click a second card (pills + preview update), edit Proton version (status chip cycles saving → saved), `+ New` opens the wizard, narrow the window below 720px (panes stack), scroll the right pane (no dual-scroll jank).
- **MIRROR**: Validation Commands section below.
- **IMPORTS**: n/a.
- **GOTCHA**: No Rust changes in this phase — `cargo test` not required. Browser dev mode uses the mock layer; autosave round-trip through real IPC needs the full `./scripts/dev-native.sh` if discrepancies appear.
- **VALIDATE**: All commands exit 0; manual checklist below complete.

---

## Testing Strategy

### Unit Tests

| Test                        | Input                                     | Expected Output                                                             | Edge Case? |
| --------------------------- | ----------------------------------------- | --------------------------------------------------------------------------- | ---------- |
| Autosave debounce           | Edit Proton path field; wait              | `persistProfileDraft(profileName, draft)` called within 350ms (≤1s waitFor) | No         |
| No premature save           | Edit field; assert immediately            | `persistProfileDraft` NOT called before debounce elapses                    | Yes        |
| Card click switches profile | Click second card                         | `selectProfile('card2')` called; editor heading updates on context change   | No         |
| Mount alignment             | Singleton points at unrelated profile     | `selectProfile(summary.name)` fired once on mount                           | Yes        |
| Rename pauses autosave      | `profileName !== selectedProfile` + dirty | No `persistProfileDraft` call (gate closed)                                 | Yes        |
| Flush before switch         | Dirty draft, then card click              | `persistProfileDraft` (flush) called before `selectProfile`                 | Yes        |
| Empty profile list          | `profileList: []` / `undefined`           | Left pane renders empty state; no crash; `+ New` still available            | Yes        |
| Card meta degradation       | `profile_load` rejects for one card       | Card renders without mono/last-used line; others unaffected                 | Yes        |
| Keyboard a11y               | Focus card, press Enter                   | Selection fires (modifier-guarded)                                          | Yes        |
| Panels switch integration   | `mode="profiles"` via factory             | `HeroDetailProfilesTab` rendered; old short-circuit text gone               | No         |
| GameDetail panelProps       | Render GameDetail                         | `updateProfile: expect.any(Function)`, `profileList: expect.any(Array)`     | No         |

### Edge Cases Checklist

- [ ] Empty input — game with zero profiles (empty left pane + `+ New`)
- [ ] `selectProfile` load failure — context `error` surfaces; stale-data note (context does not null `profile` on failure, unlike `useGameDetailsProfile`)
- [ ] Concurrent access — card switch during in-flight 350ms debounce (flush-then-switch; ref guard kills stale timer)
- [ ] `persistProfileDraft` `{ok:false}` — error tone chip, no throw
- [ ] Profile with empty `proton_path` — mono line omits proton segment
- [ ] Rapid double card click — second `selectProfile` wins; request-counter guards prevent stale renders

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run typecheck
```

EXPECT: Zero type errors (runs both `tsc --noEmit` and `tsc -p tsconfig.test.json --noEmit`)

### Lint

```bash
./scripts/lint.sh --ts
```

EXPECT: Biome + tsc clean

### Unit Tests (affected area)

```bash
cd src/crosshook-native && npm test -- HeroDetailProfilesTab && npm test -- HeroDetailPanels && npm test -- GameDetail
```

EXPECT: All tests pass

### Full Test Suite

```bash
cd src/crosshook-native && npm test
```

EXPECT: No regressions (Vitest, happy-dom)

### Browser Validation

```bash
./scripts/dev-native.sh --browser
```

EXPECT: Profiles tab renders two-pane editor; card click updates hero pills + command preview; autosave chip cycles; collapses below 720px

### Manual Validation

- [ ] Open Hero Detail → Profiles tab: left cards + right editor visible (matches PRD screenshot 1 modulo deferred built-in pill/description)
- [ ] Click second card: `Active` pill moves, hero pills + command preview update perceptibly instantly (≤200ms)
- [ ] Edit Proton version: status chip shows saving → saved; reload profile shows persisted value
- [ ] Edit a field then immediately click another card: edit is not lost (flushed)
- [ ] Rename in Identity section: autosave pauses (no file created under partial name)
- [ ] `+ New` opens the profile-create wizard; completing/dismissing closes it
- [ ] Trainer tab unchanged (read-only)
- [ ] Right pane scrolls smoothly with no dual-scroll jank (scroll-enhance registered)
- [ ] Narrow window below 720px: panes stack vertically

---

## Acceptance Criteria

- [ ] All tasks completed
- [ ] All validation commands pass
- [ ] Visually matches PRD screenshot 1 (left cards + right editor)
- [ ] Autosave debounce parity: general fields 350ms via `persistProfileDraft` (env 400ms stays Phase 5 territory)
- [ ] Card click updates hero pills + command preview within 200ms (synchronous context-driven re-render)
- [ ] Trainer tab remains the existing read-only view — not mutated
- [ ] Tests: Proton edit → `persistProfileDraft` within 350ms; second-card click → `selectProfile('card2')` + heading update
- [ ] No type errors, no lint errors

## Completion Checklist

- [ ] Code follows discovered patterns (sync updater to sections; debounce-effect shape; load-state guard)
- [ ] Error handling matches codebase style (`{ok}` result, `{tone,label,detail}` chip, no thrown autosave errors)
- [ ] No `console.*` in components (repo convention — logging stays out)
- [ ] Tests follow repo patterns (real timers, context mock idiom, render factory, console.error spy)
- [ ] No hardcoded values (`launchOptimizationsAutosaveDelayMs` constant; CSS variables)
- [ ] New scroll container registered in `useScrollEnhance.ts`
- [ ] `ProfilesPanel` dead code removed
- [ ] No unnecessary scope additions (no built-in heuristic, no description field, no Trainer editor, no env section)
- [ ] Self-contained — no questions needed during implementation

## Risks

| Risk                                                                                                           | Likelihood | Impact | Mitigation                                                                                                                                                                                 |
| -------------------------------------------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Autosave feedback loop (`persistProfileDraft` → trailing `loadProfile` → new `profile` object re-fires effect) | M          | H      | Gate on `dirty` — `loadProfile` resets it; effect no-ops on the post-save render. If churn persists, add a `lastSavedJsonRef` signature check (mirrors `useProfileLaunchAutosaveEffects`). |
| Card switch drops in-flight debounced edit (cleanup discards timer)                                            | M          | M      | Explicit flush-before-switch in the card click handler (Task 2.1); test covers it.                                                                                                         |
| `updateProfile` shape confusion (async-draft panel channel vs sync section updater)                            | M          | H      | Plan separates them explicitly: sections ← context sync updater; panel channel ← Phase 5 consumer only. GOTCHA on Tasks 1.3 + 2.1.                                                         |
| Merge conflict with Phase 5 (#470) on `HeroDetailPanels.tsx` switch                                            | M          | L      | Both phases edit adjacent cases only; last-to-merge rebases (PRD guidance). Keep Task 3.1 diff minimal.                                                                                    |
| `GameDetail` context subscription causes extra re-renders of unrelated panels                                  | L          | L      | Values memoized; `panelProps` already a `useMemo`. Profile count per game is 3–5.                                                                                                          |
| `selectProfile` failure leaves stale editor data (context doesn't null `profile`)                              | L          | M      | Surface context `error` in the right-pane guard; documented divergence from `useGameDetailsProfile` behavior.                                                                              |
| Doubled writes: section-specific 350ms effects (gamescope etc.) + general autosave                             | M          | L      | Same data, idempotent IPC; section effects use dedicated `profile_save_*` commands, general effect uses `profile_save`. Acceptable per PRD parity goal.                                    |
| `HeroDetailPanels.test.tsx` no-op describe breaks on `ProfilesPanel` deletion                                  | H          | L      | Task 3.1 rewrites those assertions in the same change.                                                                                                                                     |

## Notes

- **Q4 resolution supersedes PRD prose**: PRD Phase-4 text lists `TrainerSection` in the flatten; issue #469 (updated 2026-04-23) removes it — Trainer stays a standalone read-only tab. The issue is authoritative.
- **PRD line numbers are stale**: `ProfilesPanel` is at `HeroDetailPanels.tsx:316-360` (PRD says 310-354); `profiles` case at 443-444; `trainer` case at 460-493. Verified against current `main`.
- **`/profiles` parity nuance**: today only env vars (400ms blur) and launch-opt/gamescope/mangohud (350ms) autosave; general fields use an explicit Save button (`useProfilesPageState.handleSave`). The issue's autosave mandate (per the PRD's resolved "Save model — autosave" decision) makes the 350ms general-field effect **net-new behavior**, not a port.
- **Deferred card elements**: built-in pill (no data source exists anywhere — confirmed) and description (no schema field). Both PRD-sanctioned deferrals; cards render `gameName` as the secondary line.
- **Persistence classification** (per CLAUDE.md): no new persisted data. Profile edits flow through the existing `profile_save` TOML path; card metadata is read-only derivation; selected card/tab state is runtime-only (singleton `ProfileContext` + component state). No SQLite, no migrations, fully offline.
- **Phase 1 (#466) landed** via PR #480 — the `updateProfile`/`profileList`/`onSetActiveTab` optional props and `data-testid` hooks this plan consumes are on `main`.
