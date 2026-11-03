# YAN-794 (M1-39) — Practices, security, guardrails and test research

Scope: the minimal API for the new `crates/crosshook-view-model`, error mapping, the repo rules this PR must satisfy, the dependency and clippy guardrails, and the test strategy. Paths are relative to the worktree root unless noted. `app/` = `src/crosshook-native/crates/crosshook-app/`.

Every **[verified]** item was checked empirically with the pinned toolchain (`rust-toolchain.toml` → 1.95.0) in throwaway crates under `/tmp`. Nothing in the repo was changed.

---

## 0. TL;DR recommendations

1. **The public surface is one trait, one form type, one form trait, a script harness and two worked examples.** There is no generic store, middleware, subscription abstraction, effect executor, derive macro or validation DSL.
2. **Use a `FormModel` trait with a per-form `Field` enum.** The `&'static str` id appears only at the `AppError` boundary. Dirty state is **derived** (`value != baseline` per field), not stored. Touched state is stored.
3. **Submit FSM: `Idle → Submitting{id} → Succeeded | Failed`.** It carries a monotonically increasing `SubmitId`, so stale results are ignored and a double submit emits no second effect.
4. **Errors:**
   - A `Validation{field}` whose field is known becomes a field error.
   - An unknown validation field, and any non-validation kind, becomes a **form-level** error that shows the already-sanitized message.
   - `Cancelled` returns to `Idle` with no banner.
   - Errors are never dropped silently.
5. **Dependencies:** the crate owns its input and error types by default. An optional `app` feature adds `From` conversions from `crosshook-app` and `crosshook-core`, and does not use a self dev-dependency, so the plain `cargo tree -p crosshook-view-model` stays tokio-free. **[verified]**
6. **clippy.toml:** list **concrete items**.
   - Module paths (`std::fs`) and globs (`std::fs::*`) are silently ineffective: clippy only warns, and `-D warnings` still exits 0. **[verified]**
   - `tokio::…` entries are inert while tokio is absent. The real tokio guard is the dependency script.
7. **Guardrail script:** generalize `scripts/check-app-deps.sh` with a per-package ban table; do not copy it.
   - Wire it into `.github/workflows/lint.yml`. Today it is **not wired at all**.
   - Add a view-model `--all-features` clippy and test step.
   - `check-host-gateway.sh` already scans the crate. No change is needed there.
8. **Versioning:** use `version.workspace = true` and **do not** add the manifest to `prepare-release.sh`. Its grep would fail on an inherited version.

---

## 1. Minimal API (KISS)

### 1.1 `ViewModel` trait (fixed by the issue)

```rust
pub trait ViewModel {
    type Intent;
    type Effect;
    /// Applies `intent`; returns effects (data) for the bridge to execute.
    fn reduce(&mut self, intent: Self::Intent) -> Vec<Self::Effect>;
}
```

- **No supertraits or bounds** (`Send`, `Default`, `'static`). The bridge decides threading: iced keeps state on the UI thread, while gpui keeps it in an `Entity`.
- **No `view`, no subscriptions, no `async`.** Selectors are free functions or `&self` methods.
- **Service results come back as intents**, for example `Intent::ProfilesLoaded(Result<Vec<Card>, ServiceError>)` or `Intent::SaveFinished { id, result }`. This is Elm's `Msg`, which carries responses.
- **`Vec::new()` does not allocate**, so returning `Vec` costs nothing for the common case of no effects. Do not reach for `SmallVec`.
- **Effects must be plain data:** `#[derive(Debug, Clone, PartialEq, Eq)]` enums with no closures, futures, `Task` or `Context`. That keeps them comparable in `testing::Script` and keeps the crate toolkit-agnostic.

**Comparison:**

| Pattern | Signature                                        | Fit                                                                                                                                                            |
| ------- | ------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Elm TEA | `update : Msg -> Model -> (Model, Cmd Msg)`      | `Cmd` is opaque, which makes it hard to test. Our `Effect` is "Cmd as data", the pattern that elm-program-test and redux-loop adopt for testability.           |
| iced    | `fn update(&mut self, m) -> Task<Message>`       | The bridge maps `Vec<Effect>` to `Task::batch(effects.map(run_service))`, and each task resolves to `Message::Vm(Intent)`. A 1:1 fit.                          |
| gpui    | `entity.update(cx, \|this, cx\| …)` + `cx.spawn` | The bridge calls `vm.reduce(i)`, spawns one task per effect, feeds results back with `entity.update(.., reduce(result_intent))`, then `cx.notify()`. Same fit. |

Both toolkits need only "call reduce, run the effects, feed the results back". The trait encodes nothing beyond that.

### 1.2 `Form<T>` — the field-keying decision

**Options:**

- **(a) `&'static str` field ids everywhere.** This is simple and matches `AppErrorKind::Validation { field: &'static str }` (`app/src/error.rs:9-12`). However, UI code can misspell ids, and per-field dirty tracking needs per-field accessors anyway.
- **(b) One enum per form, behind a small trait (recommended).** UI code is type-checked, and the string id appears in exactly one place:

