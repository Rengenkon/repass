use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, Generate, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, KeyInit, Nonce};
use rand::RngExt;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const METADATA_MAGIC: &[u8; 8] = b"REPMETA\0";
const DATA_MAGIC: &[u8; 8] = b"REPDATA\0";
const FORMAT_VERSION: u16 = 1;
const DATA_KEY_LEN: usize = 32;
const SALT_LEN: usize = 16;
const STORAGE_ID_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const WRAPPED_KEY_LEN: usize = DATA_KEY_LEN + 16;
const KDF_MEMORY_KIB: u32 = 19 * 1024;
const KDF_ITERATIONS: u32 = 2;
const KDF_LANES: u32 = 1;
const HEADER_PREFIX_LEN: usize = 8 + 2 + 4 + 4 + 4 + SALT_LEN + STORAGE_ID_LEN;
const HEADER_LEN: usize = HEADER_PREFIX_LEN + NONCE_LEN + WRAPPED_KEY_LEN;
const DATA_HEADER_LEN: usize = 8 + 2;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum VaultError {
    Io(std::io::Error),
    InvalidFormat(&'static str),
    UnsupportedVersion(u16),
    Crypto,
    Serialization(postcard::Error),
    DataKindMismatch { expected: String, actual: String },
    DataVersionMismatch { expected: u16, actual: u16 },
}

impl Display for VaultError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "vault I/O error: {error}"),
            Self::InvalidFormat(reason) => write!(formatter, "invalid vault format: {reason}"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported vault format version: {version}")
            }
            Self::Crypto => formatter.write_str("vault authentication or decryption failed"),
            Self::Serialization(error) => write!(formatter, "vault serialization failed: {error}"),
            Self::DataKindMismatch { expected, actual } => {
                write!(
                    formatter,
                    "expected data file {expected:?}, found {actual:?}"
                )
            }
            Self::DataVersionMismatch { expected, actual } => write!(
                formatter,
                "expected data schema version {expected}, found {actual}"
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

#[derive(Deserialize, Serialize)]
struct DataFile<T> {
    storage_id: [u8; STORAGE_ID_LEN],
    kind: String,
    schema_version: u16,
    value: T,
}

/// Key metadata for a directory vault. The metadata file contains the wrapped
/// data key; records and tags are stored as separate authenticated data files.
pub struct Vault {
    path: PathBuf,
    data_key: [u8; DATA_KEY_LEN],
    storage_id: [u8; STORAGE_ID_LEN],
}

impl Vault {
    /// Creates key metadata without overwriting an existing file.
    pub fn create(path: impl AsRef<Path>, master_password: &[u8]) -> Result<Self, VaultError> {
        let mut salt = [0; SALT_LEN];
        let mut storage_id = [0; STORAGE_ID_LEN];
        rand::rng().fill(&mut salt);
        rand::rng().fill(&mut storage_id);

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

        let mut header = Vec::with_capacity(HEADER_LEN);
        header.extend_from_slice(METADATA_MAGIC);
        header.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        header.extend_from_slice(&KDF_MEMORY_KIB.to_le_bytes());
        header.extend_from_slice(&KDF_ITERATIONS.to_le_bytes());
        header.extend_from_slice(&KDF_LANES.to_le_bytes());
        header.extend_from_slice(&salt);
        header.extend_from_slice(&storage_id);

        let nonce = Nonce::generate();
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
            storage_id,
        })
    }

    /// Opens key metadata and verifies the wrapped data key.
    pub fn open(path: impl AsRef<Path>, master_password: &[u8]) -> Result<Self, VaultError> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.len() != HEADER_LEN {
            return Err(VaultError::InvalidFormat("invalid metadata file length"));
        }
        if &bytes[..METADATA_MAGIC.len()] != METADATA_MAGIC {
            return Err(VaultError::InvalidFormat("incorrect metadata magic value"));
        }

        let mut offset = METADATA_MAGIC.len();
        let version = read_u16(&bytes, &mut offset)?;
        if version != FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(version));
        }
        let memory_kib = read_u32(&bytes, &mut offset)?;
        let iterations = read_u32(&bytes, &mut offset)?;
        let lanes = read_u32(&bytes, &mut offset)?;
        validate_kdf_parameters(memory_kib, iterations, lanes)?;
        let salt = take_array::<SALT_LEN>(&bytes, &mut offset)?;
        let storage_id = take_array::<STORAGE_ID_LEN>(&bytes, &mut offset)?;
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
            .map_err(|_| VaultError::InvalidFormat("invalid wrapped key length"))?;

        Ok(Self {
            path,
            data_key,
            storage_id,
        })
    }

    /// Atomically writes one encrypted, typed data file.
    pub fn save_data_file<T: Serialize>(
        &self,
        path: impl AsRef<Path>,
        kind: &str,
        schema_version: u16,
        value: &T,
    ) -> Result<(), VaultError> {
        let aad = data_aad(self.storage_id, kind, schema_version)?;
        let document = DataFile {
            storage_id: self.storage_id,
            kind: kind.to_owned(),
            schema_version,
            value,
        };
        let plaintext = postcard::to_allocvec(&document)?;
        let cipher =
            ChaCha20Poly1305::new_from_slice(&self.data_key).map_err(|_| VaultError::Crypto)?;
        let nonce = Nonce::generate();
        let encrypted = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: &plaintext,
                    aad: &aad,
                },
            )
            .map_err(|_| VaultError::Crypto)?;
        let mut bytes = Vec::with_capacity(DATA_HEADER_LEN + NONCE_LEN + encrypted.len());
        bytes.extend_from_slice(DATA_MAGIC);
        bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(nonce.as_slice());
        bytes.extend_from_slice(&encrypted);
        write_atomically(path.as_ref(), &bytes)
    }

    /// Loads and authenticates one encrypted, typed data file.
    pub fn load_data_file<T: DeserializeOwned>(
        &self,
        path: impl AsRef<Path>,
        expected_kind: &str,
        expected_schema_version: u16,
    ) -> Result<T, VaultError> {
        let bytes = fs::read(path)?;
        if bytes.len() < DATA_HEADER_LEN + NONCE_LEN + 16 {
            return Err(VaultError::InvalidFormat("truncated data file"));
        }
        if &bytes[..DATA_MAGIC.len()] != DATA_MAGIC {
            return Err(VaultError::InvalidFormat("incorrect data-file magic value"));
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(version));
        }
        let nonce_bytes: [u8; NONCE_LEN] = bytes[DATA_HEADER_LEN..DATA_HEADER_LEN + NONCE_LEN]
            .try_into()
            .map_err(|_| VaultError::InvalidFormat("invalid data-file nonce length"))?;
        let nonce = Nonce::from(nonce_bytes);
        let aad = data_aad(self.storage_id, expected_kind, expected_schema_version)?;
        let cipher =
            ChaCha20Poly1305::new_from_slice(&self.data_key).map_err(|_| VaultError::Crypto)?;
        let plaintext = cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &bytes[DATA_HEADER_LEN + NONCE_LEN..],
                    aad: &aad,
                },
            )
            .map_err(|_| VaultError::Crypto)?;
        let document: DataFile<T> = postcard::from_bytes(&plaintext)?;
        if document.storage_id != self.storage_id {
            return Err(VaultError::InvalidFormat(
                "data file belongs to another vault",
            ));
        }
        if document.kind != expected_kind {
            return Err(VaultError::DataKindMismatch {
                expected: expected_kind.to_owned(),
                actual: document.kind,
            });
        }
        if document.schema_version != expected_schema_version {
            return Err(VaultError::DataVersionMismatch {
                expected: expected_schema_version,
                actual: document.schema_version,
            });
        }
        Ok(document.value)
    }

    pub fn metadata_path(&self) -> &Path {
        &self.path
    }
}

