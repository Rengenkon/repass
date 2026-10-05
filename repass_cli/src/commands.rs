use crate::{Result, session::Session};
use clap::{Args, Parser, Subcommand, ValueEnum};
use repass_generator::dictionary::{
    cache::DictionaryCache, file_dictionary::FileDictionary, presets,
};
use repass_generator::generator::generate_multi;
use repass_generator::query::{PasswordLength, Query};
use repass_generator::separator::{
    DEFAULT_SEPARATOR, Separator, between_parts::BetweenPartsSeparator,
    fixed_count::FixedCountSeparator, fixed_interval::FixedIntervalSeparator,
    without_separator::WithoutSeparator,
};
use repass_storage::tags::TagId;
use std::io::{BufRead, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "repass",
    version,
    about = "Password generation and vault commands"
)]
pub struct Cli {
    /// Data directory (overrides REPASS_DATA_DIR; default: $HOME/.repass)
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate passwords without opening the vault
    Generate(GenerateArgs),
    /// Initialize or inspect a vault
    Vault {
        #[command(subcommand)]
        command: VaultCommand,
    },
    /// Manage records using stable IDs
    Record {
        #[command(subcommand)]
        command: RecordCommand,
    },
    /// Manage tags using stable IDs
    Tag {
        #[command(subcommand)]
        command: TagCommand,
    },
    /// Start a persistent interactive session
    Interactive,
}

#[derive(Args)]
pub struct GenerateArgs {
    /// Exact length in Unicode scalar values, including separators
    #[arg(long, value_parser = positive_usize)]
    pub length: usize,
    #[arg(long, value_enum, hide_possible_values = true, help = SEPARATOR_HELP)]
    pub separator_kind: SeparatorKind,
    #[arg(long, default_value = "1", value_parser = positive_usize)]
    pub count: usize,
    /// UTF-8 dictionary, one entry per line; default: all built-in character sets
    #[arg(long)]
    pub dictionary: Option<PathBuf>,
    /// Separator text; default: "-"; not applicable to none
    #[arg(long, allow_hyphen_values = true, value_parser = nonempty_separator)]
    pub separator: Option<String>,
    /// Unicode scalar interval for fixed-interval (default: 5)
    #[arg(long, value_parser = positive_usize)]
    pub separator_interval: Option<usize>,
    /// Number of insertions for fixed-count (default: 3; reduced on short content)
    #[arg(long, value_parser = positive_usize)]
    pub separator_count: Option<usize>,
}

pub const SEPARATOR_HELP: &str = "Separator strategy (required; enter a name or number):
1. none — join dictionary entries without a separator
2. between-parts — insert the separator between dictionary entries
3. fixed-interval — insert after each interval of Unicode scalar values in the content
4. fixed-count — distribute the requested number of separators across the content";

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SeparatorKind {
    #[value(alias = "1")]
    None,
    #[value(alias = "2")]
    BetweenParts,
    #[value(alias = "3")]
    FixedInterval,
    #[value(alias = "4")]
    FixedCount,
}

fn nonempty_separator(value: &str) -> std::result::Result<String, String> {
    if value.is_empty() {
        Err("separator must not be empty; use --separator-kind none to disable separation".into())
    } else {
        Ok(value.into())
    }
}

impl GenerateArgs {
    pub fn validate(&self) -> Result<()> {
        if self.separator_kind == SeparatorKind::None && self.separator.is_some() {
            return Err("--separator is not applicable to --separator-kind none (1)".into());
        }
        if self.separator_interval.is_some() && self.separator_kind != SeparatorKind::FixedInterval
        {
            return Err("--separator-interval requires --separator-kind fixed-interval (3)".into());
        }
        if self.separator_count.is_some() && self.separator_kind != SeparatorKind::FixedCount {
            return Err("--separator-count requires --separator-kind fixed-count (4)".into());
        }
        Ok(())
    }
}

fn positive_usize(value: &str) -> std::result::Result<usize, String> {
    match value.parse::<usize>() {
        Ok(value) if value > 0 => Ok(value),
        _ => Err("expected a positive integer".into()),
    }
}

#[derive(Subcommand)]
pub enum VaultCommand {
    /// Explicitly initialize a vault without overwriting an existing one
    Init,
    /// Show vault information
    Info,
    /// Close the current vault and select another directory (interactive only)
    #[command(hide = true)]
    Switch { dir: PathBuf },
}

