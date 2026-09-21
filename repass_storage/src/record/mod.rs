use crate::tags::TagId;
use crate::record::types::{Data, Host, Timestamp};
use serde::Serialize;
use crate::record::serialize::StoredRecord;

mod serialize;
pub mod types;


pub struct Records {
    logins: Vec<String>,
    hosts: Vec<Option<Host>>,
    data: Vec<Vec<Data>>,
    tags: Vec<Vec<TagId>>,
    created: Vec<Timestamp>,
    updated: Vec<Timestamp>,
}

#[derive(Serialize, Debug, Eq, PartialEq)]
pub struct RecordView<'a> {
    login: &'a str,
    host: Option<&'a Host>,
    data: &'a [Data],
    tags: &'a [TagId],
    created: &'a Timestamp,
    updated: &'a Timestamp,
}

impl Records {
    pub fn new(store: impl IntoIterator<Item = StoredRecord>) -> Self {
        todo!()
    }

    pub fn create(self: &mut Self, rec: StoredRecord) -> RecordView<'_> {
        todo!()
    }

    pub fn remove(self: &mut Self, id: usize) {
        todo!()
    }

    pub fn get(self: &Self, id: usize) -> RecordView<'_> {
        RecordView {
            login: &self.logins[id],
            host: self.hosts[id].as_ref(),
            data: &self.data[id],
            tags: &self.tags[id],
            created: &self.created[id],
            updated: &self.updated[id],
        }
    }

    pub fn views(&self) -> impl Iterator<Item = RecordView<'_>> {
        (0..self.logins.len()).map(|i| self.get(i))
    }
}

impl Records {
    pub fn find_by_login(self: &Self, login: &str) -> Vec<usize> {
        todo!()
    }

    pub fn find_by_tags(self: &Self, tags: &[TagId]) -> Vec<usize> {
        todo!()
    }
}

impl Records {
    pub fn change_login(self: &mut Self, id: usize, login: &str) {
        todo!()
    }

    pub fn add_tags(self: &mut Self, id: usize, tags: &[TagId]) {
        todo!()
    }

    pub fn remove_tag(self: &mut Self, id: usize, tag: TagId) {
        todo!()
    }

    pub fn clear_tags(self: &mut Self, id: usize) {
        todo!()
    }
}