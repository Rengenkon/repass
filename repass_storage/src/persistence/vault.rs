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
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const METADATA_MAGIC: &[u8; 8] = b"REPMETA\0";
const DATA_MAGIC: &[u8; 8] = b"REPDATA\0";
const COUNTS_MAGIC: &[u8; 8] = b"REPCNT\0\0";
const METADATA_FORMAT_VERSION: u16 = 2;
const DATA_FORMAT_VERSION: u16 = 2;
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
const DATA_HEADER_PREFIX_LEN: usize = 8 + 2;
const DATA_HEADER_LEN: usize = 8 + 2 + 2;
const COUNTS_HEADER_LEN: usize = 8 + NONCE_LEN;
const COUNTS_PLAINTEXT_LEN: usize = 16;
const COUNTS_CIPHERTEXT_LEN: usize = COUNTS_PLAINTEXT_LEN + 16;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
thread_local! {
    static WRITE_FAILURE: std::cell::RefCell<Option<(PathBuf, bool)>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn fail_next_write(path: PathBuf, after_publication: bool) {
    WRITE_FAILURE.with(|failure| *failure.borrow_mut() = Some((path, after_publication)));
}

#[derive(Debug)]
pub enum VaultError {
    EmptyMasterPassword,
    Io(std::io::Error),
    InvalidFormat(&'static str),
    UnsupportedVersion(u16),
    Crypto,
    Serialization(postcard::Error),
    DataKindMismatch { expected: String, actual: String },
    DataVersionMismatch { expected: u16, actual: u16 },
    WriteCommitted(std::io::Error),
}

impl Display for VaultError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMasterPassword => formatter.write_str("master password cannot be empty"),
            Self::WriteCommitted(error) => write!(
                formatter,
                "file was replaced, but directory synchronization failed: {error}"
            ),
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
            Self::Io(error) | Self::WriteCommitted(error) => Some(error),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VaultCounts {
    pub records: u64,
    pub tags: u64,
}

/// Key metadata for a directory vault. The metadata file contains the wrapped
/// data key; records and tags are stored as separate authenticated data files.
pub struct Vault {
    path: PathBuf,
    data_key: [u8; DATA_KEY_LEN],
    storage_id: [u8; STORAGE_ID_LEN],
    metadata_header: Vec<u8>,
    counts: VaultCounts,
    counts_valid: bool,
}

impl Vault {
    /// Creates key metadata without overwriting an existing file.
    pub fn create(path: impl AsRef<Path>, master_password: &[u8]) -> Result<Self, VaultError> {
        if master_password.is_empty() {
            return Err(VaultError::EmptyMasterPassword);
        }
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
        let header = metadata_header(&wrapping_key, &data_key_bytes, &salt, &storage_id)?;
        let counts = VaultCounts {
            records: 0,
            tags: 0,
        };
        let mut bytes = header.clone();
        bytes.extend_from_slice(&encode_counts(&data_key_bytes, storage_id, counts)?);

        let path = path.as_ref().to_path_buf();
        write_new_atomically(&path, &bytes)?;
        write_atomically(&backup_path(&path), &bytes)?;
        Ok(Self {
            path,
            data_key: data_key_bytes,
            storage_id,
            metadata_header: header,
            counts,
            counts_valid: true,
        })
    }

    /// Opens key metadata and verifies the wrapped data key.
    pub fn open(path: impl AsRef<Path>, master_password: &[u8]) -> Result<Self, VaultError> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.len() < METADATA_MAGIC.len() + 2 {
            return Err(VaultError::InvalidFormat("truncated metadata file"));
        }
        if &bytes[..METADATA_MAGIC.len()] != METADATA_MAGIC {
            return Err(VaultError::InvalidFormat("incorrect metadata magic value"));
        }

        let mut offset = METADATA_MAGIC.len();
        let version = read_u16(&bytes, &mut offset)?;
        if version != METADATA_FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(version));
        }
        let expected_len = HEADER_LEN + COUNTS_HEADER_LEN + COUNTS_CIPHERTEXT_LEN;
        if bytes.len() < HEADER_LEN || bytes.len() > expected_len {
            return Err(VaultError::InvalidFormat("invalid metadata file length"));
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

        let metadata_header = bytes[..HEADER_LEN].to_vec();
        let decoded_counts = decode_counts(&data_key, storage_id, &bytes[HEADER_LEN..]);
        let counts_valid = decoded_counts.is_ok();
        let counts = decoded_counts.unwrap_or(VaultCounts {
            records: 0,
            tags: 0,
        });

        Ok(Self {
            path,
            data_key,
            storage_id,
            metadata_header,
            counts,
            counts_valid,
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
        let aad = data_aad(DATA_FORMAT_VERSION, self.storage_id, kind, schema_version)?;
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
        bytes.extend_from_slice(&DATA_FORMAT_VERSION.to_le_bytes());
        bytes.extend_from_slice(&schema_version.to_le_bytes());
        bytes.extend_from_slice(nonce.as_slice());
        bytes.extend_from_slice(&encrypted);
        let path = path.as_ref();
        match fs::read(path) {
            Ok(previous) if self.decrypt_data(&previous, kind, schema_version).is_ok() => {
                write_atomically(&backup_path(path), &previous).map_err(before_primary_write)?;
            }
            Ok(_) => {} // Preserve an existing good backup during explicit recovery.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                write_atomically(&backup_path(path), &bytes).map_err(before_primary_write)?;
            }
            Err(error) => return Err(error.into()),
        }
        write_atomically(path, &bytes)
    }

    /// Loads and authenticates one encrypted, typed data file.
    pub(crate) fn load_data_file<T: DeserializeOwned>(
        &self,
        path: impl AsRef<Path>,
        expected_kind: &str,
        expected_schema_version: u16,
    ) -> Result<T, VaultError> {
        let bytes = fs::read(path)?;
        let plaintext = self.decrypt_data(&bytes, expected_kind, expected_schema_version)?;
        let (document, trailing): (DataFile<T>, _) = postcard::take_from_bytes(&plaintext)?;
        if !trailing.is_empty() {
            return Err(VaultError::InvalidFormat("trailing bytes in data file"));
        }
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
            return Err(VaultError::InvalidFormat(
                "data-file schema header does not match its payload",
            ));
        }
        Ok(document.value)
    }

    fn decrypt_data(
        &self,
        bytes: &[u8],
        expected_kind: &str,
        expected_schema_version: u16,
    ) -> Result<Vec<u8>, VaultError> {
        if bytes.len() < DATA_HEADER_LEN + NONCE_LEN + 16 {
            return Err(VaultError::InvalidFormat("truncated data file"));
        }
        if &bytes[..DATA_MAGIC.len()] != DATA_MAGIC {
            return Err(VaultError::InvalidFormat("incorrect data-file magic value"));
        }
        let version = u16::from_le_bytes([bytes[8], bytes[9]]);
        if version != DATA_FORMAT_VERSION {
            return Err(VaultError::UnsupportedVersion(version));
        }
        let file_schema_version = u16::from_le_bytes([
            bytes[DATA_HEADER_PREFIX_LEN],
            bytes[DATA_HEADER_PREFIX_LEN + 1],
        ]);
        let nonce_bytes: [u8; NONCE_LEN] = bytes[DATA_HEADER_LEN..DATA_HEADER_LEN + NONCE_LEN]
            .try_into()
            .map_err(|_| VaultError::InvalidFormat("invalid data-file nonce length"))?;
        let nonce = Nonce::from(nonce_bytes);
        let aad = data_aad(
            DATA_FORMAT_VERSION,
            self.storage_id,
            expected_kind,
            file_schema_version,
        )?;
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
        if file_schema_version != expected_schema_version {
            return Err(VaultError::DataVersionMismatch {
                expected: expected_schema_version,
                actual: file_schema_version,
            });
        }
        Ok(plaintext)
    }

    pub(crate) fn counts(&self) -> VaultCounts {
        self.counts
    }

    pub(crate) fn counts_valid(&self) -> bool {
        self.counts_valid
    }

    pub(crate) fn save_counts(&mut self, counts: VaultCounts) -> Result<(), VaultError> {
        if self.counts_valid && self.counts == counts {
            return Ok(());
        }
        let mut bytes = self.metadata_header.clone();
        bytes.extend_from_slice(&encode_counts(&self.data_key, self.storage_id, counts)?);
        let result = write_atomically(&self.path, &bytes);
        if result.is_ok() || matches!(result, Err(VaultError::WriteCommitted(_))) {
            self.counts = counts;
            self.counts_valid = true;
        }
        result
    }

    /// Rewraps the existing data key; record and tag files retain their encoding.
    pub(crate) fn change_password(&mut self, password: &[u8]) -> Result<(), VaultError> {
        if password.is_empty() {
            return Err(VaultError::EmptyMasterPassword);
        }
        let mut salt = [0; SALT_LEN];
        rand::rng().fill(&mut salt);
        let key = derive_wrapping_key(password, &salt, KDF_MEMORY_KIB, KDF_ITERATIONS, KDF_LANES)?;
        let header = metadata_header(&key, &self.data_key, &salt, &self.storage_id)?;
        let mut bytes = header.clone();
        bytes.extend_from_slice(&encode_counts(
            &self.data_key,
            self.storage_id,
            self.counts,
        )?);
        // Update the backup first so it never retains the old password after success.
        write_atomically(&backup_path(&self.path), &bytes).map_err(before_primary_write)?;
        let result = write_atomically(&self.path, &bytes);
        if result.is_ok() || matches!(result, Err(VaultError::WriteCommitted(_))) {
            self.metadata_header = header;
            self.counts_valid = true;
        }
        result
    }

    pub(crate) fn restore_data_file(
        &self,
        path: &Path,
        kind: &str,
        version: u16,
    ) -> Result<(), VaultError> {
        let backup = fs::read(backup_path(path))?;
        self.decrypt_data(&backup, kind, version)?;
        preserve_damaged(path)?;
        write_atomically(path, &backup)
    }
}

