use crate::StorageError;
use crate::persistence::vault::{backup_path, preserve_damaged};
use crate::persistence::{PersistedRecords, PersistedTag, PersistedTags, Vault, VaultCounts};
use crate::record::types::{Host, Timestamp};
use crate::record::{NewRecord, RecordId, RecordPatch, RecordView, Records};
use crate::tags::{Tag, TagId, Tags};
use frizbee::{CaseMatching, Config, Matcher, UnicodeMatching};
use std::borrow::Cow;
use std::fs::{self, File, TryLockError};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::path::{Path, PathBuf};

const METADATA_FILE: &str = "metadata.repass";
const RECORDS_FILE: &str = "records.repass";
const TAGS_FILE: &str = "tags.repass";
const LEGACY_FILE: &str = "vault.repass";
const RECORDS_KIND: &str = "records";
const TAGS_KIND: &str = "tags";
// Record schema 2 deliberately breaks compatibility with password-only records.
const DATA_SCHEMA_VERSION: u16 = 2;
const TAGS_SCHEMA_VERSION: u16 = 1;

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
    /// Opening remains possible when auxiliary count repairs cannot be written.
    pub metadata_warning: Option<String>,
}

/// Directory-level vault API. Record and tag data files are replaced atomically;
/// metadata counts are updated afterward and reconciled from data files on open.
/// The data file and metadata update are not a cross-file transaction.
pub struct Storage {
    _lock: File,
    directory: PathBuf,
    vault: Vault,
    records: Records,
    tags: Tags,
    tag_catalog_status: TagCatalogStatus,
    metadata_warning: Option<String>,
}

impl Storage {
    /// Creates a new three-file vault without overwriting any existing vault
    /// files. The optional tag catalog is created on its first mutation.
    pub fn create_in(
        directory: impl AsRef<Path>,
        master_password: &[u8],
    ) -> Result<Self, StorageError> {
        if master_password.is_empty() {
            return Err(StorageError::InvalidState(
                "master password cannot be empty",
            ));
        }
        let directory = owned_directory(directory)?;
        create_private_directory(&directory)?;
        let lock = lock_directory(&directory)?;
        let metadata_exists = path_exists(&directory.join(METADATA_FILE))?;
        let records_exist = path_exists(&directory.join(RECORDS_FILE))?;
        let tags_exist = path_exists(&directory.join(TAGS_FILE))?;
        if metadata_exists && records_exist {
            return Err(StorageError::AlreadyInitialized);
        }
        if any_backup_exists(&directory)? {
            return Err(StorageError::IncompleteInitialization);
        }
        if metadata_exists || records_exist || tags_exist {
            return Err(StorageError::IncompleteInitialization);
        }
        reject_legacy_file(&directory)?;

        #[cfg(unix)]
        lock.set_permissions(fs::Permissions::from_mode(
            lock.metadata()?.permissions().mode() & 0o700,
        ))?;

        let vault = Vault::create(directory.join(METADATA_FILE), master_password)?;
        let records = Records::new();
        save_records(&vault, &directory, &records)?;
        Ok(Self {
            _lock: lock,
            directory,
            vault,
            records,
            tags: Tags::default(),
            tag_catalog_status: TagCatalogStatus::Missing,
            metadata_warning: None,
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
        let lock = lock_directory(&directory)?;
        Self::open_locked(directory, master_password, lock)
    }

    fn open_locked(
        directory: PathBuf,
        master_password: &[u8],
        lock: File,
    ) -> Result<Self, StorageError> {
        let metadata_path = directory.join(METADATA_FILE);
        if !path_exists(&metadata_path)? {
            reject_legacy_file(&directory)?;
        }
        let mut vault = Vault::open(&metadata_path, master_password)?;
        let saved_counts = vault.counts();
        let loaded_records: PersistedRecords = vault
            .load_data_file(
                directory.join(RECORDS_FILE),
                RECORDS_KIND,
                DATA_SCHEMA_VERSION,
            )
            .map_err(|error| match error {
                crate::VaultError::Io(error) if error.kind() == io::ErrorKind::NotFound => {
                    StorageError::IncompleteInitialization
                }
                error => StorageError::Vault(error),
            })?;
        let records =
            Records::from_persisted(loaded_records.records, loaded_records.next_record_id)?;
        let used_tag_ids = records.used_tag_ids();

        let (tags, tag_catalog_status) = match vault.load_data_file::<PersistedTags>(
            directory.join(TAGS_FILE),
            TAGS_KIND,
            TAGS_SCHEMA_VERSION,
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
                if saved_counts.tags != 0
                    || !used_tag_ids.is_empty()
                    || path_exists(&backup_path(&directory.join(TAGS_FILE)))?
                {
                    TagCatalogStatus::Unavailable("tag catalog is missing; use vault recover to restore its backup or tag recover to rebuild technical names".into())
                } else {
                    TagCatalogStatus::Missing
                },
            ),
            Err(error) => (
                Tags::from_catalog([], used_tag_ids.iter().copied(), Some(0.into()))?,
                TagCatalogStatus::Unavailable(error.to_string()),
            ),
        };

        let tag_count = match &tag_catalog_status {
            TagCatalogStatus::Present => tags.iter().filter(|tag| !tag.is_technical()).count(),
            TagCatalogStatus::Missing => 0,
            TagCatalogStatus::Unavailable(_) => usize::try_from(saved_counts.tags).unwrap_or(0),
        };
        let actual_counts = VaultCounts {
            records: u64::try_from(records.len())
                .map_err(|_| StorageError::InvalidState("record count exceeds metadata range"))?,
            tags: u64::try_from(tag_count)
                .map_err(|_| StorageError::InvalidState("tag count exceeds metadata range"))?,
        };
        let metadata_warning = vault
            .save_counts(actual_counts)
            .err()
            .map(|error| error.to_string());

        Ok(Self {
            _lock: lock,
            directory,
            vault,
            records,
            tags,
            tag_catalog_status,
            metadata_warning,
        })
    }

    /// Opens an existing new-format vault or creates one only when none of its
    /// files exist. It never treats a missing records file as an empty vault.
    pub fn open_or_create_in(
        directory: impl AsRef<Path>,
        master_password: &[u8],
    ) -> Result<Self, StorageError> {
        let directory = owned_directory(directory)?;
        // An existing vault can still be opened with its original password.
        // Empty passwords must never cause even an empty directory to be created.
        if master_password.is_empty() && !path_exists(&directory.join(METADATA_FILE))? {
            return Err(StorageError::InvalidState(
                "master password cannot be empty",
            ));
        }
        create_private_directory(&directory)?;
        if path_exists(&directory.join(METADATA_FILE))? {
            return Self::open_in(directory, master_password);
        }
        if any_backup_exists(&directory)? {
            return Err(StorageError::IncompleteInitialization);
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
            metadata_warning: self.metadata_warning.clone(),
        }
    }

    pub fn list_records(&self, tag: Option<TagId>) -> Result<Vec<RecordView<'_>>, StorageError> {
        self.search_records(None, tag.as_slice())
    }

