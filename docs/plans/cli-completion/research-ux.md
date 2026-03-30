# CLI UX Research: CrossHook Native CLI Completion

## Executive Summary

CrossHook's CLI targets three distinct personas: power users scripting game launches, CI/CD pipelines running headless, and Steam Deck users in console mode. The core UX challenge is designing output that is simultaneously human-friendly for interactive use and machine-friendly for piping and scripting — without building two separate codebases.

Industry consensus (clig.dev, Heroku CLI, gh, docker, kubectl) has converged on a single pattern: human-readable default output with a `--json` flag for structured output. Progress indicators go to stderr; primary data goes to stdout. This research confirms that pattern fits CrossHook's use case exactly.

The existing CLI skeleton in `crates/crosshook-cli/` already has `--json`, `--verbose`, and `--profile` global flags wired in `args.rs`. The `launch` command is the most complete implementation. The six remaining commands need consistent output contracts, not just placeholder stubs.

**Confidence**: High — based on clig.dev (industry canonical reference), gh/docker/kubectl design analysis, and review of the actual codebase.

---

## User Workflows

### Primary Flows

#### 1. Power User / Scripter

**Goal**: Automate game + trainer launch in a shell script or systemd unit.

```bash
# Typical scripted workflow
PROFILE="elden-ring"

# Verify system is ready
crosshook status --json | jq '.steam.found' | grep -q true || exit 1

# List profiles to confirm target exists
crosshook profile list --json | jq -r '.[].name' | grep -q "$PROFILE" || exit 1

# Launch
crosshook launch --profile "$PROFILE"
echo "Exit code: $?"
```

**Key requirements**:

- `--json` must emit valid, stable JSON to stdout — no mixing with progress output
- Exit codes must be reliable: 0 = success, non-zero = failure
- stderr messages must not contaminate stdout pipes
- No interactive prompts in non-TTY contexts (auto-detect via `isatty()`)

#### 2. CI / Headless Integration

**Goal**: Run smoke tests for profile validity or Steam discovery in CI.

```bash
# In a CI script
crosshook steam discover --json > steam-info.json
crosshook status --json > status.json
jq '.profiles.count' status.json
```

**Key requirements**:

- No spinners or progress bars (not a TTY)
- No color codes in output (`NO_COLOR` awareness, TTY detection)
- Deterministic exit codes for each failure mode
- Quiet mode (`--quiet` / `-q`) to suppress all non-data output

#### 3. Steam Deck Console Mode

**Goal**: Launch a game from Konsole (Desktop Mode) or a non-Steam entry in Gaming Mode.

```bash
# Simple launch from terminal
crosshook launch --profile "elden-ring-trainer"

# Debug mode when something goes wrong
crosshook launch --profile "elden-ring-trainer" --verbose
```

**Key requirements**:

- Spinner/progress feedback for operations that take >2 seconds (Steam discovery can be slow)
- Color output when TTY detected (Steam Deck Konsole supports color)
- Verbose mode for diagnosing launch failures without log diving
- Short, scannable output — Steam Deck screen is 800p in landscape

#### 4. Profile Migration / Onboarding

**Goal**: Import legacy profiles and auto-populate new ones from Steam games.

```bash
# Migrate old profiles
crosshook profile import --legacy-path ~/.config/crosshook-old/profiles/

# Discover what Steam games are available
crosshook steam discover

# Auto-populate a new profile from a game
crosshook steam auto-populate --game-path ~/.steam/steam/steamapps/common/EldenRing/eldenring.exe

# Review and list
crosshook profile list
```

**Key requirements**:

- `profile import` must report counts: N profiles imported, M skipped, K failed
- `steam discover` must show progress (scanning can iterate many library paths)
- `steam auto-populate` must show what was detected and ask for confirmation in interactive mode

### Alternative Flows

- **Dry-run**: `--dry-run` on `launch` to preview the command that would be executed without running it
- **Verbose diagnostics**: `--verbose` escalates output detail across all commands; pairs with `crosshook status` for system health checks
- **Non-interactive import**: `crosshook profile import --legacy-path /path --yes` skips confirmation prompts for batch migration

