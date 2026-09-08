use crate::dictionary::Dictionary;
use crate::query::Length::{Float, Hard};
use crate::separator::Separator;

pub enum Length {
    Float(usize, usize),
    Hard(usize),
}

impl Length {
    pub fn to_one_size(self: &Self) -> usize {
        match self {
            Float(min, max) => rand::random_range(*min..=*max),
            Hard(value) => *value,
        }
    }
}

impl Default for Length {
    fn default() -> Self {
        Float(12, 20)
    }
}

pub struct Query<'a> {
    length: Length,
    dictionary: &'a dyn Dictionary,
    separator: &'a dyn Separator,
}

impl<'a> Query<'a> {
    pub fn new(
        length: Length,
        dictionary: &'a dyn Dictionary,
        separator: &'a dyn Separator,
    ) -> Self {
        Query {
            length,
            dictionary,
            separator,
        }
    }

    pub fn length(&self) -> &Length {
        &self.length
    }

    pub fn dictionary(&self) -> &'a dyn Dictionary {
        self.dictionary
    }

    pub fn separator(&self) -> &'a dyn Separator {
        self.separator
    }
}


#[cfg(test)]
mod test {
    use rstest::rstest;
    use crate::query::Length::{Float, Hard};

    #[test]
    fn hard() {
        let l = Hard(3);
        assert_eq!(l.to_one_size(), 3);
    }

    #[test]
    fn float() {
        let l = Float(12, 20);
        let x = l.to_one_size();
        println!("{}", x);
        assert!((12..21).contains(&(x as i32)));
    }
}