    /// Exact name matching, intersection of requested tags, ordered by stable ID.
    pub fn search_records(
        &self,
        name: Option<&str>,
        tags: &[TagId],
    ) -> Result<Vec<RecordView<'_>>, StorageError> {
        self.search_records_by_host(name, None, tags)
    }

    /// Exact name and host matching, with the intersection of requested tags.
    pub fn search_records_by_host(
        &self,
        name: Option<&str>,
        host: Option<&Host>,
        tags: &[TagId],
    ) -> Result<Vec<RecordView<'_>>, StorageError> {
        if let Some(host) = host {
            host.validate()?;
        }
        for tag in tags {
            if self.tags.get(*tag).is_none() {
                return Err(StorageError::TagNotFound(*tag));
            }
        }
        Ok(self.records.filtered_views(name, host, tags))
    }

    /// Fuzzy matching against name, username, host, notes and tag names.
    /// Each field is matched separately; the best field score ranks the record.
    /// Results are ordered by descending score, then by stable record ID.
    /// Exact filters apply before matching. Secret data is never searched.
    /// Surrounding query whitespace is trimmed; blank queries are rejected.
    /// Queries of 1–3 Unicode scalars allow no typos, 4–7 allow one, and
    /// longer queries allow two. Case is ignored and Unicode is enabled.
    pub fn fuzzy_search_records(
        &self,
        query: &str,
        name: Option<&str>,
        host: Option<&Host>,
        tags: &[TagId],
    ) -> Result<Vec<RecordView<'_>>, StorageError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(StorageError::InvalidState("search query cannot be blank"));
        }
        let records = self.search_records_by_host(name, host, tags)?;
        let mut fields: Vec<Cow<'_, str>> = Vec::new();
        let mut owners = Vec::new();
        for (index, record) in records.iter().enumerate() {
            let start = fields.len();
            fields.push(Cow::Borrowed(record.name));
            for field in record.username.into_iter().chain(record.notes) {
                fields.push(Cow::Borrowed(field));
            }
            if let Some(host) = record.host {
                fields.push(match host {
                    Host::Domain(domain) => Cow::Borrowed(domain.trim_end_matches('.')),
                    Host::IP(ip) => Cow::Owned(ip.to_string()),
                });
            }
            for tag in record.tags {
                if let Some(tag) = self.tags.get(*tag) {
                    fields.push(Cow::Borrowed(tag.name()));
                }
            }
            owners.extend(std::iter::repeat_n(index, fields.len() - start));
        }
        // Frizbee uses u32 candidate indices internally.
        if fields.len() > u32::MAX as usize {
            return Err(StorageError::InvalidState("too many searchable fields"));
        }
        let config = Config::default()
            .casing(CaseMatching::Ignore)
            .unicode(UnicodeMatching::Always)
            .max_typos(Some((query.chars().count() / 4).min(2) as u16));
        let mut matcher = Matcher::new(query, &config);
        let mut scores: Vec<Option<u16>> = vec![None; records.len()];
        for matched in matcher.match_list(&fields) {
            let score = &mut scores[owners[matched.index as usize]];
            *score = Some(score.map_or(matched.score, |current| current.max(matched.score)));
        }
        let mut ranked: Vec<_> = records
            .into_iter()
            .zip(scores)
            .filter_map(|(record, score)| score.map(|score| (record, score)))
            .collect();
        ranked.sort_unstable_by(|(a, a_score), (b, b_score)| {
            b_score.cmp(a_score).then_with(|| a.id.cmp(&b.id))
        });
        Ok(ranked.into_iter().map(|(record, _)| record).collect())
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
        let next = self.records.next_id();
        let id = self.records.create(record, Timestamp::now()?)?;
        if let Err(error) = save_records(&self.vault, &self.directory, &self.records) {
            if !write_committed(&error) {
                self.records.rollback_create(id, next)?;
            }
            return Err(error);
        }
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
        let original = self.records.snapshot(id)?;
        let changed = self.records.update(id, &patch, Timestamp::now()?)?;
        if changed {
            if let Err(error) = save_records(&self.vault, &self.directory, &self.records) {
                if !write_committed(&error) {
                    self.records.restore(original);
                }
                return Err(error);
            }
            self.save_counts_after_data_write()?;
        }
        Ok(changed)
    }

    pub fn delete_record(&mut self, id: RecordId) -> Result<(), StorageError> {
        let original = self.records.take(id)?;
        if let Err(error) = save_records(&self.vault, &self.directory, &self.records) {
            if !write_committed(&error) {
                self.records.restore(original);
            }
            return Err(error);
        }
        self.save_counts_after_data_write()?;
        Ok(())
    }

    pub fn create_tag(&mut self, name: impl Into<String>) -> Result<TagId, StorageError> {
        self.ensure_tag_catalog_writable()?;
        let mut tags = self.tags.clone();
        let id = tags.create(name)?;
        self.commit_tags(tags)?;
        self.save_counts_after_data_write()?;
        Ok(id)
    }

    pub fn list_tags(&self) -> Vec<Tag> {
        self.tags.iter().cloned().collect()
    }

    pub fn get_tag(&self, id: TagId) -> Option<&Tag> {
        self.tags.get(id)
    }

    pub fn change_master_password(&mut self, password: &[u8]) -> Result<(), StorageError> {
        if password.is_empty() {
            return Err(StorageError::InvalidState(
                "new master password cannot be empty",
            ));
        }
        self.vault.change_password(password)?;
        Ok(())
    }

    /// Restores only authenticated, domain-validated backups. Damaged originals
    /// are retained beside the files under unique `.damaged-*` names.
    pub fn recover_in(directory: impl AsRef<Path>, password: &[u8]) -> Result<Self, StorageError> {
        let directory = owned_directory(directory)?;
        let lock = lock_directory(&directory)?;
        let metadata = directory.join(METADATA_FILE);
        let (vault, restore_metadata) = match Vault::open(&metadata, password) {
            Ok(vault) => (vault, false),
            Err(primary_error) => {
                let backup = backup_path(&metadata);
                let vault = Vault::open(&backup, password)
                    .map_err(|_| StorageError::Vault(primary_error))?;
                (vault, true)
            }
        };
        let records = directory.join(RECORDS_FILE);
        let (loaded, restore_records) = match validate_records_file(&vault, &records) {
            Ok(records) => (records, false),
            Err(_) => (validate_records_file(&vault, &backup_path(&records))?, true),
        };
        let tags = directory.join(TAGS_FILE);
        let used = loaded.used_tag_ids();
        let restore_tags =
            validate_tags_file(&vault, &tags, &used).is_err() && backup_path(&tags).try_exists()?;
        if restore_tags {
            validate_tags_file(&vault, &backup_path(&tags), &used)?;
        }
        // Validate the entire recovery plan before replacing any primary file.
        if restore_records {
            vault.restore_data_file(&records, RECORDS_KIND, DATA_SCHEMA_VERSION)?;
        }
        if restore_tags {
            vault.restore_data_file(&tags, TAGS_KIND, TAGS_SCHEMA_VERSION)?;
        }
        if restore_metadata {
            preserve_damaged(&metadata)?;
            let bytes = crate::persistence::vault::read_metadata(&backup_path(&metadata))?;
            crate::persistence::vault::write_recovered(&metadata, &bytes)?;
        }
        Self::open_locked(directory, password, lock)
    }

    /// Explicitly completes initialization only when no data or data backup
    /// exists and authenticated metadata says that the vault is empty.
    pub fn finish_initialization_in(
        directory: impl AsRef<Path>,
        password: &[u8],
    ) -> Result<Self, StorageError> {
        let directory = owned_directory(directory)?;
        let lock = lock_directory(&directory)?;
        let vault = Vault::open(directory.join(METADATA_FILE), password)?;
        for name in [RECORDS_FILE, TAGS_FILE] {
            let path = directory.join(name);
            if path_exists(&path)? || path_exists(&backup_path(&path))? {
                return Err(StorageError::InvalidState(
                    "data files exist; use vault recover",
                ));
            }
        }
        if !vault.counts_valid()
            || vault.counts()
                != (VaultCounts {
                    records: 0,
                    tags: 0,
                })
        {
            return Err(StorageError::InvalidState(
                "cannot establish that initialization was empty",
            ));
        }
        save_records(&vault, &directory, &Records::new())?;
        Self::open_locked(directory, password, lock)
    }

    pub fn rename_tag(
        &mut self,
        id: TagId,
        new_name: impl Into<String>,
    ) -> Result<(), StorageError> {
        self.ensure_tag_catalog_writable()?;
        let mut tags = self.tags.clone();
        tags.rename(id, new_name)?;
        self.commit_tags(tags)?;
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
        self.commit_tags(tags)?;
        self.save_counts_after_data_write()?;
        Ok(())
    }

    /// Explicitly rebuilds an unavailable or missing tag catalog from current
    /// tags. Technical names are promoted as `#tag-<id>` entries.
    pub fn recover_tags(&mut self) -> Result<usize, StorageError> {
        let mut tags = self.tags.clone();
        tags.promote_technical();
        let recovered = tags.len();
        self.commit_tags(tags)?;
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
            TAGS_SCHEMA_VERSION,
            &persisted,
        )?;
        Ok(())
    }

    fn commit_tags(&mut self, tags: Tags) -> Result<(), StorageError> {
        let result = self.save_tags(&tags);
        if result.is_ok() || result.as_ref().is_err_and(|error| write_committed(error)) {
            self.tags = tags;
            self.tag_catalog_status = TagCatalogStatus::Present;
        }
        result
    }

    fn save_counts_after_data_write(&mut self) -> Result<(), StorageError> {
        let result = self.vault.save_counts(self.current_counts()?);
        self.metadata_warning = result.as_ref().err().map(ToString::to_string);
        result.map_err(StorageError::MetadataCountsUpdateAfterDataSave)
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

fn create_private_directory(directory: &Path) -> Result<(), StorageError> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    builder.mode(0o700);
    builder.create(directory)?;
    Ok(())
}