---

## UI/UX Best Practices

### Output Formatting Standards

**Rule: Humans first, machines via flag.**

Default output should be readable by a human scanning at a glance. `--json` opts into machine-readable structured output. This is the established convention used by gh, docker, kubectl, Heroku CLI, and Lutris (`-j` flag).

**`profile list` output — flat names, not a table:**

Default output is one profile name per line, unstyled. This enables direct piping:

```
elden-ring
dark-souls-3
hades
```

A table view (with method, health, etc.) is deferred to a future `--long` / `-l` flag. For v1, flat names are the right default — they work with `fzf`, `grep`, and `xargs` without any `--quiet` workaround.

**`steam auto-populate` output — field + state + hints:**

```
app_id:          42550                                                found
compatdata_path: /home/user/.steam/steam/steamapps/compatdata/42550  found
proton_path:     -                                                    ambiguous (set manually)

Hints:
  Multiple Proton installations found — set proton_path manually in your profile.
  Run `crosshook steam discover --verbose` to list available Proton versions.
```

Rules:

- `manual_hints` from `SteamAutoPopulateResult` are always shown (they are actionable guidance)
- `diagnostics` from `SteamAutoPopulateResult` are shown only under `--verbose` (internal discovery notes)
- "Ambiguous" state must include "set manually" inline — never just "(ambiguous)"

**`status` output — partial is better than failure:**

```
Profiles:  3  (/home/user/.config/crosshook/profiles)
Steam:     not found
           hint: run `crosshook steam discover --verbose` to diagnose
```

Always exits 0. Never fails because Steam is absent.

**JSON output for lists:**

```json
{ "profiles": ["elden-ring", "dark-souls-3", "hades"] }
```

Rules for JSON output:

- List commands wrap in a named object key, not a bare array (`{"profiles": [...]}` not `[...]`)
- Single-item commands emit a plain object
- All core result types (`SteamAutoPopulateResult`, `CommunityExportResult`, etc.) already derive `Serialize` — serialize directly, no custom mapping needed
- `status` emits a single top-level object with typed sub-objects

**Confidence**: High — this aligns with clig.dev guidelines and observed patterns in gh, docker, kubectl. JSON schema confirmed by business analysis and tech-designer review.

### Color Usage

- Green for success states
- Yellow for warnings
- Red for errors
- No color when:
  - stdout/stderr is not a TTY
  - `NO_COLOR` env var is set
  - `TERM=dumb`
  - `--no-color` flag passed

CrossHook should detect TTY via Rust's `is_terminal::IsTerminal` (or equivalent in the `console` crate) before emitting ANSI codes.

**Important**: When `--json` is active, strip all color codes unconditionally. JSON consumers do not want ANSI escape sequences in string values.

### Help Text Quality

Every command must have:

- A one-line `about` description used in `crosshook --help` listings
- A multi-line `long_about` for `crosshook <cmd> --help` with examples
- Examples lead, flags follow

Example for `crosshook launch`:

```
Launch a game with an associated trainer

Usage: crosshook launch [OPTIONS]

Options:
  --profile <NAME>   Profile to launch [env: CROSSHOOK_PROFILE]
  --dry-run          Print the launch command without executing
  --verbose          Stream launch log to stderr in real time
  --json             Emit launch result as JSON on exit

Examples:
  # Launch a profile
  crosshook launch --profile elden-ring

  # Preview what would be executed
  crosshook launch --profile elden-ring --dry-run

  # CI: capture structured result
  crosshook launch --profile elden-ring --json
```

**Clap-specific note**: Use `#[command(about = "...", long_about = "...")]` with `#[command(after_long_help = "Examples:\n...")]` for the example block.

### Global Flag Consistency

Every command must honor these global flags (already in `args.rs`):

