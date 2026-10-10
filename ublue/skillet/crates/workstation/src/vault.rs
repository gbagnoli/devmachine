//! `KeePassXC` database access, atomic persistence, and session password cache.

use keepass::{Database, DatabaseKey};
use keyutils::{keytypes::User, Keyring, Permission, SpecialKeyring};
use sha2::{Digest, Sha256};
use std::{
    error::Error as StdError,
    fs,
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const CACHE_LIFETIME: Duration = Duration::from_hours(3);
const SESSION_KEYRING_NAME: &str = "skillet-vault-cache";
static SESSION_KEYRING: Mutex<Option<Keyring>> = Mutex::new(None);

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("{context}: {source}")]
    Source {
        context: String,
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    #[error("{0}")]
    Invalid(String),
}

fn source<E>(context: impl Into<String>, error: E) -> VaultError
where
    E: StdError + Send + Sync + 'static,
{
    VaultError::Source {
        context: context.into(),
        source: Box::new(error),
    }
}

pub struct Vault {
    path: PathBuf,
    original: Vec<u8>,
    database: Database,
    password: String,
    password_cached: bool,
}

impl Vault {
    /// Enumerate entry paths without accessing Password or Username values.
    pub fn entry_paths(&self) -> Vec<String> {
        let mut paths = Vec::new();
        let mut pending = vec![(self.database.root().id(), String::new())];
        while let Some((id, prefix)) = pending.pop() {
            let Some(group) = self.database.group(id) else {
                continue;
            };
            for entry in group.entry_ids().filter_map(|id| self.database.entry(id)) {
                paths.push(format!(
                    "{prefix}{}",
                    entry.get_title().unwrap_or("<untitled>")
                ));
            }
            for child in group.group_ids().filter_map(|id| self.database.group(id)) {
                pending.push((child.id(), format!("{prefix}{}/", child.name)));
            }
        }
        paths.sort();
        paths
    }

