use crate::dictionary::Dictionary;
use crate::error::DictionaryError;
use std::collections::HashSet;

pub fn digits() -> &'static [&'static str] {
    &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
}

pub fn lowercase_letters() -> &'static [&'static str] {
    &[
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z",
    ]
}

pub fn uppercase_letters() -> &'static [&'static str] {
    &[
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ]
}

pub fn punctuation() -> &'static [&'static str] {
    &["!", "?", ".", ",", ";", ":"]
}

pub fn miscellaneous_symbols() -> &'static [&'static str] {
    &["_", "-", "@", "=", "+", "*", "/"]
}

pub fn brackets() -> &'static [&'static str] {
    &["(", ")", "[", "]", "{", "}"]
}

pub fn quotes_and_ampersand_symbols() -> &'static [&'static str] {
    &["\"", "'", "`", "&"]
}

pub fn hash_dollar_percent_caret_symbols() -> &'static [&'static str] {
    &["#", "$", "%", "^"]
}

pub fn backslash_pipe_tilde_symbols() -> &'static [&'static str] {
    &["\\", "|", "~"]
}

pub type PresetMask = u16;

const DIGITS_MASK: PresetMask = 1 << 0;
const LOWERCASE_LETTERS_MASK: PresetMask = 1 << 1;
const UPPERCASE_LETTERS_MASK: PresetMask = 1 << 2;
const PUNCTUATION_MASK: PresetMask = 1 << 3;
const MISCELLANEOUS_SYMBOLS_MASK: PresetMask = 1 << 4;
const BRACKETS_MASK: PresetMask = 1 << 5;
const QUOTES_AND_AMPERSAND_MASK: PresetMask = 1 << 6;
const HASH_DOLLAR_PERCENT_CARET_MASK: PresetMask = 1 << 7;
const BACKSLASH_PIPE_TILDE_MASK: PresetMask = 1 << 8;

fn preset_sets() -> [(PresetMask, &'static [&'static str]); 9] {
    [
        (DIGITS_MASK, digits()),
        (LOWERCASE_LETTERS_MASK, lowercase_letters()),
        (UPPERCASE_LETTERS_MASK, uppercase_letters()),
        (PUNCTUATION_MASK, punctuation()),
        (MISCELLANEOUS_SYMBOLS_MASK, miscellaneous_symbols()),
        (BRACKETS_MASK, brackets()),
        (QUOTES_AND_AMPERSAND_MASK, quotes_and_ampersand_symbols()),
        (
            HASH_DOLLAR_PERCENT_CARET_MASK,
            hash_dollar_percent_caret_symbols(),
        ),
        (BACKSLASH_PIPE_TILDE_MASK, backslash_pipe_tilde_symbols()),
    ]
}

/// Combines predefined character sets with caller-provided entries.
/// All entries are borrowed and must outlive the dictionary.
pub struct PresetDictionary<'a> {
    preset_mask: PresetMask,
    dictionary: Vec<&'a str>,
    known_entries: HashSet<&'a str>,
}

impl<'a> PresetDictionary<'a> {
    pub fn new() -> Self {
        Self {
            preset_mask: 0,
            dictionary: Vec::new(),
            known_entries: HashSet::new(),
        }
    }

    pub fn from_preset_mask(preset_mask: PresetMask) -> Self {
        let mut result = Self::new();
        for (bit, values) in preset_sets() {
            if preset_mask & bit != 0 {
                result.add_preset_set(bit, values);
            }
        }
        result.preset_mask = preset_mask;
        result
    }

    pub fn preset_mask(&self) -> PresetMask {
        self.preset_mask
    }

    fn add_preset_set(&mut self, bit: PresetMask, values: &'static [&'static str]) {
        if self.preset_mask & bit != 0 {
            return;
        }
        for value in values {
            if self.known_entries.insert(value) {
                self.dictionary.push(value);
            }
        }
        self.preset_mask |= bit;
    }

    pub fn add_all_presets(&mut self) -> &mut Self {
        self.add_digits()
            .add_lowercase_letters()
            .add_uppercase_letters()
            .add_punctuation()
            .add_miscellaneous_symbols()
            .add_brackets()
            .add_quotes_and_ampersand_symbols()
            .add_hash_dollar_percent_caret_symbols()
            .add_backslash_pipe_tilde_symbols()
    }

    pub fn add_digits(&mut self) -> &mut Self {
        self.add_preset_set(DIGITS_MASK, digits());
        self
    }

    pub fn add_lowercase_letters(&mut self) -> &mut Self {
        self.add_preset_set(LOWERCASE_LETTERS_MASK, lowercase_letters());
        self
    }

    pub fn add_uppercase_letters(&mut self) -> &mut Self {
        self.add_preset_set(UPPERCASE_LETTERS_MASK, uppercase_letters());
        self
    }

    pub fn add_punctuation(&mut self) -> &mut Self {
        self.add_preset_set(PUNCTUATION_MASK, punctuation());
        self
    }

    pub fn add_miscellaneous_symbols(&mut self) -> &mut Self {
        self.add_preset_set(MISCELLANEOUS_SYMBOLS_MASK, miscellaneous_symbols());
        self
    }

    pub fn add_brackets(&mut self) -> &mut Self {
        self.add_preset_set(BRACKETS_MASK, brackets());
        self
    }

    pub fn add_quotes_and_ampersand_symbols(&mut self) -> &mut Self {
        self.add_preset_set(QUOTES_AND_AMPERSAND_MASK, quotes_and_ampersand_symbols());
        self
    }

    pub fn add_hash_dollar_percent_caret_symbols(&mut self) -> &mut Self {
        self.add_preset_set(
            HASH_DOLLAR_PERCENT_CARET_MASK,
            hash_dollar_percent_caret_symbols(),
        );
        self
    }

    pub fn add_backslash_pipe_tilde_symbols(&mut self) -> &mut Self {
        self.add_preset_set(BACKSLASH_PIPE_TILDE_MASK, backslash_pipe_tilde_symbols());
        self
    }
}

