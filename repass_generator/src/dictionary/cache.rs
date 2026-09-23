use crate::dictionary::Dictionary;
use crate::error::GeneratorError;
use std::collections::HashMap;

/// Owns an immutable dictionary and an index from Unicode scalar length to
/// dictionary positions. Implementations must not mutate their contents through
/// interior mutability after being moved into this cache.
pub struct DictionaryCache<'a> {
    dictionary: Box<dyn Dictionary + 'a>,
    by_length: HashMap<usize, Vec<usize>>,
    available_lengths: Vec<usize>,
    min_length: usize,
}

impl<'a> DictionaryCache<'a> {
    pub fn new<D>(dictionary: D) -> Result<Self, GeneratorError>
    where
        D: Dictionary + 'a,
    {
        dictionary.validate()?;
        let mut by_length: HashMap<usize, Vec<usize>> = HashMap::new();
        for index in 0..dictionary.len() {
            let entry = dictionary
                .get(index)
                .ok_or(GeneratorError::InvalidDictionaryEntry { index })?;
            let length = entry.chars().count();
            if length == 0 {
                return Err(GeneratorError::EmptyDictionaryEntry);
            }
            by_length.entry(length).or_default().push(index);
        }
        let mut available_lengths: Vec<_> = by_length.keys().copied().collect();
        available_lengths.sort_unstable();
        let min_length = *available_lengths
            .first()
            .ok_or(GeneratorError::EmptyDictionary)?;

        Ok(Self {
            dictionary: Box::new(dictionary),
            by_length,
            available_lengths,
            min_length,
        })
    }

    pub fn len(&self) -> usize {
        self.dictionary.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dictionary.is_empty()
    }

    pub fn entry(&self, index: usize) -> Option<&str> {
        self.dictionary.get(index)
    }

    pub fn entries_with_length(&self, length: usize) -> &[usize] {
        self.by_length.get(&length).map_or(&[], Vec::as_slice)
    }

    pub fn available_lengths(&self) -> &[usize] {
        &self.available_lengths
    }

    pub fn min_length(&self) -> usize {
        self.min_length
    }

    pub fn into_dictionary(self) -> Box<dyn Dictionary + 'a> {
        self.dictionary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::file_dictionary::FileDictionary;

    #[test]
    fn indexes_unicode_scalar_lengths_without_copying_entries() {
        let dictionary = FileDictionary::from_entries(["a", "猫", "xy", "é"]).unwrap();
        let cache = DictionaryCache::new(dictionary).unwrap();

        assert_eq!(cache.entries_with_length(1), &[0, 1, 3]);
        assert_eq!(cache.entries_with_length(2), &[2]);
        assert_eq!(cache.entry(1), Some("猫"));
        assert_eq!(cache.available_lengths(), &[1, 2]);
        assert_eq!(cache.min_length(), 1);
    }

    #[test]
    fn consumes_dictionary_when_built() {
        let dictionary = FileDictionary::from_entries(["owned"]).unwrap();
        let cache = DictionaryCache::new(dictionary).unwrap();
        assert_eq!(cache.entry(0), Some("owned"));
    }
}
