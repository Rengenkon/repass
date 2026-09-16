pub mod tags;

use crate::tags::TagId;
use std::net::IpAddr;

struct Salt {}

enum Host {
    IP(IpAddr),
    Domain(String),
}

enum Data {
    Password(String),
    SshKey(String),
    TOTP(String),
    Code(String),
    Enforced(Salt, Box<Data>),
}

struct Timestamp(u64);

struct Records {
    salts: Vec<Salt>,
    logins: Vec<String>,
    hosts: Vec<Option<Host>>,
    data: Vec<Vec<Data>>,
    tags: Vec<Vec<TagId>>,
    created: Vec<Timestamp>,
    updated: Vec<Timestamp>,
}

struct RecordView<'a> {
    // salt: &'a Salt,
    login: &'a str,
    host: Option<&'a Host>,
    // data: &'a [&'a Data],
    data: &'a [Data],
    tags: &'a [TagId],
    created: &'a Timestamp,
    updated: &'a Timestamp,
}

impl Records {
    pub fn create(self: &mut Self, id: usize) -> RecordView {
        todo!()
    }

    pub fn remove(self: &mut Self, id: usize) {
        todo!()
    }

    pub fn get(self: &Self, id: usize) -> RecordView {
        RecordView {
            login: &self.logins[id],
            host: self.hosts[id].as_ref(),
            data: &self.data[id],
            tags: &self.tags[id],
            created: &self.created[id],
            updated: &self.updated[id],
        }
    }

    pub fn serialize(&self) -> impl Iterator<Item = (&Salt, RecordView<'_>)> {
        (0..self.salts.len()).map(|i| {
            (
                &self.salts[i],
                RecordView {
                    login: &self.logins[i],
                    host: self.hosts[i].as_ref(),
                    data: &self.data[i],
                    tags: &self.tags[i],
                    created: &self.created[i],
                    updated: &self.updated[i],
                },
            )
        })
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

    pub fn add_tag(self: &mut Self, id: usize, tag: TagId) {
        todo!()
    }

    pub fn add_tags(self: &mut Self, id: usize, tags: &[TagId]) {
        todo!()
    }

    pub fn remove_tag(self: &mut Self, id: usize, tag: TagId) {
        todo!()
    }

    pub fn remove_tags(self: &mut Self, id: usize) {
        todo!()
    }
}

enum PasswordType {
    Unknown,
    BitMask(u8),
}

struct Templates {
    host: Host,
    password_type: PasswordType,
}
