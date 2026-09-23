use crate::dictionary::Dictionary;
use crate::error::DictionaryError;
use std::collections::{BTreeMap, HashSet};

pub fn numbers() -> &'static [&'static str] {
    &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
}

pub fn lowercase() -> &'static [&'static str] {
    &[
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z",
    ]
}

pub fn uppercase() -> &'static [&'static str] {
    &[
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ]
}

pub fn special_punctuation() -> &'static [&'static str] {
    &["!", "?", ".", ",", ";", ":"]
}

pub fn special_math() -> &'static [&'static str] {
    &["_", "-", "@", "=", "+", "*", "/"]
}

pub fn special_brackets() -> &'static [&'static str] {
    &["(", ")", "[", "]", "{", "}"]
}

pub fn special_quotes() -> &'static [&'static str] {
    &["\"", "'", "`", "&"]
}

pub fn special_hash_percent() -> &'static [&'static str] {
    &["#", "$", "%", "^"]
}

pub fn special_escape() -> &'static [&'static str] {
    &["\\", "|", "~"]
}

pub type Mask = u16;

const NUMBERS_MASK: Mask = 1 << 0;
const LOWERCASE_MASK: Mask = 1 << 1;
const UPPERCASE_MASK: Mask = 1 << 2;
const PUNCTUATION_MASK: Mask = 1 << 3;
const MATH_MASK: Mask = 1 << 4;
const BRACKETS_MASK: Mask = 1 << 5;
const QUOTES_MASK: Mask = 1 << 6;
const HASH_PERCENT_MASK: Mask = 1 << 7;
const ESCAPE_MASK: Mask = 1 << 8;

fn static_sets() -> [(Mask, &'static [&'static str]); 9] {
    [
        (NUMBERS_MASK, numbers()),
        (LOWERCASE_MASK, lowercase()),
        (UPPERCASE_MASK, uppercase()),
        (PUNCTUATION_MASK, special_punctuation()),
        (MATH_MASK, special_math()),
        (BRACKETS_MASK, special_brackets()),
        (QUOTES_MASK, special_quotes()),
        (HASH_PERCENT_MASK, special_hash_percent()),
        (ESCAPE_MASK, special_escape()),
    ]
}

/// Combines predefined static character groups with caller-provided entries.
/// All entries are borrowed and must outlive the dictionary.
pub struct SymbolicDictionary<'a> {
    mask: Mask,
    dictionary: BTreeMap<usize, &'a [&'a str]>,
    length: usize,
    known_entries: Option<HashSet<&'a str>>,
}

impl<'a> SymbolicDictionary<'a> {
    pub fn new() -> Self {
        Self {
            mask: 0,
            dictionary: BTreeMap::new(),
            length: 0,
            known_entries: None,
        }
    }

    pub fn from_mask(mask: Mask) -> Self {
        let mut result = Self::new();
        for (bit, values) in static_sets() {
            if mask & bit != 0 {
                result.add_static_set(bit, values);
            }
        }
        result.mask = mask;
        result
    }

    pub fn mask(&self) -> Mask {
        self.mask
    }

