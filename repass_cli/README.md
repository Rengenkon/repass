# repass CLI

Run with `cargo run -p repass_cli -- <arguments>` or build the `repass` binary.

## Manual page

The manual includes command and option descriptions, input modes, recovery
behavior, and examples. Open it directly from the repository root:

```sh
man -l repass_cli/man/repass.1
```

To install it for the current user on Linux:

```sh
install -Dm644 repass_cli/man/repass.1 "$HOME/.local/share/man/man1/repass.1"
man repass
```

The installation directory must be included in your system's man search path.

## Command overview

```text
repass generate (--length <N> | --min-length <N> --max-length <N>) --separator-kind <KIND> [--count <N>] [--dictionary <FILE> | --preset <SET>...] [--shape-selection first|random] [--separator <TEXT>] [--separator-interval <N>] [--separator-count <N>] [--warnings | --no-warnings]
repass vault [--data-dir <DIR>] init
repass vault [--data-dir <DIR>] info
repass vault [--data-dir <DIR>] change-password
repass vault [--data-dir <DIR>] recover
repass vault [--data-dir <DIR>] finish-init
repass record [--data-dir <DIR>] create <NAME> [<DATA_SOURCE>] [--username <TEXT>] [--host <HOST>] [--notes <TEXT>] [--tag <TAG_ID>...]
repass record [--data-dir <DIR>] list [--name <NAME>] [--host <HOST>] [--tag <TAG_ID>...]
repass record [--data-dir <DIR>] find [--query <TEXT>] [--name <NAME>] [--host <HOST>] [--tag <TAG_ID>...]
repass record [--data-dir <DIR>] show <RECORD_ID> [--reveal] [--data-id <ID>] [--raw [--ssh-part private|public]]
repass record [--data-dir <DIR>] update <RECORD_ID> [--name <NAME>] [--username <TEXT> | --remove-username] [--host <HOST> | --remove-host] [--notes <TEXT> | --remove-notes] [<DATA_SOURCE>] [--replace-data <DATA_ID>] [--remove-data <DATA_ID>...] [--add-tag <TAG_ID>...] [--remove-tag <TAG_ID>...]
repass record [--data-dir <DIR>] remove <RECORD_ID>
repass tag [--data-dir <DIR>] create <NAME>
repass tag [--data-dir <DIR>] list
repass tag [--data-dir <DIR>] remove <TAG_ID>
repass tag [--data-dir <DIR>] rename <TAG_ID> --name <NAME>
repass tag [--data-dir <DIR>] recover
repass interactive [--data-dir <DIR>] [--warnings | --no-warnings]
repass completions <SHELL>
```

`--data-dir` is available on vault, record, tag, and interactive commands. It
can appear anywhere within that command group, such as `repass record list
--data-dir <DIR>`. Directory precedence: explicit argument, `REPASS_DATA_DIR`,
then `$HOME/.repass`. A leading `~` path component expands to `$HOME`. An empty
selected directory is an error. Generation and shell completions do not use a
data directory.

`repass` without a command displays help. Use `repass interactive` to start a
session. Enter the same commands
without the `repass` prefix; quotes preserve spaces. Missing mandatory arguments
are prompted. The master password, passwords, codes and TOTP secrets are requested with hidden
terminal input. In a one-shot invocation, `--password-stdin` reads one record
password line from stdin (removing its line ending).
Optional arguments are not prompted. Use `h` or `help` for help (including
`h generate` and `help record create`), and `q`, `quit`, or EOF to leave.
`exit` is not a supported command.
The `interactive` command is unavailable and absent from help inside a session.
Use `-h` for a compact summary and `--help` for full descriptions and examples.
Session help shows the required choice between positional and named forms;
omitting both prompts for a value. Input instructions and examples use session
syntax; SSH input ends with a line containing only `.`.

Inside a session, ordinary commands cannot accept `--data-dir`. Use
`vault switch <DIR>` to close the old vault and change the session's directory.
Opening and decryption are lazy; generation and help do not open a vault.