```rust
pub trait FormModel: Clone {
    type Field: Copy + Eq + Ord + core::fmt::Debug + 'static;
    /// Every field, in display order.
    const FIELDS: &'static [Self::Field];
    /// Backend id used by `AppErrorKind::Validation { field }` (exact string).
    fn field_id(field: Self::Field) -> &'static str;
    /// Whether `field` differs between two values (drives derived dirty state).
    fn field_changed(&self, other: &Self, field: Self::Field) -> bool;
    /// Client-side syntactic checks only (no I/O); default: none.
    fn validate(&self) -> Vec<(Self::Field, &'static str)> { Vec::new() }
}
```

The `Form<T: FormModel>` state needs only these fields:

- `baseline: T` (pristine reference)
- `value: T`
- `touched: BTreeSet<T::Field>`
- `field_errors: BTreeMap<T::Field, String>`
- `form_error: Option<FormError>`
- `submit: SubmitState`
- `next_id: u64`

`BTreeMap` is deterministic and fine for fewer than 20 fields; avoid `HashMap`, which makes iteration order non-deterministic in snapshot asserts.

**Per-field state is derived:**

- `is_dirty(f) = baseline.field_changed(&value, f)`
- `is_pristine(f) = !is_dirty(f)`
- `is_touched(f)`
- `error(f)`

Storing a dirty flag invites desync. With derived dirty state, typing a value back to its original makes the field pristine again for free.

**Methods (keep to this list):**

- `new(T)`
- `value()`
- `edit(field, impl FnOnce(&mut T))` — clears that field's error, and the form error, when not submitting
- `touch(field)`
- `begin_submit() -> Option<(SubmitId, T)>`
- `finish_submit(id, Result<(), &ServiceError>)`
- `apply_errors(&ServiceError)` — the AC name. An `AppError` carries exactly one field, so the method takes one error.
- `reset(T)` — external reload
- Selectors: `is_dirty`, `is_touched`, `error`, `form_error`, `submit_state`, `can_submit`

**Composition:** `Form` does **not** implement `ViewModel`. A screen view-model owns a `Form` and maps its own intents to these calls. It emits its own domain effect (for example `Effect::SaveSettings { id, request }`) from `begin_submit()`'s return value. This avoids a generic `Form<T, E>` effect parameter.

- Add a `debug_assert!` that `field_id`s are unique.
- Add one contract test that round-trips every `FIELDS` entry through `apply_errors` (see §5).

### 1.3 Submit state machine

```rust
pub struct SubmitId(u64);
pub enum SubmitState { Idle, Submitting(SubmitId), Succeeded, Failed }
```

| From                      | Event                                                              | To                 | Effect / notes                                                                      |
| ------------------------- | ------------------------------------------------------------------ | ------------------ | ----------------------------------------------------------------------------------- |
| Idle, Succeeded or Failed | `begin_submit`, client-valid                                       | `Submitting(id+1)` | Returns `Some((id, value.clone()))`. The screen view-model emits the save effect.   |
| Idle, Succeeded or Failed | `begin_submit`, `validate()` non-empty                             | `Failed`           | Returns `None`. Sets field errors and touches **all** fields so the errors show.    |
| `Submitting(_)`           | `begin_submit`                                                     | unchanged          | Returns `None`. **No duplicate save.**                                              |
| `Submitting(id)`          | `finish_submit(id, Ok)`                                            | `Succeeded`        | `baseline :=` the **submitted snapshot**, so edits made during the save stay dirty. |
| `Submitting(id)`          | `finish_submit(id, Err(Validation{field}))`, field known           | `Failed`           | Field error on that field.                                                          |
| `Submitting(id)`          | `Err`, unknown field or any non-validation kind except `Cancelled` | `Failed`           | `form_error = Some(..)`.                                                            |
| `Submitting(id)`          | `Err(Cancelled)`                                                   | `Idle`             | No banner (shutdown or caller-drop race, `app/src/error.rs:21-22,44-50`).           |
| `Submitting(x)`           | `finish_submit(y ≠ x, _)`, or any state that is not `Submitting`   | unchanged          | Stale result is ignored.                                                            |
| Succeeded or Failed       | `edit`                                                             | `Idle`             | Clears `form_error`, plus the edited field's error. Other field errors stay.        |
| `Submitting`              | `edit`                                                             | `Submitting`       | Allowed; the value diverges from the snapshot.                                      |

This is four states. Do not add `Validating`, `Dirty` or `Pristine` states: they are per-field derived properties, not form states.

### 1.4 Selectors and the library example

The React oracle (behavior to match):

- **Search** (`src/hooks/useLibraryProfiles.ts:6-8`): trim the query and lowercase it. Match `name` **or** `gameName` by substring. An empty query passes everything.
- **Filter** (`src/components/pages/LibraryPage.tsx:126-138`):
  - `favorites` → `isFavorite`.
  - `installed` → `steamAppId` is non-empty and not `"0"` (`:132-133`).
  - `currentlyRunning` → `runningProfiles.has(name)` (`:136`).
  - `default` → no filter.
- **Sort** (`:141-142`): `name` sorts by `(gameName || name).localeCompare(..)`. `recent` keeps source order.
- **Exposed keys:** the toolbar exposes only sort `recent|name` and filter `all|favorites|installed|currentlyRunning` (`src/components/library/LibraryToolbar.tsx:4-14`). `recentlyLaunched`, `lastPlayed` and `playtime` exist only in the TS union (`src/types/library.ts:3-5`) and are no-ops.
  - **Do not port the dead variants**, per CLAUDE.md:61 ("No dead code").