    pub fn default_path() -> Result<PathBuf, VaultError> {
        let xdg = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from);
        let home = std::env::var_os("HOME").map(PathBuf::from);
        database_path_from(xdg.as_deref(), home.as_deref())
    }

    pub fn open(path: &Path, key_file: Option<&Path>) -> Result<Self, VaultError> {
        // Resolve the symlink before any atomic replacement so the synced target
        // is updated rather than replacing the symlink itself.
        let path = fs::canonicalize(path).map_err(|error| {
            source(
                format!("KeePassXC database is missing or unreadable at {}; place the synced vault there or pass --database", path.display()),
                error,
            )
        })?;
        let original =
            fs::read(&path).map_err(|error| source("reading KeePassXC database", error))?;
        if let Some(cached) = cache_read(&path)? {
            if let Ok(database) = open_with_password(&original, &cached, key_file) {
                return Ok(Self {
                    path,
                    original,
                    database,
                    password: cached,
                    password_cached: true,
                });
            }
            cache_clear(&path)?;
        }
        let password = rpassword::prompt_password("KeePassXC database password: ")
            .map_err(|error| source("reading database password from terminal", error))?;
        let database = open_with_password(&original, &password, key_file)?;
        let password_cached = cache_store(&path, &password)?;
        Ok(Self {
            path,
            original,
            database,
            password,
            password_cached,
        })
    }

    /// Whether the verified password is currently stored in the session keyring.
    pub fn password_cached(&self) -> bool {
        self.password_cached
    }

    pub fn get(&self, path: &str) -> Result<Option<String>, VaultError> {
        lookup(&self.database, path)
    }

    /// Read a standard or custom field from an exact entry without logging values.
    pub fn get_field(&self, path: &str, field: &str) -> Result<Option<String>, VaultError> {
        lookup_field(&self.database, path, field)
    }

    pub fn insert(&mut self, path: &str, secret: &str) -> Result<(), VaultError> {
        create_entry(&mut self.database, path, secret)
    }

    /// Add metadata to an existing exact entry before an atomic verified save.
    pub fn set_field(&mut self, path: &str, field: &str, value: &str) -> Result<(), VaultError> {
        let id = find_entry(&self.database, path)?
            .ok_or_else(|| VaultError::Invalid("vault metadata entry is absent".into()))?
            .id();
        let mut entry = self
            .database
            .entry_mut(id)
            .ok_or_else(|| VaultError::Invalid("vault entry disappeared".into()))?;
        entry.set_protected(field, value);
        Ok(())
    }

    pub fn ensure_unchanged(&self) -> Result<(), VaultError> {
        if fs::read(&self.path).map_err(|error| source("rechecking vault before write", error))?
            != self.original
        {
            return Err(VaultError::Invalid(
                "KeePassXC database changed while Skillet was running; reopen it and retry".into(),
            ));
        }
        Ok(())
    }

    pub fn save_verified(
        &mut self,
        key_file: Option<&Path>,
        entry_path: &str,
        secret: &str,
    ) -> Result<(), VaultError> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| VaultError::Invalid("vault has no parent directory".into()))?;
        let lock_path = parent.join(".skillet-vault.lock");
        let lock = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .mode(0o600)
            .open(lock_path)
            .map_err(|error| source("opening vault write lock", error))?;
        lock.lock()
            .map_err(|error| source("locking vault for update", error))?;
        self.ensure_unchanged()?;

        let mut candidate = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| source("creating encrypted vault update", error))?;
        self.database
            .save(
                candidate.as_file_mut(),
                database_key(&self.password, key_file)?,
            )
            .map_err(|error| source("saving KeePassXC database", error))?;
        candidate
            .as_file_mut()
            .flush()
            .map_err(|error| source("flushing encrypted vault update", error))?;
        candidate
            .as_file()
            .set_permissions(
                fs::metadata(&self.path)
                    .map_err(|error| source("reading vault metadata", error))?
                    .permissions(),
            )
            .map_err(|error| source("preserving vault permissions", error))?;
        candidate
            .as_file()
            .sync_all()
            .map_err(|error| source("syncing encrypted vault update", error))?;
        let candidate_bytes = fs::read(candidate.path())
            .map_err(|error| source("reading encrypted vault update", error))?;
        let reopened = open_with_password(&candidate_bytes, &self.password, key_file)?;
        if lookup(&reopened, entry_path)?.as_deref() != Some(secret) {
            return Err(VaultError::Invalid(
                "saved vault did not retain the generated credential".into(),
            ));
        }

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| source("reading system time", error))?
            .as_nanos();
        let backup_name = format!(
            "{}.skillet-{stamp}.bak",
            self.path
                .file_name()
                .ok_or_else(|| VaultError::Invalid("vault has no filename".into()))?
                .to_string_lossy()
        );
        let mut backup = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| source("creating vault backup", error))?;
        backup
            .write_all(&self.original)
            .map_err(|error| source("writing encrypted vault backup", error))?;
        backup
            .as_file()
            .set_permissions(
                fs::metadata(&self.path)
                    .map_err(|error| source("reading vault metadata", error))?
                    .permissions(),
            )
            .map_err(|error| source("preserving backup permissions", error))?;
        backup
            .as_file()
            .sync_all()
            .map_err(|error| source("syncing encrypted vault backup", error))?;
        backup
            .persist_noclobber(parent.join(backup_name))
            .map_err(|error| source("preserving previous encrypted vault", error.error))?;
        self.ensure_unchanged()?;
        candidate
            .persist(&self.path)
            .map_err(|error| source("atomically replacing KeePassXC database", error.error))?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| source("syncing vault directory", error))?;
        self.original = fs::read(&self.path)
            .map_err(|error| source("refreshing vault snapshot after save", error))?;
        Ok(())
    }
}

/// Minimal exact-entry persistence boundary shared by workstation provisioners.
pub trait SecretStore {
    fn get(&self, path: &str) -> Result<Option<String>, VaultError>;
    fn ensure_unchanged(&self) -> Result<(), VaultError>;
    fn save_verified(&mut self, path: &str, secret: &str) -> Result<(), VaultError>;
}

pub struct VaultSecretStore<'a> {
    vault: &'a mut Vault,
    key_file: Option<&'a Path>,
}

impl<'a> VaultSecretStore<'a> {
    pub fn new(vault: &'a mut Vault, key_file: Option<&'a Path>) -> Self {
        Self { vault, key_file }
    }
}

