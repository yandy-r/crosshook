# Phase 4 Coverage Gaps Tracking

> **Action Required**: Create a GitHub issue from this document with labels: `type:feature`, `area:build`, `priority:medium`, `source:prd`, `feat:frontend-test-framework`, `tracking`, `phase:4`
>
> **Note**: Automated issue creation blocked by API permissions. Issue body prepared in `/tmp/tracking-issue-body.md` for manual creation via GitHub web UI.

## Summary

Track coverage gaps on critical surfaces defined in PRD Phase 4. The 60% coverage threshold has been configured in `vitest.config.ts` and is enforced on:
- `src/hooks/**` (excluding `useProfile.ts`)
- `src/lib/{ipc,events,runtime}.ts`
- `src/components/pages/*.tsx`

**Current Status**: 9.7% overall lines on critical surfaces

## Progress Summary

### ✅ Completed
- [x] **src/lib** - 96% lines (ipc.ts, events.ts, runtime.ts) - **EXCEEDS THRESHOLD**
- [x] Coverage threshold configured in vitest.config.ts
- [x] .gitignore updated for coverage reports

### 🚧 In Progress
- [ ] **src/hooks** - 10.97% lines (48 hooks at 0%, 3 at >60%)
- [ ] **src/components/pages** - 0% lines (11 pages need representative tests)

## Critical Surface Breakdown

### src/lib (3 files) ✅ COMPLETE

All three files now exceed the 60% threshold:
- ✅ **events.ts** - 96.55% lines
- ✅ **runtime.ts** - 96% lines
- ✅ **ipc.ts** - 85.71% lines

### src/hooks (53 hooks total)

#### Already at ≥60% (3 hooks)
- [x] `useCapabilityGate.ts` - 93.75%
- [x] `useGameCoverArt.ts` - 70%
- [x] `useGamepadNav.ts` - 61.24%

#### Close to threshold (1 hook)
- [ ] `useOnboarding.ts` - 53.76% (needs +7%)

#### Zero coverage - High Priority (15 hooks)
Core user flows and IPC-heavy hooks:
- [ ] `useHostReadiness.ts` - 0.94%
- [ ] `useLaunchState.ts` - 0%
- [ ] `useInstallGame.ts` - 0%
- [ ] `useUpdateGame.ts` - 0%
- [ ] `useProtonManager.ts` - 0%
- [ ] `useProtonInstalls.ts` - 0%
- [ ] `useCommunityProfiles.ts` - 0%
- [ ] `useImportCommunityProfile.ts` - 0%
- [ ] `useProfileHealth.ts` - 0%
- [ ] `useLauncherExport.ts` - 0%
- [ ] `useLauncherManagement.ts` - 0%
- [ ] `useOfflineReadiness.ts` - 0%
- [ ] `useTrainerDiscovery.ts` - 0%
- [ ] `usePrefixDeps.ts` - 0%
- [ ] `useProtonDbLookup.ts` - 0%

#### Zero coverage - Medium Priority (18 hooks)
UI state and presentation hooks:
- [ ] `useGameMetadata.ts` - 0%
- [ ] `useLibraryProfiles.ts` - 0%
- [ ] `useLibrarySummaries.ts` - 0%
- [ ] `useProfileSummaries.ts` - 0%
- [ ] `useCollectionDefaults.ts` - 0%
- [ ] `useCollectionMembers.ts` - 0%
- [ ] `useFocusTrap.ts` - 0%
- [ ] `useAccessibilityEnhancements.ts` - 0%
- [ ] `useScrollEnhance.ts` - 0%
- [ ] `useGameDetailsProfile.ts` - 0%
- [ ] `useGameDetailsRequestGuards.ts` - 0%
- [ ] `usePreviewState.ts` - 0%
- [ ] `useAcknowledgeVersionChange.ts` - 0%
- [ ] `useLaunchOptimizationCatalog.ts` - 0%
- [ ] `useLaunchPlatformStatus.ts` - 0%
- [ ] `useLaunchPrefixDependencyGate.ts` - 0%
- [ ] `useMangoHudPresets.ts` - 0%
- [ ] `useExternalTrainerSearch.ts` - 0%