## Record contents

A record can contain any number of passwords, SSH keys, TOTP configurations and
codes, including several values of the same type. A password is not mandatory;
records can also start with no data. Each element has a stable ID within its
record. Deleted data IDs are not reused, including after reopening the vault.
`list` and `find` show `ID:type` summaries. `show` masks all values unless
`--reveal` is supplied; TOTP algorithm, digit count and period remain visible.
Creation and update times are displayed as `YYYY-MM-DD HH:MM` (UTC).
Displayed metadata and revealed passwords/codes escape control characters and
backslashes. Tag names in comma-separated lists also escape commas. Notes and
revealed SSH keys use indented multiline blocks. These are display conventions;
stored text is not changed, and `show --reveal` is not a raw export format.

Use `--data-id <ID>` to display only one element, still masked unless `--reveal`
is supplied. Add `--raw` to output that element's secret verbatim, without
metadata, labels, escaping or an added newline. `--raw` requires `--data-id`
and reveals the value even without `--reveal`. For TOTP it outputs the stored
Base32 secret, not a generated code. For SSH it also requires
`--ssh-part private|public`; selecting a missing part is an error. `--ssh-part`
is only accepted with `--raw` and an SSH element.

```sh
repass record show 1 --data-id 2 --reveal
repass record show 1 --data-id 2 --raw
repass record show 1 --data-id 3 --raw --ssh-part private > id_ed25519
repass record show --record-id 1 --data-id 3 --raw --ssh-part public
```

`<DATA_SOURCE>` selects one type per operation:

```text
--password-stdin
--code-stdin
--totp-stdin [--algorithm sha1|sha256|sha512] [--digits 6|7|8] [--period <SECONDS>]
[--private-key-file <FILE> | --private-key-stdin] [--public-key-file <FILE> | --public-key-stdin]
```

One-shot passwords, codes and TOTP secrets consume one stdin line, removing its
line ending. TOTP secrets must be uppercase RFC 4648 Base32, either unpadded or
with canonical padding. Defaults are SHA-1, 6 digits and a 30-second period;
periods must be positive. This release stores configuration only and does not
calculate one-time codes.

SSH values contain a private key, a public key, or both. At least one field is
required and supplied fields cannot be blank. Files must be UTF-8. Key text,
including whitespace and line endings, is preserved; cryptographic key structure
is not parsed. In one-shot mode, a key read from stdin consumes input until EOF;
only one key field can use stdin, and the other can come from a file. In an
interactive session, each requested key field uses ordinary multiline terminal
input, terminated by a line containing only `.`. The terminator is not stored,
and EOF before it cancels the operation.

`record update` modifies fields, tags and data together. A data source without
`--replace-data` always adds a new element, even if passwords already exist.
`--replace-data <DATA_ID>` requires a data source and replaces the entire element
while retaining its ID. To replace an SSH pair and keep both parts, provide both
parts again; to retain only the public part, provide only that part.
`--remove-data <DATA_ID>...` removes elements; the same ID cannot be replaced and
removed in one operation. Omitted fields are retained; `--remove-username`,
`--remove-host` and `--remove-notes` clear optional fields. All changes are
validated before a single record update is saved.

Create a tag, obtain its ID from `tag list`, and attach it to a record:

```text
repass tag create work
repass tag list
repass record update 1 --add-tag 2
repass record update 1 --remove-tag 2
```

IDs in examples are illustrative; use the IDs returned by your own vault.
Tags can also be attached at creation with `record create mail --tag 2`.
`tag remove` rejects tags still referenced by records.

Commands with one required value take it positionally: `record create <NAME>`,
`tag create <NAME>`, `record show/update/remove <RECORD_ID>`,
`tag remove <TAG_ID>`, `vault switch <DIR>` (session only), and
`completions <SHELL>`. Quote names containing spaces. The positional value must
immediately follow the command. If it follows options, use its named form:
`--name`, `--record-id`, `--tag-id`, `--dir`, or `--shell`, respectively.
The named form can appear anywhere among that command's arguments; supplying
both forms is an error.

