pub mod dictionary;
pub mod error;
pub mod generator;
pub mod query;
pub mod separator;
pub mod utils;

#[cfg(test)]
mod tests {
    use crate::dictionary::static_dictionary::SymbolicDictionary;
    use crate::generator::generate_once;
    use crate::query::{Length, Query};
    use crate::separator::interval::FixIntervalSeparator;

    #[test]
    fn full_test() {
        let dictionary = SymbolicDictionary::default();
        let separator = FixIntervalSeparator::default();
        let length = Length::default();
        let query = Query::new(length, &dictionary, &separator);
        let password = generate_once(&query).unwrap();
        assert!((12..=20).contains(&password.chars().count()));
    }
}
