# Task Structure Analysis: Proton App ID & Tri-Art System

## Executive Summary

The proton-app-id feature spans 4 phases across ~16 files (11 Rust, 5 TypeScript). Most
infrastructure exists — work is additive. Rust core changes are the bottleneck for each phase:
they define types and APIs that both the Tauri IPC layer and the frontend depend on. Within
each phase, Rust core + frontend can be parallelized once the core API surface is finalized.
Phase 4 (security hardening) is largely independent of Phases 2–3 and its tasks do not depend
on each other.

---

## Recommended Phase Structure

### Phase 1: Proton App ID + Art Normalization (7 tasks)

**Goal**: Wire `runtime.steam_app_id` end-to-end for proton_run profiles.

| Task                                                                                                       | Files                                                      | Parallel Group  | Depends On                   |
| ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- | --------------- | ---------------------------- |
| P1-T1: Add `steam_app_id` to `RuntimeSection` + `is_empty()` + `resolve_art_app_id()`                      | `profile/models.rs`                                        | A               | —                            |
| P1-T2: Update `profile_list_summaries` to return `effectiveSteamAppId` via `resolve_art_app_id`            | `commands/profile.rs`                                      | B               | P1-T1                        |
| P1-T3: Add `steam_app_id` to TS `GameProfile.runtime`, update `LibraryCardData`, create `src/utils/art.ts` | `types/profile.ts`, `types/library.ts`, `src/utils/art.ts` | B               | P1-T1 (type shape finalized) |
| P1-T4: Rebind proton_run "Steam App ID" field to `runtime.steam_app_id`, add numeric validation            | `components/profile-sections/RuntimeSection.tsx`           | C               | P1-T3                        |
| P1-T5: Update `useLibrarySummaries` to consume `effectiveSteamAppId` from IPC                              | `hooks/useLibrarySummaries.ts`                             | C               | P1-T2, P1-T3                 |
| P1-T6: Fix `GameCoverArt` null gate (returns null when only `customCoverArtPath` is set)                   | `components/profile-sections/GameCoverArt.tsx`             | B (independent) | —                            |
| P1-T7: Update `ProfileSummary` DTO tests + add `resolve_art_app_id` unit tests                             | `commands/profile.rs` (tests), `profile/models.rs` (tests) | C               | P1-T1, P1-T2                 |

**Phase 1 parallelism**: P1-T2 and P1-T3 can start in parallel once P1-T1 lands. P1-T6 is
fully independent of all other tasks. P1-T4 and P1-T5 require both P1-T2 and P1-T3 first.

---

### Phase 2: Tri-Art Custom Upload (9 tasks)

**Goal**: Per-type custom art paths with mix-and-match selection.

**Dependency**: Phase 1 must be complete.

| Task                                                                                                                                                | Files                                               | Parallel Group                | Depends On          |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------- | ----------------------------- | ------------------- |
| P2-T1: Add `custom_portrait_art_path` + `custom_background_art_path` to `GameSection` + `LocalOverrideGameSection`                                  | `profile/models.rs`                                 | A                             | Phase 1 done        |
| P2-T2: Update `effective_profile()`, `storage_profile()`, `portable_profile()` for two new art fields                                               | `profile/models.rs`                                 | A (same PR as P2-T1)          | P2-T1               |
| P2-T3: Generalize `import_custom_cover_art` → `import_custom_art(source_path, art_type)` with type-routed subdirs; preserve backward-compat wrapper | `game_images/import.rs`                             | A (parallel with P2-T1/P2-T2) | Phase 1 done        |
| P2-T4: Add `import_custom_art` Tauri command; register in `lib.rs`                                                                                  | `commands/game_metadata.rs`, `src-tauri/src/lib.rs` | B                             | P2-T3               |
| P2-T5: Update `profile_save` to auto-import all three art types                                                                                     | `commands/profile.rs`                               | B                             | P2-T1, P2-T2, P2-T4 |
| P2-T6: Update `profile_list_summaries` to include `customPortraitArtPath` in `ProfileSummary`                                                       | `commands/profile.rs`                               | B (can combine with P2-T5)    | P2-T1               |
| P2-T7: Update TS `GameProfile` types (new per-type art path fields in `game` + `local_override.game`)                                               | `types/profile.ts`                                  | B (parallel with P2-T4)       | P2-T1 (shape known) |
| P2-T8: Expand `MediaSection` to three art slots (Cover, Portrait, Background); invoke `import_custom_art`                                           | `components/profile-sections/MediaSection.tsx`      | C                             | P2-T4, P2-T7        |
| P2-T9: Fix S-03: clear all custom art paths in `sanitize_profile_for_community_export`                                                              | `profile/exchange.rs`                               | B (independent)               | P2-T1               |

