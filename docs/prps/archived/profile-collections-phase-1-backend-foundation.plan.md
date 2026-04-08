# Plan: Profile Collections — Phase 1 (Backend Foundation)

## Summary

Make the existing dead-code collections IPC surface production-ready: fix the silent no-op on `add_profile_to_collection`, add schema migration **18 → 19** for the FK cascade + `sort_order` column, add 3 new IPC commands (`collection_rename`, `collection_update_description`, `collections_for_profile`), add browser dev-mode mocks for all 9 collection commands (6 existing + 3 new), add integration tests for every CRUD path + edge cases, and remove `#[allow(dead_code)]` from `CollectionRow`. Zero frontend consumers today — 100% greenfield on the mock side, IPC-only backend polish on the Rust side.

## User Story

As a **power user with 50+ profiles**, I want the backend foundation for collections to be **correct and complete** so that later phases can build the sidebar + view modal without hitting silent no-ops, orphan FK rows, missing rename/description APIs, or a `pnpm dev:browser` crash on the first collection IPC call.

## Problem → Solution

**Current state**: The `collections` + `collection_profiles` tables and 6 Tauri commands exist from schema v4, but `CollectionRow` is `#[allow(dead_code)]`, `add_profile_to_collection` silently swallows missing profiles and returns `Ok(())`, the `collection_profiles.profile_id` FK lacks `ON DELETE CASCADE` (so deleting a profile orphans membership rows), there is no rename / description-update / reverse-lookup API, and **no browser dev-mode mocks exist for any of the 6 existing commands** — which will crash `pnpm dev:browser` the first time a collection hook mounts.

**Desired state**: All 9 commands (6 existing + 3 new) are callable from frontend, integration-tested, wired through browser mocks, and enforce correct error semantics (missing profile → typed `Validation` error, deleted profile → cascade cleanup, duplicate name → bubbled unique-constraint error). Schema is at v19 with the FK cascade and `sort_order` column in place.

## Metadata

- **Complexity**: **Medium** (8 files touched, ~500 lines, no new dependencies, follows established patterns exactly)
- **Source PRD**: `docs/prps/prds/profile-collections.prd.md`
- **PRD Phase**: **Phase 1 — Backend foundation**
- **Depends on**: — (no predecessor)
- **Estimated Files**: 8 (6 UPDATE, 2 CREATE)
- **Schema target**: v18 → **v19** _(PRD incorrectly references "v5" — current schema is at v18, see `migrations.rs:165-172`)_

---

## UX Design

**Internal change — no user-facing UX transformation.** Phase 1 is pure backend plumbing. Users see nothing new until Phase 2 ships the sidebar and view modal. The only externally observable change is that calling `collection_add_profile` with a nonexistent profile name now returns an error string to the frontend instead of silently succeeding.

### Interaction Changes

| Touchpoint                                                          | Before                                                 | After                                                                                     | Notes                                     |
| ------------------------------------------------------------------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------- | ----------------------------------------- |
| `invoke('collection_add_profile', { collection_id, profile_name })` | `Ok(())` silently when profile missing, warn-logs it   | `Err("metadata validation error: profile not found: <name>")`                             | Frontend can now distinguish skip vs. add |
| `invoke('collection_delete', { ... })` followed by profile deletion | Orphan rows left in `collection_profiles`              | Deleting a profile cascades to remove its membership rows                                 | Via schema v19 `ON DELETE CASCADE`        |
| `pnpm dev:browser` + any collection IPC call                        | `Error: [dev-mock] Unhandled command: collection_list` | Mock handler returns seeded fixture data                                                  | Unblocks Phase 2 frontend work            |
| `invoke('collection_rename', ...)`                                  | Command does not exist                                 | Renames the collection, errors on duplicate name or missing id                            | New command                               |
| `invoke('collection_update_description', ...)`                      | Command does not exist                                 | Updates description to `Some(s)` or clears to `NULL`                                      | New command                               |
| `invoke('collections_for_profile', ...)`                            | Command does not exist                                 | Returns `Vec<CollectionRow>` for the collections containing a given profile (by filename) | New reverse-lookup command                |

---

## Mandatory Reading

Read these files before starting — the plan assumes you have this context in head.

| Priority | File                                                                      | Lines                      | Why                                                                                                                             |
| -------- | ------------------------------------------------------------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs`  | all (217)                  | Every function you will touch or mirror                                                                                         |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`   | 1-175, 372-397, 216-227    | `run_migrations` dispatch, `migrate_6_to_7` table-rebuild pattern, `migrate_1_to_2` `ALTER COLUMN` pattern                      |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`          | 98-135, 448-497, 2507-2584 | `with_conn` helper, existing collection wrapper methods, existing collection test fixtures                                      |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`       | 1-72, 294-303              | `MetadataStoreError` variants (critically: `Validation(String)` is a **tuple** variant, not struct), `CollectionRow` definition |
| **P0**   | `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs` | 72-86                      | `lookup_profile_id` signature — returns `Result<Option<String>, _>`; `None` is the silent-no-op trigger                         |
| **P0**   | `src/crosshook-native/src-tauri/src/commands/collections.rs`              | all (64)                   | Exact shape to mirror for 3 new commands                                                                                        |
| **P0**   | `src/crosshook-native/src-tauri/src/lib.rs`                               | 208-331                    | `tauri::generate_handler!` registration block; new commands insert between lines 286 and 287                                    |
| **P0**   | `src/crosshook-native/src/lib/mocks/handlers/community.ts`                | 90-165                     | Mirror pattern for a register function, typed `map.set(...)` handlers, and `[dev-mock]` error messages                          |
| **P1**   | `src/crosshook-native/src/lib/mocks/index.ts`                             | all (57)                   | Where to register the new handler barrel                                                                                        |
| **P1**   | `src/crosshook-native/src/lib/mocks/handlers/types.ts`                    | 1-3                        | `Handler` type — `(args: unknown) => unknown \| Promise<unknown>`                                                               |
| **P1**   | `src/crosshook-native/src/lib/ipc.dev.ts`                                 | all (34)                   | `runMockCommand` dispatcher and `[dev-mock] Unhandled command` error shape                                                      |
| **P2**   | `.github/workflows/release.yml`                                           | 105-120                    | `Verify no mock code in production bundle` sentinel — **all mock error strings must include `[dev-mock]` to participate**       |
| **P2**   | `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`          | 1463-1515                  | `sample_profile()` factory and `connection(&store)` accessor used in all collection tests                                       |

## External Documentation

**No external research needed** — this phase uses only established internal patterns (rusqlite migrations, Tauri IPC, existing mock handler shape). No new dependencies.

---

## Patterns to Mirror

All snippets are **verbatim from the codebase**. Follow them exactly.

### NAMING_CONVENTION — free function in `collections.rs`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs:46-68
pub fn create_collection(conn: &Connection, name: &str) -> Result<String, MetadataStoreError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(MetadataStoreError::Validation(
            "collection name must not be empty".to_string(),
        ));
    }

    let collection_id = db::new_id();
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO collections (collection_id, name, description, created_at, updated_at) \
         VALUES (?1, ?2, NULL, ?3, ?4)",
        params![collection_id, trimmed, now, now],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert a new collection",
        source,
    })?;

    Ok(collection_id)
}
```

Conventions to observe:

- `pub fn name(conn: &Connection, ...) -> Result<T, MetadataStoreError>`
- Trim input strings, validate non-empty with `Validation(String)` tuple variant
- `now = Utc::now().to_rfc3339()` for all timestamps
- `conn.execute(..., params![...])` + `.map_err(|source| MetadataStoreError::Database { action: "<verb>", source })`
- `action` strings start with an infinitive verb ("insert a new collection", "rename a collection")

### ERROR_HANDLING — `MetadataStoreError::Validation` is a **tuple** variant

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:8-22
#[derive(Debug)]
pub enum MetadataStoreError {
    HomeDirectoryUnavailable,
    Database { action: &'static str, source: SqlError },
    Io { action: &'static str, path: PathBuf, source: std::io::Error },
    Corrupt(String),
    SymlinkDetected(PathBuf),
    Validation(String),  // ← tuple, NOT struct
}
```

```rust
// CORRECT usage (from existing code at collections.rs:49):
return Err(MetadataStoreError::Validation(
    "collection name must not be empty".to_string(),
));

// INCORRECT (PRD shows struct syntax that does NOT compile):
// Err(MetadataStoreError::Validation { ... })  ❌
```

### REPOSITORY_PATTERN — `MetadataStore` wrapper in `mod.rs`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:452-497
pub fn list_collections(&self) -> Result<Vec<CollectionRow>, MetadataStoreError> {
    self.with_conn("list collections", |conn| {
        collections::list_collections(conn)
    })
}

pub fn create_collection(&self, name: &str) -> Result<String, MetadataStoreError> {
    self.with_conn("create a collection", |conn| {
        collections::create_collection(conn, name)
    })
}

pub fn delete_collection(&self, collection_id: &str) -> Result<(), MetadataStoreError> {
    self.with_conn("delete a collection", |conn| {
        collections::delete_collection(conn, collection_id)
    })
}

