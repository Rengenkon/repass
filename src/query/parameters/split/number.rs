use super::SplitStrategy;
use enum_display::EnumDisplay;
use std::cmp::{Ordering, max, min};

/// Separate parts with setuped count of separate segments
/// Length of parts doesn't matter
///
/// `X` - setuped count of separate segments
/// `L` - summary length of given parts
///
/// If `L` less that `X + 1`
/// Then use only `L - 1` separate segment
///
/// If 'L' equals 0 then program panic
#[derive(Debug)]
pub struct FixCountSpliterator<'a> {
    spliterator: &'a str,
    count: usize,
}

impl<'a> FixCountSpliterator<'a> {
    pub fn new(spliterator: &'a str, count: usize) -> Self {
        Self { spliterator, count }
    }

    fn count_splits(self: &Self, length: usize) -> usize {
        max(0, min(length as isize - 1, self.count as isize)) as usize
    }

    fn assemble_with_capacity(
        split_indexes: &[usize],
        spliterator: &str,
        raw_parts: &[&str],
        finish_size: usize,
    ) -> String {
        let mut splited = String::with_capacity(finish_size);
        FixCountSpliterator::generic_assemble(
            &mut splited,
            split_indexes.iter().copied(),
            spliterator,
            raw_parts.iter().copied(),
        );
        splited
    }

    fn assemble(split_indexes: &[usize], spliterator: &str, raw_parts: &[&str]) -> String {
        let mut splited = String::new();
        FixCountSpliterator::generic_assemble(
            &mut splited,
            split_indexes.iter().copied(),
            spliterator,
            raw_parts.iter().copied(),
        );
        splited
    }

    /// Appends assembled parts to `result`, inserting `separator` at positions
    /// defined by `split_indexes`.
    ///
    /// The `split_indexes` must satisfy:
    /// - All indexes are unique.
    /// - They are sorted in ascending order.
    fn generic_assemble<'b>(
        result: &mut String,
        mut split_indexes: impl Iterator<Item = usize>,
        separator: &str,
        mut raw_parts: impl Iterator<Item = &'b str>,
    ) {
        let mut part = "";
        let mut split_index = split_indexes.next();
        let mut cmp_result = Ordering::Less;
        let mut current_write_index = 0;

        loop {
            if split_index.is_none() {
                result.push_str(part);
                let op = raw_parts.next();
                if op.is_none() {
                    break;
                }
                part = op.unwrap();
            } else {
                if cmp_result != Ordering::Greater {
                    let op = raw_parts.next();
                    if op.is_some() {
                        part = op.unwrap();
                    } else {
                        result.push_str(separator);
                        while split_indexes.next().is_some() {
                            result.push_str(separator);
                        }
                        break;
                    }
                }
                let diff = split_index.unwrap() - current_write_index;
                cmp_result = part.len().cmp(&diff);
                if cmp_result == Ordering::Greater {
                    let (left, right) = part.split_at(diff);
                    part = right;
                    result.push_str(left);
                    current_write_index += left.len();
                    result.push_str(separator);
                    current_write_index += 1;
                    split_index = split_indexes.next();
                    continue;
                } else {
                    result.push_str(part);
                    current_write_index += part.len();
                }
            }
        }
    }

    fn separator_indexes(self: &Self, parts: &[&str]) -> Result<Vec<usize>, Errors> {
        let length = Self::char_length_for_parts(parts);
        let count_splits = self.count_splits(length);
        let parts_size = Self::parts_sizes(length, count_splits + 1)?;
        Ok(Self::part_sizes_to_separators_indexes(
            0,
            &parts_size[0..count_splits],
        ))
    }
    fn parts_sizes(length: usize, parts_count: usize) -> Result<Vec<usize>, Errors> {
        let undistributed_count_charts = length % parts_count;
        let chars_in_part = length / parts_count;

        if undistributed_count_charts & 2 == 1 && parts_count % 2 == 0 {
            Self::half_parts_sizes(
                parts_count,
                chars_in_part,
                undistributed_count_charts,
                Align::Left,
            )
        } else {
            // todo invert if user want it
            let left_align = Align::Left;
            let right_align = Align::Right;

            let lmr_sizes = [
                Self::half_parts_sizes(
                    parts_count,
                    chars_in_part,
                    undistributed_count_charts,
                    left_align,
                ),
                Self::middle_parts_sizes(parts_count, chars_in_part, undistributed_count_charts),
                Self::half_parts_sizes(
                    parts_count,
                    chars_in_part,
                    undistributed_count_charts,
                    right_align,
                ),
            ];
            let mut result = Vec::new();
            let mut err_msg = Vec::new();
            for s in lmr_sizes {
                match s {
                    Ok(sizes) => result.extend(sizes),
                    Err(error) => {
                        err_msg.push(error);
                    }
                }
            }
            if result.is_empty() {
                return Err(Errors::AllPartsIsIllegal(err_msg));
            }
            Ok(result)
        }
    }

    fn validate_undistributed(
        parts_count: usize,
        undistributed_count: usize,
    ) -> Result<(), Errors> {
        if undistributed_count >= parts_count {
            return Err(Errors::UndistributedGreaterOrEqualsParts);
        }
        Ok(())
    }

    fn half_parts_sizes(
        parts_count: usize,
        part_size: usize,
        undistributed_count: usize,
        align: Align,
    ) -> Result<Vec<usize>, Errors> {
        if parts_count == 1 || undistributed_count == 1 {
            return Err(Errors::NoPaste);
        }
        Self::validate_undistributed(parts_count, undistributed_count)?;
        let half_undistributed_count = undistributed_count / 2;
        let capacity = if part_size == 0 {
            half_undistributed_count
        } else {
            parts_count / 2
        };
        let mut sizes = Vec::with_capacity(capacity);
        for part in 0..capacity {
            if (align == Align::Left && part < half_undistributed_count)
                || (align == Align::Right && part >= capacity - half_undistributed_count)
            {
                sizes.push(part_size + 1);
            } else {
                sizes.push(part_size);
            }
        }
        Ok(sizes)
    }

    fn middle_parts_sizes(
        parts_count: usize,
        part_size: usize,
        undistributed_count: usize,
    ) -> Result<Vec<usize>, Errors> {
        Self::validate_undistributed(parts_count, undistributed_count)?;
        let mut count = undistributed_count % 2;
        if parts_count % 2 == 1 {
            count += part_size;
        }
        if count == 0 {
            return Err(Errors::NoPaste);
        }
        Ok(vec![count])
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
}

