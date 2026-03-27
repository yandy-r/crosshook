# SQLite3 Addition - Business Analysis

## Executive Summary

CrossHook already has a clear canonical data model for profile content: TOML files that are user-readable, git-friendly, and scriptable. The business opportunity for SQLite is not replacing that model, but adding a durable relationship and history layer that makes CrossHook smarter about identity, launcher lifecycle, community indexing, diagnostics, favorites, collections, and long-term usage intelligence. The key product rule is a strict authority split: files remain authoritative for editable runtime artifacts and profile payloads, while SQLite becomes authoritative for stable IDs, relationships, derived indexes, event history, and cached external metadata.

Code review confirms there is currently **no stable profile ID** in the codebase — profiles are identified exclusively by their TOML filename stem. `last_used_profile` in settings is stored by name and must be updated on rename. The launcher slug is derived from the display name at export time. There is no audit log, no launch history, and no recency tracking beyond raw file-path lists in `RecentFilesStore`.

---

## User Stories

### Primary User: power user managing many local profiles

- As a user, I want each profile to have a stable internal identity even if I rename the profile file so that favorites, usage history, and launcher mappings do not break.
- As a user, I want CrossHook to remember launcher relationships separately from profile names so that exported launcher drift can be detected and repaired.
- As a user, I want launch attempts, outcomes, timestamps, and diagnostics preserved historically so that I can understand what changed when a setup starts failing.
- As a user, I want favorites, collections, and recents to survive profile renames and file moves.
- As a user, I want recently used game paths, trainer paths, and DLL paths to be surfaced without relying on filesystem scans that filter out moved files silently.
- As a user, I want a profile duplication to produce a copy that is recognized as derived from its source rather than appearing as a wholly unrelated profile.

### Secondary User: CrossHook product owner / maintainer

- As a maintainer, I want a local event history and derived metadata graph so that new UI features do not need to rescan TOML, tap workspaces, and logs repeatedly.
- As a maintainer, I want cached ProtonDB, cover art, and community manifest metadata stored locally so that the app feels faster and can work offline after the first sync.
- As a maintainer, I want a reliable sync boundary between filesystem state and derived local intelligence so that the system stays debuggable.
- As a maintainer, I want community tap HEAD commits tracked so that re-indexing can be skipped when nothing changed.

---

## Business Rules

### Core Authority Rules

1. **TOML Canonicality**: Profile content that users edit and share remains authoritative in TOML. SQLite never becomes the source of truth for executable paths, trainer paths, launch methods, or exported script contents. The TOML fields (`game`, `trainer`, `injection`, `steam`, `runtime`, `launch`) are never shadowed by SQLite.
2. **Stable Identity Rule**: Every profile gets a stable internal ID independent from filename, display name, launcher display name, or slug. Currently filenames are the only identity — adding SQLite supplies the stable UUID layer.
3. **Relationship Authority Rule**: Cross-entity relationships that do not belong naturally inside TOML — favorites, collections, launcher mappings, usage history, cached metadata references, tap/profile joins — should live in SQLite.
4. **Event Immutability Rule**: Launch records, sync records, rename history, and diagnostic history are append-only event logs; corrections happen through new events, not silent mutation.
5. **Derived State Rule**: Health, staleness, most-used, last-success, launcher drift, and cache freshness are derived fields that may be recomputed from events plus current scans.
6. **External Drift Rule**: If users rename or move launchers outside CrossHook, the app should detect drift, preserve prior linkage history, and surface remediation rather than deleting the record.
7. **Offline-First Rule**: Cached external metadata is optional and stale-tolerant. Missing cache data must never block launching or editing a profile.
8. **Explainability Rule**: When SQLite-derived state disagrees with the filesystem, CrossHook should be able to explain which authority won and why.

### Edge Case Rules (derived from code review)