- **Running set** (`src/hooks/runningSessionsStore.ts:59-64`): only `Game` sessions with a non-null `profile_name` count. The set is **derived from the session map**, so two game sessions of one profile count once, and exiting one leaves the profile running.
- **Badge** (`src/components/layout/AppShell.tsx:93-97`): `currentlyRunning = runningProfiles.size`, shown only when greater than 0.
- **Seq contract** (`app/src/services/launch/sessions.rs:1-9`, `app/src/events.rs:113-144`, mirrored by `runningSessionsStore.ts:13-30,126-178`): subscribe, then snapshot; apply only events with `seq > snapshot.seq`; buffer events while no baseline exists; `Resync` triggers a new snapshot.

**Recommended pure functions** (the targets of the property tests):

- `select::visible(cards, &Query) -> Vec<usize>`, which returns indices.
- `RunningProfiles` tracker:
  - `apply_snapshot(sessions, seq)`
  - `apply(SessionEvent) -> Option<Effect>` (`Resync` → `RequestRunningSessions`)
  - `contains(name)`
  - `currently_playing_count()`

**Allocation (repo engineering rule: no avoidable allocation):**

- Precompute a lowercase search key per card **once** when profiles load.
- Recompute `visible` in `reduce`, on state change only, and not per frame. iced and gpui may call `view` or `render` every frame, and per-frame `to_lowercase()` on 500 cards allocates needlessly.
- A `visible()` selector then returns an iterator over the cached indices.

**Determinism:** JS `localeCompare` is locale-aware. Rust needs a **total** comparator:

1. Case-insensitive display key.
2. Then the display key.
3. Then `name`.

Profile names are unique, so the order is total. That makes "sort is order-independent of input" a valid property. Document the deliberate divergence from ICU collation; ParityResearch owns the exact rules.

### 1.5 `testing::Script` and fixtures

The issue asks for "a sequence of intents, assert state + effects". The smallest strict primitive is **one call per step**, so no step can silently ignore its effects:

```rust
Script::new(LibraryViewModel::default())
    .step(Intent::Refresh, [Effect::LoadProfiles])
    .step(Intent::ProfilesLoaded(Ok(cards)), [])
    .step(Intent::SetQuery("al".into()), [])
    .assert_state(|vm| assert_eq!(names(vm), ["Alpha"]))
    .into_inner();
```

- `step(intent, expected: impl IntoIterator<Item = Effect>)` compares the **exact, ordered** effects. It is `#[track_caller]` and panics with the step index plus the expected and actual `Debug` output.
- `assert_state(impl FnOnce(&VM))` is `#[track_caller]`.
- `into_inner()` returns the view-model for further manual assertions.
- Bound: `VM::Effect: PartialEq + Debug`.
- **Visibility:** `pub mod testing`, gated `#[cfg(any(test, feature = "testing"))]`, so `crosshook-ui` screen tests can reuse it. It has no extra dependencies.
- **Fixture builders** (in `testing::fixtures`):
  - `card("Alpha").game_name("Alpha Game").steam_app_id("123").favorite()`
  - `started(session, Some("Alpha"), Kind::Game, seq)`
  - `exited(..)`
  - `snapshot([..], seq)`
- **proptest strategies** (`arb_cards()` and similar) stay in `#[cfg(test)]` files, because `proptest` is a dev-dependency.

### 1.6 Do not build (over-abstraction warning, 3-pt scaffold)

- Generic `Store<VM>`, dispatcher, middleware, effect-executor trait, or `Command` combinators. The bridge is toolkit code and lives in `crosshook-ui`.
- `#[derive(FormModel)]` proc-macro, a validation DSL, async validators, or debounced validation. Debouncing needs a clock, and the crate gets no clock (§2.3).
- A memoized-selector library or `Lens` types. Use a cached `Vec<usize>` recomputed in `reduce`.
- A `trait Selector`. Free functions are enough.
- Speculative M2 view-models (autosave FSM, launch FSM). Each is ported with its screen.

---

## 2. Error mapping and security

### 2.1 Facts

- `AppError { kind: AppErrorKind, message: String }` (`app/src/error.rs:29-34`). The message is passed through `sanitize_message` at construction (`:37-42`; `app/src/presenter/mod.rs:31-40` redacts `$HOME`, `/home/<user>` and `/var/home/<user>`).
  - Variants with path-bearing payloads use fixed strings (`:88-103`, `:169-183`).
  - The doc comment states it never embeds raw store paths, SQL or panic payloads (`:27-28`).
- `AppErrorKind` (`:6-25`) is **not** `#[non_exhaustive]`, so an exhaustive `match` in another crate breaks at compile time when a variant is added. Use that as a drift guard.
- Validation field ids in use include:
  - Entity-level ids: `"profile"` (`:114,129,136`), `"settings"` (`:178`), `"path"` (`:232`), `"metadata"` (`:287,291`), `"launch"` (`:317`).
  - Field-level ids: `"profiles_directory"` (`app/src/services/settings.rs:241-245`), `"prefix_path"` (`services/install/mod.rs:88`), `"profile_name"`, `"tool_id"`, `"game_path"` and others.
- **Constructors visible outside `crosshook-app`:** only `AppError::validation(field, msg)` (`:53-55`) and the public `From` impls, such as `From<std::io::Error>` → `Io` (`:99-103`). `AppError::new` is `pub(crate)` (`:37`).
  - So `app`-feature conversion tests can build only `Validation` and `Io` errors directly.
  - Mapping every kind must therefore be tested on the view-model's **own** `ServiceError`.