pub fn add_profile_to_collection(
    &self,
    collection_id: &str,
    profile_name: &str,
) -> Result<(), MetadataStoreError> {
    self.with_conn("add a profile to a collection", |conn| {
        collections::add_profile_to_collection(conn, collection_id, profile_name)
    })
}
```

Use `with_conn` for all read paths and single-statement writes. `with_conn_mut` is **only** needed for `&mut Connection` — e.g., multi-statement transactions. All three new wrappers (`rename_collection`, `update_collection_description`, `collections_for_profile`) use `with_conn`.

### SERVICE_PATTERN — Tauri command handler

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/collections.rs:1-64
use crosshook_core::metadata::{CollectionRow, MetadataStore};
use tauri::State;

fn map_error(e: impl ToString) -> String {
    e.to_string()
}

#[tauri::command]
pub fn collection_create(
    name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<String, String> {
    metadata_store.create_collection(&name).map_err(map_error)
}

#[tauri::command]
pub fn collection_add_profile(
    collection_id: String,
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    metadata_store
        .add_profile_to_collection(&collection_id, &profile_name)
        .map_err(map_error)
}
```

- **`snake_case`** command names — CLAUDE.md MUST rule
- `Result<T, String>` return, always `.map_err(map_error)`
- Positional args come **before** `metadata_store: State<'_, MetadataStore>`
- Use `State<'_, MetadataStore>` (injected via `tauri::Builder::manage`)

### COMMAND_REGISTRATION — insert after line 286

```rust
// SOURCE: src/crosshook-native/src-tauri/src/lib.rs:279-292
            // Phase 3: Catalog and Intelligence
            commands::community::community_list_indexed_profiles,
            commands::collections::collection_list,
            commands::collections::collection_create,
            commands::collections::collection_delete,
            commands::collections::collection_add_profile,
            commands::collections::collection_remove_profile,
            commands::collections::collection_list_profiles,
            // ← INSERT new commands HERE, before profile_set_favorite
            commands::profile::profile_set_favorite,
            commands::profile::profile_list_favorites,
```

### MIGRATION_PATTERN — `ALTER TABLE ADD COLUMN`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:216-227
fn migrate_1_to_2(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "ALTER TABLE profiles ADD COLUMN source TEXT;
         UPDATE profiles SET source = 'initial_census' WHERE source IS NULL;",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 1 to 2",
        source,
    })?;

    Ok(())
}
```

### MIGRATION_PATTERN — FK cascade via table-rebuild (canonical)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:372-397
fn migrate_6_to_7(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        BEGIN TRANSACTION;
        CREATE TABLE health_snapshots_new (
            profile_id  TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
            status      TEXT NOT NULL,
            issue_count INTEGER NOT NULL DEFAULT 0,
            checked_at  TEXT NOT NULL
        );
        INSERT INTO health_snapshots_new (profile_id, status, issue_count, checked_at)
        SELECT profile_id, status, issue_count, checked_at
        FROM health_snapshots;
        DROP TABLE health_snapshots;
        ALTER TABLE health_snapshots_new RENAME TO health_snapshots;
        CREATE INDEX IF NOT EXISTS idx_health_snapshots_checked_at ON health_snapshots(checked_at);
        COMMIT;
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 6 to 7",
        source,
    })?;

    Ok(())
}
```

SQLite does **not** support `ALTER TABLE ... ADD CONSTRAINT`. The `BEGIN → CREATE _new → INSERT SELECT → DROP old → ALTER RENAME → recreate indexes → COMMIT` sequence is the canonical way to add an FK cascade post-hoc.

### TEST_STRUCTURE — collection integration test

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:2529-2553
#[test]
fn test_add_profile_to_collection() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");

    store
        .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
        .unwrap();

    let collection_id = store.create_collection("Test Collection").unwrap();
    store
        .add_profile_to_collection(&collection_id, "elden-ring")
        .unwrap();

    let conn = connection(&store);
    let row_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(row_count, 1);
}
```

Conventions:

- `MetadataStore::open_in_memory().unwrap()` (NOT temp files)
- Use the existing `sample_profile()` factory at `mod.rs:1463`
- Insert fixtures via `store.observe_profile_write(name, &profile, path, SyncSource::AppWrite, None)`
- Use the existing `connection(&store)` helper at `mod.rs:1508` for direct SQL assertions
- Test function names use `test_<verb>_<subject>` snake_case

### MOCK_HANDLER_PATTERN — TS `register*()` + typed handlers

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/community.ts:100-150
export function registerCommunity(map: Map<string, Handler>): void {
  map.set('community_list_profiles', async (): Promise<CommunityProfileIndex> => {
    return { ...MOCK_INDEX };
  });

  map.set('community_add_tap', async (args): Promise<CommunityTapSubscription[]> => {
    const { tap } = args as { tap: CommunityTapSubscription };
    if (!tap?.url?.trim()) {
      throw new Error('[dev-mock] community_add_tap: tap URL is required');
    }
    // ...
  });
}
```

Conventions:

- One `register<Area>()` per file, takes `Map<string, Handler>`, returns `void`
- Every `map.set('command_name', async (args): Promise<T> => { ... })`
- **All error strings MUST start with `[dev-mock]`** — the `.github/workflows/release.yml:105-120` sentinel greps for this literal string to verify no mock code escaped into production bundles
- Cast `args` with `as { ... }` — never `any`
- Module-scope `let` for mutable state (e.g., `let collections: MockCollection[] = [...]`)

### MOCK_REGISTRATION — barrel import

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/index.ts:30-56 (excerpt)
export function registerMocks(): Map<string, Handler> {
  const map = new Map<string, Handler>();

  // Boot-critical (Phase 1)
  registerSettings(map);
  registerProfile(map);

  // Phase 2 domain handlers
  registerLaunch(map);
  // ...
  registerCommunity(map);
  registerLauncher(map);
  registerLibrary(map);
  registerSystem(map);

  return wrapAllHandlers(map);
}
```

---

## Files to Change

| #   | File                                                                     | Action | Justification                                                                                                                                                                                 |
| --- | ------------------------------------------------------------------------ | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`  | UPDATE | Add `migrate_18_to_19` (FK cascade rebuild + `sort_order` column) and dispatch block                                                                                                          |
| 2   | `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs` | UPDATE | Fix `add_profile_to_collection` silent no-op; add `rename_collection`, `update_collection_description`, `collections_for_profile`; update `list_collections` ORDER BY to respect `sort_order` |
| 3   | `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`         | UPDATE | Add 3 new `MetadataStore` wrapper methods; add tests for new + edge-case paths                                                                                                                |
| 4   | `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`      | UPDATE | Remove `#[allow(dead_code)]` from `CollectionRow` (line 295)                                                                                                                                  |
| 5   | `src/crosshook-native/src-tauri/src/commands/collections.rs`             | UPDATE | Add 3 new `#[tauri::command]` handlers mirroring the existing shape                                                                                                                           |
| 6   | `src/crosshook-native/src-tauri/src/lib.rs`                              | UPDATE | Register 3 new commands in `tauri::generate_handler!` at the Phase 3 collections block                                                                                                        |
| 7   | `src/crosshook-native/src/lib/mocks/handlers/collections.ts`             | CREATE | Browser dev-mode mock handler file for all 9 collection IPC commands (6 existing + 3 new); seeds 1 fixture collection                                                                         |
| 8   | `src/crosshook-native/src/lib/mocks/index.ts`                            | UPDATE | Import + call `registerCollections(map)` in `registerMocks()`                                                                                                                                 |

## NOT Building

- **Sidebar Collections section, collection view modal, any frontend hook (`useCollections`, `useCollectionMembers`)** — Phase 2 work
- **Per-collection launch defaults** (`CollectionDefaultsSection`, `effective_profile` extension, `collection_launch_defaults` table, `collection_get_defaults`/`collection_set_defaults` commands) — Phase 3 work
- **TOML export/import, import review modal** — Phase 4 work
- **`sort_order` setter / reorder IPC / reorder UI** — Phase 2 (Phase 1 only adds the column + `ORDER BY sort_order ASC, name ASC` so Phase 2 sees correct ordering as soon as it starts writing values)
- **Changing `remove_profile_from_collection` to strict error semantics** — the current idempotent-delete behavior is intentional per repo convention; only the ADD path needs to change. Confirmed against `collections.rs:117-120`.
- **`verify:no-mocks` script in `package.json`** — does not exist; the CI sentinel is an inline `grep` step in `.github/workflows/release.yml:105-120` and requires no changes here. New mock error strings already include `[dev-mock]` to participate.
- **Removing `#[allow(dead_code)]` from other structs in `models.rs`** — only `CollectionRow` (line 295) is in scope. `ProfileRow`, `LauncherRow`, `LaunchOperationRow`, `CommunityTapRow`, `CommunityProfileRow`, `FailureTrendRow` stay as-is.
- **Refactoring the two-pattern migration style** — `migrate_4_to_5` uses `RENAME→_old`, `migrate_6_to_7` uses `CREATE→_new`. Phase 1 uses the `_new` pattern for consistency with the more recent idiom; existing migrations are not rewritten.

