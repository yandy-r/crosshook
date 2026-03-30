# Custom Env Vars: External Research

## Objective

Collect implementation patterns from Linux launcher ecosystems and low-level runtime references relevant to issue `#57`:

- Profile-level custom environment variables
- Merge and precedence with generated optimization env vars
- Validation and portability constraints

## Industry Patterns

### Structured key/value editing beats free-form command strings

- Heroic and Bottles use explicit key/value env configuration and separate env vars from command wrappers.
- This avoids `%command%` confusion and reduces malformed launch option strings.

Practical takeaway for CrossHook:

- Keep custom env vars as structured profile data (`key` + `value`), not shell fragments.
- Render shell-like output only for preview/copy surfaces.

### Deterministic precedence is required

- Launcher users frequently combine defaults, optimization toggles, and custom per-game overrides.
- Ambiguous merge rules cause support churn and "works in preview but not at runtime" regressions.

Practical takeaway for CrossHook:

- Enforce one global rule: `runtime/base env` -> `optimization env` -> `custom env` (custom wins).
- Use one shared merge path for runtime launch and preview.

### Steam launch options are fragile if built ad hoc

- Proton guidance and Steam issues show `%command%` line construction can regress when quoting/order changes.

Practical takeaway for CrossHook:

- Build Steam launch-option strings from the same merged env source used by runtime launch code.
- Never maintain a separate precedence implementation for Steam output.

## Validation and Runtime Constraints

### POSIX/runtime constraints

- Duplicate env keys are non-portable/ambiguous.
- Keys cannot be empty and cannot contain `=`.
- NUL is not allowed in keys/values.

Practical takeaway for CrossHook:

- Authoritative validation in Rust, not only frontend.
- Deduplicate keys before process spawn.

### Recommended key policy

- Hard requirement: valid process-env key (`non-empty`, no `=`, no NUL).
- Advisory recommendation: uppercase underscore pattern (`^[A-Z_][A-Z0-9_]*$`) for user clarity and portability.

## Suggested External-Informed UX

- Two-column editable table (Key / Value).
- Inline validation and duplicate-key detection.
- Clear precedence hint in UI copy.
- Effective env preview showing source and winning value.

## External References

- [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
- [Proton debugging docs](https://raw.githubusercontent.com/ValveSoftware/Proton/proton_10.0/docs/DEBUGGING-LINUX.md)
- [steam-for-linux #13012](https://github.com/ValveSoftware/steam-for-linux/issues/13012)
- [Heroic env vars wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Environment-Variables)
- [Heroic PR #1533](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/pull/1533)
- [Heroic PR #1541](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/pull/1541)
- [Lutris issue #5115](https://github.com/lutris/lutris/issues/5115)
- [Bottles CLI docs](https://docs.usebottles.com/advanced/cli)
- [POSIX env vars](https://pubs.opengroup.org/onlinepubs/9799919799.2024edition/basedefs/V1_chap08.html)
- [Rust process Command::env docs](https://doc.rust-lang.org/std/process/struct.Command.html#method.env)
