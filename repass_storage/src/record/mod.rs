use crate::StorageError;
use crate::persistence::{PersistedRecord, PersistedRecordRef, PersistedRecordsRef};
use crate::record::types::{Data, Host, Timestamp};
use crate::tags::TagId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
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

/// Stable within its parent record; deleted IDs are never reused.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct DataId(u64);

impl DataId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }
    pub fn value(self) -> u64 {
        self.0
    }
}

impl Display for DataId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for DataId {
    type Err = std::num::ParseIntError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

#[derive(Clone, Deserialize, Serialize, Eq, PartialEq)]
pub struct RecordData {
    pub id: DataId,
    pub value: Data,
}

/// In-memory records with an ephemeral position index rebuilt on load.
#[derive(Clone)]
pub(crate) struct Records {
    rows: Vec<Record>,
    positions: HashMap<RecordId, usize>,
    next_id: Option<u64>,
}

/// A borrowed view. Secret access is explicit and no Debug implementation
/// includes record contents.
#[derive(Clone, Eq, PartialEq)]
pub struct RecordView<'a> {
    pub id: RecordId,
    pub name: &'a str,
    pub username: Option<&'a str>,
    pub host: Option<&'a Host>,
    pub notes: Option<&'a str>,
    pub tags: &'a [TagId],
    pub created: &'a Timestamp,
    pub updated: &'a Timestamp,
    data: &'a [RecordData],
}

impl RecordView<'_> {
    pub fn data(&self) -> &[RecordData] {
        self.data
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
    pub add_data: Vec<Data>,
    pub replace_data: Vec<(DataId, Data)>,
    pub remove_data: Vec<DataId>,
    pub username: FieldUpdate<String>,
    pub host: FieldUpdate<Host>,
    pub notes: FieldUpdate<String>,
    pub add_tags: Vec<TagId>,
    pub remove_tags: Vec<TagId>,
}

#[derive(Clone, Eq, PartialEq)]
pub struct NewRecord {
    pub name: String,
    pub data: Vec<Data>,
    pub username: Option<String>,
    pub host: Option<Host>,
    pub notes: Option<String>,
    pub tags: Vec<TagId>,
}

#[derive(Clone)]
pub(crate) struct Record {
    pub id: RecordId,
    pub name: String,
    pub data: Vec<RecordData>,
    pub next_data_id: Option<u64>,
    pub username: Option<String>,
    pub host: Option<Host>,
    pub notes: Option<String>,
    pub tags: Vec<TagId>,
    pub created: Timestamp,
    pub updated: Timestamp,
}

impl Records {
    pub fn new() -> Self {
        Self::default()
    }

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
            validate_data(&record.data, record.next_data_id)?;
            if let Some(host) = &record.host {
                host.validate()?;
            }
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

