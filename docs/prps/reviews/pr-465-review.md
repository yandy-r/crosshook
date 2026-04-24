# PR Review #465 — feat(ui): Phase 13 polish, accessibility, and design-token docs

**Reviewed**: 2026-04-23T22:05:00Z
**Mode**: PR (parallel, --no-worktree)
**Author**: yandy-r
**Branch**: `feat/unified-desktop-phase-13` → `main`
**Head**: `486a4dd62bfbb9b29e7e783dc3be959ae2bd51e9`
**Decision**: REQUEST CHANGES

## Summary

Scope is on-target for Phase 13: axe coverage, focus-visible rings, reduced-motion guards, design-token docs, CI legacy-palette gate. Follow-up commit `486a4dd` closed the three CodeRabbit findings on the first pass. Three HIGH issues remain: (1) the new `'inspector'` FocusZone is half-wired — the `back()` branch added in the follow-up is unreachable because `dom.ts::getFocusZoneForElement` was never taught to recognize it; (2) the `InstallPage` panel-title rename from "Install & Run" → "Installation options" isn't reflected in the Playwright smoke suite, so `DASHBOARD_ROUTE_HEADINGS.install` will fail; (3) `.crosshook-library-card`'s root hover-lift (`transform: translateY(-2px)`) is not covered by a `prefers-reduced-motion` guard even though the PR claims Library card motion is suppressed.

## Findings

### HIGH

- **[F001]** `src/crosshook-native/src/hooks/gamepad-nav/dom.ts:69` — `getFocusZoneForElement` still whitelists only `'sidebar' | 'content'` in the explicit-zone check and has no `'inspector'` fallback. After this PR widened `FocusZone` to include `'inspector'` (types.ts:3) and the `486a4dd` follow-up added an `inspector` branch to `back()` (focusManagement.ts:273-281), the round-trip is incomplete: `getCurrentZone()` can never return `'inspector'`, so the new `back()` branch is unreachable dead code. Pressing back from inside the Inspector panel will still fall through to `options.onBack?.()` instead of returning focus to `content`.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: In `dom.ts` line 69, accept `explicitZone === 'inspector'` alongside `'sidebar' | 'content'`. Add a unit test in `src/hooks/__tests__/useGamepadNav.test.tsx` that focuses an element under `[data-crosshook-focus-zone="inspector"]`, calls `back()`, and asserts focus moves to the content zone.

- **[F002]** `src/crosshook-native/tests/smoke.spec.ts:52` — `DASHBOARD_ROUTE_HEADINGS.install` is `'Install & Run'` but `InstallPage.tsx:332` renamed the panel title to `'Installation options'`. The smoke-test assertion at line 99 scopes specifically to `.crosshook-dashboard-panel-section__title` with `hasText: dashboardHeading`, so the install-route smoke will fail on this PR. The Phase 13 checklist explicitly notes the panel title changed, but the existing smoke suite wasn't updated.
  - **Status**: Fixed (commit 5f21de6)
  - **Category**: Completeness
  - **Suggested fix**: Update `smoke.spec.ts:52` to `install: 'Installation options',` and reword the comment on line 97 — install no longer shares the banner title.

- **[F003]** `src/crosshook-native/src/styles/library.css:140-143` — `.crosshook-library-card:hover` animates `transform: translateY(-2px)` via the `transition: transform …` declared at line 121, but the reduced-motion guards only cover `__hover-reveal` (line 299-303), `__favorite-heart`/`__list-row`-region (line 342), and `__list-row` (line 842). The card-root hover-lift still animates under `prefers-reduced-motion: reduce`, contradicting the PR's stated goal for Library cards. The Playwright reduced-motion smoke only inspects `__hover-reveal`, so regressions on the card root won't be caught.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Extend the reduced-motion block at `library.css:299` (or the one at 842) with `.crosshook-library-card { transition: none; } .crosshook-library-card:hover { transform: none; }`. Then extend `smoke.spec.ts` reduced-motion block to also assert `transitionDuration === '0s'` on `.crosshook-library-card`.

### MEDIUM