**Phase 2 parallelism**: P2-T1/P2-T2 and P2-T3 can run in parallel. P2-T7 can start in
parallel with P2-T4 once the Rust struct shape from P2-T1 is known. P2-T9 is independent of
P2-T4 through P2-T8 and can be done at any time once P2-T1 lands.

---

### Phase 3: Background Art Infrastructure (6 tasks)

**Goal**: Add `GameImageType::Background` and wire through the full download/cache pipeline.

**Dependency**: Phase 2 must be complete.

| Task                                                                                                                                | Files                                     | Parallel Group                                | Depends On                 |
| ----------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------- | --------------------------------------------- | -------------------------- |
| P3-T1: Add `GameImageType::Background` variant to enum                                                                              | `game_images/models.rs`                   | A                                             | Phase 2 done               |
| P3-T2: Add `Background` match arms to `build_download_url()`, `filename_for()`                                                      | `game_images/client.rs`                   | A (same change wave as P3-T1)                 | P3-T1                      |
| P3-T3: Add `Background` arm to `build_endpoint()` in steamgriddb.rs                                                                 | `game_images/steamgriddb.rs`              | A (same change wave)                          | P3-T1                      |
| P3-T4: Add `"background"` match arm to `fetch_game_cover_art` IPC command; fix S-05 (unknown type now errors instead of defaulting) | `commands/game_metadata.rs`               | B                                             | P3-T1, P3-T2, P3-T3        |
| P3-T5: Fix S-01/S-06: add redirect-policy domain allow-list to `http_client()`                                                      | `game_images/client.rs`                   | B (independent once P3-T2 lands in same file) | — (can be Phase 4 instead) |
| P3-T6: Fix S-02: update `settings_load` IPC to return `has_steamgriddb_api_key: bool` instead of raw key                            | `commands/settings.rs`, `settings/mod.rs` | B (fully independent)                         | — (can be Phase 4)         |

**Phase 3 parallelism**: P3-T1, P3-T2, and P3-T3 touch separate files but form a coherent
atomic change; they should land together. P3-T5 and P3-T6 are fully independent of the enum
work and of each other.

---

### Phase 4: Security Hardening (4 tasks)

**Goal**: Address all WARNING-level security findings before ship.

**Note**: If S-01/S-06 and S-02 are done in Phase 3, this phase covers remaining items.

| Task                                                                                                            | Files                                                                          | Parallel Group | Depends On                                |
| --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ | -------------- | ----------------------------------------- |
| P4-T1: S-01/S-06: Redirect policy domain allow-list in `http_client()`                                          | `game_images/client.rs`                                                        | Independent    | —                                         |
| P4-T2: S-02: Filter API key at IPC boundary (`settings_load` returns `has_steamgriddb_api_key: bool`)           | `commands/settings.rs`, `settings/mod.rs`                                      | Independent    | —                                         |
| P4-T3: S-03: Clear all custom art paths in community export sanitizer                                           | `profile/exchange.rs`                                                          | Independent    | Phase 2 P2-T1 (needs new fields to exist) |
| P4-T4: S-12: Add `AuthFailure` error variant; fall back to Steam CDN on 401/403; surface "API key invalid" hint | `game_images/models.rs`, `game_images/client.rs`, `game_images/steamgriddb.rs` | Independent    | Phase 3 complete                          |

