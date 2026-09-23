use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratorError {
    InvalidLengthRange { min: usize, max: usize },
    ZeroLength,
    EmptyDictionary,
    EmptyDictionaryEntry,
    InvalidDictionaryEntry { index: usize },
    NoSeparationShape { target: usize },
    NoDictionaryCombination { target: usize },
    SeparatorLengthMismatch { expected: usize, actual: usize },
    ResourceLimit { target: usize },
    SearchLimitExceeded { target: usize, limit: usize },
    InvalidSeparator(SeparatorError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeparatorError {
    EmptySeparator,
    ZeroInterval,
    ZeroCount,
    EmptyParts,
    EmptyPart,
    LengthOverflow,
}

impl Display for GeneratorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidLengthRange { min, max } => {
                write!(f, "invalid length range {min}..={max}")
            }
            Self::ZeroLength => write!(f, "password length must be greater than zero"),
            Self::EmptyDictionary => write!(f, "dictionary is empty"),
            Self::EmptyDictionaryEntry => write!(f, "dictionary entries must not be empty"),
            Self::InvalidDictionaryEntry { index } => {
                write!(f, "dictionary does not provide an entry at index {index}")
            }
            Self::NoSeparationShape { target } => {
                write!(f, "separator cannot produce a result of length {target}")
            }
            Self::NoDictionaryCombination { target } => {
                write!(
                    f,
                    "dictionary cannot form the content required for length {target}"
                )
            }
            Self::SeparatorLengthMismatch { expected, actual } => write!(
                f,
                "separator produced length {actual}, but its requirement promised {expected}"
            ),
            Self::ResourceLimit { target } => {
                write!(
                    f,
                    "requested length {target} exceeds available generation resources"
                )
            }
            Self::SearchLimitExceeded { target, limit } => write!(
                f,
                "search for length {target} exceeded the limit of {limit} states; simplify the dictionary or target"
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
            Self::ZeroCount => write!(f, "separator count must be greater than zero"),
            Self::EmptyParts => write!(f, "at least one non-empty part is required"),
            Self::EmptyPart => write!(f, "parts must not contain empty strings"),
            Self::LengthOverflow => write!(f, "computed output length overflows usize"),
        }
    }
}

impl std::error::Error for SeparatorError {}
