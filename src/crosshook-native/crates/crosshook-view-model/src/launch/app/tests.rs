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

#[test]
fn launch_preview_converts_every_field_the_pipeline_reads() {
    use crosshook_core::launch::preview as core;
    use crosshook_core::profile::TrainerLoadingMode;
    use crosshook_core::settings::UmuPreference;

    let issue = |severity, code: Option<&str>| LaunchValidationIssue {
        message: format!("{severity:?}"),
        help: "help".into(),
        severity,
        code: code.map(str::to_owned),
        trainer_hash_stored: None,
        trainer_hash_current: None,
        trainer_sha256_community: None,
        hook_id: None,
        hook_name: None,
        hook_stage: None,
        hook_exit_code: None,
        hook_timed_out: None,
    };
    let source = core::LaunchPreview {
        resolved_method: ResolvedLaunchMethod::ProtonRun,
        validation: core::PreviewValidation {
            issues: vec![
                issue(ValidationSeverity::Fatal, Some("runtime_prefix_missing")),
                issue(ValidationSeverity::Warning, None),
                issue(ValidationSeverity::Info, Some("trainer_x")),
            ],
        },
        environment: Some(vec![core::PreviewEnvVar {
            key: "WINEPREFIX".into(),
            value: "/pfx".into(),
            source: core::EnvVarSource::ProtonRuntime,
        }]),
        cleared_variables: vec!["WINEESYNC".into()],
        wrappers: Some(vec!["mangohud".into()]),
        wrapper_details: Some(vec![core::PreviewWrapperDetail {
            command: vec!["mangohud".into()],
            source: core::PreviewWrapperSource::Optimization,
            active: true,
            reason: "overlay".into(),
            optimization_id: Some("mangohud".into()),
            optimization_label: Some("MangoHud".into()),
            folded_into: Some("gamescope --mangoapp".into()),
        }]),
        effective_command: Some("proton run game.exe".into()),
        directives_error: Some("bad".into()),
        steam_launch_options: Some("%command%".into()),
        proton_setup: Some(core::ProtonSetup {
            wine_prefix_path: "/pfx".into(),
            compat_data_path: "/compat".into(),
            steam_client_install_path: "/steam".into(),
            proton_executable: "/proton/proton".into(),
            umu_run_path: None,
        }),
        working_directory: "/games".into(),
        game_executable: "/games/game.exe".into(),
        game_executable_name: "game.exe".into(),
        trainer: Some(core::PreviewTrainerInfo {
            path: "/trainers/t.exe".into(),
            host_path: "/host/t.exe".into(),
            loading_mode: TrainerLoadingMode::SourceDirectory,
            staged_path: None,
        }),
        generated_at: "2026-11-23T00:00:00Z".into(),
        display_text: String::new(),
        gamescope_active: false,
        gamescope_decision: core::GamescopeDecisionPreview {
            enabled: false,
            allow_nested: false,
            inside_gamescope_session: false,
            active: false,
            reason: "off".into(),
            mangohud_folded_into_gamescope: false,
        },
        umu_decision: Some(core::UmuDecisionPreview {
            requested_preference: UmuPreference::Auto,
            umu_run_path_on_backend_path: None,
            will_use_umu: false,
            reason: "direct".into(),
            csv_coverage: CoreCsvCoverage::Found,
            gameid_resolution: Some(UmuGameIdResolution {
                game_id: "umu-1".into(),
                store: None,
                source: UmuGameIdResolutionSource::FreshLookup,
                lookup_key: None,
                fetched_at: None,
                expires_at: None,
                error_category: None,
            }),
        }),
    };

    let expected = LaunchPreview {
        resolved_method: LaunchMethod::ProtonRun,
        validation: PreviewValidation {
            issues: vec![
                ValidationIssue {
                    message: "Fatal".into(),
                    severity: Severity::Fatal,
                    code: Some("runtime_prefix_missing".into()),
                },
                ValidationIssue {
                    message: "Warning".into(),
                    severity: Severity::Warning,
                    code: None,
                },
                ValidationIssue {
                    message: "Info".into(),
                    severity: Severity::Info,
                    code: Some("trainer_x".into()),
                },
            ],
        },
        environment: Some(vec![EnvVar {
            key: "WINEPREFIX".into(),
            value: "/pfx".into(),
            source: EnvVarSource::ProtonRuntime,
        }]),
        cleared_variables: vec!["WINEESYNC".into()],
        wrappers: Some(vec!["mangohud".into()]),
        wrapper_details: Some(vec![WrapperDetail {
            command: vec!["mangohud".into()],
            source: WrapperSource::Optimization,
            active: true,
            reason: "overlay".into(),
            folded_into: Some("gamescope --mangoapp".into()),
        }]),
        effective_command: Some("proton run game.exe".into()),
        directives_error: Some("bad".into()),
        steam_launch_options: Some("%command%".into()),
        proton_setup: Some(ProtonSetup {
            wine_prefix_path: "/pfx".into(),
            proton_executable: "/proton/proton".into(),
        }),
        game_executable: "/games/game.exe".into(),
        game_executable_name: "game.exe".into(),
        trainer: Some(PreviewTrainer {
            path: "/trainers/t.exe".into(),
        }),
        umu_decision: Some(UmuDecision {
            will_use_umu: false,
            reason: "direct".into(),
            csv_coverage: CsvCoverage::Found,
            gameid_resolution: Some(GameIdResolution {
                game_id: "umu-1".into(),
                source: GameIdSource::FreshLookup,
            }),
        }),
    };
    assert_eq!(LaunchPreview::from(&source), expected);

    let empty = core::LaunchPreview {
        environment: None,
        wrappers: None,
        wrapper_details: None,
        umu_decision: None,
        ..source
    };
    let converted = LaunchPreview::from(&empty);
    assert!(converted.environment.is_none() && converted.wrappers.is_none());
    assert!(converted.wrapper_details.is_none() && converted.umu_decision.is_none());
}
