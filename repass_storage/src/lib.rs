use std::borrow::Borrow;
use std::collections::HashSet;
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

struct Tags {
    tags: HashSet<Tag>,
}

impl Tags {
    pub fn new() -> Self {
        Self {
            tags: HashSet::new(),
        }
    }

    pub fn add(self: &mut Self, name: &str) -> &Tag {
        if !self.tags.contains(name) {
            let tag = Tag::new(name);
            self.tags.insert(tag);
        }
        self.tags.get(name).unwrap()
    }

    pub fn get_mut(self: Self, name: &str) -> &mut Tag {
       todo!()
    }
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
