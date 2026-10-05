use crate::Result;
use repass_storage::record::serialize::Vault;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub fn resolve_data_dir(explicit: Option<PathBuf>) -> Result<PathBuf> {
    resolve(
        explicit,
        std::env::var_os("REPASS_DATA_DIR"),
        std::env::var_os("HOME"),
    )
}

fn resolve(
    explicit: Option<PathBuf>,
    environment: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf> {
    match explicit.or_else(|| environment.map(PathBuf::from)) {
        Some(path) => expand(path, home),
        None => Ok(home_directory(home)?.join(".repass")),
    }
}

fn home_directory(home: Option<OsString>) -> Result<PathBuf> {
    match home {
        Some(home) if !home.is_empty() => Ok(home.into()),
        _ => Err("HOME is unavailable; specify --data-dir or REPASS_DATA_DIR".into()),
    }
}

fn expand(path: PathBuf, home: Option<OsString>) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err("data directory must not be empty".into());
    }
    if let Ok(suffix) = path.strip_prefix("~") {
        return Ok(home_directory(home)?.join(suffix));
    }
    Ok(path)
}

pub struct Session {
    data_dir: PathBuf,
    vault: Option<Vault>,
}

impl Session {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            vault: None,
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn storage_operation(&mut self, requirement: &str) -> Result<()> {
        // TODO: storage must resolve its files and initialize the data directory.
        // Vault::create/open already work with a file path, not a directory.
        // Once a directory-level API exists, request the master password lazily,
        // retain the returned Vault here, and reuse its unwrapped key on every call.
        // Domain mutation and saving must use the module's record/tag schema;
        // save_documents replaces all documents, so it is not a partial-update API.
        Err(format!(
            "TODO: repass_storage needs a directory-level vault API for file discovery and initialization (directory: {}); this command also needs {requirement}",
            self.data_dir.display()
        ).into())
    }

    pub fn switch(&mut self, directory: PathBuf) -> Result<()> {
        let directory = expand(directory, std::env::var_os("HOME"))?;
        self.close();
        self.data_dir = directory;
        // The new vault remains unopened until a storage command needs it.
        Ok(())
    }

    pub fn close(&mut self) {
        // There are no implemented mutations or unsaved changes yet. Vault's
        // current save API writes synchronously; closing releases ownership.
        // TODO: when mutations exist, propagate save failures before dropping
        // the vault or switching directories, retaining the session on failure.
        self.vault.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_precedence_and_tilde_expansion() {
        let home = Some(OsString::from("/home/test"));
        let env = Some(OsString::from("/environment"));
        assert_eq!(
            resolve(Some("/explicit".into()), env.clone(), home.clone()).unwrap(),
            PathBuf::from("/explicit")
        );
        assert_eq!(
            resolve(None, env, home.clone()).unwrap(),
            PathBuf::from("/environment")
        );
        assert_eq!(
            resolve(None, None, home.clone()).unwrap(),
            PathBuf::from("/home/test/.repass")
        );
        assert_eq!(
            resolve(Some("~/custom".into()), None, home).unwrap(),
            PathBuf::from("/home/test/custom")
        );
        assert!(resolve(None, None, None).is_err());
        assert!(resolve(None, Some(OsString::new()), None).is_err());
        assert_eq!(
            resolve(Some("relative".into()), None, None).unwrap(),
            PathBuf::from("relative")
        );
    }

    #[test]
    fn switching_is_lazy_and_failed_operations_retain_the_directory() {
        let mut session = Session::new("old".into());
        assert!(session.storage_operation("listing").is_err());
        assert_eq!(session.data_dir(), Path::new("old"));
        assert!(session.switch(PathBuf::new()).is_err());
        assert_eq!(session.data_dir(), Path::new("old"));
        session.switch("new".into()).unwrap();
        assert_eq!(session.data_dir(), Path::new("new"));
        assert!(session.vault.is_none());
    }
}
