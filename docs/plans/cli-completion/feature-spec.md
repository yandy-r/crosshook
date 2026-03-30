# Feature Spec: CLI Completion

## Executive Summary

CrossHook's CLI binary (`crosshook-cli`) has full argument parsing but 6 of 7 commands are placeholders returning `not_implemented`, and the `launch` command only supports `steam_applaunch`. This feature wires all placeholders to their corresponding `crosshook-core` functions and extends `launch` to support `proton_run` and `native` methods — unblocking headless/scripted usage, CI integration, and Steam Deck console-mode workflows. All business logic already exists in `crosshook-core`; the work is pure wiring following the established `diagnostics export` pattern. The primary complexity is in the launch command's profile-to-`LaunchRequest` mapping for three distinct launch methods, plus two CRITICAL security findings (helper script path validation, import path containment) that must ship with mitigations.

## External Dependencies

### APIs and Services

No external APIs are needed. All functionality is provided by `crosshook-core` calling into local filesystem and Steam/Proton installations.

### Libraries and SDKs

| Library          | Version               | Purpose                         | Status                         |
| ---------------- | --------------------- | ------------------------------- | ------------------------------ |
| `clap`           | 4.x (derive)          | CLI argument parsing            | Already in use                 |
| `serde_json`     | 1.x                   | `--json` structured output      | Already in use                 |
| `tokio`          | 1.x (rt-multi-thread) | Async runtime, process spawning | Already in use                 |
| `crosshook-core` | path dep              | All business logic              | Already in use                 |
| `clap_complete`  | latest                | Shell completion generation     | Recommended addition (Phase 5) |

**No new dependencies are required for the core wiring work.**

### External Documentation

