pub mod dictionary;
pub mod error;
pub mod generator;
pub mod query;
pub mod separator;
pub mod utils;

#[cfg(test)]
mod tests {
    use crate::dictionary::{cache::DictionaryCache, static_dictionary::SymbolicDictionary};
    use crate::generator::generate_once;
    use crate::query::{Length, Query};
    use crate::separator::interval::FixIntervalSeparator;

    #[test]
    fn full_test() {
        let dictionary = DictionaryCache::new(SymbolicDictionary::default()).unwrap();
        let separator = FixIntervalSeparator::default();
        let length = Length::Exact(16);
        let query = Query::new(length, &dictionary, &separator).unwrap();
        let password = generate_once(&query).unwrap();
        assert!((12..=20).contains(&password.chars().count()));
    }
}