    pub(crate) fn filtered_views(
        &self,
        name: Option<&str>,
        host: Option<&Host>,
        tags: &[TagId],
    ) -> Vec<RecordView<'_>> {
        let mut records: Vec<_> = self
            .rows
            .iter()
            .filter(|record| {
                name.is_none_or(|name| record.name == name)
                    && host.is_none_or(|host| {
                        record
                            .host
                            .as_ref()
                            .is_some_and(|stored| stored.matches(host))
                    })
                    && tags.iter().all(|tag| record.tags.contains(tag))
            })
            .collect();
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
        if let Some(host) = &record.host {
            host.validate()?;
        }
        let (data, next_data_id) = append_data(Vec::new(), Some(1), &record.data)?;
        let value = self.next_id.ok_or(StorageError::RecordIdExhausted)?;
        let id = RecordId(value);
        let position = self.rows.len();
        self.rows.push(Record {
            id,
            name: record.name,
            data,
            next_data_id,
            username: record.username,
            host: record.host,
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
        self.take(id).map(|_| ())
    }

    pub(crate) fn take(&mut self, id: RecordId) -> Result<Record, StorageError> {
        let position = self
            .positions
            .remove(&id)
            .ok_or(StorageError::RecordNotFound(id))?;
        let removed = self.rows.swap_remove(position);
        if let Some(moved_record) = self.rows.get(position) {
            self.positions.insert(moved_record.id, position);
        }
        Ok(removed)
    }

    pub(crate) fn snapshot(&self, id: RecordId) -> Result<Record, StorageError> {
        let position = self
            .positions
            .get(&id)
            .ok_or(StorageError::RecordNotFound(id))?;
        Ok(self.rows[*position].clone())
    }

    pub(crate) fn restore(&mut self, record: Record) {
        if let Some(position) = self.positions.get(&record.id).copied() {
            self.rows[position] = record;
        } else {
            self.positions.insert(record.id, self.rows.len());
            self.rows.push(record);
        }
    }

    pub(crate) fn rollback_create(
        &mut self,
        id: RecordId,
        next: Option<u64>,
    ) -> Result<(), StorageError> {
        self.remove(id)?;
        self.next_id = next;
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

        // Build and validate a candidate before changing any in-memory state.
        let mut candidate = self.rows[position].clone();
        let record = &mut candidate;
        let mut touched = HashSet::new();
        for id in patch
            .replace_data
            .iter()
            .map(|(id, _)| id)
            .chain(&patch.remove_data)
        {
            if !touched.insert(*id) {
                return Err(StorageError::ConflictingDataUpdate(*id));
            }
            if !record.data.iter().any(|entry| entry.id == *id) {
                return Err(StorageError::DataNotFound(*id));
            }
        }
        for (id, value) in &patch.replace_data {
            value.validate()?;
            let entry = record
                .data
                .iter_mut()
                .find(|entry| entry.id == *id)
                .ok_or(StorageError::DataNotFound(*id))?;
            entry.value = value.clone();
        }
        record
            .data
            .retain(|entry| !patch.remove_data.contains(&entry.id));
        let (data, next) = append_data(
            std::mem::take(&mut record.data),
            record.next_data_id,
            &patch.add_data,
        )?;
        record.data = data;
        record.next_data_id = next;
        let mut changed = record.data != self.rows[position].data;
        if let Some(name) = &patch.name
            && record.name != *name
        {
            record.name.clone_from(name);
            changed = true;
        }
        changed |= apply_field(&mut record.username, &patch.username);
        if let FieldUpdate::Set(host) = &patch.host {
            host.validate()?;
        }
        changed |= apply_field(&mut record.host, &patch.host);
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
        self.rows[position] = candidate;
        Ok(changed)
    }

    pub(crate) fn uses_tag(&self, tag: TagId) -> bool {
        self.rows.iter().any(|record| record.tags.contains(&tag))
    }

    #[cfg(test)]
    pub(crate) fn persisted(&self) -> Vec<PersistedRecord> {
        self.rows.iter().map(PersistedRecord::from).collect()
    }

    pub(crate) fn persisted_view(&self) -> PersistedRecordsRef<'_> {
        PersistedRecordsRef {
            records: self
                .rows
                .iter()
                .map(|record| PersistedRecordRef {
                    id: record.id,
                    name: &record.name,
                    data: &record.data,
                    next_data_id: record.next_data_id,
                    username: record.username.as_deref(),
                    host: record.host.as_ref(),
                    notes: record.notes.as_deref(),
                    tags: &record.tags,
                    created: record.created,
                    updated: record.updated,
                })
                .collect(),
            next_record_id: self.next_id,
        }
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
            host: record.host.as_ref(),
            notes: record.notes.as_deref(),
            tags: &record.tags,
            created: &record.created,
            updated: &record.updated,
            data: &record.data,
        }
    }
}

