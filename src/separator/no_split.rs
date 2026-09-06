use super::{IllegalArgumentError, SeparatorStrategy, SeparatorInternal};

#[derive(Debug)]
pub struct WithoutSeparator {}

impl WithoutSeparator {
    pub fn new() -> Self {
        Self {}
    }
}

impl SeparatorInternal for WithoutSeparator {
    fn add_separator(self: &Self, parts: &[&str]) -> String {
        parts.concat()
    }

    fn length_with_separators(self: &Self, parts: &[&str]) -> usize {
        super::get_summary_length(parts)
    }

    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError> {
        let mut errors = Vec::new();
        if parts.is_empty() {
            errors.push(IllegalArgumentError::SummaryLengthOfPartsIsZero)
        }
        if super::get_summary_length(parts) == 0 {
            errors.push(IllegalArgumentError::SummaryLengthOfPartsIsZero)
        }
        errors
    }
}

impl SeparatorStrategy for WithoutSeparator {}

mod tests {
    use super::{WithoutSeparator, SeparatorStrategy};
    use rstest::{fixture, rstest};

    #[rstest]
    #[case(vec!["1",])]
    #[case(vec!["biba",])]
    #[case(vec!["1", "2",])]
    #[case(vec!["1", "2", "3",])]
    #[case(vec!["1", "2", "3", "4",])]
    #[case(vec!["biba", "boba",])]
    fn length_test(
        #[values(WithoutSeparator::new())] separator: WithoutSeparator,
        #[case] input: Vec<&str>,
    ) {
        let result_len = separator.separate(&input).len();
        let compute_len = separator.length_after_separate(&input);
        assert_eq!(result_len, compute_len);
    }

    #[fixture]
    fn separator() -> WithoutSeparator {
        WithoutSeparator::new()
    }

    #[rstest]
    #[case(Vec::new())]
    #[case(vec![""])]
    #[case(vec!["", "",])]
    #[should_panic]
    fn panic_test(
        #[values(separator())] separator: WithoutSeparator,
        #[case] input: Vec<&str>,
    ) {
        separator.separate(&input);
    }

    #[rstest]
    #[case(vec!["1",], "1")]
    #[case(vec!["biba",], "biba")]
    #[case(vec!["1", "2",], "12")]
    #[case(vec!["1", "2", "3",], "123")]
    #[case(vec!["1", "2", "3", "4",], "1234")]
    #[case(vec!["biba", "boba",], "bibaboba")]
    fn multi_long_test(
        #[values(separator())] separator: WithoutSeparator,
        #[case] input: Vec<&str>,
        #[case] output: &str,
    ) {
        let result = separator.separate(&input);
        assert_eq!(result, output);
    }
}