### 2.2 Mapping rules

The view-model owns a mirror type, so the default build needs no `crosshook-app`:

```rust
pub enum ServiceErrorKind { Validation { field: &'static str }, NotFound, Conflict, Unavailable, Io, Cancelled, Internal }
pub struct ServiceError { pub kind: ServiceErrorKind, pub message: String }
#[cfg(feature = "app")] impl From<&crosshook_app::AppError> for ServiceError { /* exhaustive match, no `_` arm */ }
```

| Incoming                                                                  | `Form` result                                                                                                                                                                     |
| ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Validation{field}` where `field_id(f) == field` for some `f`             | `field_errors[f] = message`; `Failed`                                                                                                                                             |
| `Validation{field}` matching no field (`"settings"`, `"profile"`, a typo) | **Form-level banner** `FormError { kind, message }`; `Failed`. Never dropped (CLAUDE.md:63, "Never silently swallow errors"). Field errors from the previous attempt are cleared. |
| `NotFound`, `Conflict`, `Unavailable`, `Io`, `Internal`                   | Form-level banner with the sanitized message; `Failed`                                                                                                                            |
| `Cancelled`                                                               | `Idle`, no banner                                                                                                                                                                 |

The library view-model uses the same `ServiceError` for a failed `ProfilesLoaded(Err)`:

- It sets `load_error`.
- It **keeps the previous list** (degraded fallback).
- A later success clears the error.

### 2.3 Security and purity rules for the crate

- **No I/O, no environment, no clock, no logging.**
  - Do not call `sanitize_message` or `display_path_tilde`: they read `$HOME` (`presenter/mod.rs:7-8,31-33`). Messages arrive already sanitized; store them verbatim as `String`.
  - Time-dependent behavior (none needed now) takes `now` as intent data.
- **Client validation messages are fixed `&'static str`** (the `validate()` signature enforces this). They never `format!` user input, so a typed path cannot leak into a message, a log or a crash report.
- **Paths:** `ProfileSummary` carries `custom_cover_art_path` and `custom_portrait_art_path` (`app/src/services/profiles/types.rs:24-27`). Carry them only if the card renders art. Never put them into error text, and keep them out of `Debug`-asserted fixtures beyond what tests need.
- **No logging dependency:** add no `tracing` or `log` dependency. Add these to `lib.rs`:
  - `#![forbid(unsafe_code)]`
  - `#![deny(missing_docs)]` (precedent: `app/src/lib.rs:8`)
  - `#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]`

  Together these make "no logging" mechanical.

### 2.4 Persistence classification (CLAUDE.md:51; AGENTS.md:252-260)

- **Everything is ephemeral runtime state (memory only):** form values, baselines, touched and error maps, submit state, library query, filter, sort, the visible index cache and the running-session map.
  - Matches today's React, where sort and filter are `useState` (`LibraryPage.tsx:76-77`).
- **No TOML, SQLite or schema change.**
- **Library view mode** stays persisted through the existing `[ui]` settings (`UiSettingsService`, `app/README.md` § Storage boundary). If a view-model ever owns it, it emits an effect to that service; it never persists anything itself.
- **Migration, offline and degraded behavior:**
  - No migration is needed.
  - Offline behavior: not applicable.
  - Degraded behavior: a load failure keeps the last good list and shows the sanitized banner.
  - The user sees exactly what Classic shows.
- **README:** the crate's README gets a short `## Storage boundary` section saying "runtime-only; no persistence". This matches the `app/README.md:592` section style.

---

## 3. Repo rules that bind this PR, and the docs to update

### 3.1 Rules (with evidence)

- **File size and modularity:**
  - Aim for about 500 lines per file (CLAUDE.md:53).
  - Small cohesive modules; no copy-paste (DRY); no circular dependencies (CLAUDE.md:54).
  - Single responsibility (CLAUDE.md:55).
- **Proposed layout.** Test files are siblings, following the app precedent `events.rs` + `events/tests.rs` and `error.rs` + `error/tests.rs`.

  ```
  crates/crosshook-view-model/
    Cargo.toml  clippy.toml  README.md
    src/lib.rs                 # crate attrs, ViewModel trait, re-exports
    src/error.rs (+ error/tests.rs)          # ServiceError/Kind, `app` From impls
    src/form.rs  (+ form/submit.rs, form/tests.rs)
    src/library.rs (+ library/select.rs, library/sessions.rs, library/tests.rs, library/proptests.rs)
    src/settings.rs (+ settings/tests.rs)    # worked field-form example
    src/testing.rs (+ testing/fixtures.rs)
  ```

- **Cargo.toml:**
  - `version.workspace = true` (as `app/Cargo.toml:4`).
  - `edition = "2021"` (all crates; `rustfmt.toml:1`).
  - `[lints] workspace = true` (`app/Cargo.toml:8-10`), so the workspace clippy set at `src/crosshook-native/Cargo.toml:13-31` applies.
  - Add the crate to `members` (`Cargo.toml:2-7`).
  - Do not use `version = "0.6.1"`. `scripts/prepare-release.sh:8-13,74-75` greps `^version = "<v>"` in every listed manifest. `crosshook-app` is deliberately not listed and inherits instead. **Do not add the new crate to that list** (contrary to `docs/research/native-ui-migration/README.md:616`), or release prep fails.
