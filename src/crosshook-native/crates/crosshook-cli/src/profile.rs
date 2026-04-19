use std::path::PathBuf;

use crate::args::{GlobalOptions, ProfileArgs, ProfileCommand};
use crate::cli_error::CliError;
use crate::store::profile_store;
use crosshook_core::profile::{export_community_profile, resolve_launch_method};

pub(crate) async fn handle_profile_command(
    command: ProfileArgs,
    global: &GlobalOptions,
) -> Result<(), CliError> {
    match command.command {
        ProfileCommand::List => {
            let store = profile_store(global.config.clone())?;
            let profiles = store
                .list()
                .map_err(|e| format!("failed to list profiles: {e}"))?;
            let count = profiles.len();
            let profiles_dir = store.base_path.to_string_lossy().into_owned();

            if global.json {
                #[derive(serde::Serialize)]
                struct ListOutput<'a> {
                    profiles: &'a [String],
                    count: usize,
                    profiles_dir: String,
                }
                let output = ListOutput {
                    profiles: &profiles,
                    count,
                    profiles_dir,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                for name in &profiles {
                    println!("{name}");
                }
                println!("{count} profile(s) in {profiles_dir}");
            }
        }
        ProfileCommand::Import(command) => {
            if global.verbose {
                eprintln!("legacy profile: {}", command.legacy_path.display());
            }

            // C-2 security mitigation: reject symlinks and non-files before opening
            let meta = std::fs::symlink_metadata(&command.legacy_path)
                .map_err(|e| format!("cannot access import path: {e}"))?;
            if !meta.file_type().is_file() {
                return Err(
                    "import path must be a regular file, not a symlink or directory".into(),
                );
            }

            let store = profile_store(global.config.clone())?;
            let profile = store
                .import_legacy(&command.legacy_path)
                .map_err(|e| format!("failed to import profile: {e}"))?;

            let profile_name = command
                .legacy_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            let launch_method = resolve_launch_method(&profile);

            if global.json {
                let output = serde_json::json!({
                    "imported": true,
                    "profile_name": profile_name,
                    "legacy_path": command.legacy_path.display().to_string(),
                    "launch_method": launch_method,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "Imported profile \"{}\" from {} (launch method: {})",
                    profile_name,
                    command.legacy_path.display(),
                    launch_method
                );
            }
        }
        ProfileCommand::Export(command) => {
            let profile_name = command
                .profile
                .or_else(|| global.profile.clone())
                .ok_or("a profile name is required; use --profile or -p")?;

            let output_path = command.output.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(format!("{profile_name}.crosshook.json"))
            });

            // W-4 security mitigation: reject symlinks at output path
            if output_path
                .symlink_metadata()
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                return Err("output path is a symlink; refusing to write".into());
            }

            if let Some(parent) = output_path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    return Err(
                        format!("output directory does not exist: {}", parent.display()).into(),
                    );
                }
            }

            let store = profile_store(global.config.clone())?;
            export_community_profile(store.base_path.as_path(), &profile_name, &output_path)
                .map_err(|e| format!("failed to export profile: {e}"))?;

            if global.json {
                let output = serde_json::json!({
                    "exported": true,
                    "profile_name": profile_name,
                    "output_path": output_path.display().to_string(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "Exported profile \"{}\" to {}",
                    profile_name,
                    output_path.display()
                );
            }
        }
    }

    Ok(())
}
