use crate::dictionary::Dictionary;
use crate::error::{DictionaryError, GeneratorError};
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct FileDictionary {
    dictionary: Vec<String>,
    known_entries: Option<HashSet<String>>,
}

impl FileDictionary {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut dictionary = Vec::new();
        loop {
            let mut buf = String::new();
            let n = reader.read_line(&mut buf)?;
            if n == 0 {
                break;
            }
            while buf.ends_with(['\n', '\r']) {
                buf.pop();
            }
            if !buf.is_empty() {
                dictionary.push(buf);
            }
        }
        Ok(Self {
            dictionary,
            known_entries: None,
        })
    }

    pub fn from_entries<I, S>(entries: I) -> Result<Self, GeneratorError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let dictionary = entries.into_iter().map(Into::into).collect();
        let result = Self {
            dictionary,
            known_entries: None,
        };
        result.validate()?;
        Ok(result)
    }

    /// Saves entries as UTF-8 text, with one entry per line.
    ///
    /// Entries containing line breaks are rejected because they cannot be
    /// represented unambiguously by this format. The target is replaced only
    /// after the complete temporary file has been written and flushed.
    pub fn save_to_path(&self, path: impl AsRef<Path>) -> io::Result<()> {
        for entry in &self.dictionary {
            if entry.is_empty() || entry.contains(['\n', '\r']) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "dictionary entries must be non-empty and must not contain line breaks",
                ));
            }
        }

        let path = path.as_ref();
        let parent = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };

        let (temporary_path, file) = loop {
            let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let temporary_name =
                format!(".repass-dictionary.{}.{}.tmp", std::process::id(), counter);
            let temporary_path = parent.join(temporary_name);

            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)
            {
                Ok(file) => break (temporary_path, file),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };

        let write_result = (|| {
            let mut writer = BufWriter::new(file);
            for entry in &self.dictionary {
                writer.write_all(entry.as_bytes())?;
                writer.write_all(b"\n")?;
            }
            writer.flush()?;
            writer.get_ref().sync_all()?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }

        if let Err(error) = fs::rename(&temporary_path, path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }

        Ok(())
    }
}

impl<'a> Dictionary<'a> for FileDictionary {
    fn get(&self, index: usize) -> Option<&str> {
        let x = self.dictionary.get(index)?;
        Some(x.as_str())
    }

    fn add(&mut self, values: &'a [&'a str]) -> Result<(), DictionaryError> {
        if values.is_empty() {
            return Ok(());
        }

        if let Some(known_entries) = &self.known_entries {
            let mut incoming_entries = HashSet::new();
            incoming_entries
                .try_reserve(values.len())
                .map_err(|_| DictionaryError::ResourceLimit)?;
            for value in values {
                if value.is_empty() {
                    return Err(DictionaryError::EmptyEntry);
                }
                if known_entries.contains(*value) || !incoming_entries.insert(*value) {
                    return Err(DictionaryError::DuplicateEntry((*value).to_owned()));
                }
            }

            let new_len = self
                .dictionary
                .len()
                .checked_add(values.len())
                .ok_or(DictionaryError::ResourceLimit)?;
            self.dictionary
                .try_reserve(values.len())
                .map_err(|_| DictionaryError::ResourceLimit)?;
            if let Some(known_entries) = self.known_entries.as_mut() {
                known_entries
                    .try_reserve(values.len())
                    .map_err(|_| DictionaryError::ResourceLimit)?;
                for value in values {
                    known_entries.insert((*value).to_owned());
                    self.dictionary.push((*value).to_owned());
                }
            }
            debug_assert_eq!(self.dictionary.len(), new_len);
            return Ok(());
        }

        let mut known_entries = HashSet::new();
        known_entries
            .try_reserve(self.dictionary.len().saturating_add(values.len()))
            .map_err(|_| DictionaryError::ResourceLimit)?;
        for entry in &self.dictionary {
            known_entries.insert(entry.clone());
        }
        let mut incoming_entries = HashSet::new();
        incoming_entries
            .try_reserve(values.len())
            .map_err(|_| DictionaryError::ResourceLimit)?;
        for value in values {
            if value.is_empty() {
                return Err(DictionaryError::EmptyEntry);
            }
            if !incoming_entries.insert(*value) || known_entries.contains(*value) {
                return Err(DictionaryError::DuplicateEntry((*value).to_owned()));
            }
        }

        self.dictionary
            .try_reserve(values.len())
            .map_err(|_| DictionaryError::ResourceLimit)?;
        for value in values {
            known_entries.insert((*value).to_owned());
            self.dictionary.push((*value).to_owned());
        }
        self.known_entries = Some(known_entries);
        Ok(())
    }

    fn len(&self) -> usize {
        self.dictionary.len()
    }
}