fn metadata_header(
    wrapping_key: &[u8; DATA_KEY_LEN],
    data_key: &[u8; DATA_KEY_LEN],
    salt: &[u8; SALT_LEN],
    storage_id: &[u8; STORAGE_ID_LEN],
) -> Result<Vec<u8>, VaultError> {
    let mut header = Vec::with_capacity(HEADER_LEN);
    header.extend_from_slice(METADATA_MAGIC);
    header.extend_from_slice(&METADATA_FORMAT_VERSION.to_le_bytes());
    header.extend_from_slice(&KDF_MEMORY_KIB.to_le_bytes());
    header.extend_from_slice(&KDF_ITERATIONS.to_le_bytes());
    header.extend_from_slice(&KDF_LANES.to_le_bytes());
    header.extend_from_slice(salt);
    header.extend_from_slice(storage_id);

    let cipher = ChaCha20Poly1305::new_from_slice(wrapping_key).map_err(|_| VaultError::Crypto)?;
    let nonce = Nonce::generate();
    let encrypted_key = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: data_key,
                aad: &header,
            },
        )
        .map_err(|_| VaultError::Crypto)?;
    header.extend_from_slice(nonce.as_slice());
    header.extend_from_slice(&encrypted_key);
    Ok(header)
}

fn counts_aad(storage_id: [u8; STORAGE_ID_LEN]) -> Vec<u8> {
    let mut aad =
        Vec::with_capacity(METADATA_MAGIC.len() + 2 + STORAGE_ID_LEN + COUNTS_MAGIC.len());
    aad.extend_from_slice(METADATA_MAGIC);
    aad.extend_from_slice(&METADATA_FORMAT_VERSION.to_le_bytes());
    aad.extend_from_slice(&storage_id);
    aad.extend_from_slice(COUNTS_MAGIC);
    aad
}

