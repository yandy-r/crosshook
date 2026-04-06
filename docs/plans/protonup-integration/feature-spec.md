# Feature Spec: ProtonUp Integration

## Executive Summary

This feature adds in-app Proton version management so CrossHook can detect when community-recommended runtimes are missing, list installable versions, and run guided installation flows without forcing users to leave the app. Implementation stays core-first in `crosshook-core`, with thin Tauri commands and typed frontend hooks. The rollout is staged to keep launch reliability intact: read-only visibility and recommendations first, explicit installs second, and recommendation quality improvements third. Key constraints are Linux path variability, integrity verification of downloaded artifacts, and preserving non-blocking launch behavior when a valid local runtime already exists.

## External Dependencies

### APIs and Services

#### protonup-rs / libprotonup

- **Documentation**: [ProtonUp-rs repository](https://github.com/auyer/Protonup-rs), [libprotonup docs](https://docs.rs/libprotonup/latest/libprotonup/)
- **Authentication**: none for local CLI/library execution
- **Key capabilities**:
  - list available releases by tool/provider
  - resolve native and Flatpak install locations
  - install/unpack compatibility tool archives
  - verify checksums when provided
- **Rate limits**: depends on upstream release source when remote metadata is fetched
- **Pricing**: open source

#### GitHub Releases metadata (fallback/direct catalog mode)

- **Documentation**: [GitHub Releases API](https://docs.github.com/en/rest/releases/releases)
- **Authentication**: optional token for higher rate limits
- **Key endpoints**:
  - `GET /repos/{owner}/{repo}/releases`
- **Rate limits**: GitHub API limits apply
- **Pricing**: free API tiers with limits

### Libraries and SDKs

| Library       | Version               | Purpose                                   | Installation            |
| ------------- | --------------------- | ----------------------------------------- | ----------------------- |
| `libprotonup` | latest compatible     | release listing and install orchestration | `cargo add libprotonup` |
| `reqwest`     | existing in workspace | optional direct release metadata fetch    | existing dependency     |
| `serde`       | existing in workspace | IPC DTO serialization                     | existing dependency     |

### External Documentation

- [Valve Proton README](https://raw.githubusercontent.com/ValveSoftware/Proton/master/README.md): compatibility tool directory expectations.
- [compatibilitytool.vdf template](https://raw.githubusercontent.com/ValveSoftware/Proton/master/compatibilitytool.vdf.template): required runtime layout metadata.
- [ProtonUp-Qt project](https://github.com/DavidoTek/ProtonUp-Qt): UX reference for install flows and terminology.

## Business Requirements

### User Stories

**Primary user: CrossHook player**

- As a player, I want CrossHook to warn me when a community profile expects a Proton version that I do not have installed.
- As a player, I want to install a recommended Proton version directly from CrossHook so I can launch quickly.
- As a player, I want to continue with my existing valid runtime when I choose not to install the suggested one.

**Secondary user: advanced profile maintainer**

- As a maintainer, I want suggestion matching that is transparent (exact match vs fallback) so I can trust recommendations.

### Business Rules

1. **Advisory recommendations by default**: community `proton_version` mismatch produces suggestion/warning, not hard launch block.
   - Validation: hard block only when configured runtime path is invalid for selected launch method.
   - Exception: optional strict mode may be added later if explicitly enabled.
2. **User-confirmed installation**: install actions require explicit confirmation.
3. **Graceful offline behavior**: remote catalog failures must not break installed runtime enumeration or valid launches.
4. **Persistence boundaries**:
   - TOML settings for user preferences/path overrides.
   - SQLite metadata for cached release catalogs and optional install history.
   - runtime-only for progress/state streams.

### Edge Cases

| Scenario                                    | Expected Behavior                                                              | Notes                            |
| ------------------------------------------- | ------------------------------------------------------------------------------ | -------------------------------- |
| Network unavailable while fetching versions | show cached versions with stale indicator, keep local installed list available | no launch blocking               |
| Missing ProtonUp binary/provider            | show actionable setup guidance and keep fallback options                       | do not dead-end UI               |
| Partial/failed install                      | mark as failed, provide retry and details, do not mark installed               | verify filesystem post-condition |
| Community version string not parseable      | show "unknown mapping" with manual chooser                                     | avoid false exact match          |

### Success Criteria

- [ ] Available Proton/GE-Proton versions are listed for installation.
- [ ] Users can install selected versions from CrossHook.
- [ ] Community profile runtime requirements trigger install suggestions.
- [ ] Launch reliability remains unchanged for already-valid runtime configurations.
- [ ] Offline/degraded states are understandable and recoverable.

## Technical Specifications

### Architecture Overview

```text
Frontend UI (Profiles/Compatibility/Settings)
  -> typed hooks (useProtonUp/useProtonInstalls)
  -> Tauri commands (snake_case, thin wrappers)
  -> crosshook-core protonup service
      -> provider adapter (libprotonup or CLI subprocess)
      -> Steam root/runtime discovery reuse
      -> metadata cache + suggestion matcher
```

### Data Models

#### `ProtonUpAvailableVersion` (runtime DTO + cached payload)

| Field         | Type     | Constraints        | Description                |
| ------------- | -------- | ------------------ | -------------------------- |
| provider      | string   | non-empty          | release source/tool family |
| version       | string   | non-empty          | version tag/name           |
| release_url   | string   | optional           | upstream release URL       |
| checksum_kind | string   | optional           | `sha256` or `sha512`       |
| fetched_at    | datetime | required for cache | catalog freshness          |

#### `ProtonUpInstallRequest` (IPC request)

| Field       | Type   | Constraints    | Description               |
| ----------- | ------ | -------------- | ------------------------- |
| provider    | string | enum-like      | target runtime provider   |
| version     | string | required       | selected version          |
| target_root | string | validated path | install destination root  |
| force       | bool   | optional       | allow overwrite/reinstall |

#### Storage classification

| Datum                                       | Storage                         | Rationale                  |
| ------------------------------------------- | ------------------------------- | -------------------------- |
| auto-suggest preference / provider defaults | TOML settings                   | user-editable behavior     |
| available version catalog                   | SQLite `external_cache_entries` | operational cache with TTL |
| install progress                            | runtime-only                    | transient streaming state  |
| optional install audit rows                 | SQLite metadata (future)        | operational history        |

### API Design

#### `protonup_list_available_versions`

**Purpose**: return installable versions with cache metadata.  
**Authentication**: none for local command, optional GitHub token internally.

**Response shape**:

```json
{
  "versions": [
    {
      "provider": "ge-proton",
      "version": "GE-Proton9-21",
      "releaseUrl": "https://github.com/GloriousEggroll/proton-ge-custom/releases/..."
    }
  ],
  "cache": { "stale": false, "fetchedAt": "2026-04-06T10:00:00Z" }
}
```

#### `protonup_install_version`

**Purpose**: install a selected version and return structured status.  
**Authentication**: none.

**Errors**:

| Status               | Condition                           | Response                  |
| -------------------- | ----------------------------------- | ------------------------- |
| `dependency_missing` | provider binary/library unavailable | actionable setup guidance |
| `permission_denied`  | no write access to target path      | path + guidance           |
| `checksum_failed`    | integrity mismatch                  | install aborted           |
| `network_error`      | download/fetch failure              | retry supported           |

### System Integration

#### Files to Create

- `src/crosshook-native/crates/crosshook-core/src/protonup/mod.rs`: service interface + domain types.
- `src/crosshook-native/crates/crosshook-core/src/protonup/service.rs`: catalog/install/suggestion orchestration.
- `src/crosshook-native/src-tauri/src/commands/protonup.rs`: Tauri command wrappers.
- `src/crosshook-native/src/hooks/useProtonUp.ts`: frontend invoke wrappers and UI state.
- `src/crosshook-native/src/types/protonup.ts`: shared TypeScript DTOs.

#### Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/lib.rs`: export new module.
- `src/crosshook-native/src-tauri/src/lib.rs`: register commands.
- `src/crosshook-native/src/hooks/useProtonInstalls.ts`: refresh strategy after install.
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`: missing-version suggestion UX.
- `src/crosshook-native/src/components/pages/CompatibilityPage.tsx`: install entry points and recommendation context.
- `src/crosshook-native/src/types/settings.ts`: optional preference/path fields.

#### Configuration

- `settings.toml` (optional additions):
  - `protonup.auto_suggest = true|false`
  - `protonup.binary_path = "/custom/path"` (optional)

## UX Considerations

### User Workflows

1. **Launch-time missing runtime assist**
   - User clicks launch.
   - System detects missing community-recommended runtime.
   - UI offers install recommended version, choose another, or continue with fallback when safe.
2. **Profile settings resolution flow**
   - User sees warning badge in profile runtime section.
   - User installs/selects runtime from same context.
   - UI confirms selected runtime and availability.
3. **Error recovery flow**
   - Install fails with specific reason.
   - User gets direct retry path and optional detailed diagnostics.

### UI Patterns

| Component              | Pattern                                    | Notes                                        |
| ---------------------- | ------------------------------------------ | -------------------------------------------- |
| missing runtime prompt | progressive disclosure                     | simple default CTA + advanced details drawer |
| version picker         | ranked list + tags                         | labels: recommended/installed/community      |
| install progress       | background task panel + inline status chip | avoid blocking full page                     |

### Accessibility Requirements

- Keyboard-complete flow for all install and recovery actions.
- Screen reader announcements for state transitions (downloading/installing/verifying/failed/success).
- Text + icon status indicators (not color-only semantics).
- Reduced-motion compliant progress indicators.

### Performance UX

- Loading states should prefer cached data immediately with stale marker.
- Install progress updates should be throttled to avoid UI churn.
- Errors should render concise summary first, detailed diagnostics on demand.

## Recommendations

### Implementation Approach

**Recommended strategy**: hybrid rollout with a stable core service abstraction, starting with read-only recommendations and adding explicit installs in phase 2.

1. Build catalog + suggestion read path first.
2. Add install pipeline with integrity verification and structured failures.
3. Improve ranking and optional one-click flows after reliability baseline.

### Technology Decisions

| Decision                   | Recommendation                                       | Rationale                                           |
| -------------------------- | ---------------------------------------------------- | --------------------------------------------------- |
| provider integration style | adapter abstraction in `crosshook-core`              | allows CLI/library swap without IPC churn           |
| initial provider scope     | GE-Proton first, Wine-GE as optional phase expansion | lowers scope risk while delivering core value       |
| cache approach             | SQLite `external_cache_entries` with TTL             | already available and consistent with repo patterns |
| recommendation behavior    | advisory by default                                  | protects launch continuity                          |

### Quick Wins

- Ship installed runtime visibility improvements and suggestion badges before install actions.
- Add explicit stale-cache indicator to reduce confusion during offline use.
- Add parser tests for community version normalization/matching.

### Future Enhancements

- Background catalog prefetch with backoff.
- Optional strict mode for teams/users who require exact runtime match.
- Batch install workflow for multi-profile imports.

## Risk Assessment

### Technical Risks

| Risk                                      | Likelihood | Impact | Mitigation                                             |
| ----------------------------------------- | ---------- | ------ | ------------------------------------------------------ |
| external provider output/behavior changes | Medium     | High   | isolate provider adapter and parse defensively         |
| install path mismatch across Linux setups | Medium     | High   | reuse Steam root discovery, validate destination paths |
| integrity verification gaps               | Low        | High   | require checksum validation before success             |
| long-running install hangs                | Medium     | Medium | enforce timeouts/cancellation and stream progress      |

### Integration Challenges

- Maintaining consistent state between install completion and runtime re-discovery.
- Matching free-form community version strings to normalized installed runtime identifiers.
- Avoiding duplicate logic between profiles, compatibility page, and settings flows.

### Security Considerations

- Never shell-interpolate user input for install commands.
- Enforce allowed-path checks to prevent writes outside compatibility tool directories.
- Treat checksums as required signal for successful installation state.

## Task Breakdown Preview

### Phase 1: Discovery and Suggestions

**Focus**: establish read-paths and non-blocking recommendation UX.  
**Tasks**:

- implement version catalog retrieval + cache.
- implement suggestion matcher between community and installed versions.
- add UI indicators in profile/compatibility views.
  **Parallelization**: cache service and frontend indicator work can proceed concurrently.

### Phase 2: Install Pipeline

**Focus**: explicit user-triggered installation flow.  
**Dependencies**: phase 1 DTOs and provider abstraction.
**Tasks**:

- implement install command and structured error mapping.
- add progress streaming and post-install re-discovery hooks.
- add install/recovery UI actions.

### Phase 3: Hardening and Quality

**Focus**: reliability, UX polish, and recommendation quality.  
**Tasks**:

- add parser and integration tests for provider and matching logic.
- tune recommendation ranking and stale/offline messaging.
- document operational diagnostics and troubleshooting.

## Decisions Confirmed

All previously open decisions are now confirmed with the recommended options.

1. **Provider scope for v1**
   - **Accepted**: GE-Proton only for initial release.
   - Follow-up: expand to Wine-GE after install reliability baseline is met.
2. **Integration mode**
   - **Accepted**: hybrid adapter with one primary path and fallback.
   - Follow-up: keep provider boundary stable so implementation can evolve without IPC churn.
3. **Recommendation strictness**
   - **Accepted**: advisory by default, with optional strict mode deferred for future phase.
   - Follow-up: preserve non-blocking launch behavior unless strict mode is explicitly enabled.

## Research References

- [research-external.md](./research-external.md): external APIs, release sources, provider trade-offs.
- [research-business.md](./research-business.md): business rules, workflows, and user success criteria.
- [research-technical.md](./research-technical.md): architecture, data model, and file impact preview.
- [research-ux.md](./research-ux.md): interaction patterns, accessibility, and feedback state design.
- [research-recommendations.md](./research-recommendations.md): phased strategy, risks, and decision checklist.
