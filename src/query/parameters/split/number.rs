use super::SplitStrategy;
use std::cmp::{Ordering, max, min};

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
        max(0, min(length - 1, self.count))
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
    /// - The last index is strictly less than the sum of lengths of all `raw_parts`.
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
                let diff = split_index.unwrap() - current_write_index;
                if cmp_result == Ordering::Less || cmp_result == Ordering::Equal {
                    part = raw_parts.next().unwrap();
                }
                if cmp_result == Ordering::Greater || cmp_result == Ordering::Equal {
                    result.push_str(separator);
                    current_write_index += 1;
                }
                cmp_result = part.len().cmp(&diff);
                let insert = if cmp_result == Ordering::Greater {
                    let (left, right) = part.split_at(diff);
                    part = right;
                    left
                } else {
                    part
                };
                result.push_str(insert);
                current_write_index += insert.len();
                if diff == 0 {
                    split_index = split_indexes.next();
                }
            }
        }
    }

    fn separator_positions() -> Vec<usize> {
        // match (count_undistributed_charts, count_parts) {
        //     (u, c) if u % 2 == 0 && c % 2 == 0 => {
        //         // 2, 4
        //     }
        //     (r, c) if r % 2 == 0 && c % 2 == 1 => {
        //         // 2, 5
        //     }
        //     (r, c) if r % 2 == 1 && c % 2 == 0 => {
        //         // 3, 4
        //     }
        //     (r, c) if r % 2 == 1 && c % 2 == 1 => {
        //         // 3, 5
        //     }
        //     (_, _) => Vec::new(),
        // }
        todo!()
    }
}

impl<'a> SplitStrategy for FixCountSpliterator<'a> {
    fn add_spliterator(self: &Self, parts: &[&str]) -> String {
        let length = Self::char_length_for_parts(parts);
        let count_parts = self.count_splits(length) + 1;
        let count_undistributed_charts = length % (count_parts);
        let parts_size = Self::separator_positions();
        Self::assemble_with_capacity(
            &parts_size,
            &self.spliterator,
            parts,
            self.compute_length(parts),
        )
    }

    fn compute_length(self: &Self, parts: &[&str]) -> usize {
        if parts.is_empty() {
            return 0;
        }
        let length = Self::char_length_for_parts(parts);
        length + self.count_splits(length) * self.spliterator.len()
    }
}

mod tests {
    use super::super::for_tests::*;
    use super::FixCountSpliterator;
    use std::vec;

    #[test]
    fn assemble_test() {}

    fn get_spliterators<'a>() -> Vec<FixCountSpliterator<'a>> {
        vec![
            FixCountSpliterator::new("-", 1),
            FixCountSpliterator::new("-", 0),
            FixCountSpliterator::new("", 1),
            FixCountSpliterator::new("biba", 1),
            FixCountSpliterator::new("aa", 3),
        ]
    }

    #[test]
    fn length_test() {
        let spliterator = get_spliterators();
        size_equal_string(&spliterator, &get_test_data());
    }

    #[test]
    fn content_test() {
        let sp = get_spliterators();
        let spliterator = sp.get(0).unwrap();
        let given = get_test_data();
        let when = vec!["", "", "1", "biba", "", "12", "123", "1234", "bibaboba"];
        equals_string(spliterator, &given, &when);
    }
}
