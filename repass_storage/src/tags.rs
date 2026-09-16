use std::collections::hash_map::Entry;
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
            next_id: max + 1,
        }
    }

    pub fn create(self: &mut Self, name: &'a str) -> Result<TagId, ()> {
        match self.tags_names.entry(name) {
            Entry::Occupied(_) => Err(()),
            Entry::Vacant(e) => {
                let id = self.next_id();
                let tag = Tag::new(id, name);
                e.insert(id);
                self.tags_ids.insert(id, tag);
                Ok(id)
            }
        }
    }

    pub fn create_with_id(self: &mut Self, name: &'a str, id: TagId) -> Result<TagId, ()> {
        match self.tags_ids.entry(id) {
            Entry::Occupied(_) => Err(()),
            Entry::Vacant(e_id) => match self.tags_names.entry(name) {
                Entry::Occupied(_) => Err(()),
                Entry::Vacant(e_name) => {
                    let tag = Tag::new(id, name);
                    e_name.insert(id);
                    e_id.insert(tag);
                    Ok(id)
                }
            },
        }
    }

    pub fn get(self: &Self, id: TagId) -> Option<&Tag<'_>> {
        self.tags_ids.get(&id)
    }

    pub fn get_or_create(self: &mut Self, name: &'a str) -> &Tag<'_> {
        match self.tags_names.entry(name) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                let tag = Tag::new(self.next_id, name);
                self.next_id += 1;
                e.insert(tag)
            }
        }
    }

    pub fn rename(self: &mut Self, name: &str, new_name: &'a str) -> Result<&Tag<'_>, ()> {
        if name == new_name {
            return self.tags_names.get(name).ok_or(());
        }
        if self.tags_names.contains_key(new_name) {
            return Err(());
        }
        let Some(mut tag) = self.tags_names.remove(name) else {
            return Err(());
        };
        tag.rename(new_name);
        Ok(self.tags_names.entry(new_name).or_insert(tag))
    }

    pub fn minimize(self: &Self) -> Minimize<'_> {
        let mut mapping = HashMap::new();
        let mut tags = HashMap::new();
        let mut index = 0;
        self.tags_names.iter().for_each(|(_, tag)| {
            mapping.insert(tag.id, index);
            let new_tag = Tag::new(index, tag.name);
            tags.insert(tag.name, new_tag);
            index += 1;
        });
        let tags = Tags {
            tags_names: tags,
            next_id: index,
        };
        Minimize {
            tags,
            id_mapping: mapping,
        }
    }
}

impl Default for Tags<'_> {
    fn default() -> Self {
        Self {
            tags_names: HashMap::new(),
            next_id: 0,
        }
    }
}

pub struct Minimize<'a> {
    tags: Tags<'a>,
    id_mapping: HashMap<TagId, TagId>,
}

impl<'a> Minimize<'a> {
    pub fn tags(self: Self) -> Tags<'a> {
        self.tags
    }

    pub fn convert(self: &Self, tags: &[TagId]) -> Vec<TagId> {
        tags.iter()
            .map(|tag| *self.id_mapping.get(tag).unwrap())
            .collect()
    }
}
