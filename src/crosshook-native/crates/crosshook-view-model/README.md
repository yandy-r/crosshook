# crosshook-view-model

Pure, toolkit-agnostic view state and reducers for CrossHook screens. A
screen's view-model owns its state and turns each intent (a user action or a
service result) into a new state plus a list of effects: plain data that
describes service calls. The UI bridge (iced, gpui, or a test) executes the
effects and feeds each result back as another intent. The same view-model
drives every toolkit, and its tests need no UI, runtime or disk.

Contents: the `ViewModel` trait, `Form<T>` (dirty/touched tracking and a
submit state machine), the `ServiceError` mirror of `crosshook_app::AppError`,
two worked screens (`library`, `settings`), the launch view-models
(`launch`: dependency gate, pipeline and its detail panel), the Hero
Detail model (`hero`), the profile autosave state machine (`profile::autosave`),
and the `testing` harness.

## Rules

A view-model is a function of its state and the intents it receives. It never:

| Banned                          | Why                                                                                             |
| ------------------------------- | ----------------------------------------------------------------------------------------------- |
| tokio / async runtime           | The bridge owns concurrency. A runtime here would tie reducers to one executor.                 |
| File, network or process I/O    | Service calls are effects; the bridge runs them through `crosshook-app`.                        |
| UI toolkit types                | One view-model serves iced, gpui and tests.                                                     |
| Clocks (`SystemTime`/`Instant`) | Time arrives as intent data, so reductions are deterministic and replayable.                    |
| Environment reads               | Same reason. Error messages arrive already sanitized; re-sanitizing would read `$HOME`.         |
| Logging (`tracing`, `log`)      | No side effects. Report state through selectors; the bridge logs. No logging dependency exists. |

Enforcement:

- `lib.rs`: `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, and
  `#![deny(clippy::print_stdout, clippy::print_stderr, clippy::dbg_macro)]`.
- Crate-local `clippy.toml` bans **concrete items**, by category: `std::fs`
  functions and `File`/`OpenOptions`/`DirBuilder`; the filesystem methods of
  `Path` (existence, metadata, symlink and directory reads); process control
  (`Command`, `Child`, `exit`, `abort`); threads (`sleep`, `spawn`, `scope`,
  `Builder`); environment and working-directory reads and writes
  (`std::env` variables, arguments, current and temp directory); clocks
  (`now` and `elapsed` on `SystemTime`/`Instant`); console I/O
  (`std::io::{stdin, stdout, stderr}`); TCP, UDP and Unix sockets; and a few
  tokio items. Clippy does not match module paths or globs (`std::fs`,
  `tokio::*`): an entry that does not resolve to a concrete item only prints a
  warning, which `-D warnings` does not make fatal, so every entry names an
  item and the CI clippy step fails on that warning. The tokio entries are
  inert while tokio is not a dependency.
- `scripts/check-app-deps.sh` is the real tokio guard: it fails when the
  crate's default tree (normal, build and dev edges) contains a UI toolkit
  crate (`tauri`, `wry`, `webkit2gtk`, `gtk`, `glib`, `iced`, `gpui`, `winit`
  and families) or an async runtime (`tokio`, `mio`, `async-std`, `smol`,
  `async-executor`, `futures-executor` and their families). CI runs it with
  `--selftest` in the `rust` job of `.github/workflows/lint.yml`.

## `ViewModel` contract

```rust,ignore
pub trait ViewModel {
    type Intent;
    type Effect;
    fn reduce(&mut self, intent: Self::Intent) -> Vec<Self::Effect>;
}
```

- `reduce` applies one intent and returns the effects to run, **in order**.
  An empty `Vec` does not allocate.
- Service results come back as intents (`ProfilesLoaded(Result<..>)`,
  `SaveFinished { id, result }`), never as callbacks.
- Effects are data (`Debug + Clone + PartialEq + Eq`), never closures or
  futures, so tests compare them exactly.
- State fields are private; the view reads selectors. Derived lists are
  cached inside `reduce`, so selectors are cheap enough to call every frame.

Bridge sketch (no bridge exists yet; screens are ported in M2). In iced,
`update` reduces the intent and batches one task per effect:

```rust,ignore
fn update(&mut self, message: Message) -> Task<Message> {
    let Message::Library(intent) = message;
    Task::batch(self.library.reduce(intent).into_iter().map(|effect| match effect {
        LibraryEffect::LoadProfiles => Task::perform(load_cards(self.app.clone()), |result| {
            Message::Library(LibraryIntent::ProfilesLoaded(result))
        }),
        LibraryEffect::SetFavorite { name, favorite } => { /* ProfileWriter, then FavoriteSaved { name, favorite, result } */ }
        LibraryEffect::RequestRunningSessions { token } => { /* snapshot, then RunningSnapshot { token, .. } */ }
    }))
}
```

In gpui the bridge calls `entity.update(cx, |vm, cx| vm.reduce(intent))` and
`cx.spawn`s one task per returned effect; each task updates the entity with
the result intent. Service calls map `AppError` into `ServiceError` (`From`,
`app` feature) before building the result intent.

## `Form<T>`

A screen describes its fields with `FormModel` and owns a `Form<T>`:

- `type Field`: one enum variant per field (`Copy + Ord`, so error and
  touched maps are deterministic). `FIELDS` lists them in display order.
- `field_id(Field) -> &'static str`: the backend id, exactly as
  `AppErrorKind::Validation { field }` reports it. The only place the string
  appears; ids must be unique (debug-asserted in `Form::new`).
- `field_changed(&self, other, field)`: per-field change detection. Dirty is
  **derived** (value vs baseline), so editing a field back to its original
  value makes it pristine again. Touched state and errors are stored.
- `validate()`: client-side syntactic checks, no I/O. Messages are
  `&'static str`, so user input never ends up in error text.

The form never emits effects. The screen calls `begin_submit()`, turns the
returned `(SubmitId, snapshot)` into its own save effect, and feeds the
result back through `finish_submit(id, result)`.

### Submit state machine

| From                         | Event                               | To                 |
| ---------------------------- | ----------------------------------- | ------------------ |
| `Idle`/`Succeeded`/`Failed`  | `begin_submit`, client-valid        | `Submitting(id)`   |
| `Idle`/`Succeeded`/`Failed`  | `begin_submit`, client-invalid      | `Failed`           |
| `Submitting(_)`              | `begin_submit`                      | unchanged (`None`) |
| `Submitting(id)`             | `finish_submit(id, Ok)`             | `Succeeded`        |
| `Submitting(id)`             | `finish_submit(id, Err(Cancelled))` | `Idle`             |
| `Submitting(id)`             | `finish_submit(id, Err(_))`         | `Failed`           |
| `Submitting(x)` or any other | `finish_submit(y != x, _)`          | unchanged          |
| `Succeeded`/`Failed`         | `edit`                              | `Idle`             |

- A client-invalid submit returns `None`, records the field errors and
  touches every field.
- Success makes the **submitted snapshot** the new baseline. Edits made while
  submitting are kept and stay dirty after the save.
- `edit` outside `Submitting` clears the edited field's error and the form
  error; other fields' errors stay.
- `SubmitId`s increase per form and survive `reset`, so a result of an attempt
  started before an external reload stays stale.
- `can_submit()` is `true` when nothing is in flight and some field is dirty.

### Error mapping

`apply_errors` (also used by `finish_submit`) maps one `ServiceError` and ends
any in-flight attempt:

| `ServiceErrorKind`                                      | Shown as                              | State    |
| ------------------------------------------------------- | ------------------------------------- | -------- |
| `Validation { field }`, `field` is a form field id      | that field's error; field touched     | `Failed` |
| `Validation { field }`, unknown id (e.g. `"settings"`)  | form error (banner), message verbatim | `Failed` |
| `NotFound`, `Conflict`, `Unavailable`, `Io`, `Internal` | form error (banner), message verbatim | `Failed` |
| `Cancelled`                                             | nothing; existing errors untouched    | `Idle`   |

Every non-cancelled error replaces the previous field and form errors.
Messages are already sanitized by `crosshook-app`; the crate never calls
`sanitize_message`. The `match` is exhaustive, so a new kind must decide where
it shows.

`show_error` shows an error from a call other than the submit (such as a
load) with the same mapping, but leaves the submit cycle alone: the submit
state, the submitted snapshot and the id are kept, so a save in flight still
finishes with its own result and a second `begin_submit` is still refused.
`Cancelled` shows nothing and changes nothing.

## Library example

`library::LibraryViewModel` ports the React library page (`LibraryPage.tsx`,
`useLibraryProfiles.ts`, `runningSessionsStore.ts`). The pipeline is
**search → filter → sort**; search and filter are independent row predicates
and sort always runs last.

