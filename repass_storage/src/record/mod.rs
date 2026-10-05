use crate::StorageError;
use crate::record::types::Timestamp;
use crate::tags::TagId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

pub mod serialize;
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

/// In-memory record columns. The position map is an ephemeral index and is
/// rebuilt from `ids` when the persisted state is loaded.
#[derive(Clone)]
pub struct Records {
    ids: Vec<RecordId>,
    names: Vec<String>,
    passwords: Vec<String>,
    usernames: Vec<Option<String>>,
    urls: Vec<Option<String>>,
    notes: Vec<Option<String>>,
    tags: Vec<Vec<TagId>>,
    created: Vec<Timestamp>,
    updated: Vec<Timestamp>,
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

impl Records {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn from_persisted(
        records: Vec<PersistedRecord>,
        next_id: Option<u64>,
    ) -> Result<Self, StorageError> {
        let mut result = Self::default();
        let mut maximum = None;
        for record in records {
            if result.positions.contains_key(&record.id) {
                return Err(StorageError::DuplicateRecordId(record.id));
            }
            if record.name.is_empty() {
                return Err(StorageError::InvalidState("record name cannot be empty"));
            }
            let record_tags = unique_tags(record.tags)?;
            let position = result.ids.len();
            maximum = Some(maximum.map_or(record.id.0, |current: u64| current.max(record.id.0)));
            result.positions.insert(record.id, position);
            result.ids.push(record.id);
            result.names.push(record.name);
            result.passwords.push(record.password);
            result.usernames.push(record.username);
            result.urls.push(record.url);
            result.notes.push(record.notes);
            result.tags.push(record_tags);
            result.created.push(record.created);
            result.updated.push(record.updated);
        }
        if maximum.is_some_and(|maximum| next_id.is_some_and(|next| next <= maximum)) {
            return Err(StorageError::InvalidState(
                "next record ID must be greater than all stored record IDs",
            ));
        }
        result.next_id = next_id;
        Ok(result)
    }

    pub fn get(&self, id: RecordId) -> Option<RecordView<'_>> {
        self.positions
            .get(&id)
            .copied()
            .map(|position| self.view_at(position))
    }

    pub fn views(&self) -> Vec<RecordView<'_>> {
        let mut positions: Vec<_> = (0..self.ids.len()).collect();
        positions.sort_unstable_by_key(|position| self.ids[*position]);
        positions
            .into_iter()
            .map(|position| self.view_at(position))
            .collect()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn find_by_login(&self, login: &str) -> Vec<RecordId> {
        self.ids
            .iter()
            .enumerate()
            .filter_map(|(position, id)| (self.names[position] == login).then_some(*id))
            .collect()
    }

