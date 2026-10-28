# YAN-835 (GitHub #543) — M1-34

_Paths are relative to_ `src/crosshook-native/`_._

## Context

The source commands are:

- `src-tauri/src/commands/community.rs` (7):
  - tap add, with dedupe and settings persistence;
  - list, export, import and prepare-import;
  - `community_sync`, a sync command that runs `git` through `host_std_command_with_env` on the main thread;
  - the indexed list.
- `lutris.rs` (2).
- `discovery.rs` (6).
- `protondb.rs`: lookup, get suggestions and dismiss. Accept moved in M1-06.

Community import performs **workspace-containment checks** on import paths (security), and no test protects them.

## Scope

- `services/community.rs`:
  - tap add and remove with dedupe;
  - sync as a job, followed by indexing, the offline-state upsert and the `last_sync_at` backfill;
  - the indexed list;
  - prepare and import through the M1-06 `ProfileWriter`;
  - community profile export.
- Factor containment into one `ensure_within_workspace(root, candidate)` and use it for every import path.
- `services/lutris.rs`: prepare and import.
- `services/discovery.rs`: catalog, external search, version compatibility, and external sources add, list and remove.
- `services/protondb.rs`: lookup and suggestions with the existing cache.

## Storage boundary

- The tap list in TOML settings is unchanged.
- SQLite `community_profiles` (including `trainer_loading_mode`, v25) and `external_cache_entries` are unchanged.
- Sync progress is runtime-only.

## Persistence & usability

- No migration.
- Offline:
  - Sync and external search report `Unavailable`.
  - Indexed lists and cached ProtonDB data still render, with stale markers.

## Acceptance criteria

- [ ] Containment tests reject:
  - `..` traversal;
  - absolute paths outside the workspace;
  - symlinks that resolve outside the workspace;
  - case and Unicode-normalization tricks.

  They accept nested valid paths.

- [ ] `community_sync` no longer blocks the caller thread. Sync can be cancelled.
- [ ] Tests cover tap dedupe and the backfill of `last_sync_at` and `trainer_loading_mode`.
- [ ] The Tauri commands delegate. Community Browse, Discover and Compatibility smoke tests pass.

## Test plan

- `cargo test -p crosshook-app community discovery protondb`, using a local bare git repo fixture as the tap
- `npm run test:smoke`

---

**Plan key:** `M1-34` · **Release:** v0.7.0-alpha.1 · **Estimate:** 5 pts · **GitHub labels:** `type:refactor`, `area:security`, `area:profiles`, `priority:high` · **Blocked by:** M1-04, M1-06