9. **Profile Name Constraint Rule**: Profile names must not contain `< > : " / \ | ? *`, must not be empty, `.`, or `..`, and must not form an absolute path. SQLite identity rows must enforce the same constraints when reconciling filesystem state. The copy suffix format is `{base} (Copy)` / `{base} (Copy N)` — duplicates of copies produce `{original} (Copy N)` not nested `(Copy) (Copy)`.
10. **Launcher Watermark Rule**: CrossHook only deletes launcher files it generated, verified by watermarks (`# Generated by CrossHook` in `.sh`, `Generated by CrossHook` in `.desktop`). SQLite must not record an artifact as "owned" if the watermark verification would fail at delete time.
11. **Rename Cascade Rule**: When a profile is renamed, the current code already performs a cascade: deletes old launcher files (best-effort), updates `steam.launcher.display_name` inside the TOML, and updates `last_used_profile` in settings. SQLite must integrate into this same cascade to append rename events and update the identity projection in a single logical operation.
12. **Delete Cascade Rule**: Profile deletion performs best-effort launcher cleanup before removing the TOML file. SQLite identity rows must not be hard-deleted when a profile file is removed — they become tombstones for history preservation.
13. **Native Launch Method Rule**: Launcher export and launcher cleanup are skipped for profiles with `launch.method = "native"`. SQLite launcher artifact records only apply to `steam_applaunch` and `proton_run` profiles.
14. **RecentFiles Canonicality Rule**: The current `RecentFilesStore` filters out non-existent paths on every load, silently dropping references to moved files. SQLite should replace this with stable path history that survives file moves.
15. **Tap Sync Idempotency Rule**: Community tap syncs are git-based (`clone` or `fetch --prune` + `reset --hard FETCH_HEAD` + `clean -fdx`). The HEAD commit is captured via `rev-parse HEAD` after sync. SQLite should track HEAD commit per tap so re-indexing can skip taps where HEAD has not changed.
16. **TrainerLoadingMode Rule**: `TrainerLoadingMode::SourceDirectory` vs `CopyToPrefix` affects how the trainer is made accessible in the Proton prefix. SQLite launch records must capture this mode along with the resolved method so diagnostics can attribute failures to loading mode issues.

---

## Workflows

### Profile Create/Import
1. User creates a new profile via the ProfileEditor UI.
2. `profile_save` Tauri command writes TOML via `ProfileStore::save`.
3. SQLite: generate stable profile ID → upsert `profile_identity` row → record creation event with initial file path and display name.

### Profile Import Legacy
1. User imports a `.profile` file via `profile_import_legacy`.
2. `ProfileStore::import_legacy` converts the legacy format (Windows paths, INI-style fields) to `GameProfile` and saves TOML.
3. SQLite: treat as a new profile creation — same identity provisioning as fresh create.

### Profile Rename
1. User renames via the profile list UI.
2. `profile_rename` Tauri command:
   a. Loads old profile TOML (to capture display name and launcher info).
   b. `ProfileStore::rename` renames the TOML file.
   c. Deletes old launcher files using `delete_launcher_for_profile` (best-effort, watermark-gated).
   d. Updates `steam.launcher.display_name` in the renamed TOML.
   e. Updates `last_used_profile` in settings if it matched the old name.
   f. Returns `bool` indicating whether launchers were cleaned up.
3. SQLite: append `profile_name_history` event → update canonical name/path projection → mark any launcher artifact for the old slug as needing re-export.

### Profile Duplicate
1. User duplicates a profile via `profile_duplicate`.
2. `ProfileStore::duplicate` generates a unique copy name (suffix `(Copy)` / `(Copy N)`, stripping existing copy suffixes to avoid nesting), then saves a byte-for-byte clone.
3. SQLite: create new stable ID for the copy → record `source_profile_id` foreign key in identity row → emit creation event tagged as `duplicated_from`.

### Profile Delete
1. User deletes via `profile_delete` Tauri command.
2. Best-effort launcher cleanup via `cleanup_launchers_for_profile_delete` (skipped for native profiles).
3. `ProfileStore::delete` removes the TOML file.
4. SQLite: soft-delete identity row (tombstone) → record deletion event with timestamp and optional launcher cleanup result → preserve all history rows.

