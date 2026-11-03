# YAN-794 (M1-39) — `crosshook-view-model` implementation plan

Spec: the issue's acceptance criteria plus [`decisions.md`](decisions.md) (binding D1–D19). Research: `research-*.md`.

Crate root: `src/crosshook-native/crates/crosshook-view-model/`.

## Acceptance criteria → evidence

| AC                                                                             | Evidence                                                                                                                                                                                                                                                                                      |
| ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Builds with no toolkit or tokio in its tree; clippy disallow config active     | `cargo tree -p crosshook-view-model` (default edges) shows only the crate and `proptest`'s tokio-free dev tree; `./scripts/check-app-deps.sh` enforces it in CI; crate `clippy.toml` with concrete items; falsification run (temporary `std::thread::sleep` fails clippy) recorded in the PR. |
| Library filter example has ≥ 15 tests, including property tests                | `library/tests.rs` (≥ 15 unit/Script tests) + `library/proptests.rs` (P1–P6).                                                                                                                                                                                                                 |
| Form validation mapping is tested                                              | `form/tests.rs` (F1–F9) + `settings/tests.rs`; `error/tests.rs` under `--features app` (F10).                                                                                                                                                                                                 |
| Public API documented; README explains how a ported screen adds its view-model | `#![deny(missing_docs)]`; crate `README.md` § "Adding a view-model for a ported screen".                                                                                                                                                                                                      |

## B0 — skeleton and frozen contracts (done by the orchestrator)

Files: `Cargo.toml`, `clippy.toml`, `src/lib.rs`, `src/error.rs`, `src/error/tests.rs`, `src/testing.rs`, workspace `members`.

- `ViewModel { type Intent; type Effect; fn reduce(&mut self, Self::Intent) -> Vec<Self::Effect>; }`
- `ServiceErrorKind` (mirror of `AppErrorKind`), `ServiceError { kind, message }`, `ServiceError::{new, validation}`, `From<&AppError>`/`From<AppError>` under `app`.
- `testing::Script::{new, step, assert_state, into_inner}`; `testing::fixtures` module (B2 owns its content).
- `lib.rs` declares `error`, `form`, `library`, `settings`, `testing` and re-exports `form::{Form, FormError, FormModel, SubmitId, SubmitState}`.

## B1 — form + settings example (parallel with B2, B3)

Files: `src/form.rs`, `src/form/*.rs`, `src/settings.rs`, `src/settings/*.rs`.

Contract (public):

```rust
pub trait FormModel: Clone {
    type Field: Copy + Eq + Ord + Debug + 'static;
    const FIELDS: &'static [Self::Field];
    fn field_id(field: Self::Field) -> &'static str;
    fn field_changed(&self, other: &Self, field: Self::Field) -> bool;
    fn validate(&self) -> Vec<(Self::Field, &'static str)> { Vec::new() }
}
pub struct SubmitId(u64);            // Copy, Eq, Debug, Ord
pub enum SubmitState { Idle, Submitting(SubmitId), Succeeded, Failed }
pub struct FormError { pub kind: ServiceErrorKind, pub message: String }
pub struct Form<T: FormModel> { .. }
impl<T: FormModel> Form<T> {
    pub fn new(value: T) -> Self;
    pub fn value(&self) -> &T;
    pub fn baseline(&self) -> &T;
    pub fn edit(&mut self, field: T::Field, f: impl FnOnce(&mut T));
    pub fn touch(&mut self, field: T::Field);
    pub fn begin_submit(&mut self) -> Option<(SubmitId, T)>;
    pub fn finish_submit(&mut self, id: SubmitId, result: Result<(), &ServiceError>);
    pub fn apply_errors(&mut self, error: &ServiceError);
    pub fn reset(&mut self, value: T);
    pub fn is_dirty(&self, field: T::Field) -> bool;
    pub fn is_form_dirty(&self) -> bool;
    pub fn is_touched(&self, field: T::Field) -> bool;
    pub fn error(&self, field: T::Field) -> Option<&str>;
    pub fn form_error(&self) -> Option<&FormError>;
    pub fn submit_state(&self) -> SubmitState;
    pub fn can_submit(&self) -> bool;   // not submitting and form dirty
}
```

