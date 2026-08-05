pub mod counting;
pub mod number;
pub mod parts;
pub mod no_split;

pub trait SplitStrategy {
    fn add_spliterator(self: &Self, parts: &[&str]) -> String;
    fn compute_length(self: &Self, parts: &[&str]) -> usize;
}

mod for_tests {
    use super::SplitStrategy;
    use std::fmt::Debug;

    pub fn size_equals_test<S>(spliterator: &S, parts: &[&str])
    where
        S: SplitStrategy + Debug,
    {
        let real_length = spliterator.add_spliterator(parts).len();
        let computed_length = spliterator.compute_length(&parts);
        assert_eq!(
            computed_length, real_length,
            "spliterator: {:?}, data: {:?}",
            spliterator, parts
        );
    }

    pub fn size_equal_str<S>(spliterators: &[S], parts: &Vec<Vec<&str>>)
    where
        S: SplitStrategy + Debug,
    {
        for i in 0..spliterators.len() {
            for j in 0..parts.len() {
                size_equals_test(&spliterators[i], &parts[j]);
            }
        }
    }

    pub fn size_equal_string<S>(spliterators: &[S], parts: &Vec<Vec<String>>)
    where
        S: SplitStrategy + Debug,
    {
        let parts = convert_vectors(&parts);
        size_equal_str(&spliterators, &parts);
    }

    fn convert_vectors(parts: &[Vec<String>]) -> Vec<Vec<&str>> {
        parts
            .iter()
            .map(|part| convert_strings(part))
            .collect::<Vec<Vec<&str>>>()
    }

    fn convert_strings(parts: &[String]) -> Vec<&str> {
        parts
            .iter()
            .map(|part| part.as_str())
            .collect::<Vec<&str>>()
    }

    pub fn get_test_data() -> Vec<Vec<String>> {
        vec![
            Vec::new(),
            vec![String::from("")],
            vec![String::from("1")],
            vec![String::from("biba")],
            vec![String::from(""), String::from("")],
            vec![String::from("1"), String::from("2")],
            vec![String::from("1"), String::from("2"), String::from("3")],
            vec![
                String::from("1"),
                String::from("2"),
                String::from("3"),
                String::from("4"),
            ],
            vec![String::from("biba"), String::from("boba")],
        ]
    }
}