    /// Finds records that contain every requested tag. An empty query matches
    /// every record.
    pub fn find_by_tags(&self, tags: &[TagId]) -> Vec<RecordId> {
        self.ids
            .iter()
            .enumerate()
            .filter_map(|(position, id)| {
                tags.iter()
                    .all(|tag| self.tags[position].contains(tag))
                    .then_some(*id)
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
        let position = self.ids.len();
        self.ids.push(id);
        self.names.push(record.name);
        self.passwords.push(record.password);
        self.usernames.push(record.username);
        self.urls.push(record.url);
        self.notes.push(record.notes);
        self.tags.push(record_tags);
        self.created.push(timestamp.clone());
        self.updated.push(timestamp);
        self.positions.insert(id, position);
        self.next_id = value.checked_add(1);
        Ok(id)
    }

    pub(crate) fn remove(&mut self, id: RecordId) -> Result<(), StorageError> {
        let position = self
            .positions
            .remove(&id)
            .ok_or(StorageError::RecordNotFound(id))?;
        self.ids.swap_remove(position);
        self.names.swap_remove(position);
        self.passwords.swap_remove(position);
        self.usernames.swap_remove(position);
        self.urls.swap_remove(position);
        self.notes.swap_remove(position);
        self.tags.swap_remove(position);
        self.created.swap_remove(position);
        self.updated.swap_remove(position);
        if let Some(moved_id) = self.ids.get(position).copied() {
            self.positions.insert(moved_id, position);
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
        if let Some(name) = &patch.name {
            if self.names[position] != *name {
                self.names[position].clone_from(name);
                changed = true;
            }
        }
        if let Some(password) = &patch.password {
            if self.passwords[position] != *password {
                self.passwords[position].clone_from(password);
                changed = true;
            }
        }
        changed |= apply_field(&mut self.usernames[position], &patch.username);
        changed |= apply_field(&mut self.urls[position], &patch.url);
        changed |= apply_field(&mut self.notes[position], &patch.notes);

        for tag in add {
            if !self.tags[position].contains(&tag) {
                self.tags[position].push(tag);
                changed = true;
            }
        }
        for tag in remove {
            let length = self.tags[position].len();
            self.tags[position].retain(|existing| *existing != tag);
            changed |= self.tags[position].len() != length;
        }
        if changed {
            self.updated[position] = timestamp;
        }
        Ok(changed)
    }

    pub(crate) fn uses_tag(&self, tag: TagId) -> bool {
        self.tags
            .iter()
            .any(|record_tags| record_tags.contains(&tag))
    }

    pub(crate) fn persisted(&self) -> Vec<PersistedRecord> {
        (0..self.ids.len())
            .map(|position| PersistedRecord {
                id: self.ids[position],
                name: self.names[position].clone(),
                password: self.passwords[position].clone(),
                username: self.usernames[position].clone(),
                url: self.urls[position].clone(),
                notes: self.notes[position].clone(),
                tags: self.tags[position].clone(),
                created: self.created[position].clone(),
                updated: self.updated[position].clone(),
            })
            .collect()
    }

    pub(crate) fn next_id(&self) -> Option<u64> {
        self.next_id
    }

    pub(crate) fn used_tag_ids(&self) -> Vec<TagId> {
        self.tags
            .iter()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn view_at(&self, position: usize) -> RecordView<'_> {
        RecordView {
            id: self.ids[position],
            name: &self.names[position],
            username: self.usernames[position].as_deref(),
            url: self.urls[position].as_deref(),
            notes: self.notes[position].as_deref(),
            tags: &self.tags[position],
            created: &self.created[position],
            updated: &self.updated[position],
            password: &self.passwords[position],
        }
    }
}

impl Default for Records {
    fn default() -> Self {
        Self {
            ids: Vec::new(),
            names: Vec::new(),
            passwords: Vec::new(),
            usernames: Vec::new(),
            urls: Vec::new(),
            notes: Vec::new(),
            tags: Vec::new(),
            created: Vec::new(),
            updated: Vec::new(),
            positions: HashMap::new(),
            next_id: Some(1),
        }
    }
}

fn unique_tags(tags: Vec<TagId>) -> Result<Vec<TagId>, StorageError> {
    let mut unique = Vec::with_capacity(tags.len());
    for tag in tags {
        if unique.contains(&tag) {
            return Err(StorageError::DuplicateRecordTag(tag));
        }
        unique.push(tag);
    }
    Ok(unique)
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
    fn swap_remove_keeps_columns_and_stable_id_index_consistent() {
        let now = Timestamp::from_unix_millis(10);
        let mut records = Records::new();
        let first = records.create(new_record("first"), now.clone()).unwrap();
        let middle = records.create(new_record("middle"), now.clone()).unwrap();
        let last = records.create(new_record("last"), now.clone()).unwrap();

        records.remove(middle).unwrap();
        assert!(records.get(middle).is_none());
        assert_eq!(records.get(first).unwrap().name, "first");
        assert_eq!(records.get(last).unwrap().name, "last");
        assert_eq!(records.find_by_login("last"), vec![last]);
        assert!(records.columns_are_aligned());
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
                    add_tags: vec![1],
                    remove_tags: vec![1],
                    ..RecordPatch::default()
                },
                Timestamp::from_unix_millis(30)
            ),
            Err(StorageError::ConflictingTagUpdate(1))
        ));
        assert!(records.columns_are_aligned());
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
        assert!(restored.columns_are_aligned());
    }

    impl Records {
        fn columns_are_aligned(&self) -> bool {
            let len = self.ids.len();
            [
                self.names.len(),
                self.passwords.len(),
                self.usernames.len(),
                self.urls.len(),
                self.notes.len(),
                self.tags.len(),
                self.created.len(),
                self.updated.len(),
                self.positions.len(),
            ]
            .into_iter()
            .all(|column| column == len)
                && self
                    .ids
                    .iter()
                    .enumerate()
                    .all(|(position, id)| self.positions.get(id) == Some(&position))
        }
    }
}
