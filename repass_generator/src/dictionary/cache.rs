use crate::dictionary::Dictionary;
use crate::error::GeneratorError;
use std::collections::HashMap;

/// Owns an immutable dictionary and an index from Unicode scalar-value count to
/// dictionary positions. Implementations must not mutate their contents through
/// interior mutability after being moved into this cache.
pub struct DictionaryCache<'a> {
    dictionary: Box<dyn Dictionary<'a> + 'a>,
    by_entry_chars: HashMap<usize, Vec<usize>>,
    available_entry_chars: Vec<usize>,
    min_entry_chars: usize,
}

impl<'a> DictionaryCache<'a> {
    pub fn new<D>(dictionary: D) -> Result<Self, GeneratorError>
    where
        D: Dictionary<'a> + 'a,
    {
        dictionary.validate()?;
        let mut by_entry_chars: HashMap<usize, Vec<usize>> = HashMap::new();
        for index in 0..dictionary.len() {
            let entry = dictionary
                .entry(index)
                .ok_or(GeneratorError::InvalidDictionaryEntry { index })?;
            let entry_chars = entry.chars().count();
            if entry_chars == 0 {
                return Err(GeneratorError::EmptyDictionaryEntry);
            }
            by_entry_chars.entry(entry_chars).or_default().push(index);
        }
        let mut available_entry_chars: Vec<_> = by_entry_chars.keys().copied().collect();
        available_entry_chars.sort_unstable();
        let min_entry_chars = *available_entry_chars
            .first()
            .ok_or(GeneratorError::EmptyDictionary)?;

        Ok(Self {
            dictionary: Box::new(dictionary),
            by_entry_chars,
            available_entry_chars,
            min_entry_chars,
        })
    }

    pub fn len(&self) -> usize {
        self.dictionary.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dictionary.is_empty()
    }

    pub fn entry(&self, index: usize) -> Option<&str> {
        self.dictionary.entry(index)
    }

    pub fn entries_with_chars(&self, entry_chars: usize) -> &[usize] {
        self.by_entry_chars
            .get(&entry_chars)
            .map_or(&[], Vec::as_slice)
    }

    pub fn available_entry_chars(&self) -> &[usize] {
        &self.available_entry_chars
    }

    pub fn min_entry_chars(&self) -> usize {
        self.min_entry_chars
    }

    pub fn into_dictionary(self) -> Box<dyn Dictionary<'a> + 'a> {
        self.dictionary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::file_dictionary::FileDictionary;

    #[test]
    fn indexes_entries_by_unicode_scalar_count_without_copying_entries() {
        let dictionary = FileDictionary::from_entries(["a", "猫", "xy", "é"]).unwrap();
        let cache = DictionaryCache::new(dictionary).unwrap();

        assert_eq!(cache.entries_with_chars(1), &[0, 1, 3]);
        assert_eq!(cache.entries_with_chars(2), &[2]);
        assert_eq!(cache.entry(1), Some("猫"));
        assert_eq!(cache.available_entry_chars(), &[1, 2]);
        assert_eq!(cache.min_entry_chars(), 1);
    }

    #[test]
    fn consumes_dictionary_when_built() {
        let dictionary = FileDictionary::from_entries(["owned"]).unwrap();
        let cache = DictionaryCache::new(dictionary).unwrap();
        assert_eq!(cache.entry(0), Some("owned"));
    }

    #[test]
    fn extracting_dictionary_drops_index_and_allows_mutation_before_rebuild() {
        static SECOND: [&str; 1] = ["second"];

        let dictionary = FileDictionary::from_entries(["first"]).unwrap();
        let cache = DictionaryCache::new(dictionary).unwrap();
        let mut dictionary = cache.into_dictionary();

        dictionary.add(&SECOND).unwrap();
        let rebuilt = DictionaryCache::new(dictionary).unwrap();
        assert_eq!(rebuilt.entry(1), Some("second"));
        assert_eq!(rebuilt.entries_with_chars(6), &[1]);
    }
}
