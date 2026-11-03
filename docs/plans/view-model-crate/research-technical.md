# YAN-794 (M1-39) — `crosshook-view-model`: technical research

All paths are relative to `src/crosshook-native/` unless they start with `scripts/`, `.github/` or `docs/`.
Toolchain is pinned to **1.95.0** by `rust-toolchain.toml` (clippy 0.1.95). The host default is 1.99, so every probe below was re-run with `cargo +1.95.0`.

Probe method: (1) a throwaway workspace at `/tmp/vm-clippy-probe` with crates `vm`, `other` (which depends on tokio) and `ui` (which enables the `vm/app` feature); (2) an rsync copy of this worktree's native workspace at `/tmp/vm-ws-sim` with a stub `crates/crosshook-view-model` added. Both were deleted afterwards. No repo files were edited.

---

## 1. Dependency conflict (tokio)

### Confirmed

- `cargo tree -p crosshook-core -e normal -i tokio` → `tokio v1.51.0`. Core pulls it in directly and again through `reqwest`/`hyper`, `tokio-util` and `zbus`. Sources: `crates/crosshook-core/Cargo.toml` (`tokio = { …features = ["fs","macros","process","rt","sync","time"] }`, `reqwest`, `zbus = { …features = ["tokio"] }`).
- `cargo tree -p crosshook-app -e normal -i tokio` shows tokio directly from crosshook-app (`crates/crosshook-app/Cargo.toml`: `tokio = { …"rt-multi-thread","sync","time","macros","process","io-util","fs" }`) and again through crosshook-core.
- In the scratch workspace, `cargo tree -p crosshook-view-model -e normal --features app` produces **251 lines**, including tokio. So any normal-edge dependency on core or app makes the "no tokio in tree" acceptance criterion impossible.

### Options

| Option                                                                                                                                            | Verdict                               | Why                                                                                                                                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **(b)** Depend on core/app directly and accept tokio                                                                                              | **Reject**                            | Breaks the AC outright. It also compiles rusqlite (bundled C), reqwest, zbus and so on just to unit-test pure reducers.              |
| **(c)** Extract a tokio-free types crate (`AppError`, `AppErrorKind`, `AppEvent`, `SessionId`/`SessionKind`, `ProfileSummary`, `RunningSessions`) | **Reject for M1-39** (possible later) | Large cross-crate refactor, see the cost list below.                                                                                 |
| **(a)** View-model owns minimal mirror types; an optional `app` feature adds conversions                                                          | **Recommend**                         | The default tree is just the crate itself (verified). Feature code compiles and lints cleanly (verified). Zero churn in core or app. |

What option (c) would cost:

- `AppError` has 12 `From` impls (`crates/crosshook-app/src/error.rs:88-320`), including `From<tokio::task::JoinError>` (`:88`) and 10 core error types. If `AppError` moved, the orphan rule would block `impl From<core::X> for types::AppError` inside crosshook-app. Every `?` conversion in app services would have to be rewritten, or the types crate would have to depend on core.
- `AppError::new` sanitizes through `presenter::sanitize_message`, which reads `$HOME` (`crates/crosshook-app/src/presenter/mod.rs:31-34`). That is environment I/O.
- `SessionId` shares a module with `tokio::sync::broadcast` (`crates/crosshook-core/src/launch/session/types.rs:11,21`).
- `AppEvent` is defined in app (`events.rs:61`) and uses core's `SessionId`/`SessionKind`.

### How option (a) behaves (verified)

**`cargo tree`**

- `cargo tree -p vm -e normal` prints only `vm`, even when the workspace member `ui` enables `vm/app`. `cargo tree -p` resolves features only for the packages you select.
- `--all-features` (or `-p vm -p ui`) shows `other` and `tokio`.
- `-i tokio` on the default tree exits **101** ("package ID specification `tokio` did not match any packages"). A gate script must therefore list the tree and match names, as `scripts/check-app-deps.sh` already does. It must not rely on `-i`.

**`cargo clippy --all-targets` across the workspace**