- **[F004]** `src/crosshook-native/src/__tests__/a11y/components.a11y.test.tsx` — No modal / dialog surfaces are exercised by axe. The app has at least a dozen modals (`ProfileReviewModal`, `ProfilePreviewModal`, `LauncherPreviewModal`, `MigrationReviewModal`, `OnboardingWizard`, `ProtonManagerPanel` uninstall confirm, `OfflineTrainerInfoModal`, `ConfigHistoryPanel`, `ProfilesOverlays` rename dialog, collection assign/view modals), each with its own focus-trap, `aria-labelledby`, and escape/close semantics. Claim of "zero axe violations" is technically accurate only for the tested surface; modals are the highest-risk category for focus traps and `aria-modal`, and are invisible to the current suite.
  - **Status**: Fixed
  - **Category**: Test Coverage
  - **Suggested fix**: Add `modals.a11y.test.tsx` covering at minimum `ProfileReviewModal`, `OnboardingWizard`, and `LauncherPreviewModal` in their `open` state — run `axe()` and assert `aria-modal="true"` plus a resolvable `aria-labelledby` target. Can land as a follow-up PR tracked with the Phase 13 completion issue.

- **[F005]** `src/crosshook-native/src/__tests__/a11y/routes.a11y.test.tsx:37` — All route axe tests run against `EMPTY_PROFILE_OVERRIDES`. Most interactive controls (library cards, profile rows, launch panel content, collection chips) are never rendered, so axe inspects mostly empty-state markup. Combined with `color-contrast` disabled globally in `test/setup.ts` (justified under happy-dom), the "zero violations" headline overstates coverage.
  - **Status**: Fixed
  - **Category**: Test Coverage
  - **Suggested fix**: Add at least one populated-fixture axe test per major page variant using the existing `makeLibraryCardData` helper. For `color-contrast`, file a follow-up issue to add `@axe-core/playwright` against `?fixture=populated` in CI so real CSS layout is audited.

- **[F006]** `src/crosshook-native/src/components/profile-sections/RunnerMethodSection.tsx:43`, `community/CommunityProfilesSection.tsx:62`, `host-readiness/HostToolFilterBar.tsx:97` (and ≥3 other call-sites) — The same three-line pattern (`<label id=X htmlFor=Y>` + `<ThemedSelect id=Y ariaLabelledby=X …>`) is now repeated across ≥6 components. CLAUDE.md is explicit: "No copy-paste duplication (DRY): extract shared logic into a shared module." The label/id contract is easy to get wrong when it lives at every call-site.
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Introduce `ThemedSelectField` in `src/components/ui/` that owns label + id wiring (or teach `ThemedSelect` a `label` prop). Migrate call-sites in a follow-up PR — tracker-issue acceptable for this PR.

### LOW

- **[F007]** `src/crosshook-native/src/components/settings/ProfilesSection.tsx:38,40` — The null-crash fix guards `settings.profiles_directory` with `?? ''` at lines 20 and 44, but the `key={\`pd-${settings.profiles_directory}\`}`at line 38 and`defaultValue={settings.profiles_directory}`at line 40 still reference the raw value. If the field is`undefined`(which is the premise of the fix, since test fixtures omit it), the key becomes the literal`"pd-undefined"`and`defaultValue`is`undefined`. Neither crashes, but it undermines the stated invariant.
  - **Status**: Open
  - **Category**: Type Safety
  - **Suggested fix**: Apply `?? ''` at lines 38 and 40, or — preferred — narrow `AppSettingsData.profiles_directory` to `string | undefined` in `src/types/settings.ts` so TS drives the guard instead of defensive coding in each consumer.

- **[F008]** `src/crosshook-native/src/types/settings.ts` (`AppSettingsData.profiles_directory`) — The type is declared `string` (non-optional), but the a11y test fixture `settings_load` in `routes.a11y.test.tsx:56-64` omits the field entirely and `PreferencesContext.applyLoadedPreferences` assigns the payload directly. The runtime invariant (possibly `undefined`) doesn't match the compile-time contract. The PR patched two consumers rather than fixing the type.
  - **Status**: Open
  - **Category**: Type Safety
  - **Suggested fix**: Either mark `profiles_directory?: string` on `AppSettingsData` and keep `?? ''` guards, or merge the loaded payload with `DEFAULT_APP_SETTINGS` inside `PreferencesContext` so the runtime matches the declared type.

- **[F009]** `docs/internal-docs/design-tokens.md:158` — `--crosshook-library-card-aspect: 3 / 4` row reads ambiguously — a reader could think it's literal text rather than an `aspect-ratio` operand.
  - **Status**: Open
  - **Category**: Documentation
  - **Suggested fix**: Add "(aspect-ratio operand)" context next to the `3 / 4` row.

