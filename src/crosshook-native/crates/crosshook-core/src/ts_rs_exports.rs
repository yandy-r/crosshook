#![cfg(feature = "ts-rs")]

use std::path::PathBuf;

use ts_rs::TS;

use crate::{
    onboarding::{
        Capability, CapabilityState, HostToolCheckResult, HostToolDetails, HostToolInstallCommand,
        ReadinessCheckResult, SteamDeckCaveats, TrainerGuidanceContent, TrainerGuidanceEntry,
        UmuInstallGuidance,
    },
    profile::health::{HealthIssue, HealthIssueSeverity},
};

const GENERATED_BASE: &str = "../../src/types";

#[allow(dead_code)]
#[derive(TS)]
#[ts(export, export_to = "generated/ts_rs_edge_cases.ts")]
struct TsRsEdgeCases {
    id: uuid::Uuid,
    generated_at: chrono::DateTime<chrono::Utc>,
    payload: Vec<u8>,
    note: Option<String>,
}

/// Export a small set of representative arg/return DTOs to TypeScript via ts-rs.
/// Intended for the Phase 5 evaluation — gated behind the `ts-rs` feature.
pub fn export_ts_types() -> Result<(), Box<dyn std::error::Error>> {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(GENERATED_BASE);
    std::fs::create_dir_all(base.join("generated"))?;
    std::env::set_var("TS_RS_EXPORT_DIR", &base);

    export_onboarding()?;
    export_edge_cases()?;
    Ok(())
}

fn export_onboarding() -> Result<(), Box<dyn std::error::Error>> {
    HostToolInstallCommand::export()?;
    HostToolCheckResult::export()?;
    UmuInstallGuidance::export()?;
    SteamDeckCaveats::export()?;
    HostToolDetails::export()?;
    CapabilityState::export()?;
    Capability::export()?;
    HealthIssueSeverity::export()?;
    HealthIssue::export()?;
    ReadinessCheckResult::export()?;
    TrainerGuidanceEntry::export()?;
    TrainerGuidanceContent::export()?;
    Ok(())
}

/// Export additional shapes that exercise ts-rs edge cases (chrono / uuid / Vec<u8>).
fn export_edge_cases() -> Result<(), Box<dyn std::error::Error>> {
    TsRsEdgeCases::export()?;
    Ok(())
}
