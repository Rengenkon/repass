pub mod query;
pub mod generator;
pub mod separator;
pub mod utils;
pub mod dictionary;

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
        let password = generate_once(&query);
        println!("{}", password);
        assert!(!password.is_empty())
    }
}