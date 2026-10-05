use crate::{Result, output, session::Session};
use clap::{Args, Parser, Subcommand, ValueEnum, ValueHint};
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
use repass_storage::{FieldUpdate, NewRecord, RecordId, RecordPatch, TagId};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Parser)]
#[command(
    name = "repass",
    version,
    about = "Password generation and vault commands",
    styles = output::styles()
)]
pub struct Cli {
    /// Data directory (overrides REPASS_DATA_DIR; default: $HOME/.repass)
    #[arg(long, global = true, value_hint = ValueHint::DirPath)]
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
    /// Print a shell completion script to stdout
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
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
    #[arg(long, value_hint = ValueHint::FilePath)]
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
    Switch {
        #[arg(value_hint = ValueHint::DirPath)]
        dir: PathBuf,
    },
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
    Rename {
        tag_id: TagId,
        #[arg(long)]
        name: String,
    },
    /// Rebuild a missing or damaged tag-name catalog using technical names
    Recover,
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
        Command::Completions { shell } => crate::completions::generate(shell, output),
        Command::Vault {
            command: VaultCommand::Switch { dir },
        } if interactive => {
            session.switch(dir)?;
            output::styled(output, output::SUCCESS, "Data directory:")?;
            writeln!(output, " {}", session.data_dir().display())?;
            Ok(())
        }
        Command::Vault {
            command: VaultCommand::Switch { .. },
        } => Err("vault switch is available only in interactive mode".into()),
        Command::Interactive => Err("already in interactive mode".into()),
        Command::Vault {
            command: VaultCommand::Init,
        } => {
            session.initialize_storage(output, interactive)?;
            success(
                output,
                format_args!("Vault initialized in {}", session.data_dir().display()),
            )
        }
        Command::Vault {
            command: VaultCommand::Info,
        } => {
            let storage = session.ensure_storage(output, interactive)?;
            let info = storage.info();
            writeln!(output, "Data directory: {}", info.directory.display())?;
            writeln!(output, "Records: {}", info.record_count)?;
            writeln!(output, "Tags: {}", info.tag_count)?;
            match info.tag_catalog {
                repass_storage::TagCatalogStatus::Present => {
                    writeln!(output, "Tag catalog: present")?
                }
                repass_storage::TagCatalogStatus::Missing => writeln!(
                    output,
                    "Tag catalog: missing (unknown IDs use technical names)"
                )?,
                repass_storage::TagCatalogStatus::Unavailable(reason) => {
                    writeln!(output, "Tag catalog: unavailable ({reason})")?
                }
            }
            Ok(())
        }
        Command::Record { command } => execute_record(command, session, input, output, interactive),
        Command::Tag { command } => execute_tag(command, session, output, interactive),
    }
}

