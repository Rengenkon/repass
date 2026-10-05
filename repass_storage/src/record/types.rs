use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Creates a timestamp from milliseconds since the Unix epoch.
    pub fn from_unix_millis(milliseconds: u64) -> Self {
        Self(milliseconds)
    }

    /// Returns milliseconds since the Unix epoch.
    pub fn as_unix_millis(&self) -> u64 {
        self.0
    }

    pub(crate) fn now() -> Result<Self, crate::StorageError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| crate::StorageError::Clock)?;
        let milliseconds =
            u64::try_from(duration.as_millis()).map_err(|_| crate::StorageError::Clock)?;
        Ok(Self(milliseconds))
    }
}
