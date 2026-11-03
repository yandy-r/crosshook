# YAN-794 parity research: React oracle for `crosshook-view-model`

Paths are relative to `src/crosshook-native/` unless marked otherwise. Line numbers are from the
`crosshook-yan-794` worktree as of 2026-11-03.

## 1. Library pipeline: search, filter and sort

### 1.1 Data source and input order

- `useLibrarySummaries` calls `profile_list_summaries`, then maps each `ProfileSummary` to a
  `LibraryCardData` with `isFavorite: favoriteSet.has(s.name)`
  (`src/hooks/useLibrarySummaries.ts:36-58`). When `favoriteProfiles` changes, it sets the
  favourite flag again without a network call (`:84-88`). A `profiles-changed` event refetches,
  except for `favorite_toggled` (`:65-82`).
- **Backend order is the "natural" order of the library.** `ProfileStore::list` sorts the file
  stems with `names.sort_unstable()`, which is byte order on `String`
  (`crates/crosshook-core/src/profile/toml_store/store.rs:134-153`). `build_summaries` keeps that
  order and skips profiles that fail to load (`crates/crosshook-app/src/services/profiles/reads.rs:90-119`).
  The browser mock uses `Map` insertion order instead (`src/lib/mocks/handlers/profile-core.ts:70-91`).
- `steamAppId` is `resolve_art_app_id(effective)`, which is the trimmed `steam.app_id` or, if that
  is empty, the trimmed `runtime.steam_app_id` (`reads.rs:121-134`,
  `crates/crosshook-core/src/profile/models/resolve.rs:45-51`). It is therefore always trimmed and
  may be `""`.
- DTO: `crates/crosshook-app/src/services/profiles/types.rs:14-30` (`name`, `game_name`,
  `steam_app_id`, `custom_cover_art_path`, `custom_portrait_art_path`, `network_isolation`).
  `ProfileSummary` does not derive `PartialEq`. The TS mirror is `src/types/library.ts:7-19`. The
  card adds `isFavorite`, and the value equality used by the inspector is
  `libraryCardDataEqual` (`library.ts:22-32`).
- Optimistic favourite toggle: `LibraryPage.tsx:279-290` flips `isFavorite` locally, calls
  `toggleFavorite`, and puts the old value back if the call fails. `toggleFavorite` then reloads
  `profile_list_favorites` (`src/hooks/profile/useProfileCrud.ts:96-116`).

### 1.2 Pipeline order (`src/components/pages/LibraryPage.tsx`)

```
searched  = useLibraryProfiles(summaries, searchQuery)          // :124
displayed = filter(searched, filterKey) then sort(sortBy)       // :126-145
```

1. **Search** (`src/hooks/useLibraryProfiles.ts:4-10`):
   - `query = searchQuery.trim().toLowerCase()`.
   - If the query is empty, the input array itself is returned (identity, same reference).
   - Otherwise the result keeps rows where `p.name.toLowerCase().includes(query)` **or**
     `p.gameName.toLowerCase().includes(query)`. Each field is matched on its own: there is no
     concatenation, no tokenising and no fuzzy matching. `steamAppId` is **not** searched. The
     field values are not trimmed. Input order is kept.
2. **Filter** (`LibraryPage.tsx:127-140`). It starts from a copy (`[...searched]`), so the input is
   never mutated.
   | key | predicate | in toolbar? |
   | ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
   | `all` | none (falls to `default`) | yes (`LibraryToolbar.tsx:10`) |
   | `favorites` | `p.isFavorite` | yes |
   | `installed` | `Boolean(p.steamAppId && p.steamAppId !== '0')`. This means "has a Steam/art app id", not "installed". `"00"` and `"0 "` pass (the backend trims, so `"0 "` never occurs). | yes |
   | `recentlyLaunched` | **none: falls to `default`, same as `all`** | **no** (`LibraryToolbar.tsx:3` comment: kept in the union only for persisted state) |
   | `currentlyRunning` | `runningProfiles.has(p.name)`. The match is exact and case-sensitive. | yes, labelled "Running" |
