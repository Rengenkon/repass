use crate::StorageError;
use crate::persistence::PersistedRecord;
use crate::record::types::Timestamp;
use crate::tags::TagId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

pub mod types;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RecordId(u64);

impl RecordId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

impl Display for RecordId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for RecordId {
    type Err = std::num::ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

/// In-memory records with an ephemeral position index rebuilt on load.
#[derive(Clone)]
pub(crate) struct Records {
    rows: Vec<Record>,
    positions: HashMap<RecordId, usize>,
    next_id: Option<u64>,
}

/// A borrowed view of one SoA row. Password access is explicit so callers can
/// avoid including it in list output by default.
#[derive(Clone, Eq, PartialEq)]
pub struct RecordView<'a> {
    pub id: RecordId,
    pub name: &'a str,
    pub username: Option<&'a str>,
    pub url: Option<&'a str>,
    pub notes: Option<&'a str>,
    pub tags: &'a [TagId],
    pub created: &'a Timestamp,
    pub updated: &'a Timestamp,
    password: &'a str,
}

impl RecordView<'_> {
    pub fn password(&self) -> &str {
        self.password
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum FieldUpdate<T> {
    #[default]
    Keep,
    Set(T),
    Clear,
}

#[derive(Clone, Default, Eq, PartialEq)]
pub struct RecordPatch {
    pub name: Option<String>,
    pub password: Option<String>,
    pub username: FieldUpdate<String>,
    pub url: FieldUpdate<String>,
    pub notes: FieldUpdate<String>,
    pub add_tags: Vec<TagId>,
    pub remove_tags: Vec<TagId>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct NewRecord {
    pub name: String,
    pub password: String,
    pub username: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<TagId>,
}

#[derive(Clone)]
pub(crate) struct Record {
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

impl Records {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn from_persisted(
        records: Vec<PersistedRecord>,
        next_id: Option<u64>,
    ) -> Result<Self, StorageError> {
        let rows = records.into_iter().map(Record::from).collect();
        Self::from_records(rows, next_id)
    }

    pub(crate) fn from_records(
        rows: Vec<Record>,
        next_id: Option<u64>,
    ) -> Result<Self, StorageError> {
        let mut positions = HashMap::with_capacity(rows.len());
        let mut maximum = None;
        for (position, record) in rows.iter().enumerate() {
            if positions.contains_key(&record.id) {
                return Err(StorageError::DuplicateRecordId(record.id));
            }
            if record.name.is_empty() {
                return Err(StorageError::InvalidState("record name cannot be empty"));
            }
            validate_unique_tags(&record.tags)?;
            maximum = Some(maximum.map_or(record.id.0, |current: u64| current.max(record.id.0)));
            positions.insert(record.id, position);
        }
        if maximum.is_some_and(|maximum| next_id.is_some_and(|next| next <= maximum)) {
            return Err(StorageError::InvalidState(
                "next record ID must be greater than all stored record IDs",
            ));
        }
        Ok(Self {
            rows,
            positions,
            next_id,
        })
    }

    pub fn get(&self, id: RecordId) -> Option<RecordView<'_>> {
        self.positions
            .get(&id)
            .copied()
            .map(|position| self.view_at(position))
    }

    pub fn views(&self) -> Vec<RecordView<'_>> {
        let mut records: Vec<_> = self.rows.iter().collect();
        records.sort_unstable_by_key(|record| record.id);
        records.into_iter().map(Self::view_of).collect()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn find_by_name(&self, name: &str) -> Vec<RecordId> {
        self.rows
            .iter()
            .filter_map(|record| (record.name == name).then_some(record.id))
            .collect()
    }

    /// Finds records that contain every requested tag. An empty query matches
    /// every record.
    pub fn find_by_tags(&self, tags: &[TagId]) -> Vec<RecordId> {
        self.rows
            .iter()
            .filter_map(|record| {
                tags.iter()
                    .all(|tag| record.tags.contains(tag))
                    .then_some(record.id)
            })
            .collect()
    }

    pub(crate) fn create(
        &mut self,
        record: NewRecord,
        timestamp: Timestamp,
    ) -> Result<RecordId, StorageError> {
        if record.name.is_empty() {
            return Err(StorageError::InvalidState("record name cannot be empty"));
        }
        let record_tags = unique_tags(record.tags)?;
        let value = self.next_id.ok_or(StorageError::RecordIdExhausted)?;
        let id = RecordId(value);
        let position = self.rows.len();
        self.rows.push(Record {
            id,
            name: record.name,
            password: record.password,
            username: record.username,
            url: record.url,
            notes: record.notes,
            tags: record_tags,
            created: timestamp,
            updated: timestamp,
        });
        self.positions.insert(id, position);
        self.next_id = value.checked_add(1);
        Ok(id)
    }

    pub(crate) fn remove(&mut self, id: RecordId) -> Result<(), StorageError> {
        let position = self
            .positions
            .remove(&id)
            .ok_or(StorageError::RecordNotFound(id))?;
        self.rows.swap_remove(position);
        if let Some(moved_record) = self.rows.get(position) {
            self.positions.insert(moved_record.id, position);
        }
        Ok(())
    }

    pub(crate) fn update(
        &mut self,
        id: RecordId,
        patch: &RecordPatch,
        timestamp: Timestamp,
    ) -> Result<bool, StorageError> {
        let position = *self
            .positions
            .get(&id)
            .ok_or(StorageError::RecordNotFound(id))?;
        let add = unique_tags(patch.add_tags.clone())?;
        let remove = unique_tags(patch.remove_tags.clone())?;
        if let Some(conflict) = add.iter().find(|tag| remove.contains(tag)) {
            return Err(StorageError::ConflictingTagUpdate(*conflict));
        }
        if patch.name.as_ref().is_some_and(String::is_empty) {
            return Err(StorageError::InvalidState("record name cannot be empty"));
        }

        let mut changed = false;
        let record = &mut self.rows[position];
        if let Some(name) = &patch.name {
            if record.name != *name {
                record.name.clone_from(name);
                changed = true;
            }
        }
        if let Some(password) = &patch.password {
            if record.password != *password {
                record.password.clone_from(password);
                changed = true;
            }
        }
        changed |= apply_field(&mut record.username, &patch.username);
        changed |= apply_field(&mut record.url, &patch.url);
        changed |= apply_field(&mut record.notes, &patch.notes);

        for tag in add {
            if !record.tags.contains(&tag) {
                record.tags.push(tag);
                changed = true;
            }
        }
        for tag in remove {
            let length = record.tags.len();
            record.tags.retain(|existing| *existing != tag);
            changed |= record.tags.len() != length;
        }
        if changed {
            record.updated = timestamp;
        }
        Ok(changed)
    }

    pub(crate) fn uses_tag(&self, tag: TagId) -> bool {
        self.rows.iter().any(|record| record.tags.contains(&tag))
    }

    pub(crate) fn persisted(&self) -> Vec<PersistedRecord> {
        self.rows.iter().map(PersistedRecord::from).collect()
    }

    pub(crate) fn next_id(&self) -> Option<u64> {
        self.next_id
    }

    pub(crate) fn used_tag_ids(&self) -> Vec<TagId> {
        self.rows
            .iter()
            .flat_map(|record| record.tags.iter())
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn view_at(&self, position: usize) -> RecordView<'_> {
        Self::view_of(&self.rows[position])
    }

    fn view_of(record: &Record) -> RecordView<'_> {
        RecordView {
            id: record.id,
            name: &record.name,
            username: record.username.as_deref(),
            url: record.url.as_deref(),
            notes: record.notes.as_deref(),
            tags: &record.tags,
            created: &record.created,
            updated: &record.updated,
            password: &record.password,
        }
    }
}

impl From<PersistedRecord> for Record {
    fn from(record: PersistedRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            password: record.password,
            username: record.username,
            url: record.url,
            notes: record.notes,
            tags: record.tags,
            created: record.created,
            updated: record.updated,
        }
    }
}

