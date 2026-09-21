use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Host {
    IP(IpAddr),
    Domain(String),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Data {
    Password(String),
    SshKey(String),
    TOTP(String),
    Code(String),
    Enforced(Box<Data>),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Timestamp(u64);

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum PasswordType {
    Unknown,
    BitMask(u8),
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct Templates {
    host: Host,
    password_type: PasswordType,
}