### Launcher Export
1. User clicks "Export Launcher" in the UI.
2. Launcher `.sh` and `.desktop` files are written to `~/.local/share/crosshook/launchers/{slug}-trainer.sh` and `~/.local/share/applications/crosshook-{slug}-trainer.desktop`.
3. SQLite: upsert `launcher_artifact` row with profile ID, slug, resolved display name, and expected paths → record export event with timestamp.

### External Launcher Drift Detection
1. CrossHook checks whether a launcher still exists via `check_launcher_for_profile`.
2. Staleness is detected by: (a) comparing `Name=` in the `.desktop` against the expected display name, (b) comparing full `.sh` content against rebuilt content.
3. SQLite: compare current disk state against stored expected paths and slug → emit drift event if mismatch → surface repair action in UI if confidence is high.

### Launch Execution
1. User initiates game or trainer launch.
2. `launch_game` or `launch_trainer` Tauri command spawns a child process and streams log lines to the UI via `launch-log` events.
3. On process exit, `analyze()` runs pattern matching against the log tail to produce a `DiagnosticReport`.
4. `launch-diagnostic` event is emitted if the report should be surfaced; `launch-complete` event carries exit code and signal.
5. SQLite: create `launch_operation` row at start → update with exit code, signal, method, `trainer_loading_mode`, log path reference, and `DiagnosticReport` summary on completion.

### Community Tap Sync
1. User adds a tap subscription or triggers refresh.
2. `CommunityTapStore::sync_tap` runs `git clone` or `git fetch --prune origin {branch}` + `reset --hard FETCH_HEAD` + `clean -fdx`.
3. After sync, `rev-parse HEAD` captures the HEAD commit hash. `index_tap` recursively scans for `community-profile.json` files.
4. `CommunityTapSyncResult` captures `workspace`, `status` (Cloned|Updated), `head_commit`, and `index`.
5. SQLite: compare HEAD commit against stored value → if unchanged, skip re-indexing → if changed, upsert manifest rows, update HEAD record, emit sync event.

### Error Recovery
- Launcher cleanup failure on rename/delete: logged as warning, does not block TOML mutation. SQLite records the failure outcome in the event row for later reconciliation.
- Profile TOML missing at rename time: `profile_rename` loads profile with `.ok()` — returns `None` if missing. Rename proceeds; SQLite reconciler should flag orphaned launcher records at next health scan.
- Tap sync git failure: `CommunityTapError::Git` propagates to Tauri as a string error. SQLite records the failed sync attempt with error message so the UI can show "last successful sync" rather than showing nothing.

---

## Domain Model

### Entities (to be created in SQLite)

| Entity | Key Fields | Authority |
|---|---|---|
| `profile_identity` | `id` (UUID), `current_file_name`, `display_name`, `created_at`, `deleted_at` | SQLite |
| `profile_name_history` | `profile_id`, `old_name`, `new_name`, `event_at` | SQLite (append-only) |
| `launcher_artifact` | `profile_id`, `launcher_slug`, `display_name`, `script_path`, `desktop_entry_path`, `last_exported_at`, `is_stale` | SQLite |
| `launch_operation` | `id`, `profile_id`, `method`, `trainer_loading_mode`, `exit_code`, `signal`, `log_path`, `diagnostic_summary`, `started_at`, `finished_at` | SQLite |
| `community_tap_state` | `tap_url`, `tap_branch`, `head_commit`, `last_sync_at`, `last_sync_status` | SQLite |
| `community_catalog_entry` | `tap_url`, `relative_path`, `game_name`, `trainer_name`, `compatibility_rating`, `schema_version`, `manifest_json_cache` | SQLite |
| `recent_file_entry` | `path_type` (game/trainer/dll), `file_path`, `last_used_at` | SQLite (replaces TOML-based `RecentFilesStore`) |
| `collection_membership` | `collection_name`, `profile_id` | SQLite |
| `external_metadata_cache` | `cache_key`, `source`, `payload`, `fetched_at`, `expires_at` | SQLite |
| `derived_health_state` | `profile_id`, `health_status`, `staleness_flags`, `last_computed_at` | SQLite (derived) |

### Existing Rust Types Mapped to Domain