#[cfg(test)]
mod tests {
    use super::FileDictionary;
    use crate::dictionary::Dictionary;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temporary_test_path(name: &str) -> std::path::PathBuf {
        static TEST_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = TEST_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "repass-dictionary-{name}-{}-{counter}.txt",
            std::process::id()
        ))
    }

    #[test]
    fn keeps_unicode_entries() {
        let dictionary = FileDictionary::from_entries(["é", "word"]).unwrap();
        assert_eq!(dictionary.get(0), Some("é"));
        assert_eq!(dictionary.get(1), Some("word"));
    }

    #[test]
    fn rejects_empty_dictionary() {
        assert!(FileDictionary::from_entries(Vec::<String>::new()).is_err());
    }

    #[test]
    fn rejects_empty_entry() {
        assert!(FileDictionary::from_entries(["valid", ""]).is_err());
    }

    #[test]
    fn add_rejects_existing_entries_without_partial_mutation() {
        let mut dictionary = FileDictionary::from_entries(["one", "two"]).unwrap();
        assert!(dictionary.known_entries.is_none());
        assert_eq!(
            dictionary.add(&["three", "one"]),
            Err(crate::error::DictionaryError::DuplicateEntry(
                "one".to_owned()
            ))
        );
        assert_eq!(dictionary.len(), 2);
        assert!(dictionary.known_entries.is_none());
        assert_eq!(dictionary.add(&["three", "four"]), Ok(()));
        assert!(dictionary.known_entries.is_some());
        assert_eq!(dictionary.get(2), Some("three"));
        assert_eq!(
            dictionary.add(&["five", "three"]),
            Err(crate::error::DictionaryError::DuplicateEntry(
                "three".to_owned()
            ))
        );
        assert_eq!(dictionary.len(), 4);
        assert_eq!(dictionary.get(3), Some("four"));
    }

    #[test]
    fn file_loader_trims_lf_and_crlf() {
        let path = temporary_test_path("loader");
        std::fs::write(&path, "alpha\r\nbeta\n").unwrap();
        let dictionary = FileDictionary::from_path(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(dictionary.get(0), Some("alpha"));
        assert_eq!(dictionary.get(1), Some("beta"));
    }

    #[test]
    fn saves_and_reloads_unicode_entries_over_existing_file() {
        let path = temporary_test_path("save");
        std::fs::write(&path, "old contents\n").unwrap();
        let dictionary = FileDictionary::from_entries(["abc", "猫", "🦀"]).unwrap();

        dictionary.save_to_path(&path).unwrap();
        let loaded = FileDictionary::from_path(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.get(0), Some("abc"));
        assert_eq!(loaded.get(1), Some("猫"));
        assert_eq!(loaded.get(2), Some("🦀"));
    }

    #[test]
    fn refuses_line_breaks_without_replacing_existing_file() {
        let path = temporary_test_path("invalid-save");
        std::fs::write(&path, "keep this\n").unwrap();
        for invalid_entry in ["not\nsafe", "not\rsafe"] {
            let dictionary = FileDictionary::from_entries(["safe", invalid_entry]).unwrap();

            let error = dictionary.save_to_path(&path).unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep this\n");
        }
        let _ = std::fs::remove_file(&path);
    }
}
