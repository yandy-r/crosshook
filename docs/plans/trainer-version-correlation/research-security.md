# Security Research: Trainer-Version-Correlation

**Feature**: Trainer and game version correlation with mismatch detection
**Scope**: Local desktop app (Linux AppImage, Rust/Tauri/React)
**Reviewer**: security-researcher agent
**Date**: 2026-03-29
**Revision**: 2 — updated with teammate findings from tech-designer, business-analyzer, practices-researcher, recommendations-agent

---

## Executive Summary

The trainer-version-correlation feature has **no CRITICAL security blockers**. This is a local desktop application where the user owns the filesystem and runs the app as themselves — the threat model is fundamentally different from a networked service. The existing codebase already has strong security posture: parameterized SQLite queries throughout, symlink detection on the DB file, 0o700/0o600 permission enforcement, and A6 bounds checking on community metadata fields.

Three **WARNING** issues require attention before shipping:

1. `game_version` and `trainer_version` are missing from the existing `check_a6_bounds()` function — the other metadata fields are bounded, these two are not. The version correlation feature makes these fields more prominent, so the gap should be closed.
2. `pinned_commit` in community tap subscriptions is passed to `git checkout` without SHA format validation — a `-flag`-shaped commit hash would be processed by git directly.
3. **Architectural decision**: If community tap version data is elevated from display-only to actively suppressing or triggering mismatch warnings, the trust model changes. Community data should remain display-only; local + Steam data should control any behavioral outcomes (blocking launches, showing alerts).

The remaining findings are **ADVISORY** — good practices worth adopting but not blocking.

---

## Findings by Severity

### WARNING — Must Address Before Shipping

| ID  | Title                                                                                 | Location                                        | Mitigation                                                                             |
| --- | ------------------------------------------------------------------------------------- | ----------------------------------------------- | -------------------------------------------------------------------------------------- |
| W1  | `game_version` / `trainer_version` unbounded in A6 check                              | `metadata/community_index.rs:check_a6_bounds()` | Add `MAX_VERSION_BYTES = 256` bounds for both fields                                   |
| W2  | `pinned_commit` passed to git subprocess without SHA validation                       | `community/taps.rs:checkout_pinned_commit()`    | Validate commit hash matches `[0-9a-fA-F]{7,64}` before passing to git                 |
| W3  | Community version data must not control behavioral outcomes (suppress/trigger alerts) | Architecture decision — no specific file yet    | Community data = display only; only local + Steam data drives warnings / launch blocks |

### ADVISORY — Best Practice, Safe to Defer

| ID  | Title                                                            | Location                                                        | Mitigation                                                                                        |
| --- | ---------------------------------------------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| A1  | VDF `buildid` field not validated as numeric before storage      | New code extracting `buildid`                                   | Filter: keep only if all chars are ASCII digits                                                   |
| A2  | `normalize_path` does not canonicalize symlinks                  | `steam/manifest.rs`, `steam/discovery.rs`, `steam/libraries.rs` | Acceptable as-is — Steam cross-device libraries commonly use symlinks                             |
| A3  | No symlink check on ACF manifest files (only DB path is checked) | `steam/manifest.rs:safe_manifest_paths()`                       | Acceptable — user-controlled filesystem; log path in diagnostics                                  |
| A4  | TOCTOU between manifest existence check and read                 | `steam/manifest.rs:parse_manifest()`                            | Negligible for local desktop; caught by `fs::read_to_string` error                                |
| A5  | Version comparison must not panic on malformed input             | New comparison logic                                            | Use `Result`-returning semver parse; never `.unwrap()` on untrusted strings                       |
| A6  | Version strings should normalize whitespace before comparison    | New comparison logic                                            | `.trim()` both sides; document that trailing whitespace is ignored                                |
| A7  | Version history tables need row-count retention limits           | New `version_records` / `version_mismatch_events` tables        | Cap rows per profile (e.g., 50 most recent); prune on insert to prevent unbounded DB growth       |
| A8  | DB failure must not block game launch                            | All new version correlation DB calls                            | Wrap in `is_available()` guard; log on error; version check is informational, never launch-gating |

