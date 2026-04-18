use super::command::build_effective_command_string;
use super::environment::{
    collect_host_environment, collect_runtime_proton_environment, collect_steam_proton_environment,
    inject_mangohud_config_preview_env, merge_custom_preview_env_only,
    merge_optimization_and_custom_preview_env,
};
use super::sections::{build_proton_setup, build_trainer_info, resolve_working_directory};
use super::types::{LaunchPreview, PreviewValidation, ResolvedLaunchMethod, UmuDecisionPreview};
use crate::launch::env::WINE_ENV_VARS_TO_CLEAR;
use crate::launch::optimizations::{
    build_steam_launch_options_command, resolve_launch_directives,
    resolve_launch_directives_for_method,
};
use crate::launch::request::{
    is_inside_gamescope_session, validate_all, LaunchRequest, METHOD_PROTON_RUN,
    METHOD_STEAM_APPLAUNCH,
};
use crate::launch::runtime_helpers::is_unshare_net_available;
use crate::launch::script_runner::{force_no_umu_for_launch_request, should_use_umu};

/// Builds a complete launch preview from a launch request.
///
/// Assembles validation, directives, environment, command, and metadata
/// into a single `LaunchPreview` struct for display in the frontend modal.
pub fn build_launch_preview(request: &LaunchRequest) -> Result<LaunchPreview, String> {
    let resolved_method = ResolvedLaunchMethod::from_request(request);
    let validation_issues = validate_all(request);
    let gamescope_config = request.effective_gamescope_config();

    let gamescope_active = gamescope_config.enabled
        && (gamescope_config.allow_nested || !is_inside_gamescope_session());

    // Resolve launch directives (wrappers + optimization env).
    // `steam_applaunch` uses the same optimization catalog as `proton_run` for Steam Launch Options,
    // without going through `resolve_launch_directives` (which is proton_run-only on the request).
    // Trainer-only launches use the same resolution path so previews match Proton-aligned semantics.
    // This can fail independently of validation (e.g., missing wrapper binary).
    let (directives, mut directives_error) = match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => match resolve_launch_directives_for_method(
            &request.optimizations.enabled_option_ids,
            METHOD_PROTON_RUN,
        ) {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e.to_string())),
        },
        _ => match resolve_launch_directives(request) {
            Ok(d) => (Some(d), None),
            Err(e) => (None, Some(e.to_string())),
        },
    };

    // Environment and command depend on successful directive resolution.
    let (environment, wrappers, effective_command) = match &directives {
        Some(directives) => {
            // Compute effective wrappers: prepend unshare for trainer-only + isolation.
            let effective_wrappers = if request.launch_trainer_only
                && request.network_isolation
                && is_unshare_net_available()
            {
                let mut w = vec!["unshare".to_string(), "--net".to_string()];
                w.extend(directives.wrappers.iter().cloned());
                w
            } else {
                directives.wrappers.clone()
            };

            let wrappers_had_mangohud = directives.wrappers.iter().any(|w| w.trim() == "mangohud");
            let mut env = Vec::new();
            collect_host_environment(&mut env);
            match resolved_method {
                ResolvedLaunchMethod::SteamApplaunch => {
                    collect_steam_proton_environment(request, &mut env);
                    merge_optimization_and_custom_preview_env(request, directives, &mut env);
                }
                ResolvedLaunchMethod::ProtonRun => {
                    collect_runtime_proton_environment(request, &mut env);
                    merge_optimization_and_custom_preview_env(request, directives, &mut env);
                }
                ResolvedLaunchMethod::Native => {
                    merge_custom_preview_env_only(request, &mut env);
                }
            }
            inject_mangohud_config_preview_env(
                &mut env,
                request,
                gamescope_active,
                wrappers_had_mangohud,
            );
            let effective_command = match build_effective_command_string(
                request,
                resolved_method,
                &effective_wrappers,
                &gamescope_config,
                gamescope_active,
            ) {
                Ok(command) => Some(command),
                Err(error) => {
                    append_preview_error(&mut directives_error, error);
                    None
                }
            };
            (Some(env), Some(effective_wrappers), effective_command)
        }
        None => (None, None, None),
    };

    // Steam launch options (for copy/paste); may still be computed when directive resolution failed
    // so errors surface consistently with the standalone Steam options panel.
    let gamescope_param = if gamescope_config.enabled {
        Some(gamescope_config)
    } else {
        None
    };
    let steam_launch_options = if resolved_method == ResolvedLaunchMethod::SteamApplaunch {
        match build_steam_launch_options_command(
            &request.optimizations.enabled_option_ids,
            &request.custom_env_vars,
            gamescope_param.as_ref(),
        ) {
            Ok(command) => Some(command),
            Err(error) => {
                append_preview_error(&mut directives_error, error.to_string());
                None
            }
        }
    } else {
        None
    };

    // These sections are independent of directive resolution.
    let proton_setup = build_proton_setup(request, resolved_method.as_str());
    let trainer = build_trainer_info(request, resolved_method.as_str());
    let cleared_variables = if resolved_method != ResolvedLaunchMethod::Native {
        WINE_ENV_VARS_TO_CLEAR
            .iter()
            .map(std::string::ToString::to_string)
            .collect()
    } else {
        Vec::new()
    };
    let working_directory = resolve_working_directory(request, resolved_method.as_str());
    let generated_at = chrono::Utc::now().to_rfc3339();

    let umu_decision = if resolved_method == ResolvedLaunchMethod::ProtonRun {
        Some(build_umu_decision_preview(request))
    } else {
        None
    };

    let mut preview = LaunchPreview {
        resolved_method,
        validation: PreviewValidation {
            issues: validation_issues,
        },
        environment,
        cleared_variables,
        wrappers,
        effective_command,
        directives_error,
        steam_launch_options,
        proton_setup,
        working_directory,
        game_executable: request.game_path.trim().to_string(),
        game_executable_name: request.game_executable_name(),
        trainer,
        generated_at,
        display_text: String::new(),
        gamescope_active,
        umu_decision,
    };

    preview.display_text = preview.to_display_toml();
    Ok(preview)
}