---

## Step-by-Step Tasks

### Task 1: Add schema migration 18 → 19

- **ACTION**: Add `migrate_18_to_19` function and its dispatch block in `migrations.rs`.
- **IMPLEMENT**:
  1. Insert a new dispatch block in `run_migrations` (after the `if version < 18` block, around `migrations.rs:173`):

     ```rust
     if version < 19 {
         migrate_18_to_19(conn)?;
         conn.pragma_update(None, "user_version", 19_u32)
             .map_err(|source| MetadataStoreError::Database {
                 action: "set user_version to 19",
                 source,
             })?;
     }
     ```

  2. Append the `migrate_18_to_19` function **after** `migrate_17_to_18` (e.g., around `migrations.rs:812`):

     ```rust
     fn migrate_18_to_19(conn: &Connection) -> Result<(), MetadataStoreError> {
         conn.execute_batch(
             "
             BEGIN TRANSACTION;

             -- 1. Add sort_order column to collections (NOT NULL DEFAULT 0).
             ALTER TABLE collections ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

             -- 2. Rebuild collection_profiles with ON DELETE CASCADE on profile_id.
             CREATE TABLE collection_profiles_new (
                 collection_id   TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
                 profile_id      TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
                 added_at        TEXT NOT NULL,
                 PRIMARY KEY (collection_id, profile_id)
             );
             INSERT INTO collection_profiles_new (collection_id, profile_id, added_at)
             SELECT collection_id, profile_id, added_at FROM collection_profiles;
             DROP TABLE collection_profiles;
             ALTER TABLE collection_profiles_new RENAME TO collection_profiles;
             CREATE INDEX IF NOT EXISTS idx_collection_profiles_profile_id
                 ON collection_profiles(profile_id);
             COMMIT;
             ",
         )
         .map_err(|source| MetadataStoreError::Database {
             action: "run metadata migration 18 to 19",
             source,
         })?;

         Ok(())
     }
     ```

- **MIRROR**: `migrate_6_to_7` (table-rebuild for cascade) + `migrate_1_to_2` (`ALTER TABLE ADD COLUMN`) — see Patterns to Mirror.
- **IMPORTS**: none — `Connection`, `MetadataStoreError`, and `rusqlite` are already imported in `migrations.rs`.
- **GOTCHA**:
  - **`PRAGMA foreign_keys` must be ON** for the rebuild to not orphan rows. The connection opener (`db::open_at_path` / `db::open_in_memory`) is responsible for this; do NOT toggle it inside the migration. If tests fail on FK enforcement, verify `db.rs` sets `PRAGMA foreign_keys = ON` at connection time.
  - Do **not** combine the `ALTER TABLE` and the table rebuild outside a single `execute_batch` transaction — if the cascade rebuild is committed in a separate transaction from the `ALTER`, a failure between the two leaves schema in a half-migrated state.
  - **Do NOT modify `migrate_17_to_18`** or any prior migration — SQLite migrations are immutable once released.
  - `sort_order INTEGER NOT NULL DEFAULT 0` backfills existing rows with `0` automatically; no secondary `UPDATE` needed (unlike `migrate_1_to_2` which used a nullable column).
- **VALIDATE**:
  - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core migrations` passes
  - New test `migration_18_to_19_adds_sort_order_and_cascade` (see Task 10) passes
  - Running against a pre-existing v18 database applies cleanly with no data loss on `collections` or `collection_profiles`

### Task 2: Fix `add_profile_to_collection` silent no-op

- **ACTION**: Replace the early `Ok(())` return with a `Validation` error when the profile name does not resolve.
- **IMPLEMENT**: In `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs:83-110`, change the function body:

  ```rust
  pub fn add_profile_to_collection(
      conn: &Connection,
      collection_id: &str,
      profile_name: &str,
  ) -> Result<(), MetadataStoreError> {
      let profile_id = lookup_profile_id(conn, profile_name)?.ok_or_else(|| {
          MetadataStoreError::Validation(format!(
              "profile not found when adding to collection: {profile_name}"
          ))
      })?;

      let now = Utc::now().to_rfc3339();
      conn.execute(
          "INSERT OR IGNORE INTO collection_profiles (collection_id, profile_id, added_at) \
           VALUES (?1, ?2, ?3)",
          params![collection_id, profile_id, now],
      )
      .map_err(|source| MetadataStoreError::Database {
          action: "add a profile to a collection",
          source,
      })?;

      Ok(())
  }
  ```

- **MIRROR**: `create_collection` at `collections.rs:49-51` for `Validation` construction; existing `add_profile_to_collection` body for the INSERT shape.
- **IMPORTS**: none new — `lookup_profile_id`, `MetadataStoreError`, `params`, `Utc` already imported.
- **GOTCHA**:
  - **Leave `remove_profile_from_collection` unchanged.** The idempotent no-op on REMOVE is intentional (lines 117-120) — only the ADD path changes.
  - Drop the `tracing::warn!` call — the error is now the surfacing mechanism. Do NOT add a `tracing::error!` above the `ok_or_else`; the caller is responsible for logging. Avoid duplicate logging.
  - `MetadataStoreError::Validation` is a **tuple** variant: `Validation(String)`, not `Validation { ... }`. The PRD example used struct syntax, which does not compile.
  - `INSERT OR IGNORE` on a duplicate `(collection_id, profile_id)` pair still returns `Ok(())` — this is correct idempotent-add behavior (adding an already-member profile is not an error).
- **VALIDATE**:
  - Existing `test_add_profile_to_collection` at `mod.rs:2529` still passes (profile exists before add)
  - New test `test_add_profile_to_collection_missing_profile_errors` (see Task 11) passes

### Task 3: Add 3 new free functions in `collections.rs`

- **ACTION**: Implement `rename_collection`, `update_collection_description`, and `collections_for_profile` as free functions in `collections.rs`, mirroring the existing 6.
- **IMPLEMENT**: Append the following after `list_favorite_profiles` at `collections.rs:217`:

  ```rust
  pub fn rename_collection(
      conn: &Connection,
      collection_id: &str,
      new_name: &str,
  ) -> Result<(), MetadataStoreError> {
      let trimmed = new_name.trim();
      if trimmed.is_empty() {
          return Err(MetadataStoreError::Validation(
              "collection name must not be empty".to_string(),
          ));
      }

      let now = Utc::now().to_rfc3339();
      let affected = conn
          .execute(
              "UPDATE collections SET name = ?1, updated_at = ?2 WHERE collection_id = ?3",
              params![trimmed, now, collection_id],
          )
          .map_err(|source| MetadataStoreError::Database {
              action: "rename a collection",
              source,
          })?;

      if affected == 0 {
          return Err(MetadataStoreError::Validation(format!(
              "collection not found: {collection_id}"
          )));
      }

      Ok(())
  }

  pub fn update_collection_description(
      conn: &Connection,
      collection_id: &str,
      description: Option<&str>,
  ) -> Result<(), MetadataStoreError> {
      let normalized: Option<String> = description
          .map(|s| s.trim().to_string())
          .filter(|s| !s.is_empty());

      let now = Utc::now().to_rfc3339();
      let affected = conn
          .execute(
              "UPDATE collections SET description = ?1, updated_at = ?2 WHERE collection_id = ?3",
              params![normalized, now, collection_id],
          )
          .map_err(|source| MetadataStoreError::Database {
              action: "update a collection description",
              source,
          })?;

      if affected == 0 {
          return Err(MetadataStoreError::Validation(format!(
              "collection not found: {collection_id}"
          )));
      }

      Ok(())
  }

  pub fn collections_for_profile(
      conn: &Connection,
      profile_name: &str,
  ) -> Result<Vec<CollectionRow>, MetadataStoreError> {
      let profile_id = match lookup_profile_id(conn, profile_name)? {
          Some(id) => id,
          None => return Ok(Vec::new()),
      };

      let mut stmt = conn
          .prepare(
              "SELECT c.collection_id, c.name, c.description, c.created_at, c.updated_at, \
               (SELECT COUNT(*) FROM collection_profiles cp2 WHERE cp2.collection_id = c.collection_id) as profile_count \
               FROM collections c \
               INNER JOIN collection_profiles cp ON cp.collection_id = c.collection_id \
               WHERE cp.profile_id = ?1 \
               ORDER BY c.sort_order ASC, c.name ASC",
          )
          .map_err(|source| MetadataStoreError::Database {
              action: "prepare collections_for_profile query",
              source,
          })?;

      let rows = stmt
          .query_map(params![profile_id], |row| {
              Ok(CollectionRow {
                  collection_id: row.get(0)?,
                  name: row.get(1)?,
                  description: row.get(2)?,
                  created_at: row.get(3)?,
                  updated_at: row.get(4)?,
                  profile_count: row.get(5)?,
              })
          })
          .map_err(|source| MetadataStoreError::Database {
              action: "query collections for profile",
              source,
          })?;

      let mut collections = Vec::new();
      for row in rows {
          collections.push(row.map_err(|source| MetadataStoreError::Database {
              action: "read a collection row in collections_for_profile",
              source,
          })?);
      }

      Ok(collections)
  }
  ```

- **ALSO**: Update the `ORDER BY` in `list_collections` (currently at `collections.rs:12`):

  ```rust
  // BEFORE
  "... FROM collections c ORDER BY c.name"
  // AFTER
  "... FROM collections c ORDER BY c.sort_order ASC, c.name ASC"
  ```

  This is a one-line change. It wires the new column into the read path so Phase 2 gets correct ordering as soon as it starts writing `sort_order` values.
- **MIRROR**: `create_collection` (validation + trim), `list_collections` (prepare + query_map + row construction), `set_profile_favorite` (UPDATE shape).
- **IMPORTS**: `CollectionRow` is already imported at the top of `collections.rs` via `use super::models::CollectionRow;`. No new imports needed.
- **GOTCHA**:
  - **`collections_for_profile` with missing profile returns empty Vec, NOT error.** This matches `list_profiles_in_collection` behavior — a valid profile name with zero memberships and an unknown profile name are both "not a member of anything", so `Ok(vec![])` is the correct signal. The IPC consumer distinguishes by combining with a separate profile-existence check if needed.
  - **`rename_collection` on unknown id returns `Validation`, NOT `Ok(())`**. Mirrors the Task 2 fix pattern — the frontend must know rename failed.
  - **`UNIQUE` constraint on `collections.name` bubbles as `Database { action: "rename a collection", source: SqlError::SqliteFailure(_, _) }`**. Do NOT pre-check for duplicates — let SQLite enforce it. The Tauri layer maps it to `e.to_string()` which the frontend can display.
  - `update_collection_description` with `Some("")` is treated as `None` (normalization on whitespace-only strings). This matches the "clear field" UX.
  - `collections.sort_order` is available starting from migration 19 — Task 1 must complete first, otherwise `collections_for_profile` and the updated `list_collections` ORDER BY will error on "no such column: c.sort_order".
- **VALIDATE**:
  - `cargo build -p crosshook-core` succeeds
  - Tests for new functions (Task 12) pass

### Task 4: Add 3 new `MetadataStore` wrapper methods in `mod.rs`

- **ACTION**: Add wrapper methods that delegate to the `collections::*` free functions.
- **IMPLEMENT**: Insert the following into the `impl MetadataStore` block, **inside** the "Phase 3: Collections" section (after `list_profiles_in_collection` at `mod.rs:497`, before the "Phase 3: Favorites" divider at line 499):

  ```rust
  pub fn rename_collection(
      &self,
      collection_id: &str,
      new_name: &str,
  ) -> Result<(), MetadataStoreError> {
      self.with_conn("rename a collection", |conn| {
          collections::rename_collection(conn, collection_id, new_name)
      })
  }

  pub fn update_collection_description(
      &self,
      collection_id: &str,
      description: Option<&str>,
  ) -> Result<(), MetadataStoreError> {
      self.with_conn("update a collection description", |conn| {
          collections::update_collection_description(conn, collection_id, description)
      })
  }

  pub fn collections_for_profile(
      &self,
      profile_name: &str,
  ) -> Result<Vec<CollectionRow>, MetadataStoreError> {
      self.with_conn("list collections for a profile", |conn| {
          collections::collections_for_profile(conn, profile_name)
      })
  }
  ```

- **MIRROR**: The 6 existing collection wrappers at `mod.rs:452-497`. Exact shape.
- **IMPORTS**: none — `CollectionRow`, `MetadataStoreError`, and `collections` module are already in scope.
- **GOTCHA**:
  - Use `with_conn`, NOT `with_conn_mut`. None of these three methods need multi-statement transactions.
  - `T: Default` constraint on `with_conn` is satisfied: `()` has `Default`, `Vec<CollectionRow>` has `Default` (empty vec). When the store is `disabled()`, `rename_collection` returns `Ok(())` and `collections_for_profile` returns `Ok(vec![])`. This matches the existing disabled-store semantics.
  - Keep the divider comment `// Phase 3: Collections` / `// Phase 3: Favorites` intact — do not move these around.
