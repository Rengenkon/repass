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

impl Records {
    pub fn get(self: &Self, id: usize) -> RecordView {
        RecordView {
            login: self.logins.get(id).unwrap(),
            host: self.hosts.get(id).unwrap().as_ref(),
            data: self.data.get(id).unwrap().iter().map(|x| x).collect(),
            tags: self.tags.get(id).unwrap(),
            created: self.created.get(id).unwrap(),
            updated: self.updated.get(id).unwrap(),
        }
    }
}

struct RecordView<'a> {
    // salt: &'a Salt,
    login: &'a str,
    host: Option<&'a Host>,
    // data: &'a [&'a Data],
    data: Vec<&'a Data>,
    tags: &'a [TagId],
    created: &'a Timestamp,
    updated: &'a Timestamp,
}

fn filter<F>(record: &Record, predicates: &[F]) -> bool
where
    F: Fn(&Record) -> bool,
{
    predicates.iter().any(|predicate| predicate(&record))
}

enum PasswordType {
    Unknown,
    BitMask(u8),
}

struct Templates {
    host: Host,
    password_type: PasswordType,
}
