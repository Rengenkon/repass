use std::collections::{HashMap, HashSet};

pub type TagId = u16;

#[derive(Hash)]
pub struct Tag<'a> {
    id: TagId,
    name: &'a str,
}

impl<'a> Tag<'a> {
    fn new(id: TagId, name: &'a str) -> Self {
        Self { id, name }
    }

    fn rename(self: &mut Self, new_name: &'a str) {
        self.name = new_name;
    }

    pub fn id(&self) -> TagId {
        self.id
    }

    pub fn name(&self) -> &'a str {
        self.name
    }
}

struct IdCreator {
    sequence_id: TagId,
}

impl IdCreator {
    pub fn next_id(self: &mut Self, predicate: impl Fn(TagId) -> bool) -> TagId {
        while predicate(self.sequence_id) {
            self.sequence_id += 1;
        }
        self.sequence_id
    }
}

impl Default for IdCreator {
    fn default() -> Self {
        Self { sequence_id: 0 }
    }
}

pub struct Tags<'a> {
    tags_ids: HashMap<TagId, Tag<'a>>,
    tags_names: HashMap<&'a str, TagId>,
    id_creator: IdCreator,
}

impl<'a> Tags<'a> {
    fn next_id(self: &mut Self) -> TagId {
        self.id_creator
            .next_id(|id| self.tags_ids.contains_key(&id))
    }

    fn conflict_name(self: &Self, name: &'a str) -> Result<(), ()> {
        if let Some(conflict_id) = self.tags_names.get(name) {
            let conflict = self.tags_ids.get(conflict_id).unwrap();
            return Err(());
        }
        Ok(())
    }

    fn create_tag(self: &mut Self, id: TagId, name: &'a str) -> &Tag<'_> {
        let tag = Tag::new(id, name);
        self.tags_names.insert(name, id);
        self.tags_ids.insert(id, tag);
        self.get(id).unwrap()
    }

    pub fn new(ins: HashSet<Tag<'a>>) -> Self {
        let mut tags_names = HashMap::new();
        let mut tags_ids = HashMap::new();
        let mut max = 0;
        ins.into_iter().for_each(|tag| {
            if tag.id > max {
                max = tag.id;
            }
            tags_names.insert(tag.name, tag.id);
            tags_ids.insert(tag.id, tag);
        });
        Self {
            tags_ids,
            tags_names,
            id_creator: IdCreator::default(),
        }
    }

    pub fn create(self: &mut Self, name: &'a str) -> Result<&Tag<'_>, ()> {
        self.conflict_name(name)?;
        let id = self.next_id();
        Ok(self.create_tag(id, name))
    }

    pub fn create_with_id(self: &mut Self, id: TagId, name: &'a str) -> Result<&Tag<'_>, ()> {
        self.conflict_name(name)?;
        if let Some(conflict) = self.tags_ids.get(&id) {
            return Err(());
        }
        Ok(self.create_tag(id, name))
    }

    pub fn get(self: &Self, id: TagId) -> Option<&Tag<'_>> {
        self.tags_ids.get(&id)
    }

    pub fn rename(self: &mut Self, id: TagId, new_name: &'a str) -> Result<&Tag<'_>, ()> {
        self.conflict_name(new_name)?;
        let tag = self.tags_ids.get_mut(&id);
        if tag.is_none() {
            return Err(());
        }
        let tag = tag.unwrap();
        tag.rename(new_name);
        Ok(tag)
    }
}

impl Default for Tags<'_> {
    fn default() -> Self {
        Self {
            tags_ids: HashMap::new(),
            tags_names: HashMap::new(),
            id_creator: IdCreator::default(),
        }
    }
}
