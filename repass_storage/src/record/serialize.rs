use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, Generate, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use rand::RngExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::types::{Data, Host, Timestamp};
use crate::tags::TagId;

const MAGIC: &[u8; 8] = b"REPASS\0\0";
const FORMAT_VERSION: u16 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const DATA_KEY_LEN: usize = 32;
const WRAPPED_KEY_LEN: usize = DATA_KEY_LEN + 16;
const KDF_MEMORY_KIB: u32 = 19 * 1024;
const KDF_ITERATIONS: u32 = 2;
const KDF_LANES: u32 = 1;
const HEADER_PREFIX_LEN: usize = 8 + 2 + 4 + 4 + 4 + SALT_LEN;
const HEADER_LEN: usize = HEADER_PREFIX_LEN + NONCE_LEN + WRAPPED_KEY_LEN;
const DOCUMENT_AAD: &[u8] = b"repass encrypted document v1";

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum VaultError {
    Io(std::io::Error),
    InvalidFormat(&'static str),
    UnsupportedVersion(u16),
    Crypto,
    Serialization(postcard::Error),
    DocumentKindMismatch { expected: String, actual: String },
    DocumentVersionMismatch { expected: u16, actual: u16 },
}

impl Display for VaultError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "vault I/O error: {error}"),
            Self::InvalidFormat(reason) => write!(formatter, "invalid vault format: {reason}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported vault format version: {version}")
            }
            Self::Crypto => write!(formatter, "vault authentication or decryption failed"),
            Self::Serialization(error) => write!(formatter, "vault serialization failed: {error}"),
            Self::DocumentKindMismatch { expected, actual } => {
                write!(
                    formatter,
                    "expected document kind {expected:?}, found {actual:?}"
                )
            }
            Self::DocumentVersionMismatch { expected, actual } => write!(
                formatter,
                "expected document schema version {expected}, found {actual}"
            ),
        }
    }
}

