pub mod counting;
pub mod number;
pub mod parts;

pub trait SplitStrategy {
    fn add_spliterator(self: &Self, parts: &[&str]) -> String;
    fn compute_length(self: &Self, parts: &[&str]) -> usize;
}

mod for_tests {
    use super::SplitStrategy;
    use std::fmt::Debug;

    pub fn size_equals_test<S>(spliterator: &S, str: &[&str])
    where
        S: SplitStrategy + Debug,
    {
        let real_length = spliterator.add_spliterator(str).len();
        let computed_length = spliterator.compute_length(&str);
        assert_eq!(
            computed_length, real_length,
            "spliterator: {:?}, data: {:?}",
            spliterator, str
        );
    }
}