| Rust Type | Module | SQLite Role |
|---|---|---|
| `GameProfile` | `profile/models.rs` | TOML canonical — never replaced; SQLite stores ID linkage only |
| `LauncherInfo` | `export/launcher_store.rs` | Snapshot type; SQLite `launcher_artifact` persists it durably |
| `LauncherDeleteResult` | `export/launcher_store.rs` | Event payload for delete outcome events |
| `LauncherRenameResult` | `export/launcher_store.rs` | Event payload for rename cascade outcomes |
| `CommunityTapSyncResult` | `community/taps.rs` | Contains `head_commit` — key for skip-on-unchanged optimization |
| `CommunityProfileIndexEntry` | `community/index.rs` | Source for `community_catalog_entry` rows |
| `DiagnosticReport` | `launch/diagnostics` | Serialized summary stored in `launch_operation` |
| `RecentFilesData` | `settings/recent.rs` | Replaced by `recent_file_entry` with timestamps |
| `AppSettingsData` | `settings/mod.rs` | `last_used_profile` by name — must be updated in rename cascade |

### State Transitions

**profile_identity lifecycle**:
`created` → `renamed` (N times) → `deleted` (soft, tombstone preserved)

**launcher_artifact lifecycle**:
`exported` → `stale` (drift detected) → `repaired` (re-exported) | `deleted` (via profile delete or manual)

**launch_operation lifecycle**:
`started` → `succeeded` | `failed` | `aborted` (signal)

**community_tap_state lifecycle**:
`never_synced` → `synced` (HEAD recorded) → `stale` (remote advanced) → `re-synced`

---

## Existing Codebase Integration

### File Locations
| Store | Location | Format |
|---|---|---|
| Profiles | `~/.config/crosshook/profiles/{name}.toml` | TOML |
| Settings | `~/.config/crosshook/settings.toml` | TOML |
| Recent files | `~/.local/share/crosshook/recent.toml` | TOML |
| Community taps | `~/.local/share/crosshook/community/taps/{slug}/` | Git workspaces |
| Launcher scripts | `~/.local/share/crosshook/launchers/{slug}-trainer.sh` | Shell scripts |
| Desktop entries | `~/.local/share/applications/crosshook-{slug}-trainer.desktop` | XDG |
| Launch logs | `~/.local/share/crosshook/logs/` (inferred from `create_log_path`) | Text |

### Integration Points That Affect SQLite Design

1. **`ProfileStore::rename`** (core): Renames the TOML file only — no ID awareness. The Tauri command layer adds launcher cleanup and settings update. SQLite integration must hook here to append rename events.
2. **`ProfileStore::delete`** (core): Removes TOML file. Tauri layer adds best-effort launcher cleanup. SQLite must hook here to soft-delete identity and record tombstone.
3. **`ProfileStore::duplicate`** (core): Returns `DuplicateProfileResult { name, profile }`. SQLite must create a new ID with `source_profile_id` backlink.
4. **`profile_rename` Tauri command**: Already coordinates TOML rename + launcher delete + settings update. SQLite event write should be added to this same transaction scope to avoid partial state.
5. **`CommunityTapStore::sync_tap`**: Returns `head_commit` after sync. SQLite should receive this as input to decide whether to skip re-indexing.
6. **`check_launcher_for_profile`** / **`check_launcher_exists_for_request`**: Current staleness detection compares live file vs expected. SQLite `launcher_artifact` can cache last-known good state for offline queries.
7. **`RecentFilesStore`**: Stored in `~/.local/share/crosshook/recent.toml`. Filters non-existent paths on load. Capped at 10 per category (game, trainer, dll). SQLite `recent_file_entry` with timestamps would replace this entirely and survive file renames.

### What Does Not Exist Yet (gaps SQLite fills)

- No stable profile UUID anywhere in the codebase — identity is always filename-based.
- No launch history — outcomes are emitted as events to the UI only, not persisted.
- No community tap HEAD tracking — every tap browse rescans all JSON files.
- No favorites or collections — not referenced anywhere in current code.
- No rename history log — the rename cascade updates in place and discards the old name.
- No diagnostic persistence — `DiagnosticReport` is emitted to the UI and lost.

