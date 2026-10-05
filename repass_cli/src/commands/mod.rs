use crate::{Result, output, session::Session};
use clap::{Args, Parser, Subcommand, ValueEnum, ValueHint};
use repass_storage::{DataId, Host, RecordId, TagId};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::str::FromStr;

mod generation;
mod records;

pub use generation::generate;

#[derive(Parser)]
#[command(
    name = "repass",
    version,
    about = "Password generation and vault commands",
    styles = output::styles()
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate passwords without opening the vault
    Generate(GenerateArgs),
    /// Initialize or inspect a vault
    Vault {
        /// Data directory (overrides REPASS_DATA_DIR; default: $HOME/.repass)
        #[arg(long, global = true, value_hint = ValueHint::DirPath)]
        data_dir: Option<PathBuf>,
        #[command(subcommand)]
        command: VaultCommand,
    },
    /// Manage records using stable IDs
    Record {
        /// Data directory (overrides REPASS_DATA_DIR; default: $HOME/.repass)
        #[arg(long, global = true, value_hint = ValueHint::DirPath)]
        data_dir: Option<PathBuf>,
        #[command(subcommand)]
        command: RecordCommand,
    },
    /// Manage tags using stable IDs
    Tag {
        /// Data directory (overrides REPASS_DATA_DIR; default: $HOME/.repass)
        #[arg(long, global = true, value_hint = ValueHint::DirPath)]
        data_dir: Option<PathBuf>,
        #[command(subcommand)]
        command: TagCommand,
    },
    /// Start a persistent interactive session
    Interactive(InteractiveArgs),
    /// Configure session warning output
    #[command(hide = true)]
    Warnings {
        #[command(subcommand)]
        command: Option<WarningsCommand>,
    },
    /// Print a shell completion script to stdout
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Args)]
pub struct GenerateArgs {
    /// Exact length in Unicode scalar values, including separators
    #[arg(long, value_parser = positive_usize, required_unless_present = "min_length", conflicts_with_all = ["min_length", "max_length"])]
    pub length: Option<usize>,
    /// Inclusive lower bound of a random target length
    #[arg(long, value_parser = positive_usize, requires = "max_length")]
    pub min_length: Option<usize>,
    /// Inclusive upper bound of a random target length
    #[arg(long, value_parser = positive_usize, requires = "min_length")]
    pub max_length: Option<usize>,
    #[arg(long, value_enum, hide_possible_values = true, help = SEPARATOR_HELP)]
    pub separator_kind: SeparatorKind,
    #[arg(long, default_value = "1", value_parser = positive_usize)]
    pub count: usize,
    /// UTF-8 dictionary, one entry per line; default: all built-in character sets
    #[arg(long, value_hint = ValueHint::FilePath, conflicts_with = "preset")]
    pub dictionary: Option<PathBuf>,
    /// Built-in character sets (repeat or pass multiple names); default: all
    #[arg(long, value_enum, num_args = 1..)]
    pub preset: Vec<PresetKind>,
    /// Choose the first feasible layout or a random feasible layout
    #[arg(long, value_enum, default_value = "first")]
    pub shape_selection: ShapeKind,
    /// Separator text; default: "-"; ignored by none
    #[arg(long, allow_hyphen_values = true, value_parser = nonempty_separator)]
    pub separator: Option<String>,
    /// Unicode scalar interval for fixed-interval (default: 5)
    #[arg(long, value_parser = positive_usize)]
    pub separator_interval: Option<usize>,
    /// Number of insertions for fixed-count (default: 3; reduced on short content)
    #[arg(long, value_parser = positive_usize)]
    pub separator_count: Option<usize>,
    /// Show warnings for separator options ignored by the selected strategy
    #[arg(long, conflicts_with = "no_warnings")]
    pub warnings: bool,
    /// Do not show warnings for separator options ignored by the selected strategy
    #[arg(long, conflicts_with = "warnings")]
    pub no_warnings: bool,
}

