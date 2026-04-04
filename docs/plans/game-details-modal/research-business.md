# Game details modal - business research

## Executive Summary

CrossHook library users currently click cards for selection and use footer actions for launch/favorite/edit. The feature introduces a read-only details modal opened from card body interaction so users can inspect game and profile context without leaving Library. This should preserve existing action-button behavior, remain usable offline with cached/unavailable states, and avoid adding new persistence.

## User Stories

- As a library user, I want a fast details view from a card so I can inspect setup and status without navigation.
- As a frequent launcher user, I want Launch/Favorite/Edit to behave exactly as they do now.
- As a keyboard and controller user, I want Esc/back and outside-click dismissal that returns me to my prior Library context.
- As an offline user, I want clear cached vs unavailable indicators so I trust what I see.

## Business Rules

1. The details modal is read-only and does not replace edit workflows.
2. Card-body interaction opens modal; footer controls keep existing behavior and do not open modal.
3. Data is assembled from existing sources (profile summary, metadata snapshots, health/offline status, and cached external lookups where available).
4. Missing data must be shown as unavailable/unknown, not silently omitted.
5. No storage schema or settings changes for initial scope.

## Workflows

### Primary flow

1. User clicks card body in Library.
2. Modal opens with sectioned details and quick actions.
3. User closes via Esc, outside click, or close button, returning to Library context.

### Existing action flow

1. User clicks Launch/Favorite/Edit in card footer.
2. Existing flows run unchanged.
3. Modal is not opened by those controls.

### Degraded/offline flow

1. User opens modal while some data is unavailable.
2. App renders cached values when present.
3. App renders explicit unavailable states where data cannot be resolved.

## Domain Concepts

- Profile summary: lightweight list/card model used by Library.
- Aggregated details: normalized read-only presentation of game, trainer, activity, and organization context.
- Quick actions: shortcuts that trigger existing routes/commands.
- Availability state: per-section loaded/cached/unavailable status.

## Success Criteria

- Card-body opens details modal reliably without regressing footer actions.
- Modal provides meaningful aggregated context with explicit empty/error states.
- Dismissal and focus behavior are consistent with existing modal patterns.
- Offline and partial-data behavior remains understandable and stable.
- No new persisted data introduced for the initial release.

## Open Questions

- Should opening details also select/activate profile context, or stay purely inspect-only until user takes an action?
- Which quick actions ship in v1 (Launch/Edit only vs Export/Copy options too)?
- What activity depth is expected for v1 (single latest launch vs summary metrics)?
- Should list-view interactions mirror the same details entry point if/when list mode is active?
