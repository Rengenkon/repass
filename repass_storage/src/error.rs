use crate::VaultError;
use crate::record::{DataId, RecordId};
use crate::tags::TagId;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io;

#[derive(Debug)]
pub enum StorageError {
    Locked,
    Io(io::Error),
    Vault(VaultError),
    MetadataCountsUpdateAfterDataSave(VaultError),
    DuplicateRecordId(RecordId),
    RecordIdExhausted,
    RecordNotFound(RecordId),
    DataIdExhausted,
    DataNotFound(DataId),
    DuplicateDataId(DataId),
    ConflictingDataUpdate(DataId),
    DuplicateTagId(TagId),
    DuplicateTagName(String),
    TagIdExhausted,
    TagNotFound(TagId),
    TagInUse(TagId),
    TagCatalogUnavailable(String),
    DuplicateRecordTag(TagId),
    ConflictingTagUpdate(TagId),
    InvalidState(&'static str),
    IncompleteInitialization,
    AlreadyInitialized,
    LegacyFormat,
    Clock,
}

impl Display for StorageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Locked => formatter.write_str("vault is already open in another session"),
            Self::Io(error) => write!(formatter, "storage I/O error: {error}"),
            Self::Vault(error) => Display::fmt(error, formatter),
            Self::MetadataCountsUpdateAfterDataSave(error) => write!(
                formatter,
                "data was saved, but metadata counts could not be updated: {error}"
            ),
            Self::DuplicateRecordId(id) => write!(formatter, "duplicate record ID {id}"),
            Self::RecordIdExhausted => formatter.write_str("record ID space is exhausted"),
            Self::RecordNotFound(id) => write!(formatter, "record {id} was not found"),
            Self::DataIdExhausted => formatter.write_str("record data ID space is exhausted"),
            Self::DataNotFound(id) => write!(formatter, "record data {id} was not found"),
            Self::DuplicateDataId(id) => write!(formatter, "duplicate record data ID {id}"),
            Self::ConflictingDataUpdate(id) => {
                write!(formatter, "record data {id} was targeted more than once")
            }
            Self::DuplicateTagId(id) => write!(formatter, "duplicate tag ID {id}"),
            Self::DuplicateTagName(name) => write!(formatter, "tag name {name:?} already exists"),
            Self::TagIdExhausted => formatter.write_str("tag ID space is exhausted"),
            Self::TagNotFound(id) => write!(formatter, "tag {id} was not found"),
            Self::TagInUse(id) => {
                write!(formatter, "tag {id} is still used by one or more records")
            }
            Self::TagCatalogUnavailable(reason) => {
                write!(
                    formatter,
                    "tag catalog is unavailable: {reason}; explicit recovery is required"
                )
            }
            Self::DuplicateRecordTag(id) => {
                write!(formatter, "record contains tag {id} more than once")
            }
            Self::ConflictingTagUpdate(id) => {
                write!(formatter, "tag {id} cannot be both added and removed")
            }
            Self::InvalidState(reason) => write!(formatter, "invalid storage state: {reason}"),
            Self::IncompleteInitialization => formatter
                .write_str("storage initialization is incomplete; existing files were preserved"),
            Self::AlreadyInitialized => formatter.write_str("a vault is already initialized"),
            Self::LegacyFormat => formatter.write_str(
                "found legacy vault.repass; this format is unsupported and was left unchanged",
            ),
            Self::Clock => {
                formatter.write_str("system clock is outside the supported timestamp range")
            }
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Vault(error) => Some(error),
            Self::MetadataCountsUpdateAfterDataSave(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for StorageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<VaultError> for StorageError {
    fn from(error: VaultError) -> Self {
        Self::Vault(error)
    }
}
