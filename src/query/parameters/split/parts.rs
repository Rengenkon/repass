use core::str;
use super::SplitStrategy;

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
    fn add_spliterator(self: &Self, parts: Vec<&str>) -> String {
        let mut splited = String::new();
        let mut iter = parts.iter();
        let part = iter.next();
        match part {
            None => {}
            Some(value) => {
                splited.push_str(value);
                while let Some(value) = iter.next() {
                    splited.push_str(self.spliterator);
                    splited.push_str(value);
                }
            }
        }
        splited
    }

    fn compute_length(self: &Self, parts: Vec<&str>) -> usize {
        if parts.is_empty() {
            return 0;
        }
        let base_length = parts.iter().map(|part| part.len()).sum::<usize>();
        base_length + self.spliterator.len() * (parts.len() - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::BetweenPartsSpliterator;
    use super::SplitStrategy;

    #[test]
    fn dash_split() {
        let spliterator = BetweenPartsSpliterator::new("-");

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "1-2");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "abo-ba");
    }

    #[test]
    fn empty_split() {
        let spliterator = BetweenPartsSpliterator::new("");

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "12");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "aboba");
    }

    #[test]
    fn long_split() {
        let spliterator = BetweenPartsSpliterator::new("biba");

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "1biba2");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "abobibaba");
    }
}