impl From<&Record> for PersistedRecord {
    fn from(record: &Record) -> Self {
        Self {
            id: record.id,
            name: record.name.clone(),
            password: record.password.clone(),
            username: record.username.clone(),
            url: record.url.clone(),
            notes: record.notes.clone(),
            tags: record.tags.clone(),
            created: record.created,
            updated: record.updated,
        }
    }
}

impl Default for Records {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            positions: HashMap::new(),
            next_id: Some(1),
        }
    }
}

fn validate_unique_tags(tags: &[TagId]) -> Result<(), StorageError> {
    for (index, tag) in tags.iter().enumerate() {
        if tags[..index].contains(tag) {
            return Err(StorageError::DuplicateRecordTag(*tag));
        }
    }
    Ok(())
}

fn unique_tags(tags: Vec<TagId>) -> Result<Vec<TagId>, StorageError> {
    validate_unique_tags(&tags)?;
    Ok(tags)
}

fn apply_field(target: &mut Option<String>, update: &FieldUpdate<String>) -> bool {
    let replacement = match update {
        FieldUpdate::Keep => return false,
        FieldUpdate::Set(value) => Some(value.clone()),
        FieldUpdate::Clear => None,
    };
    if *target == replacement {
        false
    } else {
        *target = replacement;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_record(name: &str) -> NewRecord {
        NewRecord {
            name: name.to_owned(),
            password: format!("{name}-secret"),
            username: None,
            url: None,
            notes: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn swap_remove_keeps_records_and_stable_id_index_consistent() {
        let now = Timestamp::from_unix_millis(10);
        let mut records = Records::new();
        let first = records.create(new_record("first"), now.clone()).unwrap();
        let middle = records.create(new_record("middle"), now.clone()).unwrap();
        let last = records.create(new_record("last"), now.clone()).unwrap();

        records.remove(middle).unwrap();
        assert!(records.get(middle).is_none());
        assert_eq!(records.get(first).unwrap().name, "first");
        assert_eq!(records.get(last).unwrap().name, "last");
        assert_eq!(records.find_by_name("last"), vec![last]);
        assert!(records.index_is_aligned());
    }

    #[test]
    fn patch_changes_only_requested_fields_and_rejects_conflicting_tags() {
        let now = Timestamp::from_unix_millis(10);
        let mut records = Records::new();
        let id = records.create(new_record("mail"), now.clone()).unwrap();
        records
            .update(
                id,
                &RecordPatch {
                    username: FieldUpdate::Set("user".into()),
                    ..RecordPatch::default()
                },
                Timestamp::from_unix_millis(20),
            )
            .unwrap();
        let view = records.get(id).unwrap();
        assert_eq!(view.name, "mail");
        assert_eq!(view.username, Some("user"));
        assert_eq!(view.password(), "mail-secret");
        assert_eq!(view.updated.as_unix_millis(), 20);
        assert!(matches!(
            records.update(
                id,
                &RecordPatch {
                    add_tags: vec![TagId::new(1)],
                    remove_tags: vec![TagId::new(1)],
                    ..RecordPatch::default()
                },
                Timestamp::from_unix_millis(30)
            ),
            Err(StorageError::ConflictingTagUpdate(tag)) if tag == TagId::new(1)
        ));
        assert!(records.index_is_aligned());
    }

    #[test]
    fn persisted_records_reject_duplicate_ids_and_restore_id_sequence() {
        let now = Timestamp::from_unix_millis(10);
        let mut records = Records::new();
        let id = records.create(new_record("mail"), now).unwrap();
        let persisted = records.persisted();
        let restored = Records::from_persisted(persisted.clone(), records.next_id()).unwrap();
        assert_eq!(restored.get(id).unwrap().name, "mail");
        assert!(matches!(
            Records::from_persisted(vec![persisted[0].clone(), persisted[0].clone()], Some(2)),
            Err(StorageError::DuplicateRecordId(_))
        ));
        assert!(restored.index_is_aligned());
    }

    impl Records {
        fn index_is_aligned(&self) -> bool {
            self.positions.len() == self.rows.len()
                && self
                    .rows
                    .iter()
                    .enumerate()
                    .all(|(position, record)| self.positions.get(&record.id) == Some(&position))
        }
    }
}