    fn add_static_set(&mut self, bit: Mask, values: &'static [&'static str]) {
        if self.mask & bit != 0 {
            return;
        }

        self.dictionary.insert(self.length, values);
        self.length += values.len();
        self.mask |= bit;

        if let Some(known_entries) = &mut self.known_entries {
            for value in values {
                known_entries.insert(value);
            }
        }
    }

    pub fn add_all(&mut self) -> &mut Self {
        self.add_numbers()
            .add_lowercase()
            .add_uppercase()
            .add_special_punctuation()
            .add_special_math()
            .add_special_brackets()
            .add_special_quotes()
            .add_special_hash_percent()
            .add_special_escape()
    }

    pub fn add_numbers(&mut self) -> &mut Self {
        self.add_static_set(NUMBERS_MASK, numbers());
        self
    }

    pub fn add_lowercase(&mut self) -> &mut Self {
        self.add_static_set(LOWERCASE_MASK, lowercase());
        self
    }

    pub fn add_uppercase(&mut self) -> &mut Self {
        self.add_static_set(UPPERCASE_MASK, uppercase());
        self
    }

    pub fn add_special_punctuation(&mut self) -> &mut Self {
        self.add_static_set(PUNCTUATION_MASK, special_punctuation());
        self
    }

    pub fn add_special_math(&mut self) -> &mut Self {
        self.add_static_set(MATH_MASK, special_math());
        self
    }

    pub fn add_special_brackets(&mut self) -> &mut Self {
        self.add_static_set(BRACKETS_MASK, special_brackets());
        self
    }

    pub fn add_special_quotes(&mut self) -> &mut Self {
        self.add_static_set(QUOTES_MASK, special_quotes());
        self
    }

    pub fn add_special_hash_percent(&mut self) -> &mut Self {
        self.add_static_set(HASH_PERCENT_MASK, special_hash_percent());
        self
    }

    pub fn add_special_escape(&mut self) -> &mut Self {
        self.add_static_set(ESCAPE_MASK, special_escape());
        self
    }

    pub fn length(&self) -> usize {
        self.length
    }
}

impl<'a> Dictionary<'a> for SymbolicDictionary<'a> {
    fn get(&self, index: usize) -> Option<&str> {
        let (segment_start, segment) = self.dictionary.range(..=index).next_back()?;
        let offset = index.checked_sub(*segment_start)?;
        let entry: Option<&&str> = segment.get(offset);
        let entry: Option<&str> = match entry {
            Some(value) => Some(*value),
            None => None,
        };
        entry
    }

    fn add(&mut self, values: &'a [&'a str]) -> Result<(), DictionaryError> {
        if values.is_empty() {
            return Ok(());
        }

        let new_length = self
            .length
            .checked_add(values.len())
            .ok_or(DictionaryError::ResourceLimit)?;

        if let Some(known_entries) = &mut self.known_entries {
            let mut incoming_entries = HashSet::new();
            incoming_entries
                .try_reserve(values.len())
                .map_err(|_| DictionaryError::ResourceLimit)?;

            for value in values {
                if value.is_empty() {
                    return Err(DictionaryError::EmptyEntry);
                }
                if known_entries.contains(value) || !incoming_entries.insert(*value) {
                    return Err(DictionaryError::DuplicateEntry((*value).to_owned()));
                }
            }

            // Reserve before changing the dictionary so allocation failure is atomic.
            known_entries
                .try_reserve(values.len())
                .map_err(|_| DictionaryError::ResourceLimit)?;

            self.dictionary.insert(self.length, values);
            self.length = new_length;

            for value in values {
                known_entries.insert(*value);
            }
        } else {
            let mut known_entries = HashSet::new();
            known_entries
                .try_reserve(new_length)
                .map_err(|_| DictionaryError::ResourceLimit)?;
            for entries in self.dictionary.values() {
                for value in *entries {
                    known_entries.insert(*value);
                }
            }
            for value in values {
                if value.is_empty() {
                    return Err(DictionaryError::EmptyEntry);
                }
                if !known_entries.insert(*value) {
                    return Err(DictionaryError::DuplicateEntry((*value).to_owned()));
                }
            }

            self.dictionary.insert(self.length, values);
            self.length = new_length;
            self.known_entries = Some(known_entries);
        }

        Ok(())
    }

    fn len(&self) -> usize {
        self.length
    }
}

