# Task Structure Analysis: protondb-lookup

## Executive Summary

The work breaks cleanly into four phases: burn down the backend contract/risk first, expose a thin IPC surface, wire a dedicated editor card, then close with docs and verification. The most important sequencing rule is that exact-tier modeling and cache/fallback behavior must be defined before UI work begins, because those choices affect styling, copy, and recommendation-application semantics across the entire feature. The only part that remains externally risky is richer recommendation aggregation from ProtonDB’s undocumented report feed, so that work should stay encapsulated and summary-first fallback should be treated as mandatory.

## Proposed Phase Structure

### Phase 1: Core Lookup and Cache Boundary

- Define exact ProtonDB tier and normalized DTO contracts.
- Implement cache-backed summary fetch and recommendation aggregation behind a fallback boundary.
- Add Rust coverage for parsing, cache, and safe suggestion normalization.

### Phase 2: IPC and Frontend State

- Expose a thin `protondb_lookup` Tauri command.
- Mirror DTOs in TypeScript and build an invoke-driven hook.
- Add dedicated exact-tier styling primitives without disturbing existing compatibility badges.

### Phase 3: Profile Editor Integration

- Build a dedicated ProtonDB card component.
- Compose it into the profile editor near Steam metadata.
- Wire explicit copy/apply actions through existing launch/custom-env surfaces.

### Phase 4: Docs and Verification

- Update quickstart and feature docs.
- Run Rust and TypeScript verification plus manual Tauri regression checks.
- Record outcomes in `tasks/todo.md`.

## Candidate Task Breakdown

### Phase 1

- `1.1` Define backend ProtonDB contracts and export surface.
- `1.2` Implement cache-backed fetch/normalize logic.
- `1.3` Add backend tests for tier mapping, stale cache, and safe recommendation parsing.

### Phase 2

- `2.1` Add `protondb_lookup` Tauri command and registration.
- `2.2` Add frontend ProtonDB types and `useProtonDbLookup`.
- `2.3` Extend `theme.css` with exact-tier and stale/unavailable panel states.

### Phase 3

- `3.1` Build `ProtonDbLookupCard`.
- `3.2` Mount the card inside `ProfileFormSections` / `ProfilesPage`.
- `3.3` Implement explicit copy/apply merge behavior for supported suggestions.

### Phase 4

- `4.1` Update user docs.
- `4.2` Run verification and record the closeout.

## Dependency Notes

- `1.1` must precede any IPC or styling work because exact-tier names and DTO shape are foundational.
- `1.2` must precede `2.1`; the Tauri command should not invent lookup logic of its own.
- `2.3` can run in parallel with `2.1` / `2.2` once exact-tier names from `1.1` are known.
- `3.3` depends on both the card UI and a stable normalized suggestion shape from the backend.

## Parallelization Notes

- Backend parser/cache tests (`1.3`) can run independently from IPC registration after `1.2`.
- CSS exact-tier work (`2.3`) can proceed independently of Tauri registration.
- Docs can start as soon as the final UI placement and behavior are stable.

## Risks That Must Shape Task Order

- Do not start recommendation apply flows until raw launch-option parsing is safely constrained.
- Do not wire UI copy around `CompatibilityRating`; exact ProtonDB tiers need a separate contract.
- Do not let richer report aggregation block the stable summary tier path.