impl SecretStore for VaultSecretStore<'_> {
    fn get(&self, path: &str) -> Result<Option<String>, VaultError> {
        self.vault.get(path)
    }

    fn ensure_unchanged(&self) -> Result<(), VaultError> {
        self.vault.ensure_unchanged()
    }

    fn save_verified(&mut self, path: &str, secret: &str) -> Result<(), VaultError> {
        self.vault.insert(path, secret)?;
        self.vault.save_verified(self.key_file, path, secret)
    }
}

pub fn lock(path: Option<&Path>) -> Result<(), VaultError> {
    let path = match path {
        Some(path) => path.to_path_buf(),
        None => Vault::default_path()?,
    };
    let canonical =
        fs::canonicalize(path).map_err(|error| source("locating KeePassXC database", error))?;
    cache_clear(&canonical)
}

pub fn database_path_from(
    xdg_data_home: Option<&Path>,
    home: Option<&Path>,
) -> Result<PathBuf, VaultError> {
    let data_home = if let Some(path) = xdg_data_home.filter(|path| path.is_absolute()) {
        path.to_path_buf()
    } else {
        let home = home.filter(|path| path.is_absolute()).ok_or_else(|| {
            VaultError::Invalid("set an absolute XDG_DATA_HOME or HOME, or pass --database".into())
        })?;
        home.join(".local/share")
    };
    Ok(data_home.join("skillet/secrets.kdbx"))
}

fn database_key(password: &str, key_file: Option<&Path>) -> Result<DatabaseKey, VaultError> {
    let mut key = DatabaseKey::new().with_password(password);
    if let Some(path) = key_file {
        let mut file =
            fs::File::open(path).map_err(|error| source("opening KeePassXC key file", error))?;
        key = key
            .with_keyfile(&mut file)
            .map_err(|error| source("reading KeePassXC key file", error))?;
    }
    Ok(key)
}

fn open_with_password(
    bytes: &[u8],
    password: &str,
    key_file: Option<&Path>,
) -> Result<Database, VaultError> {
    let mut reader = bytes;
    Database::open(&mut reader, database_key(password, key_file)?).map_err(|error| {
        source(
            "opening KeePassXC database; check password and key file",
            error,
        )
    })
}

fn lookup(database: &Database, path: &str) -> Result<Option<String>, VaultError> {
    let Some(secret) = lookup_field(database, path, "Password")? else {
        if find_entry(database, path)?.is_some() {
            return Err(VaultError::Invalid(format!(
                "KeePassXC entry {path} has no Password field"
            )));
        }
        return Ok(None);
    };
    if secret.is_empty() {
        return Err(VaultError::Invalid(format!(
            "KeePassXC entry {path} has an empty Password field"
        )));
    }
    Ok(Some(secret))
}

fn lookup_field(
    database: &Database,
    path: &str,
    field: &str,
) -> Result<Option<String>, VaultError> {
    Ok(find_entry(database, path)?.and_then(|entry| entry.get(field).map(str::to_owned)))
}

fn find_entry<'a>(
    database: &'a Database,
    path: &str,
) -> Result<Option<keepass::db::EntryRef<'a>>, VaultError> {
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return Err(VaultError::Invalid("invalid KeePassXC entry path".into()));
    }
    let mut groups = vec![database.root().id()];
    for part in &parts[..parts.len() - 1] {
        groups = groups
            .into_iter()
            .filter_map(|id| database.group(id))
            .flat_map(|group| group.group_ids().collect::<Vec<_>>())
            .filter(|id| database.group(*id).is_some_and(|group| group.name == *part))
            .collect();
    }
    let mut matches = groups
        .iter()
        .filter_map(|id| database.group(*id))
        .flat_map(|group| group.entry_ids().collect::<Vec<_>>())
        .filter_map(|id| database.entry(id))
        .filter(|entry| entry.get_title() == parts.last().copied())
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err(VaultError::Invalid(format!(
            "KeePassXC entry {path} is ambiguous"
        )));
    }
    Ok(matches.pop())
}