- **Search**: the query is trimmed (`str::trim`) and lowercased; a card
  matches when its lowercase `name` or `game_name` contains it. Empty matches
  all. `str::trim` strips U+0085 and keeps U+FEFF, the reverse of JS `trim`;
  this only matters for queries made entirely of those characters.
- **Filters**: `All`; `Favorites`; `Installed` (Steam App ID non-empty and not
  `"0"`, meaning "has an art app id", not "installed on disk"); and
  `CurrentlyRunning` (exact, case-sensitive name with a live game session).
- **Sorts**: `Recent` keeps source order (the backend's name byte order);
  `Name` uses `select::name_order`. The TS keys `recentlyLaunched`,
  `lastPlayed` and `playtime` are dead in React and not ported.
- **Comparator**: key is `game_name` when non-empty, else `name`. Compare the
  lowercase key, then the key with ASCII case swapped (lowercase before
  uppercase), then `name` (unique, so the order is total and
  input-order-independent). It matches ICU `localeCompare` for ASCII letters,
  digits and spaces. Known divergences, not fixed (no ICU dependency): ASCII
  punctuation such as `_` sorts after digits (ICU: before); accented letters
  sort after `z` (ICU: next to their base letter). Neither sorts numbers
  numerically (`"Game 10"` before `"Game 2"`).
- **Selectors** (`select` module): `visible_indices` takes the lowercase keys
  from `search_keys(&cards)` (two per card: name, game name). Build them once
  per load, not per frame; a mismatched key slice panics in debug builds.
  `LibraryViewModel` does this itself and caches the visible indices.
- **Favorites** are optimistic: `ToggleFavorite` flips the card and emits
  `SetFavorite { name, favorite }`; the bridge echoes both in
  `FavoriteSaved { name, favorite, result }`. Per profile the view-model keeps
  the confirmed value (`Ok` confirms the echoed `favorite`; `Err` leaves it
  and sets `load_error`), the latest requested value and the number of saves
  in flight. While saves are in flight the card shows the latest request; once
  none are, it shows the confirmed value. A `ProfilesLoaded` taken while saves
  are in flight makes the loaded value the confirmed one of those profiles
  (what a failed save falls back to) but keeps their latest request on the
  cards; other cards take the loaded value. Replies may arrive in any order.
- **Currently Playing count**: distinct profile names with a live **game**
  session. Trainer and anonymous (unsaved) sessions do not count, and the
  count is not intersected with loaded profiles (React parity). The UI hides
  the badge at 0.
- **Session seq contract** (`RunningProfiles`, matching `crosshook-app`
  `services/launch/sessions.rs` and the `fetchToken` model of
  `runningSessionsStore.ts`): the bridge subscribes to session events first,
  then sends `Start`, which emits the first
  `RequestRunningSessions { token }`. The bridge never requests a snapshot on
  its own and answers each request with
  `RunningSnapshot { token, sessions, seq }` (`running_snapshot` supplies
  `sessions` and `seq`). Each request gets a
  new token and supersedes the previous one; only the reply carrying the
  latest token applies, so an older reply can never replace a newer one.
  Until that reply arrives (before the first snapshot, after `Resync`, after
  overflow), events are buffered, at most 1024; on overflow the buffer is
  dropped and one new request is emitted. The reply replaces the state and
  applies the buffered events with `seq > snapshot.seq`. Once synced, events
  with `seq <= snapshot.seq` are ignored; a duplicate start or an exit for an
  unknown id is a no-op. `Resync` emits a new `RequestRunningSessions`.

```rust
use crosshook_view_model::library::{
    LibraryEffect, LibraryFilter, LibraryIntent, LibrarySort, LibraryViewModel, ProfileCard,
    RunningSession, SessionEvent, SessionId, SessionKind,
};
use crosshook_view_model::ViewModel;

let card = |name: &str, game: &str| ProfileCard {
    name: name.into(),
    game_name: game.into(),
    steam_app_id: String::new(),
    is_favorite: false,
};
let mut vm = LibraryViewModel::default();
assert_eq!(vm.reduce(LibraryIntent::Refresh), [LibraryEffect::LoadProfiles]);
vm.reduce(LibraryIntent::ProfilesLoaded(Ok(vec![
    card("beta", "Zeta Quest"),
    card("alpha", ""),
])));
vm.reduce(LibraryIntent::SetSort(LibrarySort::Name));
assert_eq!(vm.visible_names().collect::<Vec<_>>(), ["alpha", "beta"]);

// Optimistic favorite: the card flips now; the bridge persists it and echoes
// the request back with the result.
assert_eq!(
    vm.reduce(LibraryIntent::ToggleFavorite("beta".into())),
    [LibraryEffect::SetFavorite { name: "beta".into(), favorite: true }]
);
vm.reduce(LibraryIntent::SetFilter(LibraryFilter::Favorites));
assert_eq!(vm.visible_names().collect::<Vec<_>>(), ["beta"]);
vm.reduce(LibraryIntent::FavoriteSaved {
    name: "beta".into(),
    favorite: true,
    result: Ok(()),
});

// Subscribed first, then `Start`: events are buffered until the reply to the
// latest request arrives.
assert_eq!(
    vm.reduce(LibraryIntent::Start),
    [LibraryEffect::RequestRunningSessions { token: 1 }]
);
vm.reduce(LibraryIntent::Session(SessionEvent::Started {
    session_id: SessionId::new("s2"),
    profile_name: Some("beta".into()),
    kind: SessionKind::Game,
    seq: 8,
}));
vm.reduce(LibraryIntent::RunningSnapshot {
    token: 1,
    sessions: vec![RunningSession {
        session_id: SessionId::new("s1"),
        profile_name: Some("alpha".into()),
        kind: SessionKind::Game,
    }],
    seq: 7,
});
assert_eq!(vm.currently_playing_count(), 2); // snapshot + buffered seq 8
assert_eq!(
    vm.reduce(LibraryIntent::Session(SessionEvent::Resync)),
    [LibraryEffect::RequestRunningSessions { token: 2 }]
);
```

With the `app` feature, `ProfileCard::from_summary(&summary, is_favorite)`,
`running_snapshot(&RunningSessions)` (the `sessions` and `seq` of
`RunningSnapshot`; the bridge adds the request's `token`) and
`session_event(&AppEvent)` convert `crosshook-app` types. `session_event` is a
function returning `Option` because `AppEvent` is `#[non_exhaustive]`; events
other than session start/exit and `Resync` map to `None`.

## Settings example

`settings::SettingsViewModel` drives a `Form<SettingsFields>` with one field
of each kind:

| Field                | Input                          | Errors from                                                                          |
| -------------------- | ------------------------------ | ------------------------------------------------------------------------------------ |
| `profiles_directory` | text; trimmed, empty = default | backend `Validation { field: "profiles_directory" }` from `SettingsService::load`    |
| `recent_files_limit` | numeric text                   | client check: integer in `1..=100` (`RECENT_FILES_LIMIT_MESSAGE`); core still clamps |
| `log_filter`         | select over `LogFilter`        | none (select; dirty when changed)                                                    |

`Submit` emits `SettingsEffect::Save { id, request }`, where
`SettingsSaveRequest` carries the parsed values; the bridge answers with
`SaveFinished { id, result }`. `Loaded(fields)` resets the form after an
external reload. `LoadFailed(error)` shows a failed load on the form with
`Form::show_error`, so a save in flight is not affected: `SettingsService::load`
reports a profiles directory it cannot resolve as
`Validation { field: "profiles_directory" }`, which lands on that field.
`LogFilter::parse` reads the stored `log_filter` string (trimmed;
the lowercase `as_str` names; empty means `Info`, the backend default). Other
backend values, such as directives like `crosshook=debug`, return `None`: the
example select cannot represent them.

```rust
use crosshook_view_model::settings::{
    LogFilter, SettingsEffect, SettingsField, SettingsFields, SettingsIntent, SettingsViewModel,
    RECENT_FILES_LIMIT_MESSAGE,
};
use crosshook_view_model::{ServiceError, SubmitState, ViewModel};

let mut vm = SettingsViewModel::new(SettingsFields {
    profiles_directory: String::new(),
    recent_files_limit: "10".into(),
    log_filter: LogFilter::Info,
});

// Client check fails: no effect, field error shown.
vm.reduce(SettingsIntent::EditRecentFilesLimit("500".into()));
assert!(vm.reduce(SettingsIntent::Submit).is_empty());
assert_eq!(
    vm.form().error(SettingsField::RecentFilesLimit),
    Some(RECENT_FILES_LIMIT_MESSAGE)
);

// Valid submit: one Save effect with the parsed request.
vm.reduce(SettingsIntent::EditRecentFilesLimit("25".into()));
vm.reduce(SettingsIntent::EditProfilesDirectory("  /games/profiles ".into()));
let effects = vm.reduce(SettingsIntent::Submit);
let SettingsEffect::Save { id, request } = &effects[0];
assert_eq!(request.profiles_directory, "/games/profiles");
assert_eq!(request.recent_files_limit, 25);

// The save succeeds: the submitted values become the new baseline.
vm.reduce(SettingsIntent::SaveFinished { id: *id, result: Ok(()) });
assert_eq!(vm.form().submit_state(), SubmitState::Succeeded);

// A later load cannot resolve the directory: the error lands on that field
// and the submit state is left alone.
vm.reduce(SettingsIntent::LoadFailed(ServiceError::validation(
    "profiles_directory",
    "Directory not found.",
)));
assert_eq!(vm.form().submit_state(), SubmitState::Succeeded);
assert_eq!(
    vm.form().error(SettingsField::ProfilesDirectory),
    Some("Directory not found.")
);
assert_eq!(LogFilter::parse(" debug "), Some(LogFilter::Debug));
assert_eq!(LogFilter::parse("crosshook=debug"), None);
```

## Launch and Hero Detail

Ported once from Classic TypeScript so every native screen shares one tested
copy. Classic keeps its TypeScript; the parity fixtures freeze the behaviour
both must match.

| Module                    | Classic source                                            |
| ------------------------- | --------------------------------------------------------- |
| `launch::dep_gate`        | `useLaunchDepGate.ts`, `LaunchDepGateModal.tsx`           |
| `launch::pipeline`        | `derivePipelineNodes.ts`                                  |
| `launch::pipeline_detail` | `derivePipelineDetail.ts`, `launchPreviewPresentation.ts` |
| `launch::validation_map`  | `mapValidationToNode.ts`                                  |
| `hero::model`             | `hero-detail-model.ts`, `AppShell.tsx` tab fallback       |

- **Inputs** are plain data (`launch::preview`): the subset of core
  `LaunchPreview` the derivations read, and `PipelineProfile` (tier-1 profile
  fields). With the `app` feature, `From<&core LaunchPreview>`,
  `PipelineProfile::from_profile`, `GateScope::from_profile` and the
  prefix-dependency conversions build them.
- **Pipeline** (`derive_pipeline`): tier 1 (no preview) checks only that the
  profile paths are set; tier 2 uses the preview, where only **fatal** issues
  mark a node as an error (warnings are not shown yet) and the launch node
  summarizes the nodes before it. A non-idle `LaunchPhase` overlays live
  status; an exited session is `Idle`. Labels are `PipelineNodeId::label`.
- **Dependency gate** (`DepGate`, a `ViewModel`): `RequestLaunch` checks the
  prefix repair status, then the required packages, and returns
  `GateEffect::Launch` when the launch may start. Otherwise it shows a busy
  prefix (retry), a required repair, or missing packages (install, skip or
  cancel; `auto_install` installs without asking). Every service effect
  carries an `OpId`; a new request, `Cancel` or a different profile or prefix
  (`ScopeChanged`) makes older ids stale, so late results never launch.
  Classic quirks kept on purpose: a failed dependency or repair-status lookup
  allows the launch unless this session already saw a repair requirement for
  the prefix (remembered per prefix, also across profile changes);
  `CheckFailed` and `UserSkipped` packages do not block; duplicate required
  packages stay duplicated.
- **Hero Detail** (`hero::model`): Classic shows all six tabs for every
  launch method, with or without a trainer; an unknown requested id opens
  `Overview`. `display_path` shows `Not set` for blank paths and
  `resolve_hero_image` picks custom background, SteamGridDB background,
  SteamGridDB hero, then Steam header image (a loading source shows a
  skeleton). DOM test ids are not ported.
- **Bridge contract**: subscribe to `prefix-dep-complete` before running
  `GateEffect::InstallDependencies` (the job can finish before the request
  returns), drop a gate's pending tasks with the gate, and keep one gate per
  launch panel across profile changes (`ScopeChanged`).

### Deviations from Classic and from the YAN-842 wording

- `GateIntent` / `ViewModel::reduce` instead of `GateEvent` / `apply`: the
  crate's `ViewModel` contract fixes the names.
- `RequestLaunch` is ignored while an install or a repair runs (Classic
  started a new check and dropped the running one's result). `Retry` keeps
  the busy modal up until its answer arrives and ignores a second press.
- A busy status after a repair or install shows the busy modal; Classic's
  repair path showed it too, through the modal's `in_progress` check.
- The gate reads `required_packages` and `auto_install` when a result
  arrives, so a same-profile settings change applies to a check in flight
  (Classic captured them when the check started).
- `HeroTab::from_id` accepts only the six ids; Classic's `in` check also
  matched object prototype keys such as `constructor`.
- `str::trim` strips U+0085 and keeps U+FEFF, the reverse of JS `trim`; this
  only matters for paths made of those characters.
- After a successful install the gate re-checks the profile's current
  packages before launching (Classic launched straight away), and never
  auto-installs a second time: packages still missing show the modal. A
  changed package list re-checks an open missing-packages prompt.
- `OpId`s carry a process-unique gate id (a static counter; the only shared
  state in the crate), so a reply meant for an earlier gate never matches a
  new one.
- The completion event carries no job id, so any completion for the same
  profile and prefix counts, as in Classic.

## Profile autosave

`profile::autosave::ProfileAutosave` ports Classic's per-section debounce,
flush, single-flight serialization, self-write suppression, and status chip
merging into one pure state machine:

| Classic source                                                     | Section / behaviour                                               |
| ------------------------------------------------------------------ | ----------------------------------------------------------------- |
| `useHeroProfilesAutosave.ts`, `useProfileCrud.ts`                  | `ProfileDraft` (350 ms, `FullProfile`, single-flight + revision)  |
| `useHeroLaunchHooksAutosave.ts`, `useHeroTrainerAutosave.ts`       | `LaunchHooks` and `Trainer` (350 ms, `FullProfile`)               |
| `useLaunchEnvironmentAutosave.ts`                                  | `Environment` (400 ms blur debounce, `FullProfile`)               |
| `useProfileLaunchAutosave.ts`, `useProfileLaunchAutosaveEffects.ts`| Five dedicated `SectionSaveKind` writers (`350 ms`, method-gated) |
| `useAutoSaveChip.ts`                                               | `combine_statuses` priority order + 3,000 ms `"Saved"` hold       |

- **Grouping**: dedicated launch sections (`LaunchOptimizations`,
  `Gamescope`, `TrainerGamescope`, `MangoHud`, `CommandArguments`) persist via
  `SaveTarget::Section(...)` so `ProfileWriter` keeps the cached health
  snapshot valid. Any due or flushed `FullProfile` edit (`ProfileDraft`,
  `LaunchHooks`, `Trainer`, `Environment`) persists as one atomic
  `SaveTarget::FullProfile` write covering all dirty eligible sections.
- **Bridge contract**: normalize drafts through `crosshook_core::profile::normalize_profile`
  before computing revision hashes, pass an injected monotonic clock
  (`AutosaveInstant`), drive at most one timer per editor matching
  `scheduled_tick()`, echo `SaveRequest.id` in `SaveSucceeded` / `SaveFailed`,
  and forward `ProfilesChanged` / `Resync` via `external_changes_from_app_event`
  (`app` feature). `FullProfile` edits update `latest_draft_revision` and
  `base_revision`; dedicated `Section` edits track section fingerprints
  independently in `section_saved_revs` without clobbering the full-profile
  baseline. To suppress self-write `ProfilesChanged` echoes that arrive before
  `SaveSucceeded` on dedicated `Section` saves, either tag `Edit` /
  `SaveRequest.revision` with the full-profile revision hash or use a unified
  revision scheme between `Edit` and `ExternalChange`.
- **Idle gate**: `Idle`, `Conflict`, and non-retryable or exhausted `Error`
  states never emit `AutosaveEffect::ScheduleTick`.

```rust
use crosshook_view_model::launch::LaunchMethod;
use crosshook_view_model::profile::{
    AutosaveEffect, AutosaveEvent, AutosaveInstant, AutosaveScope, AutosaveSection,
    AutosaveState, ProfileAutosave, RevisionHash,
};

let mut vm = ProfileAutosave::new(
    AutosaveScope::saved("Elden Ring", LaunchMethod::ProtonRun),
    Some(RevisionHash::new("rev-0")),
);
let t0 = AutosaveInstant::from_millis(0);
let effects = vm.apply(
    AutosaveEvent::Edit {
        section: AutosaveSection::ProfileDraft,
        revision: RevisionHash::new("rev-1"),
    },
    t0,
);
assert!(effects.contains(&AutosaveEffect::ScheduleTick(AutosaveInstant::from_millis(350))));

let save_effects = vm.apply(AutosaveEvent::Tick, AutosaveInstant::from_millis(350));
let AutosaveEffect::Save(req) = &save_effects[0] else {
    unreachable!("expected Save");
};
let _ = vm.apply(
    AutosaveEvent::SaveSucceeded {
        id: req.id,
        committed_rev: RevisionHash::new("rev-1"),
    },
    AutosaveInstant::from_millis(360),
);
let _ = vm.apply(AutosaveEvent::Tick, AutosaveInstant::from_millis(3_360));
assert_eq!(vm.state(), &AutosaveState::Idle);
assert_eq!(vm.scheduled_tick(), None);
```

### Deviations from Classic and from the YAN-850 wording

- **`AutosaveInstant` instead of `std::time::Instant`**: `clippy.toml` bans
  `Instant::now` and `Instant::elapsed` across `--all-targets`, so tests could
  not construct `std::time::Instant` values cleanly. `AutosaveInstant` wraps
  monotonic milliseconds with saturating `Duration` arithmetic.
- **`AutosaveIntent` + `ViewModel::reduce` alongside `apply`**: `ProfileAutosave`
  exposes `apply(&mut self, AutosaveEvent, AutosaveInstant) -> Vec<AutosaveEffect>`
  as requested and also implements `ViewModel<Intent = AutosaveIntent>`.
- **`max_wait` and `retry_backoff` defaults**: Classic does not cap continuous
  typing or auto-retry failed writes on a backoff timer. `AutosavePolicy::default`
  enables a 2,000 ms `max_wait` cap and `[1s, 2s, 4s, 8s]` backoff for
  retryable errors (`Io`, `Unavailable`, `Internal`); `AutosavePolicy::classic_parity`
  disables both for parity fixture replay.
- **Independent section deadlines**: Classic's `useProfileLaunchAutosaveEffects`
  shared `[profile]` across all five `useEffect`s, so editing MangoHud reset
  Gamescope's timer. `ProfileAutosave` tracks each section's deadline
  independently while preserving identical save-target grouping.
- **Process-unique `SaveId` mount**: `SaveId` adds an atomic `mount` counter
  alongside `OpId` so replies from an earlier instance never match a new one.

### Parity fixtures

`src/crosshook-native/src/utils/__tests__/exportParityFixtures.test.ts` and
`exportAutosaveParityFixtures.test.ts` run the real TypeScript functions and
Classic hooks on fixed inputs (every Vitest case of these modules plus a state
matrix: idle, validating, blocked, launching, waiting for the trainer, running,
exited, trainer attached, each method, and autosave debounce/grouping/chip
scenarios) and write
`tests/fixtures/ts_parity/{pipeline,pipeline_detail,validation_map,hero,autosave}.json`.
The Rust tests (`src/launch/parity_tests.rs`, `src/hero/model/tests.rs`,
`src/profile/autosave/parity_tests.rs`) load them with `include_str!` and
assert equal output, so CI needs no Node.

- `./scripts/export-ts-parity-fixtures.sh` rewrites the files; then
  `git diff --exit-code` on that directory proves the export is reproducible.
- Plain `npm test` runs the same file in compare mode and fails when the
  TypeScript output no longer matches the committed JSON.
- The dependency gate's Vitest cases are async React-hook scenarios, not
  input/output pairs, so each one is a named intent/effect script in
  `src/launch/dep_gate/tests.rs` (`vitest_*`) instead of a `dep_gate.json`
  fixture. Unmount cases become "a remounted gate ignores results it did not
  request"; subscription races become stale `OpId`s.
- The hero image fixture types carry the TypeScript camelCase names through
  `cfg_attr(test, serde(rename = ..))`, which compiles away outside tests.
- `serde`/`serde_json` are **dev-dependencies** only; the fixture types derive
  `Deserialize` under `cfg(test)`.

## Adding a view-model for a ported screen

1. Read the React oracle (component, hooks, stores) and list its user actions,
   service calls and derived state. Port only what the UI exposes; no dead
   keys.
2. Add `src/<screen>.rs` with tests in `src/<screen>/tests.rs`, and
   `pub mod <screen>;` in `lib.rs`. Document every public item
   (`missing_docs` is denied).
3. Define the input types the screen needs in the crate (as `ProfileCard` and
   `RunningSession` do) instead of using `crosshook-core`/`crosshook-app`
   types, so the default build stays tokio-free.
4. Define `<Screen>Intent` (user actions, plus one variant per service result
   carrying `Result<_, ServiceError>`) and `<Screen>Effect` (service calls as
   data, deriving `Debug, Clone, PartialEq, Eq`). Pass time, ids and any other
   outside input inside intents.
5. Keep state fields private and expose selectors. Cache derived lists inside
   `reduce` when they change, not in the selectors.
6. For editable data, implement `FormModel` with a `Field` enum whose
   `field_id` values match the backend's `AppErrorKind::Validation` ids, and
   drive it with `Form` (`begin_submit` → save effect → `finish_submit`).
7. `impl ViewModel`. Match intents and error kinds exhaustively.
8. Put conversions from `crosshook-app`/`crosshook-core` types in
   `src/<screen>/app.rs` behind `#[cfg(feature = "app")]`. Use exhaustive
   matches (drift guards); for `#[non_exhaustive]` enums, write a function
   that returns `Option`.
9. Test with `testing::Script` and `testing::fixtures`; add proptest
   properties for selector invariants. Run the commands under _Testing_ and
   `./scripts/check-app-deps.sh`.

## Testing

- **`testing::Script`** runs intents one step at a time and asserts each
  step's effects exactly and in order (the panic names the 1-based step):

  ```rust,ignore
  Script::new(LibraryViewModel::default())
      .step(LibraryIntent::Refresh, [LibraryEffect::LoadProfiles])
      .step(LibraryIntent::ProfilesLoaded(Ok(cards)), [])
      .assert_state(|vm| assert_eq!(vm.visible().count(), 2))
      .into_inner();
  ```

- **`testing::fixtures`**: `card("alpha").game_name("Alpha").steam_app_id("123").favorite().build()`,
  `started(id, profile, kind, seq)`, `exited(id, seq)`,
  `running(id, profile, kind)` and `synced(sessions, seq)` (a
  `RunningProfiles` whose first request is answered).
- The `testing` module is compiled for this crate's tests and, with the
  **`testing` feature**, for other crates' tests (for example future
  `crosshook-ui` screen tests). It adds no dependencies.
- **proptest** (dev-dependency, `default-features = false, features = ["std"]`
  to keep the dev tree tokio-free) covers the library in
  `src/library/proptests.rs`: search and filter are sound and complete,
  `Recent` keeps source order and `Name` is a sorted permutation, longer
  queries never show more cards, `Name` order ignores input order, stale events
  never change the running set, and the Currently Playing count matches a
  reference model. Strategies stay in `#[cfg(test)]` files; regressions are
  kept in `proptest-regressions/`.
- Tests in `src/error/tests.rs`, `src/library/app/tests.rs`,
  `src/launch/app/tests.rs`, and `src/profile/autosave/app/tests.rs` need the
  `app` feature, so run both feature sets:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-view-model
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-view-model --all-features
cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-view-model --all-features --all-targets -- -D warnings
./scripts/check-app-deps.sh
```

Workspace clippy and tests build default features only; CI adds the
`--all-features` clippy and test steps so the `app` code is linted and tested.

## Storage boundary

- **Runtime-only:** all view-model state (cards, search keys, query, filter,
  sort, visible indices, pending favorite saves, running sessions, the event
  buffer, the dependency gate and its remembered repairs, profile autosave
  deadlines/in-flight ids/self-write tokens/conflict state, form values, baselines, touched fields, errors and submit ids). It is
  rebuilt from service results on each start and never persisted.
- **No persistence here:** the crate reads and writes no file, database or
  setting. Persistent changes (favorites, settings) are effects; the bridge
  runs them through `crosshook-app` services, whose storage is described in
  [`crosshook-app/README.md`](../crosshook-app/README.md#storage-boundary).

## Dependencies

- **Default build: none.** Dev: `proptest`, `serde`, `serde_json` (parity
  fixtures). `cargo tree -p crosshook-view-model`
  contains no tokio and no UI toolkit; `check-app-deps.sh` guards that.
- **`app` feature** (off by default): adds optional `crosshook-app` and
  `crosshook-core` and the conversions (`From<&AppError>`/`From<AppError>` for
  `ServiceError`, `ProfileCard::from_summary`, `From` for `SessionId`,
  `SessionKind` and `RunningSession`, `running_snapshot`, `session_event`, and
  the launch conversions listed in § _Launch and Hero Detail_).
  It is off because both crates depend on tokio, which would put tokio back in
  the tree. There is no self dev-dependency with features for the same reason.
  A bridge crate (which already depends on `crosshook-app`) enables it.
- **`testing` feature**: exposes `testing` to other crates; no dependencies.
- **Version:** `version.workspace = true`. The crate is deliberately not listed
  in `scripts/prepare-release.sh` or the `release.yml` version check: both look
  for a literal `version = "X"` line, which an inherited version does not have.
  The workspace version bump already covers it.
