use enum_display::EnumDisplay;

pub mod counting;
pub mod no_split;
pub mod number;
pub mod parts;

trait SeparatorInternal {
    fn get_summary_length(parts: &[&str]) -> usize {
        parts.iter().map(|part| part.len()).sum::<usize>()
    }

    fn format_error_msg(errors: &Vec<IllegalArgumentError>) -> String {
        let mut result = String::new();
        for error in errors {
            todo!()
        }
        result
    }

    fn add_spliterator(self: &Self, parts: &[&str]) -> Result<String, String>;
    fn length_with_separators(self: &Self, parts: &[&str]) -> usize;

    fn chack_errors(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError>;

    fn panic_with_errors<T, F>(self: &Self, parts: &[&str], f: F) -> T
    where
        F: Fn(&Self, &[&str]) -> Result<T, String>,
    {
        let errors = self.chack_errors(parts);
        if errors.is_empty() {
            let result = f(self, parts);
            if result.is_ok() {
                return result.ok().unwrap();
            }
            panic!("{}", result.err().unwrap())
        }
        let errors_msg = Self::format_error_msg(&errors);
        panic!("{}", errors_msg)
    }
}

/// Determine public functions for working with 'Separators'
///
/// 'Sequence' is 'parts or character of parts'
pub trait SplitStrategy: SeparatorInternal {
    /// Separate sequence with setuped separate segment
    fn separate(self: &Self, parts: &[&str]) -> String {
        self.panic_with_errors(parts, Self::add_spliterator)
    }

    /// Compute final length of sequence with separators
    fn compute_final_length(self: &Self, parts: &[&str]) -> usize {
        self.panic_with_errors(parts, Self::length_with_separators)
    }

    /// Check common errors in Separator or parts
    fn validate(self: &Self, parts: &[&str]) -> Vec<IllegalArgumentError> {
        self.chack_errors(parts)
    }
}

#[derive(Debug, EnumDisplay)]
pub enum IllegalArgumentError {
    #[display("")]
    EmptySeparator,
    #[display("")]
    CountOfSeparatorsIsZero,
    #[display("")]
    SummaryLengthOfPartsIsZero,
    #[display("")]
    Specific(String),
}

mod for_tests {
    use super::SplitStrategy;
    use std::fmt::Debug;

    pub fn size_equals_test<S>(spliterator: &S, parts: &[&str])
    where
        S: SplitStrategy + Debug,
    {
        let real_length = spliterator.add_spliterator(parts).len();
        let computed_length = spliterator.length_with_separators(&parts);
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

    pub fn equals_str<S>(spliterators: &S, parts: &Vec<Vec<&str>>, results: &Vec<&str>)
    where
        S: SplitStrategy + Debug,
    {
        let mut p_iter = parts.iter();
        let mut r_iter = results.iter();
        loop {
            let p = p_iter.next();
            let r = r_iter.next();
            match (p, r) {
                (Some(p), Some(r)) => {
                    let value = spliterators.add_spliterator(p);
                    assert_eq!(value.as_str(), *r);
                }
                (None, None) => {
                    break;
                }
                _ => {
                    panic!("Given data difference size")
                }
            }
        }
    }

    pub fn equals_string<S>(spliterators: &S, parts: &Vec<Vec<String>>, results: &Vec<&str>)
    where
        S: SplitStrategy + Debug,
    {
        let parts = convert_vectors(&parts);
        equals_str(spliterators, &parts, results);
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
