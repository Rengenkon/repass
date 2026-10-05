use crate::StorageError;
use std::collections::{BTreeMap, HashMap};

pub type TagId = u16;

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

    fn technical(id: TagId) -> Self {
        Self {
            id,
            name: format!("#tag-{id}"),
            technical: true,
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
pub struct Tags {
    by_id: BTreeMap<TagId, Tag>,
    by_name: HashMap<String, TagId>,
    next_id: Option<TagId>,
}

impl Tags {
    pub fn new(tags: impl IntoIterator<Item = Tag>) -> Result<Self, StorageError> {
        let tags: Vec<_> = tags.into_iter().collect();
        let next_id = tags
            .iter()
            .map(|tag| tag.id)
            .max()
            .map_or(Some(0), |id| id.checked_add(1));
        let referenced_ids = tags
            .iter()
            .filter(|tag| tag.technical)
            .map(|tag| tag.id)
            .collect::<Vec<_>>();
        Self::from_catalog(
            tags.into_iter()
                .filter(|tag| !tag.technical)
                .map(|tag| (tag.id, tag.name)),
            referenced_ids,
            next_id,
        )
    }

    pub(crate) fn from_catalog(
        stored: impl IntoIterator<Item = (TagId, String)>,
        referenced_ids: impl IntoIterator<Item = TagId>,
        next_id: Option<TagId>,
    ) -> Result<Self, StorageError> {
        let mut by_id = BTreeMap::new();
        let mut by_name = HashMap::new();
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
            by_id.entry(id).or_insert_with(|| Tag::technical(id));
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

    pub fn create_with_id(
        &mut self,
        id: TagId,
        name: impl Into<String>,
    ) -> Result<(), StorageError> {
        let name = name.into();
        if name.is_empty() {
            return Err(StorageError::InvalidState("tag name cannot be empty"));
        }
        if self.by_name.contains_key(&name) {
            return Err(StorageError::DuplicateTagName(name));
        }
        self.insert_with_id(id, name)?;
        if self.next_id.is_some_and(|next| next <= id) {
            self.next_id = id.checked_add(1);
        }
        Ok(())
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

    pub fn get_by_name(&self, name: &str) -> Option<&Tag> {
        self.by_name.get(name).and_then(|id| self.by_id.get(id))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tag> {
        self.by_id.values()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
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
        if !tag.technical {
            self.by_name.remove(&tag.name);
        }
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
        if !tag.technical {
            self.by_name.remove(&tag.name);
        }
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

impl Default for Tags {
    fn default() -> Self {
        Self {
            by_id: BTreeMap::new(),
            by_name: HashMap::new(),
            next_id: Some(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(id: TagId, name: &str) -> (TagId, String) {
        (id, name.to_owned())
    }

    #[test]
    fn unknown_ids_receive_technical_names_and_allocator_skips_them() {
        let mut tags = Tags::from_catalog([stored(0, "work")], [1, 5], Some(1)).unwrap();
        assert!(tags.get(1).unwrap().is_technical());
        assert_eq!(tags.get(1).unwrap().name(), "#tag-1");
        assert_eq!(tags.create("personal").unwrap(), 6);
        assert_eq!(
            tags.persisted(),
            vec![stored(0, "work"), stored(6, "personal")]
        );
    }

    #[test]
    fn renaming_a_technical_tag_promotes_only_that_id() {
        let mut tags = Tags::from_catalog([], [3, 4], Some(0)).unwrap();
        tags.rename(3, "renamed").unwrap();
        assert!(!tags.get(3).unwrap().is_technical());
        assert!(tags.get(4).unwrap().is_technical());
        assert_eq!(tags.persisted(), vec![stored(3, "renamed")]);
    }

    #[test]
    fn stored_tags_validate_unique_ids_and_names() {
        assert!(matches!(
            Tags::from_catalog([stored(1, "one"), stored(1, "two")], [], Some(2)),
            Err(StorageError::DuplicateTagId(1))
        ));
        assert!(matches!(
            Tags::from_catalog([stored(1, "same"), stored(2, "same")], [], Some(3)),
            Err(StorageError::DuplicateTagName(_))
        ));
    }

    #[test]
    fn ids_do_not_overflow_or_get_reused() {
        let mut tags =
            Tags::from_catalog([stored(TagId::MAX - 1, "before")], [], Some(TagId::MAX)).unwrap();
        assert_eq!(tags.create("last").unwrap(), TagId::MAX);
        assert!(matches!(
            tags.create("overflow"),
            Err(StorageError::TagIdExhausted)
        ));
    }
}