- **VALIDATE**: `cargo build -p crosshook-core` succeeds and all existing tests still pass.

### Task 5: Remove `#[allow(dead_code)]` from `CollectionRow`

- **ACTION**: Delete the `#[allow(dead_code)]` line on `CollectionRow` in `models.rs`.
- **IMPLEMENT**: In `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:294-303`, change:

  ```rust
  #[derive(Debug, Clone, Serialize)]
  #[allow(dead_code)]   // ← DELETE THIS LINE
  pub struct CollectionRow {
      // ...
  }
  ```

  to:

  ```rust
  #[derive(Debug, Clone, Serialize)]
  pub struct CollectionRow {
      // ...
  }
  ```

- **MIRROR**: n/a — deletion only.
- **IMPORTS**: none.
- **GOTCHA**:
  - The attribute is stale because `CollectionRow` is already used in the Tauri command return type (`commands/collections.rs:11` — `Result<Vec<CollectionRow>, String>`). The fields are serialized across IPC, so they ARE read — `dead_code` would only fire if every field were unused, which is no longer the case. Removal is safe.
  - **Do NOT remove the attribute from other structs in `models.rs`** (`ProfileRow` line 192, `LauncherRow` line 206, `LaunchOperationRow` line 220, `CommunityTapRow` line 259, `CommunityProfileRow` line 273, `FailureTrendRow` line 306). Those are outside Phase 1 scope.
- **VALIDATE**: `cargo build -p crosshook-core` succeeds with no new warnings.

### Task 6: Add 3 new Tauri command handlers

- **ACTION**: Add `collection_rename`, `collection_update_description`, `collections_for_profile` to `src-tauri/src/commands/collections.rs`.
- **IMPLEMENT**: Append the following to `src/crosshook-native/src-tauri/src/commands/collections.rs` (after `collection_list_profiles` at line 64):

  ```rust
  #[tauri::command]
  pub fn collection_rename(
      collection_id: String,
      new_name: String,
      metadata_store: State<'_, MetadataStore>,
  ) -> Result<(), String> {
      metadata_store
          .rename_collection(&collection_id, &new_name)
          .map_err(map_error)
  }

  #[tauri::command]
  pub fn collection_update_description(
      collection_id: String,
      description: Option<String>,
      metadata_store: State<'_, MetadataStore>,
  ) -> Result<(), String> {
      metadata_store
          .update_collection_description(&collection_id, description.as_deref())
          .map_err(map_error)
  }

  #[tauri::command]
  pub fn collections_for_profile(
      profile_name: String,
      metadata_store: State<'_, MetadataStore>,
  ) -> Result<Vec<CollectionRow>, String> {
      metadata_store
          .collections_for_profile(&profile_name)
          .map_err(map_error)
  }
  ```

- **MIRROR**: `collection_create` and `collection_add_profile` — exact shape, exact ordering of args, identical `.map_err(map_error)` tail.
- **IMPORTS**: none new. `CollectionRow`, `MetadataStore`, `State`, `map_error` already in scope.
- **GOTCHA**:
  - **Command names MUST be `snake_case`** per CLAUDE.md. `collection_rename` (not `collectionRename`).
  - `description: Option<String>` — Tauri deserializes JSON `null` and missing fields both to `None`, and a string to `Some(s)`. `.as_deref()` converts to `Option<&str>`.
  - Positional args come **before** `metadata_store: State<...>` — Tauri requires `State` to be last.
  - Do NOT add a `#[tauri::command(rename_all = "camelCase")]` attribute — the rest of the file uses the default snake_case convention.
- **VALIDATE**: `cargo build --manifest-path src/crosshook-native/src-tauri/Cargo.toml` succeeds.

### Task 7: Register 3 new commands in `tauri::generate_handler!`

- **ACTION**: Add 3 lines to the handler registration block in `src/crosshook-native/src-tauri/src/lib.rs`.
- **IMPLEMENT**: In `src-tauri/src/lib.rs`, find the block at lines 281-286 (6 existing collection commands) and insert 3 new lines immediately after `collection_list_profiles`:

  ```rust
              // Phase 3: Catalog and Intelligence
              commands::community::community_list_indexed_profiles,
              commands::collections::collection_list,
              commands::collections::collection_create,
              commands::collections::collection_delete,
              commands::collections::collection_add_profile,
              commands::collections::collection_remove_profile,
              commands::collections::collection_list_profiles,
              commands::collections::collection_rename,               // ← NEW
              commands::collections::collection_update_description,   // ← NEW
              commands::collections::collections_for_profile,         // ← NEW
              commands::profile::profile_set_favorite,
  ```

- **MIRROR**: The existing 6 `commands::collections::*` lines.
- **IMPORTS**: none.
- **GOTCHA**:
  - Trailing commas are mandatory — `tauri::generate_handler!` is a macro and missing commas produce a confusing error.
  - Order inside the block is not functionally significant but the convention is to keep related commands grouped. Insert at the end of the collections sub-block to keep the diff clean.
