use super::SplitStrategy;
use core::str;

#[derive(Debug)]
pub struct BetweenPartsSpliterator<'a> {
    spliterator: &'a str,
}

impl<'a> BetweenPartsSpliterator<'a> {
    pub fn new(spliterator: &'a str) -> Self {
        Self { spliterator }
    }
}

impl SplitStrategy for BetweenPartsSpliterator<'_> {
    fn add_spliterator(self: &Self, parts: &[&str]) -> String {
        parts.join(self.spliterator)
    }

    fn compute_length(self: &Self, parts: &[&str]) -> usize {
        if parts.is_empty() {
            return 0;
        }
        let base_length = parts.iter().map(|part| part.len()).sum::<usize>();
        base_length + self.spliterator.len() * (parts.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::super::for_tests::size_equals_test;
    use super::BetweenPartsSpliterator;
    use super::SplitStrategy;

    #[test]
    fn dash_split() {
        let spliterator = BetweenPartsSpliterator::new("-");

        assert_eq!(spliterator.add_spliterator(&Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(&vec![""]), "");
        assert_eq!(spliterator.add_spliterator(&vec!["", ""]), "-");
        assert_eq!(spliterator.add_spliterator(&vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(&vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2"]), "1-2");
        assert_eq!(spliterator.add_spliterator(&vec!["abo", "ba"]), "abo-ba");
    }

    #[test]
    fn empty_split() {
        let spliterator = BetweenPartsSpliterator::new("");

        assert_eq!(spliterator.add_spliterator(&Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(&vec![""]), "");
        assert_eq!(spliterator.add_spliterator(&vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(&vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2"]), "12");
        assert_eq!(spliterator.add_spliterator(&vec!["abo", "ba"]), "aboba");
    }

    #[test]
    fn long_split() {
        let spliterator = BetweenPartsSpliterator::new("biba");

        assert_eq!(spliterator.add_spliterator(&Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(&vec![""]), "");
        assert_eq!(spliterator.add_spliterator(&vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(&vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(&vec!["1", "2"]), "1biba2");
        assert_eq!(spliterator.add_spliterator(&vec!["abo", "ba"]), "abobibaba");
    }

    #[test]
    fn size_dash_split() {
        let spliterator = BetweenPartsSpliterator::new("-");

        size_equals_test(&spliterator, &Vec::new());
        size_equals_test(&spliterator, &vec![""]);
        size_equals_test(&spliterator, &vec!["", ""]);
        size_equals_test(&spliterator, &vec!["1"]);
        size_equals_test(&spliterator, &vec!["aboba"]);
        size_equals_test(&spliterator, &vec!["1", "2"]);
        size_equals_test(&spliterator, &vec!["abo", "ba"]);
    }

    #[test]
    fn size_empty_split() {
        let spliterator = BetweenPartsSpliterator::new("");

        size_equals_test(&spliterator, &Vec::new());
        size_equals_test(&spliterator, &vec![""]);
        size_equals_test(&spliterator, &vec!["", ""]);
        size_equals_test(&spliterator, &vec!["1"]);
        size_equals_test(&spliterator, &vec!["aboba"]);
        size_equals_test(&spliterator, &vec!["1", "2"]);
        size_equals_test(&spliterator, &vec!["abo", "ba"]);
    }

    #[test]
    fn size_long_split() {
        let spliterator = BetweenPartsSpliterator::new("biba");

        size_equals_test(&spliterator, &Vec::new());
        size_equals_test(&spliterator, &vec![""]);
        size_equals_test(&spliterator, &vec!["", ""]);
        size_equals_test(&spliterator, &vec!["1"]);
        size_equals_test(&spliterator, &vec!["aboba"]);
        size_equals_test(&spliterator, &vec!["1", "2"]);
        size_equals_test(&spliterator, &vec!["abo", "ba"]);
    }
}