fn encode_counts(
    data_key: &[u8; DATA_KEY_LEN],
    storage_id: [u8; STORAGE_ID_LEN],
    counts: VaultCounts,
) -> Result<Vec<u8>, VaultError> {
    let cipher = ChaCha20Poly1305::new_from_slice(data_key).map_err(|_| VaultError::Crypto)?;
    let nonce = Nonce::generate();
    let mut plaintext = [0; COUNTS_PLAINTEXT_LEN];
    plaintext[..8].copy_from_slice(&counts.records.to_le_bytes());
    plaintext[8..].copy_from_slice(&counts.tags.to_le_bytes());
    let encrypted = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: &plaintext,
                aad: &counts_aad(storage_id),
            },
        )
        .map_err(|_| VaultError::Crypto)?;
    let mut bytes = Vec::with_capacity(COUNTS_HEADER_LEN + encrypted.len());
    bytes.extend_from_slice(COUNTS_MAGIC);
    bytes.extend_from_slice(nonce.as_slice());
    bytes.extend_from_slice(&encrypted);
    Ok(bytes)
}

fn decode_counts(
    data_key: &[u8; DATA_KEY_LEN],
    storage_id: [u8; STORAGE_ID_LEN],
    bytes: &[u8],
) -> Result<VaultCounts, VaultError> {
    if bytes.len() != COUNTS_HEADER_LEN + COUNTS_CIPHERTEXT_LEN
        || &bytes[..COUNTS_MAGIC.len()] != COUNTS_MAGIC
    {
        return Err(VaultError::InvalidFormat("invalid metadata counts section"));
    }
    let nonce_bytes: [u8; NONCE_LEN] = bytes[COUNTS_MAGIC.len()..COUNTS_HEADER_LEN]
        .try_into()
        .map_err(|_| VaultError::InvalidFormat("invalid metadata counts nonce"))?;
    let cipher = ChaCha20Poly1305::new_from_slice(data_key).map_err(|_| VaultError::Crypto)?;
    let plaintext = cipher
        .decrypt(
            &Nonce::from(nonce_bytes),
            Payload {
                msg: &bytes[COUNTS_HEADER_LEN..],
                aad: &counts_aad(storage_id),
            },
        )
        .map_err(|_| VaultError::Crypto)?;
    if plaintext.len() != COUNTS_PLAINTEXT_LEN {
        return Err(VaultError::InvalidFormat("invalid metadata counts length"));
    }
    let records = u64::from_le_bytes(
        plaintext[..8]
            .try_into()
            .map_err(|_| VaultError::InvalidFormat("invalid record count"))?,
    );
    let tags = u64::from_le_bytes(
        plaintext[8..]
            .try_into()
            .map_err(|_| VaultError::InvalidFormat("invalid tag count"))?,
    );
    Ok(VaultCounts { records, tags })
}