impl Error for VaultError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Serialization(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for VaultError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<postcard::Error> for VaultError {
    fn from(error: postcard::Error) -> Self {
        Self::Serialization(error)
    }
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct StoredRecord {
    login: String,
    host: Option<Host>,
    data: Vec<Data>,
    tags: Vec<TagId>,
    created: Timestamp,
    updated: Timestamp,
}

#[derive(Serialize, Deserialize)]
struct StoredDocument<T> {
    kind: String,
    schema_version: u16,
    id: String,
    value: T,
}

/// An encrypted file containing independently encrypted, typed documents.
///
/// Document values use Postcard, while the public `kind` and schema version are
/// authenticated as part of each encrypted document's serialized payload.
pub struct Vault {
    path: PathBuf,
    data_key: [u8; DATA_KEY_LEN],
    header: Vec<u8>,
}

impl Vault {
    /// Creates a new empty vault protected by `master_password`.
    pub fn create(path: impl AsRef<Path>, master_password: &[u8]) -> Result<Self, VaultError> {
        let mut salt = [0; SALT_LEN];
        rand::rng().fill(&mut salt);

        let data_key = Key::generate();
        let data_key_bytes: [u8; DATA_KEY_LEN] = data_key.into();
        let wrapping_key = derive_wrapping_key(
            master_password,
            &salt,
            KDF_MEMORY_KIB,
            KDF_ITERATIONS,
            KDF_LANES,
        )?;
        let cipher =
            ChaCha20Poly1305::new_from_slice(&wrapping_key).map_err(|_| VaultError::Crypto)?;
        let nonce = Nonce::generate();

        let mut header = Vec::with_capacity(HEADER_LEN);
        header.extend_from_slice(MAGIC);
        header.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        header.extend_from_slice(&KDF_MEMORY_KIB.to_le_bytes());
        header.extend_from_slice(&KDF_ITERATIONS.to_le_bytes());
        header.extend_from_slice(&KDF_LANES.to_le_bytes());
        header.extend_from_slice(&salt);

        let encrypted_key = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: &data_key_bytes,
                    aad: &header,
                },
            )
            .map_err(|_| VaultError::Crypto)?;
        header.extend_from_slice(nonce.as_slice());
        header.extend_from_slice(&encrypted_key);

        let path = path.as_ref().to_path_buf();
        write_new_atomically(&path, &header)?;
        Ok(Self {
            path,
            data_key: data_key_bytes,
            header,
        })
    }

    /// Opens an existing vault and verifies the wrapped data key.
    pub fn open(path: impl AsRef<Path>, master_password: &[u8]) -> Result<Self, VaultError> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.len() < HEADER_LEN {
            return Err(VaultError::InvalidFormat("truncated header"));
        }
        if &bytes[..MAGIC.len()] != MAGIC {
            return Err(VaultError::InvalidFormat("incorrect magic value"));
        }

        let mut offset = MAGIC.len();
        let version = read_u16(&bytes, &mut offset)?;
        if version != FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(version));
        }
        let memory_kib = read_u32(&bytes, &mut offset)?;
        let iterations = read_u32(&bytes, &mut offset)?;
        let lanes = read_u32(&bytes, &mut offset)?;
        validate_kdf_parameters(memory_kib, iterations, lanes)?;
        let salt = take_array::<SALT_LEN>(&bytes, &mut offset)?;
        let nonce_bytes = take_array::<NONCE_LEN>(&bytes, &mut offset)?;
        let encrypted_key = &bytes[offset..offset + WRAPPED_KEY_LEN];

        let wrapping_key =
            derive_wrapping_key(master_password, &salt, memory_kib, iterations, lanes)?;
        let cipher =
            ChaCha20Poly1305::new_from_slice(&wrapping_key).map_err(|_| VaultError::Crypto)?;
        let nonce = Nonce::from(nonce_bytes);
        let data_key = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: encrypted_key,
                    aad: &bytes[..HEADER_PREFIX_LEN],
                },
            )
            .map_err(|_| VaultError::Crypto)?;
        let data_key: [u8; DATA_KEY_LEN] = data_key
            .try_into()
            .map_err(|_| VaultError::InvalidFormat("invalid wrapped data key length"))?;

        Ok(Self {
            path,
            data_key,
            header: bytes[..HEADER_LEN].to_vec(),
        })
    }

    /// Replaces the vault contents with independently encrypted documents.
    pub fn save_documents<T: Serialize>(
        &self,
        kind: &str,
        schema_version: u16,
        documents: &[(String, T)],
    ) -> Result<(), VaultError> {
        if kind.is_empty() {
            return Err(VaultError::InvalidFormat("document kind cannot be empty"));
        }

        let mut output = self.header.clone();
        let cipher =
            ChaCha20Poly1305::new_from_slice(&self.data_key).map_err(|_| VaultError::Crypto)?;
        let mut ids = HashSet::with_capacity(documents.len());
        for (id, value) in documents {
            if id.is_empty() {
                return Err(VaultError::InvalidFormat("document ID cannot be empty"));
            }
            if !ids.insert(id) {
                return Err(VaultError::InvalidFormat("duplicate document ID"));
            }
            let document = StoredDocument {
                kind: kind.to_owned(),
                schema_version,
                id: id.clone(),
                value,
            };
            let plaintext = postcard::to_allocvec(&document)?;
            let nonce = Nonce::generate();
            let encrypted = cipher
                .encrypt(
                    &nonce,
                    Payload {
                        msg: &plaintext,
                        aad: DOCUMENT_AAD,
                    },
                )
                .map_err(|_| VaultError::Crypto)?;

            let mut frame = Vec::with_capacity(NONCE_LEN + encrypted.len());
            frame.extend_from_slice(nonce.as_slice());
            frame.extend_from_slice(&encrypted);
            output.extend_from_slice(&cobs::encode_vec(&frame));
            output.push(0);
        }

        write_atomically(&self.path, &output)
    }

    /// Loads all documents of the requested kind and schema version.
    pub fn load_documents<T: DeserializeOwned>(
        &self,
        expected_kind: &str,
        expected_schema_version: u16,
    ) -> Result<Vec<(String, T)>, VaultError> {
        let bytes = fs::read(&self.path)?;
        if bytes.len() < HEADER_LEN {
            return Err(VaultError::InvalidFormat("truncated header"));
        }
        validate_header(&bytes)?;
        if bytes[..HEADER_LEN] != self.header {
            return Err(VaultError::InvalidFormat(
                "vault header changed after opening",
            ));
        }
        let cipher =
            ChaCha20Poly1305::new_from_slice(&self.data_key).map_err(|_| VaultError::Crypto)?;
        let framed_records = &bytes[HEADER_LEN..];
        if framed_records.is_empty() {
            return Ok(Vec::new());
        }
        if !framed_records.ends_with(&[0]) {
            return Err(VaultError::InvalidFormat("unterminated COBS frame"));
        }

        let mut documents = Vec::new();
        let mut ids = HashSet::new();
        let frames: Vec<_> = framed_records.split(|byte| *byte == 0).collect();
        for (index, encoded_frame) in frames.iter().enumerate() {
            if encoded_frame.is_empty() && index + 1 == frames.len() {
                continue;
            }
            if encoded_frame.is_empty() {
                return Err(VaultError::InvalidFormat("empty COBS frame"));
            }
            let frame = cobs::decode_vec(encoded_frame)
                .map_err(|_| VaultError::InvalidFormat("invalid COBS frame"))?;
            if frame.len() < NONCE_LEN + 16 {
                return Err(VaultError::InvalidFormat(
                    "encrypted document frame is too short",
                ));
            }
            let (nonce_bytes, encrypted) = frame.split_at(NONCE_LEN);
            let nonce_bytes: [u8; NONCE_LEN] = nonce_bytes
                .try_into()
                .map_err(|_| VaultError::InvalidFormat("invalid nonce length"))?;
            let nonce = Nonce::from(nonce_bytes);
            let plaintext = cipher
                .decrypt(
                    &nonce,
                    Payload {
                        msg: encrypted,
                        aad: DOCUMENT_AAD,
                    },
                )
                .map_err(|_| VaultError::Crypto)?;
            let document: StoredDocument<T> = postcard::from_bytes(&plaintext)?;
            if document.kind != expected_kind {
                return Err(VaultError::DocumentKindMismatch {
                    expected: expected_kind.to_owned(),
                    actual: document.kind,
                });
            }
            if document.schema_version != expected_schema_version {
                return Err(VaultError::DocumentVersionMismatch {
                    expected: expected_schema_version,
                    actual: document.schema_version,
                });
            }
            if document.id.is_empty() {
                return Err(VaultError::InvalidFormat("document ID cannot be empty"));
            }
            if !ids.insert(document.id.clone()) {
                return Err(VaultError::InvalidFormat("duplicate document ID"));
            }
            documents.push((document.id, document.value));
        }

        Ok(documents)
    }
}

