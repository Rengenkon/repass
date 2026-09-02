use super::{IllegalArgumentError, SeparatorStrategy, SeparatorInternal};
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
    pub use super::super::*;
    pub use super::*;
    pub use std::vec;

    // mod parts_sizes {
    //     pub use super::{Align, FixCountSeparator};
    //     mod validate {
    //         use super::{FixCountSeparator};
    //
    //         #[test]
    //         fn nulls() {
    //             let correct = Errors::UndistributedGreaterOrEqualsParts;
    //             let sizes = FixCountSeparator::validate_undistributed(0, 0)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn right() {
    //             let correct = ();
    //             let sizes = FixCountSeparator::validate_undistributed(1, 0).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn ones() {
    //             let correct = Errors::UndistributedGreaterOrEqualsParts;
    //             let sizes = FixCountSeparator::validate_undistributed(0, 1)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //     }
    //
    //     mod half {
    //         use super::*;
    //
    //         #[test]
    //         fn nulls() {
    //             let correct = Errors::UndistributedGreaterOrEqualsParts;
    //             let sizes = FixCountSeparator::half_parts_sizes(0, 0, 0, Align::Left)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_7_0_1() {
    //             let correct = Errors::NoPaste;
    //             let sizes = FixCountSeparator::half_parts_sizes(7, 0, 1, Align::Left)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_7_0_2() {
    //             let correct = vec![1];
    //             let sizes = FixCountSeparator::half_parts_sizes(7, 0, 2, Align::Left).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_7_0_6() {
    //             let correct = vec![1, 1, 1];
    //             let sizes = FixCountSeparator::half_parts_sizes(7, 0, 6, Align::Left).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_7_0_8_err() {
    //             let correct = Errors::UndistributedGreaterOrEqualsParts;
    //             let sizes = FixCountSeparator::half_parts_sizes(7, 0, 7, Align::Left)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_7_1_2_left() {
    //             let correct = vec![2, 1, 1];
    //             let sizes = FixCountSeparator::half_parts_sizes(7, 1, 2, Align::Left).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_7_1_2_right() {
    //             let correct = vec![1, 1, 2];
    //             let sizes = FixCountSeparator::half_parts_sizes(7, 1, 2, Align::Right).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //     }
    //
    //     mod middle {
    //         use super::{Errors, FixCountSeparator};
    //
    //         #[test]
    //         fn nulls() {
    //             let correct = Errors::UndistributedGreaterOrEqualsParts;
    //             let sizes = FixCountSeparator::middle_parts_sizes(0, 0, 0)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn empty() {
    //             let correct = Errors::NoPaste;
    //             let sizes = FixCountSeparator::middle_parts_sizes(1, 0, 0)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_1_0_1() {
    //             let correct = Errors::UndistributedGreaterOrEqualsParts;
    //             let sizes = FixCountSeparator::middle_parts_sizes(1, 0, 1)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_1_1_0() {
    //             let correct = vec![1];
    //             let sizes = FixCountSeparator::middle_parts_sizes(1, 1, 0).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_1_1_1() {
    //             let correct = Errors::UndistributedGreaterOrEqualsParts;
    //             let sizes = FixCountSeparator::middle_parts_sizes(1, 1, 1)
    //                 .err()
    //                 .unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_3_1_1() {
    //             let correct = vec![2];
    //             let sizes = FixCountSeparator::middle_parts_sizes(3, 1, 1).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_12_10_11() {
    //             let correct = vec![1];
    //             let sizes = FixCountSeparator::middle_parts_sizes(12, 10, 11).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //
    //         #[test]
    //         fn d_13_10_11() {
    //             let correct = vec![11];
    //             let sizes = FixCountSeparator::middle_parts_sizes(13, 10, 11).unwrap();
    //             assert_eq!(sizes, correct);
    //         }
    //     }
    // }
    //
    // mod size_to_index {
    //     use super::FixCountSeparator;
    //
    //     #[test]
    //     fn nulls() {
    //         let given = vec![];
    //         let correct = vec![];
    //         let sizes = FixCountSeparator::part_sizes_to_separators_indexes(0, &given);
    //         assert_eq!(sizes, correct);
    //     }
    //
    //     #[test]
    //     fn zero() {
    //         let given = vec![0];
    //         let correct = vec![0];
    //         let sizes = FixCountSeparator::part_sizes_to_separators_indexes(0, &given);
    //         assert_eq!(sizes, correct);
    //     }
    //
    //     #[test]
    //     fn offset() {
    //         let given = vec![0];
    //         let correct = vec![10];
    //         let sizes = FixCountSeparator::part_sizes_to_separators_indexes(10, &given);
    //         assert_eq!(sizes, correct);
    //     }
    //
    //     #[test]
    //     fn base() {
    //         let given = vec![1, 1, 1];
    //         let correct = vec![1, 3, 5];
    //         let sizes = FixCountSeparator::part_sizes_to_separators_indexes(0, &given);
    //         assert_eq!(sizes, correct);
    //     }
    //
    //     #[test]
    //     fn base_dif() {
    //         let given = vec![1, 2, 3];
    //         let correct = vec![1, 4, 8];
    //         let sizes = FixCountSeparator::part_sizes_to_separators_indexes(0, &given);
    //         assert_eq!(sizes, correct);
    //     }
    //
    //     #[test]
    //     fn base_dif_off() {
    //         let given = vec![1, 2, 3];
    //         let correct = vec![11, 14, 18];
    //         let sizes = FixCountSeparator::part_sizes_to_separators_indexes(10, &given);
    //         assert_eq!(sizes, correct);
    //     }
    // }
    //
    // mod assembling {
    //     use super::FixCountSeparator;
    //
    //     #[test]
    //     fn assemble_test_empty() {
    //         let mut result = String::new();
    //         let indexes = vec![];
    //         let separator = "";
    //         let parts = vec![];
    //         FixCountSeparator::generic_assemble(
    //             &mut result,
    //             indexes.iter().copied(),
    //             &separator,
    //             parts.iter().copied(),
    //         );
    //         assert_eq!(result, "")
    //     }
    //
    //     #[test]
    //     fn assemble_test_12() {
    //         let mut result = String::new();
    //         let indexes = vec![];
    //         let separator = "";
    //         let parts = vec!["1", "2"];
    //         FixCountSeparator::generic_assemble(
    //             &mut result,
    //             indexes.iter().copied(),
    //             &separator,
    //             parts.iter().copied(),
    //         );
    //         assert_eq!(result, "12")
    //     }
    //
    //     #[test]
    //     fn assemble_test_1_2() {
    //         let mut result = String::new();
    //         let indexes = vec![1];
    //         let separator = "_";
    //         let parts = vec!["1", "2"];
    //         FixCountSeparator::generic_assemble(
    //             &mut result,
    //             indexes.iter().copied(),
    //             &separator,
    //             parts.iter().copied(),
    //         );
    //         assert_eq!(result, "1_2")
    //     }
    //
    //     #[test]
    //     fn assemble_test___1_2() {
    //         let mut result = String::new();
    //         let indexes = vec![0, 1, 3];
    //         let separator = "_";
    //         let parts = vec!["1", "2"];
    //         FixCountSeparator::generic_assemble(
    //             &mut result,
    //             indexes.iter().copied(),
    //             &separator,
    //             parts.iter().copied(),
    //         );
    //         assert_eq!(result, "__1_2")
    //     }
    //
    //     #[test]
    //     fn assemble_test_12_3() {
    //         let mut result = String::new();
    //         let indexes = vec![2];
    //         let separator = "_";
    //         let parts = vec!["1", "2", "3"];
    //         FixCountSeparator::generic_assemble(
    //             &mut result,
    //             indexes.iter().copied(),
    //             &separator,
    //             parts.iter().copied(),
    //         );
    //         assert_eq!(result, "12_3")
    //     }
    //
    //     #[test]
    //     fn assemble_test_1_2_3_with_wrapping() {
    //         let mut result = String::new();
    //         let indexes = vec![0, 1, 2, 4, 6, 8, 9, 10];
    //         let separator = "_";
    //         let parts = vec!["1", "2", "3"];
    //         FixCountSeparator::generic_assemble(
    //             &mut result,
    //             indexes.iter().copied(),
    //             &separator,
    //             parts.iter().copied(),
    //         );
    //         assert_eq!(result, "___1_2_3___")
    //     }
    //
    //     #[test]
    //     fn assemble_test__1_23() {
    //         let mut result = String::new();
    //         let indexes = vec![0, 2];
    //         let separator = "_";
    //         let parts = vec!["123"];
    //         FixCountSeparator::generic_assemble(
    //             &mut result,
    //             indexes.iter().copied(),
    //             &separator,
    //             parts.iter().copied(),
    //         );
    //         assert_eq!(result, "_1_23")
    //     }
    //
    //     #[test]
    //     fn assemble_test__1__2_3456_7__8__9() {
    //         let mut result = String::new();
    //         let indexes = vec![0, 2, 3, 5, 10, 12, 13, 15, 16];
    //         let separator = "_";
    //         let parts = vec!["", "12", "", "3", "45", "", "678", "", "9"];
    //         FixCountSeparator::generic_assemble(
    //             &mut result,
    //             indexes.iter().copied(),
    //             &separator,
    //             parts.iter().copied(),
    //         );
    //         assert_eq!(result, "_1__2_3456_7__8__9")
    //     }
    // }

    mod public_functional {
        use super::{FixCountSeparator, SeparatorStrategy};
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
        #[case(Vec::new(), "")]
        #[case(vec![""], "")]
        #[case(vec!["1",], "")]
        #[case(vec!["biba",], "")]
        #[case(vec!["", "",], "")]
        #[case(vec!["1", "2",], "")]
        #[case(vec!["1", "2", "3",], "")]
        #[case(vec!["1", "2", "3", "4",], "")]
        #[case(vec!["biba", "boba",], "")]
        #[should_panic]
        fn without_separator_test(
            #[from(without_separator)] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.separate(&input);
            assert_eq!(result, output);
        }

        #[rstest]
        #[case(Vec::new(), "")]
        #[case(vec![""], "")]
        #[case(vec!["1",], "")]
        #[case(vec!["biba",], "")]
        #[case(vec!["", "",], "")]
        #[case(vec!["1", "2",], "")]
        #[case(vec!["1", "2", "3",], "")]
        #[case(vec!["1", "2", "3", "4",], "")]
        #[case(vec!["biba", "boba",], "")]
        #[should_panic]
        fn without_count_test(
            #[from(without_count)] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.separate(&input);
            assert_eq!(result, output);
        }

        #[rstest]
        #[should_panic]
        #[case(Vec::new(), "")]
        #[should_panic]
        #[case(vec![""], "")]
        #[case(vec!["1",], "")]
        #[case(vec!["biba",], "")]
        #[case(vec!["", "",], "")]
        #[case(vec!["1", "2",], "")]
        #[case(vec!["1", "2", "3",], "")]
        #[case(vec!["1", "2", "3", "4",], "")]
        #[case(vec!["biba", "boba",], "")]
        fn one_dash_test(
            #[from(one_dash)] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.separate(&input);
            assert_eq!(result, output);
        }

        #[rstest]
        #[should_panic]
        #[case(Vec::new(), "")]
        #[should_panic]
        #[case(vec![""], "")]
        #[case(vec!["1",], "")]
        #[case(vec!["biba",], "")]
        #[case(vec!["", "",], "")]
        #[case(vec!["1", "2",], "")]
        #[case(vec!["1", "2", "3",], "")]
        #[case(vec!["1", "2", "3", "4",], "")]
        #[case(vec!["biba", "boba",], "")]
        fn one_long_test(
            #[from(one_long)] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.separate(&input);
            assert_eq!(result, output);
        }

        #[rstest]
        #[should_panic]
        #[case(Vec::new(), "")]
        #[should_panic]
        #[case(vec![""], "")]
        #[case(vec!["1",], "")]
        #[case(vec!["biba",], "")]
        #[case(vec!["", "",], "")]
        #[case(vec!["1", "2",], "")]
        #[case(vec!["1", "2", "3",], "")]
        #[case(vec!["1", "2", "3", "4",], "")]
        #[case(vec!["biba", "boba",], "")]
        fn multi_long_test(
            #[from(multi_long)] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.separate(&input);
            assert_eq!(result, output);
        }

        #[rstest]
        #[case(Vec::new())]
        #[case(vec![""])]
        #[case(vec!["1",])]
        #[case(vec!["biba",])]
        #[case(vec!["", "",])]
        #[case(vec!["1", "2",])]
        #[case(vec!["1", "2", "3",])]
        #[case(vec!["1", "2", "3", "4",])]
        #[case(vec!["biba", "boba",])]
        fn length_test(
            #[values(one_dash(), one_long(), multi_long())] separator: FixCountSeparator,
            #[case] input: Vec<&str>,
        ) {
            let result_len = separator.separate(&input).len();
            let compute_len = separator.length_after_separate(&input);
            assert_eq!(result_len, compute_len);
        }
    }
}
