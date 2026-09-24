use crate::dictionary::cache::DictionaryCache;
use crate::error::GeneratorError;
use crate::query::PasswordLength::{Exact, Range};
use crate::separator::Separator;

/// Upper bounds for separator-shape enumeration and dictionary search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenerationLimits {
    max_shapes: usize,
    max_planner_states: usize,
}

impl GenerationLimits {
    pub fn new(max_shapes: usize, max_planner_states: usize) -> Self {
        Self {
            max_shapes,
            max_planner_states,
        }
    }

    pub fn max_shapes(&self) -> usize {
        self.max_shapes
    }

    pub fn max_planner_states(&self) -> usize {
        self.max_planner_states
    }
}

impl Default for GenerationLimits {
    fn default() -> Self {
        Self {
            max_shapes: 100_000,
            max_planner_states: 100_000,
        }
    }
}

pub enum PasswordLength {
    Range { min_chars: usize, max_chars: usize },
    Exact(usize),
}

impl PasswordLength {
    pub fn validate(&self) -> Result<(), GeneratorError> {
        match self {
            Range {
                min_chars,
                max_chars,
            } if min_chars > max_chars => Err(GeneratorError::InvalidPasswordLengthRange {
                min_chars: *min_chars,
                max_chars: *max_chars,
            }),
            Range { min_chars: 0, .. } | Exact(0) => Err(GeneratorError::ZeroPasswordLength),
            _ => Ok(()),
        }
    }
}

impl Default for PasswordLength {
    fn default() -> Self {
        Range {
            min_chars: 12,
            max_chars: 20,
        }
    }
}

pub struct Query<'a, 'entries> {
    password_length: PasswordLength,
    dictionary: &'a DictionaryCache<'entries>,
    separator: &'a dyn Separator,
    limits: GenerationLimits,
}

impl<'a, 'entries> Query<'a, 'entries> {
    pub fn new(
        password_length: PasswordLength,
        dictionary: &'a DictionaryCache<'entries>,
        separator: &'a dyn Separator,
    ) -> Result<Self, GeneratorError> {
        Self::with_limits(
            password_length,
            dictionary,
            separator,
            GenerationLimits::default(),
        )
    }

    pub fn with_limits(
        password_length: PasswordLength,
        dictionary: &'a DictionaryCache<'entries>,
        separator: &'a dyn Separator,
        limits: GenerationLimits,
    ) -> Result<Self, GeneratorError> {
        password_length.validate()?;
        if dictionary.is_empty() {
            return Err(GeneratorError::EmptyDictionary);
        }
        separator
            .validate()
            .map_err(GeneratorError::InvalidSeparator)?;
        Ok(Query {
            password_length,
            dictionary,
            separator,
            limits,
        })
    }

    pub fn password_length(&self) -> &PasswordLength {
        &self.password_length
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
    use crate::query::PasswordLength::{Exact, Range};

    #[test]
    fn accepts_exact_password_char_count() {
        let password_length = Exact(3);
        assert_eq!(password_length.validate(), Ok(()));
    }

    #[test]
    fn accepts_valid_password_char_count_range() {
        let password_length = Range {
            min_chars: 12,
            max_chars: 20,
        };
        assert_eq!(password_length.validate(), Ok(()));
    }

    #[test]
    fn rejects_invalid_range() {
        assert!(
            Range {
                min_chars: 9,
                max_chars: 3,
            }
            .validate()
            .is_err()
        );
    }
}