fn data_aad(
    storage_id: [u8; STORAGE_ID_LEN],
    kind: &str,
    schema_version: u16,
) -> Result<Vec<u8>, VaultError> {
    if kind.is_empty() {
        return Err(VaultError::InvalidFormat("data-file kind cannot be empty"));
    }
    let kind_length = u32::try_from(kind.len())
        .map_err(|_| VaultError::InvalidFormat("data-file kind is too long"))?;
    let mut aad = Vec::with_capacity(DATA_HEADER_LEN + STORAGE_ID_LEN + 4 + kind.len() + 2);
    aad.extend_from_slice(DATA_MAGIC);
    aad.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    aad.extend_from_slice(&storage_id);
    aad.extend_from_slice(&kind_length.to_le_bytes());
    aad.extend_from_slice(kind.as_bytes());
    aad.extend_from_slice(&schema_version.to_le_bytes());
    Ok(aad)
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
        let _ = fs::remove_file(&temp_path);
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

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("repass-{name}-{}", std::process::id()))
    }

    #[test]
    fn metadata_and_independent_data_files_round_trip() {
        let directory = test_directory("separate-files");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let metadata_path = directory.join("metadata.repass");
        let records_path = directory.join("records.repass");
        let tags_path = directory.join("tags.repass");
        let vault = Vault::create(&metadata_path, b"master password").unwrap();
        let records = vec!["one", "two"];
        let tags = vec!["work"];
        vault
            .save_data_file(&records_path, "records", 1, &records)
            .unwrap();
        vault.save_data_file(&tags_path, "tags", 1, &tags).unwrap();

        let reopened = Vault::open(&metadata_path, b"master password").unwrap();
        let loaded_records: Vec<String> = reopened
            .load_data_file(&records_path, "records", 1)
            .unwrap();
        let loaded_tags: Vec<String> = reopened.load_data_file(&tags_path, "tags", 1).unwrap();
        assert_eq!(loaded_records, ["one", "two"]);
        assert_eq!(loaded_tags, ["work"]);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn data_files_are_bound_to_vault_and_kind() {
        let directory = test_directory("bound-files");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let first_path = directory.join("metadata-1.repass");
        let second_path = directory.join("metadata-2.repass");
        let data_path = directory.join("records.repass");
        let first = Vault::create(&first_path, b"master").unwrap();
        first
            .save_data_file(&data_path, "records", 1, &"record")
            .unwrap();
        let second = Vault::create(&second_path, b"master").unwrap();
        assert!(matches!(
            second.load_data_file::<String>(&data_path, "records", 1),
            Err(VaultError::Crypto)
        ));
        assert!(matches!(
            first.load_data_file::<String>(&data_path, "tags", 1),
            Err(VaultError::Crypto)
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn wrong_password_and_corrupt_data_are_rejected() {
        let directory = test_directory("corruption");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let metadata_path = directory.join("metadata.repass");
        let data_path = directory.join("records.repass");
        let vault = Vault::create(&metadata_path, b"master password").unwrap();
        vault
            .save_data_file(&data_path, "records", 1, &"secret")
            .unwrap();
        assert!(matches!(
            Vault::open(&metadata_path, b"wrong password"),
            Err(VaultError::Crypto)
        ));

        let mut bytes = fs::read(&data_path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        fs::write(&data_path, bytes).unwrap();
        assert!(matches!(
            vault.load_data_file::<String>(&data_path, "records", 1),
            Err(VaultError::Crypto)
        ));
        fs::remove_dir_all(directory).unwrap();
    }
}
