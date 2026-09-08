use crate::dictionary::Dictionary;
use crate::utils::FuzzyGet;
use std::collections::BTreeMap;

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

pub struct SymbolicDictionary<'a> {
    length: usize,
    dictionary: BTreeMap<usize, &'a [&'a str]>,
    is_simple: bool,
}

impl<'a> SymbolicDictionary<'a> {
    pub fn new() -> Self {
        SymbolicDictionary {
            length: 0,
            dictionary: BTreeMap::new(),
            is_simple: true,
        }
    }

    // todo add value several times
    pub fn add(self: &mut Self, value: &'a [&'a str]) {
        self.dictionary.insert(self.length, value);
        self.length += value.len();
        let long_value = value.iter().any(|v| v.len() > 1);
        if long_value {
            self.is_simple = false;
        }
    }

    pub fn add_numbers(self: &mut Self) -> &mut Self {
        self.add(numbers());
        self
    }

    // todo add methods
}

impl<'b> Dictionary for SymbolicDictionary<'b> {
    fn get(self: &Self, index: usize) -> Option<&str> {
        let part = self.dictionary.get_left(&index)?;
        let index = index % part.len();
        Some(part[index])
    }

    fn len(self: &Self) -> usize {
        self.length
    }

    fn is_simple(self: &Self) -> bool {
        self.is_simple
    }
}

impl<'a> Default for SymbolicDictionary<'a> {
    fn default() -> Self {
        let mut s = Self::new();
        s.add_numbers();
        s
    }
}