impl Default for SymbolicDictionary<'static> {
    fn default() -> Self {
        Self {
            mask: NUMBERS_MASK,
            dictionary: BTreeMap::from([(0, numbers())]),
            length: numbers().len(),
            known_entries: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static ATOMIC_VALUES: [&str; 2] = ["new", "0"];
    static CUSTOM_VALUES: [&str; 2] = ["new", "🦀"];
    static DUPLICATE_VALUES: [&str; 2] = ["unique", "unique"];
    static FIRST_VALUES: [&str; 2] = ["first", "second"];
    static THIRD_VALUES: [&str; 1] = ["third"];

    #[test]
    fn static_groups_are_selected_by_independent_bits() {
        let mut dictionary = SymbolicDictionary::new();
        dictionary.add_numbers().add_lowercase();

        assert_eq!(dictionary.mask(), NUMBERS_MASK | LOWERCASE_MASK);
        assert_eq!(dictionary.len(), numbers().len() + lowercase().len());
        assert_eq!(dictionary.get(0), Some("0"));
        assert_eq!(dictionary.get(10), Some("a"));
    }

    #[test]
    fn adding_same_static_set_is_idempotent() {
        let mut dictionary = SymbolicDictionary::new();
        dictionary.add_numbers();
        let mask = dictionary.mask();
        assert!(dictionary.known_entries.is_none());

        dictionary.add_numbers();
        assert_eq!(dictionary.mask(), mask);
        assert!(dictionary.known_entries.is_none());
    }

    #[test]
    fn adding_custom_entries_checks_each_entry_atomically() {
        let mut dictionary = SymbolicDictionary::default();
        assert_eq!(
            dictionary.add(&ATOMIC_VALUES),
            Err(DictionaryError::DuplicateEntry("0".to_owned()))
        );
        assert_eq!(dictionary.len(), numbers().len());
        assert!(dictionary.known_entries.is_none());
        dictionary.add(&CUSTOM_VALUES).unwrap();
        assert!(dictionary.known_entries.is_some());
        assert_eq!(dictionary.get(dictionary.len() - 1), Some("🦀"));
        assert_eq!(
            dictionary.add(&CUSTOM_VALUES[..1]),
            Err(DictionaryError::DuplicateEntry("new".to_owned()))
        );
    }

    #[test]
    fn static_addition_updates_an_initialized_hash_set() {
        static CUSTOM_VALUE: [&str; 1] = ["custom"];

        let mut dictionary = SymbolicDictionary::new();
        dictionary.add(&CUSTOM_VALUE).unwrap();
        assert!(dictionary.known_entries.is_some());

        dictionary.add_numbers();
        let known_entries = dictionary.known_entries.as_ref();
        assert!(known_entries.is_some());
        let contains_number = match known_entries {
            Some(entries) => entries.contains("0"),
            None => false,
        };
        assert!(contains_number);

        static NUMBER_VALUE: [&str; 1] = ["0"];
        assert_eq!(
            dictionary.add(&NUMBER_VALUE),
            Err(DictionaryError::DuplicateEntry("0".to_owned()))
        );
    }

    #[test]
    fn duplicate_inside_input_is_rejected_without_partial_mutation() {
        let mut dictionary = SymbolicDictionary::new();

        assert_eq!(
            dictionary.add(&DUPLICATE_VALUES),
            Err(DictionaryError::DuplicateEntry("unique".to_owned()))
        );
        assert_eq!(dictionary.len(), 0);
        assert!(dictionary.known_entries.is_none());
    }

    #[test]
    fn static_and_custom_entries_are_read_from_ordered_segments() {
        let mut dictionary = SymbolicDictionary::new();
        dictionary.add(&FIRST_VALUES).unwrap();
        dictionary.add(&THIRD_VALUES).unwrap();
        dictionary.add_numbers();

        assert_eq!(dictionary.len(), numbers().len() + 3);
        assert_eq!(dictionary.get(0), Some("first"));
        assert_eq!(dictionary.get(1), Some("second"));
        assert_eq!(dictionary.get(2), Some("third"));
        assert_eq!(dictionary.get(3), Some("0"));
        assert_eq!(dictionary.get(12), Some("9"));
        assert_eq!(dictionary.get(13), None);
    }
}