**Phase 4 parallelism**: All four tasks are independent of each other. P4-T3 can be folded
into Phase 2 as P2-T9 (recommended). P4-T1 and P4-T2 can be folded into Phase 3.

---

## Task Granularity Recommendations

### 1-file tasks (safest parallelism)

- P1-T6 `GameCoverArt.tsx` — one-line null-gate fix, no dependencies
- P2-T3 `game_images/import.rs` — self-contained generalization
- P2-T9 / P4-T3 `profile/exchange.rs` — add two `.clear()` calls
- P3-T3 `game_images/steamgriddb.rs` — add one match arm + one test
- P4-T1 `game_images/client.rs` (redirect policy only)
- P4-T2 `commands/settings.rs` + `settings/mod.rs` — two small files, logically one task

### 2-file tasks (tightly coupled changes)

- P1-T2 + tests: `commands/profile.rs` — `ProfileSummary` DTO change + `profile_list_summaries` update
- P2-T4: `commands/game_metadata.rs` + `src-tauri/src/lib.rs` — add command + register it
- P3-T1/P3-T2: `game_images/models.rs` + `game_images/client.rs` — enum variant + all match sites

### 3-file tasks (maximum granularity boundary)

- P2-T1/P2-T2: `profile/models.rs` only (GameSection + LocalOverrideGameSection + three merge methods — all in one file)
- P4-T4: `game_images/models.rs` + `game_images/client.rs` + `game_images/steamgriddb.rs` — auth error variant touches three files

---

## Dependency Analysis

```
Phase 1
  P1-T1 (models.rs: RuntimeSection + resolve_art_app_id)
    ├── P1-T2 (commands/profile.rs: ProfileSummary + profile_list_summaries)
    │     └── P1-T5 (hooks/useLibrarySummaries.ts)
    └── P1-T3 (types/profile.ts + library.ts + utils/art.ts)
          └── P1-T4 (RuntimeSection.tsx: rebind field + validation)
          └── P1-T5 (hooks/useLibrarySummaries.ts)

  P1-T6 (GameCoverArt.tsx: null gate) — no dependencies
  P1-T7 (tests) — depends on P1-T1 and P1-T2

Phase 2 (requires Phase 1)
  P2-T1/P2-T2 (models.rs: new art fields + merge logic)  ┐ parallel
  P2-T3 (import.rs: generalize import_custom_art)         ┘
    ├── P2-T4 (commands/game_metadata.rs + lib.rs: new command)
    │     └── P2-T8 (MediaSection.tsx: three slots)
    ├── P2-T5 (commands/profile.rs: auto-import all types)  ← also needs P2-T1
    ├── P2-T6 (commands/profile.rs: customPortraitArtPath in summary)
    ├── P2-T7 (types/profile.ts: new TS art fields)  ← also enables P2-T8
    └── P2-T9 (profile/exchange.rs: S-03 clear paths)  ← needs P2-T1

Phase 3 (requires Phase 2)
  P3-T1 (models.rs: Background variant)
    ├── P3-T2 (client.rs: Background in build_download_url, filename_for)
    ├── P3-T3 (steamgriddb.rs: Background in build_endpoint)
    │
  P3-T1 + P3-T2 + P3-T3 → P3-T4 (commands/game_metadata.rs: "background" arm + S-05)

  P3-T5 (client.rs: redirect policy) — independent
  P3-T6 (settings.rs + settings/mod.rs: API key filter) — independent

Phase 4 (security, mostly independent)
  P4-T1 = P3-T5 (if deferred)
  P4-T2 = P3-T6 (if deferred)
  P4-T3 = P2-T9 (if deferred)
  P4-T4 (AuthFailure variant) — requires Phase 3 for Background context
```