fn derive_wrapping_key(
    master_password: &[u8],
    salt: &[u8; SALT_LEN],
    memory_kib: u32,
    iterations: u32,
    lanes: u32,
) -> Result<[u8; DATA_KEY_LEN], VaultError> {
    let params = Params::new(memory_kib, iterations, lanes, Some(DATA_KEY_LEN))
        .map_err(|_| VaultError::InvalidFormat("invalid Argon2 parameters"))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0; DATA_KEY_LEN];
    argon2
        .hash_password_into(master_password, salt, &mut key)
        .map_err(|_| VaultError::InvalidFormat("invalid Argon2 parameters"))?;
    Ok(key)
}

fn validate_header(bytes: &[u8]) -> Result<(), VaultError> {
    if bytes.len() < HEADER_LEN {
        return Err(VaultError::InvalidFormat("truncated header"));
    }
    if &bytes[..MAGIC.len()] != MAGIC {
        return Err(VaultError::InvalidFormat("incorrect magic value"));
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != FORMAT_VERSION {
        return Err(VaultError::UnsupportedVersion(version));
    }
    Ok(())
}

fn validate_kdf_parameters(memory_kib: u32, iterations: u32, lanes: u32) -> Result<(), VaultError> {
    if (memory_kib, iterations, lanes) != (KDF_MEMORY_KIB, KDF_ITERATIONS, KDF_LANES) {
        return Err(VaultError::InvalidFormat(
            "unsupported Argon2 parameters for this format version",
        ));
    }
    Ok(())
}

fn read_u16(bytes: &[u8], offset: &mut usize) -> Result<u16, VaultError> {
    let value = take_array::<2>(bytes, offset)?;
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, VaultError> {
    let value = take_array::<4>(bytes, offset)?;
    Ok(u32::from_le_bytes(value))
}

fn take_array<const N: usize>(bytes: &[u8], offset: &mut usize) -> Result<[u8; N], VaultError> {
    let end = offset
        .checked_add(N)
        .ok_or(VaultError::InvalidFormat("header length overflow"))?;
    let slice = bytes
        .get(*offset..end)
        .ok_or(VaultError::InvalidFormat("truncated header"))?;
    *offset = end;
    slice
        .try_into()
        .map_err(|_| VaultError::InvalidFormat("invalid header field length"))
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_path = path.with_extension(format!("tmp-{}-{counter}", std::process::id()));
    let write_result = (|| -> Result<(), VaultError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp_path, path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn write_new_atomically(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_path = path.with_extension(format!("tmp-{}-{counter}", std::process::id()));
    let write_result = (|| -> Result<(), VaultError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::hard_link(&temp_path, path)?;
        fs::remove_file(&temp_path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("repass-{name}-{}.vault", std::process::id()))
    }

    #[test]
    fn creates_saves_and_reopens_independent_documents() {
        let path = test_path("round-trip");
        let vault = Vault::create(&path, b"master password").unwrap();
        let documents = vec![
            ("doc-1".to_owned(), "alpha".to_owned()),
            ("doc-2".to_owned(), "beta\nwith newline".to_owned()),
        ];
        vault.save_documents("note", 1, &documents).unwrap();

        let reopened = Vault::open(&path, b"master password").unwrap();
        let loaded: Vec<(String, String)> = reopened.load_documents("note", 1).unwrap();
        assert_eq!(loaded, documents);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_wrong_password_and_corrupted_frame() {
        let path = test_path("corruption");
        let vault = Vault::create(&path, b"master password").unwrap();
        vault
            .save_documents("note", 1, &[("doc-1".to_owned(), "secret")])
            .unwrap();
        assert!(matches!(
            Vault::open(&path, b"wrong password"),
            Err(VaultError::Crypto)
        ));

        let mut bytes = fs::read(&path).unwrap();
        let last_data_byte = bytes.len() - 2;
        bytes[last_data_byte] ^= 1;
        fs::write(&path, bytes).unwrap();
        let reopened = Vault::open(&path, b"master password").unwrap();
        assert!(matches!(
            reopened.load_documents::<String>("note", 1),
            Err(VaultError::Crypto) | Err(VaultError::InvalidFormat(_))
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn empty_vault_has_no_document_frames() {
        let path = test_path("empty");
        let vault = Vault::create(&path, b"master password").unwrap();
        let loaded: Vec<(String, String)> = vault.load_documents("note", 1).unwrap();
        assert!(loaded.is_empty());
        fs::remove_file(path).unwrap();
    }
}
