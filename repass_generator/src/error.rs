use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratorError {
    InvalidGenerationLimits,
    BatchLimitExceeded {
        count: usize,
    },
    InvalidPasswordLengthRange {
        min_chars: usize,
        max_chars: usize,
    },
    ZeroPasswordLength,
    EmptyDictionary,
    EmptyDictionaryEntry,
    DuplicateDictionaryEntry(String),
    DictionaryValidationResourceLimit,
    InvalidDictionaryEntry {
        index: usize,
    },
    NoSeparationShape {
        target_chars: usize,
    },
    NoDictionaryCombination {
        target_chars: usize,
    },
    SeparatorCharacterCountMismatch {
        expected_chars: usize,
        actual_chars: usize,
    },
    ResourceLimit {
        target_chars: usize,
    },
    SearchLimitExceeded {
        target_chars: usize,
        limit: usize,
    },
    InvalidSeparator(SeparatorError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeparatorError {
    EmptySeparator,
    ZeroInterval,
    ZeroSeparatorCount,
    EmptyParts,
    EmptyPart,
    CharacterCountOverflow,
}

impl Display for GeneratorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidGenerationLimits => write!(f, "generation limits must be positive"),
            Self::BatchLimitExceeded { count } => write!(
                f,
                "batch of {count} passwords exceeds the configured output limits"
            ),
            Self::InvalidPasswordLengthRange {
                min_chars,
                max_chars,
            } => {
                write!(
                    f,
                    "invalid password character-count range {min_chars}..={max_chars}"
                )
            }
            Self::ZeroPasswordLength => {
                write!(f, "password character count must be greater than zero")
            }
            Self::EmptyDictionary => write!(f, "dictionary is empty"),
            Self::EmptyDictionaryEntry => write!(f, "dictionary entries must not be empty"),
            Self::DuplicateDictionaryEntry(entry) => {
                write!(f, "dictionary contains duplicate entry {entry:?}")
            }
            Self::DictionaryValidationResourceLimit => {
                write!(f, "not enough memory to validate dictionary entries")
            }
            Self::InvalidDictionaryEntry { index } => {
                write!(f, "dictionary does not provide an entry at index {index}")
            }
            Self::NoSeparationShape { target_chars } => {
                write!(
                    f,
                    "separator cannot produce a result of {target_chars} Unicode scalar values"
                )
            }
            Self::NoDictionaryCombination { target_chars } => {
                write!(
                    f,
                    "dictionary cannot form the content required for {target_chars} Unicode scalar values"
                )
            }
            Self::SeparatorCharacterCountMismatch {
                expected_chars,
                actual_chars,
            } => write!(
                f,
                "separator produced {actual_chars} Unicode scalar values, but its requirement promised {expected_chars}"
            ),
            Self::ResourceLimit { target_chars } => {
                write!(
                    f,
                    "requested password character count {target_chars} exceeds available generation resources"
                )
            }
            Self::SearchLimitExceeded {
                target_chars,
                limit,
            } => write!(
                f,
                "search for target character count {target_chars} exceeded the limit of {limit} states; simplify the dictionary or target"
            ),
            Self::InvalidSeparator(error) => Display::fmt(error, f),
        }
    }
}

impl std::error::Error for GeneratorError {}

impl Display for SeparatorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySeparator => write!(f, "separator must not be empty"),
            Self::ZeroInterval => write!(f, "separator interval must be greater than zero"),
            Self::ZeroSeparatorCount => write!(f, "separator count must be greater than zero"),
            Self::EmptyParts => write!(f, "at least one non-empty part is required"),
            Self::EmptyPart => write!(f, "parts must not contain empty strings"),
            Self::CharacterCountOverflow => {
                write!(f, "computed output character count overflows usize")
            }
        }
    }
}

impl std::error::Error for SeparatorError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictionaryError {
    DuplicateEntry(String),
    EmptyEntry,
    ResourceLimit,
}

impl Display for DictionaryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateEntry(entry) => write!(f, "dictionary already contains {entry:?}"),
            Self::EmptyEntry => write!(f, "dictionary entries must not be empty"),
            Self::ResourceLimit => write!(f, "not enough memory to add dictionary entries"),
        }
    }
}

impl std::error::Error for DictionaryError {}