fn execute_record(
    command: RecordCommand,
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    match command {
        RecordCommand::Add {
            name,
            password_stdin: _,
            username,
            url,
            notes,
            tag,
        } => {
            let storage = session.ensure_storage(output, interactive)?;
            let password = read_record_password(input, output, interactive)?;
            let id = storage.create_record(NewRecord {
                name,
                password,
                username,
                url,
                notes,
                tags: tag,
            })?;
            success(output, format_args!("Record created with ID {id}"))
        }
        RecordCommand::List { tag } => {
            let storage = session.ensure_storage(output, interactive)?;
            let tags = storage.list_tags();
            let records = storage.list_records(tag)?;
            for record in records {
                let tag_names = record
                    .tags
                    .iter()
                    .map(|id| {
                        tags.iter()
                            .find(|tag| tag.id() == *id)
                            .map(|tag| {
                                if tag.is_technical() {
                                    format!("{}:{} (technical)", id, tag.name())
                                } else {
                                    format!("{}:{}", id, tag.name())
                                }
                            })
                            .unwrap_or_else(|| format!("{id}:#tag-{id} (technical)"))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}",
                    record.id,
                    record.name,
                    record.username.unwrap_or(""),
                    tag_names
                )?;
            }
            Ok(())
        }
        RecordCommand::Show { record_id, reveal } => {
            let id = parse_record_id(&record_id)?;
            let storage = session.ensure_storage(output, interactive)?;
            let tags = storage.list_tags();
            let record = storage.get_record(id)?;
            writeln!(output, "ID: {}", record.id)?;
            writeln!(output, "Name: {}", record.name)?;
            writeln!(output, "Username: {}", record.username.unwrap_or(""))?;
            writeln!(output, "URL: {}", record.url.unwrap_or(""))?;
            writeln!(output, "Notes: {}", record.notes.unwrap_or(""))?;
            let tag_names = record
                .tags
                .iter()
                .map(|id| {
                    tags.iter()
                        .find(|tag| tag.id() == *id)
                        .map(|tag| {
                            if tag.is_technical() {
                                format!("{} (technical)", tag.name())
                            } else {
                                tag.name().to_owned()
                            }
                        })
                        .unwrap_or_else(|| format!("#tag-{id}"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(output, "Tags: {tag_names}")?;
            writeln!(
                output,
                "Password: {}",
                if reveal {
                    record.password()
                } else {
                    "********"
                }
            )?;
            writeln!(
                output,
                "Created (Unix ms): {}",
                record.created.as_unix_millis()
            )?;
            writeln!(
                output,
                "Updated (Unix ms): {}",
                record.updated.as_unix_millis()
            )?;
            Ok(())
        }
        RecordCommand::Update {
            record_id,
            name,
            username,
            url,
            notes,
            password_stdin,
            add_tag,
            remove_tag,
        } => {
            let id = parse_record_id(&record_id)?;
            let storage = session.ensure_storage(output, interactive)?;
            let password = if password_stdin {
                Some(read_record_password(input, output, interactive)?)
            } else {
                None
            };
            let changed = storage.update_record(
                id,
                RecordPatch {
                    name,
                    password,
                    username: username.map_or(FieldUpdate::Keep, FieldUpdate::Set),
                    url: url.map_or(FieldUpdate::Keep, FieldUpdate::Set),
                    notes: notes.map_or(FieldUpdate::Keep, FieldUpdate::Set),
                    add_tags: add_tag,
                    remove_tags: remove_tag,
                },
            )?;
            if changed {
                success(output, format_args!("Record {id} updated"))
            } else {
                writeln!(output, "Record {id} unchanged")?;
                Ok(())
            }
        }
        RecordCommand::Delete { record_id } => {
            let id = parse_record_id(&record_id)?;
            session
                .ensure_storage(output, interactive)?
                .delete_record(id)?;
            success(output, format_args!("Record {id} deleted"))
        }
    }
}

fn execute_tag(
    command: TagCommand,
    session: &mut Session,
    output: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    match command {
        TagCommand::Add { name } => {
            let id = session
                .ensure_storage(output, interactive)?
                .create_tag(name)?;
            success(output, format_args!("Tag created with ID {id}"))
        }
        TagCommand::List => {
            let storage = session.ensure_storage(output, interactive)?;
            for tag in storage.list_tags() {
                if tag.is_technical() {
                    writeln!(output, "{}\t{}\t(technical)", tag.id(), tag.name())?;
                } else {
                    writeln!(output, "{}\t{}", tag.id(), tag.name())?;
                }
            }
            Ok(())
        }
        TagCommand::Delete { tag_id } => {
            session
                .ensure_storage(output, interactive)?
                .delete_tag(tag_id)?;
            success(output, format_args!("Tag {tag_id} deleted"))
        }
        TagCommand::Rename { tag_id, name } => {
            session
                .ensure_storage(output, interactive)?
                .rename_tag(tag_id, name)?;
            success(output, format_args!("Tag {tag_id} renamed"))
        }
        TagCommand::Recover => {
            let count = session
                .ensure_storage(output, interactive)?
                .recover_tags()?;
            success(
                output,
                format_args!("Tag catalog rebuilt with {count} tags"),
            )
        }
    }
}

fn parse_record_id(value: &str) -> Result<RecordId> {
    Ok(RecordId::from_str(value)?)
}

fn read_record_password(
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<String> {
    if interactive {
        output::styled(output, output::PROMPT, "Password: ")?;
        output.flush()?;
        return Ok(rpassword::read_password()?);
    }
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
    Ok(password)
}

fn success(output: &mut impl Write, message: impl std::fmt::Display) -> Result<()> {
    output::styled(output, output::SUCCESS, message)?;
    writeln!(output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use repass_storage::Storage;
    use std::fs;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("repass-cli-commands-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn record_add_reads_secret_from_stdin_and_show_masks_unless_revealed() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        let mut session = Session::with_storage(directory.0.clone(), storage);
        let mut output = Vec::new();
        execute(
            Command::Record {
                command: RecordCommand::Add {
                    name: "mail".into(),
                    password_stdin: true,
                    username: Some("alice".into()),
                    url: None,
                    notes: None,
                    tag: Vec::new(),
                },
            },
            &mut session,
            &mut Cursor::new(b"sensitive-password\n".to_vec()),
            &mut output,
            false,
        )
        .unwrap();
        let added = String::from_utf8(output).unwrap();
        assert!(added.contains("Record created with ID 1"));
        assert!(!added.contains("sensitive-password"));

        let mut masked = Vec::new();
        execute(
            Command::Record {
                command: RecordCommand::Show {
                    record_id: "1".into(),
                    reveal: false,
                },
            },
            &mut session,
            &mut Cursor::new(Vec::new()),
            &mut masked,
            false,
        )
        .unwrap();
        let masked = String::from_utf8(masked).unwrap();
        assert!(masked.contains("Password: ********"));
        assert!(!masked.contains("sensitive-password"));

        let mut revealed = Vec::new();
        execute(
            Command::Record {
                command: RecordCommand::Show {
                    record_id: "1".into(),
                    reveal: true,
                },
            },
            &mut session,
            &mut Cursor::new(Vec::new()),
            &mut revealed,
            false,
        )
        .unwrap();
        assert!(
            String::from_utf8(revealed)
                .unwrap()
                .contains("Password: sensitive-password")
        );
    }

    #[test]
    fn tag_rename_persists_a_technical_tag_and_recover_is_explicit() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag_id = storage.create_tag("old").unwrap();
        storage
            .create_record(NewRecord {
                name: "mail".into(),
                password: "secret".into(),
                username: None,
                url: None,
                notes: None,
                tags: vec![tag_id],
            })
            .unwrap();
        drop(storage);
        fs::remove_file(directory.0.join("tags.repass")).unwrap();
        let storage = Storage::open_in(&directory.0, b"master").unwrap();
        assert!(storage.list_tags()[0].is_technical());
        let mut session = Session::with_storage(directory.0.clone(), storage);
        execute(
            Command::Tag {
                command: TagCommand::Rename {
                    tag_id,
                    name: "new".into(),
                },
            },
            &mut session,
            &mut Cursor::new(Vec::new()),
            &mut Vec::new(),
            false,
        )
        .unwrap();
        assert_eq!(
            Storage::open_in(&directory.0, b"master")
                .unwrap()
                .list_tags()[0]
                .name(),
            "new"
        );
        let mut output = Vec::new();
        execute(
            Command::Tag {
                command: TagCommand::Recover,
            },
            &mut session,
            &mut Cursor::new(Vec::new()),
            &mut output,
            false,
        )
        .unwrap();
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("Tag catalog rebuilt")
        );
    }
}
