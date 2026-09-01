use super::{IllegalArgumentError, SeparatorStrategy, SeparatorInternal};
use core::str;

#[derive(Debug)]
pub struct BetweenPartsSeparator<'a> {
    separator: &'a str,
}

impl<'a> BetweenPartsSeparator<'a> {
    pub fn new(separator: &'a str) -> Self {
        Self { separator }
    }
}

impl SeparatorInternal for BetweenPartsSeparator<'_> {
    fn add_separator(self: &Self, parts: &[&str]) -> String {
        parts.join(self.separator)
    }

    fn length_with_separators(self: &Self, parts: &[&str]) -> usize {
        let base_length = Self::get_summary_length(parts);
        base_length + self.separator.len() * (parts.len() - 1)
    }

    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError> {
        let mut errors = Vec::new();
        if self.separator.is_empty() {
            errors.push(IllegalArgumentError::EmptySeparator)
        }
        if parts.is_empty() {
            errors.push(IllegalArgumentError::SummaryLengthOfPartsIsZero)
        }
        if Self::get_summary_length(parts) == 0 {
            errors.push(IllegalArgumentError::SummaryLengthOfPartsIsZero)
        }
        errors
    }
}

impl SeparatorStrategy for BetweenPartsSeparator<'_> {}

#[cfg(test)]
mod tests {
    use rstest::fixture;
    use super::BetweenPartsSeparator;
    use super::SeparatorStrategy;
    
    #[fixture]
    fn empty<'a>() -> BetweenPartsSeparator<'a> {
        BetweenPartsSeparator::new("")
    }

    #[fixture]
    fn dash<'a>() -> BetweenPartsSeparator<'a> {
        BetweenPartsSeparator::new("-")
    }

    #[fixture]
    fn long<'a>() -> BetweenPartsSeparator<'a> {
        BetweenPartsSeparator::new("boba")
    }

    
}
