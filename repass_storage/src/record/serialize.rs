use crate::tags::TagId;
use crate::record::RecordView;
use crate::record::types::{Data, Host, Timestamp};
use chacha20poly1305::aead::{Aead, Generate};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use postcard::{from_bytes, to_allocvec};
use serde::{Deserialize, Serialize};

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
