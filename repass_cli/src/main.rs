mod commands;
mod completions;
mod interactive;
mod output;
mod session;

use clap::{CommandFactory, Parser};
use commands::{Cli, Command};
use std::io::{self, Write};
use std::process::ExitCode;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn run() -> Result<()> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        writeln!(io::stdout())?;
        return Ok(());
    };
    if let Command::Completions { shell } = command {
        return completions::generate(shell, &mut io::stdout().lock());
    }
    if matches!(
        command,
        Command::Vault {
            command: commands::VaultCommand::Switch { .. }
        }
    ) {
        return Err("vault switch is available only in interactive mode".into());
    }
    let mut session = session::Session::new(session::resolve_data_dir(cli.data_dir)?);
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut output = anstream::AutoStream::auto(io::stdout().lock());
    match command {
        Command::Interactive => interactive::run(&mut session, &mut input, &mut output),
        command => commands::execute(command, &mut session, &mut input, &mut output, false),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = output::error(&mut anstream::AutoStream::auto(io::stderr().lock()), error);
            ExitCode::FAILURE
        }
    }
}
