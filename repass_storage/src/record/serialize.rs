use crate::record::RecordView;
use crate::record::types::{Data, Host, Timestamp};
use crate::tags::TagId;
use chacha20poly1305::aead::{Aead, Generate, Key};
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use postcard::{from_bytes, to_allocvec};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

#[derive(Deserialize)]
pub struct StoredRecord {
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

    pub fn decode(self: &Self, decoder: &ChaCha20Poly1305) -> StoredRecord {
        let nonce = Nonce::cast_from_core(&self.nonce);
        let bytes = decoder.decrypt(nonce, self.data.as_slice()).unwrap();
        from_bytes(&bytes).unwrap()
    }
}

fn decode(bytes: &[u8], decoder: &ChaCha20Poly1305) -> StoredRecord {
    let nonce: &[u8; 12] = bytes[..12].try_into().unwrap();
    let data: &[u8] = &bytes[12..];
    let nonce = Nonce::cast_from_core(nonce);
    let bytes = decoder.decrypt(nonce, data).unwrap();
    from_bytes(&bytes).unwrap()
}

pub struct Vault {
    path: Box<Path>,
    key: Key<ChaCha20Poly1305>,
}

impl Vault {
    pub fn new(path: Box<Path>) -> Self {
        let file = File::open(&path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = String::new();
        reader.read_line(&mut buffer);
        // let key = decrypt
        todo!()
    }

    pub fn lines(self: &Self) {
        let file = File::open(&self.path).unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = String::new();
        reader.read_line(&mut buffer); // skip first

        let cipher = ChaCha20Poly1305::new(&self.key);

        reader.read_line(&mut buffer); // capacity
        let capacity = buffer.parse(usize);

        let mut result = Vec::with_capacity(capacity);
        while let Ok(count) = reader.read_line(&mut buffer) {
            let bytes = buffer.as_bytes();
            let rec = decode(bytes, &cipher);
            result.push(rec);
        }
        todo!()
    }
}

fn test() {
    let vault = Vault::new();
    vault.lines()
}
