# Proton Migration Tool — Business Analysis

## Executive Summary

When users upgrade Proton (e.g., GE-Proton10-21 → GE-Proton10-34), profiles referencing the old path fail silently with a generic "path does not exist" validation error. GE-Proton releases approximately every 1-2 weeks and ProtonUp-Qt installs new builds without updating profile references — making this a frequent, recurring pain point for Steam Deck users. The stale path is already detected by the existing health system (`HealthStatus::Stale`); what is missing is an actionable suggestion and a one-click migration path. This feature adds Proton-specific stale detection within the health layer, a family-aware and major-version-aware suggestion algorithm, and a batch or per-profile migration workflow backed by the existing profile persistence system.

---

## User Stories

### Primary Actors

- **Steam Deck users** — gamepad-first, rarely type long paths; expect one-click fixes. Frequently use ProtonUp-Qt to install new GE-Proton builds (which does not remove old builds or update profile references), making this the most common trigger scenario.
- **Linux desktop gamers** — manage multiple Proton variants (official, GE, TKG) across many games; may manually delete old Proton versions to save disk space.
- **Community profile importers** — install profiles from taps; the bundled Proton version referenced in the profile may not match what is locally installed.

### Stories

| ID   | As a…                      | I want to…                                                                    | So that…                                                                                   |
| ---- | -------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| US-1 | Steam Deck user            | see a clear message that GE-Proton was upgraded and my profile needs updating | I don't have to guess why my game won't launch                                             |
| US-2 | Linux gamer                | get a suggested replacement Proton when mine is gone                          | I don't have to browse through deep filesystem paths manually                              |
| US-3 | Power user                 | migrate all broken profiles at once                                           | I don't repeat the same action for 10 profiles after a Proton update                       |
| US-4 | Community profile importer | see a candidate Proton from the same family even if I have a newer build      | the imported profile works without manual path hunting                                     |
| US-5 | Any user                   | review before/after paths before they're written                              | I don't accidentally downgrade a working Proton                                            |
| US-6 | Careful user               | be told when no suitable replacement exists                                   | I know I need to install the right Proton rather than blindly picking one                  |
| US-7 | Any user                   | be warned when the only available replacement is a different major version    | I can decide whether to risk a prefix migration or install the correct major version first |

---

## Business Rules

### Core Rules

**BR-1 — Stale Detection Trigger**
A profile has a stale Proton path when the effective Proton executable path does not exist on disk (`ErrorKind::NotFound`). This applies to both `steam.proton_path` (for `steam_applaunch` method) and `runtime.proton_path` (for `proton_run` method). "Wrong type" (e.g., path is a directory) is _Broken_, not Stale — only `NotFound` qualifies for automated migration.

**BR-2 — Method-Aware Path Targeting**
Migration must target the correct Proton field based on the profile's effective launch method:

- `steam_applaunch` → update `steam.proton_path`
- `proton_run` → update `runtime.proton_path`
- `native` → no Proton path; not eligible for migration

**BR-3 — Local Override Layer Awareness**
`ProfileStore::load()` returns the effective path (local_override merged into base, then cleared). `ProfileStore::save()` always writes paths to `local_override` via `storage_profile()`. Migration can therefore use the normal load/save cycle without special-casing the storage layer — path writes are machine-local by construction.

**BR-4 — Version Suggestion Strategy: Latest Within Safe Tier**
When suggesting a replacement, always select the **latest available** build within the highest-confidence tier. Do not prefer the closest version number — the user is most likely to have the newest build installed (via ProtonUp-Qt), and the latest within a tier is the safest forward-compatible choice.

Priority tiers with confidence scores (descending):

1. **Same family, same major version, highest build** (confidence 0.9) — default selection, no extra warning; included in default batch
2. **Versionless same family** (confidence 0.8) — e.g., "Proton Experimental" → "Proton Experimental"; no version comparison possible
3. **Same family, higher major version** (confidence 0.75) — requires major-version prefix-reset warning (BR-9); excluded from default batch
4. **Same family, older build / same major** (confidence 0.7) — shown with downgrade warning; excluded from default batch
5. **Same family, older major** (confidence 0.5) — shown with both cross-major and downgrade warnings; excluded from default batch
6. **Different family (fallback)** — shown with cross-family disclaimer; excluded from default batch; requires explicit per-profile opt-in