- **Dependency hygiene** (CLAUDE.md:62): the only new third-party crate is `proptest` (dev, MIT/Apache-2.0, required by the AC).
  - Use `default-features = false, features = ["std"]`. That drops the `fork`/`timeout` features, and with them `rusty-fork` and `tempfile`.
  - **[verified]** `proptest` 1.12.0 with these features runs `proptest!` tests, and passes `--all-targets` clippy under the disallow list below.
  - It is already in the local cargo cache; `Cargo.lock` has no `proptest` yet.
- **Tests ship with the change** (CLAUDE.md:64). No-tautology norm (`docs/plans/launch-service/launch-service.plan.md:628`, "Only critical tests. There are no wiring or tautology tests"; `docs/plans/bootstrap/plan.md:40`).
- **Labels** (CLAUDE.md:96-103): `type:feature`, `area:ui`, and `area:build` (CI and script change). No other labels.
- **PR title:** a Conventional Commit, squash-merged verbatim (CLAUDE.md:42,44).
  - Precedent: `feat(app): add toolkit-independent service scaffold (#513)`.
  - Suggested: `feat(view-model): add pure view-model crate with library filter and settings form examples`.
  - These plan docs commit as `docs(internal): …` (CLAUDE.md:45).

### 3.2 Exact doc sections to update in this PR

The precedent is `b9021d2`, which touched `CLAUDE.md`, `AGENTS.md` and `.ai/rules/project.md` with one line each, plus the crate README.

| File                                                               | Section / line                                      | Change                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------------------------------------------ | --------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CLAUDE.md`                                                        | MUST § **Architecture** bullet (line 31)            | One sentence: `crosshook-view-model` holds pure, toolkit-agnostic view state and reducers (`ViewModel::reduce` → effects as data). It has no tokio, I/O or toolkit, and its only optional dependency is `crosshook-app`/`crosshook-core` behind the `app` feature.                                                                                       |
| `AGENTS.md`                                                        | Policy mirror § **Architecture** (line 44)          | Same sentence, kept in sync with CLAUDE.md (AGENTS.md:16-21 sync policy).                                                                                                                                                                                                                                                                                |
| `.ai/rules/project.md`                                             | **Architecture** bullet (line 14)                   | Same sentence. Note that `.cursor/rules/project.mdc`, referenced at AGENTS.md:10, does not exist in this tree.                                                                                                                                                                                                                                           |
| `AGENTS.md`                                                        | **Stack Overview** table (lines 153-162)            | New row after "Application services": `View models` · **Rust** (`crosshook-view-model` crate) · pure state + reducers, `Form<T>`, `testing::Script`; no tokio/I/O/UI toolkit; see the crate README.                                                                                                                                                      |
| `AGENTS.md`                                                        | **Directory Map** (lines 179-198)                   | Add `│   ├── crosshook-view-model/  # Pure view state + reducers (ViewModel, Form<T>, library/settings examples, testing::Script)` between `crosshook-cli/` and `crosshook-core/`.                                                                                                                                                                       |
| `CLAUDE.md` + `AGENTS.md`                                          | **Commands** (CLAUDE.md:114-134; AGENTS.md:123-141) | Add `./scripts/check-app-deps.sh   # no UI toolkit in crosshook-app; no toolkit/tokio in crosshook-view-model`. It is missing today even though every plan runs it.                                                                                                                                                                                      |
| `src/crosshook-native/crates/crosshook-view-model/README.md` (new) | Whole file                                          | Purpose; rules (no tokio/I/O/toolkit/clock/logging, and why); the `ViewModel` contract; `Form` + FSM table; error mapping table; `## Adding a view-model for a ported screen` (the AC); `## Testing` (Script, fixtures, proptest); `## Storage boundary` (runtime-only); `## Dependencies` (+ the `app` feature, and how `check-app-deps.sh` guards it). |
| `docs/architecture/adr-0001-platform-host-gateway.md`              | § Enforcement (line 118)                            | **No change.** It already lists `crates/crosshook-view-model/src`.                                                                                                                                                                                                                                                                                       |
| `app/README.md`                                                    | —                                                   | **No change.** The view-model does not alter `crosshook-app`.                                                                                                                                                                                                                                                                                            |
| `CHANGELOG.md`                                                     | —                                                   | Do not edit; git-cliff generates it (CLAUDE.md:44).                                                                                                                                                                                                                                                                                                      |

**Outline for the README section "Adding a view-model for a ported screen":**

1. Create `src/<screen>.rs`. Define `State`, `Intent` (user actions **plus** service results) and `Effect` (service-call data, `PartialEq`).
2. Implement `ViewModel`. Keep the pure logic in free functions in `<screen>/select.rs`.
3. For forms: define a `Field` enum and implement `FormModel`. `field_id` must equal the backend `AppErrorKind::Validation` field string.
4. Write a `testing::Script` test for each user flow, plus property tests for any filter or sort.
5. In the bridge, not in this crate: map each `Effect` to a `crosshook-app` service call, and convert its `Result<_, AppError>` with `ServiceError::from` (`app` feature) into the result intent.

---

## 4. Guardrails

### 4.1 Generalize `scripts/check-app-deps.sh` (recommended over a new script)

**Today:**