- Code behind `#[cfg(feature = "app")]` is **not compiled** while no workspace member enables the feature. A deliberate lint violation there passed.
- Once a member enables the feature (for example a future `crosshook-ui` with `features = ["app"]`), resolver-2 feature unification compiles it and the violation fails.
- `-p crosshook-view-model --all-targets --all-features -- -D warnings` compiles and lints the feature code. In the scratch copy of the real workspace it passed against the real `crosshook_app::AppError`/`AppErrorKind`.

**Avoid `cargo clippy --workspace --all-features`.** It would also turn on core `ts-rs`, `experimental-providers` and `bootstrap-trace`, and app `test-seams`. The lint job asserts `bootstrap-trace` stays out of the release graph (`.github/workflows/lint.yml:54-61`). Use `-p crosshook-view-model --features app` (or `--all-features` scoped by `-p`, which only touches the selected package's features).

### Recommended manifest

```toml
[package]
name = "crosshook-view-model"
version.workspace = true
edition = "2021"
description = "Toolkit-free, I/O-free view-models: intent/effect reducers, form state and pure selectors for native screens."

[lints]
workspace = true

[features]
default = []
# From/TryFrom adapters for crosshook-app DTOs. Pulls tokio; never default.
app = ["dep:crosshook-app", "dep:crosshook-core"]

[dependencies]
crosshook-app = { path = "../crosshook-app", optional = true }
crosshook-core = { path = "../crosshook-core", optional = true }

[dev-dependencies]
proptest = { version = "1.12", default-features = false, features = ["std"] }
```

- **Why `crosshook-core` is also needed:** `AppEvent::SessionStarted/SessionExited` and `RunningSession` carry `crosshook_core::launch::{SessionId, SessionKind}` (`events.rs:3,116-144`; `services/launch/sessions.rs:13-29`). `crosshook-app` does not re-export them (`lib.rs:20-35`). Core exports both at `crates/crosshook-core/src/launch/mod.rs:62-66`.
- **No tokio, no dev-dependency on crosshook-app.** Either one puts tokio into the `-e dev` tree and switches on the tokio `clippy.toml` entries for tests.
- **Workspace member:** add `"crates/crosshook-view-model"` to `Cargo.toml:2-7`.

### Mirror types and conversions under the `app` feature

| View-model type                                                                                                          | Source type               | Conversion                                                            | Notes                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------------------------------------------------------------------------------------------ | ------------------------- | --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ServiceError { kind: ServiceErrorKind, message: String }`, where `ServiceErrorKind::Validation { field: &'static str }` | `AppError`/`AppErrorKind` | `impl From<&crosshook_app::AppError>`                                 | Mirror `AppErrorKind` exactly (`error.rs:7-25`): Validation{field}, NotFound, Conflict, Unavailable, Io, Cancelled, Internal. `field` is `&'static str` upstream too, so mapping needs no allocation. The message is already sanitized upstream; the default-feature mirror must document "presenter-sanitized text only", because the view-model cannot sanitize (env access is disallowed). |
| `LibraryCard` + favorites set                                                                                            | `ProfileSummary`          | `From<crosshook_app::services::profiles::ProfileSummary>`             | Fields at `services/profiles/types.rs:17-30`: name, game_name, steam_app_id, custom_cover/portrait_art_path, network_isolation. Favorites come separately from `list_favorites` (`services/profiles/reads.rs:64`), so `is_favorite` is joined in the reducer.                                                                                                                                 |
| `SessionsSnapshot { sessions, seq }`                                                                                     | `RunningSessions`         | `From<RunningSessions>`                                               | `sessions.rs:31-38`. Turn `SessionId` into an opaque `SessionKey(String)` through `Display`; the inner `Uuid` is private (`session/types.rs:21-33`).                                                                                                                                                                                                                                          |
| `SessionEvent::{Started, Exited}`                                                                                        | `AppEvent`                | `fn session_event(&AppEvent) -> Option<SessionEvent>`, **not** `From` | `AppEvent` is `#[non_exhaustive]` (`events.rs:60`), so the conversion needs a wildcard arm. `Resync` (`:63`) should map to a `Resync` intent that asks for a snapshot effect.                                                                                                                                                                                                                 |

Seq contract for the "Currently Playing" reducer, from `sessions.rs:6-9` and `events.rs:113-125`: subscribe first, then snapshot; apply only events with `seq > snapshot.seq`. The React oracle buffers events until the snapshot lands and drops anything `<=` the baseline (`src/hooks/runningSessionsStore.ts:136-178`). The count is distinct profile names with a running **Game** session; anonymous (`profile_name: None`) and trainer sessions are excluded (`runningSessionsStore.ts:59-64`). The badge is the set size, hidden when 0 (`src/components/layout/AppShell.tsx:93-97`).

### CI wiring proposal (`.github/workflows/lint.yml`)

Today's steps: clippy (`:49`, workspace default features), tests (`:52`, explicit `-p crosshook-app -p crosshook-native -p crosshook-cli`). `scripts/check-app-deps.sh` is **not wired into any workflow**; a grep of `.github` and `scripts` found only plan docs. Add:

```yaml
- name: View-model crate (default + app feature)
  run: |
    set -euo pipefail
    m=src/crosshook-native/Cargo.toml
    cargo clippy --manifest-path "$m" -p crosshook-view-model --all-targets --features app -- -D warnings
    cargo test   --manifest-path "$m" -p crosshook-view-model
    cargo test   --manifest-path "$m" -p crosshook-view-model --features app
- name: Dependency gates (no toolkit in app; no toolkit/tokio in view-model)
  run: |
    ./scripts/check-app-deps.sh --selftest
    ./scripts/check-app-deps.sh
    ./scripts/check-app-deps.sh --package crosshook-view-model
```

The default-feature clippy pass for the crate is already covered by the workspace clippy step at `:49`.

**Gate script.** Generalize `check-app-deps.sh` rather than copying it. It hard-codes `PACKAGE="crosshook-app"` (`:14`) and `BANNED_RE` (`:18`). Proposed changes:

- Add a `--package <name>` flag.
- When the package is `crosshook-view-model`, also ban `^(tokio)([-_0-9].*)?$` (covers tokio, tokio-util, tokio-macros, tokio-rustls) and `^crosshook-(app|core)$`. The second pattern proves the `app` feature never becomes default.
- Add `--selftest` cases for each new pattern.
- Keep the existing `cargo tree … -e normal --prefix none --format '{p}'` approach. With the recommended manifest, the default tree is exactly one line: `crosshook-view-model v0.6.1 (…)`.

---

## 2. Crate-local `clippy.toml` (verified with clippy 0.1.95)

| Question                                                                                                                                       | Result                                                                                                                                                                                                                                                                                                         |
| ---------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Picked up from the crate dir inside a workspace?                                                                                               | **Yes.** `crates/vm/clippy.toml` applied to `vm` only; the sibling `other` was not linted by it. Clippy searches `CARGO_MANIFEST_DIR` and its parents.                                                                                                                                                         |
| Merged with a workspace-root `clippy.toml`?                                                                                                    | **No.** A root `clippy.toml` banning `String::new` flagged `other` but **not** `vm`, which has its own file. The nearest file wins outright. The repo has no `clippy.toml` today (`find` found none), so nothing conflicts now. If a root file is added later, the crate file must repeat any shared settings. |
| Interplay with `[lints] workspace = true`                                                                                                      | Independent. Lint **levels** come from `Cargo.toml` (`[workspace.lints.clippy]`, `Cargo.toml:13-31`); `clippy.toml` holds lint **configuration**. The workspace `uninlined_format_args` still fired in the crate that had its own `clippy.toml`.                                                               |
| Adding `[lints.rust] unsafe_code = "forbid"` next to `workspace = true`                                                                        | **Hard error**: "cannot override `workspace.lints` in `lints`". Use `#![forbid(unsafe_code)]` in `lib.rs` instead.                                                                                                                                                                                             |
| Default level                                                                                                                                  | `disallowed_methods` and `disallowed_types` are **warn** by default. CI's `-D warnings` turns hits into errors (rc 101 observed).                                                                                                                                                                              |
| Path that doesn't resolve inside a crate that **is** in the graph (`std::fs::does_not_exist`, `std::fs::NoSuchType`)                           | **Warning** anchored at the `clippy.toml` line: "`…` does not refer to a reachable function/type", with help "add `allow-invalid = true` to the entry to suppress this warning". `-D warnings` does **not** promote it (rc 0).                                                                                 |
| Path into a crate **not** in the graph (`tokio::spawn` with no tokio dep)                                                                      | **Silently ignored.** No warning, rc 0.                                                                                                                                                                                                                                                                        |
| tokio present, item feature-gated off (`tokio::process::Command`, `tokio::fs::File`, `tokio::net::TcpStream` with tokio `rt,time,sync,macros`) | Warning "does not refer to a reachable type".                                                                                                                                                                                                                                                                  |
| `allow-invalid = true`                                                                                                                         | **Supported**: it silences the unresolved warning, and the entry still fires once it resolves (`tokio::time::sleep` with `allow-invalid` still errored in a `#[tokio::test]`).                                                                                                                                 |
| `tokio::*` glob                                                                                                                                | **Not supported.** It gives the "does not refer to a reachable function" warning and bans nothing. Every tokio item must be listed on its own.                                                                                                                                                                 |
| Fires through deref?                                                                                                                           | Yes. `PathBuf::exists()` and `Path::new(p).join("x").is_dir()` were flagged by `std::path::Path::exists` / `is_dir`.                                                                                                                                                                                           |
| False positives from `proptest!` expansion in `#[cfg(test)]`                                                                                   | None. Clean with `-D warnings`.                                                                                                                                                                                                                                                                                |
| Change tracking                                                                                                                                | Editing only `clippy.toml` re-ran the lint without touching sources (the new config warning appeared).                                                                                                                                                                                                         |

Entry syntax that was exercised: `{ path = "std::thread::sleep", reason = "…" }`; the `reason` shows as `= note: …`. Optional key: `allow-invalid = true`.

### Recommended `crates/crosshook-view-model/clippy.toml`

All `std` entries below resolved on 1.95.0 with **zero** unreachable-path warnings: the whole file was linted clean against the stub crate in the scratch workspace copy, and seeded violations fired. The tokio entries carry `allow-invalid = true` so a future tokio dev-dependency with narrow features doesn't add noise.

```toml
# crosshook-view-model is pure: no I/O, no processes, no blocking, no async
# runtime, no ambient process state. Clippy uses this file instead of any
# workspace-root clippy.toml (no merge). CI's `-D warnings` makes hits errors.
# tokio entries resolve only if tokio ever enters this crate's graph; the
# `scripts/check-app-deps.sh --package crosshook-view-model` gate is the
# primary guard. `tokio::*` globs are not supported, so items are listed.

disallowed-methods = [
  # std::fs
  { path = "std::fs::canonicalize", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::copy", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::create_dir", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::create_dir_all", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::exists", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::hard_link", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::metadata", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::read", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::read_dir", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::read_link", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::read_to_string", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::remove_dir", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::remove_dir_all", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::remove_file", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::rename", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::set_permissions", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::symlink_metadata", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  { path = "std::fs::write", reason = "view-models are I/O-free; emit an Effect for the UI bridge" },
  # std::path methods that touch the filesystem (also caught through PathBuf deref)
  { path = "std::path::Path::canonicalize", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::exists", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::try_exists", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::is_dir", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::is_file", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::is_symlink", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::metadata", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::read_dir", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::read_link", reason = "touches the filesystem; view-models are I/O-free" },
  { path = "std::path::Path::symlink_metadata", reason = "touches the filesystem; view-models are I/O-free" },
  # std::process
  { path = "std::process::abort", reason = "view-models never terminate the process" },
  { path = "std::process::exit", reason = "view-models never terminate the process" },
  # std::thread
  { path = "std::thread::sleep", reason = "reducers must not block; model time as an Intent" },
  { path = "std::thread::spawn", reason = "reducers are synchronous; the UI bridge owns concurrency" },
  { path = "std::thread::scope", reason = "reducers are synchronous; the UI bridge owns concurrency" },
  # std::env (ambient state makes reducers non-deterministic)
  { path = "std::env::var", reason = "pass configuration in through state or an Intent" },
  { path = "std::env::var_os", reason = "pass configuration in through state or an Intent" },
  { path = "std::env::vars", reason = "pass configuration in through state or an Intent" },
  { path = "std::env::vars_os", reason = "pass configuration in through state or an Intent" },
  { path = "std::env::set_var", reason = "view-models never mutate process state" },
  { path = "std::env::remove_var", reason = "view-models never mutate process state" },
  { path = "std::env::current_dir", reason = "pass paths in through state or an Intent" },
  { path = "std::env::set_current_dir", reason = "view-models never mutate process state" },
  { path = "std::env::home_dir", reason = "pass paths in through state or an Intent" },
  { path = "std::env::temp_dir", reason = "pass paths in through state or an Intent" },
  # console handles
  { path = "std::io::stdin", reason = "view-models are I/O-free" },
  { path = "std::io::stdout", reason = "view-models are I/O-free" },
  { path = "std::io::stderr", reason = "view-models are I/O-free" },
  # tokio
  { path = "tokio::spawn", reason = "no async runtime in view-models; emit an Effect", allow-invalid = true },
  { path = "tokio::task::spawn", reason = "no async runtime in view-models; emit an Effect", allow-invalid = true },
  { path = "tokio::task::spawn_blocking", reason = "no async runtime in view-models; emit an Effect", allow-invalid = true },
  { path = "tokio::task::spawn_local", reason = "no async runtime in view-models; emit an Effect", allow-invalid = true },
  { path = "tokio::time::sleep", reason = "no async runtime in view-models; model time as an Intent", allow-invalid = true },
  { path = "tokio::time::timeout", reason = "no async runtime in view-models; model time as an Intent", allow-invalid = true },
  { path = "tokio::time::interval", reason = "no async runtime in view-models; model time as an Intent", allow-invalid = true },
]

disallowed-types = [
  { path = "std::fs::File", reason = "view-models are I/O-free" },
  { path = "std::fs::OpenOptions", reason = "view-models are I/O-free" },
  { path = "std::fs::DirBuilder", reason = "view-models are I/O-free" },
  { path = "std::fs::ReadDir", reason = "view-models are I/O-free" },
  { path = "std::process::Command", reason = "host processes go through crosshook-app services (ADR-0001)" },
  { path = "std::process::Child", reason = "host processes go through crosshook-app services (ADR-0001)" },
  { path = "std::thread::JoinHandle", reason = "reducers are synchronous; the UI bridge owns concurrency" },
  { path = "std::thread::Builder", reason = "reducers are synchronous; the UI bridge owns concurrency" },
  { path = "std::net::TcpStream", reason = "view-models are I/O-free" },
  { path = "std::net::TcpListener", reason = "view-models are I/O-free" },
  { path = "std::net::UdpSocket", reason = "view-models are I/O-free" },
  { path = "std::time::Instant", reason = "reducers must be deterministic; pass time in through an Intent" },
  { path = "std::time::SystemTime", reason = "reducers must be deterministic; pass time in through an Intent" },
  { path = "tokio::runtime::Runtime", reason = "no async runtime in view-models", allow-invalid = true },
  { path = "tokio::runtime::Handle", reason = "no async runtime in view-models", allow-invalid = true },
  { path = "tokio::task::JoinHandle", reason = "no async runtime in view-models", allow-invalid = true },
  { path = "tokio::process::Command", reason = "host processes go through crosshook-app services (ADR-0001)", allow-invalid = true },
  { path = "tokio::fs::File", reason = "view-models are I/O-free", allow-invalid = true },
  { path = "tokio::net::TcpStream", reason = "view-models are I/O-free", allow-invalid = true },
  { path = "tokio::sync::mpsc::Sender", reason = "no async runtime in view-models", allow-invalid = true },
  { path = "tokio::sync::broadcast::Receiver", reason = "no async runtime in view-models", allow-invalid = true },
]
```

Design notes:

- **`Instant`/`SystemTime`** keep reducers deterministic and easy to `Script`-test. If the submit state machine or a debounce needs time, carry it in an intent such as `Tick { now_ms: u64 }`.
- **Why the `std::env` entries:** core and app read `$HOME` freely (`presenter/mod.rs:8,32,50`). Banning env reads keeps that pattern out of reducers.
- **What the `app` feature compiles:** only adapter code in this crate. The disallow list applies to this crate's code, not to crosshook-app's internals, so the feature build stays clean (verified).
- **To check the lint is active:** temporarily add `std::thread::sleep(...)` to `lib.rs` → `cargo clippy -p crosshook-view-model -- -D warnings` fails with `use of a disallowed method`. This is the "clippy disallow config active" evidence for the PR test plan.

---

## 3. `AppError` API (`crates/crosshook-app/src/error.rs`)

- `AppErrorKind` is `Copy + Eq`. `Validation { field: &'static str }` (`:7-25`).
- `AppError` holds private `kind` and `message`. Its accessors are `kind()` (`:78`) and `message()` (`:83`, sanitized).
- The only **public** constructor is `AppError::validation(field, message)` (`:53`). `AppError::new` is `pub(crate)` (`:37`). App-feature tests can therefore build validation errors directly, but other kinds only through `From<std::io::Error>` (`:99`, gives `Io`) or core error types.

Validation `field` names used in crosshook-app today:

| field                                                                                                     | Where                                                                                                                      |
| --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `settings`                                                                                                | `error.rs:178` (settings TOML parse); launcher-export tests `tests/export.rs:173`, `tests/list_orphans.rs:191`             |
| `profiles_directory`                                                                                      | `services/settings.rs:241-246` (`SettingsService::load` → `resolve_profiles_directory_from_config` error)                  |
| `ui`                                                                                                      | only test fakes `services/settings/ui_writer_tests.rs:51,63`                                                               |
| `profile`, `launch`, `community_profile`, `recent_files`, `path`, `tap`, `lutris`, `metadata`, `revision` | `error.rs:114-317` (`From` impls)                                                                                          |
| `event_capacity`                                                                                          | `events.rs:161`, `jobs/registry/mod.rs:305`                                                                                |
| `collection_id`, `output_path`, `path`                                                                    | `services/collections/transfer.rs:43-72`                                                                                   |
| `attachments`                                                                                             | `services/diagnostics.rs:92`                                                                                               |
| `profile_name`                                                                                            | `services/discovery/compatibility.rs:17`, `services/launch/queries.rs:144`, `services/protondb/suggestions.rs:55`, install |
| `prefix_path`                                                                                             | `services/install/mod.rs:88`                                                                                               |
| `external_source`                                                                                         | `services/discovery/sources.rs:31`                                                                                         |
| `request`, `method`                                                                                       | `services/launcher_export/export.rs:28`, `list.rs:136`                                                                     |
| `version`                                                                                                 | `services/protonup/install.rs:138`                                                                                         |
| `art_type` / `image_type`                                                                                 | `services/steam/metadata.rs:86-87`                                                                                         |

**Settings save produces no per-field validation.** `SettingsService::save_request` (`services/settings.rs:281-289`) merges and then **normalizes** (clamps `recent_files_limit`, defaults an empty `log_filter` to `"info"`; `:194-205`). It returns `Validation { field: "settings" }` only when the on-disk TOML fails to parse (`error.rs:177-180`). For `Form::apply_errors`:

- A `field` that matches a form field key goes to that field.
- An **unknown** field (`"settings"`) becomes a **form-level** error. This is a realistic case and should be a required test.

Form field keys should be `&'static str` equal to the `SettingsSaveRequest` snake_case names (`services/settings.rs:57-106`). That makes the mapping identity.

## 4. Settings form example

`AppSettingsData` and the clamp constants live in core (`crates/crosshook-core/src/settings/types.rs:10-13,36-38`). The React oracle is `src/components/settings/LoggingAndUiSection.tsx` and `ProfilesSection.tsx`.

**Recommended worked form: "Logging and UI"**

- **`recent_files_limit`** is the client-validation example. It is numeric text input; valid range `1..=100` (`RECENT_FILES_LIMIT_MIN/MAX`, `types.rs:10-13`); default 10 (`:27-29`).
  - React (`LoggingAndUiSection.tsx:58-79`) uses `<input type=number min=1 max=100>` and, on blur, `parseInt` → silently ignores non-finite input → clamps to 1..100 → persists only if changed.
  - A view-model selector can expose both `validate` (non-numeric or out-of-range → field error) and parity behaviour (clamp on submit). Pick one and document it; the parity research should decide. Either way, the pristine/dirty/touched/apply-errors paths get exercised.
- **`log_filter`** is the enum/select example: options error|warn|info|debug|trace (`LoggingAndUiSection.tsx:17-38`); an empty value normalizes to `"info"` (`services/settings.rs:199-204`; `types.rs:15-17`). Good for a "dirty only when the value differs from pristine" test, since React persists only when `v !== settings.log_filter` (`:27-30`).

**Server-error mapping:**

- Use `profiles_directory` (`ProfilesSection.tsx:31-47`: trimmed on blur, empty = default) as the field that receives `AppError::validation("profiles_directory", …)` (`services/settings.rs:241-246`). This is a real upstream field name.
- Use `"settings"` as the unknown → form-level case.
- Optionally add a `profiles_directory_requires_restart` selector (`services/settings.rs:45-47`; `ProfilesSection.tsx:76-79`) as a pure-selector example.

## 5. Crate conventions

**`Cargo.toml`**

- `version.workspace = true`, `edition = "2021"`, `description = …`, `[lints] workspace = true` (`crates/crosshook-app/Cargo.toml:1-10`).
- core/cli/src-tauri use a literal `version = "0.6.1"` and also `[lints] workspace = true`.
- Every crate is edition 2021. No crate sets `rust-version`.

**Crate attributes**

- `#![deny(missing_docs)]` (`crates/crosshook-app/src/lib.rs:8`) is the only crate that uses it.
- No crate uses `#![forbid(unsafe_code)]`; it will be new, and it must be an attribute (see §2).
- The crate-level `//!` doc should point at the README (`lib.rs:1-6`).
- For the "public API documented" AC, optionally run `RUSTDOCFLAGS="-D warnings" cargo doc -p crosshook-view-model --no-deps` in CI.

**Tests**

- Use sibling modules: `#[cfg(test)] mod tests;` next to the code, in `foo/tests.rs` (`services/settings.rs:27-34`; `error.rs` → `error/tests.rs`). Keep `testing` (the `Script` harness plus fixture builders) as a `pub mod testing` gated by `#[cfg(any(test, feature = "testing"))]` if the future `crosshook-ui` tests should reuse it. A non-`app` `testing` feature adds no dependencies.
- **proptest**
  - Not in the offline registry: `~/.cargo/registry/cache` has no `proptest*` crate, although `rand-0.10.3`/`rand_core-0.10.1` are cached.
  - Online: **1.12.0** (MSRV 1.88 ≤ 1.95), which resolved and ran under 1.95.
  - `proptest = "1"` with default features adds **15** lock entries: bit-set, bit-vec, borsh, borsh-derive, chacha20, core_detect, proptest, quick-error, rand 0.10.3, rand_core 0.10.1, rand_xorshift, rusty-fork, syn 3.0.9, unarray, wait-timeout.
  - `default-features = false, features = ["std"]` adds only **7**: chacha20, core_detect, proptest, rand, rand_core, rand_xorshift, unarray. It drops the fork/timeout runners, which pure reducers don't need, and the `bit-set` strategies. **Recommend the std-only form.** A `proptest!` test with `collection::vec` and regex string strategies (`"[a-z]{0,8}"`) passed under it, and `cargo tree -p crosshook-view-model -e normal` stayed a single line.
  - Failure persistence writes a `proptest-regressions/` file next to the test source on failure. Either commit those files or set `failure_persistence: None` in `ProptestConfig`.

**README style** (`crates/crosshook-app/README.md`)

- `# crate` heading, an intro paragraph, a `## Design` table of pieces (`:1-27`), per-area `##` sections, and a closing `## Dependencies` section that lists deps and names the guard script (`:640-646`).
- The new README needs the AC section "Porting a screen", listing the steps a ported screen takes:
  1. define `State`/`Intent`/`Effect`;
  2. implement `ViewModel::reduce`;
  3. add selectors;
  4. write a `testing::Script` test and a proptest;
  5. have the UI bridge execute effects through crosshook-app services and feed results back as intents.
- Also update the AGENTS.md/CLAUDE.md architecture rows (`AGENTS.md:44,157`) to mention the crate. Those files are context-only here; flag them for the docs pass.

**`scripts/prepare-release.sh`**

- The version-manifest list (`:8-13`) holds root, core, cli and src-tauri; **crosshook-app is absent**.
- For each listed manifest the script rewrites `[package] name=… version = "…"` and then requires `^version = "X"$` (`:63-79`). A `version.workspace = true` manifest **fails** that check. Verified: the perl regex leaves it unchanged and the grep does not match, so the script would `die`.
- `.github/workflows/release.yml:68-90` uses the same 4-manifest list, and its awk reader returns an empty string for an inherited version ("Failed to read version").
- **Recommendation:** use `version.workspace = true` (as crosshook-app does), so the `[workspace.package]` bump syncs it automatically, and **do not** add the crate to either list. Migration-plan §11.1 says to add app/view-model/ui to the list (`docs/research/native-ui-migration/README.md:616`). With workspace inheritance that would break the release, so note in the PR that inheritance already covers the intent.

**`scripts/check-host-gateway.sh`**

- `SCAN_DIRS` **already includes** `crates/crosshook-view-model/src` (`:17`).
- ADR-0001 already lists it (`docs/architecture/adr-0001-platform-host-gateway.md:118`).
- Missing directories are skipped (`:64-73`), so the check is live as soon as the crate exists. No change needed.

**`Cargo.lock`**

- Adding the member adds a `crosshook-view-model 0.6.1` package entry plus the proptest closure above (7 entries std-only); the lockfile must be committed.
- The CI cache keys hash `Cargo.lock` (`lint.yml:35`; `copilot-setup-steps.yml:65`), so the first run after merge starts with a cold cache.
- `copilot-setup-steps.yml:70` runs `cargo fetch --locked`, so the updated lockfile must be committed.
- The Flatpak manifest has no cargo-sources file to regenerate (`packaging/flatpak/` has only the yml, desktop, metainfo and README).

## Summary of recommendations

1. **Option (a).** No default deps. An optional `app = ["dep:crosshook-app", "dep:crosshook-core"]` feature carries `From` adapters plus `session_event(&AppEvent) -> Option<_>`, because `AppEvent` is `#[non_exhaustive]`.
2. **Crate-local `clippy.toml` from §2.** Set `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` as attributes, and keep `[lints] workspace = true`.
3. **CI.** Add `-p crosshook-view-model` clippy and tests with `--features app`, plain tests, and dependency gates. Generalize `check-app-deps.sh` with `--package` and the tokio/crosshook-\* bans, and wire it into `lint.yml`; it is not wired in CI today.
4. **proptest.** `proptest = { version = "1.12", default-features = false, features = ["std"] }`.
5. **Settings form.** `recent_files_limit` + `log_filter` for client-side state, `profiles_directory` for a field-level server error, `settings` for a form-level server error.
6. **Versioning.** `version.workspace = true`; do not add the crate to the prepare-release or release.yml manifest lists. Gateway `SCAN_DIRS` already covers the crate.
