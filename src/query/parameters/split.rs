pub mod parts;
pub mod counting;
pub mod number;

pub trait SplitStrategy {
    fn add_spliterator(self: &Self, parts: &Vec<&str>) -> String;
    fn compute_length(self: &Self, parts: &Vec<&str>) -> usize;
}