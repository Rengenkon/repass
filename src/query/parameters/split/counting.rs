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

    fn len(self: &Self, current_len: &usize, part: &'a str) -> (usize, &'a str) {
        let diff = self.length - *current_len;
        match part.len().cmp(&diff) {
            Ordering::Less => {
                (*current_len + part.len(), part)
            }
            Ordering::Equal => {
                (0, part)
            }
            Ordering::Greater => {
                let (left, right) = part.split_at(diff);
                let size = 0;
                let (size, right) = self.len(&size, right);
                let mut left = left.to_string();
                left.push_str(self.spliterator);
                left.push_str(right);
                (size, left.as_str())
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
            splited.push_str(self.len(&mut current_len, part).as_str());

            // let diff = self.length - current_len;
            // match part.len().cmp(&diff) {
            //     Ordering::Less => {
            //         splited.push_str(part);
            //         current_len += part.len();
            //     }
            //     Ordering::Equal => {
            //         splited.push_str(part);
            //         current_len = 0;
            //     }
            //     Ordering::Greater => {
            //         let mut left;
            //         let mut right;
            //         (left, right) = part.split_at(diff);
            //         splited.push_str(left);
            //         splited.push_str(self.spliterator);
            //         splited.push_str(right);
            //         current_len = right.len();
            //     }
            // }
            first_iteration = false;
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
