use crate::error::{DictionaryError, GeneratorError};

pub mod cache;
pub mod file_dictionary;
pub mod preset_dictionary;
pub mod presets;

pub trait Dictionary<'a> {
    fn entry(&self, index: usize) -> Option<&str>;

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
            match self.entry(index) {
                Some("") => return Err(GeneratorError::EmptyDictionaryEntry),
                None => return Err(GeneratorError::EmptyDictionary),
                Some(_) => {}
            }
        }
        Ok(())
    }
}

impl<'a, T: Dictionary<'a> + ?Sized> Dictionary<'a> for Box<T> {
    fn entry(&self, index: usize) -> Option<&str> {
        let dictionary: &T = &**self;
        dictionary.entry(index)
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