3. **Sort** (`LibraryPage.tsx:141-143`):
   - Only `name` does anything: `list.sort((a,b) => (a.gameName || a.name).localeCompare(b.gameName || b.name))`.
   - `recent` (the **default**, `LibraryPage.tsx:76`), `lastPlayed` and `playtime` are **no-ops**,
     so the result stays in backend order (profile file stem, byte order). Only `Recent` and
     `Name` appear in the toolbar (`LibraryToolbar.tsx:4-7`). `lastPlayed` and `playtime` cannot be
     reached from the UI. `recentlyLaunched`, `lastPlayed` and `playtime` appear nowhere else in
     `src/`. The only hits are in `types/library.ts:3,5`.
   - Display-key fallback: `gameName || name`. JS `||` falls back only when `gameName === ''`. A
     whitespace-only `gameName` (`" "`) is truthy and **is** used as the key. The backend does not
     trim `game.name` (`reads.rs:128`).
   - Stability: `Array.prototype.sort` is stable (ES2019+). Rows whose keys compare equal
     (`localeCompare === 0`) keep their input order, which is backend name order. A JS check
     confirmed that `"e\u0301".localeCompare("\u00e9") === 0`.
   - Collation: `localeCompare` with no locale uses the ICU default locale (en-US in the Node
     check). It is case-insensitive at the primary level and puts lowercase before uppercase at the
     tertiary level. It is accent-aware and not numeric. The table below shows the observed order
     for a set of sample keys.

     | `localeCompare` (ICU)              | Rust `str` `Ord` (bytes)               | Rust `to_lowercase()` key, stable sort |
     | ---------------------------------- | -------------------------------------- | -------------------------------------- |
     | `" space"`                         | `" space"`                             | `" space"`                             |
     | `"_under"`                         | `"10 Games"`                           | `"10 Games"`                           |
     | `"10 Games"`                       | `"2 Games"`                            | `"2 Games"`                            |
     | `"2 Games"`                        | `"Alpha"`                              | `"_under"`                             |
     | `"alpha"`                          | `"Game 10"`                            | `"Alpha"` (input order)                |
     | `"Alpha"`                          | `"Game 2"`                             | `"alpha"`                              |
     | `"b"`, `"beta"`                    | `"Zed"`                                | `"b"`, `"beta"`                        |
     | `"eclair"`, `"éclair"`, `"Éclair"` | `"_under"`, `"alpha"`, `"b"`, `"beta"` | `"eclair"`                             |
     | `"Game 10"`, `"Game 2"`            | `"eclair"`, `"Éclair"`, `"éclair"`     | `"Game 10"`, `"Game 2"`, `"Zed"`       |
     | `"Zed"`                            |                                        | `"Éclair"`, `"éclair"` (after `z`)     |

     **Parity recommendation.** Do not add ICU (`icu_collator` pulls in a large data crate). Use a
     stable `sort_by` (`slice::sort_by` is stable) with this comparator:
     1. Compare `key.to_lowercase()`.
     2. On a tie, apply a "lowercase-first" tertiary rule. One cheap way is to byte-compare the
        case-swapped keys: `"alpha"`→`"ALPHA"` sorts before `"Alpha"`→`"aLPHA"`.
     3. Otherwise keep input order.

     This matches ICU exactly for ASCII letters, spaces and digits within letters, which covers
     every fixture. Document these known deviations in the selector rustdoc:
     - ASCII punctuation such as `_` sorts after digits in Rust but before them in ICU.
     - Accented letters sort after `z` in Rust.
     - Numbers are not compared numerically in either (`"Game 10"` sorts before `"Game 2"`).

     Property tests must not assert ICU order. Golden parity tests should use ASCII
     letter, space and digit names only.

4. **Search ⟂ filter.** Both are pure row predicates, so they commute. Sort always runs last.

### 1.3 JS vs Rust string primitives (checked with `node` and `rustc`)

- `trim`: JS `String.prototype.trim` strips U+FEFF but not U+0085. Rust `str::trim` (Unicode
  `White_Space`) strips U+0085 but not U+FEFF. Both strip U+00A0, U+2028 and U+3000. Neither
  strips U+180E or U+200B. For exact parity, use `trim_matches(|c| c.is_whitespace() || c ==
'\u{FEFF}') ` and leave U+0085 alone, or accept and document the difference. It only affects
  queries made entirely of exotic whitespace.