- One package is hard-coded (`PACKAGE="crosshook-app"`, line 14).
- One regex (`BANNED_RE`, line 18).
- `cargo tree … -e normal` (line 97).
- A pure-bash `check_tree` (lines 38-54) and `--selftest` (lines 56-85).

A copy-pasted `check-view-model-deps.sh` would duplicate about 100 lines (CLAUDE.md:54, DRY). Renaming the script would break dozens of plan and doc references (`docs/plans/**`). **Keep the name and make the script table-driven:**

```bash
TOOLKIT_RE='^(tauri|wry|webkit2gtk|gtk|glib|iced|gpui|winit)([-_0-9].*)?$'
ASYNC_RE='^(tokio|mio)([-_].*)?$'          # tokio, tokio-util, tokio-macros, tokio_stream, mio
# package | edges | banned regexes
CHECKS=(
  "crosshook-app|normal|$TOOLKIT_RE"
  "crosshook-view-model|normal,build,dev|$TOOLKIT_RE $ASYNC_RE"
)
```

- `check_tree` takes its regex list as an argument.
- The `main` loop runs `cargo tree -p "$pkg" -e "$edges" --prefix none --format '{p}'` per entry.
- The help text is updated.

**Edges for the view-model:** use `normal,build,dev`, which is the **default** `cargo tree` edge set.

- This makes the AC wording ("`cargo tree -p crosshook-view-model` must contain no toolkit, gtk or tokio") literally true.
- It forbids a dev-dependency on `crosshook-app` (which brings tokio).
- `proptest` with `std` only has no tokio. **[verified]** Its tree: `bitflags cfg-if chacha20 core_detect cpufeatures getrandom libc num-traits proptest rand rand_core rand_xorshift regex-syntax unarray`.

**Feature-unification facts [verified, resolver 2, toy workspace]:**

- With `vm` declaring `app = ["dep:heavy"]` and `ui` enabling it:
  - `cargo tree -p vm -e normal` → `vm` only.
  - `cargo tree -p vm -p ui -e normal` → includes `heavy` and `tokio`.
  - So per-package checks are unaffected by sibling crates that enable `app`.
- A self dev-dependency `vm = { path = ".", features = ["app"] }` (the pattern at `app/Cargo.toml:31`) makes plain `cargo tree -p vm` show `heavy` and `tokio`. **Do not copy that pattern.**
  - Test the feature with `cargo test -p crosshook-view-model --features app` instead.

**Selftest additions:**

- `tokio` is rejected for the view-model and accepted for the app.
- `tokio-util`, `tokio_stream` and `mio` are rejected.
- `tokioz` and `miox` are accepted, so the regex is not over-broad.
- `crosshook-core` alone (as a stand-in line) is accepted, so the ban is by family name, not by "any crosshook crate".
- The selftest must pass a regex list to `check_tree`, so the existing 11 cases stay unchanged.

**Optional hard-ban:** `tracing`/`log` in the view-model regex. This is defense in depth for "no logging" (§2.3); it is cheap and has no false positives today.

### 4.2 CI wiring (`.github/workflows/lint.yml`)

`check-app-deps.sh` is **not wired anywhere today**: there is no reference in `lint.yml`, `scripts/lint.sh` or `lefthook.yml`. This was already noted in `docs/plans/session-registry-events/research-risks.md:210`.

Add to the **`rust` job**, which already has the 1.95.0 toolchain, the cargo cache and system dependencies (lines 17-61):

```yaml
- name: UI-free dependency guardrail
  run: |
    ./scripts/check-app-deps.sh --selftest
    ./scripts/check-app-deps.sh

- name: View-model clippy and tests (all features)
  run: |
    cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-view-model --all-features --all-targets -- -D warnings
    cargo test   --manifest-path src/crosshook-native/Cargo.toml -p crosshook-view-model --all-features
```

**Why the extra step:**

- The workspace clippy (line 49) builds default features only, and no member enables `app` or `testing` yet. Without this step, the feature-gated `From` impls and `testing` would never be linted or tested.
- The test list at line 52 (`-p crosshook-app -p crosshook-native -p crosshook-cli`) does not include the new crate. The `--all-features` test above covers the default build too, so line 52 needs no change. Alternatively, append `-p crosshook-view-model` there and keep only the feature run separate.
- **Optional:** add `--app-deps` to `scripts/lint.sh` (flags at lines 11-60, run blocks at 210-241) so `./scripts/lint.sh` reproduces CI locally. It mirrors `RUN_STARTUP_PATH`.

### 4.3 `check-host-gateway.sh`

`SCAN_DIRS` already contains `crates/crosshook-view-model/src`:

- `scripts/check-host-gateway.sh:17`
- The self-test `ROOTS` (`scripts/tests/check-host-gateway/run.sh:15`)
- ADR-0001:118

Missing roots are skipped (`:64-74`), so the crate is scanned automatically once it exists. **No change.** The no-I/O clippy bans make a `Command::new` impossible anyway.

### 4.4 `clippy.toml` (crate-local)

**Verified behavior:**

- A `clippy.toml` in the **crate directory** is honored for that package inside a workspace. A toy workspace with `crates/vm/clippy.toml` failed `cargo clippy --all-targets -- -D warnings` on `std::thread::sleep` for both lib and lib-test. No workspace-root `clippy.toml` exists here.
- **Module paths don't work:**
  - `{ path = "std::fs" }` / `{ path = "std::process" }` → `warning: expected a function, found a module`.
  - Globs: `std::fs::*` → `does not refer to a reachable function`.
  - These config warnings **do not fail** `-D warnings` (exit 0). The ban would be silently empty. **Enumerate concrete items.**