| Flag               | Behavior                               |
| ------------------ | -------------------------------------- |
| `--json`           | Emit primary output as JSON to stdout  |
| `--verbose` / `-v` | Increase output detail; send to stderr |
| `--profile <NAME>` | Override active profile                |
| `--config <PATH>`  | Override config directory              |

Add to global flags:

| Flag             | Behavior                                |
| ---------------- | --------------------------------------- |
| `--quiet` / `-q` | Suppress all non-error output to stdout |
| `--no-color`     | Disable ANSI color codes                |

**Confidence**: High — clig.dev mandates these; they are table stakes.

---

## Error Handling UX

### Error Message Design

Errors should answer three questions: **what failed**, **why it failed**, and **what to do next**.

Bad:

```
Error: No such file or directory (os error 2)
```

Good:

```
error: profile "elden-ring" not found
  hint: run `crosshook profile list` to see available profiles
  hint: check your config directory with `crosshook status`
```

Pattern:

```
error: <what failed>
  hint: <actionable suggestion>
  hint: <second suggestion if applicable>
```

**Launch validation errors** use `LaunchValidationIssue` which has `message`, `help`, and `severity` fields. The `help` field maps directly to the `hint:` line. In human mode:

```
error: launch validation failed
  [fatal]   proton_path is required for proton_run method
            hint: set proton_path in your profile or pass a Proton installation path
  [warning] trainer path does not exist — game will launch without trainer
```

In `--json` mode, serialize the full `LaunchValidationIssue` array — not just `to_string()`. The current CLI collapses validation issues to plain strings; the structured `help` field must be preserved in JSON output.

For `--json` mode, all other errors must also be JSON:

```json
{
  "error": "profile not found",
  "code": "profile_not_found",
  "context": {
    "profile": "elden-ring"
  },
  "hints": ["run `crosshook profile list` to see available profiles"]
}
```

Emit JSON errors to **stderr**, not stdout, so pipelines don't confuse error objects with data. The convention used by tools like `jq` and `gh` is: errors always go to stderr, exit code signals failure.

### Exit Code Conventions

| Code | Meaning                                                  |
| ---- | -------------------------------------------------------- |
| 0    | Success                                                  |
| 1    | General error (config parse failure, IO error)           |
| 2    | Usage error (invalid arguments, missing required option) |
| 3    | Profile not found                                        |
| 4    | Launch failure (helper script non-zero exit)             |
| 5    | Steam not found or Steam discovery failed                |
| 6    | Validation warning treated as failure (with `--strict`)  |

**Rationale**: Codes 0-2 are universally understood (POSIX convention). Codes 3-6 are CrossHook-specific and enable scripted decision-making. Scripts can check `$?` to branch on specific failures without parsing text.

**Confidence**: Medium — specific code assignments are CrossHook design decisions; the 0/1/2 convention is high confidence; domain-specific codes (3-6) are reasonable proposals.

### Stderr vs Stdout Separation

| Output type                        | Stream |
| ---------------------------------- | ------ |
| Primary command data (lists, JSON) | stdout |
| Progress spinners, status updates  | stderr |
| Error messages and hints           | stderr |
| Verbose debug info                 | stderr |
| Log streaming (launch command)     | stdout |

**Critical for launch command**: The existing `stream_helper_log` in `main.rs` correctly writes to stdout. Error analysis (`launch::analyze`) writes to stderr. This pattern is correct.

### Verbose vs Quiet Modes

Three verbosity levels:

- `--quiet` / `-q`: Only emit primary data (stdout). Silence all stderr. Non-zero exit still works.
- Default: Primary data + brief status messages on stderr
- `--verbose` / `-v`: Primary data + detailed progress + debug context on stderr

**In practice for `launch`**:

- Quiet: Exit 0 silently on success, emit JSON error object to stderr on failure
- Default: "Launching elden-ring..." spinner, then "Done (2.3s)" or error summary
- Verbose: Full log streaming to stdout (current behavior in `stream_helper_log`)

### Actionable Suggestions

Implement "did you mean?" for profile names using Levenshtein distance. When `crosshook launch --profile eldin-ring` fails with "profile not found", check all known profile names and suggest the closest match within edit distance 3:

