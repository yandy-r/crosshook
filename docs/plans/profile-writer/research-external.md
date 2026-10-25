# External Research — YAN-811 / GitHub #532 (M1-06): `ProfileWriter` in `crosshook-app`

Scope: libraries and patterns relevant to moving profile mutation, revision and rollback flows into
`crates/crosshook-app/src/services/profiles/`. Paths below are relative to `src/crosshook-native/`
unless they start with `docs/`. All repo facts were read from the worktree
`crosshook-yan-811`; all external facts come from pages fetched through `read` (URLs in
[Sources](#sources)). Retrieval date: 2026-10-25. Versions are the ones pinned in `Cargo.lock`
(tokio 1.51.0, tauri 2.11.2, serde_json 1.0.149, similar 2.7.0) and `package.json` (vitest ^4.1.12).

---

## Executive Summary

**Bottom line: the refactor needs zero new crates (Rust or npm).** Every pattern the issue asks for is
already established in this repo, and each has a direct precedent to copy.

| Question                            | Answer                                                                                                                                                                                                                                       | Precedent in repo                                                                                                                                                         |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Wrap sync stores in async API       | `tokio::task::spawn_blocking`; panics/cancellation → `AppError` via `From<JoinError>`                                                                                                                                                        | `crates/crosshook-app/src/services/mod.rs:31-37` (`run_blocking`), `error.rs:68-77`                                                                                       |
| Event bus lag                       | `tokio::sync::broadcast` bounded (256); lagged receiver gets `RecvError::Lagged(n)`, mapped to `AppEvent::Resync { missed }`                                                                                                                 | `crates/crosshook-app/src/events.rs`, tests in `events/tests.rs` (`lag_yields_resync_then_retained_tail`)                                                                 |
| `ProfileChangeReason` serialization | `#[serde(tag = "kind", rename_all = "snake_case")]` internally-tagged enum; `Renamed { from }` → `{"kind":"renamed","from":"Old"}`                                                                                                           | `JobState` (`bootstrap/report.rs:120-122`, `tag="state"`), `AcceptSuggestionRequest` (`crosshook-core/src/protondb/suggestions.rs:73-75`, `tag="kind"`), `MetadataStatus` |
| Tauri v2 `emit` of objects          | `Emitter::emit(event, payload)` takes any `Serialize + Clone`; JSON-serialized; JS `listen<T>` gets the parsed object as `event.payload`                                                                                                     | `src-tauri/src/startup_bridge.rs` emits `FinishedPayload { job, state }` the same way                                                                                     |
| Golden-file testing                 | **Plain fixture files + `fs::read` + `assert_eq!`** — `insta`/`expect-test`/`goldenfile` are **not** in `Cargo.lock` or any `Cargo.toml`; the repo already has one golden test                                                               | `crates/crosshook-core/tests/launch_request_golden.rs` + `tests/fixtures/launch_request/*.json`                                                                           |
| Vitest listener tests               | Two repo patterns: (a) `vi.mock('@/lib/events')` capturing handlers; (b) real `@/lib/events` adapter + `emitMockEvent` / `resetBrowserEventBus`. Call-counter = `mockCallCommand.mock.calls.filter(c => c[0] === 'batch_validate_profiles')` | `src/hooks/__tests__/useProfileSummaries.test.ts`, `useProfileHealth.startup.test.ts`                                                                                     |

Highest-value findings for the plan (details in later sections):

1. **Publish the event from inside the blocking closure (or accept that a dropped caller future skips it).**
   A `spawn_blocking` closure keeps running after the awaiting future is dropped
   (`abort` is a no-op once started). Anything done _after_ `.await` is not guaranteed to run.
   `EventBus::publish` is synchronous, so it can be called from the closure.
2. **`Resync` currently has no profile-change meaning.** `startup_bridge.rs:121-130` handles `Resync`
   only by re-emitting startup-job terminal events. A lagged bridge would silently drop
   `profiles-changed`. The plan must define what the bridge emits on `Resync` (recommendation: a
   `profiles-changed` with empty `names`, treated as "everything may have changed").
3. **All writes to one profile must be serialized.** `ProfileStore` docs say "the last completed write
   wins" (`store.rs:128-129,191`). With every command now on the blocking pool, concurrent
   load→mutate→save sequences for the same profile can interleave. Put a writer-level
   `std::sync::Mutex<()>` _inside_ the blocking closure.
4. **The real wire payload changes shape (string → object); the browser mocks already emit a
   _different_ object (`{name, action}`) and listeners are typed `subscribeEvent<string>`.** Five hooks
   plus ~15 mock call sites must move to one shared type + parser.
5. **Golden fixtures must be captured from pre-refactor code**, then committed as plain files. Since
   `ProfileWriter` still calls `ProfileStore::save` (`toml::to_string_pretty(&storage_profile)` →
   `fs_util::write_atomic`, `store.rs:95-111`), byte identity is structural; the goldens guard
   against a re-implementation drifting (e.g. someone serializing the `GameProfile` instead of
   `storage_profile()`).

---

## Primary APIs

No external service APIs: the refactor uses only in-workspace crates (tokio, serde, tauri v2) — see Libraries.

## Libraries

### Already in the workspace — use these

| Crate / package                                        | Version (lock)      | Where declared                                                                                              | Use in this refactor                                                                                                                                                                        |
| ------------------------------------------------------ | ------------------- | ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tokio`                                                | 1.51.0              | `crosshook-app` deps: `rt-multi-thread, sync, time, macros, process`; dev: `test-util, macros, rt, io-util` | `spawn_blocking`, `broadcast`, `#[tokio::test]`                                                                                                                                             |
| `serde` (+`derive`)                                    | 1                   | `crosshook-app` deps                                                                                        | `Serialize` on `ProfileChangeReason` and the Tauri payload struct                                                                                                                           |
| `serde_json`                                           | 1.0.149             | `crosshook-app` **dev-deps only**; `src-tauri` deps                                                         | wire-shape tests (`serde_json::to_value`) as in `startup_bridge/tests.rs` and `health/tests.rs::health_wire_shape_*`                                                                        |
| `toml`                                                 | 1.1.2 (`1.1.0` req) | `crosshook-app` **dev-deps**; core deps                                                                     | read fixtures / compare parsed form in diagnostics only; byte equality is asserted on raw bytes                                                                                             |
| `tempfile`                                             | 3                   | `crosshook-app` dev-deps                                                                                    | tempdir `ProfileStore` + `MetadataStore::try_new` DB (enabled)                                                                                                                              |
| `sha2` 0.11 via `crosshook_core::metadata::sha256_hex` | —                   | re-exported at `metadata/mod.rs:150`                                                                        | recompute expected revision hash in tests; do **not** add `sha2` to `crosshook-app`                                                                                                         |
| `similar`                                              | 2.7.0               | `crosshook-core` deps (`profile/config_diff.rs`)                                                            | unified diff + 2000-line / 512 KiB caps already implemented in core (`DIFF_MAX_LINES`, `MAX_DIFF_OUTPUT_BYTES`, `cap_diff_output_bytes`); the service only calls it inside `spawn_blocking` |
| `thiserror`, `tracing`                                 | 2 / 0.1             | `crosshook-app` deps                                                                                        | error mapping, "revisions skipped" warning                                                                                                                                                  |

### Frontend (already present)

| Package                  | Version                                       | Use                                                         |
| ------------------------ | --------------------------------------------- | ----------------------------------------------------------- |
| `vitest`                 | ^4.1.12 (`happy-dom` env; `vitest.config.ts`) | listener tests                                              |
| `@testing-library/react` | ^16.3.2                                       | `renderHook`, `act`, `waitFor`                              |
| `@tauri-apps/api`        | ^2.11.0                                       | `listen` (dynamically imported by `src/lib/events.ts:7-12`) |
| `@playwright/test`       | ^1.60.0                                       | `npm run test:smoke` (browser mocks)                        |

### Considered and **not recommended**

| Candidate                                                     | Why not                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `insta` (snapshot testing, https://insta.rs/docs/quickstart/) | Not in `Cargo.lock` (grep for `insta`, `expect-test`, `goldenfile`, `pretty_assertions`, `proptest` → no matches). Adds a dev-dep, `.snap` metadata headers and a `cargo-insta` review workflow; the acceptance criterion is _byte-identical TOML_, and insta stores text with a YAML front-matter header and normalizes trailing newlines for text snapshots — not suitable as a byte oracle without extra care [INFERENCE from insta docs; not tested]. The repo's existing golden test uses plain files. Also CI behaviour depends on `CI` env var. |
| `expect-test`, `goldenfile`                                   | Same: new dependency for something three lines of `fs::read` + `assert_eq!` do.                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `tokio-test`, `mockall`                                       | Not needed; the call-counter requirement is satisfied on the frontend (Vitest) and by asserting published events on the Rust side (see below).                                                                                                                                                                                                                                                                                                                                                                                                         |
| `rayon` / semaphore for blocking pool                         | Tokio docs recommend a semaphore for _many CPU-bound_ blocking tasks; profile writes are short and I/O bound (single TOML file + SQLite). A per-writer mutex (below) is the only limiter needed.                                                                                                                                                                                                                                                                                                                                                       |

---

## Integration Patterns

### 1. `spawn_blocking` in async service APIs

**Existing helper** — `crates/crosshook-app/src/services/mod.rs:31-37`:

```rust
async fn run_blocking<T, F>(work: F) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(work).await?
}
```

`?` converts `JoinError` through `impl From<tokio::task::JoinError> for AppError`
(`error.rs:68-77`): `is_cancelled()` → `AppErrorKind::Cancelled` ("background task was cancelled"),
otherwise → `AppErrorKind::Internal` ("background task failed"). The panic payload is deliberately
discarded ("may embed user paths"). `JoinError` exposes `is_cancelled`, `is_panic`, `into_panic`
(docs.rs `tokio::task::JoinError`). **`run_blocking` is private to `services` (`async fn`, no `pub`);
sub-modules like `services/profiles/` can call it as `super::run_blocking`** — same as `settings`
and `health` presumably do (not re-verified per file).

Semantics that matter for the writer (tokio `spawn_blocking` docs):

- **Not abortable once started.** "If you call `abort` on a `spawn_blocking` task, this will not have
  any effect… The exception is if the task has not started running yet." Consequences:
  - If the caller's future is dropped (e.g. Tauri command future dropped, or app shutdown),
    the load→mutate→save→observe→revision sequence **still completes**. Good for atomicity, bad for
    "post-await" side effects: **do the event publish inside the closure** (it is sync), otherwise a
    dropped caller yields a persisted write with no `profiles-changed`.
  - Runtime shutdown "will wait indefinitely for all started `spawn_blocking`" unless
    `shutdown_timeout` is used. The crate README already documents this limit
    (`crates/crosshook-app/README.md:22-24`: "non-abortable blocking work inside spawned tasks is
    unsupported (tasks are dropped at shutdown)").
- **Pool size is large by default**; short I/O tasks are the intended use. Long-lived loops should use
  a dedicated thread — not relevant here.
- **Panic isolation:** a panic inside the closure is caught by the runtime and surfaces as
  `JoinError::is_panic()`. The workspace has no `panic = "abort"` profile (checked
  `Cargo.toml`, `src-tauri/Cargo.toml`: no `[profile]` panic key), so this works. A panic while
  holding `MetadataStore`'s inner `Mutex<Connection>` poisons it; subsequent calls return
  `MetadataStoreError::Corrupt("metadata store mutex poisoned while …")`
  (`metadata/store.rs:187-189`). So: **a panic in a revision-capture path degrades all later metadata
  operations until restart** — the writer must not `unwrap()`/`expect()` inside closures that hold or
  call the store.
- **Ordering/atomicity of the pipeline** must live in _one_ closure: validate → load → mutate →
  save → `observe_profile_write` → `capture_config_revision` → health invalidate → publish. Splitting
  it across multiple `run_blocking` calls (each `await`) opens races and drop-points.

**Where existing `spawn_blocking` call sites differ** (do not copy for the new service):

- `src-tauri/src/commands/*` use `tauri::async_runtime::spawn_blocking` and stringly errors
  (`.map_err(|e| e.to_string())`) — this is the code being moved.
- `crosshook-core/src/protonup/install/archive.rs:337-350` maps join errors into a domain error
  string including the panic `Display` — that leaks detail; the app-layer convention (sanitized
  `AppError`) is the right one.
- `src-tauri/src/commands/launch/execution.rs:567-575` treats a failed join as "ignore, continue" —
  acceptable for fire-and-forget launch bookkeeping, **not** for profile writes (a lost write must
  surface).

**Recommended additions** (all inside existing conventions):

- Log panics _server-side_ before sanitizing. `From<JoinError>` cannot log (no context). Either
  add a `run_blocking`-adjacent helper `run_blocking_named(op: &'static str, …)` that logs
  `tracing::error!(op, is_panic = e.is_panic(), "…")` on `Err(JoinError)` before converting, or
  accept the status quo (`bootstrap/scans.rs:10-13` logs raw join errors at the call site).
  _Open question 1._
- **Per-writer lock.** Since every public fn becomes async and Tauri commands run concurrently
  (Tauri async commands run on its runtime), two quick `Save`+`Favorite` calls on the same profile
  can interleave their load/save halves. `ProfileStore` explicitly does not serialize
  (`store.rs:128-129`, `:191`: "writes to the same profile are not synchronized; the last completed
  write wins"). Hold a `std::sync::Mutex<()>` (global, or keyed per name via
  `Mutex<HashMap<String, Arc<Mutex<()>>>>`) _inside_ the closure. Use
  `lock().unwrap_or_else(std::sync::PoisonError::into_inner)` — the guarded value is `()`, so
  poisoning carries no invariant. A `tokio::sync::Mutex` held across the `.await` also works but
  would be held while the caller waits for the pool; the in-closure std mutex only blocks a pool thread.
- **Do not capture an `App` clone inside the closure** (README warning, `README.md:22-24`); capture
  `ProfileStore` (cheap `Clone`), `MetadataStore` (`Arc<Mutex<Connection>>` inside), `SettingsStore`
  and a cloned `EventBus` (`#[derive(Clone)]`, `events.rs`).

### 2. Event bus semantics (lag)

Facts (tokio `sync::broadcast` docs + repo implementation):

- Bounded ring of `capacity` messages; **a send when full drops the oldest**; the slow receiver's
  next `recv` returns `Err(RecvError::Lagged(n))` and its cursor jumps to the _oldest retained_
  value; the next `recv` returns that value. New receivers see only events sent _after_ `subscribe`.
- Values are stored once and **cloned per receiver on demand** → `AppEvent: Clone` is required (it
  is; `events.rs` derives `Debug, Clone, PartialEq, Eq` and `#[non_exhaustive]`). Keep
  `ProfilesChanged.names: Vec<String>` small; bulk operations should emit **one** event with many
  names, not N events.
- `Sender::send` errors only when there are zero receivers; `EventBus::publish` maps that to `0`
  (not an error) — so publishing is always safe and never blocks (publishers "never block").
- Repo mapping (`events.rs`): `EventReceiver::recv()` converts `Lagged(missed)` to
  `Some(AppEvent::Resync { missed })` and `Closed` to `None`. Default capacity
  `DEFAULT_EVENT_CAPACITY = 256`, max `1 << 20`.
- Existing consumer: `src-tauri/src/startup_bridge.rs:119-135` — `while let Some(event) =
rx.recv().await { match event { StartupJobFinished{job} => …, Resync{..} => re-emit all bridged
jobs, _ => {} } }`. The receiver is created **before** `tauri::Builder` in `src-tauri/src/lib.rs:32`
  (`core.subscribe()`) and moved into the `.setup()` closure (`lib.rs:46-48`), so events published
  during bootstrap are not lost to a late subscribe.

Implications for `ProfilesChanged`:

- **Add a `ProfilesChanged` arm to the same bridge loop (or a second receiver/bridge).** One bridge
  with one receiver is simplest and keeps `Resync` handling in one place; two receivers each see the
  same `Resync` independently, which is also fine. _Open question 2._
- **Define `Resync` → profiles behaviour.** After a lag the bridge cannot know which profiles
  changed. Recommended: on `Resync`, additionally `emit("profiles-changed", { reason: {kind:"saved"}… })`
  is wrong semantically; better to either (a) add a `ProfileChangeReason::Resync`-style variant (not
  in the issue's nine reasons — needs sign-off), or (b) emit the existing health-affecting reason with
  `names: []` and document "empty `names` = unknown/all". Today a lagged startup bridge re-emits
  startup jobs only; without this, profile UIs could stay stale after a burst (e.g. a Lutris import
  of >256 profiles emitting per-profile events; one coalesced event avoids this).
- **Ordering vs. IPC response.** Previously `emit_profiles_changed(&app, …)` ran synchronously
  inside the command, _before_ the `invoke` result was returned
  (`commands/profile/lifecycle.rs:224`, `favorites.rs:16`, …). With publish → broadcast → bridge task →
  `app.emit`, the event is emitted asynchronously, so it may arrive _after_ the `invoke` promise
  resolves. Frontend hooks that call `save()` then rely on the event to refetch are fine (they
  refetch on arrival); hooks that assume the event already fired when the promise resolves are not.
  I did not audit all call sites (`useProfile.ts:233` and the mock handlers only). _Open question 3._
- Tauri's own caveat: listeners are called in registration order but async listeners processing
  rapid events may complete out of order (Tauri docs, "Event ordering and async listeners"). For
  `profiles-changed` listeners that just `void refetch()` this is harmless.

### 3. Serde tagged enum for `ProfileChangeReason`

Serde representations (serde.rs/enum-representations): internally tagged
(`#[serde(tag = "kind")]`) puts the tag _inside_ the object next to variant fields; works for
struct, newtype-of-struct/map, and **unit** variants; a tuple variant is a **compile-time error**.
So `Renamed { from: String }` must be a struct variant (not `Renamed(String)`), which matches the
issue (`Renamed { from }`).

Repo conventions for tagged enums crossing IPC:

| Type                                                                           | Attributes                                                    | Wire                                             |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------- | ------------------------------------------------ |
| `JobState<T>` (`crosshook-app/src/bootstrap/report.rs:120-122`)                | `Serialize`, `#[serde(tag="state", rename_all="snake_case")]` | `{"state":"complete","data":…}`                  |
| `MetadataStatus` (`crosshook-core/src/metadata/models.rs:47-49`)               | `tag="state", rename_all="snake_case"`                        | `{"state":"newer_schema","found":…}`             |
| `AcceptSuggestionRequest` (`crosshook-core/src/protondb/suggestions.rs:73-75`) | `tag="kind", rename_all="snake_case"`                         | + per-variant `rename_all="camelCase"` on fields |
| `StartupJobKind` (`events.rs`)                                                 | `rename_all="snake_case"` on a unit enum                      | `"health_batch"`                                 |
| `startup-job-finished` payload                                                 | `FinishedPayload { job, state }` (snake_case fields)          | `{"job":"health_batch","state":"complete"}`      |

**Recommended shape** (matches `tag="kind"` precedent from `AcceptSuggestionRequest`, snake*case
variant \_and* field names, matching the other bridge events and the health/startup TS types which use
`snake_case`; `ProfileSummary` is the camelCase exception — don't copy it):

```jsonc
// favorite
{ "reason": { "kind": "favorite_toggled" }, "names": ["Elden Ring"] }
// rename
{ "reason": { "kind": "renamed", "from": "Old Name" }, "names": ["New Name"] }
```

Serde gotchas relevant here:

- `rename_all` on the enum renames _variants_; variant struct _fields_ need a separate
  `#[serde(rename_all = "snake_case")]` on the variant (already snake in Rust, so a no-op for `from`).
- Internally tagged enums are `Serialize`-only for our needs. Deserialization (needed only if
  Rust tests round-trip) buffers content; fine for a test but unnecessary — assert with
  `serde_json::to_value` + `json!` equality as `startup_bridge/tests.rs` does.
- `AppEvent` itself must **not** gain a serde derive just for this: it's `#[non_exhaustive]`,
  `PartialEq, Eq`, and used in-process. Serialize `ProfileChangeReason` (derive in
  `crosshook-app`, `serde` dep already has `derive`) and build a small private
  `ProfilesChangedPayload { reason: ProfileChangeReason, names: Vec<String> }` (or `&`-borrowing
  variant — but Tauri `emit` needs `Clone`) **in `src-tauri`**, like `FinishedPayload`.
- Tag-name collisions: a struct variant field called `kind` would collide with `tag = "kind"` — avoid.
- Keep `Eq` derivable: `ProfileChangeReason` contains only `String`s.
- Unit variants under `tag="kind"` serialize as `{"kind":"saved"}` — an _object_, never a bare string.
  That is deliberate (uniform type for TS discriminated unions) and the reason the old string payload
  `"saved"` is not wire-compatible.

### 4. Tauri v2 `emit` serialization

From https://v2.tauri.app/develop/calling-frontend/ and docs.rs `tauri::Emitter` (2.11.2):

- `Emitter::emit<S: Serialize + Clone>(&self, event: &str, payload: S) -> tauri::Result<()>` emits
  to **all targets** (global event). `emit_str(event, String)` takes an already-serialized JSON
  string ("payload is json serialized") — the way to serialize _once_ and emit to multiple targets.
  (The `Serialize + Clone` bound is from the Tauri guide text "any serializable type that also
  implements Clone"; docs.rs's rustdoc-JSON rendering omitted the bounds.)
- "Event payloads are always JSON strings" on the wire; the JS side's `listen<T>(name, cb)` gives
  `cb({ event, id, payload })` with `payload` already `JSON.parse`d to `T`. A Rust `String` payload
  arrives as a JS string (today's `"saved"`); a struct arrives as an object.
- Events have no capability/permission gating and "no strong type support" (Tauri docs), so the
  shape contract is purely by convention: keep it in a shared TS type
  (`src/types/…`) and the Rust payload struct; test both ends.
- `AppHandle` implements `Emitter` (as used at `shared.rs:69`). In the bridge it's the cloned
  `AppHandle` passed to `spawn`, same as `startup_bridge.rs`.
- On emit failure the existing code logs `tracing::warn!` and continues
  (`shared.rs:68-72`, `startup_bridge.rs:89-91`); keep that — an emit error must not fail a
  completed write.
- JS pitfall (Tauri docs): `listen()` returns a Promise of `unlisten`; calling unlisten before it
  resolves does nothing. The repo's hooks already follow `unlisten.then(fn => fn())`; do not "simplify".

### 5. Golden-file testing (no new crate)

**Repo precedent — `crates/crosshook-core/tests/launch_request_golden.rs`:**

- Fixtures at `crates/crosshook-core/tests/fixtures/launch_request/*.json` (17 files), located via
  `Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/launch_request")` + `fs::read_dir`,
  sorted, with `assert!(!paths.is_empty())` so an empty/missing dir fails loudly.
- Shared with a TS test (`src/utils/__tests__/launchRequestGolden.test.ts`) — fixture is the contract.
- No regenerate-on-env mechanism was visible in the portion I read (lines 38-100 of the file; the
  middle `check_case` body was elided in my view) — _unverified_.

**Plan for profile goldens** (crosshook-app integration or unit tests; both dev-deps exist):

- Location: `crates/crosshook-app/tests/fixtures/profile_writer/` (+ test file
  `crates/crosshook-app/tests/profile_writer_golden.rs`), or `services/profiles/tests.rs` using
  `include_bytes!`. Integration test dir is already used (`tests/bootstrap_report.rs`).
- Fixture capture: generate the expected `.toml` bytes **on the pre-refactor commit** by running the
  existing `ProfileStore::save`/`rename`/`duplicate`/rollback paths from a throwaway test, copy
  results into the fixtures dir, and commit. Never regenerate from the new `ProfileWriter`.
- Assertions: `assert_eq!(std::fs::read(profile_path).unwrap(), include_bytes!("fixtures/…toml"))`
  (bytes, not `String`, so line endings and trailing newline are covered). On mismatch, byte
  vectors are unreadable in failure output → convert with `String::from_utf8_lossy` in a custom
  message, or compare `&str` after asserting valid UTF-8.
- What must be covered (acceptance criteria): save, rename (file content after
  `ProfileStore::rename` equals original content — already asserted in core `load_save.rs:406-409`;
  golden adds the writer path), duplicate, rollback (restored TOML == stored `snapshot_toml`, then
  re-saved bytes).
- Deterministic inputs only: profiles with fixed fields; `GameProfile::storage_profile()` strips
  runtime-only data; `toml::to_string_pretty` ordering is struct-field order (deterministic).
  **Avoid** fixtures that include `HashMap`-typed fields with >1 entry unless confirmed sorted
  (BTreeMap) — _unverified; check `GameProfile` field types when writing fixtures._
- SHA-256 dedupe: assert against `crosshook_core::metadata::sha256_hex(fixture_bytes)` and
  `MetadataStore::list_config_revisions` row count after two identical commits = 1
  (`insert_config_revision` dedupes against `ORDER BY id DESC LIMIT 1`,
  `config_history_store/mod.rs:46-65`).
- Diff caps: `compute_unified_diff` + `cap_diff_output_bytes` already have unit tests
  (`config_diff.rs:117-135`); the service test only needs to assert the service applies them
  (large input → `truncated == true`, `diff_text.len() <= MAX_DIFF_OUTPUT_BYTES`).
- Metadata disabled: `MetadataStore::disabled()` makes `with_conn` return `T::default()`
  (`store.rs:181-183`), so `capture_config_revision` is a silent no-op — tests for the "warning logged
  and surfaced" criterion must assert whatever signal the service returns, not infer it from errors.
  (Mirrors `health/tests.rs::batch_and_cached_snapshots_work_without_metadata`.)

If a human-reviewable text snapshot of _non-byte_ structures is later wanted (semantic diff output,
event JSON), `similar` is already present; `insta` would be the standard choice, but it is an extra
dependency decision for the user, not this issue.

### 6. Vitest for Tauri event listeners

**Adapter** `src/lib/events.ts`: `subscribeEvent<T>(name, handler)` → real `listen` when `isTauri()`,
otherwise an in-memory `browserBus`; `emitMockEvent(name, payload)` delivers `handler({event, id:0,
payload})` to browser listeners and returns whether any received it; `resetBrowserEventBus()` clears
(`events.ts:7-37`). Under Vitest/happy-dom `isTauri()` is false (existing tests rely on this).

Two patterns already in the repo:

1. **Capture handlers via module mock** — `src/hooks/__tests__/useProfileSummaries.test.ts`:
   `vi.mock('@/lib/events', () => ({ subscribeEvent: vi.fn((name, handler) => { …push to
capturedHandlers…; return Promise.resolve(mockUnlisten); }) }))`; helper
   `emitProfilesChanged()` iterates handlers calling `handler({event:'profiles-changed', id:0,
payload:''})` (lines 12-17, 51-56). Needs `await act(async () => { await Promise.resolve(); })`
   so the `subscribeEvent` promise resolves before emitting (lines 98-101). Also asserts `unlisten`
   is called on unmount.
2. **Real adapter + `emitMockEvent`** — `useProfileHealth.startup.test.ts` (imports
   `emitMockEvent, resetBrowserEventBus`; `beforeEach` resets bus and mocks) and
   `components/pages/__tests__/LibraryPage.test.tsx:449-480` (`emitMockEvent('profiles-changed', '')`
   then `waitFor`).

**Call counter pattern** — `useProfileHealth.startup.test.ts`:
`const mockCallCommand = vi.fn(); vi.mock('@/lib/ipc', () => ({ callCommand: (...a) =>
mockCallCommand(...a) }))`; `batchCalls = () => mockCallCommand.mock.calls.filter(c => c[0] ===
'batch_validate_profiles')`; `settleReport()` drains timers with
`vi.useFakeTimers(); await act(() => vi.runAllTimersAsync()); vi.useRealTimers()`.

Recommended acceptance test (illustrative, untested) for "favorite toggle triggers no
`batch_validate_profiles`":

```ts
// src/hooks/__tests__/useProfileHealth.profilesChanged.test.ts
it('skips batch_validate_profiles for favorite_toggled but runs it for saved', async () => {
  mockCommands(reportWith({ state: 'complete', data: summary })); // zero batch calls at mount
  const { result } = renderHook(() => useProfileHealth());
  await waitFor(() => expect(result.current.summary).toEqual(summary));
  await settleReport();
  mockCallCommand.mockClear();

  await act(async () => {
    emitMockEvent('profiles-changed', { reason: { kind: 'favorite_toggled' }, names: ['Alpha'] });
  });
  await settleReport();
  expect(batchCalls()).toHaveLength(0); // negative: needs a drain, not just waitFor

  await act(async () => {
    emitMockEvent('profiles-changed', { reason: { kind: 'saved' }, names: ['Alpha'] });
  });
  await waitFor(() => expect(batchCalls()).toHaveLength(1)); // positive control
});
```

Notes:

- A _negative_ assertion needs a drain (`settleReport()` pattern or `vi.runAllTimersAsync`), else it
  passes vacuously before the handler could have run. The positive control (same test, `saved` →
  1 call) proves the harness actually delivers events.
- `useProfileHealth.ts:241-244` is the only `profiles-changed` listener that calls `batchValidate()`
  (verified); the other four (`useProfile.ts:233`, `useLibrarySummaries.ts:64`,
  `useProfileSummaries.ts:39`, `useCommunityProfiles.ts:433`) refetch lists. A favorite toggle must
  _still_ refresh summary/library hooks (favorite state is shown in the library) — the skip applies to
  health only. Whether `useProfile` and `useCommunityProfiles` should react to `FavoriteToggled` is
  not determinable from the listener lines alone — _Open question 4_.
- Existing tests emit `''` (`useProfileSummaries.test.ts:54`, `LibraryPage.test.tsx:475`) and the
  `EventHandler` type there is `payload: string` — those must be updated to the object shape, or the
  shared parser must treat a non-object payload as "unknown → refresh everything" (safe default).
  Recommend one `parseProfilesChanged(payload: unknown): { reason: ProfileChangeReason | null;
names: string[] }` plus an exported predicate, e.g. `reasonAffectsHealth(reason)`, in a shared
  module, with its own unit test.
- Mock handlers in `src/lib/mocks/handlers/profile-mutations.ts` (lines 29, 61, 91, 111, 131, 151,
  173, 206, 251, 267, 304, 321, 345), `profile-history.ts:104`, `profile-presets.ts:72,107` currently
  emit `{ name, action }` with ad-hoc `action` strings (`'save-gamescope-config'`, `'mock-add'`,
  `'favorite'`, …). Per CLAUDE.md, mocks are the only way browser-dev/Playwright exercise listeners;
  update them to the real shape so `npm run test:smoke` exercises the new parser (the
  `verify:no-mocks` sentinel only requires mocks stay out of production bundles).
- Event arrives at TS as `{event, id, payload}`; hooks only need `payload`.

---

## Constraints

Hard (from repo rules and code):

1. **Business logic in `crosshook-core`/`crosshook-app`; `src-tauri` thin.** The Serialize payload
   struct + `app.emit` live in `src-tauri` (bridge); the reason enum and pipeline live in
   `crosshook-app`.
2. **Every public service fn `async`; blocking work in `spawn_blocking`; panics/cancel → sanitized
   `AppError`** (`services/mod.rs:1-5`). Never surface `JoinError::Display` or panic payloads.
3. **TOML byte-compat; no schema change; `schema_version` stays 21** (per AGENTS rules). Do not
   introduce a second TOML serializer — always `ProfileStore::save` → `toml::to_string_pretty(&storage_profile)`.
4. **`AppEvent` is `#[non_exhaustive]`**; adding `ProfilesChanged { reason, names }` is
   source-compatible for matchers that have `_ => {}` (the bridge has one, `startup_bridge.rs:132`),
   but `AppEvent`'s derives (`Clone, PartialEq, Eq, Debug`) force `ProfileChangeReason` to be the same.
5. **`EventBus` capacity 256, drop-oldest**; coalesce bulk operations into one event.
6. **`MetadataStore::disabled()` semantics**: reads return defaults; revision capture silently
   skipped. The issue requires the skip to be "logged and surfaced" — the existing
   `capture_config_revision` "silently skips on any error" (`shared.rs` doc). The service must add a
   visible signal (return value/warning list), tested.
7. **Event payload shape change is breaking for 5 listeners + ~16 mock emit sites + 2 existing tests**
   (`useProfileSummaries.test.ts`, `LibraryPage.test.tsx`). `Cargo`/npm side: nothing to install.
8. **Collections / `apply_collection_defaults` out of scope (M1-35).** `lifecycle.rs` imports
   `apply_collection_defaults` from `shared` (line 16) — the service must call into whatever remains
   in `shared` for now without moving it. _(Interface decision for the design agent.)_
9. **`insta`-style review tooling cannot be assumed** (`cargo-insta` not a project dependency).
10. **Frontend**: new `overflow-y: auto` containers → `useScrollEnhance` SCROLLABLE selector (n/a
    unless UI changes); no raw `invoke()`/`listen()` — use `callCommand`/`subscribeEvent`.
11. **Tokio features**: `crosshook-app` already has `rt-multi-thread` (needed for `spawn_blocking`)
    and dev `rt`+`test-util`. `#[tokio::test]` default is current-thread; `spawn_blocking` works on it
    (tokio docs: extra threads are spawned for blocking work even on current-thread runtimes).
    `events/tests.rs` and `health/tests.rs` use plain `#[tokio::test]`.

Soft / design risks:

- Async emit ordering vs. IPC response (see Pattern 2).
- Per-profile write serialization (Pattern 1).
- Lag → stale UI (Pattern 2).

---

## Code Examples

> Illustrative. Not compiled or run. Names mirror the issue; adjust to the final design.

### A. Reason enum and event variant (`crosshook-app`)

```rust
// crates/crosshook-app/src/services/profiles/event.rs  (illustrative)
use serde::Serialize;

/// Why `profiles-changed` fired. Wire shape: `{"kind":"renamed","from":"Old"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfileChangeReason {
    Saved,
    Deleted,
    Renamed { from: String },
    Duplicated,
    FavoriteToggled,
    RolledBack,
    PresetApplied,
    Imported,
    Migrated,
}

// events.rs
pub enum AppEvent {
    Resync { missed: u64 },
    StartupJobFinished { job: StartupJobKind },
    /// Empty `names` means "unknown or all profiles".
    ProfilesChanged { reason: ProfileChangeReason, names: Vec<String> },
}
```

### B. One blocking pipeline with the event published inside the closure

```rust
// crates/crosshook-app/src/services/profiles/writer.rs  (illustrative)
use std::sync::{Arc, Mutex, PoisonError};

pub struct ProfileWriter {
    profiles: ProfileStore,
    metadata: MetadataStore,
    settings: SettingsStore,
    events: EventBus,
    write_lock: Arc<Mutex<()>>, // serializes load→mutate→save per writer
}

impl ProfileWriter {
    pub async fn commit<F>(
        &self,
        name: String,
        mutation: F,
        reason: ProfileChangeReason,
    ) -> Result<CommitOutcome, AppError>
    where
        F: FnOnce(&mut GameProfile) -> Result<(), AppError> + Send + 'static,
    {
        let (profiles, metadata, settings, events, lock) = (
            self.profiles.clone(),
            self.metadata.clone(),
            self.settings.clone(),
            self.events.clone(),
            Arc::clone(&self.write_lock),
        );
        super::super::run_blocking(move || {
            let _guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
            let mut profile = profiles.load(&name)?;      // ProfileStoreError -> AppError
            mutation(&mut profile)?;
            profiles.save(&name, &profile)?;               // byte-compat path
            // observe_profile_write + capture_config_revision + health invalidate …
            // Publish here: the closure runs to completion even if the caller is dropped.
            events.publish(AppEvent::ProfilesChanged { reason, names: vec![name] });
            Ok(CommitOutcome { /* revision id, warnings */ })
        })
        .await
    }
}
```

### C. Bridge arm (`src-tauri`, thin)

```rust
// src-tauri/src/startup_bridge.rs (or a sibling module) — illustrative
#[derive(Clone, serde::Serialize)]
struct ProfilesChangedPayload {
    reason: ProfileChangeReason,
    names: Vec<String>,
}

// inside the `while let Some(event) = rx.recv().await` match:
AppEvent::ProfilesChanged { reason, names } => {
    if let Err(error) = handle.emit("profiles-changed", ProfilesChangedPayload { reason, names }) {
        tracing::warn!(%error, "failed to emit profiles-changed event");
    }
}
AppEvent::Resync { .. } => {
    // existing startup-job re-emit … plus (decision pending, see Open Questions):
    // emit a conservative profiles-changed with empty `names`.
}
```

### D. Rust wire-shape and ordering tests (style of `startup_bridge/tests.rs`)

```rust
#[test]
fn reason_wire_shapes() {
    use serde_json::json;
    assert_eq!(
        serde_json::to_value(ProfileChangeReason::Renamed { from: "Old".into() }).unwrap(),
        json!({ "kind": "renamed", "from": "Old" })
    );
    assert_eq!(
        serde_json::to_value(ProfileChangeReason::FavoriteToggled).unwrap(),
        json!({ "kind": "favorite_toggled" })
    );
}

#[tokio::test]
async fn favorite_toggle_publishes_one_named_event() {
    let (writer, bus, _tmp) = test_writer_with_profile("Alpha"); // metadata enabled and disabled variants
    let mut rx = bus.subscribe();
    writer.set_favorite("Alpha".into(), true).await.unwrap();
    assert_eq!(
        rx.recv().await,
        Some(AppEvent::ProfilesChanged {
            reason: ProfileChangeReason::FavoriteToggled,
            names: vec!["Alpha".into()],
        })
    );
}
```

The Rust-side analogue of the "no `batch_validate_profiles` on favorite" counter is: assert the
_only_ event published for a favorite toggle is `FavoriteToggled` (exactly one `recv`, then
`try`-style no second event) — `batch_validate_profiles` is a frontend-triggered command, so the
authoritative counter is the Vitest `batchCalls()` test above.

### E. Golden byte comparison

```rust
#[tokio::test]
async fn save_matches_pre_refactor_golden() {
    let dir = tempfile::tempdir().unwrap();
    let writer = test_writer(dir.path(), MetadataStore::disabled());
    writer.save("golden".into(), fixture_profile()).await.unwrap();
    let written = std::fs::read(dir.path().join("profiles/golden.toml")).unwrap();
    let golden = include_bytes!("../tests/fixtures/profile_writer/save.toml");
    assert_eq!(
        String::from_utf8_lossy(&written),
        String::from_utf8_lossy(golden),
        "profile TOML drifted from the pre-refactor golden"
    );
    assert_eq!(written.as_slice(), golden.as_slice()); // exact bytes
}
```

### F. Shared TS type + parser (illustrative)

```ts
// src/types/profileEvents.ts
export type ProfileChangeReason =
  | { kind: 'saved' }
  | { kind: 'deleted' }
  | { kind: 'renamed'; from: string }
  | { kind: 'duplicated' }
  | { kind: 'favorite_toggled' }
  | { kind: 'rolled_back' }
  | { kind: 'preset_applied' }
  | { kind: 'imported' }
  | { kind: 'migrated' };
export interface ProfilesChangedPayload {
  reason: ProfileChangeReason;
  names: string[];
}

// Unknown/legacy payloads (e.g. '') => null => callers must refresh everything.
export function parseProfilesChanged(payload: unknown): ProfilesChangedPayload | null {
  /* runtime guard */
}
export const reasonAffectsHealth = (p: ProfilesChangedPayload | null): boolean =>
  p === null || p.reason.kind !== 'favorite_toggled';
```

---

## Open Questions

1. **Panic logging**: log `JoinError` (`is_panic`) server-side before sanitizing? Today
   `From<JoinError>` swallows it silently; `bootstrap/scans.rs` logs at call sites. A shared
   `run_blocking` change affects settings/health/etc. Recommend a small additive log in the profile
   service only; needs plan decision.
2. **Bridge topology**: extend `startup_bridge.rs` (rename to a generic `event_bridge`) vs. a new
   `profile_bridge.rs` with its own `core.subscribe()` before `Builder` (as `lib.rs:32` does).
   Either works; two receivers double `Resync` handling.
3. **Emit-after-response ordering**: audit every `await callCommand('profile_*')` call site for code
   that assumes `profiles-changed` already fired when the promise resolves (only `useProfile.ts:233`
   and the mock handlers were inspected here).
4. **Which reasons skip which listeners**: issue says health re-validation is skipped "for reasons
   that cannot affect health, such as `FavoriteToggled`". Unresolved: `PresetApplied`, `Renamed`,
   `Duplicated` (health reports are keyed by name; optimization preset changes alter launch env, not
   path validity) — and whether `useProfile` / `useCommunityProfiles` need `names`-based filtering.
5. **`Resync` handling for profiles** (adding a non-listed variant vs. empty-`names` convention).
6. **`MetadataStore::disabled()` "surfaced warning"**: how is it surfaced — return field on the
   commit outcome, a `warnings: Vec<String>` on the IPC result, or an `AppEvent`? Persisted
   behavior is runtime-only per the issue's storage boundary.
7. **Fixture capture mechanics**: confirm `GameProfile`'s serialized form has no unordered
   `HashMap` with multiple entries before committing byte goldens; confirm `launch_request_golden.rs`
   has no `UPDATE_GOLDEN` convention to mirror (middle of the file was not read).
8. **Lock granularity**: single writer-wide mutex vs. per-profile-name map; rename/duplicate touch
   two names, favoring a single lock.
9. **Mocks vs. real payload for `names` on rename**: real = `names:[new]`, `reason.from = old`;
   mocks currently emit only the new name. Confirm consumers never need both in `names`.

---

## Sources

| Source                                    | URL                                                                                                                                                                       | Notes                                                                                                      |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| tokio `spawn_blocking`                    | https://docs.rs/tokio/1.51.0/tokio/task/fn.spawn_blocking.html                                                                                                            | docs for tokio 1.51.0 (matches `Cargo.lock`); retrieved 2026-10-25; abort / shutdown / pool-size semantics |
| tokio `JoinError`                         | https://docs.rs/tokio/1.51.0/tokio/task/struct.JoinError.html                                                                                                             | `is_cancelled`, `is_panic`, `into_panic`                                                                   |
| tokio `sync::broadcast`                   | https://docs.rs/tokio/1.51.0/tokio/sync/broadcast/index.html                                                                                                              | lagging / closing semantics, subscribe-from-now                                                            |
| tokio `broadcast::Sender`                 | https://docs.rs/tokio/1.51.0/tokio/sync/broadcast/struct.Sender.html                                                                                                      | `send`, `subscribe`, `receiver_count`                                                                      |
| tokio `RecvError`                         | https://docs.rs/tokio/1.51.0/tokio/sync/broadcast/error/enum.RecvError.html                                                                                               | `Closed`, `Lagged`                                                                                         |
| tokio runtime `Builder`                   | https://docs.rs/tokio/1.51.0/tokio/runtime/struct.Builder.html#method.max_blocking_threads                                                                                | blocking-thread cap method exists (details not rendered in the fetched page)                               |
| serde enum representations                | https://serde.rs/enum-representations.html                                                                                                                                | internally tagged: no tuple variants                                                                       |
| Tauri v2 — Calling the Frontend from Rust | https://v2.tauri.app/develop/calling-frontend/                                                                                                                            | event payload rules, `listen`/`unlisten` pitfalls, ordering caveat (first 300 of 578 lines read)           |
| docs.rs `tauri::Emitter` 2.11.2           | https://docs.rs/tauri/2.11.2/tauri/trait.Emitter.html                                                                                                                     | `emit`, `emit_str`, `emit_to`, `emit_filter`                                                               |
| serde_json `to_value`                     | https://docs.rs/serde_json/1.0.149/serde_json/fn.to_value.html                                                                                                            | wire-shape tests                                                                                           |
| insta (docs.rs)                           | https://docs.rs/insta/latest/insta/                                                                                                                                       | not recommended (not a dependency); `INSTA_UPDATE` modes, CI behaviour                                     |
| insta quickstart                          | https://insta.rs/docs/quickstart/                                                                                                                                         | `CI` env var behaviour; `cargo-insta` optional                                                             |
| Vitest `vi` API                           | https://vitest.dev/api/vi.html                                                                                                                                            | `vi.mock` hoisting, `vi.hoisted`, `importOriginal` (first 300 of 1776 lines read)                          |
| Repo: event bus                           | `src/crosshook-native/crates/crosshook-app/src/events.rs`, `events/tests.rs`                                                                                              |                                                                                                            |
| Repo: services helper                     | `crates/crosshook-app/src/services/mod.rs`, `error.rs`                                                                                                                    |                                                                                                            |
| Repo: bridge                              | `src-tauri/src/startup_bridge.rs`, `lib.rs:19-83`                                                                                                                         |                                                                                                            |
| Repo: emit sites                          | `src-tauri/src/commands/profile/shared.rs:68-72`; callers in `lifecycle.rs`, `favorites.rs`, `config_history.rs`, `optimizations.rs`, `commands/lutris.rs`                |                                                                                                            |
| Repo: golden test                         | `crates/crosshook-core/tests/launch_request_golden.rs`                                                                                                                    |                                                                                                            |
| Repo: Vitest                              | `src/hooks/__tests__/useProfileSummaries.test.ts`, `useProfileHealth.startup.test.ts`, `src/components/pages/__tests__/LibraryPage.test.tsx:449-480`, `src/lib/events.ts` |                                                                                                            |

Not verified (no fetch): Tauri JS `@tauri-apps/api/event` reference page and serde container-attribute
reference (https://serde.rs/container-attrs.html) were not retrieved; statements about `rename_all`
and tag collisions rely on the serde enum-representations page and general serde behaviour
[INFERENCE for field-collision claim].

## Search Queries Executed

No web search engine queries were run; all external evidence came from direct `read` of the
documentation URLs listed above. Repo searches executed: `spawn_blocking`, `profiles-changed`
(src and src-tauri), `emit_profiles_changed\(`, `serde\(tag = `, `insta|proptest|expect-test|…` and
`"insta"|"proptest"|…` against `Cargo.lock`, `golden|fixtures?/|include_str!|include_bytes!`,
`similar|TextDiff|MAX_DIFF…`, `Mutex|poison|fn with_conn`, `panic|\[profile`, and file reads of the
paths named inline.

## Uncertainties & Gaps

- **Confidence: High** — `spawn_blocking` / `JoinError` / broadcast lag semantics (primary tokio docs
  for the exact locked version, plus repo tests exercising lag); serde internally-tagged rules (primary
  serde docs + three in-repo uses); no `insta`/golden crates in the lock file; Vitest patterns (read
  from repo tests).
- **Confidence: Medium** — Tauri `emit` bound `Serialize + Clone` (stated in Tauri guide; docs.rs
  rendering omitted bounds); emitted-after-response ordering impact (reasoned from code, not
  measured); `insta` byte-fidelity concerns for TOML (inferred, not tested).
- **Confidence: Low** — whether `GameProfile` serialization is fully deterministic for fixture
  design (field types not audited); behaviour of other `profiles-changed` consumers on favorite
  events (only listener lines were read).
- **Freshness Note**: Evidence retrieved 2026-10-25 against tokio 1.51.0 / tauri 2.11.2 docs; docs.rs
  `latest` pages (insta) may change. The Vitest API page was a 4.x-era page; `package.json` pins
  `^4.1.12`.