- [clap v4 derive tutorial](https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html): Subcommand dispatch patterns
- [Rain's Rust CLI Recommendations](https://rust-cli-recommendations.sunshowers.io/handling-arguments.html): Global flag patterns
- [CLI Guidelines (clig.dev)](https://clig.dev/): Output formatting, error handling, exit code conventions

## Business Requirements

### User Stories

**Primary User: Steam Deck Console-Mode User**

- As a Steam Deck user, I want to launch a configured game+trainer pair from the terminal so I can use CrossHook without the GUI
- As a Steam Deck user, I want `--json` output from any command so I can parse results in shell scripts or Decky Loader plugins

**Primary User: Power User / Linux Desktop User**

- As a Linux power user, I want `crosshook profile list` to enumerate my profiles so I can pipe the names into other scripts
- As a Linux power user, I want `crosshook steam discover` to tell me which Steam installations are found so I can diagnose detection issues
- As a Linux power user, I want `crosshook steam auto-populate --game-path <path>` to pre-fill Steam metadata so I don't have to manually find App IDs and compat paths

**Secondary User: Automation / CI**

- As a CI system, I want `crosshook launch --profile <name>` to return a non-zero exit code on failure for automated test pipelines
- As a scripter, I want `crosshook profile export` to produce a portable community profile JSON for sharing configurations
- As a scripter, I want `crosshook profile import --legacy-path <path>` to convert legacy `.profile` files for automated migrations

**Secondary User: System Administrator**

- As a sysadmin, I want `crosshook status` to show system diagnostics and profile summary for quick health assessment

### Business Rules

#### `crosshook status`

- Reports: profile count, profile store path, Steam root candidates, Proton installs, settings summary
- Read-only, never fails due to missing Steam — empty results are valid
- `--json` produces a structured object with typed sub-objects; `--verbose` adds diagnostic detail

#### `crosshook profile list`

- Calls `ProfileStore::list()` returning sorted profile names (`.toml` file stems)
- Human output: one name per line (pipe-friendly for `fzf`, `grep`, `xargs`)
- JSON output: `{"profiles": [...], "count": N, "profiles_dir": "..."}`
- Empty profiles directory returns empty list, not an error

#### `crosshook profile import`

- Calls `ProfileStore::import_legacy(legacy_path)` — reads legacy `.profile` JSON, converts to `GameProfile`, saves as TOML
- Profile name derived from file stem (e.g., `elden-ring.profile` → `elden-ring`)
- Silently overwrites existing profile with same name (existing core behavior)
- On success: reports saved profile name, source path, detected launch method

#### `crosshook profile export`

- Calls `export_community_profile(profiles_dir, profile_name, output_path)` — strips machine-specific paths, writes community JSON
- Requires `--profile` (from command arg or global flag)
- `--output` defaults to `<cwd>/<profile-name>.crosshook.json` when omitted
- Preserved fields: game name, Steam app ID, launch method, trainer kind

#### `crosshook steam discover`

- Calls `discover_steam_root_candidates()` + `discover_steam_libraries()` + `discover_compat_tools()`
- No required args — scans default Linux Steam paths
- Always exits 0 (empty result is informational, not an error)
- `--verbose` shows diagnostic strings from discovery functions

#### `crosshook steam auto-populate`

- Requires `--game-path` (path to game executable)
- Calls `attempt_auto_populate(&SteamAutoPopulateRequest)` — returns per-field states: `Found`, `NotFound`, `Ambiguous`
- Does NOT create or modify any profile — discovery only
- Human output shows field labels with state annotations; `Ambiguous` includes "set manually" hint

#### `crosshook launch` (full method support)

- **steam_applaunch**: Existing path via helper shell script — requires `app_id`, `compatdata_path`, `proton_path`
- **proton_run**: Direct Proton invocation via `build_proton_game_command()` — requires `runtime.prefix_path`, `runtime.proton_path`
- **native**: Direct Linux executable via `build_native_game_command()` — requires `game_path` (rejects `.exe` files)
- Method resolved via `resolve_launch_method(&profile)` or profile's explicit `launch.method`
- `launch::validate(&request)` must pass before process spawn
- Log streaming behavior identical across all methods (poll `/tmp/crosshook-logs/<slug>.log`)
- `--json` mode suppresses log streaming; emits final result JSON after process exits

### Edge Cases

| Scenario                                | Expected Behavior                                           | Notes                      |
| --------------------------------------- | ----------------------------------------------------------- | -------------------------- |
| Profile name with path separators       | `validate_name` rejects `/`, `\`, `:` — clear error message | Existing core validation   |
| Import overwrites existing profile      | Silent overwrite (existing `save()` behavior)               | Document in help text      |
| Export with no `--output`               | Defaults to `<cwd>/<name>.crosshook.json`                   | Must not panic on None     |
| Auto-populate on non-existent game path | Returns results (scans manifests by path pattern)           | File doesn't need to exist |
| Steam not installed                     | `discover` returns empty list, `status` shows "not found"   | Not an error               |
| `proton_run` with empty `proton_path`   | Validation emits `RuntimeProtonPathRequired` (Fatal)        | CLI surfaces as error      |
| `native` with `.exe` path               | Validation emits `NativeWindowsExecutableNotSupported`      | CLI surfaces as error      |
| No profiles directory                   | `ProfileStore::list()` returns empty Vec                    | Not an error               |

### Success Criteria

- [ ] All 7 CLI commands produce real output from crosshook-core functions
- [ ] `--json` flag produces valid, `jq`-parseable JSON for all commands
- [ ] `crosshook launch` supports all three launch methods (steam_applaunch, proton_run, native)
- [ ] All commands exit 0 on success, non-zero on error
- [ ] Errors print to stderr with actionable hints, not raw Rust error strings
- [ ] No regressions: existing `diagnostics export` and `steam_applaunch` launch continue to work
- [ ] CLI is documented in the quickstart guide
- [ ] `emit_placeholder()` function is deleted — no remaining stubs
- [ ] CRITICAL security findings (C-1, C-2) are mitigated before shipping

## Technical Specifications

### Architecture Overview

```
CLI Layer (main.rs)                    Core Layer (crosshook-core)
┌──────────────────────┐               ┌──────────────────────────┐
│ run()                │               │                          │
│  ├─ handle_status()  │──────────────▶│ ProfileStore::list()     │
│  ├─ profile list     │──────────────▶│ ProfileStore::list()     │
│  ├─ profile import   │──────────────▶│ ProfileStore::import_legacy() │
│  ├─ profile export   │──────────────▶│ export_community_profile()   │
│  ├─ steam discover   │──────────────▶│ discover_steam_root_candidates() │
│  │                   │──────────────▶│ discover_steam_libraries()    │
│  │                   │──────────────▶│ discover_compat_tools()       │
│  ├─ steam auto-pop   │──────────────▶│ attempt_auto_populate()  │
│  └─ launch           │               │                          │
│     ├─ steam_appl    │──────────────▶│ build_helper_command()   │
│     ├─ proton_run    │──────────────▶│ build_proton_game_command() │
│     └─ native        │──────────────▶│ build_native_game_command() │
└──────────────────────┘               └──────────────────────────┘
         │
         ▼
    Output: stdout (human/JSON)
    Errors: stderr
    Logs:   /tmp/crosshook-logs/
```

**Pattern**: Each handler follows the `diagnostics export` reference — construct store, call core function, branch on `global.json` for output formatting. No shared presentation layer, no new crates.

### Data Models

All types that cross the output boundary derive `Serialize` via serde. JSON schemas per command:

**`status`**:

```json
{
  "version": "0.2.4",
  "profiles": { "count": 3, "names": ["elden-ring", "cyberpunk-2077"] },
  "settings": { "auto_load_last_profile": true, "last_used_profile": "elden-ring" },
  "steam": {
    "roots": ["/home/user/.local/share/Steam"],
    "library_count": 2,
    "proton_installs": [{ "name": "GE-Proton-9-4", "path": "...", "is_official": false }]
  },
  "diagnostics": ["Default local Steam install: /home/user/.local/share/Steam"]
}
```

**`profile list`**: `{"profiles": ["name1", "name2"], "count": 3, "profiles_dir": "/path"}`

**`profile import`**: `{"imported": true, "profile_name": "elden-ring", "legacy_path": "/path", "launch_method": "steam_applaunch"}`

**`profile export`**: `{"exported": true, "profile_name": "elden-ring", "output_path": "/path/elden-ring.crosshook.json"}`

**`steam discover`**: `{"roots": [...], "libraries": [...], "proton_installs": [...], "diagnostics": [...]}`

**`steam auto-populate`**: Direct serialization of `SteamAutoPopulateResult` (already `Serialize`)

**`launch`** (on completion): `{"method": "proton_run", "exit_code": 0, "log_path": "/tmp/crosshook-logs/elden-ring.log"}`

### API Design

#### Core Function Signatures (CLI Must Call)

```rust
// profile/toml_store.rs
ProfileStore::list(&self) -> Result<Vec<String>, ProfileStoreError>
ProfileStore::load(&self, name: &str) -> Result<GameProfile, ProfileStoreError>
ProfileStore::import_legacy(&self, legacy_path: &Path) -> Result<GameProfile, ProfileStoreError>

// profile/exchange.rs
export_community_profile(profiles_dir: &Path, profile_name: &str, output_path: &Path)
    -> Result<CommunityExportResult, CommunityExchangeError>

// steam/discovery.rs
discover_steam_root_candidates(steam_client_install_path: impl AsRef<Path>, diagnostics: &mut Vec<String>)
    -> Vec<PathBuf>

// steam/libraries.rs (NOT re-exported from steam/mod.rs — import directly)
discover_steam_libraries(roots: &[PathBuf], diagnostics: &mut Vec<String>) -> Vec<SteamLibrary>

// steam/proton.rs
discover_compat_tools(roots: &[PathBuf], diagnostics: &mut Vec<String>) -> Vec<ProtonInstall>

// steam/auto_populate.rs
attempt_auto_populate(request: &SteamAutoPopulateRequest) -> SteamAutoPopulateResult

// launch/script_runner.rs
build_helper_command(request: &LaunchRequest, helper_script: &Path, log_path: &Path) -> Command
build_proton_game_command(request: &LaunchRequest, log_path: &Path) -> io::Result<Command>
build_native_game_command(request: &LaunchRequest, log_path: &Path) -> io::Result<Command>

// launch/mod.rs
validate(request: &LaunchRequest) -> Result<(), LaunchValidationError>
analyze(exit_status: Option<ExitStatus>, log_tail: &str, method: &str) -> LaunchReport
```

#### Key Implementation: `launch_request_from_profile()` (Replaces `steam_launch_request_from_profile()`)

```rust
fn launch_request_from_profile(profile: &GameProfile) -> Result<LaunchRequest, Box<dyn Error>> {
    let method = resolve_launch_method(profile);
    let steam_client_install_path = resolve_steam_client_install_path(&profile.steam.compatdata_path);

    Ok(LaunchRequest {
        method: method.to_string(),
        game_path: profile.game.executable_path.clone(),
        trainer_path: profile.trainer.path.clone(),
        trainer_host_path: profile.trainer.path.clone(),
        trainer_loading_mode: profile.trainer.loading_mode,
        steam: match method {
            METHOD_STEAM_APPLAUNCH => SteamLaunchConfig {
                app_id: profile.steam.app_id.clone(),
                compatdata_path: profile.steam.compatdata_path.clone(),
                proton_path: profile.steam.proton_path.clone(),
                steam_client_install_path: steam_client_install_path.to_string_lossy().into_owned(),
            },
            _ => SteamLaunchConfig {
                steam_client_install_path: steam_client_install_path.to_string_lossy().into_owned(),
                ..Default::default()
            },
        },
        runtime: match method {
            METHOD_PROTON_RUN => RuntimeLaunchConfig {
                prefix_path: profile.runtime.prefix_path.clone(),
                proton_path: profile.runtime.proton_path.clone(),
                working_directory: profile.runtime.working_directory.clone(),
            },
            METHOD_NATIVE => RuntimeLaunchConfig {
                working_directory: profile.runtime.working_directory.clone(),
                ..Default::default()
            },
            _ => RuntimeLaunchConfig::default(),
        },
        optimizations: LaunchOptimizationsRequest {
            enabled_option_ids: profile.launch.optimizations.enabled_option_ids.clone(),
        },
        launch_trainer_only: false,
        launch_game_only: true,
        profile_name: None,
    })
}
```

#### Launch Method Dispatch

```rust
let mut child = match method {
    METHOD_STEAM_APPLAUNCH => {
        let helper = scripts_dir.join(HELPER_SCRIPT_NAME);
        spawn_helper(&request, &helper, &log_path).await?
    }
    METHOD_PROTON_RUN => {
        let mut cmd = launch::script_runner::build_proton_game_command(&request, &log_path)?;
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        cmd.spawn()?
    }
    METHOD_NATIVE => {
        let mut cmd = launch::script_runner::build_native_game_command(&request, &log_path)?;
        cmd.stdout(Stdio::null()).stderr(Stdio::null());
        cmd.spawn()?
    }
    other => return Err(format!("unsupported launch method: {other}").into()),
};
```

### System Integration

#### Files to Modify

- **`crates/crosshook-cli/src/main.rs`**: Replace 6 `emit_placeholder()` calls with real handlers; refactor `launch_profile()` for multi-method; delete `emit_placeholder()` and `steam_launch_request_from_profile()`
- **`crates/crosshook-cli/src/args.rs`**: Add doc comments for `--help` output on all command variants (no structural changes)

#### Files to Create

None. All changes are in existing files.

#### Import Path Gotcha

`discover_steam_libraries` is NOT re-exported from `steam/mod.rs`. Import directly:

```rust
use crosshook_core::steam::libraries::discover_steam_libraries;
```

## UX Considerations

### User Workflows

#### Primary Workflow: Scripted Game Launch

1. **Verify system**: `crosshook status --json | jq '.steam.roots | length > 0'`
2. **Find profile**: `crosshook profile list` (pipe to `fzf` for selection)
3. **Launch**: `crosshook launch --profile elden-ring`
4. **Check result**: Exit code 0 = success, non-zero = failure

#### Primary Workflow: Profile Migration

1. **Import legacy**: `crosshook profile import --legacy-path ~/.config/crosshook-old/elden-ring.profile`
2. **Verify**: `crosshook profile list` (confirm new profile appears)
3. **Export for sharing**: `crosshook profile export --profile elden-ring --output /tmp/elden-ring.json`

#### Error Recovery Workflow

1. **Launch fails**: Non-zero exit code + diagnostic report to stderr
2. **User sees**: `error: proton_path is required for proton_run method` with `hint: set proton_path in your profile`
3. **Recovery**: Edit profile, re-launch

### Output Formatting

| Output type                            | Stream | When              |
| -------------------------------------- | ------ | ----------------- |
| Primary data (lists, JSON, log stream) | stdout | Always            |
| Progress indicators, status messages   | stderr | Default + verbose |
| Error messages and hints               | stderr | On failure        |
| Verbose debug info, diagnostics        | stderr | `--verbose` only  |

**`--json` mode**: All output is valid JSON to stdout; no spinners, no progress, no ANSI codes. Errors in JSON mode also emit structured JSON to stderr.

### Error Message Format

```
error: <what failed>
  hint: <actionable suggestion>
  hint: <second suggestion if applicable>
```

Example:

```
error: profile "eldin-ring" not found
  hint: run `crosshook profile list` to see available profiles
  hint: check your config directory with `crosshook status`
```

### Exit Codes

| Code | Meaning                                 |
| ---- | --------------------------------------- |
| 0    | Success                                 |
| 1    | General error (I/O, config parse)       |
| 2    | Usage error (invalid/missing arguments) |

### Accessibility Requirements

- Respect `NO_COLOR` environment variable
- Auto-detect TTY for color output
- Human output is plain-text scannable (no Unicode box-drawing required)
- `--json` flag for machine consumption

## Recommendations

### Implementation Approach

**Recommended Strategy**: Direct Core Calls (Thin Wrapper Pattern) — each handler instantiates stores and calls core functions inline, following the `diagnostics export` reference. No shared state, no abstractions, no new crates.

**Phasing:**

1. **Phase 1 — Simple Reads**: `profile list` + `status` (establishes output pattern, immediate scripting value)
2. **Phase 2 — Import/Export**: `profile import` + `profile export` (write operations using well-tested core functions)
3. **Phase 3 — Steam Discovery**: `steam discover` + `steam auto-populate` (filesystem-dependent, rich output formatting)
4. **Phase 4 — Launch Completion**: Refactor `launch_request_from_profile()`, wire `proton_run` + `native`, add `--dry-run`
5. **Phase 5 — Polish**: Exit codes, help text, shell completions, quickstart docs, delete `emit_placeholder()`

### Technology Decisions

| Decision              | Recommendation                                             | Rationale                                                               |
| --------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------- |
| Error handling        | `Box<dyn Error>` (existing)                                | Sufficient for thin CLI wrapper; `anyhow`/`thiserror` adds no value     |
| Output formatting     | `println!` with format specifiers                          | No table crate needed for `Vec<String>` lists                           |
| LaunchRequest builder | Single `launch_request_from_profile()` with internal match | Avoids duplication, uses `resolve_launch_method()` consistently         |
| Log streaming         | Reuse `stream_helper_log()` for all methods                | Consistent pattern, preserves log files for diagnostics                 |
| JSON schema stability | Document as unstable for v1                                | Allows iteration before locking down                                    |
| MetadataStore in CLI  | Skip for v1                                                | `status` uses profile count + Steam detection; avoids SQLite complexity |

### Quick Wins

- **Delete `emit_placeholder()`**: Dead code after all commands are wired
- **Deduplicate `resolve_steam_client_install_path()`**: CLI version at `main.rs:236` duplicates core — use `crosshook_core::launch::runtime_helpers::resolve_steam_client_install_path()`
- **Add doc comments to command variants**: Enables quality `--help` output with zero logic changes
- **Wire `build_launch_preview()` as `--dry-run`**: Already exists in core, trivial to expose

### Future Enhancements

- **Shell completions** via `clap_complete` (Bash/Zsh/Fish)
- **`crosshook profile show <name>`** for single-profile detail view
- **`--trainer-only` / `--game-only` CLI flags** (already supported in `LaunchRequest`)
- **`CROSSHOOK_PROFILE` env var** as alternative to `--profile` flag
- **Man page generation** via `clap_mangen`

## Risk Assessment

### Technical Risks

| Risk                                         | Likelihood | Impact | Mitigation                                                                |
| -------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------- |
| proton_run prefix resolution edge cases      | Medium     | High   | Test with both `pfx/` and standalone prefix styles                        |
| Missing helper scripts at runtime (AppImage) | Medium     | Medium | `--scripts-dir` override exists; `proton_run`/`native` don't need scripts |
| MetadataStore not available for status       | High       | Medium | Decided: skip for v1, use profile count + Steam detection                 |
| Launch log streaming partial lines in pipes  | Low        | Medium | Buffer by newline in `stream_helper_log()`                                |

### Integration Challenges

- **Profile-to-LaunchRequest mapping**: Current `steam_launch_request_from_profile()` only handles `steam_applaunch`. Must be refactored to a generic builder — the most complex single task
- **`discover_steam_libraries` import path**: Not re-exported from `steam/mod.rs`; must import directly from `steam::libraries`
- **Optimization directive resolution**: `resolve_launch_directives()` validates wrapper binaries exist on PATH; CLI must handle missing wrappers gracefully

### Security Considerations

#### Critical -- Hard Stops

| Finding                                                                | Risk                                                                     | Required Mitigation                                                                                                                     |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| C-1: Helper script path compile-time relative, not runtime-validated   | Arbitrary script execution if attacker places file at resolved path      | Verify script is regular file owned by current UID before invoking; long-term: embed via `include_bytes!` or use AppImage-relative path |
| C-2: `profile import --legacy-path` accepts arbitrary filesystem paths | Reads any user-accessible file; malicious profile can overwrite existing | Validate import path is a regular file (not symlink/device); warn on paths outside `~/.config/crosshook/`                               |

#### Warnings -- Must Address

| Finding                                                      | Risk                                            | Mitigation                                                                      | Alternatives                                  |
| ------------------------------------------------------------ | ----------------------------------------------- | ------------------------------------------------------------------------------- | --------------------------------------------- |
| W-1: Profile field values passed as raw args to shell script | Shell injection if script uses unquoted vars    | Audit `steam-launch-helper.sh` for unquoted usage; validate `app_id` is numeric | N/A                                           |
| W-2: Log path in `/tmp` is TOCTOU-susceptible                | Symlink attack, log injection on shared systems | Use `XDG_RUNTIME_DIR` with mode 0700 for log directory                          | Keep `/tmp` with symlink check                |
| W-4: `profile export --output` path not validated            | Overwrite arbitrary files if elevated           | Validate parent dir writable, no symlink, not in protected dirs                 | Require explicit `--force` for existing files |

#### Advisories -- Best Practices

- A-1: Replace `process::exit(1)` in `profile_store()` with `?` propagation (deferral OK)
- A-4: No lockfile for concurrent CLI invocations — acceptable for personal-use tool (deferral OK)
- A-6: Validate `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var contains `steam.sh` (deferral OK)

## Task Breakdown Preview

### Phase 1: Foundation and Simple Reads

**Focus**: Establish output pattern, deliver immediate scripting value

**Tasks**:

- Wire `crosshook profile list` handler
- Wire `crosshook status` handler (profile count + Steam detection + settings summary)
- Add doc comments to all command variants for `--help`
- Deduplicate `resolve_steam_client_install_path()` to use core version

**Parallelization**: All 4 tasks can run concurrently

### Phase 2: Import/Export

**Focus**: Profile lifecycle operations

**Dependencies**: Phase 1 establishes the output formatting pattern

**Tasks**:

- Wire `crosshook profile import` with legacy path validation (C-2 mitigation)
- Wire `crosshook profile export` with default output path and path validation (W-4 mitigation)

**Parallelization**: Both tasks can run concurrently

### Phase 3: Steam Discovery

**Focus**: Filesystem-dependent discovery commands

**Tasks**:

- Wire `crosshook steam discover` (roots + libraries + Proton installs)
- Wire `crosshook steam auto-populate` with field-state display formatting
- Design human-readable output for multi-field discovery results

**Parallelization**: Discover and auto-populate can run concurrently; output design depends on both

### Phase 4: Launch Completion

**Focus**: Most complex command — multi-method launch support

**Dependencies**: Phases 1-3 establish patterns

**Tasks**:

- Refactor `steam_launch_request_from_profile()` into generic `launch_request_from_profile()`
- Wire `proton_run` launch path via `build_proton_game_command()`
- Wire `native` launch path via `build_native_game_command()`
- Mitigate C-1 (helper script path validation)
- Mitigate W-2 (log path to `XDG_RUNTIME_DIR`)
- Add `--dry-run` flag using `build_launch_preview()`

**Parallelization**: Request builder refactor must complete first; proton_run and native can then run concurrently

### Phase 5: Polish and Documentation

**Focus**: Production readiness

**Tasks**:

- Standardize exit codes across all commands
- Update quickstart guide with CLI documentation
- Delete `emit_placeholder()` and dead code
- Add shell completion generation (`clap_complete`)
- Final `cargo test -p crosshook-core` + `cargo test -p crosshook-cli` pass

**Parallelization**: Docs and completions can run concurrently; exit codes and cleanup are sequential

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **MetadataStore scope for `status`**
   - Options: (A) Skip MetadataStore, show profile count + Steam detection only; (B) Initialize SQLite for launch history + health scores
   - Impact: Option B adds significant complexity for v1
   - Recommendation: **Option A** — defer MetadataStore to a future `status --full` enhancement

2. **Profile export format**
   - Options: (A) Community JSON via `exchange.rs` (sanitized, shareable); (B) Raw TOML copy (backup, machine-specific)
   - Impact: Community JSON is more valuable since TOML files can just be `cp`-ed
   - Recommendation: **Option A** — community JSON, matching the issue's reference to `export_community_profile()`

3. **Launch optimization flags**
   - Options: (A) Read optimizations from profile's saved preset only; (B) Accept `--optimization` CLI flags
   - Impact: Option B adds args complexity
   - Recommendation: **Option A** — read from profile; defer CLI override flags

4. **JSON schema stability**
   - Options: (A) Document as unstable/experimental for v1; (B) Version and guarantee stability
   - Impact: Premature stability guarantees constrain iteration
   - Recommendation: **Option A** — document as unstable, stabilize in a future release

5. **Trainer-only launch from CLI**
   - Options: (A) Game-only for v1 (`launch_game_only = true`); (B) Support `--trainer-only` flag
   - Impact: Trainer-only adds a flag + validation path
   - Recommendation: **Option A** — game-only for v1, add `--trainer-only` later

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Rust CLI libraries, core function signatures, code examples
- [research-business.md](./research-business.md): User stories, business rules per command, workflows, domain model
- [research-technical.md](./research-technical.md): Architecture design, JSON schemas, LaunchRequest mapping, system constraints
- [research-ux.md](./research-ux.md): Output formatting, error handling, exit codes, competitive analysis
- [research-security.md](./research-security.md): Security findings with severity levels (2 CRITICAL, 5 WARNING, 6 ADVISORY)
- [research-practices.md](./research-practices.md): KISS assessment, reusable code inventory, modularity, testability
- [research-recommendations.md](./research-recommendations.md): Implementation phasing, risk assessment, alternative approaches