```text
repass record create mail --tag 1
repass record create --tag 1 --name mail
repass record show 1 --reveal
repass record show --reveal --record-id 1
repass record update --add-tag 2 --record-id 1
repass tag create --name work
repass completions --shell bash
```

After opening and decrypting the vault, record mutation commands check the
target record, data IDs, tag IDs and conflicting operations before requesting
new secret input. New secret contents are validated after they are entered.

`--host` replaces the former `--url`: it accepts an IP address or domain without
protocol, port or path. Host filters combine with exact names and tag
intersection. IP addresses compare by their parsed value; domains compare
case-insensitively, ignoring a trailing dot. No DNS lookup is performed.

### Fuzzy search

`record find --query <TEXT>` uses `frizbee` to match the name, username, host,
notes and tag names. Each field is matched separately; a record is returned
once, ranked by its best field score. Results are ordered by descending score,
then by stable record ID. Matching ignores case and supports Unicode.

Query length is measured in Unicode scalar values: 1–3 scalars allow no typos,
4–7 allow one, and longer queries allow up to two. Noncontiguous characters can
match, so abbreviations are supported even when no typos are allowed. Leading
and trailing query whitespace is trimmed; blank queries are rejected. Query
text is a single fuzzy pattern, with no special operator syntax.

Combine `--query` with exact `--name`, `--host` and tag-intersection filters:

```text
repass record find --query githab
repass record find --query alice --host example.test --tag 1 2
```

The same syntax works in interactive sessions. Without `--query`, `find`
uses exact filters and orders results by ID. Secret values are not searched.

Examples (master-password input is handled separately by the CLI):

```text
repass record create server --host example.test --private-key-file id_ed25519 --public-key-file id_ed25519.pub
repass record update 1 --totp-stdin --algorithm sha256 --digits 8
repass record update 1 --code-stdin
repass record update 1 --replace-data 1 --private-key-file new_key --public-key-file new_key.pub
repass record find --host EXAMPLE.TEST.
repass record show 1 --reveal
repass record update 1 --remove-data 2
```

## Colors

Help headings, commands, arguments, interactive prompts, and status messages are
colored in terminals. Errors use red, TODO messages yellow, and successful status
messages green. Redirected output is plain text; setting `NO_COLOR=1` disables
colors. Generated passwords and completion scripts have no added color codes.

## Shell completion

`repass completions <SHELL>` prints a static completion script for `bash`, `zsh`,
`fish`, `powershell`, or `elvish`. It works without a vault or configured data
directory. Completion includes commands, flags, separator strategy names and
numbers, and file/directory hints where supported by the shell. This is completion
for the outer shell, not Tab completion inside the `repass>` session.

Examples (with `repass` installed in PATH):

```bash
# Bash: load for the current shell
source <(repass completions bash)
```

```zsh
# Zsh: initialize completion, then load for the current shell
autoload -Uz compinit
compinit
source <(repass completions zsh)
```

```fish
# Fish: load for the current shell
repass completions fish | source
```

For persistent setup, save the generated script in your shell's completion
directory or source it from its startup configuration. The CLI only writes the
script to stdout; it does not install or modify shell configuration files.

## Separator strategies

Generation works through `repass_generator`. Length means Unicode scalar values,
including separators. The default dictionary uses all built-in character sets.
Specify `--length` or both `--min-length` and `--max-length`, together with
`--separator-kind`. The session prompts for a missing exact length and strategy,
and displays the numbered strategy list.

Strategies accept either their name or the fixed number shown in help:

1. `none` — join dictionary entries without separators.
2. `between-parts` — insert separators between dictionary entries.
3. `fixed-interval` — insert separators after each interval of Unicode scalar
   values in the content, regardless of dictionary entry boundaries.
4. `fixed-count` — distribute the requested number of separators across content;
   short content may reduce the number of insertions.