---

## Filesystem Access Security

**Relevant files**: `steam/manifest.rs`, `steam/discovery.rs`, `steam/libraries.rs`

### What the code does well

- `safe_manifest_paths()` filters strictly to files matching `appmanifest_*.acf` — no other files in `steamapps/` are read.
- `path_is_same_or_child()` verifies the game's resolved install directory is within the library root before accepting a manifest match. This prevents a crafted manifest with a wildly incorrect `installdir` from matching an unrelated path.
- Library deduplication via `HashSet<String>` prevents re-scanning the same path twice.
- The existing VDF parser is a hand-written recursive descent parser — it does not eval or execute anything; it only builds an in-memory tree of key/value nodes.

### For version correlation specifically

The `buildid` field in appmanifest ACF files is already in the VDF tree structure — it lives under `AppState.buildid`. Extraction requires only an additional `.get_child("buildid")` call in `parse_manifest()` (or a new helper). The VDF parser handles this safely.

**Steam build IDs are always numeric** (e.g., `"11651527"`). The VDF parser will read any string value, so the new code should validate the extracted `buildid` before storage:

```rust
// Advisory A1: validate buildid is numeric
let build_id = app_state_node
    .get_child("buildid")
    .and_then(|node| node.value.as_ref())
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit()))
    .unwrap_or_default();
```

### Path traversal assessment

**Confidence: High** — No path traversal risk from ACF scanning. The steamapps root is a known, validated directory. `safe_manifest_paths()` reads only the first directory level (non-recursive for ACFs). `path_is_same_or_child()` is the containment guard that prevents a malicious `installdir` value from matching an arbitrary location.

The one gap: `normalize_path()` only trims whitespace — it does not call `fs::canonicalize()`. This means symlinks within a Steam library path are followed transparently. This is intentional and correct: Steam users commonly symlink their library folders across drives. The app runs as the user and has no elevated privilege, so this is not a security concern (advisory A2).

---

## Data Integrity (SQLite)

**Relevant files**: `metadata/db.rs`, `metadata/migrations.rs`, `metadata/community_index.rs`

### What the code does well

`db.rs` has a strong security posture for a local desktop application:

- **Symlink detection**: `symlink_metadata()` is called before opening the DB; a symlink at the DB path returns `MetadataStoreError::SymlinkDetected` immediately.
- **Permission hardening**: Parent directory set to `0o700`, file set to `0o600` after open.
- **WAL mode + integrity**: `journal_mode=WAL`, `foreign_keys=ON`, `synchronous=NORMAL`, `busy_timeout=5000`, `secure_delete=ON`.
- **Integrity check**: `PRAGMA quick_check` runs on every open; a corrupted database fails fast rather than silently.
- **Application ID**: `0x43484B00` marks the DB as CrossHook-owned.
- **Parameterized queries everywhere**: All `INSERT`, `UPDATE`, `SELECT` use `params![...]` — no string interpolation of user data into SQL.

### Migration safety

All migrations use `execute_batch()` with static SQL strings. No user data is interpolated into migration DDL. The `migrate_4_to_5` migration uses an explicit `BEGIN TRANSACTION ... COMMIT` with a table rename pattern — this is the correct approach for SQLite schema changes.

For the new feature, the version correlation migration should follow the same pattern: static DDL, explicit transaction, `user_version` bumped only after the migration completes.

### For version correlation specifically

New columns (e.g., `game_build_id TEXT`, `trainer_version_tag TEXT`) added to `community_profiles` or a new `version_correlation` table will be safe as long as:

1. Writes use `params![]` (existing convention — enforce this in review).
2. No raw string formatting of version values into SQL.
3. The migration is wrapped in a transaction if it's a multi-step change.

**SQL injection risk**: None — rusqlite's parameterized query API makes SQL injection structurally impossible for the INSERT/SELECT operations. The `game_version` and `trainer_version` strings from community taps flow into `params![]` arguments, not query text.

---

## Community Data Trust

