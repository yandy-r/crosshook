use crate::args::GlobalOptions;
use crate::cli_error::CliError;
use crate::store::profile_store;
use crosshook_core::settings::SettingsStore;
use crosshook_core::steam::discovery::discover_steam_root_candidates;
use crosshook_core::steam::proton::discover_compat_tools;
use serde::Serialize;

pub(crate) async fn handle_status_command(global: &GlobalOptions) -> Result<(), CliError> {
    let mut diagnostics: Vec<String> = Vec::new();

    let (profile_names, profiles_dir) = match profile_store(None) {
        Err(error) => {
            diagnostics.push(format!("profile store: {error}"));
            (Vec::new(), None)
        }
        Ok(store) => {
            let dir = Some(store.base_path.to_string_lossy().into_owned());
            let names = match store.list() {
                Ok(names) => names,
                Err(error) => {
                    diagnostics.push(format!("profile list: {error}"));
                    Vec::new()
                }
            };
            (names, dir)
        }
    };

    let settings_data = match SettingsStore::try_new() {
        Err(error) => {
            diagnostics.push(format!("settings store: {error}"));
            None
        }
        Ok(store) => match store.load() {
            Ok(data) => Some(data),
            Err(error) => {
                diagnostics.push(format!("settings load: {error}"));
                None
            }
        },
    };

    let steam_roots = discover_steam_root_candidates("", &mut diagnostics);

    let proton_installs = discover_compat_tools(&steam_roots, &mut diagnostics);

    if global.verbose {
        for diagnostic in &diagnostics {
            eprintln!("[status] {diagnostic}");
        }
    }

    let version = env!("CARGO_PKG_VERSION");

    if global.json {
        #[derive(Serialize)]
        struct ProtonInfo {
            display_name: String,
            proton_path: String,
        }

        #[derive(Serialize)]
        struct SteamInfo {
            roots: Vec<String>,
            proton_installs: Vec<ProtonInfo>,
        }

        #[derive(Serialize)]
        struct ProfilesInfo {
            count: usize,
            names: Vec<String>,
            profiles_dir: Option<String>,
        }

        #[derive(Serialize)]
        struct SettingsInfo {
            auto_load_last_profile: bool,
            last_used_profile: String,
            community_tap_count: usize,
            onboarding_completed: bool,
        }

        #[derive(Serialize)]
        struct StatusOutput {
            version: String,
            profiles: ProfilesInfo,
            steam: SteamInfo,
            settings: Option<SettingsInfo>,
            diagnostics: Vec<String>,
        }

        let output = StatusOutput {
            version: version.to_string(),
            profiles: ProfilesInfo {
                count: profile_names.len(),
                names: profile_names,
                profiles_dir,
            },
            steam: SteamInfo {
                roots: steam_roots
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
                proton_installs: proton_installs
                    .iter()
                    .map(|p| ProtonInfo {
                        display_name: p.name.clone(),
                        proton_path: p.path.to_string_lossy().into_owned(),
                    })
                    .collect(),
            },
            settings: settings_data.map(|s| SettingsInfo {
                auto_load_last_profile: s.auto_load_last_profile,
                last_used_profile: s.last_used_profile,
                community_tap_count: s.community_taps.len(),
                onboarding_completed: s.onboarding_completed,
            }),
            diagnostics,
        };

        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("crosshook {version}");
        println!();
        println!(
            "Profiles: {} ({})",
            profile_names.len(),
            profiles_dir.as_deref().unwrap_or("<unknown>")
        );
        for name in &profile_names {
            println!("  - {name}");
        }
        println!();
        println!("Steam roots: {}", steam_roots.len());
        for root in &steam_roots {
            println!("  {}", root.display());
        }
        println!();
        println!("Proton installs: {}", proton_installs.len());
        for install in &proton_installs {
            println!("  {} ({})", install.name, install.path.display());
        }
        if let Some(settings) = settings_data {
            println!();
            println!("Settings:");
            println!(
                "  auto_load_last_profile: {}",
                settings.auto_load_last_profile
            );
            println!("  last_used_profile:      {}", settings.last_used_profile);
            println!(
                "  community_taps:         {}",
                settings.community_taps.len()
            );
            println!(
                "  onboarding_completed:   {}",
                settings.onboarding_completed
            );
        }
    }

    Ok(())
}
