use crate::StorageError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct TagId(u16);

impl TagId {
    pub const MAX: Self = Self(u16::MAX);

    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u16 {
        self.0
    }

    fn checked_add(self, value: u16) -> Option<Self> {
        self.0.checked_add(value).map(Self)
    }
}

impl Display for TagId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for TagId {
    type Err = std::num::ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

impl From<u16> for TagId {
    fn from(value: u16) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tag {
    id: TagId,
    name: String,
    technical: bool,
}

impl Tag {
    fn new(id: TagId, name: String) -> Self {
        Self {
            id,
            name,
            technical: false,
        }
    }

    pub fn id(&self) -> TagId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_technical(&self) -> bool {
        self.technical
    }
}

#[derive(Clone)]
pub(crate) struct Tags {
    by_id: BTreeMap<TagId, Tag>,
    by_name: HashMap<String, TagId>,
    next_id: Option<TagId>,
}

impl Tags {
    pub(crate) fn from_catalog(
        stored: impl IntoIterator<Item = (TagId, String)>,
        referenced_ids: impl IntoIterator<Item = TagId>,
        next_id: Option<TagId>,
    ) -> Result<Self, StorageError> {
        let referenced_ids: Vec<_> = referenced_ids.into_iter().collect();
        let stored = stored.into_iter();
        let stored_capacity = stored.size_hint().0;
        let mut by_id = BTreeMap::new();
        let mut by_name = HashMap::with_capacity(stored_capacity + referenced_ids.len());
        for (id, name) in stored {
            if name.is_empty() {
                return Err(StorageError::InvalidState("tag name cannot be empty"));
            }
            if by_id.contains_key(&id) {
                return Err(StorageError::DuplicateTagId(id));
            }
            if by_name.contains_key(&name) {
                return Err(StorageError::DuplicateTagName(name));
            }
            by_name.insert(name.clone(), id);
            by_id.insert(id, Tag::new(id, name));
        }

        if let Some(maximum) = by_id.keys().next_back().copied()
            && next_id.is_some_and(|next| next <= maximum)
        {
            return Err(StorageError::InvalidState(
                "next tag ID must be greater than all stored tag IDs",
            ));
        }

        for id in referenced_ids {
            if let std::collections::btree_map::Entry::Vacant(entry) = by_id.entry(id) {
                let name = unique_technical_name(id, &by_name);
                by_name.insert(name.clone(), id);
                entry.insert(Tag {
                    id,
                    name,
                    technical: true,
                });
            }
        }

        let next_id = match (next_id, by_id.keys().next_back().copied()) {
            (None, _) => None,
            (Some(next), Some(maximum)) if next <= maximum => maximum.checked_add(1),
            (Some(next), _) => Some(next),
        };

        Ok(Self {
            by_id,
            by_name,
            next_id,
        })
    }

    pub fn create(&mut self, name: impl Into<String>) -> Result<TagId, StorageError> {
        let name = name.into();
        if name.is_empty() {
            return Err(StorageError::InvalidState("tag name cannot be empty"));
        }
        if self.by_name.contains_key(&name) {
            return Err(StorageError::DuplicateTagName(name));
        }
        let mut id = self.next_id.ok_or(StorageError::TagIdExhausted)?;
        while self.by_id.contains_key(&id) {
            id = id.checked_add(1).ok_or(StorageError::TagIdExhausted)?;
        }
        self.insert_with_id(id, name)?;
        self.next_id = id.checked_add(1);
        Ok(id)
    }

    fn insert_with_id(&mut self, id: TagId, name: String) -> Result<(), StorageError> {
        if self.by_id.contains_key(&id) {
            return Err(StorageError::DuplicateTagId(id));
        }
        self.by_name.insert(name.clone(), id);
        self.by_id.insert(id, Tag::new(id, name));
        Ok(())
    }

