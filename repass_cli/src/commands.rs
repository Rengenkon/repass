use crate::{Result, session::Session};
use clap::{Args, Parser, Subcommand};
use repass_generator::dictionary::{
    cache::DictionaryCache, file_dictionary::FileDictionary, presets,
};
use repass_generator::generator::generate_multi;
use repass_generator::query::{PasswordLength, Query};
use repass_generator::separator::{
    Separator, between_parts::BetweenPartsSeparator, without_separator::WithoutSeparator,
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
    /// Start a persistent interactive session (also the default without a command)
    Interactive,
}

#[derive(Args)]
pub struct GenerateArgs {
    /// Exact length in Unicode scalar values, including separators
    #[arg(long, value_parser = positive_usize)]
    pub length: usize,
    #[arg(long, default_value = "1", value_parser = positive_usize)]
    pub count: usize,
    /// UTF-8 dictionary, one entry per line; default: all built-in character sets
    #[arg(long)]
    pub dictionary: Option<PathBuf>,
    /// Separator between entries; default: "-"; empty string disables separation
    #[arg(long, allow_hyphen_values = true)]
    pub separator: Option<String>,
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
    let dictionary = match args.dictionary {
        Some(path) => DictionaryCache::new(FileDictionary::from_path(path)?)?,
        None => DictionaryCache::new(presets::all_presets())?,
    };
    let separator: Box<dyn Separator> = match args.separator.as_deref() {
        Some("") => Box::new(WithoutSeparator),
        Some(text) => Box::new(BetweenPartsSeparator::new(text)),
        None => Box::new(BetweenPartsSeparator::default()),
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
