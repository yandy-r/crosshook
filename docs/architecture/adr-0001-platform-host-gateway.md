# ADR-0001: `platform.rs` is the host-command gateway

**Status**: Accepted — 2026-04-17

---

## Context

CrossHook is a native Linux desktop application (Tauri v2, AppImage today, Flatpak target tracked under issue #276). Its job is to _orchestrate_ Windows game and trainer launches through Proton/Wine — it does not run Wine itself. Every tool that must interact with a game process (gamescope, MangoHud, winetricks, unshare, umu-run, git) therefore executes as a _host_ process, not inside any sandbox.

When packaged as a Flatpak, host-tool execution must traverse `flatpak-spawn --host`. Without a single abstraction, scattered `Command::new(...)` calls would silently break under Flatpak for three independent reasons:

1. **Binary path resolution differs.** Inside the Flatpak sandbox the host's `PATH` is not visible; `which git` and similar lookups return nothing unless routed through `flatpak-spawn --host which git`.
2. **`.env()` is silently dropped by `flatpak-spawn`.** Rust's `Command::env()` / `Command::envs()` set env vars on the _sandbox_ side of the `flatpak-spawn` call, not on the host child. The host process receives none of them. This is not an error — it is the documented behaviour of `flatpak-spawn --host`.
3. **Host filesystem paths must be normalized.** Flatpak remaps host paths under `/run/host`. A path like `/run/host/usr/share/steam/...` is valid inside the sandbox but the host process expects `/usr/share/steam/...`. Passing the un-normalized path to `flatpak-spawn --host` causes the command to fail.

These three failure modes are invisible in native or AppImage builds, which is exactly what makes them dangerous: a developer running the AppImage cannot observe the breakage that Flatpak users would see.

---

## Decision

All host-tool execution at the Flatpak boundary routes through
`src/crosshook-native/crates/crosshook-core/src/platform.rs`.

No module in `crosshook-core` or `src-tauri` directly calls `Command::new("<host-tool>")` with a literal tool name that belongs to the host. Every such invocation must use one of the gateway functions listed below.

### Public API table

| Function                                                                                    | Purpose                                                                                                                                                                                                                                                                                                                         | When to use                                                                                                                                |
| ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `is_flatpak() -> bool`                                                                      | Detects whether the process is running inside a Flatpak sandbox by checking `FLATPAK_ID` and `/.flatpak-info`. Result is computed on every call (no cache).                                                                                                                                                                     | Any conditional branch that needs to pick the Flatpak vs. native path.                                                                     |
| `is_steam_deck() -> bool`                                                                   | Detects SteamOS / Steam Deck from env vars and host os-release.                                                                                                                                                                                                                                                                 | UI adaptation and launch-optimization decisions that differ on Steam Deck.                                                                 |
| `normalize_flatpak_host_path(path: &str) -> String`                                         | Strips the `/run/host` prefix from sandbox-visible paths so the resulting path is valid on the host; resolves document-portal xattr for portal-mounted files.                                                                                                                                                                   | Any path that may have been persisted or received while running as Flatpak before being handed to a host command or stored persistently.   |
| `host_command(program: &str) -> Command`                                                    | Returns a `tokio::process::Command` that runs `program` on the host (via `flatpak-spawn --host`) or natively. No env threading.                                                                                                                                                                                                 | Host tools that inherit the sandbox environment (rare; prefer `host_command_with_env` for Proton/Wine launches).                           |
| `host_command_with_env(program, envs, custom_env_vars) -> Command`                          | Like `host_command`, but threads `envs` through `--env=KEY=VALUE` args under Flatpak (because `.env()` is dropped by `flatpak-spawn`). User-controlled `custom_env_vars` are written to a `0600` temp file and sourced via `bash` to keep sensitive values off the process argv.                                                | All Proton/Wine and umu-run launch commands that carry a fully-constructed env map.                                                        |
| `host_command_with_env_and_directory(program, envs, directory, custom_env_vars) -> Command` | Like `host_command_with_env`, but also sets the host working directory via `flatpak-spawn --directory=DIR`.                                                                                                                                                                                                                     | Proton/umu-run launches that require a specific working directory (e.g., the game's install root).                                         |
| `host_std_command(program: &str) -> StdCommand`                                             | Synchronous (`std::process::Command`) variant of `host_command`. No env threading.                                                                                                                                                                                                                                              | Synchronous host probes and one-shot queries (e.g., `test -d`, `ls`, watchdog signals).                                                    |
| `host_std_command_with_env(program, envs, custom_env_vars) -> StdCommand`                   | Synchronous variant of `host_command_with_env`.                                                                                                                                                                                                                                                                                 | Synchronous launch paths or probes that need env threading (e.g., git in `taps.rs`).                                                       |
| `host_command_exists(binary: &str) -> bool`                                                 | Checks whether `binary` is on the host's `PATH`. Under Flatpak runs `which <binary>` via `host_std_command`; natively scans the current `PATH`. Input is validated against an allowlist of safe binary-name characters.                                                                                                         | Pre-flight detection of optional host tools (gamescope, mangohud, etc.).                                                                   |
| `is_allowed_host_system_compat_listing_path(path: &Path) -> bool`                           | Returns `true` when `path` is an absolute path under `/usr` or `/usr/local` with no `..` components. Used as a security gate before any host filesystem probe.                                                                                                                                                                  | Called internally by every `host_path_is_*` and `host_read_*` function; also callable directly when a caller wants to pre-validate a path. |
| `host_path_is_dir(path: &Path) -> bool`                                                     | Returns whether `path` exists as a directory on the host filesystem. Only paths under `/usr` or `/usr/local` are allowed (no `..`).                                                                                                                                                                                             | System compat-tool root probing (Proton discovery).                                                                                        |
| `host_path_is_file(path: &Path) -> bool`                                                    | Returns whether `path` is a regular file on the host. Same path restrictions as `host_path_is_dir`.                                                                                                                                                                                                                             | Checking for VDF manifest files and other system Proton assets.                                                                            |
| `host_path_is_executable_file(path: &Path) -> bool`                                         | Returns whether `path` is an executable regular file on the host. Same path restrictions.                                                                                                                                                                                                                                       | Verifying Proton binary executability.                                                                                                     |
| `host_read_dir_names(path: &Path) -> io::Result<Vec<OsString>>`                             | Lists directory entries at `path` on the host (via `ls` under Flatpak). Same path restrictions.                                                                                                                                                                                                                                 | Scanning `compatibilitytools.d/` roots for installed Proton versions.                                                                      |
| `host_read_file_bytes_if_system_path(path: &Path) -> io::Result<Vec<u8>>`                   | Reads file bytes from the host (via `cat` under Flatpak). Same path restrictions.                                                                                                                                                                                                                                               | Reading VDF manifests and other binary/text assets from system Proton directories.                                                         |
| `normalized_path_is_file(path: &str) -> bool`                                               | Normalizes the path first (via `normalize_flatpak_host_path`), then applies `host_path_is_file` for paths under `/usr` or a direct `is_file()` check otherwise.                                                                                                                                                                 | Proton path validation where the stored path may carry a `/run/host` prefix.                                                               |
| `normalized_path_is_dir(path: &str) -> bool`                                                | Normalizes first, then directory check. Same routing logic.                                                                                                                                                                                                                                                                     | Validating working-directory and Steam root paths stored from a Flatpak session.                                                           |
| `normalized_path_is_executable_file(path: &str) -> bool`                                    | Normalizes first, then executable-file check. Same routing logic.                                                                                                                                                                                                                                                               | Checking whether a user-configured Proton binary is actually executable.                                                                   |
| `normalized_path_exists_on_host(path: &str) -> bool`                                        | Normalizes first, then probes existence on the host via `test -e` (any file type).                                                                                                                                                                                                                                              | General-purpose existence check for paths that may need normalization.                                                                     |
| `normalized_path_is_file_on_host(path: &str) -> bool`                                       | Normalizes first, then `test -f` on the host via `flatpak-spawn`.                                                                                                                                                                                                                                                               | Host-only file check for paths that may not pass the system-prefix allowlist.                                                              |
| `normalized_path_is_dir_on_host(path: &str) -> bool`                                        | Normalizes first, then `test -d` on the host.                                                                                                                                                                                                                                                                                   | Host-only directory check.                                                                                                                 |
| `normalized_path_is_executable_file_on_host(path: &str) -> bool`                            | Normalizes first, then `test -f` + `test -x` on the host.                                                                                                                                                                                                                                                                       | Host-only executability check.                                                                                                             |
| `override_xdg_for_flatpak_host_access()` (`unsafe`)                                         | Called once at app startup (`src-tauri/src/lib.rs`) before any threads spawn. Reads `HOST_XDG_*_HOME` env vars set by the Flatpak runtime and rewrites `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, and `XDG_CACHE_HOME` so CrossHook's data stores resolve to the host's real XDG directories instead of the per-app sandbox locations. | App entry point only — never call from library code.                                                                                       |

---

## Env-threading invariant

Rust's `Command::env()` and `Command::envs()` apply env vars to the child process by passing them through `execve(2)`. When the child _is_ `flatpak-spawn`, that means the vars land on `flatpak-spawn`'s own environment, not on the host process that `flatpak-spawn` subsequently forks. `flatpak-spawn --host` propagates the vars it was told to propagate through its `--env=KEY=VALUE` argument list — nothing else.

`platform.rs` works around this in two ways:

- **Structured env (launch maps):** `host_command_with_env` and `host_std_command_with_env` iterate every entry in the `envs` map and emit one `--env=KEY=VALUE` argument for each. `--clear-env` precedes the program name so the host child starts with a clean environment built entirely from the explicit list, free of sandbox-only variables that would otherwise poison Proton/Wine (e.g., `XDG_CONFIG_HOME` pointing at the sandbox's per-app directory).

- **User-supplied custom env (security-sensitive values):** `custom_env_vars` (user-defined launch overrides that may contain paths or credentials) are written to a `0600`-permission temporary file and sourced by `bash -c 'set -a; source "$1"; rm -f "$1"; set +a; shift; exec "$@"'` _after_ `flatpak-spawn` launches on the host. This keeps potentially sensitive values out of the process argv visible in `/proc/<pid>/cmdline`.

---

## Scope boundary

This decision applies **only** to host-tool execution at the Flatpak boundary — that is, literal tool names that the OS provides and that must run as host processes.

It does **not** apply to:

- **In-sandbox subprocess code.** Game binaries, trainer binaries, `bash`, `unshare`, and game-scope wrappers that are themselves launched _inside_ the sandbox (before `flatpak-spawn` hands off to the host) are started with `Command::new(...)` directly and are not subject to this rule.
- **User-supplied executable paths.** When a path comes from user configuration (game binary, trainer binary, Proton binary override), it is a variable, not a literal tool name. The code normalizes these paths via `normalize_flatpak_host_path` but does not need to route them through a gateway function.
- **Test fixtures.** Test code under `#[cfg(test)]` or `tests/` is exempt; tests that exercise `platform.rs` internals necessarily create `Command` values directly.

---

## Escape hatches

The following patterns are explicitly allowed, because they pair every non-gateway `Command::new` with an `is_flatpak()` guard that routes the Flatpak branch through `platform::host_*`:

1. **In-sandbox subprocess management.** `launch/script_runner.rs` launches `bash` and `unshare` as the outer wrapper for trainer scripts. In the non-Flatpak branch these are `Command::new("unshare")` / `Command::new(BASH_EXECUTABLE)` (lines 295–313 and 361–378). In the Flatpak branch the same call sites delegate to `build_flatpak_unshare_bash_command`, which routes through `platform::host_command_with_env`. Game binaries themselves are launched with `Command::new(normalized_game_path)` (line 662) — the path is user-supplied, not a literal tool name.

2. **User-supplied game and trainer paths.** Paths come from user configuration and are normalized via `normalize_flatpak_host_path` before use. They are not literal tool names and do not require a gateway function.

3. **Tools used in non-Flatpak branches, each paired with an `is_flatpak()` guard:**
   - `community/taps.rs` `git_command()` (lines 511–523): Under Flatpak calls `host_std_command_with_env("git", ...)`. Native uses `Command::new("git")`.
   - `settings/mod.rs` `resolve_user_home()` (lines 68–77): Under Flatpak calls `platform::host_std_command("getent")`. Native uses `Command::new("getent")`.
   - `export/diagnostics.rs` GPU section (lines 249–253): Under Flatpak calls `platform::host_std_command("lspci")`. Native uses `Command::new("lspci")`.
   - `launch/runtime_helpers.rs` `is_unshare_net_available()` (lines 816–830): Under Flatpak calls `platform::host_std_command("unshare")`. Native uses `std::process::Command::new("unshare")`.
   - `prefix_deps/runner.rs` `check_installed` / `install_packages` (lines 85–180): `binary_path` is a user-configured winetricks or protontricks path — a variable, not a literal — and `apply_host_environment` is called to thread the environment correctly.

4. **Test code** under `#[cfg(test)]` or `tests/` is unconditionally exempt.

---

## Enforcement

`scripts/check-host-gateway.sh` greps for `Command::new("<tool>")` literals across `crosshook-core` and `src-tauri` Rust source, where `<tool>` is one of the host-only tools that must always go through the gateway. The denylist (strict host-only tools, not general utilities) is:

```
proton
umu-run
gamescope
mangohud
winetricks
protontricks
gamemoderun
```

The script is wired into:

- `scripts/lint.sh` (the main lint runner)
- `.github/workflows/lint.yml` (CI, runs on every PR)
- `lefthook.yml` (local pre-commit hook)

---

## Maintenance contract

- Any new host tool added to `platform.rs` **must** also be added to the denylist in `scripts/check-host-gateway.sh`.
- The API table above **must** stay in sync with the actual `pub fn` / `pub unsafe fn` set in `platform.rs`. PR reviewers are responsible for verifying both.

---

## Monitoring signals (when to revisit)

| Signal                                                                      | Implication                                                                                                                                                      |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `flatpak/flatpak#5538` fine-grained `flatpak-spawn` command filtering lands | CrossHook can declare a bounded command allowlist rather than blanket `org.freedesktop.Flatpak`; `platform.rs` becomes the natural place to enumerate that list. |
| `umu` consolidates Proton/Wine launching                                    | CrossHook's dependency surface shrinks; the denylist may contract.                                                                                               |
| CrossHook gains a non-Linux target                                          | Sandbox assumptions no longer hold; `is_flatpak()` semantics need review.                                                                                        |

---

## References

- `docs/research/flatpak-bundling/06-archaeological.md` — per-tool architecture audit; confirms `platform.rs` as the single abstraction gateway
- `docs/research/flatpak-bundling/13-opportunities.md` — AO-1: protect the `platform.rs` abstraction
- `docs/research/flatpak-bundling/14-recommendations.md` — Phase 1 task 1.7: add ADR and CI lint
- `docs/research/flatpak-bundling/10-evidence.md` — Tier 1 claims #1, #8, #11: source-code-verified facts about CrossHook's architecture
- Issue #273 (this ADR); parent tracker #276 (Flatpak distribution)
