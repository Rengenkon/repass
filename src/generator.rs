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

        
}

struct Generator {}

impl Generator {
    fn generate(dictionary: Dictionary) -> Vec<String> {}
}