```
error: profile "eldin-ring" not found
  did you mean: elden-ring?
  hint: run `crosshook profile list` to see all profiles
```

This is low implementation cost (Rust has the `strsim` crate for Levenshtein) and significantly reduces friction for typos.

---

## Performance UX

### Progress Indicators

**Rule: Output something within 100ms. Anything taking >2s needs a progress indicator.**

Steam library discovery can be slow (multiple VDF parses across potentially large Steam libraries). Use the `indicatif` crate (already the Rust CLI standard) for progress indication.

Decision matrix:

| Operation             | Duration | Indicator                               |
| --------------------- | -------- | --------------------------------------- |
| `profile list`        | <100ms   | None                                    |
| `status`              | <500ms   | None                                    |
| `profile import`      | <1s      | Brief spinner                           |
| `profile export`      | <200ms   | None                                    |
| `steam discover`      | 1-10s    | Spinner with current path being scanned |
| `steam auto-populate` | 1-5s     | Spinner                                 |
| `launch`              | ongoing  | Log streaming (current impl)            |

For `steam discover`, the spinner should update its message with each library path being scanned:

```
  Scanning Steam libraries...
  ↳ /home/user/.steam/steam/steamapps (found 12 games)
  ↳ /mnt/games/steamapps (found 47 games)
  Done. Found 2 Steam installations, 59 games.
```

**Spinner goes to stderr**. This prevents contaminating stdout piped output.

When `--json` is active, suppress all progress indicators entirely. The consumer is a script and does not need spinners.

### Launch Feedback

The current `stream_helper_log` implementation streams the log file to stdout in a polling loop. This is correct. Two improvements needed:

1. **Timeout**: Add a configurable timeout (default 5 minutes) after which the helper is killed and an error is reported. Currently there is no timeout on the `loop` in `stream_helper_log`.

2. **Launch confirmation message**: In default mode (non-verbose, non-JSON), print a brief status line before spawning:

   ```
   Launching elden-ring (steam_applaunch)...
   ```

   This goes to stderr so it doesn't contaminate log output on stdout.

### Steam Discovery Performance

The `steam discover` command may scan multiple Steam library paths across potentially large directories. Recommendations:

- Show the current path being scanned in the spinner message
- Bail early on missing directories rather than stat-ing every subdirectory
- Report counts in real-time: "Scanning... (found 3 installations)"
- Set a configurable `--timeout` for the whole discovery operation (default 30s)

---

## Competitive Analysis

### Lutris CLI

Lutris has `--list-games` (`-l`) and `--json` (`-j`) flags. `--output-script` (`-b`) generates a standalone bash script. This is the closest analog to CrossHook's `launch` + export functionality.

**Lessons from Lutris**:

- `--json` as a global flag on list operations is the expected convention
- A "generate script without running" mode (`-b`) is valued by power users — CrossHook has this as `launchers export` in the GUI but not in the CLI
- Lutris does not have a `status` or diagnostics command — a gap CrossHook fills

**Confidence**: Medium — based on Lutris README and CLI help output analysis.

### Heroic Games Launcher

Heroic has `--no-gui` for headless operation and protocol-based game launching (`heroic://launch/AppName`). The CLI is minimal — Heroic is primarily a GUI app.

**Lessons from Heroic**:

- Protocol-based launch (`heroic://...`) is useful when games are registered as non-Steam entries
- `--no-gui` for headless is a common request; CrossHook's CLI-first design avoids needing this
- Heroic lacks scripting-friendly structured output — a clear differentiation opportunity for CrossHook

**Confidence**: Medium — based on GitHub issue analysis and documentation.

### gh (GitHub CLI)

gh is the gold standard for CLI UX in 2024. Key patterns:

- `--json` with `--jq` for inline filtering: `gh pr list --json number,title --jq '.[].title'`
- Default table output, `--json` for structured, `--template` for custom
- Errors always to stderr with contextual hints
- TTY-aware color (auto-disabled in pipes)
- Subcommand hierarchy: `gh pr create`, `gh issue list`, `gh repo clone`