- `toLowerCase` and `to_lowercase` use the same full Unicode mapping, including final sigma
  (`"ΑΣ"` → `"ας"` in both) and `"İ"` → `"i̇"` (2 code points in both). `includes` and `contains`
  give the same results on valid strings.

### 1.4 Currently Playing badge and sidebar

- `src/components/layout/AppShell.tsx:93-97`: `runningProfiles = useRunningProfiles()`. The badge
  is `{ currentlyRunning: runningProfiles.size }` when `size > 0`, otherwise `undefined`, which
  **hides the badge**; it never shows "0". AppShell provides no favourites badge.
- `src/components/layout/Sidebar.tsx:77` defines the entry "Currently Playing" with
  `filterKey: 'currentlyRunning'`. `:234` resolves the badge as
  `libraryFilterBadges?.[item.filterKey] ?? item.badge`. `:155` renders it only when it is not
  `undefined`. `:137-147` sets `aria-pressed` when `activeRoute === 'library' &&
activeLibraryFilter === filterKey`, and the click calls `onNavigate('library', { libraryFilter })`.
  `LibraryPage` applies the intent with `setFilterKey(intent.filterKey)` and switches to library
  mode (`LibraryPage.tsx:111-118`). It reports filter changes back through
  `onLibraryFilterChange` (`:120-122`).
- `src/hooks/useRunningProfiles.ts` returns `useRunningGameProfileNames()`.
- **Count semantics (confirmed):** the count is the number of **distinct non-null
  `profile_name`s over sessions with `session_kind === 'game'`**
  (`runningSessionsStore.ts:59-70` `deriveGameProfileNames`). Because it is a `Set`, two game
  sessions of the same profile count once. The registry does not stop duplicates:
  `register(Game, key)` always inserts, and `sessions_for_profile` returns many
  (`crates/crosshook-core/src/launch/session/registry.rs:83-95,176-191`). Anonymous sessions
  (`profile_name: null`) and trainer sessions are excluded.
- **Not intersected with the saved or summary list.** "Saved" holds only because a non-null
  `profile_name` means the session was launched from a saved profile
  (`crates/crosshook-app/src/services/launch/sessions.rs:94-98`, using the anonymous sentinel
  `__crosshook_anonymous_profile__` from `crates/crosshook-core/src/launch/session/keys.rs:14`). If
  a running profile is deleted or renamed, it still counts in the badge but disappears from the
  `currentlyRunning` filtered list, because that list intersects with the summaries.
  - For parity, use `badge = |running_game_names|` and `filtered list ⊆ summaries`.
  - The issue's wording "|running ∩ saved|" is a stricter variant. **Decision for Main:** keep
    React parity, which I recommend, or intersect with the known summaries and note it as a
    deliberate change.

## 2. Session events: `runningSessionsStore.ts`, `events.rs` and `sessions.rs`

### 2.1 Backend contract

- The registry increments `seq` and calls the observer **under the same lock** as the mutation.
  The observer therefore sees changes in strict `seq` order. `snapshot()` returns
  `(sessions oldest-first, seq)` read under one lock. A change with `seq <= S` is already in a
  snapshot taken at `S`; a change with `seq > S` is not
  (`registry.rs:23-30,69-77,129-139`).
- Only real insertions and removals increment `seq`. Removing an unknown or already-removed id is
  a silent no-op with no event (`registry.rs:103-127`). Each session gets **exactly one** `Exited`
  (`events.rs` `SessionExited` doc).
- `AppEvent::SessionStarted { session_id, profile_name: Option<String> (trimmed), kind, seq }`
  and `AppEvent::SessionExited { …, code, signal, seq }` are defined in
  `crates/crosshook-app/src/events.rs` (the enum is at `:58-145`). The enum is `#[non_exhaustive]`.
- `AppEvent::Resync { missed }` is produced by `EventReceiver::recv` when the broadcast receiver
  lags (`events.rs:200-210`). The Tauri bridge maps it to `{kind:'resync'}`
  (`src-tauri/src/session_bridge.rs:39-91`).
