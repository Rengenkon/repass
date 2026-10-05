use crate::{Result, commands::Cli};
use clap::{CommandFactory, builder::PossibleValuesParser};
use clap_complete::Shell;
use std::io::Write;

pub fn generate(shell: Shell, output: &mut impl Write) -> Result<()> {
    let mut command = Cli::command().mut_subcommand("generate", |generate| {
        generate.mut_arg("separator_kind", |arg| {
            // Static completion generators enumerate canonical values rather
            // than aliases. Expose the accepted numbers in the completion view.
            arg.value_parser(PossibleValuesParser::new([
                "none",
                "1",
                "between-parts",
                "2",
                "fixed-interval",
                "3",
                "fixed-count",
                "4",
            ]))
            .hide_possible_values(false)
        })
    });
    // Generate into memory first: clap_complete's writer does not report errors.
    // Propagate any failure writing the finished script to the caller's stream.
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut command, "repass", &mut script);
    output.write_all(&script)?;
    Ok(())
}