- **[F010]** `src/crosshook-native/src/test/setup.ts` (inline comment above `configureAxe`) — The one-line comment justifies disabling `color-contrast` but doesn't signal where contrast IS tested. Future readers may interpret this as "axe turned down".
  - **Status**: Open
  - **Category**: Documentation
  - **Suggested fix**: Expand the comment to reference the intended Playwright axe harness once F005's follow-up lands, or open a tracking issue and reference it inline.

- **[F011]** `.github/workflows/lint.yml` (new `check-legacy-palette.sh` step) — Placement is fine, but each CI step aborts the job on the first failure. If `check-host-gateway.sh` fails, palette regressions stay hidden. `./scripts/lint.sh` composes them with better error visibility.
  - **Status**: Open
  - **Category**: CI / Maintainability
  - **Suggested fix**: Optional — consider collapsing the three shell gates into a single `run: ./scripts/lint.sh --shell` invocation (or equivalent) so all regressions surface in one iteration. Not a blocker.

## Validation Results

| Check      | Result                                                                                                                       |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass                                                                                                                         |
| Lint       | Pass (Biome, tsc, shellcheck, host-gateway, legacy-palette all green; only pre-existing warnings in unrelated files)         |
| Tests      | Pass (187/189 — 2 pre-existing `AppShell.test.tsx` palette-autofocus failures documented in report as reproducing on `main`) |
| Build      | Pass (tsc + Vite; only pre-existing `INEFFECTIVE_DYNAMIC_IMPORT` warning in `src/lib/ipc.ts`)                                |

## Files Reviewed

- `.github/workflows/lint.yml` (Modified)
- `docs/internal-docs/design-tokens.md` (Modified)
- `docs/internal-docs/steam-deck-validation-checklist.md` (Modified)
- `docs/prps/plans/completed/unified-desktop-phase-13-polish-a11y-docs.plan.md` (Added)
- `docs/prps/reports/unified-desktop-phase-13-polish-a11y-docs-report.md` (Added)
- `src/crosshook-native/package-lock.json` (Modified)
- `src/crosshook-native/package.json` (Modified)
- `src/crosshook-native/src/__tests__/a11y/components.a11y.test.tsx` (Added)
- `src/crosshook-native/src/__tests__/a11y/routes.a11y.test.tsx` (Added)
- `src/crosshook-native/src/components/InstallGamePanel.tsx` (Modified)
- `src/crosshook-native/src/components/LaunchSubTabs.tsx` (Modified)
- `src/crosshook-native/src/components/ProfileSubTabs.tsx` (Modified)
- `src/crosshook-native/src/components/RunExecutablePanel.tsx` (Modified)
- `src/crosshook-native/src/components/SettingsPanel.tsx` (Modified)
- `src/crosshook-native/src/components/UpdateGamePanel.tsx` (Modified)
- `src/crosshook-native/src/components/community/CommunityProfilesSection.tsx` (Modified)
- `src/crosshook-native/src/components/host-readiness/HostToolFilterBar.tsx` (Modified)
- `src/crosshook-native/src/components/layout/ConsoleDrawer.tsx` (Modified)
- `src/crosshook-native/src/components/pages/InstallPage.tsx` (Modified)
- `src/crosshook-native/src/components/pages/profiles/ProfilesHero.tsx` (Modified)
- `src/crosshook-native/src/components/profile-sections/RunnerMethodSection.tsx` (Modified)
- `src/crosshook-native/src/components/settings/ProfilesSection.tsx` (Modified)
- `src/crosshook-native/src/hooks/gamepad-nav/focusManagement.ts` (Modified)
- `src/crosshook-native/src/hooks/gamepad-nav/types.ts` (Modified)
- `src/crosshook-native/src/styles/host-tool-dashboard.css` (Modified)
- `src/crosshook-native/src/styles/library.css` (Modified)
- `src/crosshook-native/src/styles/palette.css` (Modified)
- `src/crosshook-native/src/styles/sidebar.css` (Modified)
- `src/crosshook-native/src/styles/themed-select.css` (Modified)
- `src/crosshook-native/src/test/setup.ts` (Modified)
- `src/crosshook-native/tests/smoke.spec.ts` (Modified)