fn create_entry(database: &mut Database, path: &str, password: &str) -> Result<(), VaultError> {
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() < 2 || parts.iter().any(|part| part.is_empty()) {
        return Err(VaultError::Invalid("invalid KeePassXC entry path".into()));
    }
    let mut parent_id = database.root().id();
    for name in &parts[..parts.len() - 1] {
        let parent = database
            .group(parent_id)
            .ok_or_else(|| VaultError::Invalid("vault group disappeared".into()))?;
        let matching = parent
            .group_ids()
            .filter(|id| database.group(*id).is_some_and(|group| group.name == *name))
            .collect::<Vec<_>>();
        parent_id = match matching.as_slice() {
            [id] => *id,
            [] => {
                let mut parent = database
                    .group_mut(parent_id)
                    .ok_or_else(|| VaultError::Invalid("vault group disappeared".into()))?;
                let mut group = parent.add_group();
                (*name).clone_into(&mut group.name);
                group.id()
            }
            _ => {
                return Err(VaultError::Invalid(format!(
                    "KeePassXC group {name} is ambiguous"
                )))
            }
        };
    }
    if lookup(database, path)?.is_some() {
        return Err(VaultError::Invalid(format!(
            "KeePassXC entry {path} already exists"
        )));
    }
    let mut parent = database
        .group_mut(parent_id)
        .ok_or_else(|| VaultError::Invalid("vault group disappeared".into()))?;
    let mut entry = parent.add_entry();
    entry.set_unprotected("Title", parts[parts.len() - 1]);
    entry.set_protected("Password", password);
    Ok(())
}

fn cache_description(database: &Path) -> String {
    let digest = Sha256::digest(database.as_os_str().as_encoded_bytes());
    format!("skillet:vault:{}", hex::encode(digest))
}

fn session_keyring() -> Result<Keyring, VaultError> {
    let mut cached = SESSION_KEYRING
        .lock()
        .map_err(|_| VaultError::Invalid("vault session keyring lock was poisoned".into()))?;
    if let Some(ring) = cached.as_ref() {
        return Ok(ring.clone());
    }
    let current = Keyring::attach_or_create(SpecialKeyring::Session)
        .ok()
        .and_then(|ring| {
            ring.description()
                .ok()
                .filter(|description| description.description == SESSION_KEYRING_NAME)
                .map(|_| ring)
        });
    let mut ring = match current {
        Some(ring) => ring,
        None => Keyring::join_session(SESSION_KEYRING_NAME)
            .map_err(|error| source("opening the named Linux session keyring", error))?,
    };
    ring.set_permissions(Permission::POSSESSOR_ALL | Permission::USER_SEARCH)
        .map_err(|error| source("setting access on the named vault session keyring", error))?;
    let mut user = Keyring::attach_or_create(SpecialKeyring::User)
        .map_err(|error| source("opening the Linux user keyring", error))?;
    user.link_keyring(&ring)
        .map_err(|error| source("linking the named vault session keyring", error))?;
    *cached = Some(ring.clone());
    Ok(ring)
}

fn cache_read(database: &Path) -> Result<Option<String>, VaultError> {
    let Ok(ring) = session_keyring() else {
        return Ok(None);
    };
    let Ok(key) = ring.search_for_key::<User, _, _>(cache_description(database), None) else {
        return Ok(None);
    };
    String::from_utf8(
        key.read()
            .map_err(|error| source("reading cached vault password", error))?,
    )
    .map(Some)
    .map_err(|error| source("decoding cached vault password", error))
}

fn cache_store(database: &Path, password: &str) -> Result<bool, VaultError> {
    let mut ring = match session_keyring() {
        Ok(ring) => ring,
        Err(error) => {
            eprintln!("Vault password was not cached: Linux session keyring unavailable: {error}");
            return Ok(false);
        }
    };
    let Ok(mut key) = ring.add_key::<User, _, _>(cache_description(database), password.as_bytes())
    else {
        eprintln!("Vault password was not cached: could not add a kernel key");
        return Ok(false);
    };
    if key.set_timeout(CACHE_LIFETIME).is_err() {
        ring.unlink_key(&key)
            .map_err(|error| source("removing a vault key whose expiry could not be set", error))?;
        eprintln!("Vault password was not cached: kernel denied the three-hour expiry");
        return Ok(false);
    }
    Ok(true)
}

fn cache_clear(database: &Path) -> Result<(), VaultError> {
    let mut ring = session_keyring()?;
    if let Ok(key) = ring.search_for_key::<User, _, _>(cache_description(database), None) {
        ring.unlink_key(&key)
            .map_err(|error| source("removing cached vault password", error))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "vault/tests.rs"]
mod tests;
