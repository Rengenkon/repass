use super::{IllegalArgumentError, SeparatorInternal, SeparatorStrategy};
use std::cmp::{Ordering, max, min};

#[derive(Debug, PartialEq)]
pub enum Align {
    Left,
    Right,
}

struct MetaInf {
    input_length: usize,
    separates_count: usize,
    parts_count: usize,
    chars_in_part: usize,
    chars_out_part: usize,
}

/// Separate parts with setuped count of separate segments.
/// Length of parts doesn't matter
///
/// # Note
/// `X` - setuped count of separate segments
/// `L` - summary length of given parts
///
/// If `L` less that `X + 1`
/// Then use only `L - 1` separate segment
///
/// If 'L' equals 0 then program panic
#[derive(Debug)]
pub struct FixCountSeparator<'a> {
    separator: &'a str,
    count: usize,
}

impl<'a> FixCountSeparator<'a> {
    pub fn new(separator: &'a str, count: usize) -> Self {
        Self { separator, count }
    }

    fn count_separates(self: &Self, length: usize) -> usize {
        max(0, min(length as isize - 1, self.count as isize)) as usize
    }

    fn get_meta_inf(self: &Self, parts: &[&str]) -> MetaInf {
        let input_length = Self::get_summary_length(parts);
        let separates_count = self.count_separates(input_length);
        let parts_count = separates_count + 1;
        let in_part = input_length / parts_count;
        let out_part = input_length % parts_count;
        MetaInf {
            input_length,
            separates_count,
            parts_count,
            chars_in_part: in_part,
            chars_out_part: out_part,
        }
    }

    fn align_parts_size(
        capacity: usize,
        out_part: usize,
        in_part: usize,
        align: Align,
    ) -> Vec<usize> {
        let mut sizes = Vec::with_capacity(capacity);
        for part in 0..capacity {
            if (align == Align::Left && part < out_part)
                || (align == Align::Right && part >= capacity - out_part)
            {
                sizes.push(in_part + 1);
            } else {
                sizes.push(in_part);
            }
        }
        sizes
    }

    fn half_parts_sizes(meta: &MetaInf, align: Align) -> Vec<usize> {
        if meta.parts_count == 1 || meta.chars_out_part == 1 {
            return Vec::new();
        }
        let half_chars_out_part = meta.chars_out_part / 2;
        let capacity = if meta.chars_in_part == 0 {
            half_chars_out_part
        } else {
            meta.parts_count / 2
        };
        Self::align_parts_size(capacity, half_chars_out_part, meta.chars_in_part, align)
    }

    fn middle_parts_sizes(meta: &MetaInf) -> Vec<usize> {
        let mut count = meta.chars_out_part % 2;
        if meta.parts_count % 2 == 1 {
            count += meta.chars_in_part;
        }
        if count == 0 {
            return Vec::new();
        }
        vec![count]
    }

    fn parts_sizes(meta: &MetaInf) -> Vec<usize> {
        if meta.chars_out_part % 2 == 1 && meta.parts_count % 2 == 0 {
            Self::align_parts_size(
                meta.parts_count,
                meta.chars_out_part,
                meta.chars_in_part,
                Align::Left,
            )
        } else {
            // todo invert if user want it
            let left_align = Align::Left;
            let right_align = Align::Right;

            let lmr_sizes = [
                Self::half_parts_sizes(meta, left_align),
                Self::middle_parts_sizes(meta),
                Self::half_parts_sizes(meta, right_align),
            ];
            let mut result = Vec::new();
            lmr_sizes
                .iter()
                .for_each(|sizes| result.extend_from_slice(sizes));
            result
        }
    }

    fn part_sizes_to_separators_indexes(offset: usize, sizes: &[usize]) -> Vec<usize> {
        let mut result = Vec::with_capacity(sizes.len());
        let mut value: isize = offset as isize - 1;
        for size in sizes {
            value += *size as isize + 1;
            result.push(value as usize);
        }
        result
    }

    fn separator_indexes(self: &Self, meta: &MetaInf) -> Vec<usize> {
        let parts_size = Self::parts_sizes(meta);
        Self::part_sizes_to_separators_indexes(0, &parts_size[0..meta.separates_count])
    }

    fn generic_assemble<'b>(self: &Self, separate_indexes: &[usize], raw_parts: &[&str]) -> String {
        let capacity = self.length_with_separators(raw_parts);
        let mut result = String::with_capacity(capacity);
        let mut indexes_iter = separate_indexes.iter();
        let mut parts_iter = raw_parts.iter();
        let separator = self.separator;

        let mut part = "";
        let mut separate_index = indexes_iter.next();
        let mut cmp_result = Ordering::Less;
        let mut current_write_index = 0;

