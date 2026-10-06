use crate::{
    Result,
    commands::{self, Command},
    output,
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
        .map(session_help)
        .collect();
    let mut command = clap::Command::new("repass")
        .about("Session commands; use vault switch <DIR> to select a vault")
        .after_help("Use help/h for help and quit/q to leave. Enter commands without the 'repass' prefix.\nMissing required arguments are prompted; optional arguments are not.")
        .styles(output::styles())
        .subcommand_required(true)
        .subcommands(subcommands)
        .mut_subcommand("vault", |vault| {
            vault.mut_subcommand("switch", |switch| switch.hide(false))
        })
        .mut_subcommand("warnings", |warnings| warnings.hide(false));
    command.build();
    command = command.mut_subcommand("help", |help| help.visible_alias("h"));
    command.build();
    command
}

fn optional_arguments(command: clap::Command) -> clap::Command {
    let command = if command
        .get_groups()
        .any(|group| group.get_id() == commands::primary::GROUP)
    {
        command.mut_group(commands::primary::GROUP, |group| group.required(false))
    } else {
        command
    };
    command
        .mut_args(|arg| {
            let arg = arg.required(false);
            if arg.get_id().as_str() == "length" {
                arg.required_unless_present(clap::builder::Resettable::<clap::Id>::Reset)
            } else {
                arg
            }
        })
        .mut_subcommands(optional_arguments)
}

const SESSION_INPUT_HELP: &str =
    "Missing required arguments are prompted; optional arguments are not.";

fn session_help(mut command: clap::Command) -> clap::Command {
    let short_footer = match command.get_after_help() {
        Some(footer) => format!("{footer}\n\n{SESSION_INPUT_HELP}"),
        None => SESSION_INPUT_HELP.to_owned(),
    };
    if let Some(footer) = command.get_after_long_help() {
        // Completion examples run in the external shell, not in this session.
        let footer = if command.get_name() == "completions" {
            footer.to_string()
        } else {
            footer.to_string().replace("repass ", "")
        };
        command = command.after_long_help(format!("{footer}\n\n{SESSION_INPUT_HELP}"));
    }
    command.after_help(short_footer)
        .mut_args(|arg| {
            match arg.get_id().as_str() {
                "data_dir" => arg.hide(true),
                "password_stdin" => arg
                    .help("Enter one password using hidden input")
                    .long_help("Requests the password using hidden terminal input. The master password is requested separately when opening the vault."),
                "code_stdin" => arg
                    .help("Enter a recovery code or other code using hidden input")
                    .long_help("Requests one code using hidden terminal input. The code must not be blank."),
                "totp_stdin" => arg
                    .help("Enter an uppercase Base32 TOTP secret using hidden input")
                    .long_help("Requests an uppercase Base32 TOTP secret using hidden terminal input. Accepts unpadded Base32 or canonical padding; saves without padding. Use record show --data-id ID --totp-code to generate the current code. Defaults: SHA-1, 6 digits, 30 seconds."),
                "private_key_stdin" => arg
                    .help("Enter a private SSH key; finish with a line containing only '.'")
                    .long_help("Uses ordinary multiline terminal input. Finish with a line containing only '.'. Whitespace and line endings are preserved; the terminator is not stored. EOF before the terminator cancels the operation. Both SSH parts may be entered separately."),
                "public_key_stdin" => arg
                    .help("Enter a public SSH key; finish with a line containing only '.'")
                    .long_help("Uses ordinary multiline terminal input. Finish with a line containing only '.'. Whitespace and line endings are preserved; the terminator is not stored. EOF before the terminator cancels the operation. Both SSH parts may be entered separately."),
                _ => arg,
            }
        })
        .mut_subcommands(session_help)
}

fn uses_data_dir(command: &Command) -> bool {
    match command {
        Command::Vault { data_dir, .. }
        | Command::Record { data_dir, .. }
        | Command::Tag { data_dir, .. } => data_dir.is_some(),
        _ => false,
    }
}

struct Missing {
    name: String,
    long: Option<String>,
    flag: bool,
}

