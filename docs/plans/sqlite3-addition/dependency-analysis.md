# Dependency Graph Analysis - SQLite3 Addition Plan

## Executive Summary

✅ **Graph Status: HEALTHY**

- No circular dependencies
- No missing dependencies
- No orphaned tasks
- 3 parallelization opportunities identified
- All dependencies correct and necessary

**Critical Path Length:** 8 sequential tasks (can be optimized to 7 with parallel execution)
**Parallelizable Task Groups:** (1.1|2.1), (3.2|3.3), (4.2|4.3)

---

## Task Inventory (11 Tasks)

```
Phase 1:  1.1
Phase 2:  2.1, 2.2
Phase 3:  3.1, 3.2, 3.3, 3.4
Phase 4:  4.1, 4.2, 4.3
Phase 5:  5.1
```

---

## Dependency Graph (Text DAG)

```
1.1 (utility)                      [no deps]
    └─→ 4.2

2.1 (dependencies)                 [no deps]
    └─→ 2.2
        └─→ 3.1
            ├─→ 3.2
            ├─→ 3.3
            └─→ 3.4
                └─→ 4.1
                    ├─→ 4.2 (also needs 1.1)
                    └─→ 4.3
                        └─→ 5.1
                            ↑
                        4.2 ─┘
```

---

## Detailed Dependency Verification

### Task 1.1: Promote `sanitize_display_path()` utility

- **Declared Dependencies:** none ✅
- **Downstream Consumers:** 4.2 ✅
- **Correctness:** Utility is extracted from existing code, no upstream work needed
- **Assessment:** ✅ CORRECT

### Task 2.1: Add `rusqlite` and `uuid` dependencies

- **Declared Dependencies:** none ✅
- **Downstream Consumers:** 2.2 (module skeleton needs these for re-exports) ✅
- **Correctness:** No code from other tasks is needed to update Cargo.toml
- **Assessment:** ✅ CORRECT

### Task 2.2: Create metadata module skeleton

- **Declared Dependencies:** 2.1 ✅
- **Upstream Requirement:** Cargo.toml must list dependencies first
- **Downstream Consumers:** 3.1 (stub methods exist here) ✅
- **Correctness:** Module structure must exist before concrete implementations
- **Assessment:** ✅ CORRECT

### Task 3.1: Create `models.rs` — error types and row structs

- **Declared Dependencies:** 2.2 ✅
- **Upstream Requirement:** Module file must exist to add new submodule
- **Downstream Consumers:** 3.2, 3.3, 3.4 (all error handling depends on MetadataStoreError) ✅
- **Critical:** Every other metadata module depends on `MetadataStoreError` from here
- **Assessment:** ✅ CORRECT — This is the linchpin of Phase 3

### Task 3.2: Create `db.rs` — connection factory

- **Declared Dependencies:** 3.1 ✅
- **Upstream Requirement:** `MetadataStoreError` enum must exist for error returns
- **Downstream Consumers:** 3.4 (uses `db::new_id()`), 4.1 (uses connection) ✅
- **Correctness:** Isolated SQL implementation, uses types from 3.1
- **Assessment:** ✅ CORRECT

### Task 3.3: Create `migrations.rs` — schema DDL

- **Declared Dependencies:** 3.1 ✅
- **Upstream Requirement:** `MetadataStoreError` for error returns
- **Downstream Consumers:** Indirectly 3.4 and 4.1 (schema must exist for queries)
- **Note:** 3.4 (profile_sync) issues queries but doesn't explicitly call migration runner. Caller (4.1) runs migrations first.
- **Assessment:** ✅ CORRECT

### Task 3.4: Create `profile_sync.rs` — profile lifecycle reconciliation

