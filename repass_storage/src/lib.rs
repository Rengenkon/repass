pub mod tags;

use crate::tags::TagId;
use chacha20poly1305::aead::{Aead, Generate};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use postcard::{from_bytes, to_allocvec};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
enum Host {
    IP(IpAddr),
    Domain(String),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
enum Data {
    Password(String),
    SshKey(String),
    TOTP(String),
    Code(String),
    Enforced(Box<Data>),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct Timestamp(u64);

struct Records {
    logins: Vec<String>,
    hosts: Vec<Option<Host>>,
    data: Vec<Vec<Data>>,
    tags: Vec<Vec<TagId>>,
    created: Vec<Timestamp>,
    updated: Vec<Timestamp>,
}

#[derive(Serialize, Debug, Eq, PartialEq)]
struct RecordView<'a> {
    login: &'a str,
    host: Option<&'a Host>,
    data: &'a [Data],
    tags: &'a [TagId],
    created: &'a Timestamp,
    updated: &'a Timestamp,
}

#[derive(Deserialize)]
struct StoredRecord {
    login: String,
    host: Option<Host>,
    data: Vec<Data>,
    tags: Vec<TagId>,
    created: Timestamp,
    updated: Timestamp,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct Line {
    nonce: [u8; 12],
    data: Vec<u8>,
}

impl Line {
    pub fn encode(record: RecordView<'_>, encoder: &ChaCha20Poly1305) -> Self {
        let nonce = Nonce::generate();
        let payload = to_allocvec(&record).unwrap();
        let bytes = encoder.encrypt(&nonce, payload.as_slice()).unwrap();
        Self {
            nonce: nonce.into(),
            data: bytes,
        }
    }

    pub fn decode(self, decoder: &ChaCha20Poly1305) -> StoredRecord {
        let nonce = Nonce::cast_from_core(&self.nonce);
        let bytes = decoder.decrypt(nonce, self.data.as_slice()).unwrap();
        from_bytes(&bytes).unwrap()
    }
}

impl Records {
    pub fn new(store: impl IntoIterator<Item = StoredRecord>) -> Self {
        todo!()
    }

    pub fn create(self: &mut Self, rec: StoredRecord) -> RecordView {
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

    pub fn serialize(&self) -> impl Iterator<Item = RecordView<'_>> {
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

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
enum PasswordType {
    Unknown,
    BitMask(u8),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
struct Templates {
    host: Host,
    password_type: PasswordType,
}