`--separator` defaults to `-` and must be nonempty. `--separator-interval` must
be a positive integer (default `5` when used by `fixed-interval`), and
`--separator-count` must be a positive integer (default `3` when used by
`fixed-count`). Values are type-checked even when they do not apply to the chosen
strategy; valid but unused options are ignored with a warning by default.
`--warnings` enables these warnings and `--no-warnings` suppresses them for that
generation command. One-shot generation writes warnings to stderr and passwords
to stdout. In an interactive session, the command-level flags override
the session setting only for that invocation. Set the session default on entry
with `interactive --warnings` or `interactive --no-warnings`, then inspect or
change it with `warnings`, `warnings on`, or `warnings off`. Some exact lengths
cannot be formed with a chosen dictionary and separator configuration; the
generator reports an error rather than changing the requested length.

The length range is inclusive. Targets are sampled from the whole range; an
unreachable sampled target is an error. `--shape-selection first` (the default)
uses the first feasible layout. `random` samples uniformly among feasible layouts,
not among all possible passwords.

`--preset` selects one or more built-in sets: `digits`, `lowercase`, `uppercase`,
`punctuation`, `symbols`, `brackets`, `quotes`, `hash-dollar-percent-caret`, and
`backslash-pipe-tilde`. It conflicts with `--dictionary`. Selecting several sets
builds their union; it does not require every selected set to appear in each
password. Without either option, all built-in sets are used.

Individual output lengths are limited to 1,000,000 Unicode scalars, batch counts
to 100,000, and collected batch lengths to 16,000,000 scalars, calculated from
the maximum target length. The library exposes customizable limits and lazy
streaming generation.

```text
repass generate --length 16 --separator-kind none
repass generate --length 16 --separator-kind 2 --separator "::"
repass generate --length 20 --separator-kind fixed-interval --separator-interval 4
repass generate --length 20 --separator-kind 4 --separator-count 2
repass generate --min-length 12 --max-length 20 --separator-kind none --preset digits lowercase
repass generate --length 18 --separator-kind between-parts --dictionary words.txt --shape-selection random
```

## Storage format and tag catalog

Storage uses three files in the selected data directory:

```text
metadata.repass  # key metadata
records.repass   # encrypted records and the next record ID
tags.repass      # encrypted tag names and the next tag ID; optional
metadata.repass.bak  # wrapped-key backup, refreshed on master-password change
records.repass.bak   # authenticated previous records (initially the first snapshot)
tags.repass.bak      # authenticated previous tag catalog
```

Record changes atomically replace `records.repass`; tag catalog changes
atomically replace `tags.repass`. `metadata.repass` stores authenticated counts
of records and persisted tag names. The data file is replaced before its count
is updated, so the two-file operation is not a cross-file transaction. On open,
counts are reconciled against readable data files. Only the current metadata and
data-file format versions are accepted; older or unknown versions are rejected
without migration. Current data-file headers authenticate the schema version,
allowing an unsupported data schema to be reported distinctly from an
authentication failure.
Record schema version 2 stores typed data, per-record data-ID allocation state
and a host. It deliberately breaks compatibility with password-only record
schema version 1; existing schema-1 vaults are rejected and are not migrated.
Tag schema version 1 and the metadata/data container format versions are unchanged.
Vault sessions hold an exclusive advisory lock on the data directory. A second
session opening the same directory receives an error until the first closes.
Directory locking and directory synchronization are supported on the Linux/Unix
platforms targeted by this project. Data writes synchronize the temporary file
and, on Unix, the parent directory after publication. If publication succeeded
but directory synchronization failed, the error explicitly says that the file
was already replaced.

Counts are auxiliary: damaged counts can be rebuilt from readable data, and a
failure to write repaired counts does not prevent opening the records. `vault
info` reports such a repair failure. Operations that do not change counts avoid
rewriting metadata.