- **VALIDATE**: `cargo build --manifest-path src/crosshook-native/src-tauri/Cargo.toml` succeeds and the app binary links.

### Task 8: Create browser dev-mode mock handler file

- **ACTION**: Create `src/crosshook-native/src/lib/mocks/handlers/collections.ts` with 9 handlers (6 existing + 3 new) and a minimal seed fixture.
- **IMPLEMENT**: New file contents:

  ```ts
  // Mock IPC handlers for collections_* commands. See `lib/mocks/README.md`.
  // All error messages MUST start with `[dev-mock]` to participate in the
  // `.github/workflows/release.yml` "Verify no mock code in production bundle"
  // sentinel.

  import type { Handler } from './types';

  // Shape mirrors Rust `CollectionRow` in
  // crates/crosshook-core/src/metadata/models.rs (snake_case per serde default).
  interface MockCollectionRow {
    collection_id: string;
    name: string;
    description: string | null;
    profile_count: number;
    created_at: string;
    updated_at: string;
  }

  // Module-scope mutable state — resets on page reload.
  let collections: MockCollectionRow[] = [
    {
      collection_id: 'mock-collection-1',
      name: 'Action / Adventure',
      description: 'Seeded fixture collection for dev mode',
      profile_count: 0,
      created_at: new Date('2026-04-01T12:00:00Z').toISOString(),
      updated_at: new Date('2026-04-01T12:00:00Z').toISOString(),
    },
  ];
  const membership = new Map<string, Set<string>>([['mock-collection-1', new Set()]]);

  function nowIso(): string {
    return new Date().toISOString();
  }

  function recomputeProfileCounts(): void {
    for (const col of collections) {
      col.profile_count = membership.get(col.collection_id)?.size ?? 0;
    }
  }

  function findById(id: string): MockCollectionRow | undefined {
    return collections.find((c) => c.collection_id === id);
  }

  export function registerCollections(map: Map<string, Handler>): void {
    map.set('collection_list', async (): Promise<MockCollectionRow[]> => {
      recomputeProfileCounts();
      // Mirror Rust ORDER BY c.sort_order ASC, c.name ASC — sort_order is not
      // tracked in the mock; fall back to name ordering.
      return [...collections].sort((a, b) => a.name.localeCompare(b.name));
    });

    map.set('collection_create', async (args): Promise<string> => {
      const { name } = args as { name: string };
      const trimmed = (name ?? '').trim();
      if (!trimmed) {
        throw new Error('[dev-mock] collection_create: collection name must not be empty');
      }
      if (collections.some((c) => c.name === trimmed)) {
        throw new Error(`[dev-mock] collection_create: duplicate collection name: ${trimmed}`);
      }
      const id = `mock-collection-${Date.now().toString(36)}`;
      const ts = nowIso();
      collections = [
        ...collections,
        {
          collection_id: id,
          name: trimmed,
          description: null,
          profile_count: 0,
          created_at: ts,
          updated_at: ts,
        },
      ];
      membership.set(id, new Set());
      return id;
    });

    map.set('collection_delete', async (args): Promise<null> => {
      const { collection_id } = args as { collection_id: string };
      collections = collections.filter((c) => c.collection_id !== collection_id);
      membership.delete(collection_id);
      return null;
    });

    map.set('collection_add_profile', async (args): Promise<null> => {
      const { collection_id, profile_name } = args as {
        collection_id: string;
        profile_name: string;
      };
      if (!findById(collection_id)) {
        throw new Error(`[dev-mock] collection_add_profile: collection not found: ${collection_id}`);
      }
      // Mirror the Task 2 fix: unknown profile name must surface as an error,
      // not a silent success. In mock-land we accept any profile_name since we
      // do not track the profile index — but empty string is still invalid.
      if (!profile_name?.trim()) {
        throw new Error('[dev-mock] collection_add_profile: profile_name must not be empty');
      }
      const set = membership.get(collection_id) ?? new Set<string>();
      set.add(profile_name);
      membership.set(collection_id, set);
      return null;
    });

    map.set('collection_remove_profile', async (args): Promise<null> => {
      const { collection_id, profile_name } = args as {
        collection_id: string;
        profile_name: string;
      };
      // Idempotent — matches Rust semantics at collections.rs:117-120.
      membership.get(collection_id)?.delete(profile_name);
      return null;
    });

    map.set('collection_list_profiles', async (args): Promise<string[]> => {
      const { collection_id } = args as { collection_id: string };
      const set = membership.get(collection_id);
      return set ? [...set].sort() : [];
    });

    map.set('collection_rename', async (args): Promise<null> => {
      const { collection_id, new_name } = args as {
        collection_id: string;
        new_name: string;
      };
      const trimmed = (new_name ?? '').trim();
      if (!trimmed) {
        throw new Error('[dev-mock] collection_rename: collection name must not be empty');
      }
      const target = findById(collection_id);
      if (!target) {
        throw new Error(`[dev-mock] collection_rename: collection not found: ${collection_id}`);
      }
      if (collections.some((c) => c.collection_id !== collection_id && c.name === trimmed)) {
        throw new Error(`[dev-mock] collection_rename: duplicate collection name: ${trimmed}`);
      }
      target.name = trimmed;
      target.updated_at = nowIso();
      return null;
    });

    map.set('collection_update_description', async (args): Promise<null> => {
      const { collection_id, description } = args as {
        collection_id: string;
        description: string | null;
      };
      const target = findById(collection_id);
      if (!target) {
        throw new Error(`[dev-mock] collection_update_description: collection not found: ${collection_id}`);
      }
      const normalized = description?.trim();
      target.description = normalized ? normalized : null;
      target.updated_at = nowIso();
      return null;
    });

    map.set('collections_for_profile', async (args): Promise<MockCollectionRow[]> => {
      const { profile_name } = args as { profile_name: string };
      recomputeProfileCounts();
      return collections
        .filter((c) => membership.get(c.collection_id)?.has(profile_name))
        .sort((a, b) => a.name.localeCompare(b.name));
    });
  }
  ```

- **MIRROR**: `handlers/community.ts:100-165` for the `register*` + `map.set` shape; `handlers/profile.ts:406-419` for mutator-with-validation style.
- **IMPORTS**: only `import type { Handler } from './types';`. Do NOT import from `../index.ts` (circular).
- **GOTCHA**:
  - **Every thrown error MUST start with `[dev-mock]`** — the release workflow sentinel greps for this literal string in the production bundle. Consistency is mandatory.
  - **Never use `any`** — cast `args as { ... }` per CLAUDE.md type safety rule.
  - Mock seed fixture uses a synthetic ID prefix (`mock-collection-*`) not a real steam app id — complies with `.github/workflows/fixture-lint.yml` (no real PII / IDs in mock data).
  - `recomputeProfileCounts()` is called on every list-returning handler to keep counts in sync with the membership map (no normalization DB to rely on).
  - Return `null` (not `undefined`, not `void`) from mutator handlers to match the Tauri `Result<(), String>` → `null` serialization convention used by `handlers/profile.ts:419`.
  - **Do NOT seed the `mock-collection-1` fixture with profiles** — the mock `profile_name` index is not synced with the Profile handler's fixture store. Phase 2 can populate via the UI instead.
- **VALIDATE**:
  - `pnpm --dir src/crosshook-native type-check` passes (strict TS)
  - `pnpm --dir src/crosshook-native build:web` (or whatever builds the browser dev bundle) succeeds
  - Starting `./scripts/dev-native.sh --browser` and invoking any collection command via the devtools console returns expected data / errors — see Manual Validation

### Task 9: Register the new mock handler in the barrel

- **ACTION**: Import + call `registerCollections(map)` in `src/crosshook-native/src/lib/mocks/index.ts`.
- **IMPLEMENT**: In `src/crosshook-native/src/lib/mocks/index.ts`:
  1. Add the import at the top next to the other `register*` imports (around line 21-24):

     ```ts
     import { registerCommunity } from './handlers/community';
     import { registerLauncher } from './handlers/launcher';
     import { registerLibrary } from './handlers/library';
     import { registerSystem } from './handlers/system';
     import { registerCollections } from './handlers/collections'; // ← NEW
     ```

  2. Add the call inside `registerMocks()` (line 49, just before `registerSystem(map)` or grouped with the other Phase-3 catalog handlers):

     ```ts
     registerCommunity(map);
     registerLauncher(map);
     registerLibrary(map);
     registerSystem(map);
     registerCollections(map); // ← NEW
     ```

- **MIRROR**: The existing `registerCommunity` / `registerLauncher` import-and-register pair.
- **IMPORTS**: `registerCollections` from `./handlers/collections`.
- **GOTCHA**:
  - `registerCollections` must be called **before** `wrapAllHandlers(map)` (the last line of `registerMocks`) — adding it after would bypass the debug-toggle middleware.
  - Alphabetical grouping is not enforced; place it near related domain handlers. The safest slot is the end of the "Phase 2 domain handlers" block.
- **VALIDATE**: `pnpm --dir src/crosshook-native type-check` passes; no `Unhandled command: collection_*` errors from a browser dev session.

