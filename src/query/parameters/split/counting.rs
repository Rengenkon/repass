use super::SplitStrategy;
use core::str;
use std::cmp::Ordering;

#[derive(Debug)]
pub struct CountingSpliterator<'a> {
    spliterator: &'a str,
    length: usize,
}

impl<'a> CountingSpliterator<'a> {
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

impl SplitStrategy for CountingSpliterator<'_> {
    fn add_spliterator(self: &Self, parts: Vec<&str>) -> String {
        let mut splited = String::new();
        let mut iter = parts.iter();
        let mut current_len = 0;


        for part in parts {
            let diff = self.length - current_len;
            match part.len().cmp(&diff) {
                Ordering::Less => {
                    splited.push_str(part);
                    current_len += part.len();
                }
                Ordering::Equal => {
                    splited.push_str(part);
                    splited.push_str(self.spliterator);
                    current_len = 0;
                }
                Ordering::Greater => {
                    let (left, right) = part.split_at(diff);
                    splited.push_str(left);
                    splited.push_str(self.spliterator);
                    splited.push_str(right);
                    current_len = right.len();
                }
            }
        }

        splited
    }
}

#[cfg(test)]
mod tests {
    use super::CountingSpliterator;
    use super::SplitStrategy;

    #[test]
    fn dash_split() {
        let spliterator = CountingSpliterator::new("-");

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "1-2");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "abo-ba");
    }

    #[test]
    fn empty_split() {
        let spliterator = CountingSpliterator::new("");

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "12");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "aboba");
    }

    #[test]
    fn long_split() {
        let spliterator = CountingSpliterator::new("biba");

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "1biba2");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "abobibaba");
    }
}
