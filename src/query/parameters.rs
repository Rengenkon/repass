use core::str;

pub enum Strategy {
    Character,
    WordList,
}

pub trait SplitStrategy {
    fn add_spliterator(self: &Self, parts: Vec<&str>) -> String;
}

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
                splited += value;
                while let Some(value) = iter.next() {
                    splited += self.spliterator;
                    splited += value;
                }
            }
        }
        splited
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
