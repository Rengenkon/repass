use std::collections::{HashMap, HashSet};
use std::collections::hash_map::Entry;

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

pub struct Tags<'a> {
    tags: HashMap<&'a str, Tag<'a>>,
    next_id: TagId,
}

impl<'a> Tags<'a> {
    pub fn new(ins: HashSet<Tag<'a>>) -> Self {
        let mut tags = HashMap::new();
        let mut max = 0;
        ins.into_iter().for_each(|tag| {
            if tag.id > max {
                max = tag.id;
            }
            tags.insert(tag.name, tag);
        });
        Self { tags, next_id: max + 1 }
    }

    pub fn create(self: &mut Self, name: &'a str) -> Result<&Tag<'_>, ()> {
        match self.tags.entry(name) {
            Entry::Occupied(_) => Err(()),
            Entry::Vacant(e) => {
                let tag = Tag::new(self.next_id, name);
                self.next_id += 1;
                Ok(e.insert(tag))
            }
        }
    }

    pub fn get(self: &Self, name: &str) -> Option<&Tag<'_>> {
        self.tags.get(name)
    }

    pub fn get_or_create(self: &mut Self, name: &'a str) -> &Tag<'_> {
        match self.tags.entry(name) {
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
            return self.tags.get(name).ok_or(());
        }
        if self.tags.contains_key(new_name) {
            return Err(());
        }
        let Some(mut tag) = self.tags.remove(name) else {
            return Err(());
        };
        tag.rename(new_name);
        Ok(self.tags.entry(new_name).or_insert(tag))
    }

    pub fn minimize(self: &Self) -> Minimize<'_> {
        let mut mapping = HashMap::new();
        let mut tags = HashMap::new();
        let mut index = 0;
        self.tags.iter().for_each(|(_, tag)| {
            mapping.insert(tag.id, index);
            let new_tag = Tag::new(index, tag.name);
            tags.insert(tag.name, new_tag);
            index += 1;
        });
        let tags = Tags {
            tags,
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
            tags: HashMap::new(),
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