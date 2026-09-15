use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::str::FromStr;
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

#[derive(Hash, Eq, PartialEq)]
struct Tag<'a> {
    name: &'a str,
}

impl<'a> Tag<'a> {
    fn new(name: &'a str) -> Self {
        Self { name }
    }

    fn rename(self: &mut Self, new_name: &'a str) {
        self.name = new_name;
    }
}

struct Tags<'a> {
    tags: HashMap<&'a str, Tag<'a>>,
}

impl<'a> Tags<'a> {
    pub fn new() -> Self {
        Self {
            tags: HashMap::new(),
        }
    }

    pub fn add(self: &mut Self, name: &'a str) -> &Tag {
        self.tags.entry(name).or_insert_with(|| Tag::new(name))
    }

    pub fn get(self: &Self, name: &str) -> Option<&Tag> {
        self.tags.get(name)
    }

    pub fn rename(self: &mut Self, name: &str, new_name: &'a str) -> Result<&Tag<'a>, ()> {
        if self.tags.contains_key(new_name) {
            return Err(());
        }
        let Some(mut tag) = self.tags.remove(name) else {
            return Err(());
        };
        tag.rename(new_name);
        Ok(self.tags.entry(new_name).or_insert(tag))
    }
}

struct Record<'a> {
    salt: Salt,
    login: String,
    host: Option<Host>,
    data: Vec<Data>,
    tags: Vec<&'a Tag<'a>>,
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
