use crate::record::types::{Host, Timestamp};
use crate::record::{RecordData, RecordId};
use crate::tags::TagId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct PersistedRecord<S = String, T = Vec<TagId>, D = Vec<RecordData>, H = Host> {
    pub id: RecordId,
    pub name: S,
    pub data: D,
    pub next_data_id: Option<u64>,
    pub username: Option<S>,
    pub host: Option<H>,
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
pub(crate) type PersistedRecordRef<'a> =
    PersistedRecord<&'a str, &'a [TagId], &'a [RecordData], &'a Host>;
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