**Relevant files**: `community/taps.rs`, `community/index.rs`, `profile/community_schema.rs`, `metadata/community_index.rs`

### Trust model

Community taps are git repositories cloned by the user. The user chooses which taps to subscribe to. The content inside a tap (JSON manifests) is authored by whoever maintains the tap. This is analogous to a package repository: you trust the tap author to the degree you trust the repository.

**What the app does with community data**:

- Parses `community-profile.json` via `serde_json` into `CommunityProfileManifest`
- Stores metadata fields (game_name, game_version, trainer_version, etc.) into SQLite via parameterized queries
- Displays these fields in the React UI as React text nodes (not rendered HTML)
- Does NOT pass community data to subprocess arguments or shell commands

### The A6 bounds gap (WARNING W1)

`check_a6_bounds()` in `community_index.rs` already bounds five fields:

```
MAX_GAME_NAME_BYTES: 512
MAX_DESCRIPTION_BYTES: 4_096
MAX_PLATFORM_TAGS_BYTES: 2_048
MAX_TRAINER_NAME_BYTES: 512
MAX_AUTHOR_BYTES: 512
```

`game_version` and `trainer_version` are **not** bounded. With version correlation making these fields active participants in comparisons and UI display, a malicious or broken tap entry with a 1 MB version string would be stored in SQLite and loaded into memory on every comparison. This is not a crash risk (SQLite TEXT is arbitrary length), but it is a resource waste that the bounds pattern was designed to prevent.

**Recommended fix**: Add `MAX_VERSION_BYTES: usize = 256` and check both fields in `check_a6_bounds()`.

### pinned_commit validation (WARNING W2)

```rust
// taps.rs:checkout_pinned_commit
self.run_git(
    workspace,
    "checkout pinned commit",
    &["checkout", "--detach", pinned_commit],  // ← pinned_commit is user-controlled
)?;
```

`normalize_subscription()` trims whitespace and rejects whitespace-containing values, but does not validate that `pinned_commit` is a valid git SHA format. A value like `-q` or `--force` would be passed as a positional argument to git. Because Rust's `Command::arg()` does not use shell parsing, the value cannot escape to a new shell command — but git itself interprets argument strings starting with `-` as flags.

In practice, git would likely reject the invalid ref and return an error, which `run_git` would propagate as `CommunityTapError::Git`. But this is still a code quality issue and a surprising failure mode.

**Recommended fix**: Validate the format before calling git:

```rust
fn is_valid_git_sha(commit: &str) -> bool {
    let trimmed = commit.trim();
    (7..=64).contains(&trimmed.len())
        && trimmed.chars().all(|c| c.is_ascii_hexdigit())
}
```

Return `CommunityTapError::InvalidTapUrl` (or a new `InvalidPinnedCommit` variant) early if validation fails.

### XSS / injection via version strings in UI

React renders string values as `textContent`, not `innerHTML`. A `game_version` value of `<script>alert(1)</script>` would display literally as that text string in the UI. **No XSS risk.** Confidence: High.

### Community version data influencing behavioral outcomes (WARNING W3)

_Source: recommendations-agent review._

The existing `compatibility_rating` field in `CommunityProfileManifest` is display-only — it shows a badge in the UI but does not block or modify launch behavior. The version correlation feature introduces the possibility of community-supplied version data influencing outcomes: a tap could claim "this version combination is compatible" (suppressing a mismatch warning) or conversely mark a combination as broken (triggering a warning even when local conditions are fine).

**If community data is elevated to behavioral control, the trust model changes materially:**

A malicious or compromised tap author could:

- Suppress mismatch warnings for a known-broken version combination
- Trigger false-positive warnings to annoy users or confuse them into using a different trainer

**Required design constraint**: Community version data must remain informational metadata, exactly as `compatibility_rating` is today. The comparison logic must be:

```
local_build_id (from Steam ACF) vs. community game_version (from tap manifest) → display result only
```

Never:

- Use community data to gate a launch
- Use community data to suppress a locally-computed mismatch
- Use community data to override a user's explicit action

This is an architectural constraint that must be decided before implementation begins. If the spec calls for community data to influence behavior, that decision should be explicit and documented with the reasoning.