#[derive(Debug, PartialEq)]
pub enum Align {
    Left,
    Right,
}

#[derive(Debug, PartialEq, EnumDisplay)]
pub enum Errors {
    #[display("")]
    NoPaste,
    #[display("")]
    UndistributedGreaterOrEqualsParts,
    #[display("")]
    AllPartsIsIllegal(Vec<Errors>),
}

impl<'a> SplitStrategy for FixCountSpliterator<'a> {
    fn add_spliterator(self: &Self, parts: &[&str]) -> String {
        let separator_indexes = self.separator_indexes(parts);
        match separator_indexes {
            Ok(separator_indexes) => Self::assemble_with_capacity(
                &separator_indexes,
                &self.spliterator,
                parts,
                self.compute_length(parts),
            ),
            Err(e) => {
                // panic!(e.to_string().as_str());
                panic!("1");
                todo!()
            }
        }
    }

    fn compute_length(self: &Self, parts: &[&str]) -> usize {
        if parts.is_empty() {
            return 0;
        }
        let length = Self::char_length_for_parts(parts);
        length + self.count_splits(length) * self.spliterator.len()
    }
}

#[cfg(test)]
mod tests {
    pub use super::super::for_tests::*;
    pub use super::*;
    pub use std::vec;

    mod parts_sizes {
        pub use super::{Align, Errors, FixCountSpliterator};
        mod validate {
            use super::{Errors, FixCountSpliterator};