If no candidate at any tier is found: show "no Proton installed" state (BR-11).

Official Proton must never be silently substituted for a GE-Proton profile (or vice versa) — cross-family substitution always requires explicit user opt-in.

**BR-5 — User Confirmation Model (Context-Dependent)**
Confirmation requirements differ by entry point:

- **Single-profile inline fix** (Profiles page or Health Dashboard row): The displayed suggestion and a single "Apply" click constitute confirmation. An undo toast must appear immediately after the write succeeds, offering a 5-second window to revert. No blocking modal is required.
- **Batch migration** (≥2 profiles): A review modal is required before any writes. The modal shows all before/after pairs, allows deselection, and requires explicit "Apply All" to proceed.
- In both cases, the path being written must be shown to the user before or at the moment of action.

**BR-6 — Batch Migration Pre-Flight Validation**
Before writing any profile file, serialize all migration targets to TOML strings and validate that each new Proton path exists and is executable. If any pre-flight step fails, abort the entire batch with zero writes. This is simpler than a backup/rollback scheme and prevents partial-success states. Per-flight failures must be reported individually before asking the user to confirm.

After pre-flight passes and the user confirms, writes proceed best-effort: a write failure for one profile is logged and reported in the summary, but does not abort remaining writes.

**BR-7 — No Downgrade Without Warning**
If the best available candidate is an older build than the stale one (within the same major version), this must be visually flagged as a potential downgrade. Downgrades carry real risk: Steam has surfaced "invalid version" warnings when prefix Wine version regresses.

**BR-9 — Major Version Boundary Warning (New)**
A major Proton version increment (e.g., GE-Proton9-x → GE-Proton10-x, or Proton 8.x → Proton 9.x) involves a different Wine/DXVK base and can break an existing WINE prefix. When the best candidate crosses a major version boundary, the confirmation dialog MUST surface an explicit warning: "This changes the major Proton version. Your game prefix may need to be reset." Users must have the option to cancel and install a matching major-version build instead. Cross-major candidates must not be selected silently and must not appear in default batch migration selections.

**BR-10 — "Proton Experimental" Is Not Version-Comparable**
"Proton Experimental" is a rolling release with no version number. It cannot be compared against numbered installs for upgrade/downgrade detection. It is treated as a special-case family unto itself. Migration away from a stale "Proton Experimental" path must only suggest another "Proton Experimental" install, or fall back with a disclaimer if none is found.

**BR-8 — Profiles Without a Proton Path Are Excluded**
Profiles using `native` launch method, or those with an empty/unconfigured Proton path, are silently excluded from migration candidate lists. These are `Broken` (misconfigured) rather than `Stale` (path was valid, now missing).

**BR-11 — Zero Matches State**
When `discover_compat_tools()` finds no installed Proton at all (not just no matching family):

- Do not offer a migration dialog or suggest any path
- Surface a "No Proton installed" message with a "Browse manually" CTA for users to select a path themselves
- Include a contextual reference to ProtonUp-Qt for users who want GE-Proton, and Steam's Compatibility settings for official Proton
- This state is informational only — no writes occur

**BR-12 — Migration History Logging**
Each successful migration writes a record to the SQLite metadata layer capturing: profile name, profile_id, old path, new path, timestamp, and whether the migration crossed a major version boundary. This reuses the existing connection/transaction pattern from `profile_name_history`. The log is read-only from the user's perspective (no UI to manage it in Phase 1) but provides an audit trail and enables future "undo last migration" functionality.

### Edge Cases

