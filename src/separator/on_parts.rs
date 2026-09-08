use super::{DEFAULT_SEPARATOR, IllegalArgumentError, Separator, SeparatorInternal};
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

impl<'a> Default for BetweenPartsSeparator<'a> {
    fn default() -> Self {
        Self {
            separator: DEFAULT_SEPARATOR,
        }
    }
}

impl SeparatorInternal for BetweenPartsSeparator<'_> {
    fn add_separator(self: &Self, parts: &[&str]) -> String {
        parts.join(self.separator)
    }

    fn length_with_separators(self: &Self, parts: &[&str]) -> usize {
        let base_length = super::get_summary_length(parts);
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
        if super::get_summary_length(parts) == 0 {
            errors.push(IllegalArgumentError::SummaryLengthOfPartsIsZero)
        }
        errors
    }
}

impl Separator for BetweenPartsSeparator<'_> {
    fn try_compute_free_space(self: &Self, target_length: usize) -> Option<usize> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{BetweenPartsSeparator, Separator};
    use rstest::{fixture, rstest};

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

    #[rstest]
    #[case(vec!["1",])]
    #[case(vec!["biba",])]
    #[case(vec!["1", "2",])]
    #[case(vec!["1", "2", "3",])]
    #[case(vec!["1", "2", "3", "4",])]
    #[case(vec!["biba", "boba",])]
    #[should_panic]
    fn invalid_separator_panic_test(
        #[values(empty())] separator: BetweenPartsSeparator,
        #[case] input: Vec<&str>,
    ) {
        separator.separate(&input);
    }

    #[rstest]
    #[case(Vec::new())]
    #[case(vec![""])]
    #[case(vec!["", "",])]
    #[should_panic]
    fn invalid_data_panic_test(
        #[values(dash(), long())] separator: BetweenPartsSeparator,
        #[case] input: Vec<&str>,
    ) {
        separator.separate(&input);
    }

    #[rstest]
    #[case(vec!["1",], "1")]
    #[case(vec!["biba",], "biba")]
    #[case(vec!["1", "2",], "1-2")]
    #[case(vec!["1", "2", "3",], "1-2-3")]
    #[case(vec!["1", "2", "3", "4",], "1-2-3-4")]
    #[case(vec!["biba", "boba",], "biba-boba")]
    fn dash_test(
        #[from(dash)] separator: BetweenPartsSeparator,
        #[case] input: Vec<&str>,
        #[case] output: &str,
    ) {
        let result = separator.separate(&input);
        assert_eq!(result, output);
    }

    #[rstest]
    #[case(vec!["1",], "1")]
    #[case(vec!["biba",], "biba")]
    #[case(vec!["1", "2",], "1boba2")]
    #[case(vec!["1", "2", "3",], "1boba2boba3")]
    #[case(vec!["1", "2", "3", "4",], "1boba2boba3boba4")]
    #[case(vec!["biba", "boba",], "bibabobaboba")]
    fn long_test(
        #[from(long)] separator: BetweenPartsSeparator,
        #[case] input: Vec<&str>,
        #[case] output: &str,
    ) {
        let result = separator.separate(&input);
        assert_eq!(result, output);
    }
}
