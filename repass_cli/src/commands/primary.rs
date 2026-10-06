use clap::{Arg, ArgGroup, ArgMatches, Args, Command, FromArgMatches};
use repass_storage::TagId;
use std::path::PathBuf;

pub(crate) const GROUP: &str = "primary_value";

// One domain value, with mutually exclusive positional and named spellings.
macro_rules! primary_value {
    ($name:ident, $ty:ty, $id:literal, $named:literal, $long:literal, $label:literal, $help:literal) => {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $name(pub $ty);

        impl From<$ty> for $name {
            fn from(value: $ty) -> Self {
                Self(value)
            }
        }

        impl std::ops::Deref for $name {
            type Target = $ty;
            fn deref(&self) -> &$ty {
                &self.0
            }
        }

        impl Args for $name {
            fn augment_args(command: Command) -> Command {
                let rule = concat!(
                    "Put ",
                    $label,
                    " immediately after the command, or use --",
                    $long,
                    " in any position. Do not supply both forms."
                );
                command
                    .arg(
                        Arg::new($id)
                            .value_name($label)
                            .value_hint(if $long == "dir" {
                                clap::ValueHint::DirPath
                            } else {
                                clap::ValueHint::Unknown
                            })
                            .help($help)
                            .value_parser(clap::value_parser!($ty)),
                    )
                    .arg(
                        Arg::new($named)
                            .long($long)
                            .value_name($label)
                            .value_hint(if $long == "dir" {
                                clap::ValueHint::DirPath
                            } else {
                                clap::ValueHint::Unknown
                            })
                            .help(concat!($help, " (named form; any position)"))
                            .value_parser(clap::value_parser!($ty)),
                    )
                    .group(ArgGroup::new(GROUP).args([$id, $named]).required(true))
                    .after_help(rule)
            }

            fn augment_args_for_update(command: Command) -> Command {
                Self::augment_args(command)
            }
        }

        impl FromArgMatches for $name {
            fn from_arg_matches(matches: &ArgMatches) -> Result<Self, clap::Error> {
                if matches.contains_id($id) && matches.index_of($id) != Some(1) {
                    return Err(clap::Error::raw(
                        clap::error::ErrorKind::InvalidValue,
                        concat!(
                            $label,
                            " must immediately follow the command; otherwise use --",
                            $long,
                            "\n"
                        ),
                    ));
                }
                matches
                    .get_one::<$ty>($id)
                    .or_else(|| matches.get_one::<$ty>($named))
                    .cloned()
                    .map(Self)
                    .ok_or_else(|| {
                        clap::Error::raw(
                            clap::error::ErrorKind::MissingRequiredArgument,
                            concat!("provide ", $label, " or --", $long, "\n"),
                        )
                    })
            }

            fn update_from_arg_matches(&mut self, matches: &ArgMatches) -> Result<(), clap::Error> {
                *self = Self::from_arg_matches(matches)?;
                Ok(())
            }
        }
    };
}

primary_value!(
    Name,
    String,
    "name",
    "name_option",
    "name",
    "NAME",
    "Nonempty name"
);
primary_value!(
    Record,
    String,
    "record_id",
    "record_id_option",
    "record-id",
    "RECORD_ID",
    "Record ID from record list or record find"
);
primary_value!(
    Tag,
    TagId,
    "tag_id",
    "tag_id_option",
    "tag-id",
    "TAG_ID",
    "Tag ID from tag list"
);
primary_value!(
    Directory,
    PathBuf,
    "dir",
    "dir_option",
    "dir",
    "DIR",
    "Directory for subsequent storage commands"
);
primary_value!(
    Shell,
    clap_complete::Shell,
    "shell",
    "shell_option",
    "shell",
    "SHELL",
    "Shell for the completion script"
);

impl From<&str> for Name {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl From<&str> for Record {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}
