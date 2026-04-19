mod args;
mod cli_error;
mod diagnostics;
mod launch;
mod profile;
mod status;
mod steam;
mod store;

use args::{Cli, Command};
use clap::{CommandFactory, Parser};
use cli_error::CliError;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(error.exit_code());
    }
}

async fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Launch(command) => launch::launch_profile(command, &cli.global).await?,
        Command::Profile(command) => profile::handle_profile_command(command, &cli.global).await?,
        Command::Steam(command) => steam::handle_steam_command(command, &cli.global).await?,
        Command::Diagnostics(args) => diagnostics::handle_diagnostics_command(args, &cli.global)?,
        Command::Status => status::handle_status_command(&cli.global).await?,
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "crosshook", &mut std::io::stdout());
        }
    }

    Ok(())
}