fn missing_arguments(command: &clap::Command, matches: &ArgMatches, missing: &mut Vec<Missing>) {
    if let Some(group) = command
        .get_groups()
        .find(|group| group.get_id() == commands::primary::GROUP)
    {
        let ids: Vec<_> = group.get_args().collect();
        if !ids.iter().any(|id| matches.contains_id(id.as_str())) {
            let named = command
                .get_arguments()
                .find(|arg| ids.contains(&arg.get_id()) && arg.get_long().is_some());
            if let Some(arg) = named {
                missing.push(Missing {
                    name: arg.get_id().to_string(),
                    long: arg.get_long().map(str::to_owned),
                    flag: false,
                });
            }
        }
    }
    for arg in command.get_arguments() {
        if arg.is_required_set()
            || (arg.get_id().as_str() == "length"
                && matches.value_source("length") != Some(ValueSource::CommandLine)
                && matches.value_source("min_length") != Some(ValueSource::CommandLine))
        {
            if matches.value_source(arg.get_id().as_str()) == Some(ValueSource::CommandLine) {
                continue;
            }
            missing.push(Missing {
                name: arg.get_id().to_string(),
                long: arg.get_long().map(str::to_owned),
                flag: matches!(arg.get_action(), ArgAction::SetTrue),
            });
        }
    }
    if let Some((name, submatches)) = matches.subcommand()
        && let Some(subcommand) = command.find_subcommand(name)
    {
        missing_arguments(subcommand, submatches, missing);
    }
}

