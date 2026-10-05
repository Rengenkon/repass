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
    let data_dir = command_data_dir(&command);
    if matches!(
        command,
        Command::Vault {
            command: commands::VaultCommand::Switch { .. },
            ..
        }
    ) {
        return Err("vault switch is available only in interactive mode".into());
    }
    match command {
        Command::Completions { shell } => completions::generate(shell, &mut io::stdout().lock()),
        Command::Generate(args) => run_generate(args),
        Command::Interactive(args) => {
            let mut session = session::Session::new(session::resolve_data_dir(data_dir)?);
            session.set_warnings_enabled(args.warnings_enabled());
            let stdin = io::stdin();
            let mut input = stdin.lock();
            let mut output = anstream::AutoStream::auto(io::stdout().lock());
            interactive::run(&mut session, &mut input, &mut output)
        }
        Command::Warnings { .. } => Err("warnings is available only in interactive mode".into()),
        command => {
            let mut session = session::Session::new(session::resolve_data_dir(data_dir)?);
            let stdin = io::stdin();
            let mut input = stdin.lock();
            let mut output = anstream::AutoStream::auto(io::stdout().lock());
            commands::execute(command, &mut session, &mut input, &mut output, false)
        }
    }
}

fn command_data_dir(command: &Command) -> Option<std::path::PathBuf> {
    match command {
        Command::Vault { data_dir, .. }
        | Command::Record { data_dir, .. }
        | Command::Tag { data_dir, .. } => data_dir.clone(),
        Command::Interactive(args) => args.data_dir.clone(),
        _ => None,
    }
}

fn run_generate(args: commands::GenerateArgs) -> Result<()> {
    let (passwords, warnings) = commands::generate(args, true)?;
    let stderr = io::stderr();
    let mut warning_output = anstream::AutoStream::auto(stderr.lock());
    for warning in warnings {
        output::warning(&mut warning_output, warning)?;
    }
    let stdout = io::stdout();
    let mut output = anstream::AutoStream::auto(stdout.lock());
    for password in passwords {
        writeln!(output, "{password}")?;
    }
    Ok(())
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