- **Multiple profiles pointing to the same stale path**: All should be included in the batch; migrating them together is the expected UX.
- **Profile with both `steam.proton_path` and `runtime.proton_path` non-empty**: Unusual; only the field required by the effective launch method is considered.
- **Community tap profiles**: Portable profiles from taps have no local Proton path set in the base layer; after a local import, paths live in `local_override`. Migration treats them identically to hand-created profiles.
- **Proton discovery finds zero tools**: Show "No Proton installed" state per BR-11. Provide a "Browse manually" CTA and contextual ProtonUp-Qt reference for GE builds.
- **Stale path from a family not present in discovered tools**: Offer fallback (any latest Proton) with a clear warning that the family is unavailable. `DifferentFamilyFallback` candidates must NOT be included in the default batch — they require explicit per-profile opt-in.
- **Cross-family heuristic matching**: The existing `tool_matches_requested_name_heuristically()` uses substring containment which can match across families. Migration suggestion must use the stricter family-prefix extraction approach, not the existing heuristic, to avoid false same-family matches.
- **GE-Proton naming scheme transition (Phase 1 limitation)**: Legacy GE builds (`Proton-9.23-GE-2`) normalize to family key `protonge`; modern GE builds (`GE-Proton9-4`) normalize to `geproton`. In Phase 1, these are treated as separate families — a profile with a stale legacy GE path will not receive an automatic modern GE suggestion; it will show the "no same-family match" state (BR-4 tier 4 fallback). Phase 2 will add a heuristic or alias table to unify them. This is a known Phase 1 limitation and should be communicated clearly in the zero-matches UI message.
- **Official Proton `X.Y` vs `X.Y-Z`**: Steam directories contain `Proton X.Y` but display names include a patch suffix `Proton X.Y-Z`. Family extraction for official Proton must handle both forms.
- **ProtonUp-Qt users**: ProtonUp-Qt installs new GE-Proton builds alongside old ones without cleaning up or updating profile references. This is the primary trigger scenario — after a ProtonUp-Qt update, multiple profiles may all point to the same old path. Batch migration is critical for this use case.
- **Post-OS-reinstall / clean disk space cleanup**: Users who deleted old builds manually or reinstalled their OS won't have the old exact version. Migration must not require the old version to still exist anywhere.
- **Proton path in a system compat-tools directory** (`/usr/share/steam/compatibilitytools.d`): These are valid discovery targets; migration may suggest them as replacements.

---

## Workflows

### Workflow 1 — Auto-Detection on App Start / Health Dashboard Load

```
App starts / Health Dashboard opened
    │
    ▼
batch_check_health() runs for all profiles
    │
    ▼
For each profile:
    ├── launch_method = steam_applaunch → check steam.proton_path
    └── launch_method = proton_run     → check runtime.proton_path
    │
    ▼
HealthIssue with field = "steam.proton_path" and severity = Warning → HealthStatus::Stale
    │
    ▼
UI: Health Dashboard shows "Stale" badge
    │
    ▼
[New] For issues on Proton path fields:
    ├── Run discover_compat_tools() to find installed versions
    ├── Compute best-match suggestion per profile
    └── Enrich HealthIssue.remediation with "Upgrade to GE-Proton 9-7?" CTA
```

### Workflow 2 — Single Profile Migration (Inline, No Modal)

```
User sees stale Proton warning in Profile Editor or Health Dashboard row
    │
    ▼
Inline suggestion displayed:
    ├── Old path: /home/user/.steam/root/steamapps/common/GE-Proton10-21/proton  [missing]
    ├── Suggested: GE-Proton10-34  →  [full path]  [found ✓]
    └── [Apply] button (single click = confirmation per BR-5)
    │
    ▼
[If cross-major or downgrade: warning badge shown inline before Apply is enabled]
    │
    ▼
User clicks [Apply]
    │
    ▼
store.load(name)
profile.steam.proton_path = new_path  (or runtime.proton_path)
store.save(name, &profile)
    │
    ▼
Health re-check runs; profile should now report Healthy
    │
    ▼
store.load() → update proton_path → store.save()
metadata_store.observe_profile_write() (fail-soft)
Write migration log entry to SQLite (BR-12)
    │
    ▼
Success: undo toast shown for ~5 seconds
    ├── User clicks Undo → revert write, show confirmation
    └── Toast expires → migration is committed
    │
    ▼
Health badge refreshes to Healthy (or re-runs check)
```

