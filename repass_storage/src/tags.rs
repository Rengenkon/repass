use std::collections::HashMap;

pub type TagId = u16;

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

struct IdGenerator {
    sequence_id: TagId,
}

impl IdGenerator {
    pub fn next_id(self: &mut Self, predicate: impl Fn(TagId) -> bool) -> TagId {
        while predicate(self.sequence_id) {
            self.sequence_id += 1;
        }
        self.sequence_id
    }
}

impl Default for IdGenerator {
    fn default() -> Self {
        Self { sequence_id: 0 }
    }
}

pub struct Tags<'a> {
    tags_ids: HashMap<TagId, Tag<'a>>,
    tags_names: HashMap<&'a str, TagId>,
    id_generator: IdGenerator,
}

impl<'a> Tags<'a> {
    fn next_id(self: &mut Self) -> TagId {
        self.id_generator
            .next_id(|id| self.tags_ids.contains_key(&id))
    }

    fn conflict_name(self: &Self, name: &str) -> Result<(), ()> {
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

    pub fn new(tags: impl IntoIterator<Item = Tag<'a>>) -> Self {
        // todo не уникальные ид+имя
        let mut tags_names = HashMap::new();
        let mut tags_ids = HashMap::new();
        tags.into_iter().for_each(|tag| {
            tags_names.insert(tag.name, tag.id);
            tags_ids.insert(tag.id, tag);
        });
        Self {
            tags_ids,
            tags_names,
            id_generator: IdGenerator::default(),
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
        self.tags_names.remove(tag.name);
        self.tags_names.insert(new_name, id);
        tag.rename(new_name);
        Ok(tag)
    }
}

impl Default for Tags<'_> {
    fn default() -> Self {
        Self {
            tags_ids: HashMap::new(),
            tags_names: HashMap::new(),
            id_generator: IdGenerator::default(),
        }
    }
}