    pub fn get(&self, id: TagId) -> Option<&Tag> {
        self.by_id.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tag> {
        self.by_id.values()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn rename(&mut self, id: TagId, new_name: impl Into<String>) -> Result<(), StorageError> {
        let new_name = new_name.into();
        if new_name.is_empty() {
            return Err(StorageError::InvalidState("tag name cannot be empty"));
        }
        if self
            .by_name
            .get(&new_name)
            .is_some_and(|existing_id| *existing_id != id)
        {
            return Err(StorageError::DuplicateTagName(new_name));
        }
        let tag = self
            .by_id
            .get_mut(&id)
            .ok_or(StorageError::TagNotFound(id))?;
        self.by_name.remove(&tag.name);
        tag.name.clone_from(&new_name);
        tag.technical = false;
        self.by_name.insert(new_name, id);
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: TagId) -> Result<Tag, StorageError> {
        let tag = self
            .by_id
            .remove(&id)
            .ok_or(StorageError::TagNotFound(id))?;
        self.by_name.remove(&tag.name);
        Ok(tag)
    }

    pub(crate) fn persisted(&self) -> Vec<(TagId, String)> {
        self.by_id
            .values()
            .filter(|tag| !tag.technical)
            .map(|tag| (tag.id, tag.name.clone()))
            .collect()
    }

    pub(crate) fn next_id(&self) -> Option<TagId> {
        self.next_id
    }

    pub(crate) fn promote_technical(&mut self) {
        for tag in self.by_id.values_mut() {
            if tag.technical {
                tag.technical = false;
                self.by_name.insert(tag.name.clone(), tag.id);
            }
        }
    }
}

fn unique_technical_name(id: TagId, occupied: &HashMap<String, TagId>) -> String {
    let base = format!("#tag-{id}");
    if !occupied.contains_key(&base) {
        return base;
    }
    for suffix in 1..=occupied.len() + 1 {
        let candidate = format!("{base}~{suffix}");
        if !occupied.contains_key(&candidate) {
            return candidate;
        }
    }
    format!("{base}~{}", occupied.len() + 2)
}

impl Default for Tags {
    fn default() -> Self {
        Self {
            by_id: BTreeMap::new(),
            by_name: HashMap::new(),
            next_id: Some(TagId::new(0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(id: impl Into<TagId>, name: &str) -> (TagId, String) {
        (id.into(), name.to_owned())
    }

    #[test]
    fn unknown_ids_receive_technical_names_and_allocator_skips_them() {
        let mut tags =
            Tags::from_catalog([stored(0, "work")], [1.into(), 5.into()], Some(1.into())).unwrap();
        assert!(tags.get(1.into()).unwrap().is_technical());
        assert_eq!(tags.get(1.into()).unwrap().name(), "#tag-1");
        assert_eq!(tags.create("personal").unwrap(), 6.into());
        assert_eq!(
            tags.persisted(),
            vec![stored(0, "work"), stored(6, "personal")]
        );
    }

    #[test]
    fn renaming_a_technical_tag_promotes_only_that_id() {
        let mut tags = Tags::from_catalog([], [3.into(), 4.into()], Some(0.into())).unwrap();
        tags.rename(3.into(), "renamed").unwrap();
        assert!(!tags.get(3.into()).unwrap().is_technical());
        assert!(tags.get(4.into()).unwrap().is_technical());
        assert_eq!(tags.persisted(), vec![stored(3, "renamed")]);
    }

    #[test]
    fn stored_tags_validate_unique_ids_and_names() {
        assert!(matches!(
            Tags::from_catalog([stored(1, "one"), stored(1, "two")], [], Some(2.into())),
            Err(StorageError::DuplicateTagId(id)) if id == TagId::new(1)
        ));
        assert!(matches!(
            Tags::from_catalog([stored(1, "same"), stored(2, "same")], [], Some(3.into())),
            Err(StorageError::DuplicateTagName(_))
        ));
    }

    #[test]
    fn technical_names_are_made_unique_before_catalog_recovery() {
        let mut tags = Tags::from_catalog(
            [stored(99, "#tag-3")],
            [3.into(), 4.into()],
            Some(100.into()),
        )
        .unwrap();
        assert_eq!(tags.get(3.into()).unwrap().name(), "#tag-3~1");
        assert_eq!(tags.get(4.into()).unwrap().name(), "#tag-4");
        assert_eq!(tags.by_name.get("#tag-3"), Some(&99.into()));

        tags.promote_technical();
        assert_eq!(tags.by_name.get("#tag-3~1"), Some(&3.into()));
        assert_eq!(tags.persisted().len(), 3);
    }

    #[test]
    fn ids_do_not_overflow_or_get_reused() {
        let mut tags = Tags::from_catalog(
            [stored(TagId::new(u16::MAX - 1), "before")],
            [],
            Some(TagId::MAX),
        )
        .unwrap();
        assert_eq!(tags.create("last").unwrap(), TagId::MAX);
        assert!(matches!(
            tags.create("overflow"),
            Err(StorageError::TagIdExhausted)
        ));
    }

    #[test]
    fn tag_id_newtype_keeps_the_existing_postcard_integer_encoding() {
        assert_eq!(
            postcard::to_allocvec(&TagId::new(42)).unwrap(),
            postcard::to_allocvec(&42u16).unwrap()
        );
    }
}
