use crate::{Result, output};
use repass_storage::{Storage, TagCatalogStatus};
use std::ffi::OsString;
use std::io::{self, Write};
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
    storage: Option<Storage>,
    warnings_enabled: bool,
}

impl Session {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            storage: None,
            warnings_enabled: true,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_storage(data_dir: PathBuf, storage: Storage) -> Self {
        Self {
            data_dir,
            storage: Some(storage),
            warnings_enabled: true,
        }
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn warnings_enabled(&self) -> bool {
        self.warnings_enabled
    }

    pub fn set_warnings_enabled(&mut self, enabled: bool) {
        self.warnings_enabled = enabled;
    }

    pub fn ensure_storage(
        &mut self,
        output_stream: &mut impl Write,
        interactive: bool,
    ) -> Result<&mut Storage> {
        if self.storage.is_none() {
            let mut password = read_master_password(output_stream, interactive)?.into_bytes();
            let result = Storage::open_or_create_in(&self.data_dir, &password);
            password.fill(0);
            let storage = result?;
            if let TagCatalogStatus::Unavailable(reason) = storage.tag_catalog_status() {
                output::warning(
                    output_stream,
                    format!("tag catalog unavailable: {reason}; use `tag recover` to rebuild it"),
                )?;
            }
            self.storage = Some(storage);
        }
        self.storage
            .as_mut()
            .ok_or_else(|| "storage initialization did not produce an open vault".into())
    }

    pub fn initialize_storage(
        &mut self,
        output_stream: &mut impl Write,
        interactive: bool,
    ) -> Result<()> {
        let mut password = read_master_password(output_stream, interactive)?.into_bytes();
        let result = Storage::create_in(&self.data_dir, &password);
        password.fill(0);
        self.storage = Some(result?);
        Ok(())
    }

    pub fn switch(&mut self, directory: PathBuf) -> Result<()> {
        let directory = expand(directory, std::env::var_os("HOME"))?;
        self.close();
        self.data_dir = directory;
        // The new vault remains unopened until a storage command needs it.
        Ok(())
    }

    pub fn close(&mut self) {
        self.storage.take();
    }
}

fn read_master_password(output_stream: &mut impl Write, interactive: bool) -> Result<String> {
    if interactive {
        output::styled(output_stream, output::PROMPT, "Master password: ")?;
        output_stream.flush()?;
    } else {
        write!(io::stderr(), "Master password: ")?;
        io::stderr().flush()?;
    }
    Ok(rpassword::read_password()?)
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
        assert_eq!(session.data_dir(), Path::new("old"));
        assert!(session.switch(PathBuf::new()).is_err());
        assert_eq!(session.data_dir(), Path::new("old"));
        session.switch("new".into()).unwrap();
        assert_eq!(session.data_dir(), Path::new("new"));
        assert!(session.storage.is_none());
    }
}