#[derive(Args, Default)]
pub struct InteractiveArgs {
    /// Data directory (overrides REPASS_DATA_DIR; default: $HOME/.repass)
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub data_dir: Option<PathBuf>,
    /// Enable warnings in this interactive session
    #[arg(long, conflicts_with = "no_warnings")]
    pub warnings: bool,
    /// Disable warnings in this interactive session
    #[arg(long, conflicts_with = "warnings")]
    pub no_warnings: bool,
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

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ShapeKind {
    First,
    Random,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PresetKind {
    Digits,
    Lowercase,
    Uppercase,
    Punctuation,
    Symbols,
    Brackets,
    Quotes,
    HashDollarPercentCaret,
    BackslashPipeTilde,
}

#[derive(Subcommand)]
pub enum WarningsCommand {
    /// Enable warnings for this session
    On,
    /// Disable warnings for this session
    Off,
}

fn nonempty_separator(value: &str) -> std::result::Result<String, String> {
    if value.is_empty() {
        Err("separator must not be empty; use --separator-kind none to disable separation".into())
    } else {
        Ok(value.into())
    }
}

impl GenerateArgs {
    fn warnings_enabled(&self, default: bool) -> bool {
        if self.warnings {
            true
        } else if self.no_warnings {
            false
        } else {
            default
        }
    }
}

impl InteractiveArgs {
    pub fn warnings_enabled(&self) -> bool {
        !self.no_warnings
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
    /// Restore authenticated backups, preserving damaged originals
    Recover,
    /// Complete an interrupted initialization with authenticated empty metadata
    FinishInit,
    /// Rewrap the data key with a new master password
    ChangePassword,
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
        #[command(flatten)]
        data: DataInput,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        host: Option<Host>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, num_args = 1..)]
        tag: Vec<TagId>,
    },
    List {
        #[arg(long)]
        host: Option<Host>,
        #[arg(long, num_args = 1..)]
        tag: Vec<TagId>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Find records by fuzzy text and/or exact name, host and tag filters
    Find {
        /// Fuzzy search across name, username, host, notes and tag names
        #[arg(long, value_parser = nonblank_query)]
        query: Option<String>,
        #[arg(long)]
        host: Option<Host>,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, num_args = 1..)]
        tag: Vec<TagId>,
    },
    Show {
        record_id: String,
        /// Include secret values in the output
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
        host: Option<Host>,
        #[arg(long)]
        notes: Option<String>,
        #[arg(long, conflicts_with = "username")]
        clear_username: bool,
        #[arg(long, conflicts_with = "host")]
        clear_host: bool,
        #[arg(long, conflicts_with = "notes")]
        clear_notes: bool,
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
    /// Append a typed value to a record
    DataAdd {
        record_id: String,
        #[command(flatten)]
        data: DataInput,
    },
    /// Replace a typed value while retaining its stable ID
    DataUpdate {
        record_id: String,
        data_id: DataId,
        #[command(flatten)]
        data: DataInput,
    },
    /// Delete one typed value; its ID will not be reused
    DataDelete {
        record_id: String,
        data_id: DataId,
    },
}

fn nonblank_query(value: &str) -> std::result::Result<String, String> {
    let query = value.trim();
    if query.is_empty() {
        return Err("search query cannot be blank".into());
    }
    Ok(query.to_owned())
}