---

## Success Criteria

- A profile rename does not orphan favorites, collections, launcher mappings, or usage history.
- CrossHook can distinguish profile identity from profile filename and launcher naming.
- Launch history answers "what failed, when, and with what diagnostics" locally without parsing raw log files every time.
- Community tap browsing can query indexed metadata efficiently without repeated recursive manifest scans; syncs skip re-indexing when HEAD commit is unchanged.
- The system can reconcile TOML/filesystem changes into SQLite without hidden authority conflicts.
- `last_used_profile` in settings and SQLite identity agree after every rename — no divergence.
- Launcher cleanup failure on profile delete/rename does not block profile operations and is recorded for later reconciliation.

---

## UX-Driven Business Rule Additions

Resolved from UX research findings. These add new rules or sharpen existing ones.

### UX-1: Externally Deleted Profile Display (Anti-Ghost-Profile Rule)

When a profile TOML file is removed outside CrossHook, the SQLite identity row must NOT present as a normal loadable profile. It becomes a tombstone displayed as "Removed from filesystem" with three actions: **Delete** (purge SQLite record and any associated history), **Restore** (re-create the TOML from the last known content if available, otherwise surface the editor with defaults), **Archive** (keep tombstone in history, suppress from active profile list). Silently retaining ghost profiles as if they were launchable is explicitly prohibited.

### UX-2: Launcher Drift Repair is Never Silent

Auto-repair of launcher drift (re-exporting to the current expected path) must never execute without visible feedback. If confidence is high (e.g. profile ID matches, slug derivation is deterministic), show a **soft confirmation banner with a timed undo window** before taking action. If confidence is low (e.g. display name changed and slug no longer matches), require explicit user action. The `is_stale` flag returned by `check_launcher_for_profile` is the trigger; the remediation level is determined by confidence.

### UX-3: Batch Drift Scan, Not Per-Launcher Notification

The drift reconciliation scan must collect all drifted launchers in a single pass before emitting any notification. Business rule: drift detection is a batch operation — one scan event, one grouped summary notification. Never emit individual drift alerts per launcher. This means the scan must be triggered explicitly (startup, profile list open, manual refresh) not reactively per launcher check.

### UX-4: SQLite Corrupt/Unavailable → TOML-Only Mode

Confirmed from RF-3. A corrupt or unavailable SQLite database must never block launching or editing a profile. The app degrades gracefully to TOML-only mode for all core operations. SQLite-dependent features (launch history, collections, drift detection, community catalog cache) are individually suppressed. Recovery runs in the background without user intervention. The "SQLite disabled" state is held in Tauri managed app state (in-memory flag), not in SQLite itself.

### UX-5: Collections Write — Optimistic with Undo Window

Collection and favorites writes to SQLite are **optimistic**: the UI updates immediately and the write confirms in the background. On write failure, the UI rolls back visually with an inline error. Destructive metadata actions (remove from collection, delete favorite) within the same session have a **30-second undo window**. This is a new business rule — no undo window exists anywhere in the current codebase and must be implemented alongside collections support.

### UX-6: Community Tap and Cache Freshness Defaults

Default staleness thresholds (all configurable in `AppSettingsData`):
- ProtonDB / external metadata cache: **48 hours**
- Community tap index: **7 days** (triggers "stale" badge in CommunityBrowser; does not auto-sync)

These become new fields in `AppSettingsData` or a new `CacheSettingsData` struct, persisted in `settings.toml`. SQLite `external_metadata_cache` and `community_tap_state` rows use these thresholds when computing freshness in queries.

### UX-7: File Watcher for External Profile Changes (New Capability)

A file watcher detecting external rename/delete of TOML files is a **new capability** not present in the current codebase. The current rename cascade is entirely UI-triggered via the Tauri `profile_rename` command. If a file watcher is implemented, it must trigger the same cascade logic as the Tauri command (launcher cleanup, settings update, SQLite event). This is a Phase 2+ feature. Phase 1 uses retroactive detection (next app open, profile list refresh) with explicit disambiguation when multiple filesystem candidates could match a renamed profile.