- `LaunchService::running_sessions_snapshot() -> RunningSessions { sessions:
Vec<RunningSession{session_id, profile_name, kind}>, seq: u64 }`
  (`crates/crosshook-app/src/services/launch/sessions.rs:20-50`). The protocol, as documented at
  `sessions.rs:1-9`, is: subscribe first, then snapshot, then apply only events with
  `seq > snapshot.seq`.
- The Rust reference model is the `apply_after` test helper
  (`crates/crosshook-app/src/services/launch/tests/sessions.rs:65-89`). It filters
  `seq > snapshot_seq`, inserts on start and removes on exit, and **asserts** on a double start
  or an unknown exit, because those would be backend bugs. Related tests in the same file:
  - `a_snapshot_racing_an_in_flight_change_matches_the_event_stream` (`:319`)
  - `concurrent_changes_reach_the_bus_gap_free_and_consistent_with_snapshots` (`:385`)
  - `a_lagging_subscriber_gets_resync_and_recovers_from_the_snapshot` (`:438`, bus capacity 2, 5
    registrations, so `Resync{missed:3}` followed by a tail that the re-snapshot already covers)

### 2.2 React store semantics (`src/hooks/runningSessionsStore.ts`)

The rules are summarised in the docblock at `:13-30`.

1. `start()` (`:180-206`) awaits `subscribeEvent` **before** calling `requestSnapshot()`.
2. While a snapshot is pending, `baselineSeq === null` and every started or exited event is pushed
   to `buffered` (`:166-173`).
3. On snapshot success (`:126-156`):
   - Build a map keyed by `session_id`.
   - Apply buffered events **in arrival order** whose `event.seq > snapshot.seq`.
   - Clear the buffer, set `baselineSeq = snapshot.seq`, and commit.
   - This **replaces** the old state rather than merging into it.
4. After the baseline is set, any event with `seq <= baselineSeq` is dropped (`:174-177`).
   **`baselineSeq` is not moved forward as events apply.** After the snapshot, duplicates are
   handled only by `applyEvent` being idempotent (`:101-124`):
   - `started` for an id that already exists returns the same map (no-op).
   - `exited` for an unknown id is a no-op.
   - `seq` values are compared only against the snapshot and are never assumed contiguous
     (`:19-20`).
5. **Exited before started:** the exit is ignored for the running set because the id is unknown.
   A later `started` for that id **would add it**, giving a ghost session. The store does not use
   its exited-id memory to suppress that. With the backend contract (bus order = seq order) this
   cannot happen inside one stream. Across a snapshot boundary it is handled by the seq filter.
   `recordExit` keeps a bounded (64) FIFO of exited ids (`:37-38,87-99`), but that list exists
   only for `useSessionExited(sessionId)` (`:291-298`), so a launch can detect an exit that
   arrived before it knew its own id. It does not affect the count.
6. `resync`, or a payload that fails `parseLaunchSessionChanged` (`src/lib/launchSessionEvents.ts:35-64`),
   calls `requestSnapshot()` (`:158-163`), which resets `baselineSeq = null` so buffering starts
   again. `fetchToken` makes sure only the latest in-flight snapshot is applied (`:126-133`), and
   `lifecycle` discards results from a torn-down subscription.
7. **Failed or malformed snapshot:** the previous state is kept, buffering continues, and
   `snapshotRetryPending = true`. The retry fires on the **next received event** (`:170-172`), on
   window `focus` or `visibilitychange`, or when a new consumer subscribes (`:208-238`). No timers
   are used.
8. Commit derives `gameProfileNames` and **reuses the previous Set when its members are
   unchanged** (`:59-70,78-85`), so consumers do not re-render needlessly. The view model
   equivalent is: do not emit a change when the derived set is equal.
9. Validation (`launchSessionEvents.ts:10-33`):
   - `seq` must be a safe integer ≥ 0.
   - `session_id` must be a non-empty string.
   - `profile_name` must be a string or `null`.
   - `session_kind` must be `game` or `trainer`.
   - `code` and `signal` must be integers or `null`.
   - Anything else counts as a resync. In Rust, typed `AppEvent`s make most of this unnecessary.

