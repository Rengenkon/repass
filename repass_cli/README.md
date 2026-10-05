# repass CLI

Run with `cargo run -p repass_cli -- <arguments>` or build the `repass` binary.

```text
repass generate --length <N> --separator-kind <KIND> [--count <N>] [--dictionary <FILE>] [--separator <TEXT>] [--separator-interval <N>] [--separator-count <N>] [--warnings | --no-warnings]
repass vault [--data-dir <DIR>] init
repass vault [--data-dir <DIR>] info
repass record [--data-dir <DIR>] add --name <NAME> --password-stdin [--username <TEXT>] [--url <URL>] [--notes <TEXT>] [--tag <TAG_ID>...]
repass record [--data-dir <DIR>] list [--tag <TAG_ID>]
repass record [--data-dir <DIR>] show <RECORD_ID> [--reveal]
repass record [--data-dir <DIR>] update <RECORD_ID> [--name <NAME>] [--username <TEXT> | --clear-username] [--url <URL> | --clear-url] [--notes <TEXT> | --clear-notes] [--password-stdin] [--add-tag <TAG_ID>...] [--remove-tag <TAG_ID>...]
repass record [--data-dir <DIR>] delete <RECORD_ID>
repass tag [--data-dir <DIR>] add --name <NAME>
repass tag [--data-dir <DIR>] list
repass tag [--data-dir <DIR>] delete <TAG_ID>
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
are prompted. The master password and record passwords are requested with hidden
terminal input. In a one-shot invocation, `--password-stdin` reads one record
password line from stdin (removing its line ending).
Optional arguments are not prompted. Use `h` or `help` for help (including
`h generate` and `help record add`), and `q`, `quit`, or EOF to leave.
`exit` is not a supported command.
The `interactive` command is unavailable and absent from help inside a session.

Inside a session, ordinary commands cannot accept `--data-dir`. Use
`vault switch <DIR>` to close the old vault and change the session's directory.
Opening and decryption are lazy; generation and help do not open a vault.

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
Both `--length` and `--separator-kind` are mandatory in one-shot mode; the session
prompts for missing values and displays the numbered strategy list.

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

```text
repass generate --length 16 --separator-kind none
repass generate --length 16 --separator-kind 2 --separator "::"
repass generate --length 20 --separator-kind fixed-interval --separator-interval 4
repass generate --length 20 --separator-kind 4 --separator-count 2
```

## Storage format and tag catalog

Storage uses three files in the selected data directory:

```text
metadata.repass  # key metadata
records.repass   # encrypted records and the next record ID
tags.repass      # encrypted tag names and the next tag ID; optional
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
Storage commands lazily request the master password and create a new vault when
no vault files exist. `vault init` explicitly creates one and refuses to replace
existing files. A partial initialization is reported as an error rather than
silently treated as an empty vault.

The tag catalog is optional. If it is missing or unreadable, records remain
available. Referenced IDs without a stored name are shown as temporary technical
tags such as `#tag-42`. `tag rename <TAG_ID> --name <NAME>` saves that tag under
the same ID. A damaged catalog is not overwritten by ordinary tag operations;
`tag recover` explicitly rebuilds it using technical names for the tags currently
known from records. Names that could not be read from a damaged catalog cannot be
recovered by this operation.

This is a new storage format. Existing `vault.repass` files are not migrated or
modified automatically; the CLI reports them as unsupported.
