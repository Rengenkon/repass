use core::str;

pub enum Strategy {
    Character,
    WordList,
}

pub trait SplitStrategy {
    fn add_spliterator(parts: Vec<&str>) -> String;
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

impl SplitStrategy for BetweenPartsSpliterator<'_> {
    fn add_spliterator(parts: Vec<&str>) -> String {
        let mut splited = String::new();
        for part in parts {
            splited += part;
        }
        splited
    }
}