**Lessons from gh applicable to CrossHook**:

- The `--json --jq <filter>` compound is powerful for one-liners; consider adding `--jq` as a convenience
- gh's table output auto-truncates columns based on terminal width — useful for `profile list` on a Steam Deck's narrower terminal
- gh shows `X of Y` counts in list headers: "Showing 5 of 12 profiles" — useful when profiles are numerous

**Confidence**: High — directly observable from gh documentation and source.

### kubectl

kubectl uses `-o json`, `-o yaml`, `-o wide` for output format. The `-o` flag is multi-valued (not just a boolean `--json`).

**Crosshook assessment**: CrossHook's boolean `--json` flag is simpler and sufficient. The multi-format approach only matters when YAML is also a first-class format (it is for kubectl's Kubernetes resources). CrossHook profiles are TOML internally but don't need to be exposed as YAML via CLI. Stick with `--json`.

**Confidence**: High.

### docker CLI

Docker CLI uses a mix of `--format` (Go template) and `--quiet` (IDs only). The `docker ps` table output is auto-aligned.

**Lessons from docker**:

- `--quiet` / `-q` outputting only names/IDs is extremely useful in scripts: `docker stop $(docker ps -q)`
- For CrossHook: `crosshook profile list --quiet` could output only profile names, one per line, enabling: `crosshook launch --profile $(crosshook profile list --quiet | fzf)`
- docker separates "container create" vs "container start" — model for CrossHook's distinction between profile management and launch

**Confidence**: High.

---

## Recommendations

### Must Have

1. **Consistent `--json` output contract**: Every command emits either an array (for lists) or an object (for single items). Errors in JSON mode also go to stderr as JSON objects with `error`, `code`, and `hints` fields.

2. **Exit codes 0-6 as documented above**: Scripts cannot rely on text parsing; exit codes are the stable API.

3. **Stderr/stdout discipline**: All progress, spinners, status messages, and errors go to stderr. Only primary data goes to stdout. This is already correct in the `launch` command; enforce it in the remaining six commands.

4. **TTY-aware color**: Use `is_terminal::IsTerminal` (or the `console` crate) to auto-disable color when not a TTY. Respect `NO_COLOR` env var.

5. **`--quiet` / `-q` global flag**: Suppress all non-data stderr output. Essential for CI and script use.

6. **Spinner for `steam discover`**: This operation can take several seconds. Without feedback it looks broken. Spinner writes to stderr; suppressed when `--json` or `--quiet`.

7. **Error message format with hints**: Every error message follows the `error: <what> / hint: <action>` pattern. Avoids raw Rust error strings like `No such file or directory (os error 2)` reaching the user.

### Should Have

8. **"Did you mean?" for profile names**: Use `strsim` crate Levenshtein distance to suggest close profile names when lookup fails. Low cost, high impact.

9. **`--quiet` on `profile list` outputs names only**: One name per line, no table headers. Enables `fzf` piping patterns.

10. **`--dry-run` on `launch`**: Print the full launch command that would be executed (game path, helper script path, env vars) without running it. Essential for debugging.

11. **Timeout on `launch`**: Default 5-minute timeout on the helper process. Currently unbounded.

12. **Help examples block**: Each command's `--help` output should include 2-3 copy-paste examples. Use clap's `after_long_help` for this.

13. **`profile list` table with HEALTH column**: Show health status inline so users can spot broken profiles without running a separate command.

### Nice to Have

14. **`--jq <filter>` convenience flag**: Combine `--json` with an inline jq filter. `crosshook profile list --json --jq '.[].name'`. Avoids requiring users to have `jq` installed for simple cases.

15. **Terminal width awareness on table output**: Auto-truncate columns to fit terminal width. Useful on Steam Deck's Konsole.

16. **`CROSSHOOK_PROFILE` env var support on `launch`**: Allow `export CROSSHOOK_PROFILE=elden-ring` as an alternative to `--profile` in scripts. The arg parser in `args.rs` can be extended with `#[arg(env = "CROSSHOOK_PROFILE")]`.

