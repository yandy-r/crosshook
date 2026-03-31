use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "crosshook",
    version,
    about = "CrossHook native CLI",
    propagate_version = true,
    subcommand_required = true,
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalOptions,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Args, Clone, Default)]
pub struct GlobalOptions {
    /// Profile name to use (overrides auto-load)
    #[arg(short, long, global = true, value_name = "NAME")]
    pub profile: Option<String>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Emit structured JSON to stdout instead of human-readable output
    #[arg(long, global = true)]
    pub json: bool,

    /// Path to a crosshook config file (default: ~/.config/crosshook/settings.toml)
    #[arg(long, value_name = "PATH", global = true)]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
pub enum Command {
    /// Launch a game and trainer using a saved profile
    Launch(LaunchCommand),
    /// Manage CrossHook profiles (list, import, export)
    Profile(ProfileArgs),
    /// Steam discovery and auto-population utilities
    Steam(SteamArgs),
    /// Export a diagnostic bundle for troubleshooting
    Diagnostics(DiagnosticsArgs),
    /// Show system status: profiles, Steam installations, and settings summary
    Status,
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell)
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Args)]
pub struct LaunchCommand {
    /// Name of the profile to launch (overrides --profile global flag)
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Override the profiles directory (default: ~/.config/crosshook/profiles/)
    #[arg(long, hide = true, value_name = "PATH")]
    pub profile_dir: Option<PathBuf>,

    /// Override the helper scripts directory (default: bundled AppImage scripts)
    #[arg(long, hide = true, value_name = "PATH")]
    pub scripts_dir: Option<PathBuf>,

    /// Show what would be launched without executing
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
pub enum ProfileCommand {
    /// List all saved profiles (one name per line)
    List,
    /// Import a legacy .profile file and convert it to TOML format
    Import(ProfileImportCommand),
    /// Export a profile as a portable community JSON file
    Export(ProfileExportCommand),
}

#[derive(Debug, Args)]
pub struct ProfileImportCommand {
    /// Path to the legacy .profile file to import
    #[arg(long = "legacy-path", value_name = "PATH")]
    pub legacy_path: PathBuf,
}

#[derive(Debug, Args)]
pub struct ProfileExportCommand {
    /// Name of the profile to export (overrides --profile global flag)
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,

    /// Output path for the exported JSON file (default: <cwd>/<profile-name>.crosshook.json)
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SteamArgs {
    #[command(subcommand)]
    pub command: SteamCommand,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
pub enum SteamCommand {
    /// Scan for Steam installations, libraries, and Proton versions
    Discover,
    /// Pre-fill Steam metadata (App ID, compat path) from a game executable path
    AutoPopulate(SteamAutoPopulateCommand),
}

#[derive(Debug, Args)]
pub struct SteamAutoPopulateCommand {
    /// Path to the game executable used to look up Steam App ID and compat data
    #[arg(long = "game-path", value_name = "PATH")]
    pub game_path: PathBuf,
}

#[derive(Debug, Args)]
pub struct DiagnosticsArgs {
    #[command(subcommand)]
    pub command: DiagnosticsCommand,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "kebab-case")]
pub enum DiagnosticsCommand {
    /// Export a diagnostic bundle as a .tar.gz archive
    Export(DiagnosticsExportCommand),
}

#[derive(Debug, Args)]
pub struct DiagnosticsExportCommand {
    /// Redact home directory paths in profile configs and settings
    #[arg(long)]
    pub redact_paths: bool,

    /// Output directory for the archive (default: system temp directory)
    #[arg(long, value_name = "PATH")]
    pub output: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, DiagnosticsCommand, ProfileCommand, SteamCommand};

    #[test]
    fn parses_launch_command_with_global_flags() {
        let cli = Cli::try_parse_from([
            "crosshook",
            "--verbose",
            "--json",
            "--config",
            "/tmp/crosshook.toml",
            "launch",
            "--profile",
            "elden-ring",
        ])
        .expect("parser should accept launch");

        assert!(cli.global.verbose);
        assert!(cli.global.json);
        assert_eq!(
            cli.global.config.as_deref(),
            Some(std::path::Path::new("/tmp/crosshook.toml"))
        );

        match cli.command {
            Command::Launch(command) => {
                assert_eq!(command.profile.as_deref(), Some("elden-ring"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_profile_import_command() {
        let cli = Cli::try_parse_from([
            "crosshook",
            "profile",
            "import",
            "--legacy-path",
            "/tmp/elden-ring.profile",
        ])
        .expect("parser should accept profile import");

        match cli.command {
            Command::Profile(profile) => match profile.command {
                ProfileCommand::Import(command) => {
                    assert_eq!(
                        command.legacy_path,
                        std::path::PathBuf::from("/tmp/elden-ring.profile")
                    );
                }
                other => panic!("unexpected profile command: {other:?}"),
            },
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_steam_auto_populate_command() {
        let cli = Cli::try_parse_from([
            "crosshook",
            "steam",
            "auto-populate",
            "--game-path",
            "/games/elden-ring/eldenring.exe",
        ])
        .expect("parser should accept steam auto-populate");

        match cli.command {
            Command::Steam(steam) => match steam.command {
                SteamCommand::AutoPopulate(command) => {
                    assert_eq!(
                        command.game_path,
                        std::path::PathBuf::from("/games/elden-ring/eldenring.exe")
                    );
                }
                other => panic!("unexpected steam command: {other:?}"),
            },
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_launch_dry_run_flag() {
        let cli = Cli::try_parse_from([
            "crosshook",
            "launch",
            "--profile",
            "test",
            "--dry-run",
        ])
        .expect("parser should accept launch with --dry-run");

        match cli.command {
            Command::Launch(command) => {
                assert!(command.dry_run);
                assert_eq!(command.profile.as_deref(), Some("test"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_completions_command() {
        let cli = Cli::try_parse_from(["crosshook", "completions", "zsh"])
            .expect("parser should accept completions");

        match cli.command {
            Command::Completions { shell } => {
                assert_eq!(shell, clap_complete::Shell::Zsh);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_diagnostics_export_command() {
        let cli = Cli::try_parse_from(["crosshook", "diagnostics", "export"])
            .expect("parser should accept diagnostics export");

        match cli.command {
            Command::Diagnostics(args) => match args.command {
                DiagnosticsCommand::Export(command) => {
                    assert!(!command.redact_paths);
                    assert!(command.output.is_none());
                }
                #[allow(unreachable_patterns)]
                other => panic!("unexpected diagnostics command: {other:?}"),
            },
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_diagnostics_export_with_flags() {
        let cli = Cli::try_parse_from([
            "crosshook",
            "diagnostics",
            "export",
            "--redact-paths",
            "--output",
            "/tmp/diag",
        ])
        .expect("parser should accept diagnostics export with flags");

        match cli.command {
            Command::Diagnostics(args) => match args.command {
                DiagnosticsCommand::Export(command) => {
                    assert!(command.redact_paths);
                    assert_eq!(
                        command.output.as_deref(),
                        Some(std::path::Path::new("/tmp/diag"))
                    );
                }
                #[allow(unreachable_patterns)]
                other => panic!("unexpected diagnostics command: {other:?}"),
            },
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