---

## Input Validation

### Build ID format (Advisory A1)

Steam `buildid` values from appmanifest ACF files are always unsigned decimal integers (e.g., `11651527`). The new extraction code should enforce this with an `ascii_digit`-only filter. This prevents a corrupted or manually-edited manifest from storing a non-numeric string in a column semantically meant for a build ID.

### Trainer version strings (Advisory A5, A6)

Trainer version strings (from community profiles, e.g., `"v1"`, `"1.2.3"`, `"FLiNG-1234"`) are not standardized. They are free-form strings set by the trainer distributor. The version correlation comparison logic must:

1. **Not panic on malformed input.** If a semver crate is used, use the `Result`-returning `Version::parse()` — not the panicking variant.
2. **Fall back gracefully.** If the version string cannot be parsed as semver, fall back to string equality comparison.
3. **Normalize whitespace** before comparison (advisory A6): `version.trim()` prevents `"1.0 "` vs `"1.0"` false mismatches.

### Version field presence

The `game_version` and `trainer_version` fields default to empty string in `CommunityProfileMetadata` (via `#[serde(default)]`). A comparison against a locally-discovered version should treat an empty community version as "unknown / unspecified" — not as a mismatch against a real version. Document this behavior clearly.

---

## Trainer Binary Hashing

_Source: practices-researcher — `sha2` crate already in use in `profile_sync.rs`._

If the feature includes hashing the trainer executable to detect file changes (e.g., to distinguish "same version, binary replaced" from "no change"), the `sha2` crate is already a dependency and is safe to reuse.

**Trust boundary assessment**: The trainer file path comes from `profile.trainer.path` — a user-configured field. The user chose this file; the app hashes it on their behalf. No new trust boundary is introduced. This is equivalent to how the profile content hash already works.

**Constraints**:

- The hash is a fixed 64-character hex string (SHA-256). Store it in a `TEXT NOT NULL` column with an explicit length check: `hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())`.
- Do not expose the raw hash in user-facing UI — it is an internal change-detection signal, not a meaningful version identifier to the user.
- Hash is read-only metadata; it must not be used to make launch decisions.

---

## Dependency Security

**Confidence: High** — All current crosshook-core dependencies are well-maintained, current releases.

| Crate                  | Version        | Status  | Notes                                                                     |
| ---------------------- | -------------- | ------- | ------------------------------------------------------------------------- |
| `rusqlite`             | 0.38 (bundled) | Current | Bundles SQLite 3.x; bundled feature avoids system SQLite version variance |
| `serde` / `serde_json` | 1.x            | Current | Standard, pervasive                                                       |
| `toml`                 | 0.8            | Current | Profile TOML parsing                                                      |
| `sha2`                 | 0.10           | Current | RustCrypto family; well-audited                                           |
| `chrono`               | 0.4            | Current | Timestamps                                                                |
| `uuid`                 | 1.x            | Current | Profile IDs                                                               |
| `directories`          | 5.x            | Current | XDG paths                                                                 |

### Potential new dependencies for version correlation

**`semver` crate** (if version comparison uses semver semantics)

- Crate: `semver 1.x`
- Used by cargo itself — extremely well-maintained and well-audited
- No known vulnerabilities in 1.x series
- **Recommendation**: Safe to add if needed. Use `Version::parse()` (returns `Result`) — never `Version::parse().unwrap()`.
- **Confidence: High**

**`notify` crate** (if filesystem watching for manifest changes is considered)

- Current release: `notify 8.2.0` (the 8.x series is the current maintained line; confirmed against docs.rs — earlier research incorrectly cited 6.x)
- Widely adopted: rust-analyzer, Deno, cargo-watch. MIT/Apache-2.0. No network access — pure filesystem watching.
- Adds a background thread and OS inotify fd — higher complexity than the feature likely warrants
- **Recommendation**: Avoid for v1. Polling on app startup (re-reading manifests when the page opens) is simpler and safer for the initial implementation.
- **Confidence: High** (version confirmed by api-researcher against docs.rs)

