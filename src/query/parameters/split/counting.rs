use super::{IllegalArgumentError, SeparatorStrategy, SeparatorInternal};
use core::str;
use std::cmp::Ordering;

#[derive(Debug)]
pub struct FixIntervalSeparator<'a> {
    separator: &'a str,
    length: usize,
}

impl<'a> FixIntervalSeparator<'a> {
    pub fn new(separator: &'a str, length: usize) -> Self {
        Self {
            separator,
            length,
        }
    }
}

impl SeparatorInternal for FixIntervalSeparator<'_> {
    fn add_separator(self: &Self, parts: &[&str]) -> String {
        let mut separated = String::new();
        let mut iter = parts.iter();
        let mut cmp_result = Ordering::Less;
        let mut value = "";
        let mut diff = self.length;

        loop {
            if cmp_result != Ordering::Greater {
                let part = iter.next();
                if part.is_none() {
                    break;
                }
                value = part.unwrap();
                if !value.is_empty() && cmp_result == Ordering::Equal {
                    separated.push_str(self.separator);
                    diff = self.length;
                }
            } else {
                separated.push_str(self.separator);
                diff = self.length;
            }
            cmp_result = value.len().cmp(&diff);
            match cmp_result {
                Ordering::Less | Ordering::Equal => {
                    separated.push_str(value);
                    diff -= value.len();
                }
                Ordering::Greater => {
                    let (left, right) = value.split_at(diff);
                    separated.push_str(left);
                    value = right;
                }
            }
        }
        separated

    }

    fn length_with_separators(self: &Self, parts: &[&str]) -> usize {
        let sum_length = Self::get_summary_length(parts);
        let mut separators_count = sum_length / self.length;
        if separators_count > 0 && sum_length % self.length == 0 {
            separators_count -= 1;
        }
        sum_length + self.separator.len() * separators_count

    }

    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError> {
        let mut errors = Vec::new();
        if self.separator.is_empty() {
            errors.push(IllegalArgumentError::EmptySeparator)
        }
        if self.length == 0 {
            errors.push(IllegalArgumentError::AdditionalParameterIsZero)
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

impl SeparatorStrategy for FixIntervalSeparator<'_> {}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use super::*;

    #[fixture]
    fn without_separator<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator{
            separator: "",
            length: 1
        }
    }

    #[fixture]
    fn without_count<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator{
            separator: "biba",
            length: 0
        }
    }

    #[fixture]
    fn one_dash<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator{
            separator: "-",
            length: 1
        }
    }

    #[fixture]
    fn one_long<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator{
            separator: "biba",
            length: 1
        }
    }

    #[fixture]
    fn multi_long<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator{
            separator: "biba",
            length: 3
        }
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
        #[values(
            without_separator(),
            without_count(),
        )] separator: FixIntervalSeparator,
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
        #[values(
            one_dash(),
            one_long(),
            multi_long(),
        )] separator: FixIntervalSeparator,
        #[case] input: Vec<&str>,
    ) {
        separator.separate(&input);
    }

    #[rstest]
    #[case(vec!["1",])]
    #[case(vec!["biba",])]
    #[case(vec!["1", "2",])]
    #[case(vec!["1", "2", "3",])]
    #[case(vec!["1", "2", "3", "4",])]
    #[case(vec!["biba", "boba",])]
    fn length_test(
        #[values(
            one_dash(),
            one_long(),
            multi_long(),
        )] separator: FixIntervalSeparator,
        #[case] input: Vec<&str>,
    ) {
        let result_len = separator.separate(&input).len();
        let compute_len = separator.length_after_separate(&input);
        assert_eq!(result_len, compute_len);
    }

    #[rstest]
    #[case(vec!["1",], "1")]
    #[case(vec!["biba",], "b-i-b-a")]
    #[case(vec!["1", "2",], "1-2")]
    #[case(vec!["1", "2", "3",], "1-2-3")]
    #[case(vec!["1", "2", "3", "4",], "1-2-3-4")]
    #[case(vec!["biba", "boba",], "b-i-b-a-b-o-b-a")]
    fn one_dash_test(
        #[from(one_dash)] separator: FixIntervalSeparator,
        #[case] input: Vec<&str>,
        #[case] output: &str,
    ) {
        let result = separator.separate(&input);
        assert_eq!(result, output);
    }
    
    #[rstest]
    #[case(vec!["1",], "1")]
    #[case(vec!["biba",], "bbibaibibabbibaa")]
    #[case(vec!["1", "2",], "1biba2")]
    #[case(vec!["1", "2", "3",], "1biba2biba3")]
    #[case(vec!["1", "2", "3", "4",], "1biba2biba3biba4")]
    #[case(vec!["biba", "boba",], "bbibaibibabbibaabibabbibaobibabbibaa")]
    fn one_long_test(
        #[from(one_long)] separator: FixIntervalSeparator,
        #[case] input: Vec<&str>,
        #[case] output: &str,
    ) {
        let result = separator.separate(&input);
        assert_eq!(result, output);
    }

    #[rstest]
    #[case(vec!["1",], "1")]
    #[case(vec!["biba",], "bibbibaa")]
    #[case(vec!["1", "2",], "12")]
    #[case(vec!["1", "2", "3",], "123")]
    #[case(vec!["1", "2", "3", "4",], "123biba4")]
    #[case(vec!["biba", "boba",], "bibbibaabobibaba")]
    fn multi_long_test(
        #[from(multi_long)] separator: FixIntervalSeparator,
        #[case] input: Vec<&str>,
        #[case] output: &str,
    ) {
        let result = separator.separate(&input);
        assert_eq!(result, output);
    }
}