fn parse(words: Vec<String>, input: &mut impl BufRead, output: &mut impl Write) -> Result<Command> {
    let strict = schema();
    let mut arguments = vec!["repass".to_owned()];
    arguments.extend(words);
    let matches = match optional_arguments(strict.clone()).try_get_matches_from(&arguments) {
        Ok(matches) => matches,
        Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
            // The relaxed parser exists only to gather arguments for prompting.
            // Render help from the canonical schema, with required arguments.
            return Err(strict
                .try_get_matches_from(&arguments)
                .err()
                .unwrap_or(error)
                .into());
        }
        Err(error) => return Err(error.into()),
    };
    let mut missing = Vec::new();
    missing_arguments(&strict, &matches, &mut missing);
    for arg in missing {
        let value = if arg.flag {
            // Supply a required flag; the executor handles its input.
            None
        } else {
            if arg.name == "separator_kind" {
                output::styled(output, output::HEADING, commands::SEPARATOR_HELP)?;
                writeln!(output)?;
            }
            output::styled(
                output,
                output::PROMPT,
                format_args!("{}: ", arg.long.as_deref().unwrap_or(&arg.name)),
            )?;
            output.flush()?;
            let mut value = crate::input::read_line(input, crate::input::MAX_TEXT_BYTES)?;
            if value.is_empty() {
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
    if uses_data_dir(&command) {
        return Err("interactive commands cannot override the session data directory".into());
    }
    Ok(command)
}

pub fn run(session: &mut Session, input: &mut impl BufRead, output: &mut impl Write) -> Result<()> {
    output::styled(output, output::HEADING, "Interactive mode.")?;
    writeln!(output, " Use h or help for help; q or quit to leave.")?;
    loop {
        output::styled(output, output::PROMPT, "repass> ")?;
        output.flush()?;
        let line = crate::input::read_line(input, crate::input::MAX_TEXT_BYTES)?;
        if line.is_empty() {
            break;
        }
        let Some(words) = shlex::split(&line) else {
            output::error(output, "unmatched quote or incomplete escape")?;
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
                    output::error(output, error)?;
                }
            }
            Err(error) => {
                if let Some(clap_error) = error.downcast_ref::<clap::Error>() {
                    write!(output, "{}", clap_error.render().ansi())?;
                } else if error
                    .downcast_ref::<io::Error>()
                    .is_some_and(|error| error.kind() == io::ErrorKind::UnexpectedEof)
                {
                    break;
                } else {
                    output::error(output, error)?;
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
            run(
                &mut session,
                &mut input,
                &mut anstream::AutoStream::new(&mut output, anstream::ColorChoice::Never),
            )
            .unwrap();
            let text = String::from_utf8(output).unwrap();
            assert!(text.contains("unrecognized subcommand 'exit'"));
            assert!(!text.contains("should-not-run"));
            assert_eq!(session.data_dir(), std::path::Path::new("initial"));
        }
    }

    #[test]
    fn session_help_aliases_exclude_interactive_and_reject_it() {
        for suffix in [
            "",
            " generate",
            " record create",
            " record update",
            " vault switch",
            " completions",
        ] {
            let help = parse(
                shlex::split(&format!("help{suffix}")).unwrap(),
                &mut Cursor::new(""),
                &mut Vec::new(),
            )
            .err()
            .unwrap()
            .downcast::<clap::Error>()
            .unwrap();
            let alias = parse(
                shlex::split(&format!("h{suffix}")).unwrap(),
                &mut Cursor::new(""),
                &mut Vec::new(),
            )
            .err()
            .unwrap()
            .downcast::<clap::Error>()
            .unwrap();
            assert_eq!(help.kind(), clap::error::ErrorKind::DisplayHelp);
            assert_eq!(help.to_string(), alias.to_string());
            if suffix.is_empty() {
                assert!(!help.to_string().contains("interactive"));
            }
            if suffix == " record update" {
                let text = help.to_string();
                assert!(text.contains("<RECORD_ID|--record-id <RECORD_ID>>"));
                assert!(text.contains(SESSION_INPUT_HELP));
                assert!(text.contains("line containing only '.'"));
                assert!(!text.contains("stdin until EOF"));
                assert!(text.contains("update 1 --replace-data 2 --private-key-file"));
                assert!(!text.contains("repass record update 1"));
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
    fn session_help_flags_keep_required_ids_and_do_not_prompt() {
        for flag in ["-h", "--help"] {
            let mut output = Vec::new();
            let error = parse(
                vec!["record".into(), "update".into(), flag.into()],
                &mut Cursor::new(""),
                &mut output,
            )
            .err()
            .unwrap()
            .downcast::<clap::Error>()
            .unwrap();
            assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
            let text = error.to_string();
            assert!(text.contains("<RECORD_ID|--record-id <RECORD_ID>>"));
            assert!(!text.contains("--data-dir"));
            assert!(text.contains(SESSION_INPUT_HELP));
            assert!(output.is_empty());
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
        assert!(
            matches!(command, Command::Generate(args) if args.length == Some(7) && args.count == 1)
        );
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
            vec!["tag".into(), "remove".into()],
            &mut Cursor::new("42\n"),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(matches!(
            command,
            Command::Tag {
                command: commands::TagCommand::Remove { tag_id },
                ..
            } if tag_id.0 == 42.into()
        ));
    }

    #[test]
    fn primary_named_forms_and_prompting_work_with_options() {
        for line in [
            "record create --tag 1 --name 'My mail'",
            "record show --reveal --record-id 1",
            "tag create --name work",
            "tag remove --tag-id 2",
            "vault switch --dir other",
            "completions --shell bash",
        ] {
            let mut output = Vec::new();
            assert!(
                parse(
                    shlex::split(line).unwrap(),
                    &mut Cursor::new(""),
                    &mut output
                )
                .is_ok(),
                "{line}"
            );
            assert!(output.is_empty());
        }
        let mut output = Vec::new();
        let command = parse(
            shlex::split("record create --tag 1").unwrap(),
            &mut Cursor::new("My mail\n"),
            &mut output,
        )
        .unwrap();
        assert!(
            matches!(command, Command::Record { command: commands::RecordCommand::Create { name, tag, .. }, .. } if name == "My mail" && tag == vec![1.into()])
        );
        assert!(String::from_utf8(output).unwrap().contains("name: "));
        let error = parse(
            shlex::split("record show --reveal 1").unwrap(),
            &mut Cursor::new(""),
            &mut Vec::new(),
        )
        .err()
        .unwrap();
        let error = error.downcast_ref::<clap::Error>().unwrap();
        assert!(error.to_string().contains("otherwise use --record-id"));
        assert!(error.to_string().ends_with('\n'));
    }

    #[test]
    fn existing_arguments_and_quoted_values_do_not_prompt() {
        let words = shlex::split("record create 'My mail' --password-stdin").unwrap();
        let mut output = Vec::new();
        assert!(parse(words, &mut Cursor::new(""), &mut output).is_ok());
        assert!(output.is_empty());
        let command = parse(
            shlex::split("record create").unwrap(),
            &mut Cursor::new("My mail\n"),
            &mut Vec::new(),
        )
        .unwrap();
        assert!(
            matches!(command, Command::Record { command: commands::RecordCommand::Create { name, data, .. }, .. } if name == "My mail" && !data.password_stdin)
        );
    }

    #[test]
    fn complete_range_does_not_prompt_for_an_exact_length() {
        let mut output = Vec::new();
        let command = parse(
            shlex::split("generate --min-length 4 --max-length 8 --separator-kind none").unwrap(),
            &mut Cursor::new(""),
            &mut output,
        )
        .unwrap();
        assert!(
            matches!(command, Command::Generate(args) if args.length.is_none() && args.min_length == Some(4) && args.max_length == Some(8))
        );
        assert!(output.is_empty());
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
    fn interactive_storage_help_hides_data_directory_override() {
        let error = schema()
            .try_get_matches_from(["repass", "help", "record", "list"])
            .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
        assert!(!error.to_string().contains("--data-dir"));
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
        assert!(text.contains("Master password:"));
        assert!(!text.contains("TODO: repass_storage"));
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
