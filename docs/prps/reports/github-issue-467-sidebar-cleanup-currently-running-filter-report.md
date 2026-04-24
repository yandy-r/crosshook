---
title: GitHub Issue 467 Sidebar Cleanup + Currently Running Filter Implementation Report
date: 04/24/2026
original-plan: docs/prps/plans/completed/github-issue-467-sidebar-cleanup-currently-running-filter.plan.md
---

# GitHub Issue 467 Sidebar Cleanup + Currently Running Filter Implementation Report

## Overview

Implemented the Phase 2 sidebar cleanup by making the Game section Library-only and moving Favorites plus Currently Playing into fixed Collections-side quick filters. The Library page now accepts filter intents from shell navigation, applies a new `currentlyRunning` filter, and reads active game profile names from the in-memory launch session registry through a thin Tauri command and reusable frontend hook. The approach keeps running state runtime-only, fails open to an empty filter set, and preserves Profiles/Launch access through the command palette.

## Files Changed

### Created

- `src/crosshook-native/src/hooks/useRunningProfiles.ts`: Polls `list_running_profiles`, refreshes on `launch-complete`, and exposes running profile names as a `Set<string>`.
- `src/crosshook-native/src/types/navigation.ts`: Shares the expanded `AppNavigateOptions` and `LibraryFilterIntent` route contracts across shell and pages.

### Modified

- `src/crosshook-native/crates/crosshook-core/src/launch/session/registry.rs`: Added sorted, deduped active profile key reads with kind filtering and registry unit coverage.
- `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`: Added the `list_running_profiles` read command over `LaunchSessionRegistry`.
- `src/crosshook-native/src-tauri/src/commands/launch/mod.rs`: Re-exported the new launch query command and macro symbol.
- `src/crosshook-native/src-tauri/src/lib.rs`: Registered `list_running_profiles` with Tauri command handling.
- `src/crosshook-native/src/components/icons/SidebarIcons.tsx`: Added Heart and Play sidebar icons.
- `src/crosshook-native/src/components/layout/Sidebar.tsx`: Removed Profiles and Launch from the Game section, added library-filter sidebar entries, and preserved collection rendering.
- `src/crosshook-native/src/components/layout/AppShell.tsx`: Centralized navigation through an option-aware handler and emits repeatable Library filter intents.
- `src/crosshook-native/src/components/layout/ContentArea.tsx`: Forwards navigation options and Library filter intent into route content.
- `src/crosshook-native/src/components/pages/LibraryPage.tsx`: Applies incoming filter intents, supports `currentlyRunning`, and returns to Library grid mode for filter jumps.
- `src/crosshook-native/src/components/library/LibraryToolbar.tsx`: Added the Running filter chip.
- `src/crosshook-native/src/components/library/LibraryGrid.tsx`: Widened navigation callback typing to accept route options.
- `src/crosshook-native/src/components/library/LibraryList.tsx`: Widened navigation callback typing to accept route options.
- `src/crosshook-native/src/types/library.ts`: Added `currentlyRunning` to `LibraryFilterKey`.
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts`: Added browser-dev mock state and commands for running profile names.
- `src/crosshook-native/src/lib/mocks/wrapHandler.ts`: Treats `list_running_profiles` as a read command in mock error mode.
- `src/crosshook-native/src/components/layout/__tests__/Sidebar.test.tsx`: Covers the Library-only Game section, fixed quick filters, and filter badges.
- `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`: Covers repeated quick-filter intents and command-palette focus timing.
- `src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx`: Covers Running chip behavior and tab order.
- `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`: Covers Favorites and Running filter intents with deterministic IPC mocks.
- `src/crosshook-native/tests/collections.spec.ts`: Scopes collection modal smoke selection to real collection list items.
- `src/crosshook-native/tests/pipeline.spec.ts`: Navigates to hidden Profiles/Launch routes through the command palette.
- `src/crosshook-native/tests/smoke.spec.ts`: Updates sidebar route expectations, adds quick-filter smoke coverage, and routes Profiles/Launch smoke checks through the command palette.

## New Features

**Library-Only Game Sidebar**: The Game section now exposes only Library, matching the consolidation direction while keeping Profiles and Launch reachable from palette commands.

**Sidebar Quick Filters**: Favorites and Currently Playing appear as first-class Collections entries that navigate to Library and activate the corresponding toolbar chip.

**Currently Running Filter**: Library supports a `currentlyRunning` filter backed by active game sessions rather than static mock data.

**Running Profiles IPC Surface**: `list_running_profiles` exposes runtime-only game profile names from `LaunchSessionRegistry` without adding persisted state.

**Browser Dev Mock Support**: Browser dev mode can list and reset running profiles, so tests and smoke flows exercise the same command adapter path as production.

## Additional Notes

- No database migration, TOML setting, or persistent data was added; running profile data remains runtime-only.
- `useRunningProfiles` intentionally fails open to an empty set and retries on the next poll/event so transient IPC failures do not break Library rendering.
- The sidebar filter entries reuse Library filter state rather than introducing a separate route or duplicate UI mode.
- Full Biome and project lint still report existing warnings in unrelated files, but the commands exit successfully.
- The local git config sets `core.whitespace=indent-with-non-tab`, which conflicts with this repo's Rust/Biome space indentation; a trailing-whitespace-only diff check passes.

## E2E Tests To Perform

### Test 1: Sidebar Navigation Cleanup

**Steps:**

1. Start browser dev mode with populated fixtures.
2. Inspect the Game section in the sidebar.
3. Open the command palette and search for Profiles and Launch.

**Expected Result:**
The Game section shows Library only. Profiles and Launch are absent from the sidebar but are still available and navigable from the command palette.

### Test 2: Favorites Quick Filter

**Steps:**

1. Click Favorites under Collections.
2. Confirm the Library page is visible.
3. Confirm the Favorites toolbar chip is pressed.
4. Click Favorites again from the sidebar.

**Expected Result:**
Library stays in grid mode with the Favorites filter active each time, including repeated clicks.

### Test 3: Currently Playing Quick Filter

**Steps:**

1. Launch a game profile in a dev or native session.
2. Click Currently Playing under Collections.
3. Confirm the Running toolbar chip is pressed.
4. Stop the game and wait for the next running-profile refresh.

**Expected Result:**
Only profiles with active game sessions appear while running. After shutdown, the Running view updates to the empty running state.

### Test 4: Collections Still Open

**Steps:**

1. Click an existing saved collection under the fixed Favorites and Currently Playing entries.
2. Close the collection modal with Escape.

**Expected Result:**
The collection view modal opens for the saved collection, and the fixed quick-filter entries do not intercept collection modal behavior.

### Test 5: Running-State Failure Fallback

**Steps:**

1. In browser dev mode, force read-command errors.
2. Navigate to Library and select the Running filter.

**Expected Result:**
The page renders without uncaught errors and treats the running profile set as empty until the command succeeds again.
