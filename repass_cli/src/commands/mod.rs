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
    after_long_help = "Examples:\n  repass generate --length 16 --separator-kind none\n  repass vault init\n  repass record list\n  repass interactive\n\nUse '<command> -h' for a summary or '<command> --help' for details and examples.",
    styles = output::styles()
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Generate passwords without opening a vault
    ///
    /// Specify --length or both --min-length and --max-length, plus a separator
    /// strategy. Length includes separators and counts Unicode scalar values.
    /// By default, all built-in character sets are used. Output is one password
    /// per line; an unreachable length is reported as an error.
    #[command(
        after_long_help = "Examples:\n  repass generate --length 16 --separator-kind none\n  repass generate --min-length 12 --max-length 20 --separator-kind 2\n  repass generate --length 16 --separator-kind none --preset digits lowercase --count 3",
        after_help = "Required: --length or both --min-length and --max-length, plus --separator-kind."
    )]
    Generate(GenerateArgs),
    /// Create, inspect, recover or change the password of a vault
    Vault {
        /// Vault directory (default: REPASS_DATA_DIR, then $HOME/.repass)
        #[arg(long, global = true, value_hint = ValueHint::DirPath)]
        data_dir: Option<PathBuf>,
        #[command(subcommand)]
        command: VaultCommand,
    },
    /// Manage records and their passwords, SSH keys, TOTP data and codes
    ///
    /// Use list or find to obtain record IDs. Use show to inspect the data IDs
    /// within a record, then data-add, data-update or data-delete to edit its
    /// contents. Storage commands request a master password and create a vault
    /// on first use if none exists.
    Record {
        /// Vault directory (default: REPASS_DATA_DIR, then $HOME/.repass)
        #[arg(long, global = true, value_hint = ValueHint::DirPath)]
        data_dir: Option<PathBuf>,
        #[command(subcommand)]
        command: RecordCommand,
    },
    /// Create, list, rename and remove tags
    Tag {
        /// Vault directory (default: REPASS_DATA_DIR, then $HOME/.repass)
        #[arg(long, global = true, value_hint = ValueHint::DirPath)]
        data_dir: Option<PathBuf>,
        #[command(subcommand)]
        command: TagCommand,
    },
    /// Start a persistent interactive session
    ///
    /// Enter commands without the 'repass' prefix. Use help or h for help and
    /// quit or q to leave. Missing required arguments are prompted; optional
    /// arguments are not. Passwords, codes and TOTP secrets use hidden input.
    /// Use 'vault switch <DIR>' to select another vault; it opens on first use.
    /// Session commands use that directory rather than --data-dir.
    Interactive(InteractiveArgs),
    /// Show or change warnings about unused generation options
    ///
    /// With no subcommand, show the current setting. Warnings are enabled by
    /// default. on/off changes the session default; generate --warnings or
    /// --no-warnings overrides it for one invocation.
    #[command(hide = true)]
    Warnings {
        #[command(subcommand)]
        command: Option<WarningsCommand>,
    },
    /// Print a shell completion script to stdout
    ///
    /// Generates completion for the external shell, not Tab completion inside
    /// a repass session. No vault is opened. Load the script in your shell or
    /// save it in its completion directory.
    #[command(after_long_help = "Example (Bash):\n  source <(repass completions bash)")]
    Completions {
        /// Shell for which to generate the script
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
    #[arg(long, value_enum, hide_possible_values = true, help = SEPARATOR_SHORT_HELP, long_help = SEPARATOR_HELP)]
    pub separator_kind: SeparatorKind,
    /// Number of passwords to generate
    #[arg(long, default_value = "1", value_parser = positive_usize)]
    pub count: usize,
    /// UTF-8 dictionary file, one entry per line
    ///
    /// Replaces the built-in character sets and cannot be combined with --preset.
    /// Line endings are removed; entries may contain multiple Unicode characters.
    #[arg(long, value_hint = ValueHint::FilePath, conflicts_with = "preset")]
    pub dictionary: Option<PathBuf>,
    /// Combine built-in character sets (default: all)
    ///
    /// Repeat this option or pass several names to form their union. Selecting
    /// sets does not require every set to appear in each generated password.
    /// Cannot be combined with --dictionary.
    ///
    /// Sets: digits, lowercase, uppercase, punctuation, symbols, brackets,
    /// quotes, hash-dollar-percent-caret, backslash-pipe-tilde.
    #[arg(long, value_enum, num_args = 1.., hide_possible_values = true)]
    pub preset: Vec<PresetKind>,
    /// Choose the first or a random valid arrangement of dictionary entries
    ///
    /// first uses the first feasible arrangement of entry lengths and separators.
    /// random samples among those arrangements, not uniformly among all passwords.
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
    /// Enable warnings about unused separator options
    #[arg(long, conflicts_with = "no_warnings")]
    pub warnings: bool,
    /// Disable warnings about unused separator options
    #[arg(long, conflicts_with = "warnings")]
    pub no_warnings: bool,
}