---

## Technical Constraints Affecting Business Rules

Confirmed by tech-designer analysis. These constraints close remaining ambiguities in the authority split and cascade design.

### TC-1: Profile Identity Matching (Bootstrapping)

No content hash is required for first-run identity assignment. Filename (TOML stem) is sufficient as the bootstrapping key because `ProfileStore` already guarantees uniqueness-by-filename — two profiles cannot coexist with the same name. The UUID is generated on first observation and stored in SQLite keyed by filename. Content hashes are out of scope for identity but could be used later as a derived health check (detecting out-of-band edits).

### TC-2: Launcher Identity

No persistent launcher ID exists today. The natural SQLite primary key for `launcher_artifact` is `(profile_id, launcher_slug)` — slug is derived from display name + steam_app_id + trainer_path at export time and changes on profile rename. Business rule confirmed from RF-2: slug change on rename means old row is tombstoned and a new row is created on re-export. SQLite never tries to keep a "same" launcher across a slug change.

### TC-3: Community Tap Identity

`community_tap_state` primary key is `(tap_url, tap_branch)` — matching `CommunityTapSubscription`. `tap_branch = NULL` maps to the `DEFAULT_TAP_BRANCH` default. `head_commit` is the version marker for skip-on-unchanged optimization.

**Authority split for tap subscriptions**: `AppSettingsData.community_taps` in `settings.toml` remains the canonical subscription list. SQLite `community_tap_state` stores only the sync state (HEAD, last sync time, status) and catalog cache for known subscriptions. SQLite never adds or removes subscriptions — only reflects sync outcomes for subscriptions that already exist in TOML.

### TC-4: SQLite Integration Point is the Tauri Command Layer

`ProfileStore` is synchronous file I/O with no hooks or callbacks. SQLite writes must be triggered from Tauri commands — not from inside `ProfileStore` methods. This is the correct layering: core store handles TOML, Tauri command handles cascade. Business implication: SQLite operations follow the same best-effort pattern as launcher cleanup and settings update — they must not block or roll back the primary file operation. A failed SQLite write after a successful TOML rename is logged and surfaced via a health reconciliation scan at next startup, not returned as a command error.

### TC-5: Launch Persistence Gap is Real

`LaunchResult` and `DiagnosticReport` are emitted as Tauri events and discarded after the session. SQLite fills this gap entirely. The `launch-complete` event (exit code + signal) and `launch-diagnostic` event (full `DiagnosticReport`) together provide all fields needed to populate a `launch_operation` row. Business rule: both events must be captured — a `launch_operation` row without a terminal outcome is valid (process still running or app was killed) and should be marked `status = 'incomplete'`.

### TC-6: community_sync is the Natural Upsert Point

`CommunityTapStore::sync_many` returns `Vec<CommunityTapSyncResult>` containing the full `CommunityProfileIndex` per tap. The Tauri `community_sync` command is the correct place to upsert catalog rows after a successful sync. Business rule confirmed from RF-4: upsert only runs when `head_commit` differs from the stored value.

---

## Risk Factor Resolutions

Raised by codebase analysis. These resolve ambiguous business rules that affect implementation strategy.

### RF-1: ID vs Name Transition Strategy

**Risk**: Profile names are the primary key everywhere today — Tauri commands, frontend state, settings, recent files. Introducing stable IDs creates a dual-lookup period.

**Resolution**: IDs are **backend-only in Phase 1**. The frontend continues to reference profiles by name for the entire first release. SQLite identity rows are assigned and maintained server-side, but no Tauri command changes its signature — `name: &str` remains the public API. ID-based frontend references (e.g., navigating by UUID) are deferred to Phase 2+ when the UI is explicitly updated to consume them. This avoids a flag day and keeps the first SQLite release non-breaking.

### RF-2: Launcher Slug Divergence on Profile Rename

**Risk**: Launcher slugs derive from display names. If renaming a profile decouples identity from name, it's unclear whether slugs follow or diverge.