- **`tokio::…` entries when tokio is not a dependency:** no warning, and inert, with or without `allow-invalid = true`. Keep them for intent, but the enforcement is §4.1.
- `disallowed-types = std::fs::File` flags `File::open(..)`, the `use std::fs::File as F` import, and aliased uses.

**Recommended content** (all paths verified to resolve; no warnings):

```toml
# crosshook-view-model is pure: no I/O, processes, threads, clocks, env or async runtime.
disallowed-methods = [
  { path = "std::thread::sleep", reason = "view-models are pure; time arrives as intent data" },
  { path = "std::thread::spawn", reason = "the bridge owns concurrency" },
  { path = "std::fs::read", reason = "no I/O" }, { path = "std::fs::read_to_string", reason = "no I/O" },
  { path = "std::fs::read_dir", reason = "no I/O" }, { path = "std::fs::write", reason = "no I/O" },
  { path = "std::fs::create_dir", reason = "no I/O" }, { path = "std::fs::create_dir_all", reason = "no I/O" },
  { path = "std::fs::remove_file", reason = "no I/O" }, { path = "std::fs::remove_dir_all", reason = "no I/O" },
  { path = "std::fs::rename", reason = "no I/O" }, { path = "std::fs::copy", reason = "no I/O" },
  { path = "std::fs::metadata", reason = "no I/O" }, { path = "std::fs::canonicalize", reason = "no I/O" },
  { path = "std::process::exit", reason = "no process control" },
  { path = "std::env::var", reason = "no environment reads" }, { path = "std::env::var_os", reason = "no environment reads" },
  { path = "std::time::SystemTime::now", reason = "no clocks; pass time in intents" },
  { path = "std::time::Instant::now", reason = "no clocks; pass time in intents" },
  { path = "tokio::spawn", reason = "no async runtime" },
  { path = "tokio::task::spawn_blocking", reason = "no async runtime" },
  { path = "tokio::time::sleep", reason = "no async runtime" },
]
disallowed-types = [
  { path = "std::fs::File", reason = "no I/O" }, { path = "std::fs::OpenOptions", reason = "no I/O" },
  { path = "std::process::Command", reason = "no processes" }, { path = "std::process::Child", reason = "no processes" },
  { path = "std::net::TcpStream", reason = "no network" },
  { path = "tokio::runtime::Runtime", reason = "no async runtime" },
  { path = "tokio::runtime::Handle", reason = "no async runtime" },
  { path = "tokio::process::Command", reason = "no async runtime" },
]
```

- The AC literally requires `std::fs`, `std::process`, `std::thread::sleep` and `tokio::*`. The `std::env` and `std::time` entries are a cheap purity extension.
- **Before committing:** run `cargo clippy -p crosshook-view-model -- -D warnings` once and confirm there are **no** "does not refer to a reachable" warnings. Any such entry is a no-op.
- **Falsifiability:** add one temporary `std::thread::sleep` call to prove the configuration is active, then remove it. Repo memory: security tests must be proven to fail. Mention this in the PR test plan, not as a committed test.

---

## 5. Test strategy

**Principles:**

- No tautologies: no tests of derives, getters, "reduce returns a Vec", or an oracle that re-implements the filter (`launch-service.plan.md:628`).
- Properties are stated as independent invariants.
- Effects are asserted exactly with `Script::step`.
- No sleeps or timers (repo convention, `docs/plans/session-registry-events/research-risks.md:149`).
- Each critical test is proven falsifiable once, by breaking the predicate, before the PR is marked ready.

### 5.1 Library example — at least 15 tests (AC), of which 6 are property tests

**Unit and Script tests:**

1. Search trims and is case-insensitive on `name` (`"  ALP "` matches `alpha`).
2. Search matches `game_name` when `name` does not.
3. An empty or whitespace query yields all cards in source order (`Recent` sort).
4. The `Favorites` filter keeps only favorites.
5. The `Installed` filter excludes `steam_app_id` values `""` and `"0"` and keeps `"123"`.
6. `Name` sort uses `game_name`, falling back to `name` when `game_name` is empty, with a deterministic tie-break.
7. Search, filter and sort compose. A single Script: set query, then filter, then sort, asserting `visible` after each step.
8. `Refresh` emits exactly `[LoadProfiles]`. `ProfilesLoaded(Err)` keeps the previous list and sets the banner; a later `Ok` clears it.
9. The running set counts only `Game` sessions with `Some(profile_name)`. Trainer and anonymous sessions are excluded (`runningSessionsStore.ts:62`).
10. Two game sessions of one profile give a count of 1. Exiting one keeps the profile running; exiting both gives 0.
11. Seq baseline: after `snapshot(seq=5)`, events with `seq ≤ 5` are ignored and `seq 6` applies.
12. Events that arrive before the first snapshot are buffered. Only those with `seq > snapshot.seq` are applied on snapshot (mirrors `runningSessionsStore.test.ts:102`).
13. `Resync` emits exactly `[RequestRunningSessions]`. The next snapshot **replaces** state; it does not merge.
14. A duplicate `Started` is idempotent, and an `Exited` for an unknown id is a no-op.
15. The `CurrentlyRunning` filter shows exactly the running profiles, and updates in response to a `SessionStarted` intent alone, without a reload.