fn data_aad(
    format_version: u16,
    storage_id: [u8; STORAGE_ID_LEN],
    kind: &str,
    schema_version: u16,
) -> Result<Vec<u8>, VaultError> {
    if kind.is_empty() {
        return Err(VaultError::InvalidFormat("data-file kind cannot be empty"));
    }
    let kind_length = u32::try_from(kind.len())
        .map_err(|_| VaultError::InvalidFormat("data-file kind is too long"))?;
    let mut aad = Vec::with_capacity(DATA_HEADER_PREFIX_LEN + STORAGE_ID_LEN + 4 + kind.len() + 2);
    aad.extend_from_slice(DATA_MAGIC);
    aad.extend_from_slice(&format_version.to_le_bytes());
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
    #[cfg(test)]
    let injected = WRITE_FAILURE.with(|failure| {
        let mut failure = failure.borrow_mut();
        if failure.as_ref().is_some_and(|(target, _)| target == path) {
            failure.take().map(|(_, after)| after)
        } else {
            None
        }
    });
    #[cfg(test)]
    if injected == Some(false) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "injected write failure",
        )
        .into());
    }
    let (temp_path, mut file) = create_temporary(path)?;
    let write_result = (|| -> Result<(), VaultError> {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp_path, path)?;
        #[cfg(test)]
        if injected == Some(true) {
            return Err(VaultError::WriteCommitted(std::io::Error::other(
                "injected directory synchronization failure",
            )));
        }
        sync_parent(path).map_err(VaultError::WriteCommitted)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn sync_parent(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

pub(crate) fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    name.into()
}

pub(crate) fn write_recovered(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    write_atomically(path, bytes)
}

pub(crate) fn preserve_damaged(path: &Path) -> Result<(), VaultError> {
    if !path.try_exists()? {
        return Ok(());
    }
    let bytes = fs::read(path)?;
    loop {
        let id = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut name = path.as_os_str().to_os_string();
        name.push(format!(".damaged-{}-{id}", std::process::id()));
        match write_new_atomically(Path::new(&name), &bytes) {
            Err(VaultError::Io(error)) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                continue;
            }
            result => return result,
        }
    }
}

fn write_new_atomically(path: &Path, bytes: &[u8]) -> Result<(), VaultError> {
    let (temp_path, mut file) = create_temporary(path)?;
    let write_result = (|| -> Result<(), VaultError> {
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::hard_link(&temp_path, path)?;
        let _ = fs::remove_file(&temp_path);
        sync_parent(path).map_err(VaultError::WriteCommitted)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn create_temporary(path: &Path) -> Result<(PathBuf, File), VaultError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        // New files are owner-only. Replacements also preserve stricter owner
        // permissions, e.g. a file deliberately made read-only (0400).
        let mode = match fs::metadata(path) {
            Ok(metadata) => metadata.permissions().mode() & 0o600,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0o600,
            Err(error) => return Err(error.into()),
        };
        options.mode(mode);
    }
    for _ in 0..64 {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp = path.with_extension(format!("tmp-{}-{counter}", std::process::id()));
        match options.open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "cannot allocate an unused temporary vault file",
    )
    .into())
}

