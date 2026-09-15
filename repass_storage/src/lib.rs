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
struct Tag {
    name: String,
}

impl Tag {
    fn new(name: &str) -> Self {
        Self {
            name: String::from_str(name).unwrap(),
        }
    }

    pub fn rename(self: &mut Self, new_name: &str) {
        self.name.clear();
        self.name.push_str(new_name);
    }
}

impl Borrow<str> for Tag {
    fn borrow(&self) -> &str {
        &self.name
    }
}

struct Tags<'a> {
    tags: HashMap<&'a str, Tag>,
}

impl<'a> Tags<'a> {
    pub fn new() -> Self {
        Self {
            tags: HashMap::new(),
        }
    }

    pub fn add(self: &mut Self, name: &'a str) -> &Tag {
        if !self.tags.contains_key(name) {
            let tag = Tag::new(name);
            self.tags.insert(name, tag);
        }
        self.tags.get(name).unwrap()
    }

    pub fn rename(self: &mut Self, name: &str, new_name: &'a str) -> Result<&Tag, ()> {
        if self.tags.contains_key(new_name) {
            return Err(());
        }
        if !self.tags.contains_key(name) {
            return Err(());
        }
        let mut tag = self.tags.remove(name).unwrap();
        tag.rename(new_name);
        self.tags.insert(new_name, tag);
        Ok(self.tags.get(new_name).unwrap())
    }

    // pub fn get_mut(self: &mut Self, name: &str) -> Option<&mut Tag> {
    //     if !self.tags.contains_key(name) {
    //         return None
    //     }
    //     self.tags.get_mut(name)
    // }
}

struct Record<'a> {
    salt: Salt,
    login: String,
    host: Option<Host>,
    data: Vec<Data>,
    tags: Vec<&'a Tag>,
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