**Property tests** (`library/proptests.rs`; the strategy generates `Vec<Card>` with unique names, arbitrary favorites and app ids from `{"", "0", digits}`, and arbitrary queries):

- **P1, soundness and completeness:** every visible card satisfies the active predicate, and every input card that satisfies it (checked by an independent per-card predicate) is visible.
- **P2, order:** under `Recent`, `visible` is strictly increasing (a subsequence of the source order). Under `Name`, adjacent pairs are ordered by the comparator, and `visible` is a permutation of the filtered set (no duplicates or losses).
- **P3, search monotonicity:** for any query `q` and extension `q + s`, `visible(q + s) ⊆ visible(q)`.
- **P4, input-order independence:** under `Name`, shuffling the input cards yields the same visible _names_. This holds because the comparator is total.
- **P5, seq safety:** applying `snapshot(S, seq)` followed by any events with `seq ≤ S.seq` leaves the running set equal to the set derived from `S`.
- **P6, count bounds:** for any event sequence, `0 ≤ count ≤ #live Game sessions`, and `count == |distinct named live Game sessions|`. The right-hand side is computed from the generated live-session list, not from the tracker.

### 5.2 Form and settings example (the AC "form validation mapping tested")

- **F1:** `Validation{field: "profiles_directory"}` produces a field error on `ProfilesDirectory` and `Failed`, with no form error. This is the real backend id from `services/settings.rs:241-245`.
- **F2:** `Validation{field: "settings"}` (entity-level, `error.rs:178`) and an unknown id produce a form-level banner carrying the message verbatim, and no field error.
- **F3:** Table test. `NotFound`, `Conflict`, `Unavailable`, `Io` and `Internal` each produce a form banner with the matching kind, and the previous field errors are cleared.
- **F4:** `Cancelled` returns to `Idle` with no banner.
- **F5:** `begin_submit` while `Submitting` returns `None`. A Script asserts only **one** `SaveSettings` effect.
- **F6:** A stale `finish_submit(old_id, ..)` is ignored; state stays `Submitting(new_id)`.
- **F7:** Success makes the baseline the submitted snapshot. A field edited back to its original value becomes pristine, and an edit made during submit stays dirty.
- **F8:** A client-invalid submit (for example an empty required text) emits no effect, touches every field and shows the errors. Editing that field clears only its own error.
- **F9, contract:** for every `f` in `FIELDS`, `apply_errors(Validation{field: field_id(f)})` lands on `f`. This catches duplicate or misspelled ids at test time.
- **F10 (`--features app`):** `ServiceError::from(&AppError::validation("profiles_directory", "x"))` preserves the kind, field and message. `AppError::from(io::Error)` → `Io`. These are the only kinds constructible outside the crate (§2.1). The exhaustive `match` with no `_` arm provides compile-time drift protection for the rest.

**Settings example scope:** pick fields whose real validation is server-side, such as `profiles_directory`, plus a simple required or trim rule. **Do not** re-implement core's numeric clamps (for example `RECENT_FILES_LIMIT_MIN`/`MAX`, `crosshook-core/src/settings/types.rs:11-13,36-37`) in the default build. That duplicates domain rules (CLAUDE.md:54), and core cannot be a default dependency. Let the server's `Validation` drive the error.

### 5.3 Commands for the main agent (after implementation)

```bash
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-view-model --all-features --all-targets -- -D warnings
cargo test   --manifest-path src/crosshook-native/Cargo.toml -p crosshook-view-model --all-features
cargo tree   --manifest-path src/crosshook-native/Cargo.toml -p crosshook-view-model   # AC: no tokio/gtk/toolkit lines
./scripts/check-app-deps.sh --selftest && ./scripts/check-app-deps.sh
./scripts/check-host-gateway.sh && ./scripts/check-host-gateway.sh --self-test
shellcheck --severity=warning scripts/check-app-deps.sh
```

---

## 6. Risks and judgment calls for Main

| Risk / decision                                                                                                                           | Recommendation                                                                                                                                                                                                                                                                             |
| ----------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| The issue says "depends on crosshook-core/crosshook-app", but both pull in tokio (`app/Cargo.toml:18,23`; `crosshook-core/Cargo.toml:32`) | Optional `app` feature, off by default; record the reason in the README and the PR body. Extracting a tokio-free `crosshook-app-types` crate would be a far larger refactor. Free functions in the bridge would duplicate code across iced and gpui (the orphan rule blocks `From` there). |
| Clippy bans silently do nothing if the paths are wrong                                                                                    | Concrete paths only, plus the one-time falsification check in §4.4.                                                                                                                                                                                                                        |
| `localeCompare` versus a Rust comparator                                                                                                  | Documented total comparator; ParityResearch decides the fold.                                                                                                                                                                                                                              |
| Porting the dead TS keys (`recentlyLaunched`, `lastPlayed`, `playtime`)                                                                   | Do not port them. Add them with the screen that implements them.                                                                                                                                                                                                                           |
| `prepare-release.sh` list                                                                                                                 | Do not add the crate (an inherited version fails the grep at `:74-75`).                                                                                                                                                                                                                    |
| `proptest` failure-persistence files (`proptest-regressions/`) written next to the sources on failure                                     | Commit any that appear; it is proptest's recommended practice, and they are deterministic seeds with no paths or user data.                                                                                                                                                                |