#[derive(Subcommand)]
pub enum RecordCommand {
    Add {
        #[arg(long)]
        name: String,
        /// Read a password line from stdin; interactive mode uses hidden input
        #[arg(long, required = true)]
        password_stdin: bool,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, num_args = 1..)]
        tag: Vec<TagId>,
    },
    List {
        #[arg(long)]
        tag: Option<TagId>,
    },
    Show {
        record_id: String,
        /// Include the password in the output
        #[arg(long)]
        reveal: bool,
    },
    Update {
        record_id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long)]
        password_stdin: bool,
        #[arg(long, num_args = 1..)]
        add_tag: Vec<TagId>,
        #[arg(long, num_args = 1..)]
        remove_tag: Vec<TagId>,
    },
    Delete {
        record_id: String,
    },
}

#[derive(Subcommand)]
pub enum TagCommand {
    Add {
        #[arg(long)]
        name: String,
    },
    List,
    Delete {
        tag_id: TagId,
    },
}

pub fn generate(args: GenerateArgs) -> Result<Vec<String>> {
    args.validate()?;
    let dictionary = match args.dictionary {
        Some(path) => DictionaryCache::new(FileDictionary::from_path(path)?)?,
        None => DictionaryCache::new(presets::all_presets())?,
    };
    let text = args.separator.as_deref().unwrap_or(DEFAULT_SEPARATOR);
    let separator: Box<dyn Separator> = match args.separator_kind {
        SeparatorKind::None => Box::new(WithoutSeparator),
        SeparatorKind::BetweenParts => Box::new(BetweenPartsSeparator::new(text)),
        SeparatorKind::FixedInterval => Box::new(FixedIntervalSeparator::new(
            text,
            args.separator_interval.unwrap_or(5),
        )),
        SeparatorKind::FixedCount => Box::new(FixedCountSeparator::new(
            text,
            args.separator_count.unwrap_or(3),
        )),
    };
    let query = Query::new(PasswordLength::Exact(args.length), &dictionary, &*separator)?;
    Ok(generate_multi(&query, args.count)?)
}

pub fn execute(
    command: Command,
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    match command {
        Command::Generate(args) => {
            for password in generate(args)? {
                writeln!(output, "{password}")?;
            }
            Ok(())
        }
        Command::Vault {
            command: VaultCommand::Switch { dir },
        } if interactive => {
            session.switch(dir)?;
            writeln!(output, "Data directory: {}", session.data_dir().display())?;
            Ok(())
        }
        Command::Vault {
            command: VaultCommand::Switch { .. },
        } => Err("vault switch is available only in interactive mode".into()),
        Command::Interactive => Err("already in interactive mode".into()),
        command => {
            if matches!(
                &command,
                Command::Record {
                    command: RecordCommand::Add { .. }
                } | Command::Record {
                    command: RecordCommand::Update {
                        password_stdin: true,
                        ..
                    }
                }
            ) {
                // TODO: pass this secret to the storage mutation API once it exists.
                // Keep it local; it must never be added to parsed arguments or command history.
                let _password = if interactive {
                    rpassword::prompt_password("Password: ")?
                } else {
                    let mut password = String::new();
                    if input.read_line(&mut password)? == 0 {
                        return Err("expected a password line on stdin".into());
                    }
                    if password.ends_with('\n') {
                        password.pop();
                        if password.ends_with('\r') {
                            password.pop();
                        }
                    }
                    password
                };
            }
            session.storage_operation(storage_requirement(&command))
        }
    }
}

fn storage_requirement(command: &Command) -> &'static str {
    // TODO: wire each handler to the public storage API, retaining the CLI contract.
    // Records currently lack stable IDs and the agreed name/username/url/notes view.
    match command {
        Command::Vault {
            command: VaultCommand::Init,
        } => "non-destructive vault initialization",
        Command::Vault { .. } => "vault metadata",
        Command::Record {
            command: RecordCommand::Add { .. },
        } => "record creation with stable IDs and the agreed fields",
        Command::Record {
            command: RecordCommand::List { .. },
        } => "record listing and tag filtering",
        Command::Record {
            command: RecordCommand::Show { .. },
        } => "stable-ID lookup and password masking unless --reveal is set",
        Command::Record {
            command: RecordCommand::Update { .. },
        } => "partial record updates and consistent tag indexes",
        Command::Record {
            command: RecordCommand::Delete { .. },
        } => "record deletion by stable ID",
        Command::Tag {
            command: TagCommand::Add { .. },
        } => "persisted tag creation with unique names and IDs",
        Command::Tag {
            command: TagCommand::List,
        } => "persisted tag enumeration",
        Command::Tag {
            command: TagCommand::Delete { .. },
        } => "tag deletion that rejects tags referenced by records",
        _ => "vault operation",
    }
}
