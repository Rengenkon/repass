use crate::separator::Separator;
use std::borrow::Borrow;
use std::collections::{BTreeMap, Bound};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub trait FuzzyGet<K, V> {
    fn get_left<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: Ord;

    fn get_right<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: Ord;
}

impl<K, V> FuzzyGet<K, V> for BTreeMap<K, V> {
    fn get_left<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        self.range((Bound::Unbounded, Bound::Included(key)))
            .next_back()
            .map(|(_, value)| value)
    }

    fn get_right<Q: ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q> + Ord,
        Q: Ord,
    {
        self.range((Bound::Included(key), Bound::Unbounded))
            .next()
            .map(|(_, value)| value)
    }
}

trait Dictionary {
    fn get(self: &Self, index: usize) -> Option<&str>;

    fn len(self: &Self) -> usize;
    
    /// Simple dictionary - dictionary where every element is one char
    fn is_simple(self: &Self) -> bool;
}

pub struct FileDictionary {
    dictionary: Vec<String>,
    case_change: bool, // todo
    is_simple: bool,
}

impl Dictionary for FileDictionary {
    fn get(self: &Self, index: usize) -> Option<&str> {
        let x = self.dictionary.get(index)?;
        Some(x.as_str())
    }

    fn len(self: &Self) -> usize {
        self.dictionary.len()
    }

    fn is_simple(self: &Self) -> bool {
        self.is_simple
    }
}

impl FileDictionary {
    pub fn new(path: Box<Path>) -> Self {
        let file = File::open(path);
        let mut reader = BufReader::new(file.unwrap());
        let mut dictionary = Vec::new();
        let mut is_simple = true;
        loop {
            let mut buf = String::new();
            let n = reader.read_line(&mut buf).unwrap();
            if is_simple {
                if buf.len() > 1 {
                    is_simple = false;
                }
            }
            if n == 0 {
                break;
            }
            dictionary.push(buf);
        }
        FileDictionary {
            dictionary,
            case_change: false,
            is_simple
        }
    }
}

pub struct SymbolicDictionary<'a> {
    length: usize,
    dictionary: BTreeMap<usize, &'a [&'a str]>,
    is_simple: bool,
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

impl<'a> SymbolicDictionary<'a> {
    pub fn new() -> Self {
        SymbolicDictionary{
            length: 0,
            dictionary: BTreeMap::new(),
            is_simple: true,
        }
    }

    // todo add value several times
    pub fn add(self: &mut Self, value: &'a [&'a str]) {
        self.dictionary.insert(self.length, value);
        self.length += value.len();
        let long_value = value.iter().any(|v| {v.len() > 1});
        if long_value {
            self.is_simple = false;
        }
    }

    pub fn add_numbers(self: &mut Self) -> &mut Self{
        self.add(numbers());
        self
    }
    
    // todo add methods
}

impl<'a> Default for SymbolicDictionary<'a> {
    fn default() -> Self {
        let mut s = Self::new();
        s.add_numbers();
        s
    }
}

fn numbers() -> &'static [&'static str] {
    &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
}

fn lowercase() -> &'static [&'static str] {
    &[
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q",
        "r", "s", "t", "u", "v", "w", "x", "y", "z",
    ]
}

fn uppercase() -> &'static [&'static str] {
    &[
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q",
        "R", "S", "T", "U", "V", "W", "X", "Y", "Z",
    ]
}

fn special_punctuation() -> &'static [&'static str] {
    &["!", "?", ".", ",", ";", ":"]
}

fn special_math() -> &'static [&'static str] {
    &["_", "-", "@", "=", "+", "*", "/"]
}

fn special_brackets() -> &'static [&'static str] {
    &["(", ")", "[", "]", "{", "}"]
}

fn special_quotes() -> &'static [&'static str] {
    &["\"", "'", "`", "&"]
}

fn special_hash_percent() -> &'static [&'static str] {
    &["#", "$", "%", "^"]
}

fn special_escape() -> &'static [&'static str] {
    &["\\", "|", "~"]
}

pub enum Length {
    Float(usize, usize),
    Hard(usize),
}

impl Length {
    pub fn to_one_size(self: &Self) -> usize {
        match self {
            Length::Float(min, max) => rand::random_range(*min..=*max),
            Length::Hard(value) => *value,
        }
    }
}

pub struct Query<'a> {
    length: Length,
    dictionary: Box<&'a dyn Dictionary>,
    separator: Box<&'a dyn Separator>,
}

impl<'a> Query<'a> {
    pub fn new(
        length: Length,
        dictionary: Box<&'a dyn Dictionary>,
        separator: Box<&'a dyn Separator>,
    ) -> Self {
        Query {
            length,
            dictionary,
            separator,
        }
    }
}

/// Generate sequence of parts from `dictionary`
/// not guarantee that summary length of returned value equals `space`
fn generate_parts<'a>(dict: &Box<&'a dyn Dictionary>, space: usize) -> Vec<&'a str> {
    let len = dict.len();
    let mut result = Vec::new();
    let mut none_count = 0;
    let mut current_len = 0;
    while current_len < space {
        let index = rand::random_range(0..len);
        match dict.get(index) {
            None => none_count += 1,
            Some(value) => {
                result.push(value);
                current_len += value.len();
            }
        }
        if none_count > 100 {
            panic!("Not valid dictionary: attempts to get element is unsuccessful")
        }
    }
    result
}

fn generate(dictionary: &Box<&dyn Dictionary>, separator: &Box<&dyn Separator>, space: usize) -> String {
    let parts = generate_parts(&dictionary, space);
    separator.add_separator(parts.as_slice())
}

pub fn generate_once(query: &Query) -> String {
    let length = query.length.to_one_size();
    let space = query.separator.free_space(length);
    generate(&query.dictionary, &query.separator, space)
}

pub fn generate_multi(query: &Query, count: usize) -> Vec<String> {
    let length = query.length.to_one_size();
    let space = query.separator.free_space(length);
    let mut result = Vec::new();
    for _ in 0..count {
        let generate = generate(&query.dictionary, &query.separator, space);
        result.push(generate);
    }
    result
}
