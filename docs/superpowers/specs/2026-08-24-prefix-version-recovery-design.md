# Prefix Windows-Version Recovery Design

## Problem

Winetricks verbs such as `dotnet48` temporarily change a Wine prefix's Windows compatibility version and may leave that change in place. Ghost of Tsushima consequently saw Windows 7 build 7601 and refused to start after its trainer dependency was installed. A best-effort in-memory restore is insufficient because CrossHook, Flatpak, or the machine can stop after the dependency tool changes the prefix but before restoration runs.

## Goals

- Preserve the prefix's effective Windows compatibility tier across dependency installation.
- Restore after both successful and unsuccessful dependency-tool exits.
- Recover safely after task cancellation, application shutdown, or machine restart.
- Keep dependency-install and compatibility-restore results distinct and visible.
- Prevent CrossHook UI launches while a prefix repair remains pending or failed.
- Continue routing all host tooling through the platform host-command gateway.

## Non-goals

- Transactionally undo DLL, font, registry, or runtime changes made by Winetricks.
- Repair arbitrary prefix corruption unrelated to CrossHook dependency installation.
- Add a generic Wine registry editor.
- Change exported launchers or external programs that bypass CrossHook's UI launch gate.

## Architecture

### Compatibility snapshot

`crosshook-core::prefix_deps::runner` reads the resolved prefix's `system.reg` before spawning the dependency tool. It parses `CurrentBuild`, `CurrentVersion`, and `ProductName` from `Software\\Microsoft\\Windows NT\\CurrentVersion` and maps them to a supported Winetricks version verb. Build numbers are authoritative for Windows 7 and newer (`>= 22000` is `win11`, `>= 10240` is `win10`, `9600` is `win81`, `9200` is `win8`, and `7600..=7601` is `win7`); product/version fields disambiguate older Windows and Server variants.

If CrossHook cannot derive a supported restore verb, dependency installation fails before mutation. If the requested package list explicitly contains a Windows-version verb, that change is intentional and no restore journal is created.

### Persistent repair journal

SQLite schema v27 adds `prefix_version_restore_journal`, keyed by normalized `prefix_path`:

- `prefix_path` and `profile_id`
- `binary_path`, `tool_type`, and nullable `steam_app_id`
- `restore_verb`
- `state`: `pending` or `failed`
- nullable sanitized `last_error`
- `created_at` and `updated_at`

The core metadata facade owns create/load/update/delete operations. CrossHook writes the `pending` row before spawning Winetricks/Protontricks. A row written before a spawn failure is harmless: restoring the captured version is idempotent, and recovery clears it.

### Install and restoration lifecycle

The runner separates preparation from spawn so persistence occurs first:

1. Validate package verbs and resolve the prefix.
2. Snapshot the compatibility tier and construct a repair record.
3. Persist the record as `pending`.
4. Spawn and stream the dependency process.
5. After exit, invoke the saved restore verb through the same host tool and environment.
6. Bound restoration to 90 seconds and terminate it on timeout.
7. Verify the resulting registry tier.
8. Delete the journal row only after verified restoration; otherwise persist `failed` with a sanitized error.

The completion outcome reports dependency status, exit code, restoration status, and restoration error separately. Dependency packages remain `installed` when installation succeeded even if restoration failed; the repair journal independently keeps the prefix degraded and launch-blocked.

If the background task or application disappears, the journal remains `pending`. The next status check or launch attempt discovers it and offers the same idempotent repair operation.

### Recovery and launch gating

Thin Tauri commands expose repair status and retry. The Prefix Dependencies panel displays a prominent `Repair required` state and a `Repair Prefix` button for pending/failed records, including the sanitized failure reason.

The existing frontend launch dependency gate checks repair state before checking missing packages. A pending/failed repair cannot be skipped: the user may retry or cancel the launch. Successful recovery refreshes dependency state and resumes the requested game/trainer launch. Existing profiles without a journal row behave unchanged.

## Error handling

- Snapshot failure: refuse installation before prefix mutation.
- Journal write failure: refuse installation before prefix mutation.
- Dependency failure: still attempt restoration, report the dependency failure, and preserve accurate package state.
- Restore process failure, timeout, or verification mismatch: persist `failed`, expose the reason, and block CrossHook UI launch.
- SQLite unavailable during ordinary game launch: retain existing fail-soft launch behavior unless an in-memory install from the same session is known to need repair. Dependency installation itself fails closed because it cannot establish the journal.

## Storage boundary and usability

- **SQLite operational metadata:** repair journal rows and sanitized restore errors. Migration v27 is additive and backward compatible.
- **TOML settings:** no changes.
- **Runtime-only state:** active child handles, stream readers, lock guards, and current progress.

The feature works offline once the dependency tool and cached installers are available. Recovery itself requires no network. Users can view and retry repairs in the profile's Prefix Dependencies panel but cannot edit journal internals. If persistence is unavailable, CrossHook refuses new dependency mutations rather than promising recovery it cannot provide.

## Tests

- Registry fixtures for Windows 7, 10, 11, and an unsupported/ambiguous version.
- Migration v26 to v27 plus fresh-schema coverage.
- Journal store lifecycle and uniqueness per normalized prefix.
- Installation success/failure both attempt restoration.
- Restoration timeout/failure produces a structured degraded outcome and retained journal row.
- Simulated interrupted installation leaves a row that a later recovery clears.
- Explicit Windows-version verbs do not create or undo a repair record.
- Flatpak command construction remains behind the host gateway.
- Frontend panel renders repair state and retries it.
- Launch gate blocks pending repair and resumes only after successful recovery.

## Documentation

Update the trainer-launch guide, quickstart, SQLite inventory, and schema version references to describe compatibility preservation and crash recovery.
