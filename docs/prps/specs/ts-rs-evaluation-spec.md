# ts-rs shape-contract evaluation (Phase 5)

## Scope of the spike
- Added an opt-in `ts-rs` feature to `crosshook-core`; exporter lives at `src/crosshook-native/crates/crosshook-core/src/bin/ts_rs_export.rs`.
- Derived `TS` and exported to `src/crosshook-native/src/types/generated/` for onboarding/readiness shapes (HostTool*, Capability*, UmuInstallGuidance, SteamDeckCaveats, ReadinessCheckResult, TrainerGuidance*) plus supporting health types.
- Generated `src/crosshook-native/src/types/onboarding.ts` now re-exports the generated bindings; health types stay manual for now.
- Edge-case coverage sample (`ts_rs_edge_cases.ts`) exercises `Vec<u8>`, `chrono::DateTime<Utc>`, and `uuid::Uuid`.

Run/export command (opt-in; no effect on normal builds):
```bash
cargo run --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --features ts-rs --bin ts_rs_export
```

## Findings
- Annotation burden: 11 structs/enums touched (onboarding + health) with `cfg_attr(feature = "ts-rs", derive(TS))` and `ts(export, export_to = "generated/...")`. No business-logic changes.
- `serde(rename_all)` respected (`CapabilityState` → `"available" | "degraded" | "unavailable"`, health severities → `"error" | "warning" | "info"`). `Option<T>` becomes `T | null` (properties stay required).
- `skip_serializing_if` is ignored (warning on `Capability.rationale`); field still emitted as `string | null`. No other serde attrs failed.
- Edge cases: `Vec<u8>` → `Array<number>` (no base64); `chrono::DateTime<Utc>` → `string` (RFC3339 expected); `uuid::Uuid` → `string`.
- Dependency imports work: `ReadinessCheckResult` imports `HealthIssue` from generated `health.ts`; capability types share the same generated onboarding file.

## Recommendation
- **Incremental adoption**: keep the `ts-rs` feature gated and continue migrating one TS file at a time. The API surface maps cleanly with serde casing, but the ignored `skip_serializing_if` and `Option` → `null` semantics warrant intentional rollout. No blockers found for onboarding/readiness shapes.

## Suggested next steps
1) Decide on null vs. optional semantics for optional fields before broader migration (ts-rs currently emits `field: Type | null`).  
2) If we proceed, add a lightweight script/CI check to assert the generated files are up to date (behind the feature flag).  
3) Evaluate `ts-rs` handling of `flatten`, tagged enums, and mixed `rename_all` modules before touching more complex DTOs (e.g., launch/export paths).  
4) Resolve or accept the `skip_serializing_if` warning (ts-rs upstream issue) to avoid noisy exports on capability structs.
