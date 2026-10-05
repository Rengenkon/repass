use crate::StorageError;
use crate::persistence::{
    PersistedRecord, PersistedRecords, PersistedTag, PersistedTags, Vault, VaultCounts,
};
use crate::record::types::Timestamp;
use crate::record::{NewRecord, Record, RecordId, RecordPatch, RecordView, Records};
use crate::tags::{Tag, TagId, Tags};
use serde::Deserialize;
use serde::de::{DeserializeSeed, SeqAccess, Visitor};
use std::fmt::Formatter;
use std::fs;
use std::io;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

const METADATA_FILE: &str = "metadata.repass";
const RECORDS_FILE: &str = "records.repass";
const TAGS_FILE: &str = "tags.repass";
const LEGACY_FILE: &str = "vault.repass";
const RECORDS_KIND: &str = "records";
const TAGS_KIND: &str = "tags";
const DATA_SCHEMA_VERSION: u16 = 1;

struct VecSeed<S, T> {
    capacity: usize,
    seed: S,
    marker: PhantomData<T>,
}

impl<'de, S, T> DeserializeSeed<'de> for VecSeed<S, T>
where
    S: DeserializeSeed<'de, Value = T> + Clone,
{
    type Value = Vec<T>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(VecVisitor::<S, T> {
            capacity: self.capacity,
            seed: self.seed,
            marker: PhantomData,
        })
    }
}

struct VecVisitor<S, T> {
    capacity: usize,
    seed: S,
    marker: PhantomData<T>,
}

impl<'de, S, T> Visitor<'de> for VecVisitor<S, T>
where
    S: DeserializeSeed<'de, Value = T> + Clone,
{
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a sequence")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence.size_hint().unwrap_or(self.capacity);
        let mut values = Vec::with_capacity(capacity);
        while let Some(value) = sequence.next_element_seed(self.seed.clone())? {
            values.push(value);
        }
        Ok(values)
    }
}

#[derive(Clone, Copy)]
struct RecordSeed;

impl<'de> DeserializeSeed<'de> for RecordSeed {
    type Value = Record;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        PersistedRecord::deserialize(deserializer).map(Record::from)
    }
}

#[derive(Clone, Copy)]
struct PersistedTagSeed;

impl<'de> DeserializeSeed<'de> for PersistedTagSeed {
    type Value = PersistedTag;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        PersistedTag::deserialize(deserializer)
    }
}

struct LoadedRecords {
    records: Vec<Record>,
    next_record_id: Option<u64>,
}

struct PersistedRecordsSeed {
    capacity: usize,
}

impl<'de> DeserializeSeed<'de> for PersistedRecordsSeed {
    type Value = LoadedRecords;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "PersistedRecords",
            &["records", "next_record_id"],
            PersistedRecordsVisitor {
                capacity: self.capacity,
            },
        )
    }
}

struct PersistedRecordsVisitor {
    capacity: usize,
}

impl<'de> Visitor<'de> for PersistedRecordsVisitor {
    type Value = LoadedRecords;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("persisted records and the next record ID")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let records = sequence
            .next_element_seed(VecSeed::<RecordSeed, Record> {
                capacity: self.capacity,
                seed: RecordSeed,
                marker: PhantomData,
            })?
            .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
        let next_record_id = sequence
            .next_element()?
            .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
        Ok(LoadedRecords {
            records,
            next_record_id,
        })
    }
}

struct PersistedTagsSeed {
    capacity: usize,
}

impl<'de> DeserializeSeed<'de> for PersistedTagsSeed {
    type Value = PersistedTags;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "PersistedTags",
            &["tags", "next_tag_id"],
            PersistedTagsVisitor {
                capacity: self.capacity,
            },
        )
    }
}

struct PersistedTagsVisitor {
    capacity: usize,
}

impl<'de> Visitor<'de> for PersistedTagsVisitor {
    type Value = PersistedTags;

    fn expecting(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("persisted tags and the next tag ID")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let tags = sequence
            .next_element_seed(VecSeed::<PersistedTagSeed, PersistedTag> {
                capacity: self.capacity,
                seed: PersistedTagSeed,
                marker: PhantomData,
            })?
            .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
        let next_tag_id = sequence
            .next_element()?
            .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
        Ok(PersistedTags { tags, next_tag_id })
    }
}

/// State of the optional tag-name file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TagCatalogStatus {
    Present,
    Missing,
    Unavailable(String),
}