#[derive(Args, Default)]
pub struct InteractiveArgs {
    /// Vault directory (default: REPASS_DATA_DIR, then $HOME/.repass)
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub data_dir: Option<PathBuf>,
    /// Enable generation warnings for this session (default)
    #[arg(long, conflicts_with = "no_warnings")]
    pub warnings: bool,
    /// Disable generation warnings for this session
    #[arg(long, conflicts_with = "warnings")]
    pub no_warnings: bool,
}

pub const SEPARATOR_SHORT_HELP: &str = "Separator strategy (name or number):
1. none
2. between-parts
3. fixed-interval
4. fixed-count";

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
    /// Create a new vault without overwriting existing vault files
    ///
    /// Requests a nonempty master password and hidden confirmation. A mismatch
    /// cancels creation. Existing vault files or backups prevent initialization.
    Init,
    /// Show the directory, record/tag counts and tag-catalog status
    ///
    /// Requests the master password to open the vault. If no vault exists,
    /// creates one after password confirmation. Also reports metadata count
    /// repair failures.
    Info,
    /// Restore vault files from validated .bak snapshots
    ///
    /// Requests the master password and restores missing or damaged files from
    /// authenticated backups. Damaged originals are kept as .damaged-* copies.
    /// Backups contain the previous snapshot: the latest mutation may be lost.
    /// Without a usable backup, records cannot be recovered.
    Recover,
    /// Complete an interrupted empty-vault initialization
    ///
    /// Creates empty records only when authenticated metadata has valid zero
    /// counts and no record/tag files or their backups exist. This does not
    /// restore lost records; use recover when backups are available.
    FinishInit,
    /// Change the master password
    ///
    /// Opens the vault with the current password, then requests and confirms a
    /// nonempty new password. Updates key metadata and its backup without
    /// rewriting record or tag data.
    ChangePassword,
    /// Close the current vault and select another directory (interactive only)
    ///
    /// The selected vault opens when a storage command first needs it.
    #[command(hide = true)]
    Switch {
        /// Directory to use for subsequent storage commands
        #[arg(value_hint = ValueHint::DirPath)]
        dir: PathBuf,
    },
}

