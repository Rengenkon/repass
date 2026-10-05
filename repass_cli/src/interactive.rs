use crate::{
    Result,
    commands::{self, Command},
    session::Session,
};
use clap::{ArgAction, ArgMatches, CommandFactory, FromArgMatches, Parser, parser::ValueSource};
use std::io::{self, BufRead, Write};

#[derive(Parser)]
#[command(
    name = "repass",
    about = "Interactive commands; use vault switch <DIR> to select a vault"
)]
struct Line {
    #[command(subcommand)]
    command: Command,
}

fn schema() -> clap::Command {
    // Build from the shared application schema, excluding session entrypoints
    // entirely so they cannot be parsed, suggested, or shown in session help.
    let subcommands: Vec<_> = Line::command()
        .get_subcommands()
        .filter(|command| command.get_name() != "interactive")
        .cloned()
        .collect();
    let mut command = clap::Command::new("repass")
        .about("Session commands; use vault switch <DIR> to select a vault")
        .subcommand_required(true)
        .subcommands(subcommands)
        .mut_subcommand("vault", |vault| {
            vault.mut_subcommand("switch", |switch| switch.hide(false))
        });
    command.build();
    command = command.mut_subcommand("help", |help| help.visible_alias("h"));
    command.build();
    command
}

fn optional_arguments(command: clap::Command) -> clap::Command {
    command
        .mut_args(|arg| arg.required(false))
        .mut_subcommands(optional_arguments)
}

struct Missing {
    name: String,
    long: Option<String>,
    flag: bool,
}

fn missing_arguments(command: &clap::Command, matches: &ArgMatches, missing: &mut Vec<Missing>) {
    for arg in command.get_arguments() {
        if arg.is_required_set()
            && matches.value_source(arg.get_id().as_str()) != Some(ValueSource::CommandLine)
        {
            missing.push(Missing {
                name: arg.get_id().to_string(),
                long: arg.get_long().map(str::to_owned),
                flag: matches!(arg.get_action(), ArgAction::SetTrue),
            });
        }
    }
    if let Some((name, submatches)) = matches.subcommand() {
        if let Some(subcommand) = command.find_subcommand(name) {
            missing_arguments(subcommand, submatches, missing);
        }
    }
}

fn parse(words: Vec<String>, input: &mut impl BufRead, output: &mut impl Write) -> Result<Command> {
    let strict = schema();
    let mut arguments = vec!["repass".to_owned()];
    arguments.extend(words);
    let matches = optional_arguments(strict.clone()).try_get_matches_from(&arguments)?;
    let mut missing = Vec::new();
    missing_arguments(&strict, &matches, &mut missing);
    for arg in missing {
        let value = if arg.flag {
            // --password-stdin is mandatory for add. Supply the flag and let the
            // shared executor request the password with hidden terminal input.
            None
        } else {
            if arg.name == "separator_kind" {
                writeln!(output, "{}", commands::SEPARATOR_HELP)?;
            }
            write!(output, "{}: ", arg.long.as_deref().unwrap_or(&arg.name))?;
            output.flush()?;
            let mut value = String::new();
            if input.read_line(&mut value)? == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "input ended while requesting an argument",
                )
                .into());
            }
            while value.ends_with(['\n', '\r']) {
                value.pop();
            }
            if value.is_empty() {
                return Err(format!("{} must not be empty", arg.name).into());
            }
            Some(value)
        };
        if let Some(long) = arg.long {
            // Insert options before a possible `--` positional delimiter.
            let index = arguments
                .iter()
                .position(|value| value == "--")
                .unwrap_or(arguments.len());
            let argument = match value {
                Some(value) => format!("--{long}={value}"),
                None => format!("--{long}"),
            };
            arguments.insert(index, argument);
        } else if let Some(value) = value {
            // A delimiter keeps prompted IDs/paths starting with '-' positional.
            if !arguments.iter().any(|argument| argument == "--") {
                arguments.push("--".into());
            }
            arguments.push(value);
        }
    }
    let matches = strict.try_get_matches_from(arguments)?;
    let command = Line::from_arg_matches(&matches)?.command;
    if let Command::Generate(args) = &command {
        args.validate()?;
    }
    Ok(command)
}