**`pelite` crate** (optional — if PE file version resources need to be extracted from trainer executables)

- Pure Rust, no `unsafe` blocks in the public API path, no network access
- Parses binary files from disk. Risk: parsing malformed or malicious PE files
- Mitigation: `pelite` is designed defensively and has been tested against the Corkami malformed PE corpus
- Trainer PE files come from user-configured paths (user chose these files) — same trust level as existing trainer path handling
- **Recommendation**: Acceptable if PE version extraction is required by the spec. If the feature can use filename patterns or community-provided version strings instead of binary parsing, prefer that simpler approach for v1.
- **Confidence: Medium** (assessment based on api-researcher review; no independent CVE search performed)

---

## Secure Coding Guidelines

These patterns are already established in the codebase. The version correlation feature must follow them:

### 1. All community data → SQLite via `params![]`

```rust
// Correct
conn.execute(
    "INSERT INTO version_snapshots (profile_id, build_id, game_version) VALUES (?1, ?2, ?3)",
    params![profile_id, build_id, game_version],
)?;

// Never do this
conn.execute(
    &format!("INSERT ... VALUES ('{}', '{}', '{}')", profile_id, build_id, game_version),
    [],
)?;
```

### 2. Bound community metadata fields before storage

Add `game_version` and `trainer_version` to `check_a6_bounds()`:

```rust
const MAX_VERSION_BYTES: usize = 256;

if meta.game_version.len() > MAX_VERSION_BYTES {
    return Err(format!(
        "game_version exceeds {} bytes ({} bytes)",
        MAX_VERSION_BYTES,
        meta.game_version.len()
    ));
}
if meta.trainer_version.len() > MAX_VERSION_BYTES {
    return Err(format!(
        "trainer_version exceeds {} bytes ({} bytes)",
        MAX_VERSION_BYTES,
        meta.trainer_version.len()
    ));
}
```

### 3. Validate git SHA format before subprocess call

```rust
fn is_valid_git_sha(commit: &str) -> bool {
    let t = commit.trim();
    (7..=64).contains(&t.len()) && t.chars().all(|c| c.is_ascii_hexdigit())
}
```

Apply in `normalize_subscription()` when `pinned_commit` is `Some`.

### 4. Numeric-only validation for Steam build IDs

```rust
fn is_valid_build_id(build_id: &str) -> bool {
    !build_id.is_empty() && build_id.chars().all(|c| c.is_ascii_digit())
}
```

### 5. Graceful version string comparison

```rust
fn compare_versions(local: &str, community: &str) -> VersionComparison {
    let local = local.trim();
    let community = community.trim();
    if community.is_empty() {
        return VersionComparison::CommunityUnspecified;
    }
    if local.is_empty() {
        return VersionComparison::LocalUnknown;
    }
    if local == community {
        return VersionComparison::Match;
    }
    VersionComparison::Mismatch { local: local.into(), community: community.into() }
}
```

---

## Trade-off Recommendations

### Symlink following in Steam paths

The lack of `fs::canonicalize()` in `normalize_path()` is intentional and correct. Steam users routinely symlink library directories across drives. Resolving symlinks would break legitimate configurations. Accept this as-is.

### Community data as "read-only metadata"

Version strings from community taps should be treated as **informational metadata** — they inform the user, they never control execution. This design principle should be enforced: community `game_version` and `trainer_version` must never be used to derive filesystem paths, subprocess arguments, or shell script content. They may only be:

- Stored in SQLite via parameterized queries
- Compared against locally-discovered values
- Displayed in the React UI as text

### Version history table retention (Advisory A7)

_Source: tech-designer review._

The proposed `version_records` and `version_mismatch_events` tables accumulate a row on every relevant launch or version check. Without a retention policy, these tables grow without bound for active users.

**Recommended approach**: On every INSERT, prune rows beyond the N most recent for that `profile_id`:

```sql
DELETE FROM version_records
WHERE profile_id = ?1
  AND id NOT IN (
    SELECT id FROM version_records
    WHERE profile_id = ?1
    ORDER BY created_at DESC
    LIMIT 50
  );
```

