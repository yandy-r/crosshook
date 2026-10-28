# YAN-831 B0 contracts (frozen)

These contracts already compile on the branch. Paths are relative to `src/crosshook-native/`, and binding decisions are in `decisions.md`.

## Landed in B0 (do not change the signatures)

- `crates/crosshook-app/src/services/mod.rs`
  - `async fn run_job<T, F>(jobs: &JobRegistry, spec: JobSpec, work: F) -> Result<T, AppError>`. It runs `work` on the blocking pool inside a supervised job, maps cancellation to `Cancelled`, and reaps the job afterwards.
  - It exports `HostToolsService`, `HostProbe`, `CoreHostProbe`, `OnboardingService` and `OfflineService`.
- `services/host_tools/probe.rs`: `trait HostProbe: Send + Sync + 'static`, with `system_readiness()`, `generalized_readiness()` and `tool_details(&str)`. `CoreHostProbe` is the live implementation.
- `services/host_tools/mod.rs`: `HostToolsService { jobs, probe }`, built with `new(jobs)` or `with_probe(jobs, probe)`.
- `services/onboarding/mod.rs`: `OnboardingService { settings: SettingsStore, metadata: MetadataStore, jobs, probe }`, built with `new(settings, metadata, jobs)` or `with_probe(.., probe)`.
- `services/offline/mod.rs`: `OfflineService { profiles, metadata }`, built with `new(profiles, metadata)`.
- `services/health/mod.rs`: the signature is now `HealthService::new(profiles, metadata, jobs)`. `batch_summary` runs as the non-exclusive `health-batch` job.
- `bootstrap/stores.rs`: `Services` gains the `onboarding`, `host_tools` and `offline` fields.

## Public methods each slice adds

Every method is `async` and returns `Result<_, AppError>`, unless it is listed as sync. The DTOs are the existing core or adapter types, and their serde output must stay unchanged.

| Service           | Method                                                                                                                                   | Replaces Tauri command                                  |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| HostToolsService  | `tool_details(tool_id: String) -> HostToolDetails` (run_job kind `host-tool-details`, `resolved_path` sanitized)                         | `probe_host_tool_details`                               |
| HostToolsService  | `gamescope_session() -> bool` (sync, environment read)                                                                                   | `check_gamescope_session`                               |
| HostToolsService  | `platform_status() -> LaunchPlatformCapabilities` (run_blocking, because `unshare` is probed once)                                       | `launch_platform_status`                                |
| OnboardingService | `check_readiness() -> ReadinessCheckResult` (run_job `host-readiness`)                                                                   | `check_readiness`                                       |
| OnboardingService | `check_generalized_readiness() -> ReadinessCheckResult` (run_job; probe → snapshot upsert → overlays → sanitize)                         | `check_generalized_readiness`                           |
| OnboardingService | `cached_host_readiness_snapshot() -> Option<CachedHostReadinessSnapshot>` (the DTO moves into the service and its serde stays unchanged) | `get_cached_host_readiness_snapshot`                    |
| OnboardingService | `capabilities() -> Vec<Capability>` (the cached path, or a live run_job fallback)                                                        | `get_capabilities`                                      |
| OnboardingService | `dismiss_onboarding()`                                                                                                                   | `dismiss_onboarding`                                    |
| OnboardingService | `dismiss_legacy_nag(LegacyNag)`, with `pub enum LegacyNag { UmuInstall, SteamDeckCaveats }`, a single dual-write function (D5)           | `dismiss_umu_install_nag`, `dismiss_steam_deck_caveats` |
| OnboardingService | `dismiss_readiness_nag(tool_id: String, ttl_days: Option<u32>)`                                                                          | `dismiss_readiness_nag`                                 |
| OnboardingService | `trainer_guidance() -> TrainerGuidanceContent` (sync)                                                                                    | `get_trainer_guidance`                                  |
| OfflineService    | `check_readiness(name) -> OfflineReadinessReport`                                                                                        | `check_offline_readiness`                               |
| OfflineService    | `batch_readiness() -> Vec<OfflineReadinessReport>`                                                                                       | `batch_offline_readiness`                               |
| OfflineService    | `verify_trainer_hash(name) -> HashVerifyResult`                                                                                          | `verify_trainer_hash`                                   |
| OfflineService    | `network_status() -> bool`                                                                                                               | `check_network_status`                                  |
| OfflineService    | `trainer_type_catalog() -> Vec<TrainerTypeEntry>` (sync)                                                                                 | `get_trainer_type_catalog`                              |

Each adapter maps errors with `error.message().to_string()`, so the legacy texts from D8 come through unchanged.

## Startup scans (D9)

- Add `AppEvent::HealthScanCompleted { total, healthy, stale, broken }`, `AppEvent::OfflineReadinessScanCompleted { total_profiles }` and `AppEvent::VersionScanCompleted { summary: VersionScanSummary-equivalent fields }`. All are `Clone + PartialEq + Eq`, and none carries paths.
- The sequence moves to `services/startup_scans.rs`, and `bootstrap/scans.rs` calls it.
- On success, the matching named event is published right after the corresponding `StartupJobFinished`.
- `src-tauri/src/startup_bridge.rs` ignores the new variants.