        loop {
            if separate_index.is_none() {
                result.push_str(part);
                let op = parts_iter.next();
                if op.is_none() {
                    break;
                }
                part = op.unwrap();
            } else {
                if cmp_result != Ordering::Greater {
                    let op = parts_iter.next();
                    if op.is_some() {
                        part = op.unwrap();
                    } else {
                        result.push_str(separator);
                        while indexes_iter.next().is_some() {
                            result.push_str(separator);
                        }
                        break;
                    }
                }
                let diff = separate_index.unwrap() - current_write_index;
                cmp_result = part.len().cmp(&diff);
                if cmp_result == Ordering::Greater {
                    let (left, right) = part.split_at(diff);
                    part = right;
                    result.push_str(left);
                    current_write_index += left.len();
                    result.push_str(separator);
                    current_write_index += 1;
                    separate_index = indexes_iter.next();
                    continue;
                } else {
                    result.push_str(part);
                    current_write_index += part.len();
                }
            }
        }
        result
    }
}

impl SeparatorInternal for FixCountSeparator<'_> {
    fn add_separator(self: &Self, parts: &[&str]) -> String {
        let meta = self.get_meta_inf(parts);
        let separator_indexes = self.separator_indexes(&meta);
        self.generic_assemble(&separator_indexes, parts)
    }

    fn length_with_separators(self: &Self, parts: &[&str]) -> usize {
        let summary = Self::get_summary_length(parts);
        summary + self.count_separates(summary) * self.separator.len()
    }

    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError> {
        let mut errors = Vec::new();
        if self.separator.is_empty() {
            errors.push(IllegalArgumentError::EmptySeparator)
        }
        if self.count == 0 {
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

impl SeparatorStrategy for FixCountSeparator<'_> {}

#[cfg(test)]
mod tests {
    mod public_functional {
        use super::super::FixCountSeparator;
        use super::super::SeparatorStrategy;
        use rstest::{fixture, rstest};

        #[fixture]
        fn without_separator<'a>() -> FixCountSeparator<'a> {
            FixCountSeparator::new("", 10)
        }

        #[fixture]
        fn without_count<'a>() -> FixCountSeparator<'a> {
            FixCountSeparator::new("-", 0)
        }

        #[fixture]
        fn one_dash<'a>() -> FixCountSeparator<'a> {
            FixCountSeparator::new("-", 1)
        }

        #[fixture]
        fn one_long<'a>() -> FixCountSeparator<'a> {
            FixCountSeparator::new("biba", 1)
        }

        #[fixture]
        fn multi_long<'a>() -> FixCountSeparator<'a> {
            FixCountSeparator::new("aAa", 4)
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
            )] separator: FixCountSeparator,
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
            )] separator: FixCountSeparator,
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
            )] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
        ) {
            let result_len = separator.separate(&input).len();
            let compute_len = separator.length_after_separate(&input);
            assert_eq!(result_len, compute_len);
        }

        #[rstest]
        #[case(vec!["1",], "1")]
        #[case(vec!["biba",], "bi-ba")]
        #[case(vec!["1", "2",], "1-2")]
        #[case(vec!["1", "2", "3",], "12-3")]
        #[case(vec!["1", "2", "3", "4",], "12-34")]
        #[case(vec!["biba", "boba",], "biba-boba")]
        fn one_dash_test(
            #[from(one_dash)] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.separate(&input);
            assert_eq!(result, output);
        }

        #[rstest]
        #[case(vec!["1",], "1")]
        #[case(vec!["biba",], "bibibaba")]
        #[case(vec!["1", "2",], "1biba2")]
        #[case(vec!["1", "2", "3",], "12biba3")]
        #[case(vec!["1", "2", "3", "4",], "12biba34")]
        #[case(vec!["biba", "boba",], "bibabibaboba")]
        fn one_long_test(
            #[from(one_long)] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.separate(&input);
            assert_eq!(result, output);
        }

        #[rstest]
        #[case(vec!["1",], "1")]
        #[case(vec!["biba",], "baAaiaAabaAaa")]
        #[case(vec!["1", "2",], "1aAa2")]
        #[case(vec!["1", "2", "3",], "1aAa2aAa3")]
        #[case(vec!["1", "2", "3", "4",], "1aAa2aAa3aAa4")]
        #[case(vec!["biba", "boba",], "biaAabaAaabaAaoaAaba")]
        fn multi_long_test(
            #[from(multi_long)] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.separate(&input);
            assert_eq!(result, output);
        }
    }

    /// using for debug
    mod internal {
        use super::super::FixCountSeparator;

        #[test]
        fn len_3() {
            let s = FixCountSeparator::new("-", 1);
            let data = vec!["1", "2", "3",];
            let m = s.get_meta_inf(&data);
            let ind = s.separator_indexes(&m);
            assert_eq!(ind, vec![2])
        }
    }
}