### Workflow 3 — Batch Migration (≥2 Profiles, Modal Required)

```
User opens Health Dashboard
    │
    ▼
Dashboard shows N profiles with stale Proton issues
    │
    ▼
"Fix All Proton Paths" button (visible when ≥2 profiles qualify)
    │
    ▼
CrossHook runs discover_compat_tools() once
    │
    ▼
For each stale-Proton profile, compute suggestion and tier
    │
    ▼
Batch review modal shows two sections:
  [Safe to apply — checked by default]
    ├── Profile A: GE-Proton10-21 → GE-Proton10-34  ✓ (same family, same major)
    └── Profile B: GE-Proton10-21 → GE-Proton10-34  ✓
  [Needs manual review — unchecked by default]
    ├── Profile C: GE-Proton9-7 → GE-Proton10-34  ⚠ (major version change)
    └── Profile D: Proton-7-0-6  → [no same-family found; manual path required]
    │
    ▼
User reviews, optionally toggles selections
    │
    ▼
User confirms
    │
    ▼
For each selected profile (in order):
    ├── load, update path, save
    ├── Record result: success | failure (with error message)
    └── Continue even on failure
    │
    ▼
Summary report:
    ├── 2 profiles updated successfully
    └── 1 profile skipped (no matching Proton found)
    │
    ▼
Health re-check refreshes badges
```

### Workflow 4 — Error Recovery (Migration Fails)

```
Migration write fails (I/O error, TOML serialization error, etc.)
    │
    ▼
Profile file is NOT partially written (save() is atomic via fs::write)
    │
    ▼
Error message surfaced to user per profile:
    "Could not update Profile X: <error>"
    │
    ▼
Profile remains in Stale state; no data is lost
    │
    ▼
User can retry individually or fix manually
```

---

## Domain Model

### Entities

**ProtonPath** — A filesystem path pointing to a Proton `proton` executable file.

- Has a _family_ (e.g., "GE-Proton", "Proton", "Proton-tkg")
- Has a _version_ (numeric component, e.g., "9-4", "9.0.3")
- States: `Present` (file exists, executable), `Stale` (file missing), `Broken` (wrong type / permission denied)

**ProtonFamily** — A grouping of Proton versions by common lineage.

- Detected heuristically from the directory name / alias
- Examples: `ge-proton`, `proton`, `proton-tkg`
- Used for ranking migration candidates

**MigrationCandidate** — A discovered `ProtonInstall` that is a proposed replacement for a stale path.

- Has confidence: `same_family` | `same_family_older` | `different_family_fallback`
- Has the full path to the `proton` binary

**ProfileMigrationPlan** — A pending or applied migration for one profile.

- Profile name
- Stale path (field + old value)
- Proposed path (field + new value)
- Candidate confidence
- State: `pending` | `applied` | `skipped` | `failed`

### State Transitions (ProfileMigrationPlan)

```
[detected as stale]
        │
        ▼
   pending (suggestion computed, awaiting user action)
        │
        ├── user confirms → [write succeeds] → applied
        ├── user confirms → [write fails]    → failed
        └── user skips / no suggestion       → skipped
```

### Known Proton Families and Naming Schemes

| Family                | Directory Examples                     | Normalized Family Key | Notes                                       |
| --------------------- | -------------------------------------- | --------------------- | ------------------------------------------- |
| GE (modern)           | `GE-Proton9-4`, `GE-Proton10-34`       | `geproton`            | Major.Build (no dot separator)              |
| GE (legacy)           | `Proton-9.23-GE-2`, `Proton-8.16-GE-1` | `geproton`            | Must map to same family as modern GE        |
| Official Valve        | `Proton 9.0`, `Proton 8.0`             | `proton`              | Space separator; directory name has no dash |
| Official Experimental | `Proton Experimental`                  | `protonexperimental`  | Versionless; rolling target                 |
| Official Next         | `Proton Next`                          | `protonnext`          | Versionless pre-release                     |
| TKG                   | `Proton-tkg-*`                         | `protontkg`           | Community build                             |