---

## File-to-Task Mapping

Every file that changes is assigned to exactly one primary task:

| File                                                   | Task                                                                                                        | Phase |
| ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- | ----- |
| `crates/crosshook-core/src/profile/models.rs`          | P1-T1 (RuntimeSection + resolve_art_app_id); P2-T1/P2-T2 (new art fields + merge)                           | 1, 2  |
| `crates/crosshook-core/src/profile/exchange.rs`        | P2-T9 / P4-T3 (S-03: clear custom art paths)                                                                | 2/4   |
| `crates/crosshook-core/src/game_images/models.rs`      | P3-T1 (Background variant); P4-T4 (AuthFailure variant)                                                     | 3, 4  |
| `crates/crosshook-core/src/game_images/client.rs`      | P3-T2 (Background in build_download_url + filename_for); P4-T1/P3-T5 (redirect policy); P4-T4 (AuthFailure) | 3, 4  |
| `crates/crosshook-core/src/game_images/import.rs`      | P2-T3 (generalize to import_custom_art)                                                                     | 2     |
| `crates/crosshook-core/src/game_images/steamgriddb.rs` | P3-T3 (Background in build_endpoint); P4-T4 (AuthFailure)                                                   | 3, 4  |
| `crates/crosshook-core/src/game_images/mod.rs`         | P2-T3 or P2-T4 (re-export import_custom_art)                                                                | 2     |
| `crates/crosshook-core/src/settings/mod.rs`            | P4-T2 / P3-T6 (API key redaction struct)                                                                    | 3/4   |
| `src-tauri/src/commands/game_metadata.rs`              | P2-T4 (import_custom_art command); P3-T4 ("background" arm + S-05)                                          | 2, 3  |
| `src-tauri/src/commands/profile.rs`                    | P1-T2 (ProfileSummary + effective app_id); P2-T5/P2-T6 (auto-import + portrait path)                        | 1, 2  |
| `src-tauri/src/commands/settings.rs`                   | P4-T2 / P3-T6 (return has_key bool)                                                                         | 3/4   |
| `src-tauri/src/lib.rs`                                 | P2-T4 (register import_custom_art)                                                                          | 2     |
| `src/types/profile.ts`                                 | P1-T3 (runtime.steam_app_id); P2-T7 (portrait + background art fields)                                      | 1, 2  |
| `src/types/library.ts`                                 | P1-T3 (add customPortraitArtPath when ready)                                                                | 1     |
| `src/utils/art.ts` (new)                               | P1-T3 (create resolveArtAppId + resolveCustomArtPath utilities)                                             | 1     |
| `src/hooks/useGameCoverArt.ts`                         | No changes needed — already accepts `imageType` param                                                       | —     |
| `src/hooks/useLibrarySummaries.ts`                     | P1-T5 (consume effectiveSteamAppId)                                                                         | 1     |
| `src/components/profile-sections/RuntimeSection.tsx`   | P1-T4 (rebind proton_run field to runtime.steam_app_id + validation)                                        | 1     |
| `src/components/profile-sections/GameCoverArt.tsx`     | P1-T6 (fix null gate bug)                                                                                   | 1     |
| `src/components/profile-sections/MediaSection.tsx`     | P2-T8 (expand to three art slots)                                                                           | 2     |

---

## Optimization Opportunities

### Parallelism Opportunities Within Each Phase

**Phase 1:**

- P1-T1 (Rust core) blocks nothing else on day 0; start immediately
- Once P1-T1 merges: P1-T2 (backend IPC) and P1-T3 (frontend types + utils) can run in parallel
- P1-T6 (GameCoverArt null gate) can run anytime — completely independent quick win

**Phase 2:**

- P2-T1/T2 (Rust profile model) and P2-T3 (Rust import generalization) touch separate modules — two parallel work streams in pure Rust
- P2-T7 (TS types) can start in parallel with P2-T4 (Tauri command) once P2-T1 merges
- P2-T9 (security export sanitizer) is a 2-line fix that can be batched with any Phase 2 Rust task