/// Metadata returned by [`Storage::info`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageInfo {
    pub directory: PathBuf,
    pub record_count: usize,
    /// Includes temporary technical tags reconstructed from record references.
    pub tag_count: usize,
    pub tag_catalog: TagCatalogStatus,
}

/// Directory-level vault API. Record and tag data files are replaced atomically;
/// metadata counts are updated afterward and reconciled from data files on open.
/// The data file and metadata update are not a cross-file transaction.
pub struct Storage {
    directory: PathBuf,
    vault: Vault,
    records: Records,
    tags: Tags,
    tag_catalog_status: TagCatalogStatus,
}

impl Storage {
    /// Creates a new three-file vault without overwriting any existing vault
    /// files. The optional tag catalog is created on its first mutation.
    pub fn create_in(
        directory: impl AsRef<Path>,
        master_password: &[u8],
    ) -> Result<Self, StorageError> {
        let directory = owned_directory(directory)?;
        fs::create_dir_all(&directory)?;
        let metadata_exists = path_exists(&directory.join(METADATA_FILE))?;
        let records_exist = path_exists(&directory.join(RECORDS_FILE))?;
        let tags_exist = path_exists(&directory.join(TAGS_FILE))?;
        if metadata_exists && records_exist {
            return Err(StorageError::AlreadyInitialized);
        }
        if metadata_exists || records_exist || tags_exist {
            return Err(StorageError::IncompleteInitialization);
        }
        reject_legacy_file(&directory)?;

        let vault = Vault::create(directory.join(METADATA_FILE), master_password)?;
        let records = Records::new();
        save_records(&vault, &directory, &records)?;
        Ok(Self {
            directory,
            vault,
            records,
            tags: Tags::default(),
            tag_catalog_status: TagCatalogStatus::Missing,
        })
    }

    /// Opens the required metadata and record files. Missing or unreadable tag
    /// catalogs are replaced in memory by technical tags and do not prevent the
    /// records from being used.
    pub fn open_in(
        directory: impl AsRef<Path>,
        master_password: &[u8],
    ) -> Result<Self, StorageError> {
        let directory = owned_directory(directory)?;
        let metadata_path = directory.join(METADATA_FILE);
        if !path_exists(&metadata_path)? {
            reject_legacy_file(&directory)?;
        }
        let mut vault = Vault::open(&metadata_path, master_password)?;
        let saved_counts = vault.counts();
        let loaded_records: LoadedRecords = vault
            .load_data_file_seed(
                directory.join(RECORDS_FILE),
                RECORDS_KIND,
                DATA_SCHEMA_VERSION,
                PersistedRecordsSeed {
                    capacity: count_capacity(
                        saved_counts.records,
                        "record count exceeds platform capacity",
                    )?,
                },
            )
            .map_err(|error| match error {
                crate::VaultError::Io(error) if error.kind() == io::ErrorKind::NotFound => {
                    StorageError::IncompleteInitialization
                }
                error => StorageError::Vault(error),
            })?;
        let records = Records::from_records(loaded_records.records, loaded_records.next_record_id)?;
        let used_tag_ids = records.used_tag_ids();

        let (tags, tag_catalog_status) = match vault.load_data_file_seed(
            directory.join(TAGS_FILE),
            TAGS_KIND,
            DATA_SCHEMA_VERSION,
            PersistedTagsSeed {
                capacity: count_capacity(saved_counts.tags, "tag count exceeds platform capacity")?,
            },
        ) {
            Ok(persisted) => {
                let stored = persisted
                    .tags
                    .into_iter()
                    .map(|tag| (tag.id, tag.name))
                    .collect::<Vec<_>>();
                match Tags::from_catalog(
                    stored,
                    used_tag_ids.iter().copied(),
                    persisted.next_tag_id,
                ) {
                    Ok(tags) => (tags, TagCatalogStatus::Present),
                    Err(error) => (
                        Tags::from_catalog([], used_tag_ids.iter().copied(), Some(0.into()))?,
                        TagCatalogStatus::Unavailable(error.to_string()),
                    ),
                }
            }
            Err(crate::VaultError::Io(error)) if error.kind() == io::ErrorKind::NotFound => (
                Tags::from_catalog([], used_tag_ids.iter().copied(), Some(0.into()))?,
                TagCatalogStatus::Missing,
            ),
            Err(error) => (
                Tags::from_catalog([], used_tag_ids.iter().copied(), Some(0.into()))?,
                TagCatalogStatus::Unavailable(error.to_string()),
            ),
        };

        let tag_count = match &tag_catalog_status {
            TagCatalogStatus::Present => tags.iter().filter(|tag| !tag.is_technical()).count(),
            TagCatalogStatus::Missing => 0,
            TagCatalogStatus::Unavailable(_) => {
                count_capacity(saved_counts.tags, "tag count exceeds platform capacity")?
            }
        };
        let actual_counts = VaultCounts {
            records: u64::try_from(records.len())
                .map_err(|_| StorageError::InvalidState("record count exceeds metadata range"))?,
            tags: u64::try_from(tag_count)
                .map_err(|_| StorageError::InvalidState("tag count exceeds metadata range"))?,
        };
        if saved_counts != actual_counts {
            vault
                .save_counts(actual_counts)
                .map_err(StorageError::Vault)?;
        }

        Ok(Self {
            directory,
            vault,
            records,
            tags,
            tag_catalog_status,
        })
    }