#[derive(Subcommand)]
pub enum RecordCommand {
    /// Create a record, optionally with one initial data element
    ///
    /// A password is optional; a record may start with no data. Select at most
    /// one data type using its input options. SSH may include a private key, a
    /// public key, or both; every supplied key field must be nonblank.
    #[command(
        after_long_help = "Examples:\n  repass record add --name mail --password-stdin\n  repass record add --name server --host example.test --private-key-file id_ed25519 --public-key-file id_ed25519.pub\n  repass record add --name notes --notes 'Account details'"
    )]
    Add {
        /// Nonempty display name for the record
        #[arg(long, help_heading = "Record fields")]
        name: String,
        #[command(flatten)]
        data: DataInput,
        /// Login or account username
        #[arg(long, help_heading = "Record fields")]
        username: Option<String>,
        /// IP address or domain, without protocol, port or path
        #[arg(long, help_heading = "Record fields")]
        host: Option<Host>,
        /// Free-form notes
        #[arg(long, help_heading = "Record fields")]
        notes: Option<String>,
        /// Tag IDs to attach (see tag list)
        #[arg(long, num_args = 1.., help_heading = "Record fields")]
        tag: Vec<TagId>,
    },
    /// List record summaries, ordered by ID, without secret values
    List {
        /// Match an IP address or domain (domains ignore case and a trailing dot)
        #[arg(long)]
        host: Option<Host>,
        /// Require every specified tag ID (see tag list)
        #[arg(long, num_args = 1..)]
        tag: Vec<TagId>,
        /// Match the full record name, case-sensitively
        #[arg(long)]
        name: Option<String>,
    },
    /// Find records by fuzzy text and/or exact name, host and tag filters
    ///
    /// With --query, match fields case-insensitively and order by descending
    /// relevance, then record ID. Exact filters apply before fuzzy matching.
    /// Without --query, use exact filters and order by ID. Secret values are
    /// excluded from matching and output.
    #[command(
        after_long_help = "Examples:\n  repass record find --query githab\n  repass record find --query alice --host example.test --tag 1 2"
    )]
    Find {
        /// Fuzzy text across name, username, host, notes and tag names
        ///
        /// Supports Unicode, abbreviations and limited typos. Surrounding
        /// whitespace is trimmed; blank queries are rejected. A query is one
        /// fuzzy pattern, with no special operator syntax.
        #[arg(long, value_parser = nonblank_query)]
        query: Option<String>,
        /// Match an IP address or domain (domains ignore case and a trailing dot)
        #[arg(long)]
        host: Option<Host>,
        /// Match the full record name, case-sensitively
        #[arg(long)]
        name: Option<String>,
        /// Require every specified tag ID (see tag list)
        #[arg(long, num_args = 1..)]
        tag: Vec<TagId>,
    },
    /// Show record details and data IDs, masking secret values by default
    Show {
        /// Record ID from record list or record find
        record_id: String,
        /// Include secret values in the output
        #[arg(long)]
        reveal: bool,
    },
    /// Change only the specified record fields or tag associations
    ///
    /// Omitted fields are retained. Use --clear-* to remove optional fields.
    /// Use data-add/data-update/data-delete to edit individual data elements.
    Update {
        /// Record ID from record list or record find
        record_id: String,
        /// Replace the display name with a nonempty name
        #[arg(long)]
        name: Option<String>,
        /// Set the login or account username
        #[arg(long)]
        username: Option<String>,
        /// Set an IP address or domain, without protocol, port or path
        #[arg(long)]
        host: Option<Host>,
        /// Set free-form notes
        #[arg(long)]
        notes: Option<String>,
        /// Remove the username
        #[arg(long, conflicts_with = "username")]
        clear_username: bool,
        /// Remove the host
        #[arg(long, conflicts_with = "host")]
        clear_host: bool,
        /// Remove the notes
        #[arg(long, conflicts_with = "notes")]
        clear_notes: bool,
        /// Add or replace the record's only password from a stdin line
        ///
        /// Adds a password if none exists, or replaces the only password.
        /// With multiple passwords, use data-update with a data ID instead.
        /// One-shot mode reads one stdin line; sessions use hidden input.
        #[arg(long)]
        password_stdin: bool,
        /// Attach existing tag IDs (see tag list)
        #[arg(long, num_args = 1..)]
        add_tag: Vec<TagId>,
        /// Detach tag IDs without deleting the tags
        #[arg(long, num_args = 1..)]
        remove_tag: Vec<TagId>,
    },
    /// Delete a record and all of its data elements
    Delete {
        /// Record ID from record list or record find
        record_id: String,
    },
    /// Add one password, SSH, TOTP or code element to a record
    ///
    /// Select one data type using its input options; an input source is required.
    /// SSH may include a private key, a public key, or both. Each element gets
    /// an ID within its record; multiple elements of the same type are allowed.
    #[command(
        after_long_help = "Examples:\n  repass record data-add 1 --totp-stdin --algorithm sha256 --digits 8\n  repass record data-add 1 --code-stdin",
        after_help = "Select one data type using its input options; an input source is required."
    )]
    DataAdd {
        /// Record ID from record list or record find
        record_id: String,
        #[command(flatten)]
        data: DataInput,
    },
    /// Replace an entire data element, retaining its ID
    ///
    /// Select one data type using its input options; an input source is required.
    /// This is full replacement, not a partial field update. To retain both
    /// parts of an SSH pair, supply both again. Supplying only the public key
    /// removes the private part. The data type may also be changed.
    #[command(
        after_long_help = "Example:\n  repass record data-update 1 2 --private-key-file id_ed25519 --public-key-file id_ed25519.pub",
        after_help = "Select one data type using its input options; an input source is required."
    )]
    DataUpdate {
        /// Record ID from record list or record find
        record_id: String,
        /// Data element ID within this record (see record show)
        data_id: DataId,
        #[command(flatten)]
        data: DataInput,
    },
    /// Delete one data element; its ID will not be reused
    DataDelete {
        /// Record ID from record list or record find
        record_id: String,
        /// Data element ID within this record (see record show)
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
    /// Read one password line from stdin
    ///
    /// Removes the final LF or CRLF. The master password is requested separately
    /// through hidden terminal input.
    #[arg(long, help_heading = "Password", conflicts_with_all = ["code_stdin", "totp_stdin", "private_key_file", "private_key_stdin", "public_key_file", "public_key_stdin"])]
    pub password_stdin: bool,
    /// Read one recovery code or other code line from stdin
    ///
    /// Removes the final LF or CRLF. The code must not be blank.
    #[arg(long, help_heading = "Code", conflicts_with_all = ["totp_stdin", "private_key_file", "private_key_stdin", "public_key_file", "public_key_stdin"])]
    pub code_stdin: bool,
    /// Read an uppercase Base32 TOTP secret from a stdin line
    ///
    /// Accepts unpadded Base32 or canonical padding. Stores configuration only;
    /// no one-time codes are calculated. Defaults: SHA-1, 6 digits, 30 seconds.
    #[arg(long, help_heading = "TOTP", conflicts_with_all = ["private_key_file", "private_key_stdin", "public_key_file", "public_key_stdin"])]
    pub totp_stdin: bool,
    /// TOTP algorithm (default: sha1; requires --totp-stdin)
    #[arg(long, help_heading = "TOTP", value_enum, requires = "totp_stdin")]
    pub algorithm: Option<TotpAlgorithmArg>,
    /// TOTP code length: 6–8 digits (default: 6; requires --totp-stdin)
    #[arg(long, help_heading = "TOTP", requires = "totp_stdin")]
    pub digits: Option<u8>,
    /// Positive TOTP period in seconds (default: 30; requires --totp-stdin)
    #[arg(long, help_heading = "TOTP", requires = "totp_stdin")]
    pub period: Option<u32>,
    /// Read a private SSH key from a UTF-8 file
    ///
    /// Preserves whitespace and line endings. The supplied field must not be
    /// blank. May be combined with a public key source to store both parts.
    #[arg(long, help_heading = "SSH", value_hint = ValueHint::FilePath, conflicts_with = "private_key_stdin")]
    pub private_key_file: Option<PathBuf>,
    /// Read a private SSH key from stdin until EOF
    ///
    /// Preserves whitespace and line endings. Only one SSH field can use stdin
    /// until EOF; use a file for the other part. The key must not be blank.
    #[arg(long, help_heading = "SSH")]
    pub private_key_stdin: bool,
    /// Read a public SSH key from a UTF-8 file
    ///
    /// Preserves whitespace and line endings. The supplied field must not be
    /// blank. May be combined with a private key source to store both parts.
    #[arg(long, help_heading = "SSH", value_hint = ValueHint::FilePath, conflicts_with = "public_key_stdin")]
    pub public_key_file: Option<PathBuf>,
    /// Read a public SSH key from stdin until EOF
    ///
    /// Preserves whitespace and line endings. Only one SSH field can use stdin
    /// until EOF; use a file for the other part. The key must not be blank.
    #[arg(long, help_heading = "SSH")]
    pub public_key_stdin: bool,
}

#[derive(Subcommand)]
pub enum TagCommand {
    /// Create a tag with a unique, nonempty name
    Add {
        /// Unique, nonempty tag name
        #[arg(long)]
        name: String,
    },
    /// List tag IDs and names, including technical names for missing entries
    List,
    /// Delete a tag that is not used by any record
    Delete {
        /// Tag ID from tag list
        tag_id: TagId,
    },
    /// Rename a tag without changing its ID
    Rename {
        /// Tag ID from tag list
        tag_id: TagId,
        /// New unique, nonempty name
        #[arg(long)]
        name: String,
    },
    /// Rebuild a missing or damaged tag-name catalog using technical names
    ///
    /// Saves the currently known tags, including generated names such as
    /// #tag-42. Lost original names are not reconstructed. For restoration
    /// from usable backups, use vault recover instead.
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
