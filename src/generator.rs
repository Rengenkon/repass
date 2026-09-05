use std::borrow::Borrow;
use std::collections::{BTreeMap, Bound};

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

pub struct Dictionary {
    dictionary: Vec<String>,
}

impl Dictionary {
    pub fn new() -> Self {
        Dictionary {
            dictionary: Vec::new(),
        }
    }

    pub fn add_numbers(self: Self) {
        todo!()
    }

    fn numbers(self: Self) -> &'static [char] {
        &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']
    }

    fn lowercase() -> &'static [char] {
        &[
            'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
            'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
        ]
    }

    fn uppercase() -> &'static [char] {
        &[
            'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
            'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
        ]
    }

    fn special_punctuation() -> &'static [char] {
        &['!', '?', '.', ',', ';', ':']
    }

    fn special_math() -> &'static [char] {
        &['_', '-', '@', '=', '+', '*', '/']
    }

    fn special_brackets() -> &'static [char] {
        &['(', ')', '[', ']', '{', '}']
    }

    fn special_quotes() -> &'static [char] {
        &['"', '\'', '`', '&']
    }

    fn special_hash_percent() -> &'static [char] {
        &['#', '$', '%', '^']
    }

    fn special_escape() -> &'static [char] {
        &['\\', '|', '~']
    }
}

struct Generator {}

impl Generator {
    fn generate(dictionary: Dictionary) -> Vec<String> {}
}