#### Zero coverage - Lower Priority (15 hooks)
Specialized/niche functionality:
- [ ] `useImageDominantColor.ts` - 0%
- [ ] `useProtonDbSuggestions.ts` - 0%
- [ ] `useProtonInstallProgress.ts` - 0%
- [ ] `useProtonMigration.ts` - 0%
- [ ] `useProtonUp.ts` - 0%
- [ ] `useRunExecutable.ts` - 0%
- [ ] `useSetTrainerVersion.ts` - 0%
- [ ] `useTrainerTypeCatalog.ts` - 0%
- [ ] `useUmuCoverage.ts` - 0%
- [ ] `useUmuDatabaseRefresh.ts` - 0%
- [ ] `useVersionCheck.ts` - 0%
- [ ] `usePrefixStorageManagement.ts` - 0%
- [ ] `useProtonInstallProgress.ts` - 0%
- [ ] `useRunExecutable.ts` - 0%
- [ ] `useSetTrainerVersion.ts` - 0%

### src/components/pages (11 pages)

Per PRD: "representative empty/loading/error/success for pages"

#### High Priority Pages
- [ ] `LibraryPage.tsx` - 0% (loading, empty library, populated)
- [ ] `ProfilesPage.tsx` - 0% (loading, no profiles, with profiles)
- [ ] `LaunchPage.tsx` - 0% (no game selected, game selected, launch states)
- [ ] `InstallPage.tsx` - 0% (validation states, install flow)

#### Medium Priority Pages
- [ ] `HostToolsPage.tsx` - 0% (readiness states)
- [ ] `HealthDashboardPage.tsx` - 0% (health check states)
- [ ] `ProtonManagerPage.tsx` - 0% (version list states)
- [ ] `CompatibilityPage.tsx` - 0% (compatibility data states)
- [ ] `CommunityPage.tsx` - 0% (tap list states)
- [ ] `SettingsPage.tsx` - 0% (settings sections)

#### Low Priority Pages
- [ ] `DiscoverPage.tsx` - 0% (minimal page)

## Implementation Strategy

### Phase 4a ✅ (This PR)
- [x] Configure 60% threshold in vitest.config.ts
- [x] Add tests for `src/lib/{ipc,events,runtime}.ts`
- [x] Create this tracking document

### Phase 4b (Follow-up PR 1)
Focus on quick wins and IPC boundary:
- [ ] Complete `useOnboarding.ts` (7% gap)
- [ ] Add tests for 5 high-priority pages
- Target: Representative page tests showing empty/loading/error/success patterns

### Phase 4c-f (Follow-up PRs 2-5)
Systematic hook coverage in priority order:
- Each PR targets 10-12 hooks to keep reviews manageable
- Prioritize by: complexity × criticality to user flows
- PR 2: High-priority hooks (launch, install, proton)
- PR 3: Medium-priority hooks (library, profiles, collections)
- PR 4: Medium-priority hooks (metadata, UI state)
- PR 5: Low-priority hooks (utilities, specialized features)

## Test Pattern Reference

Per PRD §5 and `docs/TESTING.md`:

**Hook tests**: Use `renderHook()` + `vi.mock('@/lib/ipc')` → `registerMocks()`
**Page tests**: Use `renderWithMocks()` + representative states
**IPC tests**: Use `mockIPC` for Tauri-real branches (Pattern A)

## Notes

- **Deferred**: `useProfile.ts` (1668 lines) - Explicitly excluded per PRD §4.6
- **Deferred**: Hook subdirectories `src/hooks/{install,profile}/**` - Not in critical surface scope
- **Success Criteria**: `vitest run --coverage` must pass with 60% gate on critical surfaces
- **Current**: Gate is configured but not passing (9.7% overall on critical surfaces)

## Related

- Parent: #282 (Frontend Test Framework PRD tracker)
- Issue: #286 (This Phase 4 implementation)
- PRD: `docs/prps/prds/frontend-test-framework.prd.md` § Phase 4
- Dependencies: Phase 2 CI (#284), Phase 3 docs (#285)
