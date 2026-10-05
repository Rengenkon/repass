use crate::record::RecordId;
use crate::record::types::Timestamp;
use crate::tags::TagId;
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct PersistedRecord {
    pub id: RecordId,
    pub name: String,
    pub password: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<TagId>,
    pub created: Timestamp,
    pub updated: Timestamp,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct PersistedRecords {
    pub records: Vec<PersistedRecord>,
    pub next_record_id: Option<u64>,
}

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