### Task 10: Add migration test for 18 → 19

- **ACTION**: Add an in-line test in `migrations.rs` (under the existing `#[cfg(test)] mod tests`) that verifies the migration applies cleanly, the `sort_order` column exists with default 0, and the FK cascade works.
- **IMPLEMENT**: Append to `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` inside the `mod tests` block (around line 1078, after `migration_17_to_18_creates_trainer_sources_table`):

  ```rust
  #[test]
  fn migration_18_to_19_adds_sort_order_and_cascade() {
      let conn = db::open_in_memory().unwrap();
      run_migrations(&conn).unwrap();

      let version: u32 = conn
          .pragma_query_value(None, "user_version", |row| row.get(0))
          .unwrap();
      assert_eq!(version, 19);

      // 1. sort_order column exists with NOT NULL DEFAULT 0.
      let mut stmt = conn.prepare("PRAGMA table_info(collections)").unwrap();
      let columns: Vec<(String, String, i64)> = stmt
          .query_map([], |row| {
              Ok((
                  row.get::<_, String>(1)?,  // name
                  row.get::<_, String>(2)?,  // type
                  row.get::<_, i64>(3)?,     // notnull
              ))
          })
          .unwrap()
          .collect::<Result<Vec<_>, _>>()
          .unwrap();
      let sort_order = columns
          .iter()
          .find(|(name, _, _)| name == "sort_order")
          .expect("sort_order column should exist");
      assert_eq!(sort_order.1, "INTEGER");
      assert_eq!(sort_order.2, 1, "sort_order should be NOT NULL");

      // 2. collection_profiles.profile_id FK has ON DELETE CASCADE.
      // Insert a profile, add to a collection, delete the profile, verify the
      // membership row cascades away.
      conn.execute(
          "INSERT INTO profiles (profile_id, current_filename, current_path, game_name, created_at, updated_at)
           VALUES ('pf-1', 'game.toml', '/tmp/game.toml', 'Game', datetime('now'), datetime('now'))",
          [],
      )
      .unwrap();
      conn.execute(
          "INSERT INTO collections (collection_id, name, created_at, updated_at)
           VALUES ('col-1', 'Test', datetime('now'), datetime('now'))",
          [],
      )
      .unwrap();
      conn.execute(
          "INSERT INTO collection_profiles (collection_id, profile_id, added_at)
           VALUES ('col-1', 'pf-1', datetime('now'))",
          [],
      )
      .unwrap();

      conn.execute("DELETE FROM profiles WHERE profile_id = 'pf-1'", [])
          .unwrap();

      let orphan_count: i64 = conn
          .query_row(
              "SELECT COUNT(*) FROM collection_profiles WHERE profile_id = 'pf-1'",
              [],
              |row| row.get(0),
          )
          .unwrap();
      assert_eq!(
          orphan_count, 0,
          "collection_profiles rows must cascade when the profile is deleted"
      );

      // 3. Collection→collection_profiles cascade still works (regression check).
      let member_count: i64 = conn
          .query_row(
              "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = 'col-1'",
              [],
              |row| row.get(0),
          )
          .unwrap();
      assert_eq!(member_count, 0);
  }
  ```

