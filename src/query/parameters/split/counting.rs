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

    fn len(self: &Self, current_len: &usize, part: &'a str) -> (usize, String) {
        let diff = self.length - *current_len;
        match part.len().cmp(&diff) {
            Ordering::Less => (*current_len + part.len(), part.to_string()),
            Ordering::Equal => (0, part.to_string()),
            Ordering::Greater => {
                let (left, right) = part.split_at(diff);
                let size = 0;
                let (size, right) = self.len(&size, right);
                let mut left = left.to_string();
                left.push_str(self.spliterator);
                left.push_str(&right);
                (size, left)
            }
        }
    }
}

impl SplitStrategy for CountingSpliterator<'_> {
    fn add_spliterator(self: &Self, parts: Vec<&str>) -> String {
        let mut splited = String::new();
        let mut first_iteration = true;
        let mut current_len = 0;

        for part in parts {
            if current_len == 0 && !first_iteration {
                splited.push_str(self.spliterator);
            }
            let (s, string) = self.len(&current_len, part);
            current_len = s;
            splited.push_str(&string);
            first_iteration = first_iteration && part.len() == 0;
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
        let spliterator = CountingSpliterator::new("-", 1);

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "a-b-o-b-a");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "1-2");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "a-b-o-b-a");
    }

    #[test]
    fn zero_length_split() {
        let spliterator = CountingSpliterator::new("-", 0);

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "a-b-o-b-a");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "1-2");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "a-b-o-b-a");
    }

    #[test]
    fn empty_split() {
        let spliterator = CountingSpliterator::new("", 1);

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "aboba");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "12");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "aboba");
    }

    #[test]
    fn long_split() {
        let spliterator = CountingSpliterator::new("biba", 1);

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["aboba"]), "abibabbibaobibabbibaa");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "1biba2");
        assert_eq!(spliterator.add_spliterator(vec!["abo", "ba"]), "abibabbibaobibabbibaa");
    }

    #[test]
    fn long_and_length_split() {
        let spliterator = CountingSpliterator::new("aa", 3);

        assert_eq!(spliterator.add_spliterator(Vec::new()), "");
        assert_eq!(spliterator.add_spliterator(vec![""]), "");
        assert_eq!(spliterator.add_spliterator(vec!["1"]), "1");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2"]), "12");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2", "3"]), "123");
        assert_eq!(spliterator.add_spliterator(vec!["1", "2", "3", "4"]), "123aa4");
        assert_eq!(spliterator.add_spliterator(vec!["", "1234"]), "123aa4");
    }
}
