use crate::record::RecordId;
use crate::record::types::Timestamp;
use crate::tags::TagId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct PersistedRecord<S = String, T = Vec<TagId>> {
    pub id: RecordId,
    pub name: S,
    pub password: S,
    pub username: Option<S>,
    pub url: Option<S>,
    pub notes: Option<S>,
    pub tags: T,
    pub created: Timestamp,
    pub updated: Timestamp,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct PersistedRecords<R = PersistedRecord> {
    pub records: Vec<R>,
    pub next_record_id: Option<u64>,
}

/// Borrowed views reuse the same canonical field definitions and ordering.
pub(crate) type PersistedRecordRef<'a> = PersistedRecord<&'a str, &'a [TagId]>;
pub(crate) type PersistedRecordsRef<'a> = PersistedRecords<PersistedRecordRef<'a>>;

#[derive(Deserialize, Serialize)]
pub(crate) struct PersistedTag {
    pub id: TagId,
    pub name: String,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct PersistedTags {
    pub tags: Vec<PersistedTag>,
    pub next_tag_id: Option<TagId>,
}