fn records_path(directory: &Path) -> PathBuf {
    directory.join(RECORDS_FILE)
}

fn any_backup_exists(directory: &Path) -> Result<bool, StorageError> {
    for name in [METADATA_FILE, RECORDS_FILE, TAGS_FILE] {
        if path_exists(&backup_path(&directory.join(name)))? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn save_records(vault: &Vault, directory: &Path, records: &Records) -> Result<(), StorageError> {
    let persisted = records.persisted_view();
    vault.save_data_file(
        records_path(directory),
        RECORDS_KIND,
        DATA_SCHEMA_VERSION,
        &persisted,
    )?;
    Ok(())
}

fn lock_directory(directory: &Path) -> Result<File, StorageError> {
    let lock = File::open(directory)?;
    match lock.try_lock() {
        Ok(()) => Ok(lock),
        Err(TryLockError::WouldBlock) => Err(StorageError::Locked),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

fn write_committed(error: &StorageError) -> bool {
    matches!(
        error,
        StorageError::Vault(crate::VaultError::WriteCommitted(_))
    )
}

fn validate_records_file(vault: &Vault, path: &Path) -> Result<Records, StorageError> {
    let persisted: PersistedRecords =
        vault.load_data_file(path, RECORDS_KIND, DATA_SCHEMA_VERSION)?;
    Records::from_persisted(persisted.records, persisted.next_record_id)
}

fn validate_tags_file(vault: &Vault, path: &Path, used: &[TagId]) -> Result<(), StorageError> {
    let persisted: PersistedTags = vault.load_data_file(path, TAGS_KIND, TAGS_SCHEMA_VERSION)?;
    Tags::from_catalog(
        persisted.tags.into_iter().map(|tag| (tag.id, tag.name)),
        used.iter().copied(),
        persisted.next_tag_id,
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
    use crate::persistence::PersistedRecord;
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
            data: vec![crate::Data::Password(format!("{name}-secret"))],
            username: Some("user".to_owned()),
            host: Some("example.test".parse().unwrap()),
            notes: None,
            tags,
        }
    }

    #[test]
    fn empty_master_password_cannot_create_a_directory_or_vault() {
        let directory = TestDirectory::new();
        assert!(Storage::create_in(&directory.0, b"").is_err());
        assert!(!directory.0.exists());
        assert!(Storage::open_or_create_in(&directory.0, b"").is_err());
        assert!(!directory.0.exists());
    }

    #[cfg(unix)]
    #[test]
    fn vault_directories_and_all_published_files_have_private_permissions() {
        let directory = TestDirectory::new();
        fs::create_dir(&directory.0).unwrap();
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
        let vault_dir = directory.0.join("new-parent/vault");
        let mut storage = Storage::create_in(&vault_dir, b"master").unwrap();
        assert_eq!(
            fs::metadata(&directory.0).unwrap().permissions().mode() & 0o777,
            0o755
        );
        for path in [directory.0.join("new-parent"), vault_dir.clone()] {
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        let tag = storage.create_tag("work").unwrap();
        storage.create_record(record("mail", vec![tag])).unwrap();
        storage.rename_tag(tag, "renamed").unwrap();
        storage.change_master_password(b"new").unwrap();
        drop(storage);
        fs::write(vault_dir.join(RECORDS_FILE), b"damaged").unwrap();
        let storage = Storage::recover_in(&vault_dir, b"new").unwrap();
        assert_eq!(storage.info().record_count, 0);
        for entry in fs::read_dir(&vault_dir).unwrap() {
            let entry = entry.unwrap();
            assert_eq!(
                entry.metadata().unwrap().permissions().mode() & 0o777,
                0o600,
                "{}",
                entry.path().display()
            );
        }
        assert!(fs::read_dir(&vault_dir).unwrap().any(|entry| {
            entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .contains(".damaged-")
        }));
    }

    #[cfg(unix)]
    #[test]
    fn initializing_in_an_existing_directory_restricts_only_that_directory() {
        let directory = TestDirectory::new();
        fs::create_dir(&directory.0).unwrap();
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o755)).unwrap();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        assert_eq!(
            fs::metadata(&directory.0).unwrap().permissions().mode() & 0o777,
            0o700
        );
        drop(storage);
        fs::set_permissions(&directory.0, fs::Permissions::from_mode(0o750)).unwrap();
        assert!(Storage::create_in(&directory.0, b"replacement").is_err());
        assert_eq!(
            fs::metadata(&directory.0).unwrap().permissions().mode() & 0o777,
            0o750
        );
    }

    #[test]
    fn fuzzy_search_matches_each_metadata_field_and_supports_unicode_and_typos() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("archive").unwrap();
        let cases = [
            (
                "githab",
                NewRecord {
                    name: "GitHub".into(),
                    data: vec![],
                    username: None,
                    host: None,
                    notes: None,
                    tags: vec![],
                },
            ),
            (
                "ASTRONAUT",
                NewRecord {
                    name: "one".into(),
                    data: vec![],
                    username: Some("astronaut".into()),
                    host: None,
                    notes: None,
                    tags: vec![],
                },
            ),
            (
                "nebula",
                NewRecord {
                    name: "two".into(),
                    data: vec![],
                    username: None,
                    host: Some("Nebula.TEST.".parse().unwrap()),
                    notes: None,
                    tags: vec![],
                },
            ),
            (
                "ПОЧТА",
                NewRecord {
                    name: "three".into(),
                    data: vec![],
                    username: None,
                    host: None,
                    notes: Some("Почта".into()),
                    tags: vec![],
                },
            ),
            (
                "archive",
                NewRecord {
                    name: "four".into(),
                    data: vec![],
                    username: None,
                    host: None,
                    notes: None,
                    tags: vec![tag],
                },
            ),
            (
                "192.0.2.42",
                NewRecord {
                    name: "five".into(),
                    data: vec![],
                    username: None,
                    host: Some("192.0.2.42".parse().unwrap()),
                    notes: None,
                    tags: vec![],
                },
            ),
            (
                "RÉSUMÉ",
                NewRecord {
                    name: "résumé".into(),
                    data: vec![],
                    username: None,
                    host: None,
                    notes: None,
                    tags: vec![],
                },
            ),
            (
                "猫猫",
                NewRecord {
                    name: "猫猫".into(),
                    data: vec![],
                    username: None,
                    host: None,
                    notes: None,
                    tags: vec![],
                },
            ),
        ];
        let mut expected = Vec::new();
        for (query, record) in cases {
            expected.push((query, storage.create_record(record).unwrap()));
        }
        for (query, id) in expected {
            let matches = storage
                .fuzzy_search_records(query, None, None, &[])
                .unwrap();
            assert!(matches.iter().any(|record| record.id == id), "{query}");
            assert_eq!(matches.iter().filter(|record| record.id == id).count(), 1);
        }
        assert!(
            storage
                .fuzzy_search_records("z", None, None, &[])
                .unwrap()
                .is_empty()
        );
        assert!(
            storage
                .fuzzy_search_records("猫犬", None, None, &[])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn fuzzy_search_ranks_best_field_once_and_combines_exact_filters() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let a = storage.create_tag("work").unwrap();
        let b = storage.create_tag("personal").unwrap();
        let weak = storage.create_record(record("a_l_p_h_a", vec![a])).unwrap();
        let strong = storage.create_record(record("alpha", vec![a, b])).unwrap();
        let tie = storage.create_record(record("alpha", vec![a])).unwrap();
        let mut multi = record("alpha", vec![a]);
        multi.username = Some("alpha".into());
        multi.notes = Some("alpha".into());
        let multi = storage.create_record(multi).unwrap();
        let ranked = storage
            .fuzzy_search_records(" alpha ", None, None, &[])
            .unwrap();
        assert_eq!(
            ranked.iter().map(|record| record.id).collect::<Vec<_>>(),
            vec![strong, tie, multi, weak]
        );
        let matches = storage
            .fuzzy_search_records(
                "alpha",
                Some("alpha"),
                Some(&"EXAMPLE.TEST.".parse().unwrap()),
                &[a, b],
            )
            .unwrap();
        assert_eq!(
            matches.iter().map(|record| record.id).collect::<Vec<_>>(),
            vec![strong]
        );
        assert!(
            storage
                .fuzzy_search_records("alpha", Some("Alpha"), None, &[])
                .unwrap()
                .is_empty()
        );
        assert!(
            storage
                .fuzzy_search_records("alpha", None, Some(&"other.test".parse().unwrap()), &[])
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            storage.fuzzy_search_records("alpha", None, None, &[TagId::new(999)]),
            Err(StorageError::TagNotFound(_))
        ));
        storage.delete_record(strong).unwrap();
        assert_eq!(
            storage
                .fuzzy_search_records("alpha", None, None, &[])
                .unwrap()
                .iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
            vec![tie, multi, weak]
        );
        storage.rename_tag(a, "renamed-catalog").unwrap();
        assert_eq!(
            storage
                .fuzzy_search_records("renamed-catalog", None, None, &[])
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn fuzzy_search_does_not_match_secret_data_or_join_separate_fields() {
        use crate::{Data, SshKey, Totp, TotpAlgorithm};
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        for query in ["", " \t\n"] {
            assert!(
                storage
                    .fuzzy_search_records(query, None, None, &[])
                    .is_err()
            );
        }
        assert!(
            storage
                .fuzzy_search_records("anything", None, None, &[])
                .unwrap()
                .is_empty()
        );
        storage
            .create_record(NewRecord {
                name: "abc".into(),
                username: Some("def".into()),
                host: None,
                notes: None,
                tags: vec![],
                data: vec![
                    Data::Password("passwordsecret".into()),
                    Data::SshKey(
                        SshKey::new(
                            Some("privatesecret\n".into()),
                            Some("publicsecret\n".into()),
                        )
                        .unwrap(),
                    ),
                    Data::Totp(Totp::new("MZXW6YTB".into(), TotpAlgorithm::Sha1, 6, 30).unwrap()),
                    Data::Code("recoverysecret".into()),
                ],
            })
            .unwrap();
        for query in [
            "passwordsecret",
            "privatesecret",
            "publicsecret",
            "MZXW6YTB",
            "recoverysecret",
            "abcdef",
        ] {
            assert!(
                storage
                    .fuzzy_search_records(query, None, None, &[])
                    .unwrap()
                    .is_empty(),
                "{query}"
            );
        }
    }

    #[test]
    fn mixed_secret_data_survives_save_reopen_and_recovery_with_stable_ids() {
        use crate::{Data, DataId, SshKey, Totp, TotpAlgorithm};
        let directory = TestDirectory::new();
        let data = vec![
            Data::Password("猫-password".into()),
            Data::SshKey(
                SshKey::new(
                    Some("private\r\nkey  \r\n".into()),
                    Some("ssh-ed25519 public\n".into()),
                )
                .unwrap(),
            ),
            Data::SshKey(SshKey::new(None, Some("public only\n".into())).unwrap()),
            Data::SshKey(SshKey::new(Some("private only\n".into()), None).unwrap()),
            Data::Totp(Totp::new("MZXW6YTB".into(), TotpAlgorithm::Sha512, 8, 60).unwrap()),
            Data::Code("code-a".into()),
            Data::Code("code-b".into()),
        ];
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let id = storage
            .create_record(NewRecord {
                name: "mixed".into(),
                data: data.clone(),
                username: None,
                host: Some("Example.TEST.".parse().unwrap()),
                notes: None,
                tags: vec![],
            })
            .unwrap();
        storage
            .update_record(
                id,
                RecordPatch {
                    remove_data: vec![DataId::new(6)],
                    ..RecordPatch::default()
                },
            )
            .unwrap();
        storage
            .update_record(
                id,
                RecordPatch {
                    add_data: vec![Data::Code("code-c".into())],
                    ..RecordPatch::default()
                },
            )
            .unwrap();
        let expected = storage.get_record(id).unwrap().data().to_vec();
        assert_eq!(expected.last().unwrap().id, DataId::new(8));
        let created = *storage.get_record(id).unwrap().created;
        drop(storage);
        let mut storage = Storage::open_in(&directory.0, b"master").unwrap();
        assert!(storage.get_record(id).unwrap().data() == expected);
        assert_eq!(*storage.get_record(id).unwrap().created, created);
        assert_eq!(
            storage
                .search_records_by_host(None, Some(&"example.test".parse().unwrap()), &[])
                .unwrap()
                .len(),
            1
        );
        assert!(
            storage
                .search_records_by_host(None, Some(&"127.0.0.1".parse().unwrap()), &[])
                .unwrap()
                .is_empty()
        );
        // The backup now contains the reopened mixed record.
        storage
            .update_record(
                id,
                RecordPatch {
                    notes: crate::FieldUpdate::Set("latest".into()),
                    ..RecordPatch::default()
                },
            )
            .unwrap();
        drop(storage);
        fs::write(directory.0.join(RECORDS_FILE), b"damaged").unwrap();
        let mut storage = Storage::recover_in(&directory.0, b"master").unwrap();
        assert!(storage.get_record(id).unwrap().data() == expected);
        assert_eq!(storage.get_record(id).unwrap().notes, None);
        storage
            .update_record(
                id,
                RecordPatch {
                    add_data: vec![Data::Code("after recovery".into())],
                    ..RecordPatch::default()
                },
            )
            .unwrap();
        assert_eq!(
            storage.get_record(id).unwrap().data().last().unwrap().id,
            DataId::new(9)
        );
        let ssh_only = storage
            .create_record(NewRecord {
                name: "ssh only".into(),
                data: vec![data[1].clone()],
                username: None,
                host: Some("2001:db8::1".parse().unwrap()),
                notes: None,
                tags: vec![],
            })
            .unwrap();
        assert_eq!(
            storage
                .search_records_by_host(None, Some(&"2001:0db8::1".parse().unwrap()), &[])
                .unwrap()[0]
                .id,
            ssh_only
        );
    }

    #[test]
    fn failed_data_write_rolls_back_values_and_id_sequence() {
        use crate::{Data, DataId};
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let id = storage.create_record(record("original", vec![])).unwrap();
        let before = storage.get_record(id).unwrap().data().to_vec();
        let backup = backup_path(&directory.0.join(RECORDS_FILE));
        fs::remove_file(&backup).unwrap();
        fs::create_dir(&backup).unwrap();
        assert!(
            storage
                .update_record(
                    id,
                    RecordPatch {
                        name: Some("changed".into()),
                        add_data: vec![Data::Code("new".into())],
                        replace_data: vec![(DataId::new(1), Data::Password("replacement".into()))],
                        ..RecordPatch::default()
                    }
                )
                .is_err()
        );
        assert!(storage.get_record(id).unwrap().data() == before);
        assert_eq!(storage.get_record(id).unwrap().name, "original");
        fs::remove_dir(&backup).unwrap();
        storage
            .update_record(
                id,
                RecordPatch {
                    add_data: vec![Data::Code("saved".into())],
                    ..RecordPatch::default()
                },
            )
            .unwrap();
        assert_eq!(storage.get_record(id).unwrap().data()[1].id, DataId::new(2));
    }

    #[test]
    fn unsupported_record_schema_is_rejected_without_rewriting_it() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        let path = directory.0.join(RECORDS_FILE);
        storage
            .vault
            .save_data_file(&path, RECORDS_KIND, 1, &storage.records.persisted_view())
            .unwrap();
        let bytes = fs::read(&path).unwrap();
        drop(storage);
        assert!(matches!(
            Storage::open_in(&directory.0, b"master"),
            Err(StorageError::Vault(
                crate::VaultError::DataVersionMismatch {
                    expected: 2,
                    actual: 1
                }
            ))
        ));
        assert_eq!(fs::read(path).unwrap(), bytes);
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

        drop(storage);
        let reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(reopened.list_tags()[0].name(), "work-renamed");
        let records = reopened.list_records(Some(tag)).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].id, first);
        assert_eq!(records[0].notes, Some("updated"));
        assert!(
            matches!(&records[0].data()[0].value, crate::Data::Password(value) if value == "mail-secret")
        );
        assert_eq!(reopened.find_records_by_name("mail"), vec![first]);
        assert_eq!(reopened.find_records_with_tags(&[tag]), vec![first]);
        assert_eq!(reopened.info().record_count, 1);
    }

    #[test]
    fn missing_tag_catalog_keeps_backup_and_blocks_ordinary_writes() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        storage.create_record(record("mail", vec![tag])).unwrap();
        let backup = backup_path(&directory.0.join(TAGS_FILE));
        let before = fs::read(&backup).unwrap();
        fs::remove_file(directory.0.join(TAGS_FILE)).unwrap();
        drop(storage);
        let mut storage = Storage::open_in(&directory.0, b"master").unwrap();
        assert!(storage.create_tag("new").is_err());
        assert!(storage.rename_tag(tag, "new").is_err());
        assert_eq!(fs::read(&backup).unwrap(), before);
        drop(storage);
        let restored = Storage::recover_in(&directory.0, b"master").unwrap();
        assert_eq!(restored.get_tag(tag).unwrap().name(), "work");
        drop(restored);
        fs::remove_file(directory.0.join(TAGS_FILE)).unwrap();
        let mut storage = Storage::open_in(&directory.0, b"master").unwrap();
        storage.recover_tags().unwrap();
        assert_eq!(fs::read(&backup).unwrap(), before);
        assert!(!storage.get_tag(tag).unwrap().is_technical());
    }

    #[test]
    fn missing_tag_file_creates_technical_tags_and_rename_persists_one() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        storage.create_record(record("mail", vec![tag])).unwrap();
        fs::remove_file(directory.0.join(TAGS_FILE)).unwrap();
        drop(storage);
        let mut reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert!(matches!(
            reopened.tag_catalog_status(),
            TagCatalogStatus::Unavailable(_)
        ));
        let technical = reopened.list_tags();
        assert_eq!(technical.len(), 1);
        assert_eq!(technical[0].id(), tag);
        assert_eq!(technical[0].name(), format!("#tag-{tag}"));
        assert!(technical[0].is_technical());

        assert!(reopened.rename_tag(tag, "renamed").is_err());
        reopened.recover_tags().unwrap();
        reopened.rename_tag(tag, "renamed").unwrap();
        drop(reopened);
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
        drop(storage);
        let mut reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert!(matches!(
            reopened.tag_catalog_status(),
            TagCatalogStatus::Unavailable(_)
        ));
        assert_eq!(reopened.get_record(record_id).unwrap().name, "mail");
        assert!(reopened.rename_tag(tag, "renamed").is_err());
        assert_eq!(reopened.recover_tags().unwrap(), 1);
        drop(reopened);
        let reopened = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(reopened.list_tags()[0].name(), format!("#tag-{tag}"));
        assert!(!reopened.list_tags()[0].is_technical());
    }

    #[test]
    fn missing_tag_ids_do_not_get_reused_for_new_tags() {
        let directory = TestDirectory::new();
        let storage = Storage::create_in(&directory.0, b"master").unwrap();
        let technical_id = TagId::new(12);

        let persisted: PersistedRecords = PersistedRecords {
            records: vec![PersistedRecord {
                id: RecordId::new(1),
                name: "legacy reference".into(),
                data: vec![crate::RecordData {
                    id: crate::DataId::new(1),
                    value: crate::Data::Password("secret".into()),
                }],
                next_data_id: Some(2),
                username: None,
                host: None,
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
        drop(storage);
        let mut reopened = Storage::open_in(&directory.0, b"master").unwrap();
        reopened.recover_tags().unwrap();
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
        drop(storage);
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
        drop(storage);
        assert!(matches!(
            Storage::open_or_create_in(&directory.0, b"master"),
            Err(StorageError::IncompleteInitialization)
        ));
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

    #[test]
    fn exclusive_session_lock_prevents_lost_updates_and_releases_on_drop() {
        let directory = TestDirectory::new();
        let mut first = Storage::create_in(&directory.0, b"master").unwrap();
        assert!(matches!(
            Storage::open_in(&directory.0, b"master"),
            Err(StorageError::Locked)
        ));
        assert!(matches!(
            Storage::open_or_create_in(&directory.0, b"master"),
            Err(StorageError::Locked)
        ));
        first.create_record(record("first", vec![])).unwrap();
        drop(first);
        let mut second = Storage::open_in(&directory.0, b"master").unwrap();
        second.create_record(record("second", vec![])).unwrap();
        assert_eq!(second.list_records(None).unwrap().len(), 2);
    }

    #[test]
    fn corrupted_or_extreme_auxiliary_counts_do_not_block_records() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let id = storage.create_record(record("kept", vec![])).unwrap();
        storage
            .vault
            .save_counts(VaultCounts {
                records: u64::MAX,
                tags: u64::MAX,
            })
            .unwrap();
        drop(storage);
        let storage = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(storage.get_record(id).unwrap().name, "kept");
        drop(storage);
        let metadata = directory.0.join(METADATA_FILE);
        let mut bytes = fs::read(&metadata).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(&metadata, bytes).unwrap();
        let storage = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(storage.get_record(id).unwrap().name, "kept");
        assert!(storage.vault.counts_valid());
    }

    #[test]
    fn updates_without_count_changes_leave_metadata_bytes_unchanged() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        let id = storage.create_record(record("mail", vec![tag])).unwrap();
        let before = fs::read(directory.0.join(METADATA_FILE)).unwrap();
        storage
            .update_record(
                id,
                RecordPatch {
                    name: Some("renamed".into()),
                    ..RecordPatch::default()
                },
            )
            .unwrap();
        storage.rename_tag(tag, "renamed-tag").unwrap();
        assert_eq!(fs::read(directory.0.join(METADATA_FILE)).unwrap(), before);
    }

    #[test]
    fn backup_failure_rolls_back_create_update_and_delete() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let id = storage.create_record(record("original", vec![])).unwrap();
        let path = directory.0.join(RECORDS_FILE);
        let before = fs::read(&path).unwrap();
        let backup = backup_path(&path);
        fs::remove_file(&backup).unwrap();
        fs::create_dir(&backup).unwrap();
        assert!(storage.create_record(record("failed", vec![])).is_err());
        assert!(
            storage
                .update_record(
                    id,
                    RecordPatch {
                        name: Some("failed".into()),
                        ..RecordPatch::default()
                    }
                )
                .is_err()
        );
        assert!(storage.delete_record(id).is_err());
        assert_eq!(storage.get_record(id).unwrap().name, "original");
        assert_eq!(storage.info().record_count, 1);
        assert_eq!(fs::read(path).unwrap(), before);
        fs::remove_dir(backup).unwrap();
        assert_eq!(
            storage.create_record(record("next", vec![])).unwrap(),
            RecordId::new(2)
        );
    }

    #[test]
    fn master_password_change_rewraps_primary_and_backup_without_rewriting_data() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"old").unwrap();
        storage.create_record(record("mail", vec![])).unwrap();
        let before = fs::read(directory.0.join(RECORDS_FILE)).unwrap();
        storage.change_master_password(b"new").unwrap();
        assert_eq!(fs::read(directory.0.join(RECORDS_FILE)).unwrap(), before);
        let metadata_backup = backup_path(&directory.0.join(METADATA_FILE));
        assert!(Vault::open(&metadata_backup, b"old").is_err());
        assert!(Vault::open(&metadata_backup, b"new").is_ok());
        storage
            .create_record(record("after-change", vec![]))
            .unwrap();
        drop(storage);
        assert!(Storage::open_in(&directory.0, b"old").is_err());
        let reopened = Storage::open_in(&directory.0, b"new").unwrap();
        assert_eq!(reopened.info().record_count, 2);
    }

    #[test]
    fn recovery_restores_validated_backups_and_preserves_damaged_originals() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let tag = storage.create_tag("work").unwrap();
        storage.rename_tag(tag, "renamed").unwrap();
        let id = storage.create_record(record("kept", vec![tag])).unwrap();
        storage.create_record(record("latest", vec![])).unwrap();
        drop(storage);
        for name in [METADATA_FILE, RECORDS_FILE, TAGS_FILE] {
            fs::write(directory.0.join(name), b"damaged").unwrap();
        }
        let storage = Storage::recover_in(&directory.0, b"master").unwrap();
        assert_eq!(storage.get_record(id).unwrap().name, "kept");
        assert_eq!(storage.info().record_count, 1);
        assert_eq!(storage.get_tag(tag).unwrap().name(), "work");
        let damaged: Vec<_> = fs::read_dir(&directory.0)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .contains(".damaged-")
                    .then_some(path)
            })
            .collect();
        assert_eq!(damaged.len(), 3);
        for path in damaged {
            assert_eq!(fs::read(path).unwrap(), b"damaged");
        }
    }

    #[test]
    fn invalid_recovery_plan_never_replaces_primary_files() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        storage.create_tag("work").unwrap();
        storage.create_record(record("kept", vec![])).unwrap();
        storage.create_record(record("latest", vec![])).unwrap();
        drop(storage);
        fs::write(directory.0.join(RECORDS_FILE), b"damaged-records").unwrap();
        fs::write(directory.0.join(TAGS_FILE), b"damaged-tags").unwrap();
        fs::write(backup_path(&directory.0.join(TAGS_FILE)), b"bad-backup").unwrap();
        assert!(Storage::recover_in(&directory.0, b"master").is_err());
        assert_eq!(
            fs::read(directory.0.join(RECORDS_FILE)).unwrap(),
            b"damaged-records"
        );
        assert_eq!(
            fs::read(directory.0.join(TAGS_FILE)).unwrap(),
            b"damaged-tags"
        );
    }

    #[test]
    fn surviving_backups_are_never_overwritten_by_automatic_initialization() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        storage.create_record(record("kept", vec![])).unwrap();
        storage.create_record(record("latest", vec![])).unwrap();
        drop(storage);
        let backup = backup_path(&directory.0.join(METADATA_FILE));
        let before = fs::read(&backup).unwrap();
        for name in [METADATA_FILE, RECORDS_FILE] {
            fs::remove_file(directory.0.join(name)).unwrap();
        }
        assert!(matches!(
            Storage::open_or_create_in(&directory.0, b"replacement"),
            Err(StorageError::IncompleteInitialization)
        ));
        assert!(matches!(
            Storage::create_in(&directory.0, b"replacement"),
            Err(StorageError::IncompleteInitialization)
        ));
        assert_eq!(fs::read(backup).unwrap(), before);
        let recovered = Storage::recover_in(&directory.0, b"master").unwrap();
        assert_eq!(recovered.list_records(None).unwrap()[0].name, "kept");
    }

    #[test]
    fn count_repair_write_failure_does_not_prevent_opening_and_later_repairs() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        storage.create_record(record("kept", vec![])).unwrap();
        storage
            .vault
            .save_counts(VaultCounts {
                records: 90,
                tags: 0,
            })
            .unwrap();
        drop(storage);
        crate::persistence::vault::fail_next_write(directory.0.join(METADATA_FILE), false);
        let mut storage = Storage::open_in(&directory.0, b"master").unwrap();
        assert_eq!(storage.info().record_count, 1);
        assert!(storage.info().metadata_warning.is_some());
        storage.create_record(record("next", vec![])).unwrap();
        assert!(storage.info().metadata_warning.is_none());
        assert_eq!(storage.vault.counts().records, 2);
    }

    #[test]
    fn post_publication_failure_keeps_memory_aligned_with_saved_records() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        crate::persistence::vault::fail_next_write(directory.0.join(RECORDS_FILE), true);
        assert!(matches!(
            storage.create_record(record("saved", vec![])),
            Err(StorageError::Vault(crate::VaultError::WriteCommitted(_)))
        ));
        assert_eq!(storage.get_record(RecordId::new(1)).unwrap().name, "saved");
        storage.create_record(record("next", vec![])).unwrap();
        drop(storage);
        assert_eq!(
            Storage::open_in(&directory.0, b"master")
                .unwrap()
                .info()
                .record_count,
            2
        );
    }

    #[test]
    fn failed_primary_password_change_can_be_completed_from_new_backup() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"old").unwrap();
        storage.create_record(record("kept", vec![])).unwrap();
        crate::persistence::vault::fail_next_write(directory.0.join(METADATA_FILE), false);
        assert!(storage.change_master_password(b"new").is_err());
        assert!(Vault::open(directory.0.join(METADATA_FILE), b"old").is_ok());
        drop(storage);
        let storage = Storage::recover_in(&directory.0, b"new").unwrap();
        assert_eq!(storage.get_record(RecordId::new(1)).unwrap().name, "kept");
        drop(storage);
        assert!(Storage::open_in(&directory.0, b"new").is_ok());
    }

    #[test]
    fn finish_initialization_requires_authenticated_empty_metadata_and_no_data() {
        let directory = TestDirectory::new();
        fs::create_dir_all(&directory.0).unwrap();
        Vault::create(directory.0.join(METADATA_FILE), b"master").unwrap();
        assert!(Storage::finish_initialization_in(&directory.0, b"wrong").is_err());
        let storage = Storage::finish_initialization_in(&directory.0, b"master").unwrap();
        assert_eq!(storage.info().record_count, 0);
        drop(storage);
        assert!(Storage::finish_initialization_in(&directory.0, b"master").is_err());
    }

    #[test]
    fn record_search_intersects_name_and_multiple_tags() {
        let directory = TestDirectory::new();
        let mut storage = Storage::create_in(&directory.0, b"master").unwrap();
        let a = storage.create_tag("a").unwrap();
        let b = storage.create_tag("b").unwrap();
        let wanted = storage.create_record(record("mail", vec![a, b])).unwrap();
        storage.create_record(record("mail", vec![a])).unwrap();
        storage.create_record(record("other", vec![a, b])).unwrap();
        let results = storage.search_records(Some("mail"), &[a, b]).unwrap();
        assert_eq!(
            results.iter().map(|r| r.id).collect::<Vec<_>>(),
            vec![wanted]
        );
        assert!(matches!(
            storage.search_records(None, &[TagId::MAX]),
            Err(StorageError::TagNotFound(_))
        ));
    }
}
