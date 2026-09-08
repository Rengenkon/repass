use crate::dictionary::Dictionary;
use crate::separator::Separator;

pub enum Length {
    Float(usize, usize),
    Hard(usize),
}

impl Length {
    pub fn to_one_size(self: &Self) -> usize {
        match self {
            Length::Float(min, max) => rand::random_range(*min..=*max),
            Length::Hard(value) => *value,
        }
    }
}

pub struct Query<'a> {
    length: Length,
    dictionary: Box<&'a dyn Dictionary>,
    separator: Box<&'a dyn Separator>,
}

impl<'a> Query<'a> {
    pub fn new(
        length: Length,
        dictionary: Box<&'a dyn Dictionary>,
        separator: Box<&'a dyn Separator>,
    ) -> Self {
        Query {
            length,
            dictionary,
            separator,
        }
    }
}