Semantics: D8–D10. `apply_errors` maps one error (D10); `finish_submit(id, Err(e))` with a matching id goes through the same mapping; a stale id or a non-`Submitting` state is ignored. A client-invalid `begin_submit` returns `None`, sets `Failed`, writes the client errors and touches every field. `edit` clears the edited field's error and the form error, and returns `Succeeded`/`Failed` to `Idle`; during `Submitting` it only edits. `reset` replaces baseline and value, clears touched/errors, sets `Idle` (keeps the id counter so late results stay stale). Add `debug_assert!` that `field_id`s are unique (in `new`).

Settings example (D15), `settings` module:

```rust
pub struct SettingsFields { pub profiles_directory: String, pub recent_files_limit: String, pub log_filter: LogFilter }
pub enum SettingsField { ProfilesDirectory, RecentFilesLimit, LogFilter }   // ids "profiles_directory", "recent_files_limit", "log_filter"
pub enum LogFilter { Error, Warn, Info, Debug, Trace }                    // as_str()/ALL
pub struct SettingsSaveRequest { pub profiles_directory: String /* trimmed */, pub recent_files_limit: u32, pub log_filter: LogFilter }
pub enum SettingsIntent { Loaded(SettingsFields), Edit*(..) per field, Blur(SettingsField), Submit, SaveFinished { id: SubmitId, result: Result<(), ServiceError> } }
pub enum SettingsEffect { Save { id: SubmitId, request: SettingsSaveRequest } }
pub struct SettingsViewModel { form: Form<SettingsFields> }   // ViewModel impl; `form()` accessor
```

Client checks: `recent_files_limit` must parse as an integer in `1..=100` (fixed `&'static str` message). `profiles_directory` is trimmed into the request; empty means default (no client error).

Tests (≥ these, no tautologies): F1 known field → field error, `Failed`, no form error; F2 `"settings"` and an unknown id → form error verbatim, no field error; F3 table NotFound/Conflict/Unavailable/Io/Internal → form error of that kind and previous field errors cleared; F4 Cancelled → `Idle`, no banner; F5 double submit emits one `Save` (Script); F6 stale `SaveFinished` ignored; F7 success re-baselines to the submitted snapshot, editing back to original is pristine, edit during submit stays dirty; F8 client-invalid submit emits nothing, touches all, errors shown, editing that field clears only its error; F9 every `FIELDS` id round-trips through `apply_errors`.

## B2 — library example (parallel with B1, B3)

Files: `src/library.rs`, `src/library/*.rs` (`select.rs`, `sessions.rs`, `tests.rs`, `proptests.rs`), `src/testing/fixtures.rs`.

Contract (public):

```rust
pub struct ProfileCard { pub name: String, pub game_name: String, pub steam_app_id: String, pub is_favorite: bool }
pub enum LibraryFilter { All, Favorites, Installed, CurrentlyRunning }   // Default All
pub enum LibrarySort { Recent, Name }                                      // Default Recent
pub struct SessionId(String);                                             // opaque, Clone/Eq/Hash/Ord/Debug; SessionId::new(impl Into<String>)
pub enum SessionKind { Game, Trainer }
pub struct RunningSession { pub session_id: SessionId, pub profile_name: Option<String>, pub kind: SessionKind }
pub enum SessionEvent { Started { session_id, profile_name, kind, seq: u64 }, Exited { session_id, seq: u64 }, Resync }
pub struct RunningProfiles { .. }   // apply_snapshot(Vec<RunningSession>, seq) ; apply(SessionEvent) -> bool /* true => resync needed */ ; contains(&str) ; currently_playing_count() ; is_synced()
pub enum LibraryIntent { ProfilesLoaded(Result<Vec<ProfileCard>, ServiceError>), Refresh, SetQuery(String), SetFilter(LibraryFilter), SetSort(LibrarySort), ToggleFavorite(String), FavoriteSaved { name: String, result: Result<(), ServiceError> }, RunningSnapshot { sessions: Vec<RunningSession>, seq: u64 }, Session(SessionEvent) }
pub enum LibraryEffect { LoadProfiles, SetFavorite { name: String, favorite: bool }, RequestRunningSessions }
pub struct LibraryViewModel { .. }  // Default; ViewModel; selectors: visible() -> impl Iterator<Item=&ProfileCard>, visible_names(), query(), filter(), sort(), load_error() -> Option<&ServiceError>, currently_playing_count(), is_running(&str)
pub fn select::visible_indices(cards: &[ProfileCard], keys: &[String] /* lowercase search keys */, query: &str, filter: LibraryFilter, sort: LibrarySort, running: &RunningProfiles) -> Vec<usize>
```