impl From<PersistedRecord> for Record {
    fn from(record: PersistedRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            data: record.data,
            next_data_id: record.next_data_id,
            username: record.username,
            host: record.host,
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
            data: record.data.clone(),
            next_data_id: record.next_data_id,
            username: record.username.clone(),
            host: record.host.clone(),
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
    let mut seen = HashSet::with_capacity(tags.len());
    for tag in tags {
        if !seen.insert(*tag) {
            return Err(StorageError::DuplicateRecordTag(*tag));
        }
    }
    Ok(())
}

fn unique_tags(tags: Vec<TagId>) -> Result<Vec<TagId>, StorageError> {
    validate_unique_tags(&tags)?;
    Ok(tags)
}

fn validate_data(data: &[RecordData], next: Option<u64>) -> Result<(), StorageError> {
    let mut seen = HashSet::new();
    for entry in data {
        if !seen.insert(entry.id) {
            return Err(StorageError::DuplicateDataId(entry.id));
        }
        entry.value.validate()?;
        if next.is_some_and(|next| next <= entry.id.0) {
            return Err(StorageError::InvalidState(
                "next data ID must exceed all stored data IDs",
            ));
        }
    }
    Ok(())
}

fn append_data(
    mut data: Vec<RecordData>,
    mut next: Option<u64>,
    additions: &[Data],
) -> Result<(Vec<RecordData>, Option<u64>), StorageError> {
    for value in additions {
        value.validate()?;
        let id = next.ok_or(StorageError::DataIdExhausted)?;
        data.push(RecordData {
            id: DataId(id),
            value: value.clone(),
        });
        next = id.checked_add(1);
    }
    Ok((data, next))
}

fn apply_field<T: Clone + PartialEq>(target: &mut Option<T>, update: &FieldUpdate<T>) -> bool {
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
            data: vec![Data::Password(format!("{name}-secret"))],
            username: None,
            host: None,
            notes: None,
            tags: Vec::new(),
        }
    }

    #[test]
    fn swap_remove_keeps_records_and_stable_id_index_consistent() {
        let now = Timestamp::from_unix_millis(10);
        let mut records = Records::new();
        let first = records.create(new_record("first"), now).unwrap();
        let middle = records.create(new_record("middle"), now).unwrap();
        let last = records.create(new_record("last"), now).unwrap();

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
        let id = records.create(new_record("mail"), now).unwrap();
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
        assert!(matches!(&view.data()[0].value, Data::Password(value) if value == "mail-secret"));
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

    #[test]
    fn borrowed_and_owned_schema_views_round_trip_with_identical_bytes() {
        let mut records = Records::new();
        let mut record = new_record("猫-mail");
        record.username = Some("é-user".into());
        record.notes = Some("notes".into());
        record.host = Some("2001:db8::1".parse().unwrap());
        record.data.extend([
            Data::SshKey(
                crate::SshKey::new(Some("private\r\n猫\r\n".into()), Some("public\n".into()))
                    .unwrap(),
            ),
            Data::Totp(crate::Totp::new("MY".into(), crate::TotpAlgorithm::Sha256, 8, 60).unwrap()),
            Data::Code("code".into()),
        ]);
        record.tags = vec![TagId::new(3), TagId::new(300)];
        records
            .create(record, Timestamp::from_unix_millis(123))
            .unwrap();
        let owned = crate::persistence::PersistedRecords {
            records: records.persisted(),
            next_record_id: records.next_id(),
        };
        let borrowed = postcard::to_allocvec(&records.persisted_view()).unwrap();
        assert_eq!(borrowed, postcard::to_allocvec(&owned).unwrap());
        let restored: crate::persistence::PersistedRecords =
            postcard::from_bytes(&borrowed).unwrap();
        let restored = Records::from_persisted(restored.records, restored.next_record_id).unwrap();
        assert!(
            matches!(&restored.get(RecordId::new(1)).unwrap().data()[0].value, Data::Password(value) if value == "猫-mail-secret")
        );
    }

    #[test]
    fn data_mutations_use_stable_ids_and_validate_before_changing_any_fields() {
        let mut records = Records::new();
        let now = Timestamp::from_unix_millis(10);
        let id = records.create(new_record("mail"), now).unwrap();
        let first = records.get(id).unwrap().data()[0].id;
        records
            .update(
                id,
                &RecordPatch {
                    add_data: vec![Data::Code("a".into()), Data::Code("b".into())],
                    ..RecordPatch::default()
                },
                Timestamp::from_unix_millis(20),
            )
            .unwrap();
        let entries = records.get(id).unwrap().data().to_vec();
        let second = entries[1].id;
        let third = entries[2].id;
        records
            .update(
                id,
                &RecordPatch {
                    replace_data: vec![(third, Data::Code("replacement".into()))],
                    remove_data: vec![second],
                    add_data: vec![Data::Code("new".into())],
                    ..RecordPatch::default()
                },
                Timestamp::from_unix_millis(30),
            )
            .unwrap();
        let entries = records.get(id).unwrap().data().to_vec();
        assert_eq!(
            entries.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            vec![first, third, DataId::new(4)]
        );
        assert!(
            !records
                .update(
                    id,
                    &RecordPatch {
                        replace_data: vec![(third, Data::Code("replacement".into()))],
                        ..RecordPatch::default()
                    },
                    Timestamp::from_unix_millis(40)
                )
                .unwrap()
        );
        assert_eq!(records.get(id).unwrap().updated.as_unix_millis(), 30);
        for patch in [
            RecordPatch {
                name: Some("should not change".into()),
                replace_data: vec![(
                    first,
                    Data::SshKey(crate::SshKey {
                        private_key: None,
                        public_key: None,
                    }),
                )],
                ..RecordPatch::default()
            },
            RecordPatch {
                name: Some("should not change".into()),
                host: FieldUpdate::Set(Host::Domain("bad/path".into())),
                ..RecordPatch::default()
            },
            RecordPatch {
                replace_data: vec![(third, Data::Code("a".into()))],
                remove_data: vec![third],
                ..RecordPatch::default()
            },
            RecordPatch {
                remove_data: vec![second],
                ..RecordPatch::default()
            },
        ] {
            assert!(
                records
                    .update(id, &patch, Timestamp::from_unix_millis(40))
                    .is_err()
            );
            let view = records.get(id).unwrap();
            assert_eq!(view.name, "mail");
            assert!(view.data() == entries);
            assert_eq!(view.updated.as_unix_millis(), 30);
        }
        records
            .update(
                id,
                &RecordPatch {
                    remove_data: entries.iter().map(|entry| entry.id).collect(),
                    ..RecordPatch::default()
                },
                now,
            )
            .unwrap();
        assert!(records.get(id).unwrap().data().is_empty());
        records
            .update(
                id,
                &RecordPatch {
                    add_data: vec![Data::Code("after clear".into())],
                    ..RecordPatch::default()
                },
                now,
            )
            .unwrap();
        assert_eq!(records.get(id).unwrap().data()[0].id, DataId::new(5));
    }

    #[test]
    fn loading_rejects_invalid_values_duplicate_data_ids_and_invalid_sequences() {
        let mut records = Records::new();
        let id = records
            .create(new_record("mail"), Timestamp::from_unix_millis(10))
            .unwrap();
        let original = records.snapshot(id).unwrap();
        let mut duplicate = original.clone();
        duplicate.data.push(duplicate.data[0].clone());
        assert!(matches!(
            Records::from_records(vec![duplicate], Some(2)),
            Err(StorageError::DuplicateDataId(_))
        ));
        let mut bad_next = original.clone();
        bad_next.next_data_id = Some(1);
        assert!(Records::from_records(vec![bad_next], Some(2)).is_err());
        for data in [
            Data::SshKey(crate::SshKey {
                private_key: None,
                public_key: None,
            }),
            Data::Totp(crate::Totp {
                secret: "bad".into(),
                algorithm: crate::TotpAlgorithm::Sha1,
                digits: 6,
                period: 30,
            }),
        ] {
            let mut invalid = original.clone();
            invalid.data[0].value = data;
            assert!(Records::from_records(vec![invalid], Some(2)).is_err());
        }
        let mut exhausted = original;
        exhausted.data[0].id = DataId::new(u64::MAX);
        exhausted.next_data_id = None;
        let mut records = Records::from_records(vec![exhausted], Some(2)).unwrap();
        assert!(matches!(
            records.update(
                id,
                &RecordPatch {
                    name: Some("changed".into()),
                    add_data: vec![Data::Code("a".into())],
                    ..RecordPatch::default()
                },
                Timestamp::from_unix_millis(20)
            ),
            Err(StorageError::DataIdExhausted)
        ));
        assert_eq!(records.get(id).unwrap().name, "mail");
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
