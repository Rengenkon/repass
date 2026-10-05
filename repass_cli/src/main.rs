mod commands;
mod interactive;
mod session;

use clap::Parser;
use commands::{Cli, Command};
use std::io;
use std::process::ExitCode;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn run() -> Result<()> {
    let cli = Cli::parse();
    if matches!(
        cli.command,
        Some(Command::Vault {
            command: commands::VaultCommand::Switch { .. }
        })
    ) {
        return Err("vault switch is available only in interactive mode".into());
    }
    let mut session = session::Session::new(session::resolve_data_dir(cli.data_dir)?);
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = io::stdout().lock();
    match cli.command {
        None | Some(Command::Interactive) => {
            interactive::run(&mut session, &mut input, &mut output)
        }
        Some(command) => commands::execute(command, &mut session, &mut input, &mut output, false),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
