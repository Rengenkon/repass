use crate::error::{DictionaryError, GeneratorError};
use std::collections::HashSet;

pub mod cache;
pub mod file_dictionary;
pub mod services;
pub mod static_dictionary;

pub trait Dictionary<'a> {
    fn get(&self, index: usize) -> Option<&str>;

    /// Adds entries atomically. Empty and duplicate entries are rejected.
    fn add(&mut self, values: &'a [&'a str]) -> Result<(), DictionaryError>;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn validate(&self) -> Result<(), GeneratorError> {
        if self.is_empty() {
            return Err(GeneratorError::EmptyDictionary);
        }
        for index in 0..self.len() {
            match self.get(index) {
                Some("") => return Err(GeneratorError::EmptyDictionaryEntry),
                None => return Err(GeneratorError::EmptyDictionary),
                Some(_) => {}
            }
        }
        Ok(())
    }
}

impl<'a, T: Dictionary<'a> + ?Sized> Dictionary<'a> for Box<T> {
    fn get(&self, index: usize) -> Option<&str> {
        let dictionary: &T = &**self;
        dictionary.get(index)
    }

    fn add(&mut self, values: &'a [&'a str]) -> Result<(), DictionaryError> {
        let dictionary: &mut T = &mut **self;
        dictionary.add(values)
    }

    fn len(&self) -> usize {
        let dictionary: &T = &**self;
        dictionary.len()
    }
}

pub(crate) fn validate_new_entries<'a>(
    existing: impl IntoIterator<Item = &'a str>,
    values: &[&str],
) -> Result<(), DictionaryError> {
    let mut seen: HashSet<&str> = existing.into_iter().collect();
    for value in values {
        if value.is_empty() {
            return Err(DictionaryError::EmptyEntry);
        }
        if !seen.insert(value) {
            return Err(DictionaryError::DuplicateEntry((*value).to_owned()));
        }
    }
    Ok(())
}
