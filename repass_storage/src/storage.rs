use crate::StorageError;
use crate::record::serialize::{Vault, VaultError};
use crate::record::types::Timestamp;
use crate::record::{NewRecord, PersistedRecord, RecordId, RecordPatch, RecordView, Records};
use crate::tags::{Tag, TagId, Tags};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const METADATA_FILE: &str = "metadata.repass";
const RECORDS_FILE: &str = "records.repass";
const TAGS_FILE: &str = "tags.repass";
const LEGACY_FILE: &str = "vault.repass";
const RECORDS_KIND: &str = "records";
const TAGS_KIND: &str = "tags";
const DATA_SCHEMA_VERSION: u16 = 1;

#[derive(Deserialize, Serialize)]
struct PersistedRecords {
    records: Vec<PersistedRecord>,
    next_record_id: Option<u64>,
}

#[derive(Deserialize, Serialize)]
struct PersistedTag {
    id: TagId,
    name: String,
}

#[derive(Deserialize, Serialize)]
struct PersistedTags {
    tags: Vec<PersistedTag>,
    next_tag_id: Option<TagId>,
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

/// Directory-level vault API. Record mutations atomically update only
/// `records.repass`; tag catalog mutations atomically update only
/// `tags.repass`.
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
        let vault = Vault::open(&metadata_path, master_password)?;
        let persisted_records: PersistedRecords = vault
            .load_data_file(
                directory.join(RECORDS_FILE),
                RECORDS_KIND,
                DATA_SCHEMA_VERSION,
            )
            .map_err(|error| match error {
                VaultError::Io(error) if error.kind() == io::ErrorKind::NotFound => {
                    StorageError::IncompleteInitialization
                }
                error => StorageError::Vault(error),
            })?;
        let records =
            Records::from_persisted(persisted_records.records, persisted_records.next_record_id)?;
        let used_tag_ids = records.used_tag_ids();

        let (tags, tag_catalog_status) = match vault.load_data_file::<PersistedTags>(
            directory.join(TAGS_FILE),
            TAGS_KIND,
            DATA_SCHEMA_VERSION,
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
                        Tags::from_catalog([], used_tag_ids.iter().copied(), Some(0))?,
                        TagCatalogStatus::Unavailable(error.to_string()),
                    ),
                }
            }
            Err(VaultError::Io(error)) if error.kind() == io::ErrorKind::NotFound => (
                Tags::from_catalog([], used_tag_ids.iter().copied(), Some(0))?,
                TagCatalogStatus::Missing,
            ),
            Err(error) => (
                Tags::from_catalog([], used_tag_ids.iter().copied(), Some(0))?,
                TagCatalogStatus::Unavailable(error.to_string()),
            ),
        };

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
            Err(StorageError::Vault(VaultError::Io(error)))
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
        }
        Ok(changed)
    }

    pub fn delete_record(&mut self, id: RecordId) -> Result<(), StorageError> {
        let mut records = self.records.clone();
        records.remove(id)?;
        save_records(&self.vault, &self.directory, &records)?;
        self.records = records;
        Ok(())
    }

    pub fn create_tag(&mut self, name: impl Into<String>) -> Result<TagId, StorageError> {
        self.ensure_tag_catalog_writable()?;
        let mut tags = self.tags.clone();
        let id = tags.create(name)?;
        self.save_tags(&tags)?;
        self.tags = tags;
        self.tag_catalog_status = TagCatalogStatus::Present;
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
}

fn owned_directory(directory: impl AsRef<Path>) -> Result<PathBuf, StorageError> {
    let directory = directory.as_ref();
    if directory.as_os_str().is_empty() {
        return Err(StorageError::InvalidState("data directory cannot be empty"));
    }
    Ok(directory.to_path_buf())
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
        let technical_id = 12;

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
        assert_eq!(reopened.create_tag("new").unwrap(), 13);
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
        assert!(storage.records.is_empty());
    }
}