    /// Opens an existing new-format vault or creates one only when none of its
    /// files exist. It never treats a missing records file as an empty vault.
    pub fn open_or_create_in(
        directory: impl AsRef<Path>,
        master_password: &[u8],
    ) -> Result<Self, StorageError> {
        let directory = owned_directory(directory)?;
        fs::create_dir_all(&directory)?;
        if path_exists(&directory.join(METADATA_FILE))? {
            return Self::open_in(directory, master_password);
        }
        reject_legacy_file(&directory)?;
        if path_exists(&directory.join(RECORDS_FILE))? || path_exists(&directory.join(TAGS_FILE))? {
            return Err(StorageError::IncompleteInitialization);
        }
        match Self::create_in(&directory, master_password) {
            Ok(storage) => Ok(storage),
            Err(StorageError::Vault(crate::VaultError::Io(error)))
                if error.kind() == io::ErrorKind::AlreadyExists =>
            {
                Self::open_in(directory, master_password)
            }
            Err(error) => Err(error),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn tag_catalog_status(&self) -> &TagCatalogStatus {
        &self.tag_catalog_status
    }

    pub fn info(&self) -> StorageInfo {
        StorageInfo {
            directory: self.directory.clone(),
            record_count: self.records.len(),
            tag_count: self.tags.len(),
            tag_catalog: self.tag_catalog_status.clone(),
        }
    }

    pub fn list_records(&self, tag: Option<TagId>) -> Result<Vec<RecordView<'_>>, StorageError> {
        if let Some(tag) = tag {
            if self.tags.get(tag).is_none() {
                return Err(StorageError::TagNotFound(tag));
            }
            Ok(self
                .records
                .views()
                .into_iter()
                .filter(|record| record.tags.contains(&tag))
                .collect())
        } else {
            Ok(self.records.views())
        }
    }

    pub fn get_record(&self, id: RecordId) -> Result<RecordView<'_>, StorageError> {
        self.records.get(id).ok_or(StorageError::RecordNotFound(id))
    }

    pub fn find_records_by_name(&self, name: &str) -> Vec<RecordId> {
        self.records.find_by_name(name)
    }

    /// Returns records that have every requested tag. An empty list matches all records.
    pub fn find_records_with_tags(&self, tags: &[TagId]) -> Vec<RecordId> {
        self.records.find_by_tags(tags)
    }

    pub fn create_record(&mut self, record: NewRecord) -> Result<RecordId, StorageError> {
        for tag in &record.tags {
            if self.tags.get(*tag).is_none() {
                return Err(StorageError::TagNotFound(*tag));
            }
        }
        let mut records = self.records.clone();
        let id = records.create(record, Timestamp::now()?)?;
        save_records(&self.vault, &self.directory, &records)?;
        self.records = records;
        self.save_counts_after_data_write()?;
        Ok(id)
    }

    pub fn update_record(
        &mut self,
        id: RecordId,
        patch: RecordPatch,
    ) -> Result<bool, StorageError> {
        for tag in patch.add_tags.iter().chain(&patch.remove_tags) {
            if self.tags.get(*tag).is_none() {
                return Err(StorageError::TagNotFound(*tag));
            }
        }
        let mut records = self.records.clone();
        let changed = records.update(id, &patch, Timestamp::now()?)?;
        if changed {
            save_records(&self.vault, &self.directory, &records)?;
            self.records = records;
            self.save_counts_after_data_write()?;
        }
        Ok(changed)
    }

