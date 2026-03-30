# Analysis - Context Synthesis (config-history-rollback)

## Problem and target outcome

The feature must let users inspect recent profile configuration history, compare revisions, and safely roll back when regressions happen. It directly supports issue `#46` by answering "what changed since last working config?" using a workflow that is fast enough for Steam Deck usage and safe under partial-failure conditions.

## Architectural direction

- Use metadata SQLite for revision history; keep TOML as live source of truth.
- Key all revisions by stable `profile_id` to preserve history across rename.
- Implement append-only, deduped revisions with bounded retention.
- Execute rollback through existing `ProfileStore` save path and metadata sync.

## Required vertical slice for MVP

1. Metadata schema + store APIs for revision append/list/get/prune/known-good marking.
2. Command hooks for snapshot capture on core write paths.
3. Command APIs for history listing, diff, and rollback.
4. Frontend history panel/modal and compare/restore UX with confirmation.
5. Tests for dedup, retention, rollback correctness, and degraded metadata behavior.

## Dependency highlights

- Metadata schema must land before command APIs can be finalized.
- Command API shapes should stabilize before frontend state/hooks integration.
- Rollback correctness and known-good behavior require launch/history integration.
- Security checks (ownership, integrity, limits) apply across core and command layers.

## Key risks to account for in tasking

- Write ordering races in rapid optimization save flows.
- Ambiguous launch success semantics for known-good tagging.
- Unbounded payload/diff operations without explicit limits.
- Metadata-unavailable path requiring clear user feedback and non-crashing behavior.