#[derive(Clone, Copy, Default, ValueEnum)]
pub enum TotpAlgorithmArg {
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Args, Default)]
pub struct DataInput {
    /// Read one password line (hidden terminal input in interactive mode)
    #[arg(long, conflicts_with_all = ["code_stdin", "totp_stdin", "private_key_file", "private_key_stdin", "public_key_file", "public_key_stdin"])]
    pub password_stdin: bool,
    /// Read one recovery/code line (hidden terminal input in interactive mode)
    #[arg(long, conflicts_with_all = ["totp_stdin", "private_key_file", "private_key_stdin", "public_key_file", "public_key_stdin"])]
    pub code_stdin: bool,
    /// Read a Base32 TOTP secret; no one-time codes are calculated
    #[arg(long, conflicts_with_all = ["private_key_file", "private_key_stdin", "public_key_file", "public_key_stdin"])]
    pub totp_stdin: bool,
    #[arg(long, value_enum, requires = "totp_stdin")]
    pub algorithm: Option<TotpAlgorithmArg>,
    #[arg(long, requires = "totp_stdin")]
    pub digits: Option<u8>,
    /// TOTP period in seconds (default: 30)
    #[arg(long, requires = "totp_stdin")]
    pub period: Option<u32>,
    #[arg(long, value_hint = ValueHint::FilePath, conflicts_with = "private_key_stdin")]
    pub private_key_file: Option<PathBuf>,
    /// Read until EOF, or a line containing only '.' in interactive mode
    #[arg(long)]
    pub private_key_stdin: bool,
    #[arg(long, value_hint = ValueHint::FilePath, conflicts_with = "public_key_stdin")]
    pub public_key_file: Option<PathBuf>,
    /// Read until EOF, or a line containing only '.' in interactive mode
    #[arg(long)]
    pub public_key_stdin: bool,
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

pub fn execute(
    command: Command,
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    match command {
        Command::Generate(args) => {
            let (passwords, warnings) = generate(args, session.warnings_enabled())?;
            for warning in warnings {
                output::warning(output, warning)?;
            }
            for password in passwords {
                writeln!(output, "{password}")?;
            }
            Ok(())
        }
        Command::Completions { shell } => crate::completions::generate(shell, output),
        Command::Vault {
            command: VaultCommand::Switch { dir },
            ..
        } if interactive => {
            session.switch(dir)?;
            output::styled(output, output::SUCCESS, "Data directory:")?;
            writeln!(output, " {}", session.data_dir().display())?;
            Ok(())
        }
        Command::Vault {
            command: VaultCommand::Switch { .. },
            ..
        } => Err("vault switch is available only in interactive mode".into()),
        Command::Interactive(_) => Err("already in interactive mode".into()),
        Command::Warnings { command } => {
            if !interactive {
                return Err("warnings is available only in interactive mode".into());
            }
            match command {
                Some(WarningsCommand::On) => session.set_warnings_enabled(true),
                Some(WarningsCommand::Off) => session.set_warnings_enabled(false),
                None => {}
            }
            writeln!(
                output,
                "Warnings: {}",
                if session.warnings_enabled() {
                    "on"
                } else {
                    "off"
                }
            )?;
            Ok(())
        }
        Command::Vault {
            command: VaultCommand::Init,
            ..
        } => {
            session.initialize_storage(output, interactive)?;
            success(
                output,
                format_args!("Vault initialized in {}", session.data_dir().display()),
            )
        }
        Command::Vault {
            command: VaultCommand::Info,
            ..
        } => {
            let storage = session.ensure_storage(output, interactive)?;
            let info = storage.info();
            writeln!(output, "Data directory: {}", info.directory.display())?;
            writeln!(output, "Records: {}", info.record_count)?;
            writeln!(output, "Tags: {}", info.tag_count)?;
            if let Some(warning) = info.metadata_warning {
                output::warning(
                    output,
                    format!("metadata counts could not be repaired: {warning}"),
                )?;
            }
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
        Command::Vault {
            command: VaultCommand::Recover,
            ..
        } => {
            session.recover_storage(output, interactive, false)?;
            success(
                output,
                "Vault restored from available authenticated backups",
            )
        }
        Command::Vault {
            command: VaultCommand::FinishInit,
            ..
        } => {
            session.recover_storage(output, interactive, true)?;
            success(output, "Vault initialization completed")
        }
        Command::Vault {
            command: VaultCommand::ChangePassword,
            ..
        } => {
            session.change_master_password(output, interactive)?;
            success(output, "Master password changed")
        }
        Command::Record { command, .. } => {
            records::execute_record(command, session, input, output, interactive)
        }
        Command::Tag { command, .. } => records::execute_tag(command, session, output, interactive),
    }
}

fn parse_record_id(value: &str) -> Result<RecordId> {
    Ok(RecordId::from_str(value)?)
}

fn read_record_password(
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<String> {
    if interactive {
        output::styled(output, output::PROMPT, "Password: ")?;
        output.flush()?;
        return Ok(session.read_secret()?);
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
    use repass_storage::{Data, NewRecord, Storage};
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
                data_dir: None,
                command: RecordCommand::Add {
                    name: "mail".into(),
                    data: DataInput {
                        password_stdin: true,
                        ..DataInput::default()
                    },
                    username: Some("alice".into()),
                    host: None,
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
                data_dir: None,
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
                data_dir: None,
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
                data: vec![Data::Password("secret".into())],
                username: None,
                host: None,
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
                data_dir: None,
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
        session.close();
        assert_eq!(
            Storage::open_in(&directory.0, b"master")
                .unwrap()
                .list_tags()[0]
                .name(),
            "new"
        );
        session = Session::with_storage(
            directory.0.clone(),
            Storage::open_in(&directory.0, b"master").unwrap(),
        );
        let mut output = Vec::new();
        execute(
            Command::Tag {
                data_dir: None,
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

    #[test]
    fn update_command_can_clear_optional_fields() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        let mut session = Session::with_storage(directory.0.clone(), storage);
        let id = session
            .ensure_storage(&mut Vec::new(), false)
            .unwrap()
            .create_record(NewRecord {
                name: "mail".into(),
                data: vec![Data::Password("secret".into())],
                username: Some("alice".into()),
                host: Some("example.test".parse().unwrap()),
                notes: Some("note".into()),
                tags: Vec::new(),
            })
            .unwrap();

        execute(
            Command::Record {
                data_dir: None,
                command: RecordCommand::Update {
                    record_id: id.to_string(),
                    name: None,
                    username: None,
                    host: None,
                    notes: None,
                    clear_username: true,
                    clear_host: true,
                    clear_notes: true,
                    password_stdin: false,
                    add_tag: Vec::new(),
                    remove_tag: Vec::new(),
                },
            },
            &mut session,
            &mut Cursor::new(Vec::new()),
            &mut Vec::new(),
            false,
        )
        .unwrap();

        let record = session
            .ensure_storage(&mut Vec::new(), false)
            .unwrap()
            .get_record(id)
            .unwrap();
        assert_eq!(record.username, None);
        assert_eq!(record.host, None);
        assert_eq!(record.notes, None);
    }

    #[test]
    fn interactive_master_and_record_passwords_use_injected_secret_input() {
        let directory = TestDirectory::new();
        Storage::create_in(&directory.0, b"master").unwrap();
        let mut session = Session::with_passwords(
            directory.0.clone(),
            ["master".to_owned(), "record-secret".to_owned()],
        );
        let mut output = Vec::new();
        execute(
            Command::Record {
                data_dir: None,
                command: RecordCommand::Add {
                    name: "mail".into(),
                    data: DataInput {
                        password_stdin: true,
                        ..DataInput::default()
                    },
                    username: None,
                    host: None,
                    notes: None,
                    tag: Vec::new(),
                },
            },
            &mut session,
            &mut Cursor::new(Vec::new()),
            &mut output,
            true,
        )
        .unwrap();

        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Master password:"));
        assert!(text.contains("Password:"));
        assert!(!text.contains("record-secret"));
        let storage = session.ensure_storage(&mut Vec::new(), true).unwrap();
        assert!(
            matches!(&storage.get_record(RecordId::new(1)).unwrap().data()[0].value, Data::Password(value) if value == "record-secret")
        );
    }

    #[test]
    fn master_password_command_confirms_secrets_and_keeps_session_usable() {
        let directory = TestDirectory::new();
        Storage::create_in(&directory.0, b"old").unwrap();
        let mut session = Session::with_passwords(
            directory.0.clone(),
            ["old".into(), "new-secret".into(), "new-secret".into()],
        );
        let mut output = Vec::new();
        execute(
            Command::Vault {
                data_dir: None,
                command: VaultCommand::ChangePassword,
            },
            &mut session,
            &mut Cursor::new(""),
            &mut output,
            true,
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Master password changed"));
        assert!(!text.contains("new-secret"));
        assert_eq!(
            session
                .ensure_storage(&mut Vec::new(), true)
                .unwrap()
                .info()
                .record_count,
            0
        );
        session.close();
        assert!(Storage::open_in(&directory.0, b"old").is_err());
        assert!(Storage::open_in(&directory.0, b"new-secret").is_ok());
    }

    #[test]
    fn mismatched_new_master_passwords_leave_the_original_password_valid() {
        let directory = TestDirectory::new();
        Storage::create_in(&directory.0, b"old").unwrap();
        let mut session = Session::with_passwords(
            directory.0.clone(),
            ["old".into(), "new".into(), "different".into()],
        );
        assert!(
            execute(
                Command::Vault {
                    data_dir: None,
                    command: VaultCommand::ChangePassword
                },
                &mut session,
                &mut Cursor::new(""),
                &mut Vec::new(),
                true
            )
            .is_err()
        );
        session.close();
        assert!(Storage::open_in(&directory.0, b"old").is_ok());
    }

    #[test]
    fn find_command_filters_records_without_revealing_passwords() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let a = storage.create_tag("a").unwrap();
        let b = storage.create_tag("b").unwrap();
        let make = |name: &str, tags| NewRecord {
            name: name.into(),
            data: vec![Data::Password("hidden-secret".into())],
            username: None,
            host: None,
            notes: None,
            tags,
        };
        storage.create_record(make("wanted", vec![a, b])).unwrap();
        storage.create_record(make("other", vec![a, b])).unwrap();
        let mut session = Session::with_storage(directory.0.clone(), storage);
        let mut output = Vec::new();
        execute(
            Command::Record {
                data_dir: None,
                command: RecordCommand::Find {
                    query: None,
                    host: None,
                    name: Some("wanted".into()),
                    tag: vec![a, b],
                },
            },
            &mut session,
            &mut Cursor::new(""),
            &mut output,
            false,
        )
        .unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("wanted"));
        assert!(!text.contains("other"));
        assert!(!text.contains("hidden-secret"));
    }

    fn run_record_line(
        line: &str,
        input: &str,
        session: &mut Session,
        interactive: bool,
    ) -> Result<String> {
        let words = shlex::split(&format!("repass record {line}")).unwrap();
        let command = Cli::try_parse_from(words)?.command.unwrap();
        let mut output = Vec::new();
        execute(
            command,
            session,
            &mut Cursor::new(input),
            &mut output,
            interactive,
        )?;
        Ok(String::from_utf8(output).unwrap())
    }

    #[test]
    fn fuzzy_find_uses_ranked_metadata_matches_and_combines_filters() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        let make = |name: &str, tags| NewRecord {
            name: name.into(),
            data: vec![Data::Password("secret-only-value".into())],
            username: None,
            host: Some("example.test".parse().unwrap()),
            notes: None,
            tags,
        };
        storage.create_record(make("a_l_p_h_a", vec![tag])).unwrap();
        storage.create_record(make("alpha", vec![tag])).unwrap();
        storage.create_record(make("alpha", vec![])).unwrap();
        let mut session = Session::with_storage(directory.0.clone(), storage);
        let result = run_record_line("find --query ALPHA", "", &mut session, false).unwrap();
        let ids: Vec<_> = result
            .lines()
            .map(|line| line.split('\t').next().unwrap())
            .collect();
        assert_eq!(ids, vec!["2", "3", "1"]);
        assert!(!result.contains("secret-only-value"));
        let result = run_record_line(
            &format!("find --query alpha --name alpha --host EXAMPLE.TEST. --tag {tag}"),
            "",
            &mut session,
            false,
        )
        .unwrap();
        assert_eq!(result.lines().count(), 1);
        assert!(result.starts_with("2\t"));
        assert!(
            run_record_line("find --query secret-only-value", "", &mut session, false)
                .unwrap()
                .is_empty()
        );
        assert!(run_record_line("find --query '   '", "", &mut session, false).is_err());
        assert_eq!(
            run_record_line("find --query alpha", "", &mut session, true).unwrap(),
            run_record_line("find --query alpha", "", &mut session, false).unwrap()
        );
    }

    #[test]
    fn typed_data_commands_support_files_stdin_mutation_search_and_masking() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        let public_path = directory.0.join("public key.pub");
        fs::write(&public_path, "ssh-ed25519 public-secret comment\n").unwrap();
        let mut session = Session::with_storage(directory.0.clone(), storage);
        let private = "-----BEGIN OPENSSH PRIVATE KEY-----\r\nprivate-secret  \r\n-----END OPENSSH PRIVATE KEY-----\r\n";
        let add = format!(
            "add --name ssh --host Example.TEST. --private-key-stdin --public-key-file '{}'",
            public_path.display()
        );
        run_record_line(&add, private, &mut session, false).unwrap();
        {
            let storage = session.ensure_storage(&mut Vec::new(), false).unwrap();
            let view = storage.get_record(RecordId::new(1)).unwrap();
            assert_eq!(view.data().len(), 1);
            assert!(
                matches!(&view.data()[0].value, Data::SshKey(key) if key.private_key.as_deref() == Some(private) && key.public_key.as_deref() == Some("ssh-ed25519 public-secret comment\n"))
            );
        }
        run_record_line(
            "data-add 1 --totp-stdin --algorithm sha256 --digits 8 --period 60",
            "MZXW6YTB\n",
            &mut session,
            false,
        )
        .unwrap();
        run_record_line(
            "data-add 1 --code-stdin",
            "recovery-secret\n",
            &mut session,
            false,
        )
        .unwrap();
        run_record_line(
            "data-add 1 --password-stdin",
            "password-secret\n",
            &mut session,
            false,
        )
        .unwrap();
        let masked = run_record_line("show 1", "", &mut session, false).unwrap();
        let listed = run_record_line("list --host example.test", "", &mut session, false).unwrap();
        assert!(listed.contains("1:ssh, 2:totp, 3:code, 4:password"));
        assert!(
            run_record_line("find --host other.test", "", &mut session, false)
                .unwrap()
                .is_empty()
        );
        for secret in [
            "private-secret",
            "public-secret",
            "MZXW6YTB",
            "recovery-secret",
            "password-secret",
        ] {
            assert!(!masked.contains(secret));
            assert!(!listed.contains(secret));
            assert!(
                run_record_line("show 1 --reveal", "", &mut session, false)
                    .unwrap()
                    .contains(secret)
            );
        }
        // Replacement can retain just the public part of an SSH value.
        run_record_line(
            "data-update 1 1 --public-key-stdin",
            "new-public\n",
            &mut session,
            false,
        )
        .unwrap();
        run_record_line("data-delete 1 3", "", &mut session, false).unwrap();
        run_record_line("data-add 1 --code-stdin", "new-code\n", &mut session, false).unwrap();
        assert!(
            run_record_line("list", "", &mut session, false)
                .unwrap()
                .contains("1:ssh, 2:totp, 4:password, 5:code")
        );
        assert!(run_record_line("data-add 1", "", &mut session, false).is_err());
        assert!(
            run_record_line("data-update 1 99 --code-stdin", "x\n", &mut session, false).is_err()
        );
        assert!(
            run_record_line(
                "data-add 1 --totp-stdin",
                "invalid-secret\n",
                &mut session,
                false
            )
            .is_err()
        );
        session.close();
        let storage = Storage::open_in(&directory.0, b"master").unwrap();
        let view = storage.get_record(RecordId::new(1)).unwrap();
        assert!(
            matches!(&view.data()[0].value, Data::SshKey(key) if key.private_key.is_none() && key.public_key.as_deref() == Some("new-public\n"))
        );
        assert!(
            matches!(&view.data()[1].value, Data::Totp(totp) if totp.algorithm == repass_storage::TotpAlgorithm::Sha256 && totp.digits == 8 && totp.period == 60)
        );
    }

    #[test]
    fn interactive_ssh_accepts_both_key_parts_and_does_not_consume_next_command() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        let mut session = Session::with_storage(directory.0.clone(), storage);
        let command = Cli::try_parse_from([
            "repass",
            "record",
            "add",
            "--name",
            "ssh",
            "--private-key-stdin",
            "--public-key-stdin",
        ])
        .unwrap()
        .command
        .unwrap();
        let mut input = Cursor::new("private\r\nkey  \r\n.\r\npublic\n.\nnext-command\n");
        let mut output = Vec::new();
        execute(command, &mut session, &mut input, &mut output, true).unwrap();
        let storage = session.ensure_storage(&mut Vec::new(), true).unwrap();
        assert!(
            matches!(&storage.get_record(RecordId::new(1)).unwrap().data()[0].value, Data::SshKey(key) if key.private_key.as_deref() == Some("private\r\nkey  \r\n") && key.public_key.as_deref() == Some("public\n"))
        );
        let mut next = String::new();
        input.read_line(&mut next).unwrap();
        assert_eq!(next, "next-command\n");
        assert!(!String::from_utf8(output).unwrap().contains("key  "));
        assert!(
            run_record_line(
                "data-add 1 --private-key-stdin",
                "unterminated\n",
                &mut session,
                true
            )
            .is_err()
        );
        assert!(
            run_record_line(
                "data-add 1 --private-key-stdin --public-key-stdin",
                "key\n",
                &mut session,
                false
            )
            .is_err()
        );
        assert_eq!(
            session
                .ensure_storage(&mut Vec::new(), true)
                .unwrap()
                .get_record(RecordId::new(1))
                .unwrap()
                .data()
                .len(),
            1
        );
    }

    #[test]
    fn data_source_flags_are_exclusive_and_totp_options_require_a_totp_secret() {
        for args in [
            vec!["--password-stdin", "--private-key-stdin"],
            vec!["--code-stdin", "--totp-stdin"],
            vec!["--totp-stdin", "--public-key-stdin"],
            vec!["--private-key-file", "key", "--private-key-stdin"],
            vec!["--public-key-file", "key", "--public-key-stdin"],
            vec!["--digits", "8"],
            vec!["--algorithm", "sha256"],
            vec!["--period", "60"],
        ] {
            let mut words = vec!["repass", "record", "add", "--name", "test"];
            words.extend(args);
            assert!(Cli::try_parse_from(words).is_err());
        }
    }

    #[test]
    fn password_shortcut_rejects_ambiguity_and_empty_records_are_supported() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        let mut session = Session::with_storage(directory.0.clone(), storage);
        run_record_line("add --name empty", "", &mut session, false).unwrap();
        run_record_line("update 1 --password-stdin", "first\n", &mut session, false).unwrap();
        run_record_line(
            "update 1 --password-stdin",
            "changed\n",
            &mut session,
            false,
        )
        .unwrap();
        run_record_line(
            "data-add 1 --password-stdin",
            "second\n",
            &mut session,
            false,
        )
        .unwrap();
        assert!(
            run_record_line(
                "update 1 --password-stdin",
                "ambiguous\n",
                &mut session,
                false
            )
            .is_err()
        );
        let view = session
            .ensure_storage(&mut Vec::new(), false)
            .unwrap()
            .get_record(RecordId::new(1))
            .unwrap();
        assert!(matches!(&view.data()[0].value, Data::Password(value) if value == "changed"));
        assert!(matches!(&view.data()[1].value, Data::Password(value) if value == "second"));
    }
}
