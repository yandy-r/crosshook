use crate::args::{GlobalOptions, SteamArgs, SteamCommand};
use crate::cli_error::CliError;
use crosshook_core::steam::discovery::discover_steam_root_candidates;
use crosshook_core::steam::libraries::discover_steam_libraries;
use crosshook_core::steam::proton::discover_compat_tools;
use crosshook_core::steam::{
    attempt_auto_populate, SteamAutoPopulateFieldState, SteamAutoPopulateRequest,
};

pub(crate) async fn handle_steam_command(
    command: SteamArgs,
    global: &GlobalOptions,
) -> Result<(), CliError> {
    match command.command {
        SteamCommand::Discover => {
            let mut diagnostics: Vec<String> = Vec::new();
            let roots = discover_steam_root_candidates("", &mut diagnostics);
            let libraries = discover_steam_libraries(&roots, &mut diagnostics);
            let proton_installs = discover_compat_tools(&roots, &mut diagnostics);

            if global.verbose {
                for msg in &diagnostics {
                    eprintln!("{msg}");
                }
            }

            if global.json {
                let output = serde_json::json!({
                    "roots": roots.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>(),
                    "libraries": libraries,
                    "proton_installs": proton_installs,
                    "diagnostics": diagnostics,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!("Steam roots: {}", roots.len());
                for root in &roots {
                    println!("  {}", root.display());
                }
                println!();
                println!("Libraries: {}", libraries.len());
                for lib in &libraries {
                    println!(
                        "  {} (steamapps: {})",
                        lib.path.display(),
                        lib.steamapps_path.display()
                    );
                }
                println!();
                println!("Proton installs: {}", proton_installs.len());
                for install in &proton_installs {
                    println!("  {} ({})", install.name, install.path.display());
                }
            }
        }
        SteamCommand::AutoPopulate(command) => {
            if global.verbose {
                eprintln!("game path: {}", command.game_path.display());
            }

            let request = SteamAutoPopulateRequest {
                game_path: command.game_path.clone(),
                steam_client_install_path: Default::default(),
            };

            let result = attempt_auto_populate(&request);

            if global.verbose {
                for msg in &result.diagnostics {
                    eprintln!("{msg}");
                }
            }

            if global.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!(
                    "App ID:      {} ({})",
                    result.app_id,
                    format_field_state(result.app_id_state)
                );
                println!(
                    "Compat Data: {} ({})",
                    result.compatdata_path.display(),
                    format_field_state(result.compatdata_state)
                );
                println!(
                    "Proton:      {} ({})",
                    result.proton_path.display(),
                    format_field_state(result.proton_state)
                );

                if !result.manual_hints.is_empty() {
                    println!();
                    for hint in &result.manual_hints {
                        println!("  hint: {hint}");
                    }
                }
            }
        }
    }

    Ok(())
}

fn format_field_state(state: SteamAutoPopulateFieldState) -> &'static str {
    match state {
        SteamAutoPopulateFieldState::Found => "Found",
        SteamAutoPopulateFieldState::NotFound => "not detected",
        SteamAutoPopulateFieldState::Ambiguous => "Ambiguous — set manually",
    }
}
