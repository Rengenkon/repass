# repass CLI

Run with `cargo run -p repass_cli -- <arguments>` or build the `repass` binary.

```text
repass [--data-dir <DIR>] generate --length <N> --separator-kind <KIND> [--count <N>] [--dictionary <FILE>] [--separator <TEXT>] [--separator-interval <N>] [--separator-count <N>]
repass [--data-dir <DIR>] vault init
repass [--data-dir <DIR>] vault info
repass [--data-dir <DIR>] record add --name <NAME> --password-stdin [--username <TEXT>] [--url <URL>] [--notes <TEXT>] [--tag <TAG_ID>...]
repass [--data-dir <DIR>] record list [--tag <TAG_ID>]
repass [--data-dir <DIR>] record show <RECORD_ID> [--reveal]
repass [--data-dir <DIR>] record update <RECORD_ID> [--name <NAME>] [--username <TEXT>] [--url <URL>] [--notes <TEXT>] [--password-stdin] [--add-tag <TAG_ID>...] [--remove-tag <TAG_ID>...]
repass [--data-dir <DIR>] record delete <RECORD_ID>
repass [--data-dir <DIR>] tag add --name <NAME>
repass [--data-dir <DIR>] tag list
repass [--data-dir <DIR>] tag delete <TAG_ID>
repass [--data-dir <DIR>] interactive
```

`--data-dir` is global and can also follow a subcommand. Directory precedence:
explicit argument, `REPASS_DATA_DIR`, then `$HOME/.repass`. A leading `~` path
component expands to `$HOME`. An empty selected directory is an error.

`repass` without a command displays help. Use `repass interactive` to start a
session. Enter the same commands
without the `repass` prefix; quotes preserve spaces. Missing mandatory arguments
are prompted. Password input is hidden in the terminal; in a one-shot invocation,
`--password-stdin` reads one line from stdin (removing its line ending).
Optional arguments are not prompted. Use `h` or `help` for help (including
`h generate` and `help record add`), and `q`, `quit`, or EOF to leave.
`exit` is not a supported command.
The `interactive` command is unavailable and absent from help inside a session.

Inside a session, ordinary commands cannot accept `--data-dir`. Use
`vault switch <DIR>` to close the old vault and change the session's directory.
Opening and decryption are lazy; generation and help do not open a vault.

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

`--separator` defaults to `-`, must be nonempty, and is inapplicable to `none`.
`--separator-interval` is a positive integer for `fixed-interval` only (default
`5`). `--separator-count` is a positive integer for `fixed-count` only (default
`3`). Unsupported option/strategy combinations are errors. Some exact lengths
cannot be formed with a chosen dictionary and separator configuration; the
generator reports an error rather than changing the requested length.

```text
repass generate --length 16 --separator-kind none
repass generate --length 16 --separator-kind 2 --separator "::"
repass generate --length 20 --separator-kind fixed-interval --separator-interval 4
repass generate --length 20 --separator-kind 4 --separator-count 2
```

## Current module limitations

Storage commands currently return explanatory TODO errors. `repass_storage`
already has an owned `Vault` with file-level create/open and document save/load
APIs, but no directory-level API to select its files and initialize the directory.
The CLI creates no data directories or vault files and does not select internal
filenames. Its session reserves ownership of the existing `Vault` type; actual
lazy opening and key reuse await the directory-level API. Records also need
stable domain IDs and support for the agreed fields; tags need persisted
enumeration and deletion with reference checks. The existing generic document
save API replaces all contents and cannot serve as a partial record/tag update.

Once those APIs exist, handlers must retain the owned vault, save each successful
mutation through the module, and close it on switch or session termination. A failed save must
retain the current vault. `vault init` must not overwrite an existing vault;
other storage commands should ask the module to initialize an absent vault.
No persistence format is defined by this CLI.