    pub fn delete_record(&mut self, id: RecordId) -> Result<(), StorageError> {
        let mut records = self.records.clone();
        records.remove(id)?;
        save_records(&self.vault, &self.directory, &records)?;
        self.records = records;
        self.save_counts_after_data_write()?;
        Ok(())
    }

    pub fn create_tag(&mut self, name: impl Into<String>) -> Result<TagId, StorageError> {
        self.ensure_tag_catalog_writable()?;
        let mut tags = self.tags.clone();
        let id = tags.create(name)?;
        self.save_tags(&tags)?;
        self.tags = tags;
        self.tag_catalog_status = TagCatalogStatus::Present;
        self.save_counts_after_data_write()?;
        Ok(id)
    }

    pub fn list_tags(&self) -> Vec<Tag> {
        self.tags.iter().cloned().collect()
    }

    pub fn rename_tag(
        &mut self,
        id: TagId,
        new_name: impl Into<String>,
    ) -> Result<(), StorageError> {
        self.ensure_tag_catalog_writable()?;
        let mut tags = self.tags.clone();
        tags.rename(id, new_name)?;
        self.save_tags(&tags)?;
        self.tags = tags;
        self.tag_catalog_status = TagCatalogStatus::Present;
        self.save_counts_after_data_write()?;
        Ok(())
    }

    pub fn delete_tag(&mut self, id: TagId) -> Result<(), StorageError> {
        if self.tags.get(id).is_none() {
            return Err(StorageError::TagNotFound(id));
        }
        if self.records.uses_tag(id) {
            return Err(StorageError::TagInUse(id));
        }
        let mut tags = self.tags.clone();
        let removed = tags.remove(id)?;
        if removed.is_technical() {
            self.tags = tags;
            return Ok(());
        }
        self.ensure_tag_catalog_writable()?;
        self.save_tags(&tags)?;
        self.tags = tags;
        self.tag_catalog_status = TagCatalogStatus::Present;
        self.save_counts_after_data_write()?;
        Ok(())
    }

    /// Explicitly rebuilds an unavailable or missing tag catalog from current
    /// tags. Technical names are promoted as `#tag-<id>` entries.
    pub fn recover_tags(&mut self) -> Result<usize, StorageError> {
        let mut tags = self.tags.clone();
        tags.promote_technical();
        let recovered = tags.len();
        self.save_tags(&tags)?;
        self.tags = tags;
        self.tag_catalog_status = TagCatalogStatus::Present;
        self.save_counts_after_data_write()?;
        Ok(recovered)
    }

    fn ensure_tag_catalog_writable(&self) -> Result<(), StorageError> {
        match &self.tag_catalog_status {
            TagCatalogStatus::Unavailable(reason) => {
                Err(StorageError::TagCatalogUnavailable(reason.clone()))
            }
            TagCatalogStatus::Present | TagCatalogStatus::Missing => Ok(()),
        }
    }

    fn save_tags(&self, tags: &Tags) -> Result<(), StorageError> {
        let persisted = PersistedTags {
            tags: tags
                .persisted()
                .into_iter()
                .map(|(id, name)| PersistedTag { id, name })
                .collect(),
            next_tag_id: tags.next_id(),
        };
        self.vault.save_data_file(
            self.directory.join(TAGS_FILE),
            TAGS_KIND,
            DATA_SCHEMA_VERSION,
            &persisted,
        )?;
        Ok(())
    }

    fn save_counts_after_data_write(&mut self) -> Result<(), StorageError> {
        self.vault
            .save_counts(self.current_counts()?)
            .map_err(StorageError::MetadataCountsUpdateAfterDataSave)
    }

    fn current_counts(&self) -> Result<VaultCounts, StorageError> {
        Ok(VaultCounts {
            records: u64::try_from(self.records.len())
                .map_err(|_| StorageError::InvalidState("record count exceeds metadata range"))?,
            tags: u64::try_from(self.tags.iter().filter(|tag| !tag.is_technical()).count())
                .map_err(|_| StorageError::InvalidState("tag count exceeds metadata range"))?,
        })
    }
}

fn owned_directory(directory: impl AsRef<Path>) -> Result<PathBuf, StorageError> {
    let directory = directory.as_ref();
    if directory.as_os_str().is_empty() {
        return Err(StorageError::InvalidState("data directory cannot be empty"));
    }
    Ok(directory.to_path_buf())
}

fn count_capacity(count: u64, error: &'static str) -> Result<usize, StorageError> {
    usize::try_from(count).map_err(|_| StorageError::InvalidState(error))
}