Storage commands lazily request the master password and create a new vault when
no vault files exist. `vault init` explicitly creates one and refuses to replace
existing files. A partial initialization is reported as an error rather than
silently treated as an empty vault.
Creating a vault, explicitly or on first use, requires a nonempty master
password and hidden confirmation. A mismatch or interrupted confirmation does
not create a vault. Opening an existing vault requests the password only once.
On Unix, newly created vault directories (including newly created parent
directories) use owner-only permissions, at most `0700`. Initializing in an
existing directory removes group/other permissions from that directory while
preserving stricter owner permissions; existing ancestors are not changed.
New vault files, backups, temporary files and preserved damaged copies use at
most `0600`. Atomic replacement also retains stricter owner permissions of an
existing destination, such as `0400`, and removes group/other permissions.
The process umask can restrict these permissions further. Existing vault
directories and files are not automatically chmod'ed on open; file permissions
are restricted when files are rewritten. This Unix mode policy does not set
Windows ACLs.
Surviving `.bak` files also prevent automatic initialization when the primary
files are missing; use `vault recover` to restore them.

`vault change-password` requests the current password when opening, then requests
and confirms a nonempty new password. It rewraps the existing data key in both
metadata and its backup; record and tag files are not rewritten. The metadata
backup is updated first. If changing the primary metadata subsequently fails,
the original primary remains usable with the old password and `vault recover`
with the new password can publish the updated backup.
In one-shot mode, all master-password questions go to stderr, including the new
password and confirmation questions; redirecting stdout does not hide them.

`vault recover` validates backup authentication and domain invariants before
replacing damaged or missing files. Damaged originals are copied to unique
`.damaged-<pid>-<counter>` files. Restoring records or tags uses the previous
snapshot and can therefore lose the most recent mutation. Recovery is explicit,
not automatic, and cannot recreate data without a usable backup. Individual
files are published atomically; recovery across several files is not a single
transaction and can be retried after an I/O failure.

`vault finish-init` explicitly creates empty records only when authenticated
metadata has valid zero counts and neither records/tags nor their backups exist.
It refuses damaged count metadata and metadata with nonzero counts.
This command is an explicit decision to create empty records: stale counts alone
cannot prove that data files were never present.

`record find` and `record list` support exact `--name` and `--host` matching and
multiple `--tag` IDs. Every requested tag must match. Without `--query`, results
are ordered by stable record ID. `record find --query <TEXT>` uses fuzzy matching
and orders results by descending relevance, with record ID breaking ties.
List/find output does not reveal secret values.

The tag catalog is optional. If it is missing or unreadable, records remain
available. Referenced IDs without a stored name are shown as temporary technical
tags such as `#tag-42`. `tag rename <TAG_ID> --name <NAME>` saves that tag under
the same ID. A damaged catalog is not overwritten by ordinary tag operations;
`tag recover` explicitly rebuilds it using technical names for the tags currently
known from records. Names that could not be read from a damaged catalog cannot be
recovered by this operation. A missing catalog with evidence of previous tags
(saved counts, record references or a surviving backup) blocks ordinary catalog
changes until explicit recovery. A never-created catalog in a new vault remains
writable. Saving after explicit rebuilding does not overwrite a surviving
backup when the primary catalog is missing.

## Input and file limits

- Metadata reads are bounded by the fixed maximum size of the current format.
- Each encrypted records/tag file and its backup is limited to 64 MiB on read
  and save. Existing files above this size are rejected.
- SSH files and accumulated SSH stdin input are limited to 1 MiB per part.
- One-shot secret lines and interactive command/argument lines are limited to
  1 MiB including line endings. Hidden terminal secrets are checked against
  1 MiB after terminal input returns.
- Dictionary files are limited to 16 MiB and 100,000 nonempty entries. The same
  limits apply when saving a dictionary.

Limits count bytes. Readers enforce limits during reading; damaged-file
preservation during recovery streams the original to
disk rather than loading it into memory.

This is a new storage format. Existing `vault.repass` files are not migrated or
modified automatically; the CLI reports them as unsupported.