impl<'a> Dictionary<'a> for PresetDictionary<'a> {
    fn entry(&self, index: usize) -> Option<&str> {
        self.dictionary.get(index).copied()
    }

    fn add(&mut self, values: &'a [&'a str]) -> Result<(), DictionaryError> {
        if values.is_empty() {
            return Ok(());
        }

        let mut incoming_entries = HashSet::new();
        incoming_entries
            .try_reserve(values.len())
            .map_err(|_| DictionaryError::ResourceLimit)?;
        for value in values {
            if value.is_empty() {
                return Err(DictionaryError::EmptyEntry);
            }
            if self.known_entries.contains(value) || !incoming_entries.insert(*value) {
                return Err(DictionaryError::DuplicateEntry((*value).to_owned()));
            }
        }

        self.dictionary
            .try_reserve(values.len())
            .map_err(|_| DictionaryError::ResourceLimit)?;
        self.known_entries
            .try_reserve(values.len())
            .map_err(|_| DictionaryError::ResourceLimit)?;
        for value in values {
            self.known_entries.insert(*value);
            self.dictionary.push(value);
        }

        Ok(())
    }

    fn len(&self) -> usize {
        self.dictionary.len()
    }
}

impl Default for PresetDictionary<'static> {
    fn default() -> Self {
        Self::from_preset_mask(DIGITS_MASK)
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
    fn preset_sets_are_selected_by_independent_bits() {
        let mut dictionary = PresetDictionary::new();
        dictionary.add_digits().add_lowercase_letters();

        assert_eq!(
            dictionary.preset_mask(),
            DIGITS_MASK | LOWERCASE_LETTERS_MASK
        );
        assert_eq!(dictionary.len(), digits().len() + lowercase_letters().len());
        assert_eq!(dictionary.entry(0), Some("0"));
        assert_eq!(dictionary.entry(10), Some("a"));
    }

    #[test]
    fn adding_same_preset_set_is_idempotent() {
        let mut dictionary = PresetDictionary::new();
        dictionary.add_digits();
        let mask = dictionary.preset_mask();
        let len = dictionary.len();

        dictionary.add_digits();
        assert_eq!(dictionary.preset_mask(), mask);
        assert_eq!(dictionary.len(), len);
    }

    #[test]
    fn adding_custom_entries_checks_each_entry_atomically() {
        let mut dictionary = PresetDictionary::default();
        assert_eq!(
            dictionary.add(&ATOMIC_VALUES),
            Err(DictionaryError::DuplicateEntry("0".to_owned()))
        );
        assert_eq!(dictionary.len(), digits().len());
        assert_eq!(dictionary.known_entries.len(), digits().len());
        dictionary.add(&CUSTOM_VALUES).unwrap();
        assert_eq!(dictionary.known_entries.len(), digits().len() + 2);
        assert_eq!(dictionary.entry(dictionary.len() - 1), Some("🦀"));
        assert_eq!(
            dictionary.add(&CUSTOM_VALUES[..1]),
            Err(DictionaryError::DuplicateEntry("new".to_owned()))
        );
    }

    #[test]
    fn preset_addition_updates_an_initialized_hash_set() {
        static CUSTOM_VALUE: [&str; 1] = ["custom"];

        let mut dictionary = PresetDictionary::new();
        dictionary.add(&CUSTOM_VALUE).unwrap();
        assert!(dictionary.known_entries.contains("custom"));

        dictionary.add_digits();
        assert!(dictionary.known_entries.contains("0"));

        static NUMBER_VALUE: [&str; 1] = ["0"];
        assert_eq!(
            dictionary.add(&NUMBER_VALUE),
            Err(DictionaryError::DuplicateEntry("0".to_owned()))
        );
    }

    #[test]
    fn duplicate_inside_input_is_rejected_without_partial_mutation() {
        let mut dictionary = PresetDictionary::new();

        assert_eq!(
            dictionary.add(&DUPLICATE_VALUES),
            Err(DictionaryError::DuplicateEntry("unique".to_owned()))
        );
        assert_eq!(dictionary.len(), 0);
        assert!(dictionary.known_entries.is_empty());
    }

    #[test]
    fn overlapping_custom_and_preset_values_are_kept_once() {
        static NUMBER: [&str; 1] = ["0"];
        let mut dictionary = PresetDictionary::new();
        dictionary.add(&NUMBER).unwrap();
        dictionary.add_digits();

        assert_eq!(dictionary.len(), digits().len());
        assert_eq!(dictionary.entry(0), Some("0"));
        assert!(crate::dictionary::cache::DictionaryCache::new(dictionary).is_ok());
    }

    #[test]
    fn preset_and_custom_entries_keep_insertion_order() {
        let mut dictionary = PresetDictionary::new();
        dictionary.add(&FIRST_VALUES).unwrap();
        dictionary.add(&THIRD_VALUES).unwrap();
        dictionary.add_digits();

        assert_eq!(dictionary.len(), digits().len() + 3);
        assert_eq!(dictionary.entry(0), Some("first"));
        assert_eq!(dictionary.entry(1), Some("second"));
        assert_eq!(dictionary.entry(2), Some("third"));
        assert_eq!(dictionary.entry(3), Some("0"));
        assert_eq!(dictionary.entry(12), Some("9"));
        assert_eq!(dictionary.entry(13), None);
    }
}
