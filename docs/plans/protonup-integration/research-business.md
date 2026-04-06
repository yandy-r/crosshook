## Executive Summary

CrossHook currently detects installed Proton runtimes and validates configured paths, but it does not help users install missing versions. The ProtonUp integration should close that gap by letting users discover installable Proton variants, install them in-app, and receive actionable suggestions when community profiles reference versions that are not installed. The feature must remain non-blocking for launch workflows whenever a valid local runtime exists.

## User Stories

- As a player, I want to see if a community-required Proton version is missing so I can resolve launch setup quickly.
- As a player, I want to install GE-Proton or Wine-GE from CrossHook without leaving the app.
- As an advanced user, I want to ignore suggestions and continue with my chosen runtime when it is valid.
- As an offline user, I want CrossHook to show local installed runtimes and avoid false errors from unavailable network data.

## Business Rules

- Community `proton_version` metadata is advisory and should drive suggestions, not hard launch blocking by default.
- Launch remains blocked only for existing fatal conditions (for example, configured runtime path is invalid for the selected launch method).
- Install actions must be explicit user actions with clear destination path and risk messaging.
- Remote version listings may be cached, but stale or missing network data must not prevent using local installed runtimes.
- Persisted data must follow project boundaries:
  - user preferences in TOML settings,
  - operational/cache records in SQLite metadata,
  - install progress as runtime-only state.

## Workflows

- Primary flow:
  - user opens Proton management UI,
  - app loads installed runtimes and available installable versions,
  - user selects version and confirms install,
  - app shows progress and completion with refresh of installed list.
- Community suggestion flow:
  - profile metadata includes `proton_version`,
  - app compares required string against installed runtimes,
  - app shows install suggestion or version mismatch warning with continue option.
- Recovery flow:
  - if install fails, app shows cause and retry guidance,
  - if offline, app falls back to installed-only mode and cached list when present.

## Domain Concepts

- Installed runtime inventory: runtimes found from Steam compatibility tool directories.
- Community-required runtime: `proton_version` value from community profile metadata.
- Effective runtime selection: runtime path actually used for current profile launch.
- Suggestion status: matched, missing, unknown mapping, or stale-data state.
- Install operation: user-confirmed action that downloads and installs a target runtime.

## Success Criteria

- Available Proton versions are listed for installation from inside CrossHook.
- Selected Proton versions can be installed successfully from CrossHook.
- Community profile runtime requirements trigger install suggestions when missing.
- Offline or API failures degrade gracefully without breaking valid launch paths.
- User-facing messaging distinguishes advisory mismatches from hard validation failures.

## Open Questions

- Should v1 support GE-Proton only, or both GE-Proton and Wine-GE from day one?
- Should suggestion matching be strict exact-name, normalized alias matching, or nearest-compatible?
- Should there be an optional strict mode that blocks launch when community runtime requirements are not met?
- Should dismiss/snooze of suggestions be persisted globally, per profile, or per community tap?
