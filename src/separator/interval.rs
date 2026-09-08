use super::{DEFAULT_SEPARATOR, IllegalArgumentError, Separator, SeparatorInternal};
use core::str;
use std::cmp::Ordering;

#[derive(Debug)]
pub struct FixIntervalSeparator<'a> {
    separator: &'a str,
    interval: usize,
}

impl<'a> FixIntervalSeparator<'a> {
    pub fn new(separator: &'a str, interval: usize) -> Self {
        Self {
            separator,
            interval,
        }
    }
}

impl<'a> Default for FixIntervalSeparator<'a> {
    fn default() -> Self {
        Self {
            separator: DEFAULT_SEPARATOR,
            interval: 5,
        }
    }
}

impl SeparatorInternal for FixIntervalSeparator<'_> {
    fn add_separator(self: &Self, parts: &[&str]) -> String {
        let mut separated = String::new();
        let mut iter = parts.iter();
        let mut cmp_result = Ordering::Less;
        let mut value = "";
        let mut diff = self.interval;

        loop {
            if cmp_result != Ordering::Greater {
                let part = iter.next();
                if part.is_none() {
                    break;
                }
                value = part.unwrap();
                if !value.is_empty() && cmp_result == Ordering::Equal {
                    separated.push_str(self.separator);
                    diff = self.interval;
                }
            } else {
                separated.push_str(self.separator);
                diff = self.interval;
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
        let sum_length = super::get_summary_length(parts);
        let mut separators_count = sum_length / self.interval;
        if separators_count > 0 && sum_length % self.interval == 0 {
            separators_count -= 1;
        }
        sum_length + self.separator.len() * separators_count
    }

    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError> {
        let mut errors = Vec::new();
        if self.separator.is_empty() {
            errors.push(IllegalArgumentError::EmptySeparator)
        }
        if self.interval == 0 {
            errors.push(IllegalArgumentError::AdditionalParameterIsZero)
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

impl Separator for FixIntervalSeparator<'_> {
    fn try_compute_free_space(self: &Self, target_length: usize) -> Option<usize> {
        if target_length == 0 {
            return None;
        }
        let pairs_len = target_length - 1;
        let one_pair_len = self.interval + self.separator.len();
        let count = pairs_len / one_pair_len;
        if target_length - count * one_pair_len > self.interval {
            return None;
        }
        Some(target_length - count * self.separator.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::separator::get_summary_length;
    use rstest::{fixture, rstest};

    #[fixture]
    fn without_separator<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator {
            separator: "",
            interval: 1,
        }
    }

    #[fixture]
    fn without_count<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator {
            separator: "biba",
            interval: 0,
        }
    }

    #[fixture]
    fn one_dash<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator {
            separator: "-",
            interval: 1,
        }
    }

    #[fixture]
    fn one_long<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator {
            separator: "biba",
            interval: 1,
        }
    }

    #[fixture]
    fn multi_long<'a>() -> FixIntervalSeparator<'a> {
        FixIntervalSeparator {
            separator: "biba",
            interval: 3,
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
        #[values(without_separator(), without_count())] separator: FixIntervalSeparator,
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
        #[values(one_dash(), one_long(), multi_long())] separator: FixIntervalSeparator,
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
        #[values(one_dash(), one_long(), multi_long())] separator: FixIntervalSeparator,
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

    #[rstest]
    #[case(vec!["1",])]
    #[case(vec!["biba",])]
    #[case(vec!["1", "2",])]
    #[case(vec!["1", "2", "3",])]
    #[case(vec!["1", "2", "3", "4",])]
    #[case(vec!["biba", "boba",])]
    fn space_test(
        #[values(one_dash(), one_long(), multi_long())] separator: FixIntervalSeparator,
        #[case] input: Vec<&str>,
    ) {
        let stat_len = get_summary_length(&input);
        let compute_len = separator.length_after_separate(&input);
        let spaces = separator.try_compute_free_space(compute_len).unwrap();
        assert_eq!(stat_len, spaces);
    }

    #[rstest]
    #[case(0, None)]
    #[case(1, Some(1))]
    #[case(2, None)]
    #[case(3, Some(2))]
    #[case(4, None)]
    #[case(5, Some(3))]
    fn one_dash_space_test(
        #[from(one_dash)] separator: FixIntervalSeparator,
        #[case] input: usize,
        #[case] output: Option<usize>,
    ) {
        let result = separator.try_compute_free_space(input);
        assert_eq!(result, output);
    }

    #[rstest]
    #[case(0, None)]
    #[case(1, Some(1))]
    #[case(2, None)]
    #[case(3, None)]
    #[case(4, None)]
    #[case(5, None)]
    #[case(6, Some(2))]
    #[case(7, None)]
    #[case(8, None)]
    #[case(11, Some(3))]
    fn one_long_space_test(
        #[from(one_long)] separator: FixIntervalSeparator,
        #[case] input: usize,
        #[case] output: Option<usize>,
    ) {
        let result = separator.try_compute_free_space(input);
        assert_eq!(result, output);
    }

    #[rstest]
    #[case(0, None)]
    #[case(1, Some(1))]
    #[case(2, Some(2))]
    #[case(3, Some(3))]
    #[case(4, None)]
    #[case(5, None)]
    #[case(6, None)]
    #[case(7, None)]
    #[case(8, Some(4))]
    #[case(9, Some(5))]
    #[case(10, Some(6))]
    #[case(11, None)]
    fn multi_long_space_test(
        #[from(multi_long)] separator: FixIntervalSeparator,
        #[case] input: usize,
        #[case] output: Option<usize>,
    ) {
        let result = separator.try_compute_free_space(input);
        assert_eq!(result, output);
    }
}