**Phase 3:**

- P3-T1/T2/T3 form one atomic Rust wave; P3-T5 and P3-T6 are security fixups that can happen on a separate track in parallel
- P3-T5 (redirect policy) modifies `client.rs` but doesn't touch the Background enum — can land independently

**Phase 4 (if deferred):**

- All four security tasks are independent of each other; assign to separate reviewers

### Folding Security Tasks Into Earlier Phases

Rather than a standalone Phase 4, consider:

- **P2-T9 = S-03** (export sanitizer) — natural fit during Phase 2 when new art fields are added
- **P3-T5 = S-01/S-06** (redirect policy) — natural fit during Phase 3 when `client.rs` is open for Background
- **P3-T6 = S-02** (API key filter) — small isolated change, can land in Phase 3 or earlier
- This leaves only **S-12** (AuthFailure) for a dedicated Phase 4 task, or it can be folded into P3-T4

### Avoid These Anti-Patterns

- **Do not** change `commands/profile.rs` in multiple simultaneous PRs — it is touched in P1-T2
  and P2-T5/P2-T6; serialize these or carefully coordinate merge order
- **Do not** split the `GameImageType::Background` variant change across PRs — all four match
  sites (models.rs, client.rs build_download_url, client.rs filename_for, steamgriddb.rs
  build_endpoint) must land atomically or the Rust compiler will catch it, but staggered PRs
  create noise
- **Do not** update `src/types/profile.ts` and the Rust `ProfileSummary` DTO in separate PRs
  without coordinating the field names — camelCase/snake_case mismatch risk

---

## Implementation Strategy Recommendations

### Ordering Within Phases

1. **Always start with Rust core** (`crosshook-core` crates) — these define the types and API
   surface that all other layers depend on. The Rust compiler enforces completeness.
2. **IPC layer second** (`src-tauri/commands/`) — thin wrappers; changes are small once core is done
3. **Frontend last** — types, hooks, components in that order; avoids chasing a moving TS type shape

### Test-Inclusive Tasks

Tests for a function must live in the same task as the function itself (per CLAUDE.md and
project convention). Do not create standalone "write tests" tasks. The test file co-location
pattern (`#[cfg(test)] mod tests` in the same `.rs` file, `MetadataStore::open_in_memory()` for
DB tests) is already established — follow it for all new Rust code.

### Phase 1 Quick Wins (Do First)

1. **P1-T6** (GameCoverArt null gate) — 2-line fix, no dependencies, ships immediate value to
   existing users with `customCoverArtPath` but no `steamAppId`
2. **P1-T1** (RuntimeSection + resolve_art_app_id) — foundational, unblocks everything else in Phase 1
3. **Smoke test first**: before writing P1-T1, verify manually whether existing proton_run
   profiles with `steam.app_id` already show portrait art in Library (feature-spec.md quick win #1)

### IPC Contract Discipline

`ProfileSummary` in `commands/profile.rs` uses `#[serde(rename_all = "camelCase")]` — any new
field added to this struct (e.g., `effective_steam_app_id`, `custom_portrait_art_path`) will
automatically serialize as `effectiveSteamAppId`, `customPortraitArtPath`. The corresponding TS
`ProfileSummary` interface in `useLibrarySummaries.ts` must match exactly. Both sides change in
P1-T2 + P1-T5 (Phase 1) and P2-T6 + P2-T7 (Phase 2).

### Backward Compatibility Gate

All new `RuntimeSection`, `GameSection`, and `LocalOverrideGameSection` fields use
`#[serde(default, skip_serializing_if = "String::is_empty")]`. The `is_empty()` implementations
for `RuntimeSection` must **not** include `steam_app_id` in the emptiness check — a profile
with only `steam_app_id` set must still write the `[runtime]` section. This is a subtle
correctness requirement; add an explicit test for it in P1-T1.