            #[test]
            fn nulls() {
                let correct = Errors::UndistributedGreaterOrEqualsParts;
                let sizes = FixCountSpliterator::validate_undistributed(0, 0)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn right() {
                let correct = ();
                let sizes = FixCountSpliterator::validate_undistributed(1, 0).unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn ones() {
                let correct = Errors::UndistributedGreaterOrEqualsParts;
                let sizes = FixCountSpliterator::validate_undistributed(0, 1)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }
        }

        mod half {
            use super::*;

            #[test]
            fn nulls() {
                let correct = Errors::UndistributedGreaterOrEqualsParts;
                let sizes = FixCountSpliterator::half_parts_sizes(0, 0, 0, Align::Left)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_7_0_1() {
                let correct = Errors::NoPaste;
                let sizes = FixCountSpliterator::half_parts_sizes(7, 0, 1, Align::Left)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_7_0_2() {
                let correct = vec![1];
                let sizes = FixCountSpliterator::half_parts_sizes(7, 0, 2, Align::Left).unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_7_0_6() {
                let correct = vec![1, 1, 1];
                let sizes = FixCountSpliterator::half_parts_sizes(7, 0, 6, Align::Left).unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_7_0_8_err() {
                let correct = Errors::UndistributedGreaterOrEqualsParts;
                let sizes = FixCountSpliterator::half_parts_sizes(7, 0, 7, Align::Left)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_7_1_2_left() {
                let correct = vec![2, 1, 1];
                let sizes = FixCountSpliterator::half_parts_sizes(7, 1, 2, Align::Left).unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_7_1_2_right() {
                let correct = vec![1, 1, 2];
                let sizes = FixCountSpliterator::half_parts_sizes(7, 1, 2, Align::Right).unwrap();
                assert_eq!(sizes, correct);
            }
        }

        mod middle {
            use super::{Errors, FixCountSpliterator};

            #[test]
            fn nulls() {
                let correct = Errors::UndistributedGreaterOrEqualsParts;
                let sizes = FixCountSpliterator::middle_parts_sizes(0, 0, 0)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn empty() {
                let correct = Errors::NoPaste;
                let sizes = FixCountSpliterator::middle_parts_sizes(1, 0, 0)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_1_0_1() {
                let correct = Errors::UndistributedGreaterOrEqualsParts;
                let sizes = FixCountSpliterator::middle_parts_sizes(1, 0, 1)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_1_1_0() {
                let correct = vec![1];
                let sizes = FixCountSpliterator::middle_parts_sizes(1, 1, 0).unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_1_1_1() {
                let correct = Errors::UndistributedGreaterOrEqualsParts;
                let sizes = FixCountSpliterator::middle_parts_sizes(1, 1, 1)
                    .err()
                    .unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_3_1_1() {
                let correct = vec![2];
                let sizes = FixCountSpliterator::middle_parts_sizes(3, 1, 1).unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_12_10_11() {
                let correct = vec![1];
                let sizes = FixCountSpliterator::middle_parts_sizes(12, 10, 11).unwrap();
                assert_eq!(sizes, correct);
            }

            #[test]
            fn d_13_10_11() {
                let correct = vec![11];
                let sizes = FixCountSpliterator::middle_parts_sizes(13, 10, 11).unwrap();
                assert_eq!(sizes, correct);
            }
        }
    }

    mod size_to_index {
        use super::FixCountSpliterator;

        #[test]
        fn nulls() {
            let given = vec![];
            let correct = vec![];
            let sizes = FixCountSpliterator::part_sizes_to_separators_indexes(0, &given);
            assert_eq!(sizes, correct);
        }

        #[test]
        fn zero() {
            let given = vec![0];
            let correct = vec![0];
            let sizes = FixCountSpliterator::part_sizes_to_separators_indexes(0, &given);
            assert_eq!(sizes, correct);
        }

        #[test]
        fn offset() {
            let given = vec![0];
            let correct = vec![10];
            let sizes = FixCountSpliterator::part_sizes_to_separators_indexes(10, &given);
            assert_eq!(sizes, correct);
        }

        #[test]
        fn base() {
            let given = vec![1, 1, 1];
            let correct = vec![1, 3, 5];
            let sizes = FixCountSpliterator::part_sizes_to_separators_indexes(0, &given);
            assert_eq!(sizes, correct);
        }

        #[test]
        fn base_dif() {
            let given = vec![1, 2, 3];
            let correct = vec![1, 4, 8];
            let sizes = FixCountSpliterator::part_sizes_to_separators_indexes(0, &given);
            assert_eq!(sizes, correct);
        }

        #[test]
        fn base_dif_off() {
            let given = vec![1, 2, 3];
            let correct = vec![11, 14, 18];
            let sizes = FixCountSpliterator::part_sizes_to_separators_indexes(10, &given);
            assert_eq!(sizes, correct);
        }
    }

    mod assembling {
        use super::FixCountSpliterator;

        #[test]
        fn assemble_test_empty() {
            let mut result = String::new();
            let indexes = vec![];
            let separator = "";
            let parts = vec![];
            FixCountSpliterator::generic_assemble(
                &mut result,
                indexes.iter().copied(),
                &separator,
                parts.iter().copied(),
            );
            assert_eq!(result, "")
        }

        #[test]
        fn assemble_test_12() {
            let mut result = String::new();
            let indexes = vec![];
            let separator = "";
            let parts = vec!["1", "2"];
            FixCountSpliterator::generic_assemble(
                &mut result,
                indexes.iter().copied(),
                &separator,
                parts.iter().copied(),
            );
            assert_eq!(result, "12")
        }

        #[test]
        fn assemble_test_1_2() {
            let mut result = String::new();
            let indexes = vec![1];
            let separator = "_";
            let parts = vec!["1", "2"];
            FixCountSpliterator::generic_assemble(
                &mut result,
                indexes.iter().copied(),
                &separator,
                parts.iter().copied(),
            );
            assert_eq!(result, "1_2")
        }

        #[test]
        fn assemble_test___1_2() {
            let mut result = String::new();
            let indexes = vec![0, 1, 3];
            let separator = "_";
            let parts = vec!["1", "2"];
            FixCountSpliterator::generic_assemble(
                &mut result,
                indexes.iter().copied(),
                &separator,
                parts.iter().copied(),
            );
            assert_eq!(result, "__1_2")
        }

        #[test]
        fn assemble_test_12_3() {
            let mut result = String::new();
            let indexes = vec![2];
            let separator = "_";
            let parts = vec!["1", "2", "3"];
            FixCountSpliterator::generic_assemble(
                &mut result,
                indexes.iter().copied(),
                &separator,
                parts.iter().copied(),
            );
            assert_eq!(result, "12_3")
        }

        #[test]
        fn assemble_test_1_2_3_with_wrapping() {
            let mut result = String::new();
            let indexes = vec![0, 1, 2, 4, 6, 8, 9, 10];
            let separator = "_";
            let parts = vec!["1", "2", "3"];
            FixCountSpliterator::generic_assemble(
                &mut result,
                indexes.iter().copied(),
                &separator,
                parts.iter().copied(),
            );
            assert_eq!(result, "___1_2_3___")
        }

        #[test]
        fn assemble_test__1_23() {
            let mut result = String::new();
            let indexes = vec![0, 2];
            let separator = "_";
            let parts = vec!["123"];
            FixCountSpliterator::generic_assemble(
                &mut result,
                indexes.iter().copied(),
                &separator,
                parts.iter().copied(),
            );
            assert_eq!(result, "_1_23")
        }

        #[test]
        fn assemble_test__1__2_3456_7__8__9() {
            let mut result = String::new();
            let indexes = vec![0, 2, 3, 5, 10, 12, 13, 15, 16];
            let separator = "_";
            let parts = vec!["", "12", "", "3", "45", "", "678", "", "9"];
            FixCountSpliterator::generic_assemble(
                &mut result,
                indexes.iter().copied(),
                &separator,
                parts.iter().copied(),
            );
            assert_eq!(result, "_1__2_3456_7__8__9")
        }
    }

    mod public_functional {
        use super::{FixCountSpliterator, SplitStrategy};
        use rstest::{fixture, rstest};

        #[fixture]
        fn without_separator<'a>() -> FixCountSpliterator<'a> {
            FixCountSpliterator::new("", 10)
        }

        #[fixture]
        fn without_count<'a>() -> FixCountSpliterator<'a> {
            FixCountSpliterator::new("-", 0)
        }

        #[fixture]
        fn one_dash<'a>() -> FixCountSpliterator<'a> {
            FixCountSpliterator::new("-", 1)
        }

        #[fixture]
        fn one_long<'a>() -> FixCountSpliterator<'a> {
            FixCountSpliterator::new("biba", 1)
        }

        #[fixture]
        fn multi_long<'a>() -> FixCountSpliterator<'a> {
            FixCountSpliterator::new("aAa", 4)
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
        fn without_separator_test(
            #[from(without_separator)] separator: FixCountSpliterator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.add_spliterator(&input);
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
        fn without_count_test(
            #[from(without_count)] separator: FixCountSpliterator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.add_spliterator(&input);
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
        fn one_dash_test(
            #[from(one_dash)] separator: FixCountSpliterator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.add_spliterator(&input);
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
        fn one_long_test(
            #[from(one_long)] separator: FixCountSpliterator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.add_spliterator(&input);
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
        fn multi_long_test(
            #[from(multi_long)] separator: FixCountSpliterator,
            #[case] input: Vec<&str>,
            #[case] output: &str,
        ) {
            let result = separator.add_spliterator(&input);
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
            #[values(one_dash(), one_long(), multi_long())] separator: FixCountSpliterator,
            #[case] input: Vec<&str>,
        ) {
            let result_len = separator.add_spliterator(&input).len();
            let compute_len = separator.compute_length(&input);
            assert_eq!(result_len, compute_len);
        }
    }
}
