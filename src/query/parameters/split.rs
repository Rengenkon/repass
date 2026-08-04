pub mod parts;

pub trait SplitStrategy {
    fn add_spliterator(self: &Self, parts: Vec<&str>) -> String;
}