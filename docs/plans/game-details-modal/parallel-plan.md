# game-details-modal Implementation Plan

Implement a library-scoped, read-only game details modal by composing existing frontend hooks and Tauri commands without introducing new persistence in v1. The core architecture change is interaction orchestration in `LibraryPage` plus an isolated `GameDetailsModal` component that follows established `crosshook-modal` accessibility and focus conventions. Integration should prioritize reuse of `useLibrarySummaries`, `useGameMetadata`, `useGameCoverArt`, and existing page handlers before considering new IPC surface. Delivery should proceed shell-first, then data enrichment, then resilience/polish, with explicit safeguards for card click semantics, stale async state, and secure remote-text rendering.

## Critically Relevant Files and Documentation

- /src/crosshook-native/src/components/pages/LibraryPage.tsx: library orchestration and best owner for modal state.
- /src/crosshook-native/src/components/library/LibraryGrid.tsx: callback threading from page to cards.
- /src/crosshook-native/src/components/library/LibraryCard.tsx: card body/footer interaction boundary.
- /src/crosshook-native/src/components/ProfileReviewModal.tsx: modal lifecycle and accessibility pattern to mirror.
- /src/crosshook-native/src/components/ProfilePreviewModal.tsx: additional portal/focus pattern reference.
- /src/crosshook-native/src/styles/theme.css: shared `crosshook-modal` class contract.
- /src/crosshook-native/src/hooks/useLibrarySummaries.ts: summary loading pattern and error handling.
- /src/crosshook-native/src/hooks/useGameMetadata.ts: stale-request-safe metadata fetch pattern.
- /src/crosshook-native/src/hooks/useGameCoverArt.ts: cover art and local asset conversion behavior.
- /src/crosshook-native/src/hooks/useProtonDbLookup.ts: optional compatibility section data source.
- /src/crosshook-native/src/hooks/useScrollEnhance.ts: scroll target selector contract for inner scrollers.
- /src/crosshook-native/src-tauri/src/lib.rs: registered command names and IPC alignment reference.
- /docs/plans/game-details-modal/shared.md: canonical planning context and constraints.
- /docs/plans/game-details-modal/feature-spec.md: feature scope, acceptance behavior, and phased intent.
- /docs/plans/game-details-modal/research-technical.md: implementation constraints and integration details.
- /docs/plans/game-details-modal/research-recommendations.md: rollout strategy and interaction tradeoffs.
- /docs/plans/game-details-modal/research-security.md: trust boundaries and mandatory guardrails.
- /AGENTS.md: route layout, scroll behavior, architecture boundary, and verification conventions.

## Implementation Plan

Note: some `READ THESE BEFORE TASK` entries intentionally reference files created by prerequisite tasks listed in each task dependency line.

### Phase 1: Modal Foundation and Library Entry

#### Task 1.1: Build modal shell component Depends on [none]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/components/ProfileReviewModal.tsx
- /src/crosshook-native/src/components/ProfilePreviewModal.tsx
- /src/crosshook-native/src/styles/theme.css
- /docs/plans/game-details-modal/feature-spec.md
- /docs/plans/game-details-modal/research-security.md

**Instructions**

Files to Create

- /src/crosshook-native/src/components/library/GameDetailsModal.tsx
- /src/crosshook-native/src/components/library/GameDetailsModal.css

Files to Modify

- /src/crosshook-native/src/styles/theme.css

Create a reusable modal shell for game details that mirrors existing `crosshook-modal` semantics: portal render, dialog roles/labels, backdrop dismiss, escape close, and explicit close controls tagged for gamepad back compatibility. Keep content sections placeholder-only in this task so integration can proceed independently. Ensure remote-facing text slots are plain text by default and avoid introducing HTML-rendering paths.

#### Task 1.2: Wire modal state in Library page Depends on [1.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/components/pages/LibraryPage.tsx
- /src/crosshook-native/src/types/library.ts
- /docs/plans/game-details-modal/feature-spec.md

**Instructions**

Files to Create

- /src/crosshook-native/src/components/library/useGameDetailsModalState.ts

Files to Modify

- /src/crosshook-native/src/components/pages/LibraryPage.tsx
- /src/crosshook-native/src/components/library/GameDetailsModal.tsx

Add focused state management for selected profile/details-open behavior in `LibraryPage`, including open/close handlers and modal rendering. Opening details must immediately call `selectProfile(selectedName)` so selection and modal context stay synchronized. Keep this task scoped to state orchestration and shell mounting; defer section data hydration to later tasks.

