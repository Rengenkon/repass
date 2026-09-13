use std::net::IpAddr;
use std::time::Instant;

struct Salt {}

enum Host {
    IP(IpAddr),
    Domain(String),
}

enum Data {
    Password(String),
    SshKey(IpAddr, String),
    TOTP(String),
    Enforced(Salt, Box<Data>),
}

struct Tag {}

struct Field<'a> {
    data: &'a str,
    owner: &'a Record<'a>
}

struct Record<'b> {
    salt: Salt,
    login: Field<'b>,
    host: Option<Field<'b>>,
    data: &'b [Data],
    tags: &'b [Tag],
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
