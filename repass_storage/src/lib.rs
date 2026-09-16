pub mod tags;

use crate::tags::TagId;
use std::net::IpAddr;
use std::time::Instant;

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

struct Record {
    salt: Salt,
    login: String,
    host: Option<Host>,
    data: Vec<Data>,
    tags: Vec<TagId>,
    created: Instant,
    updated: Instant,
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
