use super::SplitStrategy;
use core::str;
use std::cmp::Ordering;

#[derive(Debug)]
pub struct FixIntervalSpliterator<'a> {
    spliterator: &'a str,
    length: usize,
}

impl<'a> FixIntervalSpliterator<'a> {
    pub fn new(spliterator: &'a str, length: usize) -> Self {
        let mut len = length;
        if length == 0 {
            len += 1;
        }
        Self {
            spliterator,
            length: len,
        }
    }
}

impl SplitStrategy for FixIntervalSpliterator<'_> {
    fn add_spliterator(self: &Self, parts: &[&str]) -> String {
        let mut splited = String::new();
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
                    splited.push_str(self.spliterator);
                    diff = self.length;
                }
            } else {
                splited.push_str(self.spliterator);
                diff = self.length;
            }
            cmp_result = value.len().cmp(&diff);
            match cmp_result {
                Ordering::Less | Ordering::Equal => {
                    splited.push_str(value);
                    diff -= value.len();
                }
                Ordering::Greater => {
                    let (left, right) = value.split_at(diff);
                    splited.push_str(left);
                    value = right;
                }
            }
        }
        splited
    }

    fn compute_length(self: &Self, parts: &[&str]) -> usize {
        let sum_length = parts.iter().map(|part| part.len()).sum::<usize>();
        let mut count_split = sum_length / self.length;
        if count_split == 0 {
            return 0;
        }
        count_split -= 1;
        if sum_length % self.length != 0 {
            count_split += 1;
        }
        sum_length + self.spliterator.len() * count_split
    }
}

#[cfg(test)]
mod tests {
    use super::super::for_tests::size_equals_test;
    use super::FixIntervalSpliterator;
    use super::SplitStrategy;

    #[test]
    fn size_test() {
        let spliterators = vec![
            FixIntervalSpliterator::new("-", 1),
            FixIntervalSpliterator::new("-", 0),
            FixIntervalSpliterator::new("", 1),
            FixIntervalSpliterator::new("biba", 1),
            FixIntervalSpliterator::new("aa", 3)
        ];
        let data = vec![
            Vec::new(),
            vec![""],
            vec!["1"],
            vec!["aboba"],
            vec!["1", "2"],
            vec!["abo", "ba"]
        ];
        for i in 0..spliterators.len() {
            for j in 0..data.len() {
                size_equals_test(&spliterators[i], &data[j]);
            }
        }
    }

    #[test]
    fn dash_split() {
        let spliterator = FixIntervalSpliterator::new("-", 1);

        assert_eq!(spliterator.add_spliterator(&Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(&vec![""]), "");
        assert_eq!(spliterator.add_spliterator(&vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(&vec!["aboba"]), "a-b-o-b-a");
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2"]), "1-2");
        assert_eq!(spliterator.add_spliterator(&vec!["abo", "ba"]), "a-b-o-b-a");
    }

    #[test]
    fn zero_length_split() {
        let spliterator = FixIntervalSpliterator::new("-", 0);

        assert_eq!(spliterator.add_spliterator(&Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(&vec![""]), "");
        assert_eq!(spliterator.add_spliterator(&vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(&vec!["aboba"]), "a-b-o-b-a");
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2"]), "1-2");
        assert_eq!(spliterator.add_spliterator(&vec!["abo", "ba"]), "a-b-o-b-a");
    }

    #[test]
    fn empty_split() {
        let spliterator = FixIntervalSpliterator::new("", 1);

        assert_eq!(spliterator.add_spliterator(&Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(&vec![""]), "");
        assert_eq!(spliterator.add_spliterator(&vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(&vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2"]), "12");
        assert_eq!(spliterator.add_spliterator(&vec!["abo", "ba"]), "aboba");
    }

    #[test]
    fn long_split() {
        let spliterator = FixIntervalSpliterator::new("biba", 1);

        assert_eq!(spliterator.add_spliterator(&Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(&vec![""]), "");
        assert_eq!(spliterator.add_spliterator(&vec!["1"]), "1");
        assert_eq!(
            spliterator.add_spliterator(&vec!["aboba"]),
            "abibabbibaobibabbibaa"
        );
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2"]), "1biba2");
        assert_eq!(
            spliterator.add_spliterator(&vec!["abo", "ba"]),
            "abibabbibaobibabbibaa"
        );
    }

    #[test]
    fn long_and_length_split() {
        let spliterator = FixIntervalSpliterator::new("aa", 3);

        assert_eq!(spliterator.add_spliterator(&Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(&vec![""]), "");
        assert_eq!(spliterator.add_spliterator(&vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2"]), "12");
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2", "3"]), "123");
        assert_eq!(
            spliterator.add_spliterator(&vec!["1", "2", "3", "4"]),
            "123aa4"
        );
        assert_eq!(spliterator.add_spliterator(&vec!["", "1234"]), "123aa4");
    }
}
