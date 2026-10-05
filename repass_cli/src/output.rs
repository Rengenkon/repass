use anstyle::{AnsiColor, Style};
use clap::builder::Styles;
use std::fmt::Display;
use std::io::{self, Write};

pub const PROMPT: Style = AnsiColor::Cyan.on_default().bold();
pub const HEADING: Style = AnsiColor::Green.on_default().bold();
pub const SUCCESS: Style = AnsiColor::Green.on_default();
const ERROR: Style = AnsiColor::Red.on_default().bold();
const TODO: Style = AnsiColor::Yellow.on_default();

pub fn styles() -> Styles {
    Styles::styled()
        .header(HEADING)
        .usage(HEADING)
        .literal(PROMPT)
        .placeholder(AnsiColor::Yellow.on_default())
        .error(ERROR)
        .valid(SUCCESS)
        .invalid(ERROR)
}

// The caller's AutoStream strips these styles for redirected output and NO_COLOR.
pub fn styled(output: &mut impl Write, style: Style, text: impl Display) -> io::Result<()> {
    write!(output, "{style}{text}{style:#}")
}

pub fn error(output: &mut impl Write, error: impl Display) -> io::Result<()> {
    styled(output, ERROR, "error:")?;
    let message = error.to_string();
    write!(output, " ")?;
    if message.starts_with("TODO:") {
        styled(output, TODO, message)?;
    } else {
        write!(output, "{message}")?;
    }
    writeln!(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anstream::{AutoStream, ColorChoice};

    #[test]
    fn styles_are_emitted_only_when_the_stream_enables_color() {
        for choice in [ColorChoice::Always, ColorChoice::Never] {
            let mut bytes = Vec::new();
            {
                let mut stream = AutoStream::new(&mut bytes, choice);
                styled(&mut stream, PROMPT, "repass> ").unwrap();
                error(&mut stream, "TODO: missing storage API").unwrap();
            }
            let text = String::from_utf8(bytes).unwrap();
            assert_eq!(text.contains('\x1b'), choice == ColorChoice::Always);
            assert!(text.contains("repass> "));
            assert!(text.contains("TODO: missing storage API"));
        }
    }
}