fn records_path(directory: &Path) -> PathBuf {
    directory.join(RECORDS_FILE)
}

fn save_records(vault: &Vault, directory: &Path, records: &Records) -> Result<(), StorageError> {
    let persisted = PersistedRecords {
        records: records.persisted(),
        next_record_id: records.next_id(),
    };
    vault.save_data_file(
        records_path(directory),
        RECORDS_KIND,
        DATA_SCHEMA_VERSION,
        &persisted,
    )?;
    Ok(())
}

fn path_exists(path: &Path) -> Result<bool, StorageError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn reject_legacy_file(directory: &Path) -> Result<(), StorageError> {
    if path_exists(&directory.join(LEGACY_FILE))? {
        return Err(StorageError::LegacyFormat);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("repass-storage-{}-{id}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn record(name: &str, tags: Vec<TagId>) -> NewRecord {
        NewRecord {
            name: name.to_owned(),
            password: format!("{name}-secret"),
            username: Some("user".to_owned()),
            url: Some("https://example.test".to_owned()),
            notes: None,
            tags,
        }
    }

    #[test]
    fn metadata_counts_track_saved_records_and_tags_and_reconcile_on_open() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        let id = storage.create_record(record("mail", vec![tag])).unwrap();
        assert_eq!(
            storage.vault.counts(),
            VaultCounts {
                records: 1,
                tags: 1,
            }
        );

        storage
            .update_record(
                id,
                RecordPatch {
                    notes: crate::record::FieldUpdate::Set("note".into()),
                    ..RecordPatch::default()
                },
            )
            .unwrap();
        storage.rename_tag(tag, "renamed").unwrap();
        assert_eq!(
            storage.vault.counts(),
            VaultCounts {
                records: 1,
                tags: 1,
            }
        );

        storage
            .vault
            .save_counts(VaultCounts {
                records: 90,
                tags: 90,
            })
            .unwrap();
        drop(storage);

        let mut reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(
            reopened.vault.counts(),
            VaultCounts {
                records: 1,
                tags: 1,
            }
        );
        reopened.delete_record(id).unwrap();
        reopened.delete_tag(tag).unwrap();
        assert_eq!(
            reopened.vault.counts(),
            VaultCounts {
                records: 0,
                tags: 0,
            }
        );
    }

    #[test]
    fn record_and_tag_files_round_trip_independently() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        assert!(directory.0.join(METADATA_FILE).exists());
        assert!(directory.0.join(RECORDS_FILE).exists());
        assert!(!directory.0.join(TAGS_FILE).exists());

        let tag = storage.create_tag("work").unwrap();
        let tags_before_records = fs::read(directory.0.join(TAGS_FILE)).unwrap();
        let first = storage.create_record(record("mail", vec![tag])).unwrap();
        assert_eq!(
            fs::read(directory.0.join(TAGS_FILE)).unwrap(),
            tags_before_records
        );
        let deleted = storage.create_record(record("old", Vec::new())).unwrap();
        storage.delete_record(deleted).unwrap();
        storage
            .update_record(
                first,
                RecordPatch {
                    notes: crate::record::FieldUpdate::Set("updated".into()),
                    ..RecordPatch::default()
                },
            )
            .unwrap();
        let records_before_tag_rename = fs::read(directory.0.join(RECORDS_FILE)).unwrap();
        storage.rename_tag(tag, "work-renamed").unwrap();
        assert_eq!(
            fs::read(directory.0.join(RECORDS_FILE)).unwrap(),
            records_before_tag_rename
        );

        let reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(reopened.list_tags()[0].name(), "work-renamed");
        let records = reopened.list_records(Some(tag)).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, first);
        assert_eq!(records[0].notes, Some("updated"));
        assert_eq!(records[0].password(), "mail-secret");
        assert_eq!(reopened.find_records_by_name("mail"), vec![first]);
        assert_eq!(reopened.find_records_with_tags(&[tag]), vec![first]);
        assert_eq!(reopened.info().record_count, 1);
    }

    #[test]
    fn missing_tag_file_creates_technical_tags_and_rename_persists_one() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        storage.create_record(record("mail", vec![tag])).unwrap();
        fs::remove_file(directory.0.join(TAGS_FILE)).unwrap();

        let mut reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(reopened.tag_catalog_status(), &TagCatalogStatus::Missing);
        let technical = reopened.list_tags();
        assert_eq!(technical.len(), 1);
        assert_eq!(technical[0].id(), tag);
        assert_eq!(technical[0].name(), format!("#tag-{tag}"));
        assert!(technical[0].is_technical());

        reopened.rename_tag(tag, "renamed").unwrap();
        let reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(reopened.list_tags()[0].name(), "renamed");
        assert!(!reopened.list_tags()[0].is_technical());
    }

    #[test]
    fn corrupt_tag_file_does_not_block_records_and_requires_explicit_recovery() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        let record_id = storage.create_record(record("mail", vec![tag])).unwrap();
        fs::write(directory.0.join(TAGS_FILE), b"damaged").unwrap();

        let mut reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert!(matches!(
            reopened.tag_catalog_status(),
            TagCatalogStatus::Unavailable(_)
        ));
        assert_eq!(reopened.get_record(record_id).unwrap().name, "mail");
        assert!(reopened.rename_tag(tag, "renamed").is_err());
        assert_eq!(reopened.recover_tags().unwrap(), 1);

        let reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(reopened.list_tags()[0].name(), format!("#tag-{tag}"));
        assert!(!reopened.list_tags()[0].is_technical());
    }

    #[test]
    fn missing_tag_ids_do_not_get_reused_for_new_tags() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        let technical_id = TagId::new(12);

        let persisted = PersistedRecords {
            records: vec![PersistedRecord {
                id: RecordId::new(1),
                name: "legacy reference".into(),
                password: "secret".into(),
                username: None,
                url: None,
                notes: None,
                tags: vec![technical_id],
                created: Timestamp::from_unix_millis(1),
                updated: Timestamp::from_unix_millis(1),
            }],
            next_record_id: Some(2),
        };
        storage
            .vault
            .save_data_file(
                directory.0.join(RECORDS_FILE),
                RECORDS_KIND,
                DATA_SCHEMA_VERSION,
                &persisted,
            )
            .unwrap();
        let mut reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(reopened.create_tag("new").unwrap(), TagId::new(13));
    }

    #[test]
    fn initialization_is_non_destructive_and_tag_deletion_checks_references() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        storage.create_record(record("mail", vec![tag])).unwrap();
        assert!(Storage::create_in(&directory.0, b"replacement").is_err());
        assert!(matches!(storage.delete_tag(tag), Err(StorageError::TagInUse(id)) if id == tag));
        assert_eq!(
            Storage::open_in(&directory.0, b"master")
                .unwrap()
                .list_tags()
                .len(),
            1
        );
    }

    #[test]
    fn legacy_file_is_rejected_and_preserved() {
        let directory = TestDirectory::new();
        fs::create_dir_all(&directory.0).unwrap();
        let legacy = directory.0.join(LEGACY_FILE);
        fs::write(&legacy, b"legacy contents").unwrap();
        assert!(matches!(
            Storage::open_or_create_in(&directory.0, b"master"),
            Err(StorageError::LegacyFormat)
        ));
        assert_eq!(fs::read(legacy).unwrap(), b"legacy contents");
    }

    #[test]
    fn incomplete_storage_is_never_reinitialized_as_empty() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        fs::remove_file(directory.0.join(RECORDS_FILE)).unwrap();
        assert!(matches!(
            Storage::open_or_create_in(&directory.0, b"master"),
            Err(StorageError::IncompleteInitialization)
        ));
        drop(storage);
    }

    #[test]
    fn failed_record_save_keeps_in_memory_state_unchanged() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        fs::remove_dir_all(&directory.0).unwrap();
        assert!(
            storage
                .create_record(record("not saved", Vec::new()))
                .is_err()
        );
        assert_eq!(storage.records.len(), 0);
    }

    #[test]
    fn count_update_failure_reports_that_the_data_file_was_already_saved() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let metadata_path = directory.0.join(METADATA_FILE);
        fs::remove_file(&metadata_path).unwrap();
        fs::create_dir(&metadata_path).unwrap();

        assert!(matches!(
            storage.create_record(record("saved", Vec::new())),
            Err(StorageError::MetadataCountsUpdateAfterDataSave(_))
        ));
        assert_eq!(storage.records.len(), 1);
        let persisted: PersistedRecords = storage
            .vault
            .load_data_file(
                directory.0.join(RECORDS_FILE),
                RECORDS_KIND,
                DATA_SCHEMA_VERSION,
            )
            .unwrap();
        assert_eq!(persisted.records.len(), 1);
    }
}