#### Task 1.3: Split card body vs footer interactions Depends on [1.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/components/library/LibraryCard.tsx
- /src/crosshook-native/src/components/library/LibraryGrid.tsx
- /src/crosshook-native/src/components/pages/LibraryPage.tsx

**Instructions**

Files to Create

- /src/crosshook-native/src/components/library/library-card-interactions.ts

Files to Modify

- /src/crosshook-native/src/components/library/LibraryCard.tsx
- /src/crosshook-native/src/components/library/LibraryGrid.tsx
- /src/crosshook-native/src/components/pages/LibraryPage.tsx

Introduce an explicit details-open interaction on card body while preserving footer action behavior (`Launch`, `Favorite`, `Edit`) through event-isolation safeguards. Keep callback names and props clear so downstream tasks can rely on a stable API. Add concise inline comments only where event ordering is non-obvious.

### Phase 2: Core Data and Actions MVP

#### Task 2.1: Add profile-backed modal view model Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useLibrarySummaries.ts
- /src/crosshook-native/src/types/library.ts
- /src/crosshook-native/src/hooks/useProfile.ts
- /src/crosshook-native/src-tauri/src/commands/profile.rs
- /src/crosshook-native/src/components/library/GameDetailsModal.tsx
- /docs/plans/game-details-modal/research-technical.md

**Instructions**

Files to Create

- /src/crosshook-native/src/hooks/useGameDetailsProfile.ts
- /src/crosshook-native/src/types/game-details-modal.ts

Files to Modify

- /src/crosshook-native/src/components/library/GameDetailsModal.tsx
- /src/crosshook-native/src/components/pages/LibraryPage.tsx

Load full profile-backed details for the selected item using existing command pathways and map data into a modal view model with explicit section states (`loading`, `ready`, `unavailable`, `error`). Keep the baseline `LibraryCardData` summary from `LibraryPage` as the primary seed so modal identity stays aligned with card content. Keep failures localized to modal sections without disturbing library-level state.

#### Task 2.2: Add metadata and cover art sections Depends on [2.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useGameMetadata.ts
- /src/crosshook-native/src/hooks/useGameCoverArt.ts
- /src/crosshook-native/src/components/profile-sections/GameMetadataBar.tsx
- /src/crosshook-native/src-tauri/src/commands/game_metadata.rs
- /docs/plans/game-details-modal/research-technical.md

**Instructions**

Files to Create

- /src/crosshook-native/src/components/library/GameDetailsMetadataSection.tsx

Files to Modify

- /src/crosshook-native/src/components/library/GameDetailsModal.tsx
- /src/crosshook-native/src/components/library/GameDetailsModal.css

Compose metadata and cover-art enrichment using existing hooks, with section-level degraded states for missing Steam App ID, offline behavior, and transient failures. Reuse existing visual patterns and copy tone. Ensure all remote text remains safely rendered and clearly labeled when stale/unavailable.

#### Task 2.3: Wire quick actions through existing handlers Depends on [1.2, 1.3, 2.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/components/pages/LibraryPage.tsx
- /src/crosshook-native/src/components/library/GameDetailsModal.tsx
- /src/crosshook-native/src/components/library/LibraryCard.tsx

**Instructions**

Files to Create

- /src/crosshook-native/src/components/library/game-details-actions.ts

Files to Modify

- /src/crosshook-native/src/components/pages/LibraryPage.tsx
- /src/crosshook-native/src/components/library/GameDetailsModal.tsx

Expose minimal modal quick actions by delegating to existing page-level handlers rather than duplicating launch/edit/favorite logic. Define clear behavior for whether actions close the modal before route changes. Keep the action layer thin and testable, with no direct persistence writes.

### Phase 3: Enrichment, Security Hardening, and UX Resilience

#### Task 3.1: Add optional compatibility and health sections Depends on [2.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useProtonDbLookup.ts
- /src/crosshook-native/src/hooks/useProfileHealth.ts
- /src/crosshook-native/src/hooks/useOfflineReadiness.ts
- /docs/plans/game-details-modal/research-integration.md

**Instructions**

Files to Create

- /src/crosshook-native/src/components/library/GameDetailsCompatibilitySection.tsx
- /src/crosshook-native/src/components/library/GameDetailsHealthSection.tsx

Files to Modify

- /src/crosshook-native/src/components/library/GameDetailsModal.tsx