50 rows per profile is a reasonable default. This keeps the DB small while preserving meaningful history. The prune should happen in the same transaction as the INSERT.

### DB failure must not block launch (Advisory A8)

_Source: practices-researcher review._

The entire version correlation feature is informational. The existing codebase uses `metadata_store.is_available()` guards and `.unwrap_or_default()` on all DB reads for exactly this reason.

The new version snapshot upsert and mismatch recording calls must follow the same pattern: wrap in a guard, log any error with `tracing::warn!`, and let the launch proceed. A version check failure must never surface as a launch error to the user.

### Error display to users

_This section updated with ux-researcher findings._

Version mismatch error messages should be helpful without leaking system internals. Prefer:

> "Trainer compatibility unverified for game version 1.3.0"
> "Game version mismatch: Steam reports build 11651527, community profile was tested on 11500000."

**Never expose**:

- Absolute filesystem paths in user-facing messages (e.g., `/home/yandy/.config/crosshook/profiles/cyberpunk.toml`)
- Raw SQLite error messages in the UI (log them; show a generic "metadata error" to the user)

The existing health system correctly uses semantic `IssueCategory` field names (`missing_trainer`, `missing_executable`) rather than paths — version mismatch warnings must follow the same pattern.

**Community data disclaimer**: When displaying community-sourced compatibility information on the Compatibility page, include a note that data is community-reported and may not reflect the user's specific configuration. This prevents users from over-trusting community data as authoritative, which is especially important for the version mismatch use case.

**Version numbers in UI are safe to display**: Game build IDs (numeric strings from Steam) and version strings like `"1.2.3"` contain no PII and are expected to be visible to the user. Do not redact these.

**Post-launch confirmation prompts**: If a "Did the trainer work? Y/N" prompt is added, responses must be stored only in local profile metadata (TOML or SQLite). They must not be transmitted anywhere and must not be readable by external processes beyond the normal file permission model (`~/.config/crosshook/` inherits user ownership).

---

## Open Questions

1. **Version storage location**: Will game version snapshots live in a new table (e.g., `version_snapshots`) or as new columns on `community_profiles` and `profiles`? New table gives better query flexibility; columns are simpler to migrate. Security posture is equivalent for both.

2. **What triggers a version re-check?**: On app startup only? On tap sync? On demand? The answer affects how stale the displayed version data can be — important for UX, low security impact.

3. **Should `game_version` in community profiles be required or optional?**: Making it required (non-empty validation at index time) would prevent accidental mismatches against the empty string. Keeping it optional preserves backward compatibility with existing community profiles that don't specify a version.

4. **Semver vs. string equality**: If the feature spec calls for "version ranges" (e.g., "works with game version ≥ 1.12"), the `semver` crate is the right choice. If it's exact match only (current schema design suggests this), plain string comparison is sufficient and simpler.

5. **Should community version data suppress mismatch warnings?** (W3) This is an explicit architectural decision that must be made before implementation. Current recommendation: no — community data is display-only; only local + Steam data drives any behavioral outcome.

6. **Retention policy for version history tables**: How many rows per profile? 50 is a reasonable default; this should be a named constant so it is easy to tune.

---

## Teammate Input Sources

This document incorporates findings from the following research team members:

- **tech-designer**: Data model proposals, TOCTOU risk on manifest reads, version history table growth concern, A6 bounds gap confirmation
- **business-analyzer**: Data sensitivity classification (all low), parameterized query confirmation, privacy surface assessment
- **recommendations-agent**: Community version data trust model concern (W3), filesystem race condition documentation, cache store reuse suggestion
- **practices-researcher**: Trainer binary hashing safety assessment, ACF symlink gap confirmation, `MAX_*` byte cap pattern, fail-soft MetadataStore pattern
- **api-researcher**: `notify` version correction (8.2.0 not 6.x), `pelite` PE parsing safety assessment, `buildid` numeric validation confirmation
- **ux-researcher**: Path exposure risk in error messages, community data disclaimer requirement, post-launch prompt data handling
