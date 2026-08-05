use super::SplitStrategy;

#[derive(Debug)]
pub struct WithoutSpliterator {}

impl WithoutSpliterator {
    pub fn new() -> Self {
        Self {}
    }
}

impl SplitStrategy for WithoutSpliterator {
    fn add_spliterator(self: &Self, parts: &[&str]) -> String {
        parts.concat()
    }

    fn compute_length(self: &Self, parts: &[&str]) -> usize {
        parts.iter().map(|s| s.len()).sum()
    }
}

mod tests {
    use std::vec;
    use crate::query::parameters::split::no_split::WithoutSpliterator;
    use super::super::for_tests::*;

    #[test]
    fn length_test() {
        let spliterator = vec![WithoutSpliterator::new()];
        size_equal_string(&spliterator, &get_test_data());
    }
}