17. **`crosshook status --json` schema stability guarantee**: Document the JSON schema for `status` output so users can depend on it. Breaking changes require a major version bump.

---

## Open Questions

1. **Should `steam auto-populate` be interactive by default?** It detects game info and populates a profile — should it prompt for confirmation before writing, or write immediately and show a diff? Recommendation: write immediately in non-TTY, prompt in TTY. But this needs confirmation from the business logic team.

2. **Should `profile export` export to community format or just serialize the TOML?** The `--output` flag exists but the format is undefined in `args.rs`. This affects what the JSON output schema looks like.

3. **What is the expected output of `crosshook status`?** The placeholder stub is the only implementation. Needs definition: Steam installation info? Profile count? DB health? Launch history summary? All of the above?

4. **Launch log streaming in JSON mode**: Currently `stream_helper_log` writes raw log text to stdout. In `--json` mode, should this be wrapped in a JSON envelope `{"type":"log","line":"..."}` for structured streaming, or should JSON mode only emit a final result object after the process exits?

5. **`profile import --legacy-path` — directory or single file?** The `PathBuf` argument suggests either. Clarify whether it imports a single profile file or all profiles in a directory.

---

## Sources

- [Command Line Interface Guidelines (clig.dev)](https://clig.dev/) — canonical reference for modern CLI UX
- [CLI UX Best Practices: 3 Patterns for Progress Displays (Evil Martians)](https://evilmartians.com/chronicles/cli-ux-best-practices-3-patterns-for-improving-progress-displays)
- [UX Patterns for CLI Tools (Lucas F. Costa)](https://lucasfcosta.com/2022/06/01/ux-patterns-cli-tools.html)
- [Heroku CLI Style Guide](https://devcenter.heroku.com/articles/cli-style-guide)
- [GitHub CLI Formatting Reference](https://cli.github.com/manual/gh_help_formatting)
- [GitHub CLI JSON Output Discussion](https://github.com/cli/cli/issues/1089)
- [kubectl Output Formatting Guide (Baeldung)](https://www.baeldung.com/ops/kubectl-output-format)
- [Table Formatting in GitHub CLI 2.0](https://heaths.dev/tips/2021/08/24/gh-table-formatting.html)
- [12 Rules of Great CLI UX (DEV Community)](https://dev.to/chengyixu/the-12-rules-of-great-cli-ux-lessons-from-building-30-developer-tools-39o6)
- [indicatif Rust crate documentation](https://docs.rs/indicatif/latest/indicatif/)
- [Heroic Games Launcher --no-gui PR](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/pull/1362)
- [Lutris CLI options (GitHub README)](https://github.com/lutris/lutris/blob/master/README.rst)
- [Running shell scripts in Steam Deck Gaming Mode (GitHub Gist)](https://gist.github.com/pudquick/981a3e495ffb5badc38e34d754873eb5)
- [Feature Request: Official Headless Mode for Steam Client](https://github.com/valvesoftware/steam-for-linux/issues/12153)

---

## Search Queries Executed

1. `CLI UX best practices 2024 output formatting human readable JSON structured`
2. `CLI error handling UX exit codes conventions best practices stderr stdout separation`
3. `Lutris CLI command line interface output format game launcher headless scripting`
4. `gh cli docker kubectl CLI output format table JSON progress indicator UX patterns 2024`
5. `Steam Deck console mode gaming CLI terminal workflow headless game launcher`
6. `heroic games launcher CLI interface command line options scripting headless 2024`
7. `CLI progress bar spinner Rust indicatif library output formatting clap 2024`
8. `CLI subcommand design patterns profile list status command output table format scripting friendly 2024`
9. `CLI error messages actionable suggestions did you mean Levenshtein distance similar command UX pattern`
10. `CLI --json output design machine readable structured data piping jq filter scripting conventions`
11. `game launcher CLI status command system diagnostics output format examples steam linux`
