use super::types::LaunchPreview;

fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

impl LaunchPreview {
    /// Renders a human-readable TOML-like text summary for clipboard copy.
    pub fn to_display_toml(&self) -> String {
        let mut lines = Vec::new();

        // [preview]
        lines.push("[preview]".to_string());
        lines.push(format!(
            "generated_at = \"{}\"",
            escape_toml_string(&self.generated_at)
        ));
        lines.push(format!(
            "method = \"{}\"",
            escape_toml_string(self.resolved_method.as_str())
        ));
        lines.push(format!(
            "game = \"{}\"",
            escape_toml_string(&self.game_executable)
        ));
        lines.push(format!(
            "game_name = \"{}\"",
            escape_toml_string(&self.game_executable_name)
        ));
        if !self.working_directory.is_empty() {
            lines.push(format!(
                "working_directory = \"{}\"",
                escape_toml_string(&self.working_directory)
            ));
        }
        lines.push(String::new());

        // [validation]
        lines.push("[validation]".to_string());
        lines.push(format!("passed = {}", self.validation.issues.is_empty()));
        lines.push(format!("issue_count = {}", self.validation.issues.len()));
        for issue in &self.validation.issues {
            lines.push(format!("  [{:?}] {}", issue.severity, issue.message));
        }
        lines.push(String::new());

        // [command]
        lines.push("[command]".to_string());
        if let Some(ref cmd) = self.effective_command {
            lines.push(format!("effective = \"{}\"", escape_toml_string(cmd)));
        }
        if let Some(ref opts) = self.steam_launch_options {
            lines.push(format!(
                "steam_launch_options = \"{}\"",
                escape_toml_string(opts)
            ));
        }
        if let Some(ref wrappers) = self.wrappers {
            if !wrappers.is_empty() {
                lines.push(format!("wrappers = {wrappers:?}"));
            }
        }
        if let Some(ref err) = self.directives_error {
            lines.push(format!("error = \"{}\"", escape_toml_string(err)));
        }
        lines.push(String::new());

        // [proton]
        if let Some(ref setup) = self.proton_setup {
            lines.push("[proton]".to_string());
            lines.push(format!(
                "proton_executable = \"{}\"",
                escape_toml_string(&setup.proton_executable)
            ));
            lines.push(format!(
                "wine_prefix_path = \"{}\"",
                escape_toml_string(&setup.wine_prefix_path)
            ));
            lines.push(format!(
                "compat_data_path = \"{}\"",
                escape_toml_string(&setup.compat_data_path)
            ));
            lines.push(format!(
                "steam_client_install_path = \"{}\"",
                escape_toml_string(&setup.steam_client_install_path)
            ));
            if let Some(ref umu) = setup.umu_run_path {
                lines.push(format!("umu_run = \"{}\"", escape_toml_string(umu)));
            }
            lines.push(String::new());
        }

        // [trainer]
        if let Some(ref trainer) = self.trainer {
            lines.push("[trainer]".to_string());
            lines.push(format!("path = \"{}\"", escape_toml_string(&trainer.path)));
            lines.push(format!(
                "host_path = \"{}\"",
                escape_toml_string(&trainer.host_path)
            ));
            lines.push(format!(
                "loading_mode = \"{}\"",
                trainer.loading_mode.as_str()
            ));
            if let Some(ref staged) = trainer.staged_path {
                lines.push(format!("staged_path = \"{}\"", escape_toml_string(staged)));
            }
            lines.push(String::new());
        }

        // [environment]
        if let Some(ref env) = self.environment {
            lines.push(format!("[environment]  # {} vars", env.len()));
            for var in env {
                lines.push(format!(
                    "{} = \"{}\"",
                    var.key,
                    escape_toml_string(&var.value)
                ));
            }
            lines.push(String::new());
        }

        // [cleared_variables]
        if !self.cleared_variables.is_empty() {
            lines.push(format!(
                "[cleared_variables]  # {} vars",
                self.cleared_variables.len()
            ));
            for var in &self.cleared_variables {
                lines.push(format!("  {var}"));
            }
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::launch::preview::types::{
        EnvVarSource, PreviewEnvVar, PreviewTrainerInfo, PreviewValidation, ProtonSetup,
        ResolvedLaunchMethod,
    };
    use crate::profile::TrainerLoadingMode;

    #[test]
    fn escape_toml_string_escapes_special_characters() {
        let input = "path\\\"line\nwith\tcarriage\r";
        let escaped = escape_toml_string(input);
        assert_eq!(escaped, "path\\\\\\\"line\\nwith\\tcarriage\\r");
    }

    #[test]
    fn to_display_toml_escapes_quoted_fields() {
        let preview = LaunchPreview {
            resolved_method: ResolvedLaunchMethod::ProtonRun,
            validation: PreviewValidation { issues: Vec::new() },
            environment: Some(vec![PreviewEnvVar {
                key: "PATH".to_string(),
                value: r"C:\Games\App\bin".to_string(),
                source: EnvVarSource::Host,
            }]),
            cleared_variables: Vec::new(),
            wrappers: None,
            effective_command: Some(r#"gamescope "C:\Games\Example.exe""#.to_string()),
            directives_error: Some("line1\nline2".to_string()),
            steam_launch_options: Some(r#""C:\Games\Example.exe""#.to_string()),
            proton_setup: Some(ProtonSetup {
                wine_prefix_path: r"C:\Games\Prefix".to_string(),
                compat_data_path: r"C:\Games\CompatData".to_string(),
                steam_client_install_path: r"C:\Program Files (x86)\Steam".to_string(),
                proton_executable: r"C:\Proton\proton".to_string(),
                umu_run_path: Some(r"C:\Tools\umu-run.exe".to_string()),
            }),
            working_directory: r"C:\Games\Working Dir".to_string(),
            game_executable: r"C:\Games\Example\Game.exe".to_string(),
            game_executable_name: r#"Cool "Game""#.to_string(),
            trainer: Some(PreviewTrainerInfo {
                path: r"C:\Trainers\MyTrainer.exe".to_string(),
                host_path: r"/home/user/trainers/MyTrainer.exe".to_string(),
                loading_mode: TrainerLoadingMode::CopyToPrefix,
                staged_path: Some(r"C:\Staged\Trainer.exe".to_string()),
            }),
            generated_at: "2024-01-01T00:00:00Z".to_string(),
            display_text: String::new(),
            gamescope_active: false,
            umu_decision: None,
        };

        let output = preview.to_display_toml();
        assert!(output.contains(r#"game_name = "Cool \"Game\"""#));
        assert!(output.contains(r#"working_directory = "C:\\Games\\Working Dir""#));
        assert!(output.contains(r#"PATH = "C:\\Games\\App\\bin""#));
        assert!(output.contains(r#"error = "line1\nline2""#));
        assert!(output.contains(r#"steam_launch_options = "\"C:\\Games\\Example.exe\"""#));
    }
}
