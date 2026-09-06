use enum_display::EnumDisplay;

pub mod interval;
pub mod no_split;
pub mod fix_count;
pub mod on_parts;

fn get_summary_length(parts: &[&str]) -> usize {
    parts.iter().map(|part| part.len()).sum::<usize>()
}

fn format_error_msg(errors: &Vec<IllegalArgumentError>) -> String {
    let mut result = String::from("Detected this errors for current Separator and parts:");
    for error in errors {
        result.push_str("\n\t-");
        result.push_str(&error.to_string());
    }
    result.push_str("\n\n");
    result
}

fn try_panic(errors: &Vec<IllegalArgumentError>) {
    if !errors.is_empty() {
        let errors_msg = format_error_msg(&errors);
        panic!("{}", errors_msg)
    }
}

pub trait SeparatorInternal {
    fn add_separator(self: &Self, parts: &[&str]) -> String;
    fn length_with_separators(self: &Self, parts: &[&str]) -> usize;
    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError>;
}

/// Determine public functions for working with 'Separators'
///
/// 'Sequence' is 'parts or character of parts'
pub trait SeparatorStrategy: SeparatorInternal {
    /// Separate sequence with setuped separator segment
    fn separate(self: &Self, parts: &[&str]) -> String {
        let errors = self.chack_errors(parts);
        try_panic(&errors);
        self.add_separator(parts)
    }

    /// Compute final length of sequence with separators
    fn length_after_separate(self: &Self, parts: &[&str]) -> usize {
        let errors = self.chack_errors(parts);
        try_panic(&errors);
        self.length_with_separators(parts)
    }

    /// Check common errors in Separator or parts
    fn validate(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError> {
        self.chack_errors(parts)
    }
}

#[derive(Debug, EnumDisplay)]
pub enum IllegalArgumentError {
    #[display("")]
    EmptySeparator,
    #[display("")]
    AdditionalParameterIsZero,
    #[display("")]
    SummaryLengthOfPartsIsZero,
    #[display("")]
    Specific(String),
}
