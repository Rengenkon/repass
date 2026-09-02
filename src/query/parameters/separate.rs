use enum_display::EnumDisplay;

pub mod interval;
pub mod no_split;
pub mod fix_count;
pub mod on_parts;

trait SeparatorInternal {
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

    fn add_separator(self: &Self, parts: &[&str]) -> String;
    fn length_with_separators(self: &Self, parts: &[&str]) -> usize;

    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError>;

    fn panic_with_errors<T, F>(self: &Self, parts: &[&str], f: F) -> T
    where
        F: Fn(&Self, &[&str]) -> T,
    {
        let errors = self.chack_errors(parts);
        if errors.is_empty() {
            return f(self, parts)
        }
        let errors_msg = Self::format_error_msg(&errors);
        panic!("{}", errors_msg)
    }
}

/// Determine public functions for working with 'Separators'
///
/// 'Sequence' is 'parts or character of parts'
pub trait SeparatorStrategy: SeparatorInternal {
    /// Separate sequence with setuped separate segment
    fn separate(self: &Self, parts: &[&str]) -> String {
        self.panic_with_errors(parts, Self::add_separator)
    }

    /// Compute final length of sequence with separators
    fn length_after_separate(self: &Self, parts: &[&str]) -> usize {
        self.panic_with_errors(parts, Self::length_with_separators)
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
