use enum_display::EnumDisplay;

pub mod counting;
pub mod no_split;
pub mod number;
pub mod parts;

trait SeparatorInternal {
    fn get_summary_length(parts: &[&str]) -> usize {
        parts.iter().map(|part| part.len()).sum::<usize>()
    }

    fn format_error_msg(errors: &Vec<IllegalArgumentError>) -> String {
        let mut result = String::new();
        for error in errors {
            todo!()
        }
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
            let result = f(self, parts);
            if result.is_ok() {
                return result.ok().unwrap();
            }
            panic!("{}", result.err().unwrap())
        }
        let errors_msg = Self::format_error_msg(&errors);
        panic!("{}", errors_msg)
    }
}

/// Determine public functions for working with 'Separators'
///
/// 'Sequence' is 'parts or character of parts'
pub trait SeparateStrategy: SeparatorInternal {
    /// Separate sequence with setuped separate segment
    fn separate(self: &Self, parts: &[&str]) -> String {
        self.panic_with_errors(parts, Self::add_separator)
    }

    /// Compute final length of sequence with separators
    fn compute_final_length(self: &Self, parts: &[&str]) -> usize {
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
    CountOfSeparatorsIsZero,
    #[display("")]
    SummaryLengthOfPartsIsZero,
    #[display("")]
    Specific(String),
}
