use crosshook_core::launch::preview::ResolvedLaunchMethod;

use super::*;

fn profile() -> GameProfile {
    let mut profile = GameProfile::default();
    profile.game.executable_path = "/games/quest.exe".into();
    profile.runtime.prefix_path = "  ".into();
    profile.runtime.proton_path = "/compat/proton".into();
    profile.steam.app_id = "9999001".into();
    profile.steam.compatdata_path = " /steam/compatdata/9999001/pfx ".into();
    profile.trainer.path = "/trainers/quest.exe".into();
    profile.trainer.required_protontricks = vec!["dotnet48".into()];
    profile.launch.optimizations.enabled_option_ids = vec!["a".into(), "b".into()];
    profile
}

#[test]
fn pipeline_profile_reads_the_tier1_fields() {
    let pipeline = PipelineProfile::from_profile(&profile());
    assert_eq!(
        pipeline,
        PipelineProfile {
            game_executable_path: "/games/quest.exe".into(),
            prefix_path: "  ".into(),
            proton_path: "/compat/proton".into(),
            steam_app_id: "9999001".into(),
            trainer_path: "/trainers/quest.exe".into(),
            enabled_optimizations: 2,
        }
    );
}

#[test]
fn gate_scope_uses_the_effective_prefix_and_required_packages() {
    let scope = GateScope::from_profile("Quest", &profile(), true);
    assert_eq!(
        scope,
        GateScope {
            profile_name: "Quest".into(),
            prefix_path: "/steam/compatdata/9999001/pfx".into(),
            required_packages: vec!["dotnet48".into()],
            auto_install: true,
        }
    );
}

#[test]
fn repair_status_and_completion_convert_field_by_field() {
    let status = PrefixVersionRepairStatus {
        required: true,
        state: Some(PrefixVersionRestoreState::Failed),
        last_error: Some("timed out".into()),
        in_progress: true,
        last_recovery: None,
    };
    assert_eq!(
        RepairStatus::from(&status),
        RepairStatus {
            required: true,
            state: Some(RestoreState::Failed),
            last_error: Some("timed out".into()),
            in_progress: true,
        }
    );

    let payload = DepCompletePayload {
        profile_name: "Quest".into(),
        prefix_path: "/pfx".into(),
        succeeded: false,
        exit_code: Some(1),
        install_succeeded: true,
        install_exit_code: Some(0),
        restore_state: PrefixVersionRestoreState::Pending,
        restore_error: None,
    };
    assert_eq!(
        InstallCompletion::from(&payload),
        InstallCompletion {
            profile_name: "Quest".into(),
            prefix_path: "/pfx".into(),
            install_succeeded: true,
            restore_state: RestoreState::Pending,
            restore_error: None,
        }
    );
}

#[test]
fn dependency_status_and_enums_convert() {
    let status = PrefixDependencyStatus {
        package_name: "vcrun2019".into(),
        state: CoreDependencyState::CheckFailed,
        checked_at: None,
        installed_at: None,
        last_error: None,
    };
    assert_eq!(
        DependencyStatus::from(&status),
        DependencyStatus {
            package_name: "vcrun2019".into(),
            state: DependencyState::CheckFailed,
        }
    );
    assert_eq!(
        LaunchMethod::from(ResolvedLaunchMethod::SteamApplaunch),
        LaunchMethod::SteamApplaunch
    );
    assert_eq!(
        Severity::from(ValidationSeverity::Warning),
        Severity::Warning
    );
    assert_eq!(
        CsvCoverage::from(CoreCsvCoverage::Missing).as_str(),
        "missing"
    );
    assert_eq!(
        GameIdSource::from(&UmuGameIdResolutionSource::StaleCache).label(),
        "stale cache fallback"
    );
}
