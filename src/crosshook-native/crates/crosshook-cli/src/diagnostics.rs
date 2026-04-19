use crate::args::{DiagnosticsArgs, DiagnosticsCommand, GlobalOptions};
use crate::cli_error::CliError;
use crate::store::profile_store;
use crosshook_core::export::diagnostics::DiagnosticBundleOptions;
use crosshook_core::settings::SettingsStore;

pub(crate) fn handle_diagnostics_command(
    args: DiagnosticsArgs,
    global: &GlobalOptions,
) -> Result<(), CliError> {
    match args.command {
        DiagnosticsCommand::Export(command) => {
            let profile_store = profile_store(global.config.clone())?;
            let settings_store =
                SettingsStore::try_new().map_err(|error| format!("settings store: {error}"))?;

            let options = DiagnosticBundleOptions {
                redact_paths: command.redact_paths,
                output_dir: command.output,
            };

            let result = crosshook_core::export::export_diagnostic_bundle(
                &profile_store,
                &settings_store,
                &options,
            )?;

            if global.json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                println!("Diagnostic bundle exported: {}", result.archive_path);
                println!("  Profiles:        {}", result.summary.profile_count);
                println!("  Log files:       {}", result.summary.log_file_count);
                println!("  Proton versions: {}", result.summary.proton_install_count);
            }

            Ok(())
        }
    }
}