### Proton Family Extraction Algorithm

Given a stale path like `/home/user/.steam/root/steamapps/common/GE-Proton9-4/proton`:

1. Take the parent directory name: `GE-Proton9-4`
2. Normalize: strip non-alphanumeric, lowercase → `geproton94`
3. Strip trailing digit sequences → family key `geproton`
4. Compare against normalized aliases of discovered installs, prefix-matching on family key

For legacy GE names like `Proton-9.23-GE-2`:

1. Normalize → `proton923ge2`
2. After stripping trailing digits: `proton923ge` — not `geproton`
3. This requires a **legacy GE detection rule**: if the normalized name contains `ge` AND (`proton` prefix OR `ge` suffix after digits), map to family `geproton`

This legacy normalization is an implementation gotcha. The family extraction function needs explicit special-casing or a lookup table for known legacy GE patterns.

### Version Components

For versioned families, version is extracted as a tuple of integers from the numeric portions of the directory name:

- `GE-Proton10-34` → `(10, 34)`
- `GE-Proton9-4` → `(9, 4)`
- `Proton-9.23-GE-2` → `(9, 23, 2)` (legacy GE; major is first segment)
- `Proton 9.0` → `(9, 0)`

Comparison uses tuple ordering: `(10, 34) > (10, 21) > (9, 7)`.

Major version boundary: the first tuple element differs between stale and candidate.

---

## Existing Codebase Integration

### What Already Exists (High Readiness)

| Capability           | Location                                               | Notes                                                                              |
| -------------------- | ------------------------------------------------------ | ---------------------------------------------------------------------------------- |
| Proton discovery     | `steam/proton.rs:discover_compat_tools()`              | Scans official + custom + system roots                                             |
| ProtonInstall model  | `steam/models.rs:ProtonInstall`                        | `name`, `path`, `aliases`, `normalized_aliases`, `is_official`                     |
| Stale detection      | `profile/health.rs:check_required_executable()`        | Returns `(HealthIssue, is_stale=true)` for `NotFound`                              |
| Health batch scan    | `profile/health.rs:batch_check_health()`               | Already runs on Health Dashboard load                                              |
| Profile persistence  | `profile/toml_store.rs:ProfileStore`                   | `load()`/`save()` with transparent local_override handling                         |
| Health IPC command   | `src-tauri/commands/health.rs:batch_validate_profiles` | Tauri command already called by Health Dashboard                                   |
| HealthIssue struct   | `profile/health.rs:HealthIssue`                        | Has `field`, `path`, `message`, `remediation` — remediation could carry suggestion |
| Local override layer | `profile/models.rs:LocalOverrideSection`               | Proton paths written to local layer on save — migration works transparently        |

### What Needs to Be Added (Implementation Gap)

| Capability                                | Notes                                                                                                                       |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Family extraction from path/name          | New pure function; no I/O required                                                                                          |
| Version ranking / sorting within a family | Parse version components from normalized alias; must compare as integer tuples (not lexicographic) to handle "9-10" > "9-9" |
| `suggest_proton_replacement()` function   | Composing discovery + family match + ranking                                                                                |
| New Tauri IPC commands                    | `suggest_proton_migration`, `apply_proton_migration`, `apply_batch_proton_migration`                                        |
| Migration result types                    | `ProfileMigrationPlan`, `MigrationResult`, `BatchMigrationSummary`                                                          |
| Enriched health issue with migration hint | Extend `HealthIssue` or `EnrichedProfileHealthReport` to carry `ProtonSuggestion`                                           |
| Pre-flight TOML serialization             | Serialize all new profiles to strings before writing any files                                                              |
| Launcher drift invalidation notice        | Surface to user that exported `.sh` scripts are now stale after migration                                                   |

