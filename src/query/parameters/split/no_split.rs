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
    use super::super::for_tests::*;
    use crate::query::parameters::split::no_split::WithoutSpliterator;
    use std::vec;

    #[test]
    fn length_test() {
        let spliterator = vec![WithoutSpliterator::new()];
        size_equal_string(&spliterator, &get_test_data());
    }

    #[test]
    fn content_test() {
        let spliterator = WithoutSpliterator::new();
        let given = get_test_data();
        let when = vec!["", "", "1", "biba", "", "12", "123", "1234", "bibaboba"];
        equals_string(&spliterator, &given, &when);
    }
}