- **Declared Dependencies:** 3.1, 3.2, 3.3 ✅
- **Upstream Requirements:**
  - 3.1: `MetadataStoreError`, `SyncSource`, `SyncReport` enums/structs
  - 3.2: `db::new_id()` function for UUID generation
  - 3.3: Schema tables defined (implicit; doesn't call migration runner directly)
- **Downstream Consumers:** 4.1 (metadata_store.sync_profiles_from_store calls this), 5.1 (tested) ✅
- **Dependency Necessity Check:**
  - Needs 3.1? YES — uses MetadataStoreError for returns, SyncSource/SyncReport for signatures
  - Needs 3.2? YES — `db::new_id()` called for new profile UUIDs in `observe_profile_write`
  - Needs 3.3? YES (semantically) — queries will fail without schema; migrations run before this is called
- **Assessment:** ✅ CORRECT — All three dependencies justified

### Task 4.1: Register `MetadataStore` in Tauri app

- **Declared Dependencies:** 3.2, 3.3, 3.4 ✅
- **Upstream Requirements:**
  - 3.2: Connection factory (mod.rs creates connections)
  - 3.3: Migrations must be runnable
  - 3.4: `sync_profiles_from_store()` is called in startup
- **Downstream Consumers:** 4.2, 4.3 (both use `metadata_store` parameter), 5.1 (integration tests) ✅
- **Correctness:** MetadataStore wrapper can't be initialized until all submodules exist
- **Assessment:** ✅ CORRECT

### Task 4.2: Add metadata sync hooks to profile commands

- **Declared Dependencies:** 1.1, 4.1 ✅
- **Upstream Requirements:**
  - 1.1: `sanitize_display_path` utility is imported
  - 4.1: MetadataStore is in Tauri state and available via `State<'_, MetadataStore>`
- **Downstream Consumers:** 5.1 (integration tests call these commands) ✅
- **Correctness:** Can't invoke metadata hooks until store exists; don't need sanitize_path yet, but it's already available from 1.1
- **Assessment:** ✅ CORRECT

### Task 4.3: Add startup reconciliation scan

- **Declared Dependencies:** 4.1 ✅
- **Upstream Requirement:** `MetadataStore` must be registered and available
- **Downstream Consumers:** 5.1 (tests this path) ✅
- **Note:** Does NOT depend on 4.2 even though both use MetadataStore — they're orthogonal (one hooks mutations, one runs at startup)
- **Assessment:** ✅ CORRECT

### Task 5.1: Add metadata module unit and integration tests

- **Declared Dependencies:** 4.2, 4.3 ✅
- **Upstream Requirements:**
  - 4.2: Profile commands with hooks exist (can test end-to-end launch → metadata mutation)
  - 4.3: Startup reconciliation path exists (can test reconciliation scenario)
- **Alternative Consideration:** Unit tests in 3.1–3.4 modules could theoretically run after each task using `open_in_memory()`. However:
  - Current plan puts all tests in 5.1 for coherence
  - Tests described include integration scenarios (profile commands → metadata mutations)
  - This is a design choice, not a dependency error
- **Downstream Consumers:** None (final validation task) ✅
- **Assessment:** ✅ CORRECT

---

## 1. Circular Dependencies Analysis

**Result: NONE FOUND ✅**

Traversal from each node:

- 1.1 → 4.2 → ∅ (4.2 has no outgoing edges)
- 2.1 → 2.2 → 3.1 → {3.2, 3.3, 3.4} → 4.1 → {4.2, 4.3} → 5.1 → ∅
- No backward edges, no cycles detected

---

## 2. Missing Dependencies Analysis

### Check: File Creation Chain

All files created are either:

1. **New files** (metadata/\*.rs, startup additions) — no upstream files depend on them yet
2. **Modified files** — modifications happen after dependencies are satisfied

| File                        | Created By | Dependencies Before            | Assessment                   |
| --------------------------- | ---------- | ------------------------------ | ---------------------------- |
| metadata/models.rs          | 3.1        | 2.2 (module exists)            | ✅ module created in 2.2     |
| metadata/db.rs              | 3.2        | 3.1 (MetadataStoreError)       | ✅ exists after 3.1          |
| metadata/migrations.rs      | 3.3        | 3.1 (MetadataStoreError)       | ✅ exists after 3.1          |
| metadata/profile_sync.rs    | 3.4        | 3.1, 3.2, 3.3                  | ✅ all exist after each task |
| Tauri lib.rs (register)     | 4.1        | All metadata modules           | ✅ all ready after 3.4       |
| profile.rs (hooks)          | 4.2        | 1.1 (sanitize_display_path)    | ✅ exists after 1.1          |
| startup.rs (reconciliation) | 4.3        | 4.1 (MetadataStore registered) | ✅ registered in 4.1         |

**Result: NO MISSING DEPENDENCIES ✅**

### Check: Cross-Module References

- 3.4 uses `validate_name()` from profile module — already exists (imported from profile/legacy.rs) ✅
- 3.4 uses `ProfileStore` API — already exists ✅
- 4.1 needs `MetadataStore` struct and methods — all defined in 2.2 and implemented in 3.1–3.4 ✅
- 4.2 needs `SyncSource` enum — defined in 3.1 ✅
- Task 2.2 advice notes: "`profile_path()` may be private; hook code needs to reconstruct from `base_path.join(format!(\"{name}.toml\"))`" — this is a known concern, not a missing dependency (can be worked around) ⚠️ **See Recommendations section**

---

## 3. Orphaned Tasks Analysis

**Definition:** Tasks with no downstream consumers (excluding final tests/validation tasks which are acceptable endpoints).

| Task | Downstream Consumers               | Assessment                            |
| ---- | ---------------------------------- | ------------------------------------- |
| 1.1  | 4.2 (sanitize_display_path)        | ✅ Used in Tauri commands             |
| 2.1  | 2.2 (Cargo.toml dependency)        | ✅ Dependency for module skeleton     |
| 2.2  | 3.1 (module registration)          | ✅ Prerequisite for all Phase 3 tasks |
| 3.1  | 3.2, 3.3, 3.4 (MetadataStoreError) | ✅ Core types used throughout         |
| 3.2  | 3.4 (db::new_id), 4.1 (Connection) | ✅ Used by sync and Tauri init        |
| 3.3  | 4.1 (migrations run in init)       | ✅ Schema required for queries        |
| 3.4  | 4.1 (sync_profiles_from_store)     | ✅ Called during startup              |
| 4.1  | 4.2, 4.3 (MetadataStore available) | ✅ State registration for commands    |
| 4.2  | 5.1 (integration tests)            | ✅ Hooks tested end-to-end            |
| 4.3  | 5.1 (startup path tests)           | ✅ Reconciliation tested              |
| 5.1  | None                               | ✅ Final validation — acceptable      |

**Result: NO ORPHANED TASKS ✅**

---

## 4. Parallelization Opportunities

### Group A: Independent Prerequisites (can start immediately)

```
START
  ├─→ 1.1 (promote utility) — 30 min
  └─→ 2.1 (add dependencies) — 10 min
       [both complete independently]
```

**Win:** 40 min → 30 min (25% speedup)

### Group B: Parallel Core Module Tasks

```
3.1 (models) — 45 min
  ├─→ 3.2 (db.rs) — 60 min  ┐
  ├─→ 3.3 (migrations) — 30 min │ ALL RUN IN PARALLEL
  └─→ 3.4 (profile_sync) — must wait for all three ┘
```

**Win:** 3.2 and 3.3 run in parallel, reducing Phase 3 from (60+30+wait-for-both) to (max(60, 30) + wait)

- **Current sequential assumption:** 45 + 60 + 30 + 90 = 225 min
- **With 3.2||3.3:** 45 + max(60, 30) + 90 = 45 + 60 + 90 = 195 min ✅ **30-min savings**

### Group C: Parallel Tauri Integration Tasks

```
4.1 (register MetadataStore) — 20 min
  ├─→ 4.2 (sync hooks) — 60 min ┐
  └─→ 4.3 (startup) — 20 min    │ BOTH RUN AFTER 4.1
```

**Win:** 4.2 and 4.3 independent after 4.1, run in parallel

- **Current sequential:** 20 + 60 + 20 = 100 min
- **With 4.2||4.3:** 20 + max(60, 20) = 80 min ✅ **20-min savings**

### Group D: Final Testing

```
5.1 (unit + integration tests) — must wait for 4.2 AND 4.3
   [sequential after Group C completes]
```

---

## 5. Critical Path Analysis

### Longest Chain (Sequential Dependency Path)

```
2.1 (10 min)
  → 2.2 (15 min)
  → 3.1 (45 min)
  → 3.4 (90 min) [waits for 3.2||3.3 in parallel]
  → 4.1 (20 min)
  → 4.2 (60 min) [4.3 runs in parallel]
  → 5.1 (120 min)

TOTAL: 10+15+45+90+20+60+120 = 360 minutes (6 hours)
```

### Optimized Path (with parallelization)

```
Start: 1.1 (30 min) in parallel with 2.1 (10 min)
Phase 1: max(30, 10) = 30 min

2.2 (15 min) [blocked on 2.1]
3.1 (45 min)
3.2 (60 min) ┐
3.3 (30 min) ┤ max(60, 30) = 60 min
3.4 (90 min)
4.1 (20 min)

4.2 (60 min) ┐
4.3 (20 min) ┤ max(60, 20) = 60 min

5.1 (120 min)

TOTAL: 30 + 15 + 45 + 60 + 90 + 20 + 60 + 120 = 440 minutes
```

**Wait, that's longer.** Let me recalculate:

Critical path is the dependency chain. Parallelization savings apply to independent branches:

````
Path 1 (2.1→2.2→3.1→3.2/3.3→3.4→4.1→4.2→5.1):
  2.1(10) + 2.2(15) + 3.1(45) + max(3.2(60), 3.3(30)) + 3.4(90) + 4.1(20) + 4.2(60) + 5.1(120)
  = 10+15+45+60+90+20+60+120 = 420 min

Path 2 (1.1→4.2→5.1):
  1.1(30) + 4.2(60) + (already waiting for 4.2)
  = 30 + 60 = 90 min

Path 3 (4.1→4.3→5.1):
  After 4.1, 4.3 runs in parallel with 4.2

**Actual Critical Path:** 2.1→2.2→3.1→3.2/3.3→3.4→4.1→max(4.2,4.3)→5.1

With parallelization opportunities:
- 1.1 and 2.1 in parallel: saves 10 min (1.1 takes 30, so they run concurrently)
- 3.2 and 3.3 in parallel: saves 30 min (they run together instead of sequentially)
- 4.2 and 4.3 in parallel: saves 20 min (they run together instead of sequentially)

**Total optimized:** 420 - 10 - 30 - 20 = 360 min (same as original because dependencies chain them anyway)

Actually, the critical path doesn't change because every optimization involves tasks that are already serialized by the dependency chain. The parallelization helps reduce wall-clock time if multiple developers work on it, but the critical path length (longest chain of dependencies) remains:

2.1(10) + 2.2(15) + 3.1(45) + 3.4(90) + 4.1(20) + 4.2(60) + 5.1(120) = **360 min (6 hours)** for critical path
1.1(30) must complete before 4.2, so the real constraint is max(360, 30+60) = 360

---

## 6. Dependency Correctness Justifications

### Why Does 3.4 Require ALL of [3.1, 3.2, 3.3]?

- **3.1 (models):** Provides `MetadataStoreError`, `SyncSource`, `SyncReport` types used in function signatures
  ```rust
  pub fn observe_profile_write(
      conn: &Connection,
      name: &str,
      profile: &GameProfile,
      path: &Path,
      source: SyncSource,  // ← from 3.1
  ) -> Result<(), MetadataStoreError>  // ← from 3.1
````

**Necessity: CRITICAL** — Won't compile without these types

- **3.2 (db.rs):** Provides `db::new_id()` for UUID generation

  ```rust
  let profile_id = db::new_id();  // ← calls this
  ```

  **Necessity: CRITICAL** — Function needs UUIDs for new profiles

- **3.3 (migrations.rs):** Defines schema that queries operate on

  ```rust
  INSERT INTO profiles (...)  // ← table defined in 3.3
  ```

  **Necessity: SEMANTIC** — Queries reference tables that must exist; migrations run before this is called

**Conclusion:** ✅ All three are necessary; no can be dropped

### Why Does 4.1 Require [3.2, 3.3, 3.4] and NOT 3.1?

Because 3.1 is transitively required through 3.2, 3.3, 3.4 (they all depend on 3.1). Explicitly listing 3.1 would be redundant. However, the dependency spec is valid as-is.

---

## Issues Found

### ✅ Issue #1: Potential File Visibility Problem (Not a Dependency Error)

**Location:** Task 2.2 Advice section
**Concern:** `ProfileStore::profile_path()` may be private; Task 4.2 needs the path
**Impact:** Medium (Task 4.2 can work around with `base_path.join(format!("{name}.toml"))`, but less clean)
**Status:** ⚠️ **Requires verification before 4.2 starts** — Check actual visibility in code

### ✅ Issue #2: Startup Blocking Risk (Not a Dependency Error)

**Location:** Task 2.2 Advice section
**Concern:** "Startup reconciliation must not block the app" — `sync_profiles_from_store` could hang
**Impact:** Medium (Runtime issue, not scheduling issue)
**Status:** ⚠️ **Noted in plan; implementor should consider timeout or async spawn**

---

## Recommendations

### 1. **Verify `ProfileStore::profile_path()` Visibility** (Priority: HIGH)

Before Task 4.2 starts, confirm whether `profile_path()` is public or private.

- If **public:** No action needed
- If **private:** Task 4.2 must reconstruct path as `base_path.join(format!("{name}.toml"))`
- **Action:** path-validator teammate should check this at start of 4.2

### 2. **Parallelize Phase 3 Task Scheduling** (Priority: MEDIUM)

Tasks 3.2 and 3.3 should be assigned to different implementors once 3.1 is done.

- Both depend only on 3.1 (models)
- Both are independent of each other
- Implement in parallel to save ~30 minutes of wall-clock time

### 3. **Parallelize Phase 4 Task Scheduling** (Priority: MEDIUM)

Tasks 4.2 and 4.3 should start simultaneously after 4.1 is done.

- Both depend only on 4.1
- Both are independent of each other
- Implement in parallel to save ~20 minutes of wall-clock time

### 4. **Dependency Specification is Correct** (Priority: LOW)

No changes needed to the declared dependencies. All 11 tasks have correct, justified dependencies.

### 5. **Consider Early Unit Testing** (Priority: LOW)

Individual metadata modules (3.1–3.4) could include inline unit tests (e.g., `#[cfg(test)] mod tests` at the bottom of each file) to catch issues early. Final integration in 5.1 would then focus on Tauri command flow.

- This is optional; current plan (5.1 after 4.3) is valid
- Would provide faster feedback during Phase 3 implementation

---

## Summary Table

| Criterion                     | Status        | Finding                                         |
| ----------------------------- | ------------- | ----------------------------------------------- |
| Circular Dependencies         | ✅ PASS       | None found                                      |
| Missing Dependencies          | ✅ PASS       | All file creation chains satisfied              |
| Orphaned Tasks                | ✅ PASS       | All tasks have consumers or are final           |
| Dependency Correctness        | ✅ PASS       | All 11 dependencies justified and necessary     |
| Parallelization Opportunities | ✅ IDENTIFIED | 3 groups: (1.1\|2.1), (3.2\|3.3), (4.2\|4.3)    |
| Critical Path                 | ✅ CALCULATED | 360 minutes (6 hours) for serial execution      |
| File Visibility Risk          | ⚠️ FLAGGED    | `ProfileStore::profile_path()` visibility TBD   |
| Startup Blocking Risk         | ⚠️ FLAGGED    | Consider timeout/async spawn for reconciliation |

---

## Next Steps

1. **path-validator:** Check `ProfileStore::profile_path()` visibility in current codebase
2. **completeness-validator:** Review task descriptions for estimation accuracy
3. **Proceed with implementation:** Dependency graph is sound and ready for execution
