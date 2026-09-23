use crate::dictionary::cache::DictionaryCache;
use crate::error::GeneratorError;
use crate::query::Length::{Exact, Range};
use crate::separator::Separator;

/// Upper bounds for separator-shape enumeration and dictionary search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationLimits {
    pub max_shapes: usize,
    pub max_planner_states: usize,
}

impl Default for GenerationLimits {
    fn default() -> Self {
        Self {
            max_shapes: 100_000,
            max_planner_states: 100_000,
        }
    }
}

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

pub struct Query<'a, 'entries> {
    length: Length,
    dictionary: &'a DictionaryCache<'entries>,
    separator: &'a dyn Separator,
    limits: GenerationLimits,
}

impl<'a, 'entries> Query<'a, 'entries> {
    pub fn new(
        length: Length,
        dictionary: &'a DictionaryCache<'entries>,
        separator: &'a dyn Separator,
    ) -> Result<Self, GeneratorError> {
        Self::with_limits(length, dictionary, separator, GenerationLimits::default())
    }

    pub fn with_limits(
        length: Length,
        dictionary: &'a DictionaryCache<'entries>,
        separator: &'a dyn Separator,
        limits: GenerationLimits,
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
            limits,
        })
    }

    pub fn length(&self) -> &Length {
        &self.length
    }

    pub fn dictionary(&self) -> &'a DictionaryCache<'entries> {
        self.dictionary
    }

    pub fn separator(&self) -> &'a dyn Separator {
        self.separator
    }

    pub fn limits(&self) -> GenerationLimits {
        self.limits
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
