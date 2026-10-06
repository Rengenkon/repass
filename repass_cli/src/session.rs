use crate::{Result, output};
use repass_storage::{Storage, StorageError, TagCatalogStatus, VaultError};
#[cfg(test)]
use std::collections::VecDeque;
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
    secret_input: Box<dyn SecretInput>,
}

trait SecretInput {
    fn read_secret(&mut self) -> io::Result<String>;
}

struct TerminalSecretInput;

impl SecretInput for TerminalSecretInput {
    fn read_secret(&mut self) -> io::Result<String> {
        rpassword::read_password()
    }
}

#[cfg(test)]
struct QueuedSecrets(VecDeque<String>);

#[cfg(test)]
impl SecretInput for QueuedSecrets {
    fn read_secret(&mut self) -> io::Result<String> {
        self.0
            .pop_front()
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "no test secret remains"))
    }
}

impl Session {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            storage: None,
            warnings_enabled: true,
            secret_input: Box::new(TerminalSecretInput),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_storage(data_dir: PathBuf, storage: Storage) -> Self {
        Self {
            data_dir,
            storage: Some(storage),
            warnings_enabled: true,
            secret_input: Box::new(TerminalSecretInput),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_passwords(
        data_dir: PathBuf,
        passwords: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            data_dir,
            storage: None,
            warnings_enabled: true,
            secret_input: Box::new(QueuedSecrets(passwords.into_iter().collect())),
        }
    }

    pub(crate) fn read_secret(&mut self) -> io::Result<String> {
        let secret = self.secret_input.read_secret()?;
        if secret.len() > crate::input::MAX_TEXT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "secret exceeds 1 MiB size limit",
            ));
        }
        Ok(secret)
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
            let mut password =
                read_master_password(self.secret_input.as_mut(), output_stream, interactive)?
                    .into_bytes();
            let result: Result<Storage> = (|| match Storage::open_in(&self.data_dir, &password) {
                Ok(storage) => Ok(storage),
                Err(error) if missing_vault(&error) => {
                    confirm_master_password(
                        self.secret_input.as_mut(),
                        &password,
                        output_stream,
                        interactive,
                    )?;
                    Ok(Storage::open_or_create_in(&self.data_dir, &password)?)
                }
                Err(error) => Err(error.into()),
            })();
            password.fill(0);
            let storage = result?;
            if let TagCatalogStatus::Unavailable(reason) = storage.tag_catalog_status() {
                output::warning(
                    output_stream,
                    format!(
                        "tag catalog unavailable: {reason}; use `vault recover` to restore a backup or `tag recover` to rebuild technical names"
                    ),
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
        let mut password =
            read_master_password(self.secret_input.as_mut(), output_stream, interactive)?
                .into_bytes();
        let result = confirm_master_password(
            self.secret_input.as_mut(),
            &password,
            output_stream,
            interactive,
        )
        .and_then(|()| Storage::create_in(&self.data_dir, &password).map_err(Into::into));
        password.fill(0);
        self.storage = Some(result?);
        Ok(())
    }

    pub fn recover_storage(
        &mut self,
        output_stream: &mut impl Write,
        interactive: bool,
        finish: bool,
    ) -> Result<()> {
        self.close();
        let mut password =
            read_master_password(self.secret_input.as_mut(), output_stream, interactive)?
                .into_bytes();
        let result = if finish {
            Storage::finish_initialization_in(&self.data_dir, &password)
        } else {
            Storage::recover_in(&self.data_dir, &password)
        };
        password.fill(0);
        self.storage = Some(result?);
        Ok(())
    }

    pub fn change_master_password(
        &mut self,
        output_stream: &mut impl Write,
        interactive: bool,
    ) -> Result<()> {
        self.ensure_storage(output_stream, interactive)?;
        let mut password = read_secret_prompt(
            self.secret_input.as_mut(),
            output_stream,
            interactive,
            "New master password: ",
        )?
        .into_bytes();
        let confirmation = read_secret_prompt(
            self.secret_input.as_mut(),
            output_stream,
            interactive,
            "Confirm new master password: ",
        );
        let result = match confirmation {
            Ok(value) => {
                let mut confirmation = value.into_bytes();
                let matches = confirmation == password;
                confirmation.fill(0);
                if !matches {
                    Err("master passwords do not match".into())
                } else if password.is_empty() {
                    Err("new master password must not be empty".into())
                } else {
                    self.storage
                        .as_mut()
                        .ok_or("vault is not open")?
                        .change_master_password(&password)
                        .map_err(Into::into)
                }
            }
            Err(error) => Err(error),
        };
        password.fill(0);
        result
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

fn read_master_password(
    secret_input: &mut dyn SecretInput,
    output_stream: &mut impl Write,
    interactive: bool,
) -> Result<String> {
    read_secret_prompt(
        secret_input,
        output_stream,
        interactive,
        "Master password: ",
    )
}

fn missing_vault(error: &StorageError) -> bool {
    match error {
        StorageError::Io(error) | StorageError::Vault(VaultError::Io(error)) => {
            error.kind() == io::ErrorKind::NotFound
        }
        _ => false,
    }
}

fn confirm_master_password(
    secret_input: &mut dyn SecretInput,
    password: &[u8],
    output_stream: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    if password.is_empty() {
        return Err("master password cannot be empty".into());
    }
    let mut confirmation = read_secret_prompt(
        secret_input,
        output_stream,
        interactive,
        "Confirm master password: ",
    )?
    .into_bytes();
    let matches = confirmation == password;
    confirmation.fill(0);
    if !matches {
        return Err("master passwords do not match".into());
    }
    Ok(())
}

fn read_secret_prompt(
    secret_input: &mut dyn SecretInput,
    output_stream: &mut impl Write,
    interactive: bool,
    prompt: &str,
) -> Result<String> {
    if interactive {
        output::styled(output_stream, output::PROMPT, prompt)?;
        output_stream.flush()?;
    } else {
        write!(io::stderr(), "{prompt}")?;
        io::stderr().flush()?;
    }
    let secret = secret_input.read_secret()?;
    if secret.len() > crate::input::MAX_TEXT_BYTES {
        return Err("secret exceeds 1 MiB size limit".into());
    }
    Ok(secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn one_shot_password_change_does_not_write_prompts_to_stdout() {
        let directory = TestDirectory::new();
        Storage::create_in(&directory.0, b"old").unwrap();
        let mut session = Session::with_passwords(
            directory.0.clone(),
            ["old".into(), "new".into(), "new".into()],
        );
        let mut stdout = Vec::new();
        session.change_master_password(&mut stdout, false).unwrap();
        assert!(stdout.is_empty());
        session.close();
        assert!(Storage::open_in(&directory.0, b"new").is_ok());
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!("repass-session-{}-{id}", std::process::id())))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn explicit_and_lazy_creation_reject_empty_mismatched_and_missing_confirmation() {
        for explicit in [false, true] {
            for secrets in [vec![""], vec!["master", "different"], vec!["master"]] {
                let directory = TestDirectory::new();
                let mut session = Session::with_passwords(
                    directory.0.clone(),
                    secrets.iter().map(|s| s.to_string()),
                );
                let mut output = Vec::new();
                let result = if explicit {
                    session.initialize_storage(&mut output, true)
                } else {
                    session.ensure_storage(&mut output, true).map(|_| ())
                };
                assert!(result.is_err());
                assert!(session.storage.is_none());
                assert!(!directory.0.exists());
                let output = String::from_utf8(output).unwrap();
                assert!(output.contains("Master password:"));
                assert_eq!(
                    output.contains("Confirm master password:"),
                    !secrets[0].is_empty()
                );
                assert!(!output.contains("different"));
            }
        }
    }

    #[test]
    fn explicit_and_lazy_creation_confirm_but_reopening_only_reads_one_password() {
        for explicit in [false, true] {
            let directory = TestDirectory::new();
            let mut session = Session::with_passwords(
                directory.0.clone(),
                ["master-secret".into(), "master-secret".into()],
            );
            let mut output = Vec::new();
            if explicit {
                session.initialize_storage(&mut output, true).unwrap();
            } else {
                session.ensure_storage(&mut output, true).unwrap();
            }
            let output = String::from_utf8(output).unwrap();
            assert!(output.contains("Confirm master password:"));
            assert!(!output.contains("master-secret"));
            session.close();
            let mut session =
                Session::with_passwords(directory.0.clone(), ["master-secret".into()]);
            let mut output = Vec::new();
            session.ensure_storage(&mut output, true).unwrap();
            assert!(!String::from_utf8(output).unwrap().contains("Confirm"));
        }
    }

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