### Profile Load/Save for Migration (Critical Detail)

`ProfileStore::load()` returns the **effective** profile (local_override merged in, then cleared). `ProfileStore::save()` calls `storage_profile()` which always writes paths to `local_override`. Therefore a migration is simply:

```rust
let mut profile = store.load(name)?;
profile.steam.proton_path = new_path;  // or runtime.proton_path
store.save(name, &profile)?;
```

**Important**: Do NOT manipulate the raw TOML file directly. If the raw file has a stale `local_override.steam.proton_path` and you only update the base `steam.proton_path`, `effective_profile()` will still prefer the local override and the stale path persists. Always go through the `ProfileStore` API.

### MetadataStore Sync (Required Post-Save Step)

Every profile write in the Tauri layer calls `metadata_store.observe_profile_write()` to keep the SQLite registry in sync. Migration commands must do the same:

```rust
// After store.save(name, &profile)
if let Err(e) = metadata_store.observe_profile_write(
    &name, &updated_profile, &profile_path, SyncSource::AppWrite, None
) {
    tracing::warn!(%e, "metadata sync after proton migration failed");
}
```

This is fail-soft (logged but not fatal) — consistent with how all other profile save commands handle it.

### Launcher Drift Cascade

When a profile's Proton path changes, any exported `.sh` launcher script that embeds the old path becomes stale. The existing `DriftState` tracking in `launchers` table will detect this on the next health check. Migration should surface a notice: "Your exported launcher scripts may need re-export after this change."

### Relevant Discovery Lookup Paths

- Official Proton: `{steam_root}/steamapps/common/{name}/proton`
- Custom GE-Proton: `{steam_root}/compatibilitytools.d/{name}/proton`
- System tools: `/usr/share/steam/compatibilitytools.d/{name}/proton`
- Steam root candidates discovered via `steam/discovery.rs:discover_steam_root_candidates()`

---

## Success Criteria

1. A user who upgrades GE-Proton and opens the Health Dashboard sees the affected profiles marked Stale with a specific "Proton version upgraded" message and a suggested replacement from the same family.
2. Single-profile inline migration applies in one click with an undo toast; no blocking modal required.
3. Batch migration (≥2 profiles) shows a review modal with safe/needs-review sections; cross-major candidates are unchecked by default.
4. If no installed Proton matches the stale family, a clear "no match" message is shown with a "Browse manually" CTA and ProtonUp-Qt reference; no silent fallback is applied.
5. Migration never silently substitutes a different Proton family (e.g., GE → official Proton) without explicit per-profile user opt-in.
6. Each successful migration is logged to SQLite with old path, new path, and timestamp.
7. The feature integrates with the existing Health Dashboard and Profile Editor without requiring a new navigation page.

---

## Open Questions

1. **Steam root discovery for `proton_run` profiles**: `discover_compat_tools()` requires `steam_root_candidates`. For `proton_run` profiles that have no `steam.compatdata_path`, the derivation heuristic returns empty. Fallback: scan default Steam root paths (`~/.steam/root`, `~/.local/share/Steam`, Flatpak path). Needs explicit decision on whether to always scan defaults as a fallback or require the user to provide a Steam root.

2. **Undo implementation scope**: The undo toast for single-profile migration requires storing the previous path in memory for the toast duration. Should undo also reverse the SQLite migration log entry, or leave the log intact as a record of "applied then reverted"?

3. **Batch threshold**: "Fix All" button appears when ≥2 profiles qualify. Should it appear for a single profile too (using the batch flow), or only show the inline single-profile widget for one affected profile?

4. **Phased rollout**: Phase 1 (single-profile inline fix only) is lower risk and validates the algorithm. Phase 2 adds batch modal. Is this the preferred split?

5. **Portable profile portability impact**: Migration writes to `local_override` (machine-local). A community tap profile's base layer still references the old (stale) path. `local_override` is the correct layer for machine-specific paths by design — confirmed by the storage model. This is not a bug, but users sharing profiles via taps should be aware their local migration doesn't update the shared base.