fn before_primary_write(error: VaultError) -> VaultError {
    match error {
        VaultError::WriteCommitted(error) => VaultError::Io(error),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("repass-{name}-{}", std::process::id()))
    }

    #[test]
    fn empty_master_password_is_rejected_without_publishing_metadata() {
        let directory = test_directory("empty-password");
        let _ = fs::remove_dir_all(&directory);
        let path = directory.join("metadata.repass");
        assert!(matches!(
            Vault::create(&path, b""),
            Err(VaultError::EmptyMasterPassword)
        ));
        assert!(!directory.exists());
        fs::create_dir(&directory).unwrap();
        let mut vault = Vault::create(&path, b"master").unwrap();
        let primary = fs::read(&path).unwrap();
        let backup = fs::read(backup_path(&path)).unwrap();
        assert!(matches!(
            vault.change_password(b""),
            Err(VaultError::EmptyMasterPassword)
        ));
        assert_eq!(fs::read(&path).unwrap(), primary);
        assert_eq!(fs::read(backup_path(&path)).unwrap(), backup);
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn atomic_writes_restrict_access_and_preserve_stricter_owner_permissions() {
        let directory = test_directory("file-modes");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir(&directory).unwrap();
        let path = directory.join("records.repass");
        let (temp, file) = create_temporary(&path).unwrap();
        assert_eq!(file.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        drop(file);
        fs::remove_file(temp).unwrap();
        write_new_atomically(&path, b"initial").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        write_atomically(&path, b"restricted").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        write_atomically(&path, b"read-only replacement").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o400
        );
        assert_eq!(fs::read(&path).unwrap(), b"read-only replacement");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn metadata_counts_round_trip_and_authenticate() {
        let directory = test_directory("metadata-counts");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let metadata_path = directory.join("metadata.repass");
        let mut vault = Vault::create(&metadata_path, b"master").unwrap();
        assert_eq!(
            vault.counts(),
            VaultCounts {
                records: 0,
                tags: 0
            }
        );
        vault
            .save_counts(VaultCounts {
                records: 17,
                tags: 4,
            })
            .unwrap();
        drop(vault);

        let reopened = Vault::open(&metadata_path, b"master").unwrap();
        assert_eq!(
            reopened.counts(),
            VaultCounts {
                records: 17,
                tags: 4,
            }
        );
        let mut bytes = fs::read(&metadata_path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(&metadata_path, bytes).unwrap();
        assert!(
            !Vault::open(&metadata_path, b"master")
                .unwrap()
                .counts_valid()
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn unsupported_metadata_version_is_rejected_without_migration() {
        let directory = test_directory("unsupported-metadata");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let metadata_path = directory.join("metadata.repass");
        let _vault = Vault::create(&metadata_path, b"master").unwrap();
        let mut bytes = fs::read(&metadata_path).unwrap();
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        fs::write(&metadata_path, bytes).unwrap();
        assert!(matches!(
            Vault::open(&metadata_path, b"master"),
            Err(VaultError::UnsupportedVersion(1))
        ));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn unsupported_data_version_is_rejected_without_migration() {
        let directory = test_directory("unsupported-data-version");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let metadata_path = directory.join("metadata.repass");
        let data_path = directory.join("records.repass");
        let vault = Vault::create(&metadata_path, b"master").unwrap();
        vault
            .save_data_file(&data_path, "records", 1, &"record")
            .unwrap();
        let mut bytes = fs::read(&data_path).unwrap();
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        fs::write(&data_path, bytes).unwrap();
        assert!(matches!(
            vault.load_data_file::<String>(&data_path, "records", 1),
            Err(VaultError::UnsupportedVersion(1))
        ));
        fs::remove_dir_all(directory).unwrap();
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
    fn data_schema_mismatch_is_reported_after_authenticating_the_versioned_header() {
        let directory = test_directory("schema-mismatch");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        let metadata_path = directory.join("metadata.repass");
        let data_path = directory.join("records.repass");
        let vault = Vault::create(&metadata_path, b"master").unwrap();
        vault
            .save_data_file(&data_path, "records", 2, &"new schema")
            .unwrap();
        assert!(matches!(
            vault.load_data_file::<String>(&data_path, "records", 1),
            Err(VaultError::DataVersionMismatch {
                expected: 1,
                actual: 2
            })
        ));
        // A genuinely incompatible value must still report the version first.
        assert!(matches!(
            vault.load_data_file::<u64>(&data_path, "records", 1),
            Err(VaultError::DataVersionMismatch {
                expected: 1,
                actual: 2
            })
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