### 2.3 Suggested view-model mapping

- Intents:
  - `SessionStarted{id, profile_name, kind, seq}`
  - `SessionExited{…, seq}`
  - `Resync`
  - `SnapshotLoaded{token, sessions, seq}`
  - `SnapshotFailed{token}`
  - `Recover` (the UI bridge sends this on window focus)
- Effects: `FetchRunningSessions{token}`. Emit it on init, on `Resync`, and on the first event
  after a failure.
- Keep the buffer, baseline and token rules exactly as above. Tolerate double starts and unknown
  exits without panicking.

## 3. Existing tests: the parity oracle

| File                                                              | Test (verbatim name)                                                                                                             | Covers                                                                                                                                                      |
| ----------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/pages/__tests__/LibraryPage.test.tsx:145`         | `activates the Name sort chip when clicked`                                                                                      | sort key state (not the order)                                                                                                                              |
| `…LibraryPage.test.tsx:208`                                       | `preserves library search when returning from hero detail`                                                                       | the search query survives a detail round trip                                                                                                               |
| `…LibraryPage.test.tsx:240`                                       | `applies incoming favorite filter intent to the toolbar`                                                                         | filter intent → `filterKey`                                                                                                                                 |
| `…LibraryPage.test.tsx:248`                                       | `filters to running profiles from the launch-session snapshot`                                                                   | snapshot `{sessions:[alpha game], seq:1}` with `currentlyRunning`: Alpha shown, `Dev Game Beta` hidden                                                      |
| `…LibraryPage.test.tsx:433`                                       | `refreshes library cards when profiles-changed fires`                                                                            | summaries refetch                                                                                                                                           |
| `src/components/library/__tests__/LibraryToolbar.test.tsx:18`     | `emits sort and filter changes from chip presses`                                                                                | `name`, `favorites`, `currentlyRunning` ("Running")                                                                                                         |
| `…LibraryToolbar.test.tsx:35`                                     | `reflects aria-pressed for active sort and filter chips`                                                                         |                                                                                                                                                             |
| `…LibraryToolbar.test.tsx:43`                                     | `reflects aria-pressed for the running filter chip`                                                                              |                                                                                                                                                             |
| `src/components/layout/__tests__/Sidebar.test.tsx:55`             | `renders fixed library-filter entries with optional badge text`                                                                  | badge rendering `{favorites:2,currentlyRunning:1}`                                                                                                          |
| `src/components/layout/__tests__/AppShell.test.tsx:128`           | `opens library with sidebar quick-filter intent`                                                                                 | Favorites/All/Currently Playing → toolbar chip `aria-pressed`                                                                                               |
| `tests/smoke.spec.ts:234` (Playwright)                            | `Favorites and Currently Playing activate matching library chips`                                                                | badge accessible name `"Currently Playing 1"` (`:259`)                                                                                                      |
| `src/hooks/__tests__/runningSessionsStore.test.ts:82`             | `awaits the event subscription before requesting the snapshot`                                                                   | subscribe → snapshot order                                                                                                                                  |
| `…runningSessionsStore.test.ts:102`                               | `applies buffered events only past the snapshot seq and drops stale ones afterwards`                                             | buffered: `started c seq3`, `started b seq5`; snapshot `{a}, seq4` → `{Alpha, Beta}`. Then `started c seq4` is dropped and `exited a seq6` gives `{Beta}`.  |
| `…runningSessionsStore.test.ts:125`                               | `re-fetches and replaces state on resync; a failed snapshot keeps state and retries on the next event`                           | resync replaces state. A failed snapshot keeps it. The next event (`started c seq10`) triggers a retry, and the snapshot `{b}, seq9` gives `{Beta, Gamma}`. |
| `…runningSessionsStore.test.ts:158` / `:182`                      | `discards a subscription that resolves after its consumer unmounted and remounted` / `discards a snapshot that resolves after …` | lifecycle and token staleness (a stale seq 5 must not filter a live seq 3)                                                                                  |
| `…runningSessionsStore.test.ts:200`                               | `excludes anonymous and trainer sessions from the running profile names`                                                         | sessions size 3, names `{Beta}`                                                                                                                             |
| `…runningSessionsStore.test.ts:215`                               | `shares one subscription between consumers and unsubscribes after the last unmounts`                                             | ref counting (bridge concern)                                                                                                                               |
| `…runningSessionsStore.test.ts:229`                               | `schedules no timers`                                                                                                            |                                                                                                                                                             |
| `…runningSessionsStore.test.ts:259`                               | `retries a failed snapshot on window focus and document visibility, then removes its listeners`                                  | recovery triggers; a healthy store does not refetch on focus                                                                                                |
| `…runningSessionsStore.test.ts:309`                               | `retries a failed subscription on focus and on a new consumer, then snapshots`                                                   |                                                                                                                                                             |
| `src/hooks/__tests__/profilesChangedListeners.test.ts:85` / `:92` | `does not refetch summaries for favorite_toggled` / `refetches summaries for saved (positive control)`                           | summaries refresh rule                                                                                                                                      |

**Gap:** no Vitest test covers the search predicate, the `installed` predicate, name-sort order,
the `gameName || name` fallback, or the no-op sort keys. The Rust tests will be the first to pin
them down. Use section 1 as the specification.

## 4. Settings form oracle

- **There is no form abstraction in React.** Nothing in `src/` tracks a "touched" state. The only
  matches are unrelated: `useFocusTrap.ts:176-250`, a comment in `useProfileCrud.ts:186`, and
  `types/profile.ts:443`.
- **Numeric settings fields (best Rust example):**
  - `recent_files_limit` is in `src/components/settings/LoggingAndUiSection.tsx:61-80`.
    `config_history.max_revisions` is in `src/components/settings/ProfilesSection.tsx:90-113`.
  - Both are uncontrolled `<input type="number" min max defaultValue key=…>`. The input remounts
    when the persisted value changes.
  - On **blur** (`:73-79`):
    - `parseInt(value, 10)`. Not finite → **return silently**: no error, and the input keeps the
      bad text.
    - `v = clamp(raw, 1, 100)` (or `5, 100`).
    - Persist only if `v !== current`, so the dirty check compares against the persisted value.
  - The backend clamps again in `normalize`:
    - `crates/crosshook-app/src/services/settings.rs:193-197` calls `clamp_recent_files_limit` and
      `clamp_config_history_max_revisions`.
    - The constants are `RECENT_FILES_LIMIT_MIN/MAX = 1/100` (default 10) and
      `CONFIG_HISTORY_MAX_REVISIONS_MIN/MAX = 5/100` (default 20), in
      `crates/crosshook-core/src/settings/types.rs:10-13,27-29,36-38,40-47,70-75`.
    - Rust tests: `merge_normalizes_limits_and_log_filter` and
      `update_normalizes_and_preserves_unknown_keys` (`crates/crosshook-app/src/services/settings/tests.rs:228,384`),
      and `config_history_settings_default_and_clamp` (`crates/crosshook-core/src/settings/tests.rs:612`).
  - **Out-of-range values never produce `AppError::Validation`; they are clamped.**
- **Backend validation fields for settings:**
  - `Validation { field: "profiles_directory" }` on load (`settings.rs:240-245`).
  - `Validation { field: "settings" }` for a TOML parse failure
    (`crates/crosshook-app/src/error.rs:170-182`).
  - `"recent_files"` (`error.rs:186-198`).
  - `"ui"` appears only in `services/settings/ui_writer_tests.rs:51,63`.
  - The `settings_save` IPC command flattens `AppError` to `error.message()`
    (`src-tauri/src/commands/settings.rs:15-17,144-154`), so React **never sees the field**.
- **Error display:**
  - The page-level banner `settingsError` (`src/components/pages/SettingsPage.tsx:41-45`) is set
    only by `handleAutoLoadChange` and `handleSteamGridDbApiKeyChange`
    (`src/context/PreferencesContext.tsx:162-192`).
  - `persistSettings` itself does not catch errors (`:150-160`). Sections call
    `void onPersistSettings(…)`, so a save failure from a numeric field is an unhandled rejection
    with no UI.
- **Submit state machine example:** `src/components/settings/SteamGridDbSection.tsx:11-55,100-136`.
  - Local state: `localKey`, `isSaving`, `saveError`, `saved`, `lastAction`.
  - Submit: `isSaving = true`, clear the error and `saved`, then `await`.
    - On success: clear the input, `saved = true`, `lastAction = 'save' | 'clear'`.
    - On failure: `saveError = message`.
    - In all cases: `isSaving = false`.
  - The Save button is disabled while `isSaving` or when `localKey.trim()` is empty.
  - Typing clears `saved`.
  - The error appears under the field.
- **Per-field backend error mapping (the only real precedent):**
  - `RunCommandError { kind:'validation', variant, field, message } | { kind:'runtime', message }`
    (`src-tauri/src/commands/run_executable.rs:20-44`; TS type in `src/types/run-executable.ts:93-117`).
  - `useRunExecutable.applyCommandError` (`src/hooks/useRunExecutable.ts:188-205`):
    - **validation** → `fieldErrors[field] = message`, `generalError = null`, other field errors
      kept.
    - **any other kind** → `fieldErrors = {}`, `generalError = message`.
  - `updateField` sets the value and **clears only that field's error** (`:207-225`).
  - `startRun` clears all validation and moves to stage `'preparing'` (`:247-272`).
  - Stages: `'idle'|'preparing'|'running'|'busy'|'complete'|'failed'` (`types/run-executable.ts:110`).
  - `useInstallGame.ts:132-141` and `useUpdateGame.ts:176-197` follow the same pattern.
- **Dirty and saving in the profile editor:**
  - `useProfileCrud` has `dirty`, `saving` and `error` (`src/hooks/profile/useProfileCrud.ts:58-64`).
  - Any edit sets `dirty = true` (`:280-288`).
  - Client validation runs before saving (`validateProfileForSave`,
    `src/hooks/profile/profileValidation.ts:3-9`). Then `saving = true`, `error = null`, and
    finally `saving = false`; on failure `error = message` (`:290-337`).
  - The autosave chip maps `saving`, `error` and `dirty` to a status
    (`src/components/library/profiles/useHeroProfilesAutosave.ts:64-78`).

**Recommended worked example:** a `RecentFilesLimit` / `ConfigHistoryRetention` settings form with
two `u32` fields.

- Pristine baseline = the loaded `AppSettingsData` values.
- Dirty = the parsed value differs from the baseline.
- Touched = a `Blur` intent was received. This is new semantics with no React oracle; document it.
- Client validation: parse a whole number. React silently ignores a bad value; prefer a field
  error and **document the deviation**. Then clamp to the core constants for parity. The view model
  should take the bounds as parameters or mirror them as constants, because a default-feature view
  model cannot import `crosshook-core` (see the tokio conflict).
- Submit: `Idle → Submitting → Succeeded | Failed`.
  - Submit is refused while submitting, while invalid, and (optionally) while pristine.
  - Emit `Effect::SaveSettings{patch}`.
  - A second submit while one is in flight is ignored or disabled, as with `isSaving`.
- `apply_errors(AppError::Validation{field})`:
  - Field matches a form field → field error. Keep the other field errors (useRunExecutable
    parity).
  - Unknown field (e.g. `"settings"`) → form-level error.
  - Non-validation kind → clear field errors and set a form-level error.
  - Editing a field clears only that field's error.

The SteamGridDB key flow fits a one-field variant: trim, submit disabled when empty, success
message, input cleared on success.

## 5. Property-test invariants and edge cases

Notation: `S` = summaries (backend order), `q` = query, `F` = filter key, `K` = sort key,
`R` = running game names, `out = select(S, q, F, K, R)`.

1. **Subset and multiplicity:**
   - `out` is a sub-multiset of `S`, and every row is unchanged (fields not mutated).
   - For `K ∈ {recent, lastPlayed, playtime}`, `out` is a subsequence of `S` with input order
     kept.
   - With `K = name`, `out` is a permutation of the unsorted filtered list.
2. **Identity:**
   - `q.trim() == ""` with `F ∈ {all, recentlyLaunched}` and `K ≠ name` → `out == S`.
   - `select(S, "", all, recent, R) == S` for any `R`.
3. **Idempotence:**
   - Filtering twice equals filtering once.
   - Sorting an already sorted list gives the same list.
   - Searching the output of a search with the same `q` gives the same rows.
4. **Commutativity:** search∘filter == filter∘search, compared as sequences.
5. **Monotonic search:** if `q2` contains `q1` (after normalisation), then `results(q2) ⊆ results(q1)`.
6. **Case-insensitivity:** `results(q) == results(q.to_uppercase())` for ASCII. Uppercasing a
   Unicode string and lowercasing it again does not always round-trip, so restrict this to
   ASCII-letter queries.
7. **Whitespace:** `results("  q  ") == results("q")`. A query of only whitespace gives the
   identity.
8. **Search fields:** a row matches iff the lowercased `name` contains the lowercased query **or**
   the lowercased `gameName` contains it. Searching for a row's exact `steamAppId` does not match
   unless the id also appears in its name or game name.
9. **Favourites:** `out(F=favorites) == [r ∈ S | r.is_favorite]`, and every row in it has
   `is_favorite`.
10. **Installed:** `r ∈ out(F=installed) ⇔ r.steam_app_id != "" && r.steam_app_id != "0"`.
    Edge cases: `""`, `"0"`, `"00"`, `"0 "` (included because the frontend does not trim), and
    `"123"`.
11. **Running:**
    - `out(F=currentlyRunning) == [r ∈ S | R.contains(r.name)]`, with an exact, case-sensitive
      match (`"alpha"` ∉ `{"Alpha"}`).
    - `|out(F=currentlyRunning, q="")| == |R ∩ names(S)| ≤ |R| == badge`.
12. **Badge (session reducer):**
    - `badge == |{name | game session with Some(name)}|`.
    - Anonymous or trainer sessions never change it.
    - Two game sessions of one profile give a badge of 1.
    - Badge 0 means "hide" (`None`).
13. **Sort `name`:**
    - The result is ordered by `key = if game_name.is_empty() { name } else { game_name }`.
      `" "` is a non-empty key.
    - Stable: rows with equal comparator keys keep their relative input order.
    - Tie cases: the same `gameName` on two profiles, or `""` vs a `gameName` equal to the other
      row's `name`.
14. **Session reducer (from `apply_after`):** generate a random registry history with unique ids,
    `seq` 1..n, and starts before exits. Then, for every snapshot point `S`:
    - `snapshot(S)` + events with seq > S, applied in any arrival interleaving before or after
      the snapshot response, equals the final registry state.
    - Replaying events with `seq ≤ S` changes nothing.
    - Duplicate delivery of an event changes nothing (idempotence).
    - `Resync` followed by a fresh snapshot replaces the state (it does not merge).
    - A stale snapshot token is ignored.
    - A failed snapshot keeps the state, and the next event re-emits the fetch effect.
15. **Inputs to generate:** empty `S`; duplicate `gameName`s; empty `gameName`; whitespace-only
    `gameName`; non-ASCII names (`"Éclair"`, `"ΑΣ"`, `"İ"`); a query longer than every field; a
    query containing regex-like characters (`".*"`, which must be matched literally); names
    differing only in case; `R` containing names absent from `S`; `R` empty.

## 6. Notes for the crate design

- React keeps search, filter and sort as local component state. Search is not debounced.
  `sortBy` defaults to `recent` and `filterKey` to `all`. Only the view mode is persisted (through
  settings: `LibraryPage.tsx:75`, `LibraryPage.viewMode.test.tsx`).
- The intents that map one-to-one are `SetQuery`, `SetFilter`, `SetSort`, `FilterIntent` (from the
  sidebar), `SummariesLoaded`, `FavoritesChanged`, `ToggleFavorite` (optimistic, with an effect
  and a revert on failure), and the session intents listed in §2.3.
- Keep the `recentlyLaunched`, `lastPlayed` and `playtime` variants for persisted-state
  compatibility, but document them as no-ops matching React. Do **not** invent semantics.
