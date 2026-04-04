# Feature Spec: Game Details Modal

## Executive Summary

The Game Details Modal adds a read-only, in-context details surface for Library cards so users can inspect game/profile state without leaving the grid. The feature improves discoverability and reduces context switching by aggregating existing data already tracked by CrossHook: profile summary fields, health/offline signals, trainer and launch context, and activity indicators. Implementation should stay frontend-first by reusing existing Tauri commands, hooks, and modal patterns in `src/crosshook-native`, with no new persistence in the initial release. Key challenges are interaction semantics (card click versus existing selection behavior), robust degraded/offline states, and avoiding regressions in existing Launch/Favorite/Edit actions.

## External Dependencies

### APIs and Services

#### ProtonDB (existing optional compatibility enrichment)

- **Documentation**: [ProtonDB app page pattern](https://www.protondb.com/app/730) and in-repo usage via existing backend client.
- **Authentication**: None for current public JSON endpoints used by CrossHook.
- **Key Endpoints**:
  - `GET /api/v1/reports/summaries/{appId}.json`: compatibility summary fields.
  - `GET /data/counts.json`: report-count metadata used for feed derivation.
- **Rate Limits**: No official published limits; treat conservatively and rely on existing caching behavior.
- **Pricing**: No paid API model used in current scope.

#### Steam Store `appdetails` (optional metadata enrichment)

- **Documentation**: community reference at [Team Fortress Wiki storefront API notes](https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI#appdetails).
- **Authentication**: None.
- **Key Endpoints**:
  - `GET /api/appdetails?appids={id}`: optional metadata and artwork-related fields.
- **Rate Limits**: Unpublished; use current backend caching and retry patterns.
- **Pricing**: No paid model for this endpoint.

### Libraries and SDKs

| Library                     | Purpose                          | Usage in this feature                                                             |
| --------------------------- | -------------------------------- | --------------------------------------------------------------------------------- |
| `@tauri-apps/api`           | IPC invoke bridge                | Reuse existing commands and DTOs                                                  |
| `react`                     | UI composition/state             | Modal state orchestration and section rendering                                   |
| existing custom modal stack | Dialog semantics/focus shell     | Reuse `crosshook-modal` conventions instead of introducing a new dialog framework |
| `reqwest` / `serde` in core | Existing remote lookup + parsing | No new remote client required for v1                                              |

### External Documentation

- [GitHub Issue #143](https://github.com/yandy-r/crosshook/issues/143): source requirements and initial scope.
- [WAI-ARIA Authoring Practices - Dialog (Modal)](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/): accessibility behavior baseline.

## Business Requirements

### User Stories

**Primary User: Library user**

- As a library user, I want to open a game details modal from a card so that I can inspect key information without navigating away.
- As a library user, I want existing card actions to keep their current behavior so that muscle memory and workflow are preserved.

**Secondary User: Offline or partially connected user**

- As an offline user, I want cached/unavailable states to be explicit so that I understand which values are stale or missing.

### Business Rules

1. **Read-only scope**: The modal is informational; profile edits remain in existing routes.
   - Validation: No inline-edit controls in v1.
   - Exception: Quick actions can route users to existing edit/launch flows.
2. **Interaction isolation**: Card-body interaction opens modal; footer controls keep current behavior.
3. **Data reuse first**: Modal aggregates existing data sources and existing IPC contracts.
4. **Degraded transparency**: Missing or unreachable data must be shown as unavailable/cached rather than silently hidden.
5. **Storage boundary**: No new TOML keys, SQLite schema changes, or migrations for initial release.

### Edge Cases

| Scenario                        | Expected Behavior                                                 | Notes                                          |
| ------------------------------- | ----------------------------------------------------------------- | ---------------------------------------------- |
| Profile has no Steam App ID     | Modal still opens with local fields and fallback content          | ProtonDB/Steam sections show unavailable state |
| Remote lookup unavailable       | Cached value if present; otherwise unavailable label              | Modal remains usable                           |
| Rapidly opening different cards | Only latest modal selection should render final loaded state      | Cancel or ignore stale in-flight responses     |
| User triggers Launch from modal | Existing launch flow runs and modal closes/navigates consistently | Follow final UX decision                       |

### Success Criteria

- [ ] Card-body opens details modal with no regressions to Launch/Favorite/Edit controls.
- [ ] Modal content renders useful identity, status, and action context for at least one representative profile in each major state (healthy, partial, unavailable data).
- [ ] Esc/click-outside/close-button dismissal works consistently with focus restoration.
- [ ] No persistence schema/config changes are introduced for v1.

## Technical Specifications

### Architecture Overview

```text
[LibraryCard body click]
        |
        v
[LibraryPage modal state] ----> [GameDetailsModal]
        |                              |
        |                              +--> existing quick-action handlers
        |
        +--> [useLibrarySummaries] (baseline fields)
        +--> [health/offline hooks/context] (status fields)
        +--> [existing IPC lookups] (optional ProtonDB/metadata)
```

### Data Models

#### Runtime view model (frontend only)

| Field         | Type                      | Constraints | Description                                   |
| ------------- | ------------------------- | ----------- | --------------------------------------------- |
| `profileName` | `string`                  | required    | Selected profile identity                     |
| `summary`     | `LibraryCardData`         | required    | Card-level baseline data                      |
| `sections`    | map of section states     | required    | `loading/ready/unavailable/error` per section |
| `actions`     | action availability model | required    | Enables quick actions by capability           |

**Indexes:** none (runtime-only)

**Relationships:**

- `profileName` maps to existing summary and status lookups.
- Modal model composes existing persisted data; no new persisted entity is introduced.

### API Design

#### Reused IPC surface

**Purpose**: gather existing profile and status data without adding new backend endpoints in v1.

**Representative requests:**

```json
{ "command": "profile_list_summaries" }
```

```json
{ "command": "profile_load", "args": { "name": "My Profile" } }
```

**Representative response shape (conceptual):**

```json
{
  "profileName": "My Profile",
  "status": "ready",
  "details": { "steamAppId": 730, "favorite": true }
}
```

**Errors:**

| Status                 | Condition            | Response                                  |
| ---------------------- | -------------------- | ----------------------------------------- |
| UI `error` state       | Lookup failed        | Section-level error with retry/close path |
| UI `unavailable` state | Data not set/offline | Explicit unavailable badge/text           |

### System Integration

#### Files to Create

- `src/crosshook-native/src/components/library/GameDetailsModal.tsx`: modal component and section composition.
- `src/crosshook-native/src/components/library/GameDetailsModal.css` (or equivalent style additions): modal-specific layout styling.

#### Files to Modify

- `src/crosshook-native/src/components/library/LibraryCard.tsx`: card-body interaction hook-up.
- `src/crosshook-native/src/components/pages/LibraryPage.tsx`: modal state, selected profile wiring, action delegation.
- `src/crosshook-native/src/components/library/LibraryGrid.tsx`: pass-through props/events as needed.
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: only if new scrollable containers are added.

#### Configuration

- No new configuration keys required for v1.

## UX Considerations

### User Workflows

#### Primary Workflow: Open and inspect details

1. **Open**
   - User: clicks card body.
   - System: opens modal overlay with immediate shell and loading placeholders.
2. **Inspect**
   - User: scans hero/game/trainer/activity/organization sections.
   - System: progressively hydrates sections and marks unavailable/cached states.
3. **Act or close**
   - User: launches, edits profile, exports/copies action (if enabled), or dismisses modal.
   - System: executes existing flow or restores focus to invoking card.

#### Error Recovery Workflow

1. **Error Occurs**: one or more data sources fail to resolve.
2. **User Sees**: section-level error/unavailable message with retry when applicable.
3. **Recovery**: user retries section fetch or continues with available sections and closes modal.

### UI Patterns

| Component               | Pattern                              | Notes                                            |
| ----------------------- | ------------------------------------ | ------------------------------------------------ |
| Details modal container | Existing `crosshook-modal` structure | Keep parity with current focus/backdrop behavior |
| Section cards           | Read-only grouped fields             | Consistent heading and state badges              |
| Quick action bar        | Existing action semantics            | Avoid duplicating business logic paths           |

### Accessibility Requirements

- Dialog semantics: `role="dialog"`, `aria-modal="true"`, labeled by visible title.
- Keyboard behavior: Esc to dismiss, tab containment, focus restore to invoking control.
- Pointer/touch parity: clear hit targets and no hover-only critical affordances.
- Status messaging: errors/offline states must include text labels, not color alone.

### Performance UX

- **Loading States**: show shell-first skeletons and progressive section hydration.
- **Optimistic Updates**: keep favorite interaction consistent with existing optimistic pattern.
- **Error Feedback**: section-scoped errors where possible; modal remains dismissible at all times.

## Recommendations

### Implementation Approach

**Recommended Strategy**: deliver a frontend-first modal that reuses existing commands and UI patterns, then incrementally enrich fields while preserving interaction stability.

**Decision Lock (confirmed):**

- Modal open also sets the active profile.
- V1 quick actions are minimal.
- Dismiss behavior includes Esc, outside click, and close button.

**Phasing:**

1. **Phase 1 - Foundation**: modal shell, open/close semantics, baseline fields, core quick actions.
2. **Phase 2 - Aggregation**: health/activity/organization enrichment and robust degraded states.
3. **Phase 3 - Polish**: performance tuning, finer accessibility polish, optional richer section content.

### Technology Decisions

| Decision         | Recommendation                               | Rationale                                        |
| ---------------- | -------------------------------------------- | ------------------------------------------------ |
| Dialog framework | Reuse existing modal implementation patterns | Reduces dependency churn and behavior drift      |
| Data access      | Reuse existing IPC/hook surfaces first       | Keeps `src-tauri` and core thin for this feature |
| Persistence      | No new storage in v1                         | Matches issue scope and avoids migration risk    |

### Quick Wins

- Introduce modal shell and card-body entry path without backend changes.
- Reuse existing quick action handlers (launch/edit/favorite) inside modal.
- Add clear unavailable/cached states to remote-dependent sections from day one.

### Future Enhancements

- Dedicated lightweight detail DTO if repeated multi-source fetch becomes expensive.
- User preference for default-open section/tab (requires explicit persistence decision).
- Optional expanded activity timeline or richer ProtonDB detail drill-down.

## Risk Assessment

### Technical Risks

| Risk                                            | Likelihood | Impact | Mitigation                                                              |
| ----------------------------------------------- | ---------- | ------ | ----------------------------------------------------------------------- |
| Interaction regression on card controls         | Medium     | High   | Keep button event isolation and add targeted UI regression checks       |
| Focus/scroll issues in modal                    | Medium     | High   | Reuse existing modal/scroll patterns and verify keyboard + Deck layouts |
| Stale async responses when switching cards fast | Medium     | Medium | Track request identity and ignore stale completions                     |
| Over-fetching remote data                       | Low        | Medium | Use cached-first strategy and section-level lazy fetch                  |

### Integration Challenges

- Coordinating modal-open behavior with current profile selection semantics.
- Keeping Launch/Edit routing behavior consistent whether initiated from card or modal.
- Ensuring new scroll containers do not break existing enhanced scroll targeting.

### Security Considerations

- Treat displayed local paths and runtime details as potentially sensitive; avoid exposing hidden secrets.
- Do not introduce new secret-bearing configuration for this feature.
- Preserve existing safe clipboard behavior for copy actions.

## Task Breakdown Preview

### Phase 1: Modal Foundation

**Focus**: entry interaction and baseline modal shell.
**Tasks**:

- Add `GameDetailsModal` component and styles.
- Wire Library card-body interaction to modal open state.
- Hook baseline summary fields and close semantics.
  **Parallelization**: modal styling and data wiring can proceed in parallel after interface is defined.

### Phase 2: Data Aggregation and Actions

**Focus**: section enrichment and quick-action parity.
**Dependencies**: Phase 1 modal shell and entry point complete.
**Tasks**:

- Connect health/activity/organization sections to existing data sources.
- Add quick-action bar with existing handlers and error states.
- Validate offline/unavailable state presentation.

### Phase 3: UX and Hardening

**Focus**: resilience and polish.
**Tasks**:

- Optimize loading behavior for rapid card switching.
- Accessibility and keyboard flow verification.
- Visual and responsive polish for Steam Deck-like and narrow viewports.

## Decisions Needed

Finalized decisions:

1. **Card interaction semantics**
   - Decision: modal open also changes active profile.
   - Impact: affects state coupling and side effects across Launch/Profiles routes.
2. **V1 quick action scope**
   - Decision: minimal.
   - Impact: scope and validation complexity.
3. **Dismiss behavior**
   - Decision: Esc + outside click + close button.
   - Impact: usability versus accidental dismiss risk.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): external APIs and integration constraints.
- [research-business.md](./research-business.md): user stories, business rules, and workflows.
- [research-technical.md](./research-technical.md): architecture and file impact analysis.
- [research-ux.md](./research-ux.md): interaction, accessibility, and state design.
- [research-recommendations.md](./research-recommendations.md): phased strategy and risk mitigations.
