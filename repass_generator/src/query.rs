use crate::dictionary::cache::DictionaryCache;
use crate::error::GeneratorError;
use crate::query::Length::{Exact, Range};
use crate::separator::Separator;

pub enum Length {
    Range { min: usize, max: usize },
    Exact(usize),
}

impl Length {
    pub fn validate(&self) -> Result<(), GeneratorError> {
        match self {
            Range { min, max } if min > max => Err(GeneratorError::InvalidLengthRange {
                min: *min,
                max: *max,
            }),
            Range { min: 0, .. } | Exact(0) => Err(GeneratorError::ZeroLength),
            _ => Ok(()),
        }
    }
}

impl Default for Length {
    fn default() -> Self {
        Range { min: 12, max: 20 }
    }
}

pub struct Query<'a> {
    length: Length,
    dictionary: &'a DictionaryCache<'a>,
    separator: &'a dyn Separator,
}

impl<'a> Query<'a> {
    pub fn new(
        length: Length,
        dictionary: &'a DictionaryCache<'a>,
        separator: &'a dyn Separator,
    ) -> Result<Self, GeneratorError> {
        length.validate()?;
        if dictionary.is_empty() {
            return Err(GeneratorError::EmptyDictionary);
        }
        separator
            .validate()
            .map_err(GeneratorError::InvalidSeparator)?;
        Ok(Query {
            length,
            dictionary,
            separator,
        })
    }

    pub fn length(&self) -> &Length {
        &self.length
    }

    pub fn dictionary(&self) -> &'a DictionaryCache<'a> {
        self.dictionary
    }

    pub fn separator(&self) -> &'a dyn Separator {
        self.separator
    }
}

#[cfg(test)]
mod test {
    use crate::query::Length::{Exact, Range};

    #[test]
    fn hard() {
        let l = Exact(3);
        assert_eq!(l.validate(), Ok(()));
    }

    #[test]
    fn float() {
        let l = Range { min: 12, max: 20 };
        assert_eq!(l.validate(), Ok(()));
    }

    #[test]
    fn rejects_invalid_range() {
        assert!(Range { min: 9, max: 3 }.validate().is_err());
    }
}