Add optional sections for compatibility and profile health/offline context using existing hooks and command contracts. Keep each section independently degradable and avoid blocking core modal usability when one data source fails.

#### Task 3.2: Harden async sequencing and stale-state behavior Depends on [2.1]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useGameMetadata.ts
- /src/crosshook-native/src/hooks/useGameDetailsProfile.ts
- /src/crosshook-native/src/components/library/GameDetailsModal.tsx

**Instructions**

Files to Create

- /src/crosshook-native/src/hooks/useGameDetailsRequestGuards.ts

Files to Modify

- /src/crosshook-native/src/hooks/useGameDetailsProfile.ts
- /src/crosshook-native/src/components/library/GameDetailsModal.tsx

Implement request-id/cancellation guards for rapid profile switching so stale responses do not overwrite newer selection state. Keep loading transitions stable and predictable, with clear section fallback copy for timeout or unavailable states.

#### Task 3.3: Audit and harden focus/scroll contracts Depends on [none]

**READ THESE BEFORE TASK**

- /src/crosshook-native/src/hooks/useScrollEnhance.ts
- /src/crosshook-native/src/App.tsx
- /src/crosshook-native/src/components/ProfileReviewModal.tsx
- /AGENTS.md

**Instructions**

Files to Create

- /docs/plans/game-details-modal/manual-checklist.md

Files to Modify

- /src/crosshook-native/src/components/library/GameDetailsModal.css
- /src/crosshook-native/src/hooks/useScrollEnhance.ts

Ensure modal focus and scroll behavior remain consistent on keyboard and gamepad flows, especially at narrow viewport sizes. If new inner scroll regions are introduced, add their selectors to `SCROLLABLE`; otherwise keep scrolling centered on `.crosshook-modal__body`. Document concrete manual checks in `manual-checklist.md`.

### Phase 4: Verification, Closeout, and Follow-Ups

#### Task 4.1: Execute verification matrix and capture outcomes Depends on [2.2, 2.3, 3.1, 3.2, 3.3]

**READ THESE BEFORE TASK**

- /docs/plans/game-details-modal/manual-checklist.md
- /docs/plans/game-details-modal/feature-spec.md
- /AGENTS.md

**Instructions**

Files to Create

- /docs/plans/game-details-modal/verification-results.md

Files to Modify

- /tasks/todo.md
- /docs/plans/game-details-modal/manual-checklist.md

Run the manual verification matrix and focused compile/build checks relevant to frontend-only changes. Record pass/fail evidence, open risks, and any discovered regressions. Update `tasks/todo.md` with completion status and residual caveats.

#### Task 4.2: Apply final polish fixes from verification Depends on [4.1]

**READ THESE BEFORE TASK**

- /docs/plans/game-details-modal/verification-results.md
- /src/crosshook-native/src/components/library/GameDetailsModal.tsx
- /src/crosshook-native/src/components/pages/LibraryPage.tsx

**Instructions**

Files to Create

- /docs/plans/game-details-modal/follow-up-issues.md

Files to Modify

- /src/crosshook-native/src/components/library/GameDetailsModal.tsx
- /src/crosshook-native/src/components/library/GameDetailsModal.css
- /src/crosshook-native/src/components/pages/LibraryPage.tsx

Address high-priority regressions uncovered during verification, then capture deferred non-blockers in a follow-up issues document. Keep this task strictly focused on polish/stability, not scope expansion.

#### Task 4.3: Prepare implementation handoff notes Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/game-details-modal/shared.md
- /docs/plans/game-details-modal/analysis-tasks.md
- /docs/plans/game-details-modal/research-practices.md

**Instructions**

Files to Create

- /docs/plans/game-details-modal/implementation-handoff.md

Files to Modify

- /tasks/todo.md

Produce a concise handoff guide with execution order, ownership suggestions, and explicit do-not-change constraints (no v1 persistence changes, reuse-first policy, secure text rendering). This can run in parallel with feature coding and reduces coordination overhead during implementation.

## Advice

- Keep `LibraryCard` interaction changes isolated early; this is the most likely source of behavioral regression.
- Prefer a thin modal view-model hook over bloating `LibraryPage` with multi-source fetch orchestration.
- Do not add new IPC surface until an explicit field gap is identified during implementation.
- Treat focus/scroll behavior as acceptance criteria, not post-launch polish, especially for Deck/controller flows.
- If verification reveals large UX disagreement, open a scoped follow-up issue instead of expanding v1.
