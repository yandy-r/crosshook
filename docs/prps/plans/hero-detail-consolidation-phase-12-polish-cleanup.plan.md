# Plan: Phase 12 — Polish, Design-Token Docs, Dead-Asset Cleanup (Issue #477)

## Summary

Close out the Hero Detail Consolidation PRD with its polish tail: document the five
command-preview token classes in `docs/internal-docs/design-tokens.md`, delete the
true orphan `LaunchPanel.tsx` and its transitively-dead `launch-panel/` modules, prune
the CSS selectors those files solely owned, extend the Steam Deck validation checklist
with a Hero Detail section at 1280×800, and verify ADR-0001 is untouched. Release-notes
copy lands via the squash-PR Conventional Commit title — no `CHANGELOG.md` edit.

**Critical research finding — the issue text is stale relative to the shipped code.**
Enhanced research (7 parallel researchers) established:

| Issue #477 names                                                 | Repo reality                                                                                                                                         | Plan action                         |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| `--cmd-token--env-key` etc. (CSS custom properties)              | Don't exist. Real names are BEM **classes** `.crosshook-hero-detail__command-token--{comment,env-key,value,binary,flag}` (`hero-detail.css:294-310`) | Document the real class names       |
| `.crosshook-profiles-page__body`, `.crosshook-launch-page__grid` | Already removed (Phase 10, PRs #495/#496) — zero matches                                                                                             | Grep guard only (already satisfied) |
| `.crosshook-launch-pipeline`                                     | **LIVE** — `LaunchPipeline.tsx` reused by `HeroLaunchGate.tsx:20,217`; smoke asserts it (`smoke.spec.ts:278-279,324`)                                | **Do NOT remove**                   |
| `ProfilesIcon`, `LaunchIcon`                                     | **LIVE** — `CommandPalette.tsx:12,14,101,103` category icons                                                                                         | **Do NOT remove**                   |
| _(not named in issue)_ `LaunchPanel.tsx`                         | **TRUE ORPHAN** — zero importers; cascades to 5 more dead modules                                                                                    | Delete the chain                    |

**User decision (recorded):** the PRD criterion "Profiles tab collapses to single-column
at 1280×800" is **stale** — the only collapse breakpoint is `@media (max-width: 720px)`
(`hero-detail.css:912`). At 1280×800 the layout stays two-column and fits without
horizontal overflow (already proven by `smoke.spec.ts:586-705`). The Deck checklist
validates **fit, not collapse**. No CSS behavior changes in this phase.

## User Story

As a maintainer, I want no dead assets, no stale docs, and a clean release note on the
next tag.

## Problem → Solution

- **Problem**: Phase 10/11 deleted the legacy `/profiles` and `/launch` routes but left
  one orphaned wrapper component tree (`LaunchPanel.tsx` + 5 transitively-dead modules)
  and ~30 CSS rules only that tree emitted. The new command-preview token classes are
  undocumented. The Steam Deck checklist has no Hero Detail section.
- **Solution**: One focused PR — two doc updates, one verified deletion cascade, one
  CSS prune, one verification gate. Grep-before-delete discipline throughout (the same
  pattern PR #496 used, per `docs/prps/plans/completed/github-issues-475-476-legacy-page-deletion-smoke-rewrite.plan.md:478`).

## Metadata

| Field             | Value                                                                                                                                                                               |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Complexity        | Low                                                                                                                                                                                 |
| Source PRD        | `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md` (Phase 12, line 447)                                                                                              |
| PRD Phase         | 12 of 12 (final)                                                                                                                                                                    |
| GitHub Issues     | Closes #477 · Part of #478                                                                                                                                                          |
| Estimated Files   | 12 (2 docs edit, 6 delete, 1 trim, 3 CSS edit)                                                                                                                                      |
| Research Dispatch | `--enhanced --parallel` — 7 standalone `ycc:prp-researcher` agents (api / business / tech / ux / security / practices / recommendations)                                            |
| Worktree Mode     | none (`--no-worktree`)                                                                                                                                                              |
| Confidence Score  | 9/10 — all deletions grep-proven by 3 independent researchers + orchestrator tiebreak greps; only residual risk is CSS line drift (mitigated: tasks grep, never trust line numbers) |

## Storage Boundary & Persistence

No persisted data is created, changed, or removed.

| Datum                                      | Classification                                                     |
| ------------------------------------------ | ------------------------------------------------------------------ |
| design-tokens / Deck-checklist doc updates | Repo docs (git only)                                               |
| Deleted TSX/CSS                            | Code only — no TOML settings, no SQLite metadata, no runtime state |

- **Migration/backward compatibility**: N/A — no schema, no settings keys.
- **Offline expectations**: unchanged.
- **Degraded behavior**: N/A.
- **User visibility/editability**: none — internal docs and dead code only.

## Batches

| Batch | Tasks         | Depends On         | Parallel Width |
| ----- | ------------- | ------------------ | -------------- |
| 1     | 1.1, 1.2, 1.3 | —                  | 3              |
| 2     | 2.1           | 1.3                | 1              |
| 3     | 3.1           | 1.1, 1.2, 1.3, 2.1 | 1              |

- **Total tasks**: 5 · **Total batches**: 3 · **Max parallel width**: 3
- **Same-file collision check**: PASS — Batch 1 file sets are disjoint
  (1.1 → `design-tokens.md`; 1.2 → `steam-deck-validation-checklist.md`;
  1.3 → `src/components/LaunchPanel.tsx` + `src/components/launch-panel/*`).
  Task 2.1 (CSS) is serialized after 1.3 so CSS grep proofs reflect post-deletion truth.
- **Green gate between batches**: `cd src/crosshook-native && npm run typecheck && npm test`

## UX Design

### Before / After

No user-visible UI change. The deleted component tree is unreachable (no route, no
importer). The two CSS files lose only rules whose sole emitters are deleted.

### Interaction Changes

| Surface                | Change                                  |
| ---------------------- | --------------------------------------- |
| Hero Detail (all tabs) | none                                    |
| Command palette        | none (`ProfilesIcon`/`LaunchIcon` kept) |
| Launch pipeline viz    | none (`LaunchPipeline` kept)            |

## Mandatory Reading

| Priority | File                                                                                         | Lines                      | Why                                                                                                                      |
| -------- | -------------------------------------------------------------------------------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| P0       | `docs/internal-docs/design-tokens.md`                                                        | 122–265                    | Lower "token family" block — `---`-delimited `##` sections; the new section's home and format                            |
| P0       | `src/crosshook-native/src/styles/hero-detail.css`                                            | 269–312                    | `__highlighted-command` (scrolls, `overflow-x: auto`) + the five `__command-token--*` rules to document                  |
| P0       | `src/crosshook-native/src/components/library/HighlightedCommandBlock.tsx`                    | 9, 17–19, 61–100           | `TokenTone` union = authoritative list of the five tones; `tokenClass()`; tone-assignment logic                          |
| P0       | `src/crosshook-native/src/components/LaunchPanel.tsx`                                        | all                        | The orphan root — its imports define the deletion cascade                                                                |
| P1       | `src/crosshook-native/src/components/launch-panel/helpers.tsx`                               | all                        | PARTIAL file — only `sortPatternMatchesBySeverity` (line 16) survives                                                    |
| P1       | `src/crosshook-native/src/components/library/launch/HeroLaunchGate.tsx`                      | 20–21, 184, 216–220        | The LIVE consumer that keeps `LaunchPipeline`, `LaunchPanelFeedback`, and `__runner-stack`/`__indicator-*` classes alive |
| P1       | `docs/internal-docs/steam-deck-validation-checklist.md`                                      | 62–113                     | Phase 13 grouped-checkbox format to mirror (incl. "Blocking issues" sign-off gate)                                       |
| P1       | `src/crosshook-native/src/styles/variables.css`                                              | 27–36, 141                 | Token values the command-token classes consume; `--crosshook-route-hero-launch-panel-min-height` to prune                |
| P2       | `docs/prps/plans/completed/github-issues-475-476-legacy-page-deletion-smoke-rewrite.plan.md` | 46–61, 252–348, 478–502    | Sibling plan: deletion discipline, grep-before-delete, PR conventions                                                    |
| P2       | `src/crosshook-native/tests/smoke.spec.ts`                                                   | 278–279, 324, 528, 586–705 | Locators that must survive CSS prune; existing 1280×800 no-overflow assertions                                           |
| P2       | `.git-cliff.toml`                                                                            | 27–61                      | Why the PR title must be `feat(ui)` to appear in the changelog                                                           |

## External Documentation

| Resource                   | URL                                            | Why                                                                                  |
| -------------------------- | ---------------------------------------------- | ------------------------------------------------------------------------------------ |
| Conventional Commits 1.0.0 | https://www.conventionalcommits.org/en/v1.0.0/ | PR title is the squash commit subject, validated by `.github/workflows/pr-title.yml` |
| git-cliff                  | https://git-cliff.org/docs/                    | Changelog generation at release prep (`scripts/prepare-release.sh`)                  |

## Patterns to Mirror

### DOCS_SECTION_FORMAT (design-tokens.md)

Standalone `##` section preceded by `---`, "used only here" caveat, three-column table —
verbatim precedent at `design-tokens.md:210-219`:

```markdown
## Command palette overlay tokens

Used only in `palette.css` for the overlay surface. Do not use elsewhere — the palette
intentionally uses a deeper dark than the standard `--crosshook-color-bg`.

| Token                                | Value                       | Usage                  |
| ------------------------------------ | --------------------------- | ---------------------- |
| `--crosshook-palette-border-on-dark` | `rgba(255, 255, 255, 0.08)` | Palette surface border |
```

### CHECKLIST_FORMAT (steam-deck-validation-checklist.md)

Phase 13 style (`steam-deck-validation-checklist.md:62-113`): `## Phase N — <scope>`
header stating the target ("Steam Deck native (1280×800, WebKitGTK, gamepad +
touchscreen)"), grouped `- [ ]` checkboxes under `###` subheadings, closing
`### Blocking issues (sign-off gate — must be zero)` group seeded with `- [ ] NONE`.

### DELETION_DISCIPLINE (PR #496 precedent)

1. Grep-prove zero importers immediately before each `git rm` (point-in-time proofs go
   stale; re-prove at execution).
2. Delete component + its solely-owned tests/mocks in the same task (here: none exist —
   verified, no vitest file references `LaunchPanel`).
3. CSS prune is a separate, later task; grep each class for surviving TSX emitters
   before deleting its rules. Never delete a class a smoke locator uses.
4. Green gate (`npm run typecheck && npm test`) after every batch.

### TEST_STRUCTURE

No new tests. `HighlightedCommandBlock.test.tsx:44-110` already asserts all five tone
classes; `HeroLaunchGate.test.tsx:111-116` mocks the two LIVE modules (`LaunchPipeline`,
`LaunchPanelFeedback`) — both mocks survive untouched.

## Files to Change

| Action | File                                                                            | Detail                                                                                              |
| ------ | ------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| EDIT   | `docs/internal-docs/design-tokens.md`                                           | New `## Command preview token classes` section (Task 1.1)                                           |
| EDIT   | `docs/internal-docs/steam-deck-validation-checklist.md`                         | New `## Phase 12 — Hero Detail at 1280×800` section (Task 1.2)                                      |
| DELETE | `src/crosshook-native/src/components/LaunchPanel.tsx`                           | Orphan root (218 lines, zero importers)                                                             |
| DELETE | `src/crosshook-native/src/components/launch-panel/LaunchPanelControls.tsx`      | Sole importer: LaunchPanel.tsx:12                                                                   |
| DELETE | `src/crosshook-native/src/components/launch-panel/LaunchPanelVersionStatus.tsx` | Sole importer: LaunchPanel.tsx:14                                                                   |
| DELETE | `src/crosshook-native/src/components/launch-panel/PreviewModal.tsx`             | Sole importer: LaunchPanel.tsx:15 (distinct from live `LauncherPreviewModal`/`ProfilePreviewModal`) |
| DELETE | `src/crosshook-native/src/components/launch-panel/focusTrap.ts`                 | Sole importer: launch-panel/PreviewModal.tsx:8 (cascade)                                            |
| DELETE | `src/crosshook-native/src/components/launch-panel/types.ts`                     | Sole importer: LaunchPanel.tsx:16,20                                                                |
| EDIT   | `src/crosshook-native/src/components/launch-panel/helpers.tsx`                  | Trim to `sortPatternMatchesBySeverity` only; drop now-unused line-3 import                          |
| EDIT   | `src/crosshook-native/src/styles/theme.css`                                     | Prune LaunchPanel-only selectors + 2 stale comments (Task 2.1)                                      |
| EDIT   | `src/crosshook-native/src/styles/variables.css`                                 | Remove `--crosshook-route-hero-launch-panel-min-height` (~line 141)                                 |
| EDIT   | `src/crosshook-native/src/styles/collapsible-section.css`                       | Remove dangling `.crosshook-launch-panel` descendant rule (~line 76)                                |

**KEEP — explicitly out of bounds** (live consumers in parentheses):
`LaunchPipeline.tsx` + `launch-pipeline.css` (HeroLaunchGate; smoke locators),
`launch-panel/LaunchPanelFeedback.tsx` (HeroLaunchGate), `preview.css`
(LauncherPreviewModal, ProfilePreviewModal, MigrationReviewModal), all 14
`SidebarIcons.tsx` exports (Sidebar, CommandPalette, InfoTooltip, …), CSS families
`.crosshook-launch-panel__runner-stack` / `__indicator-copy` / `__indicator-guidance`
(HeroLaunchGate.tsx:216-220), `.crosshook-launch-panel__feedback*`
(LaunchPanelFeedback, OfflineReadinessPanel, OfflineTabContent),
`.crosshook-preview-modal*` (live modals), `.crosshook-launch-pipeline*` (smoke).

## NOT Building

- **No CSS behavior changes** — no new breakpoint; the 1280×800 "single-column
  collapse" PRD criterion is recorded as stale (user decision).
- **No SCROLLABLE registration** for `__highlighted-command` (it is `overflow-x` only;
  CLAUDE.md mandates registration for `overflow-y: auto` containers).
- **No new smoke assertions** — existing no-overflow sweep at 1280×800 suffices.
- **No CHANGELOG.md edit** — git-cliff regenerates it at release prep (PRD line 457;
  `validate-release-notes.sh` rejects hand edits).
- **No ADR-0001 update** — zero hero-detail commits touched `platform.rs` (verified;
  Task 3.1 re-proves).
- **No trainer-tab / runtime-hooks work** — deferred to #479 / #482 (no overlap).

## Step-by-Step Tasks

### Task 1.1: Document command-preview token classes in design-tokens.md — Depends on [none]

- **File**: `docs/internal-docs/design-tokens.md`
- Insert a `---`-delimited section `## Command preview token classes` between
  `## Capability indicator tokens` (ends ~line 181) and `## Pipeline connector tokens`
  (~line 183), mirroring DOCS_SECTION_FORMAT.
- Content requirements:
  1. Prose: classes are emitted by `HighlightedCommandBlock.tsx` (`tokenClass()`,
     lines 17–19) inside `.crosshook-hero-detail__highlighted-command`
     (`hero-detail.css:269-286`, `white-space: pre; overflow-x: auto`). They **consume
     existing color tokens — they define no new tokens**. Used only in
     `hero-detail.css`; do not use elsewhere.
  2. Class→token mapping table:

     | Class                                            | Token consumed                    | Default value               | Tone                                |
     | ------------------------------------------------ | --------------------------------- | --------------------------- | ----------------------------------- |
     | `.crosshook-hero-detail__command-token--comment` | `--crosshook-color-text-subtle`   | `rgba(224, 224, 224, 0.56)` | preview header / separators         |
     | `.crosshook-hero-detail__command-token--env-key` | `--crosshook-color-success`       | `#28c76f`                   | env var keys                        |
     | `.crosshook-hero-detail__command-token--value`   | `--crosshook-color-warning`       | `#f5c542`                   | env var values                      |
     | `.crosshook-hero-detail__command-token--binary`  | `--crosshook-color-accent-strong` | `#6ba3d9`                   | wrappers, proton + game executables |
     | `.crosshook-hero-detail__command-token--flag`    | `--crosshook-color-text-muted`    | `rgba(224, 224, 224, 0.76)` | command flags                       |

  3. Note: the base class `.crosshook-hero-detail__command-token` is a structural hook
     with no CSS rule; only the five modifiers carry styling. The authoritative tone
     list is the `TokenTone` union at `HighlightedCommandBlock.tsx:9`.

- **GOTCHA**: issue #477 names these `--cmd-token--*` custom properties — those names
  do not exist. Document the real BEM class names above.
- **VALIDATE**: `npx prettier --check docs/internal-docs/design-tokens.md`

### Task 1.2: Extend Steam Deck checklist with Hero Detail at 1280×800 — Depends on [none]

- **File**: `docs/internal-docs/steam-deck-validation-checklist.md`
- Append `## Phase 12 — Hero Detail at 1280×800` after the Phase 13 section, in
  CHECKLIST_FORMAT. State the target ("Steam Deck native, 1280×800, WebKitGTK") and the
  desktop stand-in command (`gamescope -W 1280 -H 800 -r 60 -- ./CrossHook_amd64.AppImage`,
  per line 11) plus the fast-iteration alternative (`./scripts/dev-native.sh --browser`
  - devtools 1280×800 emulation; not WebKitGTK-accurate — re-verify in Tauri dev mode).
- Subgroups and checks (validate **fit, not collapse** — user decision):
  - `### Profiles tab at deck viewport`: two-column layout (`minmax(220px,280px) 1fr`)
    renders without horizontal page overflow; cards rail and editor both readable;
    editor pane scrolls vertically (`__profiles-editor` is in `SCROLL_ENHANCE_SELECTORS`).
  - `### Launch tab / hooks at deck viewport`: hook rows render readably (rows shrink
    via `min-width: 0`; they stack only below 720px — expected, not a regression);
    no horizontal page overflow.
  - `### Command preview at deck viewport`: a long command in
    `.crosshook-hero-detail__highlighted-command` shows a horizontal scrollbar and
    scrolls without dragging the page (`overscroll-behavior: contain`).
  - `### Automated cross-check`: `npm run test:smoke` green (the
    `hero detail responsive no-horizontal-overflow` describe at `smoke.spec.ts:586-705`
    covers 1280×800).
  - `### Blocking issues (sign-off gate — must be zero)`: seed `- [ ] NONE`.
- **GOTCHA**: do not phrase any check as "collapses to single-column" — the collapse
  breakpoint is 720px and 1280×800 intentionally keeps two columns.
- **VALIDATE**: `npx prettier --check docs/internal-docs/steam-deck-validation-checklist.md`

### Task 1.3: Delete orphan LaunchPanel tree + trim helpers.tsx — Depends on [none]

- **Working dir**: `src/crosshook-native`
- For EACH file below, grep-prove zero external importers immediately before deleting
  (`grep -rn "<module-name>" src tests --include="*.ts" --include="*.tsx"` must show
  only the files being deleted in this task), then delete:
  1. `src/components/LaunchPanel.tsx`
  2. `src/components/launch-panel/LaunchPanelControls.tsx`
  3. `src/components/launch-panel/LaunchPanelVersionStatus.tsx`
  4. `src/components/launch-panel/PreviewModal.tsx`
  5. `src/components/launch-panel/focusTrap.ts`
  6. `src/components/launch-panel/types.ts`
- Trim `src/components/launch-panel/helpers.tsx`:
  - **KEEP**: `sortPatternMatchesBySeverity` (line 16; imported by
    `LaunchPanelFeedback.tsx:2` — LIVE).
  - **DELETE** exports: `severityIcon`, `methodLabel`, `groupEnvBySource`, `isStale`,
    `buildSummaryParts`, `buildGameOnlyRequest`, `buildTrainerOnlyRequest`.
  - Remove the line-3 import from `../../utils/launchPreviewPresentation` once its
    consumers are gone (Biome flags it otherwise).
- **GOTCHA**: `src/components/config-history/helpers.ts` matches `from './helpers'`
  greps — it is a different module; ignore it. `LauncherPreviewModal.tsx:7`,
  `ProfilePreviewModal.tsx:6`, `HeroLaunchGate.tsx:4` mention "LaunchPanel" in
  comments only — update or leave the prose, but they are not imports.
- **GOTCHA**: do NOT touch `LaunchPanelFeedback.tsx`, `LaunchPipeline.tsx`, or any
  `SidebarIcons.tsx` export — all LIVE (see KEEP list).
- **VALIDATE**:
  `npm run typecheck && npx @biomejs/biome ci src/ && npm test`

### Task 2.1: Prune CSS selectors orphaned by Task 1.3 — Depends on [1.3]

- **Files**: `src/crosshook-native/src/styles/theme.css`, `variables.css`,
  `collapsible-section.css`
- For EACH selector family below, grep `src/` for surviving TSX emitters first
  (`grep -rn "<class-name>" src --include="*.tsx"` must be empty); only then delete its
  rules. Do not trust line numbers — they drift; grep the class name in the stylesheet.
  - `.crosshook-launch-panel-stack` (+ descendant rules; research located rules near
    theme.css 186, 437, 442, 517, 3148–3149, 4234)
  - `.crosshook-route-hero-launch-panel` (theme.css ~449) and its variable
    `--crosshook-route-hero-launch-panel-min-height` (variables.css ~141)
  - bare `.crosshook-launch-panel` block class (theme.css; plus the descendant rule in
    `collapsible-section.css` ~76)
  - `.crosshook-launch-panel__profile-row*` family (theme.css ~3172–3199, ~4347–4375)
  - `.crosshook-launch-panel__action*` family (theme.css ~3388–3434, ~4346, ~4369)
  - Stale "LaunchPanel" comments at theme.css ~3191 and ~4353
- **KEEP** (live emitters — grep will prove it): `__runner-stack`, `__indicator-copy`,
  `__indicator-guidance` (HeroLaunchGate.tsx:216-220); `__feedback*` family
  (LaunchPanelFeedback, LaunchPanelVersionStatus is deleted but OfflineReadinessPanel.tsx:36-78
  and OfflineTabContent.tsx:56-87 also emit); `.crosshook-preview-modal*`;
  `.crosshook-launch-pipeline*`; everything in `launch-pipeline.css` and `preview.css`.
- **GOTCHA**: there is no CSS-usage linter — a wrongly-deleted live class fails no
  gate except smoke/manual. The per-class grep IS the gate. Cross-check deleted class
  names against `tests/smoke.spec.ts` locators (only `.crosshook-launch-pipeline*`,
  `.crosshook-palette__row`, `section.crosshook-dashboard-panel-section` are used —
  none in the prune list).
- **VALIDATE**:
  `./scripts/lint.sh && cd src/crosshook-native && npm run test:smoke`

### Task 3.1: Final verification gate + ADR no-op proof — Depends on [1.1, 1.2, 1.3, 2.1]

- Grep guards (all must return empty):

  ```bash
  cd src/crosshook-native
  grep -rn "crosshook-profiles-page__\|crosshook-launch-page__" src tests
  grep -rn "crosshook-launch-panel-stack\|crosshook-route-hero-launch-panel\|crosshook-launch-panel__profile-row\|crosshook-launch-panel__action" src
  grep -rn "from.*components/LaunchPanel'\|launch-panel/PreviewModal\|launch-panel/types\|launch-panel/focusTrap\|LaunchPanelControls\|LaunchPanelVersionStatus" src tests
  ```

- Survivor guard (must still match — LIVE assets intact):

  ```bash
  grep -rln "LaunchPipeline" src/components/library/launch/HeroLaunchGate.tsx
  grep -rln "LaunchPanelFeedback" src/components/library/launch/HeroLaunchGate.tsx
  grep -c "command-token--" src/styles/hero-detail.css   # EXPECT: 5
  ```

- ADR-0001 no-op proof:
  `git log --oneline <merge-base>..HEAD -- src/crosshook-native/crates/crosshook-core/src/platform.rs`
  → EXPECT empty; record in the PR body ("ADR-0001 untouched — no platform.rs diff").
- Run the full validation suite (see Validation Commands below).
- **VALIDATE**: all commands green; greps as expected.

## Testing Strategy

### Unit Tests

No new unit tests — this phase deletes unreferenced code and edits docs. Existing
coverage already pins the surviving surfaces:

- `HighlightedCommandBlock.test.tsx:44-110` — all five tone classes asserted.
- `HeroLaunchGate.test.tsx` — mocks `@/components/LaunchPipeline` (:111-112) and
  `@/components/launch-panel/LaunchPanelFeedback` (:115-116); both targets survive, so
  the suite passes unmodified. If it fails, a KEEP file was wrongly touched.

### Edge Cases Checklist

- [ ] `helpers.tsx` trim leaves `sortPatternMatchesBySeverity` importable —
      `LaunchPanelFeedback.tsx:2` resolves (typecheck proves).
- [ ] No barrel/index re-exports of deleted modules (typecheck two-pass —
      `tsc --noEmit && tsc -p tsconfig.test.json --noEmit` — proves both app and test graphs).
- [ ] Smoke locators `.crosshook-launch-pipeline*` still match (smoke proves).
- [ ] `coverage/` HTML mentioning `LaunchPanel` is a generated artifact — ignore;
      regenerated on next `test:coverage`.

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run typecheck
cd src/crosshook-native && npx @biomejs/biome ci src/
```

EXPECT: clean — this is the primary net for dangling imports after deletion.

### Focused Unit Tests

```bash
cd src/crosshook-native && npx vitest run src/components/library/__tests__/HeroLaunchGate.test.tsx src/components/library/__tests__/HighlightedCommandBlock.test.tsx
```

EXPECT: pass — proves LIVE survivors intact.

### Full Frontend Suite

```bash
cd src/crosshook-native && npm test
```

EXPECT: pass.

### Grep Guards

See Task 3.1 — orphan greps empty, survivor greps non-empty.

### Lint

```bash
./scripts/lint.sh
npx prettier --check "docs/internal-docs/*.md"
```

EXPECT: green (includes rustfmt/clippy no-op, Biome, tsc, ShellCheck,
`check-host-gateway.sh`, `check-legacy-palette.sh`). Prettier is NOT in lint.sh — run
it separately for the doc edits.

### Smoke

```bash
cd src/crosshook-native && npm run test:smoke
```

EXPECT: green — includes the 1280×800 hero-detail no-overflow sweep
(`smoke.spec.ts:586-705`). No screenshot assertions exist, so CSS pruning cannot cause
baseline drift; failures here mean a live locator class was deleted.

### Manual Validation (Steam Deck pass)

Follow the new `## Phase 12 — Hero Detail at 1280×800` checklist section (Task 1.2):

```bash
gamescope -W 1280 -H 800 -r 60 -- ./CrossHook_amd64.AppImage   # canonical
# or fast iteration (NOT WebKitGTK-accurate; re-verify in ./scripts/dev-native.sh):
./scripts/dev-native.sh --browser   # then devtools device emulation 1280×800
```

Library → open Hero Detail → Profiles tab (two-column, no horizontal overflow) →
Launch tab (hooks rows readable) → long command in the highlighted command block
scrolls horizontally without dragging the page. Tick the checklist; blocking issues
must be zero.

## Acceptance Criteria

- [ ] `./scripts/lint.sh` green (issue #477 AC)
- [ ] No orphan `.crosshook-profiles-page__*` / `.crosshook-launch-*page*` selectors
      remain — grep guard empty (issue #477 AC; already true, gate re-proves)
- [ ] Manual Steam Deck 1280×800 pass recorded with zero blocking issues (issue #477 AC)
- [ ] `design-tokens.md` documents the five real command-token classes with their
      consumed tokens (issue #477 scope item 1, corrected names)
- [ ] `LaunchPanel.tsx` + 5 transitively-dead modules deleted; `helpers.tsx` trimmed;
      LaunchPanel-only CSS pruned; all KEEP assets untouched (survivor greps pass)
- [ ] ADR-0001 no-op proven and recorded in PR body (issue #477 scope item 2)
- [ ] `npm run typecheck`, `npm test`, `npm run test:smoke` green

## Completion Checklist

- [ ] All 5 tasks complete, batches gated green
- [ ] Report written to `docs/prps/reports/hero-detail-consolidation-phase-12-polish-cleanup-report.md`
- [ ] Plan archived to `docs/prps/plans/completed/`
- [ ] PR opened: title `feat(ui): document command-preview tokens and remove orphaned launch panel assets`
      (user decision: `feat(ui)` — appears under Features via git-cliff), body per
      `.github/pull_request_template.md` with `Closes #477` + `Part of #478`, labels
      `type:feature area:ui priority:low phase:12 feat:hero-detail-consolidation source:prd`
- [ ] Tracking issue #478: after merge, #477 is the last open phase — tick its checkbox
      (and the stale #475/#476 boxes if still unticked); #478 then closable (#479/#482
      stay open as declared deferred follow-ups)

## Risks

| Risk                                                                                      | Likelihood | Impact                         | Mitigation                                                                                               |
| ----------------------------------------------------------------------------------------- | ---------- | ------------------------------ | -------------------------------------------------------------------------------------------------------- |
| CSS prune deletes a live class (no CSS-usage linter)                                      | Low        | Visual regression              | Per-class grep gate before every deletion; smoke + manual Deck pass; KEEP list is explicit               |
| Research line numbers drift before execution                                              | Medium     | Wrong rule deleted             | Tasks instruct grep-by-name, never line-number edits                                                     |
| `helpers.tsx` trim breaks `LaunchPanelFeedback`                                           | Low        | Vitest/typecheck fail (caught) | KEEP `sortPatternMatchesBySeverity`; focused test gate                                                   |
| Stale issue text misleads implementor into deleting LIVE assets (`LaunchPipeline`, icons) | Low        | Hero Detail breakage           | Summary table + KEEP list up front; survivor grep guard in Task 3.1                                      |
| Deck pass finds a real regression                                                         | Low        | Reopens scope                  | Sign-off gate in checklist; file `platform:steam-deck` + `area:ui` issue rather than scope-creep this PR |

## Notes

- **One PR** for the whole phase (repo precedent: one squash PR per phase issue).
  Squash subject: `feat(ui): document command-preview tokens and remove orphaned launch panel assets` —
  lands in `CHANGELOG.md` verbatim under Features at next release prep.
- The PRD's suggested changelog bullet ("fold Profiles and Launch into Hero Detail")
  is already represented by merged PR titles #495/#496 — do not duplicate it.
- This plan file's own commit: `docs(internal): phase 12 polish and cleanup plan`
  (`docs/prps/plans/` is in the `docs(internal)` MUST list; note `docs/internal-docs/`
  is NOT — the checklist/design-tokens edits ride the `feat(ui)` PR, per the
  `5eaf89c1` precedent).
- Worktree annotations omitted (`--no-worktree`).
- Next step: `/ycc:prp-implement --parallel docs/prps/plans/hero-detail-consolidation-phase-12-polish-cleanup.plan.md`