/// Builds the diagnostic `umu_decision` field for `proton_run` previews.
fn build_umu_decision_preview(request: &LaunchRequest) -> UmuDecisionPreview {
    use crate::settings::UmuPreference;
    let requested = request.umu_preference;
    let force_no_umu = force_no_umu_for_launch_request(request);
    let (will_use_umu, umu_run_path) = should_use_umu(request, force_no_umu);
    let reason = match (requested, umu_run_path.as_deref(), will_use_umu) {
        (_, _, true) => format!(
            "using umu-run at {}",
            umu_run_path.as_deref().unwrap_or("<unknown>")
        ),
        (UmuPreference::Proton, _, false) => {
            "preference = Proton — direct Proton always".to_string()
        }
        (UmuPreference::Auto, _, false) => {
            "preference = Auto but umu-run was not found on the backend PATH — falling back to direct Proton".to_string()
        }
        (UmuPreference::Umu, None, false) => {
            "preference = Umu but umu-run was not found on the backend PATH".to_string()
        }
        (UmuPreference::Umu, Some(_), false) => {
            "preference = Umu and umu-run found, but should_use_umu returned false (bug)"
                .to_string()
        }
    };
    let app_id = crate::launch::script_runner::resolve_steam_app_id_for_umu(request);
    let csv_coverage = crate::umu_database::check_coverage(app_id, Some("steam"));
    UmuDecisionPreview {
        requested_preference: requested,
        umu_run_path_on_backend_path: umu_run_path,
        will_use_umu,
        reason,
        csv_coverage,
    }
}

fn append_preview_error(target: &mut Option<String>, message: String) {
    match target {
        Some(existing) => {
            if existing != &message {
                existing.push('\n');
                existing.push_str(&message);
            }
        }
        None => *target = Some(message),
    }
}