- **MIRROR**: `migration_14_to_15_creates_prefix_dependency_state_table` at `migrations.rs:909-970` — same style: run all migrations, assert version, assert table + index existence, exercise a cascade end-to-end.
- **IMPORTS**: none new — `db`, `run_migrations`, `Connection`, `rusqlite::params` are already imported in the test module.
- **GOTCHA**:
  - The FK cascade test requires `PRAGMA foreign_keys = ON` at the connection level. `db::open_in_memory` must already enable this — if the test fails with "orphan_count = 1", verify `db.rs` sets the pragma on open. Do NOT toggle the pragma inside the test; do NOT toggle it inside the migration.
  - Use `[]` (empty slice) for parameter-less `execute` calls, per the existing style in `migrations.rs:843-852`.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core migration_18_to_19_adds_sort_order_and_cascade` passes.

### Task 11: Test — `add_profile_to_collection` returns typed error on missing profile

- **ACTION**: Add a test in `mod.rs` that asserts the Task 2 fix — unknown profile name returns `Validation` error.
- **IMPLEMENT**: Append to the "Phase 3: Collections tests" section in `mod.rs` (after `test_add_profile_to_collection` at line 2553):

  ```rust
  #[test]
  fn test_add_profile_to_collection_missing_profile_errors() {
      let store = MetadataStore::open_in_memory().unwrap();
      let collection_id = store.create_collection("Ghosts").unwrap();

      let result = store.add_profile_to_collection(&collection_id, "does-not-exist");

      match result {
          Err(MetadataStoreError::Validation(msg)) => {
              assert!(
                  msg.contains("does-not-exist"),
                  "error message should include the missing profile name, got: {msg}"
              );
          }
          other => panic!("expected Validation error, got {other:?}"),
      }

      // Verify no row was inserted.
      let conn = connection(&store);
      let count: i64 = conn
          .query_row(
              "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
              params![collection_id],
              |row| row.get(0),
          )
          .unwrap();
      assert_eq!(count, 0);
  }
  ```

- **MIRROR**: `test_add_profile_to_collection` (line 2529) for the setup; `test_create_collection_returns_id` for the direct SQL assertion.
- **IMPORTS**: none — `MetadataStoreError` is re-exported from `metadata/mod.rs:24-31` and in-scope for the test module.
- **GOTCHA**:
  - `MetadataStoreError::Validation(msg)` is a tuple match (`Validation(msg)`), not struct match (`Validation { msg }`). Matches the variant declaration at `models.rs:21`.
  - Do NOT bypass the validation by pre-inserting a profile — this test specifically exercises the missing-profile branch.
- **VALIDATE**: Test passes against the fixed `add_profile_to_collection`; the same test fails against the pre-fix version (quick regression confidence check).

### Task 12: Tests — rename / update_description / collections_for_profile

- **ACTION**: Add 5 tests covering the new wrappers and edge cases.
- **IMPLEMENT**: Append to "Phase 3: Collections tests" section in `mod.rs`:

  ```rust
  #[test]
  fn test_rename_collection_updates_name() {
      let store = MetadataStore::open_in_memory().unwrap();
      let id = store.create_collection("Old Name").unwrap();

      store.rename_collection(&id, "New Name").unwrap();

      let collections = store.list_collections().unwrap();
      assert_eq!(collections.len(), 1);
      assert_eq!(collections[0].name, "New Name");
  }

  #[test]
  fn test_rename_collection_unknown_id_errors() {
      let store = MetadataStore::open_in_memory().unwrap();
      let result = store.rename_collection("nope", "Whatever");
      assert!(matches!(result, Err(MetadataStoreError::Validation(_))));
  }

  #[test]
  fn test_rename_collection_duplicate_name_errors() {
      let store = MetadataStore::open_in_memory().unwrap();
      let _ = store.create_collection("A").unwrap();
      let id_b = store.create_collection("B").unwrap();

      // Duplicate name violates the UNIQUE constraint on collections.name.
      let result = store.rename_collection(&id_b, "A");
      assert!(
          matches!(result, Err(MetadataStoreError::Database { .. })),
          "duplicate name should bubble as a Database error (UNIQUE violation)"
      );
  }

  #[test]
  fn test_update_collection_description_set_and_clear() {
      let store = MetadataStore::open_in_memory().unwrap();
      let id = store.create_collection("Target").unwrap();

      store
          .update_collection_description(&id, Some("a helpful description"))
          .unwrap();
      let row = store
          .list_collections()
          .unwrap()
          .into_iter()
          .next()
          .unwrap();
      assert_eq!(row.description.as_deref(), Some("a helpful description"));

      // Clearing with Some("") normalizes to None.
      store
          .update_collection_description(&id, Some("   "))
          .unwrap();
      let row = store
          .list_collections()
          .unwrap()
          .into_iter()
          .next()
          .unwrap();
      assert_eq!(row.description, None);

      // Clearing with None also works.
      store
          .update_collection_description(&id, Some("again"))
          .unwrap();
      store.update_collection_description(&id, None).unwrap();
      let row = store
          .list_collections()
          .unwrap()
          .into_iter()
          .next()
          .unwrap();
      assert_eq!(row.description, None);
  }

  #[test]
  fn test_collections_for_profile_returns_multi_membership() {
      let store = MetadataStore::open_in_memory().unwrap();
      let profile = sample_profile();
      let path = std::path::Path::new("/profiles/elden-ring.toml");
      store
          .observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None)
          .unwrap();

      let id_a = store.create_collection("Action").unwrap();
      let id_b = store.create_collection("Backlog").unwrap();
      let _id_c = store.create_collection("Untouched").unwrap();

      store
          .add_profile_to_collection(&id_a, "elden-ring")
          .unwrap();
      store
          .add_profile_to_collection(&id_b, "elden-ring")
          .unwrap();

      let result = store.collections_for_profile("elden-ring").unwrap();
      assert_eq!(result.len(), 2);
      let names: Vec<&str> = result.iter().map(|c| c.name.as_str()).collect();
      assert!(names.contains(&"Action"));
      assert!(names.contains(&"Backlog"));
      assert!(!names.contains(&"Untouched"));

      // Unknown profile name returns empty vec (not error).
      let empty = store.collections_for_profile("nobody").unwrap();
      assert!(empty.is_empty());
  }

  #[test]
  fn test_profile_delete_cascades_collection_membership() {
      let store = MetadataStore::open_in_memory().unwrap();
      let profile = sample_profile();
      let path = std::path::Path::new("/profiles/vanishing.toml");
      store
          .observe_profile_write("vanishing", &profile, path, SyncSource::AppWrite, None)
          .unwrap();

      let collection_id = store.create_collection("Ephemeral").unwrap();
      store
          .add_profile_to_collection(&collection_id, "vanishing")
          .unwrap();

      // Hard-delete the profile row (bypassing the soft-delete code path, which
      // only sets deleted_at). We simulate a hard delete to verify the FK cascade.
      let conn = connection(&store);
      conn.execute(
          "DELETE FROM profiles WHERE current_filename = 'vanishing'",
          [],
      )
      .unwrap();
      drop(conn);

      // Membership row must be gone.
      let conn = connection(&store);
      let count: i64 = conn
          .query_row(
              "SELECT COUNT(*) FROM collection_profiles WHERE collection_id = ?1",
              params![collection_id],
              |row| row.get(0),
          )
          .unwrap();
      assert_eq!(count, 0, "collection_profiles row must cascade on profile delete");
  }
  ```

- **MIRROR**: `test_add_profile_to_collection` (line 2529), `test_collection_delete_cascades` (line 2555), `test_set_profile_favorite_toggles` (line 2586).
- **IMPORTS**: none new.
- **GOTCHA**:
  - `test_rename_collection_duplicate_name_errors` asserts `Database { .. }` not `Validation` — the UNIQUE constraint fires at the SQLite layer and bubbles through `map_err(|source| MetadataStoreError::Database { ... })`. This is intentional and correct (do not pre-check in Rust — let SQL enforce).
  - `test_profile_delete_cascades_collection_membership` does a **raw SQL hard-delete**, not `store.observe_profile_delete(...)` which is a soft-delete (sets `deleted_at`). We test the FK cascade, which only fires on hard-delete.
  - `drop(conn)` before re-locking is needed because `connection(&store)` returns a `MutexGuard` and a second call would deadlock without the drop. This pattern matches `test_set_profile_favorite_toggles` at `mod.rs:2608-2611`.
  - `sample_profile()` at `mod.rs:1463` produces the canonical test profile — reuse it, do not construct ad-hoc.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes all Collection tests.

### Task 13: Remove or tighten the stale `tracing::warn!` reference (nothing to do — handled in Task 2)

_Informational — this was folded into Task 2 (the `tracing::warn!` on `collections.rs:90-94` is deleted as part of the no-op fix). No separate task needed._

---

## Testing Strategy

### Unit Tests

| Test                                                    | Input                                   | Expected Output                                          | Edge Case?                   |
| ------------------------------------------------------- | --------------------------------------- | -------------------------------------------------------- | ---------------------------- |
| `migration_18_to_19_adds_sort_order_and_cascade`        | fresh in-memory DB                      | version == 19, `sort_order` col exists, FK cascade works | ✓ (new test)                 |
| `test_add_profile_to_collection_missing_profile_errors` | unknown profile name                    | `Err(Validation("profile not found: ..."))`              | ✓ (error path)               |
| `test_rename_collection_updates_name`                   | valid id + new name                     | name updated, `updated_at` refreshed                     | happy path                   |
| `test_rename_collection_unknown_id_errors`              | bogus id                                | `Err(Validation(...))`                                   | ✓ (error path)               |
| `test_rename_collection_duplicate_name_errors`          | existing name                           | `Err(Database { .. })` (UNIQUE violation)                | ✓ (constraint path)          |
| `test_update_collection_description_set_and_clear`      | `Some("text")` → `Some("   ")` → `None` | description set, then `NULL`, then `NULL`                | ✓ (whitespace normalization) |
| `test_collections_for_profile_returns_multi_membership` | profile in 2 of 3 collections           | `Vec<CollectionRow>` length 2, correct names             | ✓ (multi-membership)         |
| `test_profile_delete_cascades_collection_membership`    | hard-delete profile with membership row | membership row removed via FK cascade                    | ✓ (FK cascade)               |

**Total: 8 new tests.** All use `MetadataStore::open_in_memory()` — zero filesystem I/O.

### Edge Cases Checklist

- [x] **Empty input** — `rename_collection("", ...)` returns `Validation`; `create_collection("")` already handled
- [x] **Whitespace-only input** — `update_collection_description(Some("   "))` normalizes to `None`
- [x] **Missing profile** — `add_profile_to_collection` returns typed error; `collections_for_profile` returns empty vec
- [x] **Duplicate name** — `rename_collection` bubbles UNIQUE constraint as `Database` error
- [x] **Unknown collection id** — `rename_collection` / `update_collection_description` return `Validation` error when `affected == 0`
- [x] **Multi-membership** — `collections_for_profile` returns all collections containing a profile
- [x] **FK cascade on profile hard-delete** — orphan rows are cleaned up automatically
- [x] **Disabled store** — `list_collections()` returns `Ok(vec![])`, `rename_collection()` returns `Ok(())` (via `with_conn` + `T: Default`)
- [ ] **Concurrent access** — not relevant at this layer; `MetadataStore` uses a `Mutex<Connection>` and operations are serialized
- [ ] **Permission denied** — not relevant; in-memory only for tests, real DB path managed by `db.rs`

### Integration Coverage (manual, post-implementation)

- [ ] `./scripts/dev-native.sh --browser` starts without crash on collection IPC calls
- [ ] Devtools console `invoke('collection_list')` returns seed fixture
- [ ] Devtools console `invoke('collection_create', { name: 'Test' })` returns an id; subsequent `invoke('collection_list')` shows it
- [ ] Devtools console `invoke('collection_add_profile', { collection_id: 'x', profile_name: '' })` throws `[dev-mock] ... must not be empty`
- [ ] `./scripts/build-native.sh --binary-only` builds and the released binary does NOT contain any `[dev-mock]` string (`grep '\[dev-mock\]' src/crosshook-native/dist/assets/*.js || echo OK`)

---

## Validation Commands

### Static Analysis (Rust)

```bash
cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
cargo check --manifest-path src/crosshook-native/src-tauri/Cargo.toml
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings
```

**EXPECT**: Zero errors, zero new warnings.

### Rust Unit + Integration Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

**EXPECT**: All tests pass. Focus outputs to verify:

```
test metadata::migrations::tests::migration_18_to_19_adds_sort_order_and_cascade ... ok
test metadata::tests::test_add_profile_to_collection_missing_profile_errors ... ok
test metadata::tests::test_rename_collection_updates_name ... ok
test metadata::tests::test_rename_collection_unknown_id_errors ... ok
test metadata::tests::test_rename_collection_duplicate_name_errors ... ok
test metadata::tests::test_update_collection_description_set_and_clear ... ok
test metadata::tests::test_collections_for_profile_returns_multi_membership ... ok
test metadata::tests::test_profile_delete_cascades_collection_membership ... ok
```

### Full Tauri Build (link check)

```bash
cargo check --manifest-path src/crosshook-native/src-tauri/Cargo.toml --all-targets
```

**EXPECT**: `tauri::generate_handler!` macro expansion succeeds — a missing comma or typo in Task 7 fails here.

### Frontend Static Analysis (TS)

```bash
pnpm --dir src/crosshook-native type-check
# or whatever the repo script is — see package.json
```

**EXPECT**: Zero type errors in `src/lib/mocks/handlers/collections.ts` and `src/lib/mocks/index.ts`.

### Browser Dev-Mode Smoke

```bash
./scripts/dev-native.sh --browser
# Open http://localhost:<port>/, open devtools, then in console:
await window.__TAURI_INTERNALS__ ? true : true  // confirm browser mode
# Run these via the app or a temporary harness:
# invoke('collection_list')
# invoke('collection_create', { name: 'Smoke' })
# invoke('collection_rename', { collection_id: '<id>', new_name: 'Renamed' })
# invoke('collection_add_profile', { collection_id: '<id>', profile_name: '' })
#   → throws [dev-mock] ... must not be empty
```

**EXPECT**: Every command resolves or throws a `[dev-mock]`-prefixed error. No `Unhandled command: collection_*` errors.

### Production Bundle Sentinel (matches CI)

```bash
./scripts/build-native.sh --binary-only
grep -l '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' \
  src/crosshook-native/dist/assets/*.js 2>/dev/null \
  && echo "❌ mock code leaked into production bundle" \
  || echo "✅ no mock code in production bundle"
```

**EXPECT**: `✅ no mock code in production bundle`. This mirrors `.github/workflows/release.yml:105-120`.

### Manual Validation

- [ ] Schema version after fresh launch = 19 (`sqlite3 ~/.local/share/crosshook/metadata.db 'PRAGMA user_version;'` — expect `19`)
- [ ] `sqlite3 ~/.local/share/crosshook/metadata.db '.schema collections'` shows `sort_order INTEGER NOT NULL DEFAULT 0`
- [ ] `sqlite3 ~/.local/share/crosshook/metadata.db '.schema collection_profiles'` shows `profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE`
- [ ] No regressions in the existing `test_collection_delete_cascades` and `test_add_profile_to_collection` tests
- [ ] Browser dev-mode session with `?fixture=default`: collection IPC calls succeed, inspection of devtools console shows no red errors on boot

---

## Acceptance Criteria

- [ ] Migration 18 → 19 created with FK cascade + `sort_order` column
- [ ] `add_profile_to_collection` returns `Validation` error on missing profile
- [ ] `MetadataStore::rename_collection` / `update_collection_description` / `collections_for_profile` implemented and wired through free functions
- [ ] 3 new Tauri commands (`collection_rename`, `collection_update_description`, `collections_for_profile`) registered in `tauri::generate_handler!`
- [ ] `CollectionRow` no longer has `#[allow(dead_code)]`
- [ ] `list_collections` SQL uses `ORDER BY sort_order ASC, name ASC`
- [ ] Browser dev-mode handler file `src/lib/mocks/handlers/collections.ts` covers all 9 commands
- [ ] Mock handler registered in `src/lib/mocks/index.ts`
- [ ] **8 new tests** in place and green (1 migration, 7 metadata store)
- [ ] All existing Collection tests still pass (no regressions)
- [ ] `cargo test -p crosshook-core` returns zero failures
- [ ] `cargo check` on both `crosshook-core` and `src-tauri` succeed with zero new warnings
- [ ] `pnpm --dir src/crosshook-native type-check` succeeds
- [ ] `./scripts/dev-native.sh --browser` does not crash on collection IPC
- [ ] Production bundle does not contain any `[dev-mock]` strings

## Completion Checklist

- [ ] Code follows discovered patterns (free function + `with_conn` wrapper + `#[tauri::command]` handler)
- [ ] Error handling uses `MetadataStoreError::Validation(String)` tuple variant — never struct syntax
- [ ] All mock error messages start with `[dev-mock]`
- [ ] Tests mirror `test_create_collection_returns_id` / `test_add_profile_to_collection` structure
- [ ] No hardcoded schema version constants introduced
- [ ] No new dependencies added
- [ ] `#[allow(dead_code)]` removed only from `CollectionRow` (not from other rows)
- [ ] `remove_profile_from_collection` remains idempotent (not touched)
- [ ] Commit follows Conventional Commits: `feat(core): collections backend foundation (schema v19, typed errors, new IPC)` or split into multiple commits per CLAUDE.md
- [ ] No frontend consumer changes — Phase 1 is backend-only + mock layer
- [ ] Self-contained — no questions needed during implementation

## Risks

| Risk                                                                                                                                              | Likelihood | Impact | Mitigation                                                                                                                                                                                               |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PRD's "schema v5" reference misleads implementation; wrong migration number produces a no-op migration that runs on existing v18 installs         | **Medium** | High   | Plan explicitly targets v18→v19; Task 1 test asserts `version == 19` after `run_migrations` to fail loudly on off-by-one                                                                                 |
| `PRAGMA foreign_keys = ON` not enforced in `db.rs`, so the FK cascade migration runs but the cascade does not fire at runtime                     | **Low**    | High   | Task 10 test exercises an end-to-end cascade (insert → delete → assert count) against an in-memory DB opened via `db::open_in_memory`. If the pragma is off, the test fails before merge                 |
| Task 3 `rename_collection` UNIQUE collision produces a `Database` error the frontend cannot disambiguate from a transient SQL error               | **Low**    | Low    | Document the behavior in Task 3 GOTCHA; Phase 2 can inspect the error string for "UNIQUE constraint failed: collections.name" if needed. Optional follow-up: pre-check in Rust for a cleaner error class |
| `MetadataStoreError::Validation` syntax mistake — implementer uses struct syntax (`Validation { ... }`) per stale PRD example                     | **Medium** | Low    | Plan provides verbatim tuple-variant examples and a dedicated PATTERN snippet showing both correct and incorrect forms. Compiler catches it immediately                                                  |
| Mock handler missing `[dev-mock]` prefix on an error string, causing the release.yml grep sentinel to miss it OR mock code sneaks into production | **Low**    | Medium | Task 8 specifies the convention; the production-bundle sentinel validation command catches leaks before merge. Review all `throw new Error` lines in the new file                                        |
| Adding `sort_order INTEGER NOT NULL DEFAULT 0` to an existing table with data fails because SQLite requires a constant default                    | **Low**    | Medium | `DEFAULT 0` is a constant — this is a false risk but worth noting. Alternative fallback: `ALTER TABLE + UPDATE` style from `migrate_1_to_2` if the single-statement fails unexpectedly                   |
| `tauri::generate_handler!` macro error on missing comma after new lines                                                                           | **Low**    | Low    | Plan snippet includes trailing commas; `cargo check` catches it                                                                                                                                          |
| `with_conn` `T: Default` constraint not met for new methods returning a type without `Default`                                                    | **Low**    | Low    | All 3 new return types (`()`, `Vec<CollectionRow>`) implement `Default`. Verified                                                                                                                        |
| Existing `test_add_profile_to_collection` regresses because the new error path interferes                                                         | **Low**    | Low    | The existing test inserts a profile BEFORE calling add; the fix only affects the missing-profile branch. Test will still pass                                                                            |
| Frontend mock seed fixture collides with a future real-data check in `.github/workflows/fixture-lint.yml`                                         | **Low**    | Low    | Mock uses `mock-collection-*` ids and synthetic created_at — no real Steam ids, no PII                                                                                                                   |

## Notes

### Key divergences from the PRD

- **PRD says "schema v5", actual target is v19.** The collections tables were introduced in `migrate_3_to_4` (schema v4) and the current schema is at v18 (`migrations.rs:165-172`). The next migration must be **18 → 19**. The test in Task 10 asserts `version == 19` to catch any confusion.
- **PRD uses struct syntax for `MetadataStoreError::Validation { ... }`**. The actual variant is a **tuple** variant (`models.rs:21` — `Validation(String)`). The plan uses the correct tuple syntax everywhere; do not copy from the PRD verbatim.
- **PRD lists `sort_order` addition without a column placement.** We add it as `NOT NULL DEFAULT 0` in the single migration transaction alongside the FK rebuild. Phase 2 will add a setter.
- **PRD omits that mocks are 100% greenfield.** A grep of `src/crosshook-native/src` for `collection_list|invoke\('collection_` returns zero hits. All 9 commands need fresh mock handlers (6 existing + 3 new); the existing 6 Rust commands have never had a frontend caller.

### Things that look concerning but are actually fine

- **`remove_profile_from_collection` silent no-op on missing profile is intentional.** Idempotent DELETE semantics match the SQL convention. Only the ADD path changes behavior.
- **`#[allow(dead_code)]` removal is safe.** `CollectionRow` is already used across IPC via `Result<Vec<CollectionRow>, String>` in `commands/collections.rs:11`. The attribute was added defensively when the struct was introduced and is now stale.
- **`with_conn`'s `T: Default` constraint is not a footgun here.** All three new methods return types that implement `Default`.
- **No `verify:no-mocks` script to run locally.** The dev-time helper `pnpm dev:browser:check` (= `scripts/check-mock-coverage.sh`) is contributor convenience only; the real CI gate is the `release.yml:105-120` workflow step. Both are already in place — Phase 1 just needs to respect the `[dev-mock]` prefix convention.

### Future phases that depend on Phase 1

- **Phase 2** — requires: the 3 new commands (rename, update_description, collections_for_profile), the browser mock handlers (sidebar and modal make IPC calls on mount), and the sort_order column (for sidebar display ordering).
- **Phase 3** — requires: the typed error on `add_profile_to_collection` (so the launch chain does not silently skip a member profile when the fixture is stale), and is more comfortable with the FK cascade (so `collection_launch_defaults` rows can also cascade off `collections` later).
- **Phase 4** — requires: rename + update_description APIs (the TOML import review modal uses them after disambiguation), and the mock handlers (so `pnpm dev:browser` can exercise the import flow end-to-end).
- **Phase 5** — integration and polish; depends on everything above.

### Conventional Commit suggestions (CLAUDE.md MUST)

Pick one per logical grouping:

```text
feat(core): backend foundation for profile collections (schema v19, typed errors, new IPC)
feat(core): schema v19 migration — FK cascade on collection_profiles + sort_order column
feat(core): add collection_rename / update_description / collections_for_profile IPC
fix(core): add_profile_to_collection returns Validation error on missing profile
feat(ui): browser dev-mode mocks for all 9 collection IPC commands
```

Tag the PR with `type:feature`, `area:profiles`, `priority:high` per the CLAUDE.md label taxonomy. Link with `Closes #73` (the source GitHub issue from the PRD).
