use super::types::LaunchPreview;

impl LaunchPreview {
    /// Renders a human-readable TOML-like text summary for clipboard copy.
    pub fn to_display_toml(&self) -> String {
        let mut lines = Vec::new();

        // [preview]
        lines.push("[preview]".to_string());
        lines.push(format!("generated_at = \"{}\"", self.generated_at));
        lines.push(format!("method = \"{}\"", self.resolved_method.as_str()));
        lines.push(format!("game = \"{}\"", self.game_executable));
        lines.push(format!("game_name = \"{}\"", self.game_executable_name));
        if !self.working_directory.is_empty() {
            lines.push(format!(
                "working_directory = \"{}\"",
                self.working_directory
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
            lines.push(format!("effective = \"{cmd}\""));
        }
        if let Some(ref opts) = self.steam_launch_options {
            lines.push(format!("steam_launch_options = \"{opts}\""));
        }
        if let Some(ref wrappers) = self.wrappers {
            if !wrappers.is_empty() {
                lines.push(format!("wrappers = {wrappers:?}"));
            }
        }
        if let Some(ref err) = self.directives_error {
            lines.push(format!("error = \"{err}\""));
        }
        lines.push(String::new());

        // [proton]
        if let Some(ref setup) = self.proton_setup {
            lines.push("[proton]".to_string());
            lines.push(format!(
                "proton_executable = \"{}\"",
                setup.proton_executable
            ));
            lines.push(format!("wine_prefix_path = \"{}\"", setup.wine_prefix_path));
            lines.push(format!("compat_data_path = \"{}\"", setup.compat_data_path));
            lines.push(format!(
                "steam_client_install_path = \"{}\"",
                setup.steam_client_install_path
            ));
            if let Some(ref umu) = setup.umu_run_path {
                lines.push(format!("umu_run = \"{umu}\""));
            }
            lines.push(String::new());
        }

        // [trainer]
        if let Some(ref trainer) = self.trainer {
            lines.push("[trainer]".to_string());
            lines.push(format!("path = \"{}\"", trainer.path));
            lines.push(format!("host_path = \"{}\"", trainer.host_path));
            lines.push(format!(
                "loading_mode = \"{}\"",
                trainer.loading_mode.as_str()
            ));
            if let Some(ref staged) = trainer.staged_path {
                lines.push(format!("staged_path = \"{staged}\""));
            }
            lines.push(String::new());
        }

        // [environment]
        if let Some(ref env) = self.environment {
            lines.push(format!("[environment]  # {} vars", env.len()));
            for var in env {
                lines.push(format!("{} = \"{}\"", var.key, var.value));
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
