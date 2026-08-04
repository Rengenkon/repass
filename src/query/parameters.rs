use core::str;

pub enum Strategy {
    Character,
    WordList,
}

pub trait SplitStrategy {
    fn add_spliterator(parts: Vec<&str>) -> &str;
}

#[derive(Debug)]
pub struct BetweenPartsSpliterator<'a> {
    spliterator: &'a str,
}

impl<'a> BetweenPartsSpliterator<'a> {
    fn new(&mut self, spliterator: &'a str) -> Self {
        Self { spliterator }
    }
}
