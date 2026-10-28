mod dismissals;
mod readiness;
mod snapshot;

use std::sync::Arc;

use crosshook_core::metadata::MetadataStore;
use crosshook_core::onboarding::{
    HostToolCheckResult, HostToolDetails, HostToolInstallCommand, ReadinessCheckResult,
    SteamDeckCaveats, UmuInstallGuidance,
};
use crosshook_core::profile::health::{HealthIssue, HealthIssueSeverity};
use crosshook_core::settings::SettingsStore;

use super::OnboardingService;
use crate::jobs::JobRegistry;
use crate::services::host_tools::fake::FakeHostProbe;

fn tool_check() -> HostToolCheckResult {
    HostToolCheckResult {
        tool_id: "umu_run".into(),
        display_name: "umu-run".into(),
        is_available: false,
        is_required: true,
        category: "runtime".into(),
        docs_url: "https://example.invalid/umu".into(),
        tool_version: None,
        resolved_path: Some(format!(
            "{}/.local/bin/umu-run",
            std::env::var("HOME").unwrap()
        )),
        install_guidance: Some(HostToolInstallCommand {
            distro_family: "Arch".into(),
            command: "install umu".into(),
            alternatives: String::new(),
        }),
    }
}

fn readiness_result() -> ReadinessCheckResult {
    ReadinessCheckResult {
        checks: vec![HealthIssue {
            field: "umu_run_available".into(),
            path: format!("{}/.local/bin/umu-run", std::env::var("HOME").unwrap()),
            message: "umu missing".into(),
            remediation: "install umu".into(),
            severity: HealthIssueSeverity::Warning,
        }],
        all_passed: false,
        critical_failures: 1,
        warnings: 1,
        umu_install_guidance: Some(UmuInstallGuidance {
            install_command: "install umu".into(),
            docs_url: "https://example.invalid/umu".into(),
            description: "install guidance".into(),
        }),
        steam_deck_caveats: Some(SteamDeckCaveats {
            description: "test caveats".into(),
            items: vec!["caveat one".into()],
            docs_url: "https://example.invalid/deck".into(),
        }),
        tool_checks: vec![tool_check()],
        detected_distro_family: "Arch".into(),
    }
}

fn details() -> HostToolDetails {
    HostToolDetails {
        tool_id: "umu_run".into(),
        tool_version: None,
        resolved_path: None,
    }
}

fn fake_probe() -> Arc<FakeHostProbe> {
    Arc::new(FakeHostProbe::new(
        readiness_result(),
        readiness_result(),
        details(),
    ))
}

fn service(settings: SettingsStore, metadata: MetadataStore) -> OnboardingService {
    OnboardingService::with_probe(settings, metadata, JobRegistry::default(), fake_probe())
}