**Resolution**: **Slugs follow display names — the existing behavior is the rule.** When a profile is renamed, the current code already: (1) deletes the old launcher files (best-effort, watermark-gated), (2) updates `steam.launcher.display_name` in the TOML to the new name. The new launcher export will then use a new slug derived from the new name. There is no in-place launcher rename — old artifact is deleted, user re-exports to create the new artifact. SQLite must record this as: old `launcher_artifact` row is tombstoned at rename time, new row is created on next export. The `LauncherRenameResult` type in `launcher_store.rs` exists but is used for in-place file rename (old slug → new slug path on disk), not profile rename. These are distinct operations.

### RF-3: First-Run Bootstrapping Strategy

**Risk**: Existing users have no SQLite database, no history, and no launcher mappings.

**Resolution**:
- **Profiles**: On first run, scan `~/.config/crosshook/profiles/` and create one `profile_identity` row per `.toml` file. Assign a new UUID. Record a synthetic `created` event with timestamp = file mtime (best approximation). No prior history is fabricated beyond this initial observation.
- **Launchers**: Do **not** attempt to retroactively map existing launcher files to profiles. Launcher artifacts discovered on disk without a SQLite record are treated as "untracked external launchers." The watermark check already protects against accidental deletion. They become tracked only after the user explicitly exports a launcher while SQLite is active.
- **Launch history**: Starts empty. No synthetic launch events are created for prior runs.
- **RecentFiles**: Migrate the existing `recent.toml` contents into SQLite `recent_file_entry` rows on first run (preserve ordering, use current timestamp for all migrated entries). Delete `recent.toml` after successful migration so there is no dual-write period.
- **Failure mode**: If bootstrapping fails partially, the app must start normally. SQLite is additive — the app falls back to name-based operation on any SQLite error during startup. A "SQLite not ready" flag suppresses ID-dependent features (history, collections, drift detection) until the DB is healthy.

### RF-4: Community Tap Indexing — Augment vs Replace

**Risk**: Current `index_taps()` does a full recursive scan every time. Should SQLite replace this or sit alongside it?

**Resolution**: **SQLite augments, not replaces.** The git workspace scan remains the source of truth. SQLite is a cache layer on top of it. The skip-on-unchanged optimization works as follows: after sync, compare `head_commit` to stored value → if equal, return cached `CommunityProfileIndex` from SQLite → if changed or cache is absent, run `index_taps()` and upsert catalog rows. The in-memory `CommunityProfileIndex` type continues to be used throughout the app; SQLite is populated from it, not read in place of it. This keeps the two paths independently correct and makes the SQLite path verifiable by comparing against a forced rescan.

### RF-5: RecentFilesStore Migration — In Scope

**Resolution**: Migrating `RecentFilesStore` to SQLite is **in scope** and planned for the initial SQLite release. Rationale: it's the simplest migration (no relational complexity, no identity concerns), it eliminates a second TOML file in `~/.local/share/crosshook/`, and it's the best proof-of-concept for the migration pattern other stores will follow. The `~/.local/share/crosshook/` path for `recent.toml` also confirms that `metadata.db` belongs in the same XDG data directory rather than in `~/.config/crosshook/`.

---

## Open Questions

- Should stable profile IDs be written back into TOML comments/fields for portability (e.g. `# crosshook-id: <uuid>`), or remain purely local? Writing into TOML would allow moving profiles between machines with history preserved.
- Should collections stay local-only or be designed for future export/import? The community tap system already models shareable profiles; collections could be a shareable overlay.
- How aggressively should CrossHook attempt automatic launcher drift recovery versus conservative warning-only behavior? The current code detects drift (staleness flag) but only surfaces it — no auto-repair exists.
- Should `RecentFilesStore` be migrated to SQLite immediately (replacing the TOML file) or should it be kept in TOML as a fallback for the first iteration?
- Should the SQLite database live at `~/.local/share/crosshook/metadata.db` or `~/.config/crosshook/metadata.db`? The `local/share` path aligns with community taps and launchers; `config` aligns with settings and profiles.
- Should launch logs be retained indefinitely in the log directory with a SQLite pointer, or should log rotation be managed by SQLite (delete logs older than N days)?