pub fn run(session: &mut Session, input: &mut impl BufRead, output: &mut impl Write) -> Result<()> {
    writeln!(
        output,
        "Interactive mode. Use h or help for help; q or quit to leave."
    )?;
    loop {
        write!(output, "repass> ")?;
        output.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            break;
        }
        let Some(words) = shlex::split(&line) else {
            writeln!(output, "error: unmatched quote or incomplete escape")?;
            continue;
        };
        if words.is_empty() {
            continue;
        }
        if words.len() == 1 && matches!(words[0].as_str(), "q" | "quit") {
            break;
        }
        match parse(words, input, output) {
            Ok(command) => {
                if let Err(error) = commands::execute(command, session, input, output, true) {
                    writeln!(output, "error: {error}")?;
                }
            }
            Err(error) => {
                if let Some(clap_error) = error.downcast_ref::<clap::Error>() {
                    write!(output, "{clap_error}")?;
                } else if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() == io::ErrorKind::UnexpectedEof)
                {
                    break;
                } else {
                    writeln!(output, "error: {error}")?;
                }
            }
        }
    }
    session.close();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::ValueEnum;
    use std::io::Cursor;

    #[test]
    fn q_and_quit_leave_but_exit_is_an_unknown_command() {
        for quit in ["q", "quit"] {
            let mut session = Session::new("initial".into());
            let mut input = Cursor::new(format!("exit\n{quit}\nvault switch should-not-run\n"));
            let mut output = Vec::new();
            run(&mut session, &mut input, &mut output).unwrap();
            let text = String::from_utf8(output).unwrap();
            assert!(text.contains("unrecognized subcommand 'exit'"));
            assert!(!text.contains("should-not-run"));
            assert_eq!(session.data_dir(), std::path::Path::new("initial"));
        }
    }

    #[test]
    fn session_help_aliases_exclude_interactive_and_reject_it() {
        for suffix in ["", " generate", " record add"] {
            let help = schema()
                .try_get_matches_from(shlex::split(&format!("repass help{suffix}")).unwrap())
                .unwrap_err();
            let alias = schema()
                .try_get_matches_from(shlex::split(&format!("repass h{suffix}")).unwrap())
                .unwrap_err();
            assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
            assert_eq!(help.to_string(), alias.to_string());
            if suffix.is_empty() {
                assert!(!help.to_string().contains("interactive"));
            }
        }
        for words in [
            vec!["repass", "interactive"],
            vec!["repass", "help", "interactive"],
            vec!["repass", "h", "interactive"],
        ] {
            let error = schema().try_get_matches_from(words).unwrap_err();
            assert_ne!(error.kind(), clap::error::ErrorKind::DisplayHelp);
        }
    }

    #[test]
    fn separator_prompt_lists_numbered_choices_and_accepts_names_or_numbers() {
        for (name, number) in [
            ("none", "1"),
            ("between-parts", "2"),
            ("fixed-interval", "3"),
            ("fixed-count", "4"),
        ] {
            for value in [name, number] {
                let mut output = Vec::new();
                let command = parse(
                    shlex::split("generate --length 13").unwrap(),
                    &mut Cursor::new(format!("{value}\n")),
                    &mut output,
                )
                .unwrap();
                assert!(
                    matches!(command, Command::Generate(args) if args.separator_kind == commands::SeparatorKind::from_str(name, false).unwrap())
                );
                let text = String::from_utf8(output).unwrap();
                for choice in [
                    "1. none",
                    "2. between-parts",
                    "3. fixed-interval",
                    "4. fixed-count",
                ] {
                    assert!(text.contains(choice));
                }
            }
        }
        for value in ["0", "5", "unknown"] {
            assert!(
                parse(
                    shlex::split("generate --length 13").unwrap(),
                    &mut Cursor::new(format!("{value}\n")),
                    &mut Vec::new()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn missing_required_arguments_are_prompted_and_validated() {
        let mut output = Vec::new();
        let command = parse(
            vec!["generate".into()],
            &mut Cursor::new("7\n1\n"),
            &mut output,
        )
        .unwrap();
        assert!(matches!(command, Command::Generate(args) if args.length == 7 && args.count == 1));
        assert!(String::from_utf8(output).unwrap().contains("length: "));
        assert!(
            parse(
                vec!["generate".into()],
                &mut Cursor::new("0\n1\n"),
                &mut Vec::new()
            )
            .is_err()
        );
        let command = parse(
            vec!["tag".into(), "delete".into()],
            &mut Cursor::new("42\n"),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(matches!(
            command,
            Command::Tag {
                command: commands::TagCommand::Delete { tag_id: 42 }
            }
        ));
    }

    #[test]
    fn existing_arguments_and_quoted_values_do_not_prompt() {
        let words = shlex::split("record add --name 'My mail' --password-stdin").unwrap();
        let mut output = Vec::new();
        assert!(parse(words, &mut Cursor::new(""), &mut output).is_ok());
        assert!(output.is_empty());
        let command = parse(
            shlex::split("record add").unwrap(),
            &mut Cursor::new("My mail\n"),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(
            matches!(command, Command::Record { command: commands::RecordCommand::Add { name, password_stdin: true, .. } } if name == "My mail")
        );
    }

    #[test]
    fn interactive_commands_reject_directory_options() {
        for line in [
            "--data-dir other record list",
            "record list --data-dir other",
        ] {
            assert!(
                parse(
                    shlex::split(line).unwrap(),
                    &mut Cursor::new(""),
                    &mut Vec::new()
                )
                .is_err()
            );
        }
    }

    #[test]
    fn session_survives_errors_switches_and_generates() {
        let mut session = Session::new("initial".into());
        let mut input = Cursor::new(
            "record list\nunknown\n'bad\nvault switch 'another directory'\ngenerate\n5\n1\nhelp\nquit\n",
        );
        let mut output = Vec::new();
        run(&mut session, &mut input, &mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("TODO: repass_storage"));
        assert!(text.contains("unrecognized subcommand"));
        assert!(text.contains("unmatched quote"));
        assert!(text.contains("length: "));
        assert!(text.contains("Usage:"));
        assert_eq!(
            session.data_dir(),
            std::path::Path::new("another directory")
        );
    }
}