Semantics: D11–D14. `Refresh` → `[LoadProfiles]`. `ProfilesLoaded(Err)` keeps the previous list and sets `load_error`; `Ok` replaces and clears it. `ToggleFavorite` flips optimistically and emits `SetFavorite`; `FavoriteSaved(Err)` reverts that card (React `LibraryPage.tsx:279-289`) and sets `load_error`. `Session(Resync)` → `[RequestRunningSessions]`; a `Started`/`Exited` before the first snapshot is buffered (bounded: keep the last 1024 buffered events; on overflow drop the buffer and emit `RequestRunningSessions` once). `RunningSnapshot` replaces state and applies buffered events with `seq > snapshot seq`. Visible indices are cached and recomputed in `reduce`; lowercase search keys (`name` and `game_name`) are computed once per load.

Fixtures (`testing::fixtures`): `card(name) -> CardBuilder` with `.game_name()`, `.steam_app_id()`, `.favorite()`, `.build()`; `started(id, Option<&str>, SessionKind, seq)`, `exited(id, seq)`, `running(id, Option<&str>, SessionKind)`.

`app` feature conversions in `library/app.rs`: `ProfileCard::from_summary(&ProfileSummary, is_favorite)`, `From<&RunningSessions>` → `(Vec<RunningSession>, u64)` via `running_snapshot(&RunningSessions)`, `session_event(&AppEvent) -> Option<SessionEvent>` (`Resync{..}` → `Resync`), `SessionKind` from core's, `SessionId` from core's `Display`.

Tests: the 15 listed in `research-practices.md` §5.1 + P1–P6 property tests.

## B3 — guardrail script, CI, agent docs (parallel with B1, B2)

Files: `scripts/check-app-deps.sh`, `.github/workflows/lint.yml`, `CLAUDE.md`, `AGENTS.md`, `.ai/rules/project.md`.

- Table-driven `check-app-deps.sh` (D4): `crosshook-app` normal edges, toolkit regex (unchanged behaviour); `crosshook-view-model` default edges (`normal,build,dev`), toolkit + `^(tokio|mio)([-_].*)?$`. Keep the existing selftest cases; add the view-model cases from `research-practices.md` §4.1. `shellcheck --severity=warning` clean.
- `lint.yml` `rust` job: `./scripts/check-app-deps.sh --selftest`, `./scripts/check-app-deps.sh`, view-model `--all-features` clippy (`--all-targets -D warnings`) and test; add `-p crosshook-view-model` to the existing test line.
- Agent docs (D18): Architecture sentence in `CLAUDE.md`, `AGENTS.md`, `.ai/rules/project.md`; AGENTS.md Stack Overview row and Directory Map entry; `./scripts/check-app-deps.sh` in both Commands lists.

## B4 — crate README (after B1, B2)

`crates/crosshook-view-model/README.md`: purpose and rules (no tokio/I/O/toolkit/clock/env/logging, and why); `ViewModel` contract; `Form` FSM and error-mapping tables; library and settings examples; `## Adding a view-model for a ported screen`; `## Testing` (Script, fixtures, proptest, `testing` feature); `## Storage boundary` (runtime-only); `## Dependencies` (`app` feature; guarded by `check-app-deps.sh`).

## Gates (orchestrator, after each batch and at the end)

```bash
cd src/crosshook-native
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p crosshook-view-model --all-features --all-targets -- -D warnings
cargo test -p crosshook-view-model && cargo test -p crosshook-view-model --all-features
cargo tree -p crosshook-view-model
cd ../.. && ./scripts/check-app-deps.sh --selftest && ./scripts/check-app-deps.sh
shellcheck --severity=warning scripts/check-app-deps.sh
./scripts/check-host-gateway.sh
```

Falsification (not committed): a temporary `std::thread::sleep` in the crate must fail clippy; breaking the `Installed` predicate and the seq baseline check must fail tests.
