//! Process ownership for an explicitly named database world.
//!
//! The lock file is only the stable object on which the operating system holds
//! an advisory lock. Its contents and existence carry no ownership meaning:
//! there is no PID record to inspect and no stale sentinel to delete.

use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;
#[cfg(any(windows, target_os = "macos"))]
use unicode_normalization::UnicodeNormalization;

const LOCK_DIRECTORY: &str = ".spock-named-state-locks";
const LOCK_SUFFIX: &str = ".lock";

/// Derive the advisory-lock path for one named `--db` value.
///
/// The database is first reduced to a normalized absolute filesystem identity.
/// Its lock then lives in a reserved sibling directory under a SHA-256 name.
/// Database, WAL, SHM, and historical `*.spock.lock` paths therefore cannot
/// accidentally be the advisory object itself.
#[must_use]
pub fn named_state_lock_path(database_path: &Path) -> PathBuf {
    let identity = resolved_database_entry(database_path).unwrap_or_else(|_| {
        lexically_absolute(database_path).unwrap_or_else(|_| database_path.into())
    });
    lock_path_for_identity(&identity)
}

/// Failures before a host owns its named database world.
#[derive(Debug, Error)]
pub enum NamedStateLockError {
    #[error("could not resolve named database identity {}: {source}", path.display())]
    ResolveDatabase {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "named database {} is inside reserved lock namespace `{LOCK_DIRECTORY}`",
        path.display()
    )]
    ReservedDatabasePath { path: PathBuf },
    #[error(
        "named database {} has {links} hard links; use a database path with one directory entry",
        path.display()
    )]
    HardLinkedDatabase { path: PathBuf, links: u64 },
    #[error("could not create named-state lock directory {}: {source}", path.display())]
    CreateParent {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not open named-state lock {}: {source}", path.display())]
    Open {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "named database {} is already owned by another process (lock {})",
        database_path.display(),
        lock_path.display()
    )]
    Contended {
        database_path: PathBuf,
        lock_path: PathBuf,
    },
    #[error("could not acquire named-state lock {}: {source}", path.display())]
    Acquire {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Exclusive process-lifetime ownership of one named database world.
///
/// Acquire this before creating, deleting, or opening the database, its WAL or
/// SHM files, or any related mutable framework state. Keep the value in the
/// host owner for at least as long as those resources are live.
#[derive(Debug)]
pub struct NamedStateLock {
    database_path: PathBuf,
    resolved_database_path: PathBuf,
    lock_path: PathBuf,
    file: File,
}

impl NamedStateLock {
    /// Acquire an exclusive lock without waiting.
    ///
    /// This creates missing parent directories for the lock but deliberately
    /// does not touch the database path. A competing owner produces
    /// [`NamedStateLockError::Contended`] immediately.
    pub fn acquire(database_path: impl AsRef<Path>) -> Result<Self, NamedStateLockError> {
        let database_path = database_path.as_ref().to_path_buf();
        let resolved_database_path = resolved_database_entry(&database_path).map_err(|source| {
            NamedStateLockError::ResolveDatabase {
                path: database_path.clone(),
                source,
            }
        })?;
        if database_uses_lock_namespace(&resolved_database_path) {
            return Err(NamedStateLockError::ReservedDatabasePath {
                path: database_path,
            });
        }
        let link_count =
            existing_regular_file_link_count(&resolved_database_path).map_err(|source| {
                NamedStateLockError::ResolveDatabase {
                    path: database_path.clone(),
                    source,
                }
            })?;
        if let Some(links) = link_count.filter(|links| *links > 1) {
            return Err(NamedStateLockError::HardLinkedDatabase {
                path: database_path,
                links,
            });
        }
        let lock_path = lock_path_for_identity(&resolved_database_path);

        if let Some(parent) = lock_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent).map_err(|source| {
                NamedStateLockError::CreateParent {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| NamedStateLockError::Open {
                path: lock_path.clone(),
                source,
            })?;

        match file.try_lock() {
            Ok(()) => Ok(Self {
                database_path,
                resolved_database_path,
                lock_path,
                file,
            }),
            Err(TryLockError::WouldBlock) => Err(NamedStateLockError::Contended {
                database_path,
                lock_path,
            }),
            Err(TryLockError::Error(source)) => Err(NamedStateLockError::Acquire {
                path: lock_path,
                source,
            }),
        }
    }

    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    /// The stable directory-entry path protected by this lock.
    ///
    /// Destructive database bootstrap must use this path rather than the
    /// caller's spelling. Its parent has been resolved without following the
    /// final database component, so a final-component symlink cannot change
    /// the lock identity when bootstrap replaces it.
    #[must_use]
    pub fn resolved_database_path(&self) -> &Path {
        &self.resolved_database_path
    }

    #[must_use]
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

fn resolved_database_entry(database_path: &Path) -> std::io::Result<PathBuf> {
    let absolute = lexically_absolute(database_path)?;
    let file_name = absolute.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "database path must name a directory entry",
        )
    })?;
    let parent = absolute.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "database path must have a parent directory",
        )
    })?;
    resolve_directory(parent).map(|resolved_parent| resolved_parent.join(file_name))
}

fn resolve_directory(directory: &Path) -> std::io::Result<PathBuf> {
    let mut cursor = directory.to_path_buf();
    let mut missing = Vec::<OsString>::new();

    loop {
        match std::fs::symlink_metadata(&cursor) {
            Ok(_) => {
                let mut resolved = std::fs::canonicalize(&cursor)?;
                for component in missing.iter().rev() {
                    resolved.push(component);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let name = cursor.file_name().ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "database path has no existing ancestor",
                    )
                })?;
                missing.push(name.to_os_string());
                cursor = cursor
                    .parent()
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "database path has no existing ancestor",
                        )
                    })?
                    .to_path_buf();
            }
            Err(error) => return Err(error),
        }
    }
}

fn existing_regular_file_link_count(path: &Path) -> std::io::Result<Option<u64>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() {
        // Destructive bootstrap intentionally acts on a final-component
        // symlink as a directory entry rather than following it. Preserve that
        // behavior; only existing regular database files have hard-link aliases.
        return Ok(None);
    }
    metadata_link_count(path, &metadata)
}

#[cfg(unix)]
fn metadata_link_count(_path: &Path, metadata: &std::fs::Metadata) -> std::io::Result<Option<u64>> {
    use std::os::unix::fs::MetadataExt;

    Ok(Some(metadata.nlink()))
}

#[cfg(windows)]
fn metadata_link_count(path: &Path, _metadata: &std::fs::Metadata) -> std::io::Result<Option<u64>> {
    use cap_fs_ext::MetadataExt as _;
    use std::os::windows::fs::OpenOptionsExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE,
    };

    // Stable Rust does not expose a Windows hard-link count on path metadata.
    // Query capability metadata derived from a handle instead. Opening the
    // final entry as a reparse point also prevents a raced symlink from being
    // followed while obtaining that handle.
    let file = std::fs::OpenOptions::new()
        .access_mode(FILE_READ_ATTRIBUTES)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let metadata = cap_std::fs::File::from_std(file).metadata()?;
    if !metadata.is_file() {
        return Ok(None);
    }

    Ok(Some(metadata.nlink()))
}

#[cfg(not(any(unix, windows)))]
fn metadata_link_count(
    _path: &Path,
    _metadata: &std::fs::Metadata,
) -> std::io::Result<Option<u64>> {
    // The supported release targets expose link counts. Other targets retain
    // path ownership rather than guessing at unavailable file identity.
    Ok(None)
}

fn lexically_absolute(path: &Path) -> std::io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    Ok(normalized)
}

fn lock_path_for_identity(identity: &Path) -> PathBuf {
    let parent = identity
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut name = hex_digest(&database_identity_digest(identity));
    name.push_str(LOCK_SUFFIX);
    parent.join(LOCK_DIRECTORY).join(name)
}

fn database_uses_lock_namespace(identity: &Path) -> bool {
    identity.components().any(|component| match component {
        Component::Normal(segment) => lock_directory_name_matches(segment),
        _ => false,
    })
}

#[cfg(any(windows, target_os = "macos"))]
fn lock_directory_name_matches(segment: &OsStr) -> bool {
    segment
        .to_str()
        .is_some_and(|segment| segment.eq_ignore_ascii_case(LOCK_DIRECTORY))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn lock_directory_name_matches(segment: &OsStr) -> bool {
    segment == LOCK_DIRECTORY
}

fn database_identity_digest(identity: &Path) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"spock-named-state/3\0");
    hash_os_str(&mut hasher, identity.as_os_str());
    hasher.finalize().into()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn hash_os_str(hasher: &mut Sha256, value: &OsStr) {
    use std::os::unix::ffi::OsStrExt;
    hasher.update(value.as_bytes());
}

#[cfg(any(windows, target_os = "macos"))]
fn hash_os_str(hasher: &mut Sha256, value: &OsStr) {
    // Supported case-insensitive filesystems compare through uppercase-style
    // mappings, and supported macOS filesystems also collapse canonical Unicode
    // equivalents. Match the portable path-key policy so aliases such as Greek
    // sigma/final-sigma and composed/decomposed accents share one lock.
    let folded = value
        .to_string_lossy()
        .chars()
        .flat_map(char::to_uppercase)
        .nfd()
        .collect::<String>();
    hasher.update(folded.as_bytes());
}

#[cfg(not(any(unix, windows)))]
fn hash_os_str(hasher: &mut Sha256, value: &OsStr) {
    hasher.update(value.to_string_lossy().as_bytes());
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

impl Drop for NamedStateLock {
    fn drop(&mut self) {
        // Closing the handle releases the OS lock even if this explicit call
        // fails. In particular, abnormal process termination closes it without
        // running Drop; correctness never depends on removing the lock file.
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    const CHILD_DATABASE_ENV: &str = "SPOCK_HOST_NAMED_STATE_LOCK_CHILD_DATABASE";
    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "spock-host-{label}-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn lock_path_is_deterministic_and_disjoint_from_database_names() {
        let root = temp_root("path-shape");
        std::fs::create_dir(&root).expect("temporary root");
        let sqlite = named_state_lock_path(&root.join("world.sqlite"));
        let canonical_root = std::fs::canonicalize(&root).expect("canonical temporary root");
        assert_eq!(
            sqlite.parent(),
            Some(canonical_root.join(LOCK_DIRECTORY).as_path())
        );
        assert_eq!(
            sqlite.file_name().and_then(OsStr::to_str).map(str::len),
            Some(64 + LOCK_SUFFIX.len())
        );
        assert!(sqlite
            .extension()
            .is_some_and(|extension| extension == "lock"));
        assert_ne!(sqlite, named_state_lock_path(&root.join("world.db")));
        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    #[test]
    fn lexical_aliases_share_one_lock_identity() {
        let root = temp_root("alias");
        std::fs::create_dir_all(root.join("nested")).expect("temporary tree");
        let direct = root.join("world.sqlite");
        let aliased = root.join("nested/../world.sqlite");

        let first = NamedStateLock::acquire(&direct).expect("first alias owner");
        let error = NamedStateLock::acquire(&aliased).expect_err("alias must contend");
        assert!(matches!(error, NamedStateLockError::Contended { .. }));
        assert_eq!(first.lock_path(), named_state_lock_path(&aliased));

        drop(first);
        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn multiply_hard_linked_database_entries_are_rejected() {
        let root = temp_root("hard-link-alias");
        std::fs::create_dir(&root).expect("temporary root");
        let database = root.join("world.sqlite");
        let alias = root.join("alias.sqlite");
        std::fs::write(&database, b"existing database").expect("database fixture");
        std::fs::hard_link(&database, &alias).expect("hard-link alias");

        for path in [&database, &alias] {
            let error = NamedStateLock::acquire(path).expect_err("hard links must be rejected");
            assert!(matches!(
                error,
                NamedStateLockError::HardLinkedDatabase {
                    path: rejected,
                    links
                } if rejected == *path && links >= 2
            ));
        }
        assert!(!root.join(LOCK_DIRECTORY).exists());

        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn single_link_existing_database_remains_lockable() {
        let root = temp_root("single-link");
        std::fs::create_dir(&root).expect("temporary root");
        let database = root.join("world.sqlite");
        std::fs::write(&database, b"existing database").expect("database fixture");

        let owner = NamedStateLock::acquire(&database).expect("single-link database owner");
        assert_eq!(
            owner.resolved_database_path(),
            std::fs::canonicalize(&root)
                .expect("canonical temporary root")
                .join("world.sqlite")
        );

        drop(owner);
        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_final_component_symlink_cannot_change_lock_identity() {
        use std::os::unix::fs::symlink;

        let root = temp_root("final-symlink");
        let real_parent = root.join("real");
        let parent_alias = root.join("parent-alias");
        std::fs::create_dir_all(&real_parent).expect("temporary tree");
        symlink(&real_parent, &parent_alias).expect("parent alias");

        let target = root.join("target.sqlite");
        std::fs::write(&target, b"target").expect("symlink target");
        let database = parent_alias.join("link.sqlite");
        symlink(&target, &database).expect("database symlink");

        let first = NamedStateLock::acquire(&database).expect("first owner");
        let expected_database = std::fs::canonicalize(&real_parent)
            .expect("canonical parent")
            .join("link.sqlite");
        assert_eq!(first.resolved_database_path(), expected_database);

        // Engine bootstrap removes this directory entry, not its target. A
        // later owner using the same spelling must still address one lock.
        std::fs::remove_file(&database).expect("remove final symlink");
        std::fs::write(&database, b"replacement").expect("replacement database");
        assert!(matches!(
            NamedStateLock::acquire(&database),
            Err(NamedStateLockError::Contended { .. })
        ));
        assert_eq!(std::fs::read(&target).expect("target survives"), b"target");

        drop(first);
        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    #[cfg(any(windows, target_os = "macos"))]
    #[test]
    fn case_and_normalization_aliases_share_one_conservative_lock_identity() {
        let root = temp_root("case-alias");
        std::fs::create_dir(&root).expect("temporary root");

        for (left, right) in [
            ("World.sqlite", "world.sqlite"),
            ("caf\u{e9}.sqlite", "cafe\u{301}.sqlite"),
            ("\u{3c3}.sqlite", "\u{3c2}.sqlite"),
        ] {
            assert_eq!(
                named_state_lock_path(&root.join(left)),
                named_state_lock_path(&root.join(right)),
                "{left} and {right} must share one lock identity"
            );
        }

        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    #[test]
    fn database_named_like_legacy_lock_cannot_replace_live_lock() {
        let root = temp_root("legacy-suffix");
        std::fs::create_dir(&root).expect("temporary root");
        let database = root.join("world.sqlite");
        let legacy_lock_named_database = root.join("world.sqlite.spock.lock");

        let first = NamedStateLock::acquire(&database).expect("first owner");
        let legacy_named = NamedStateLock::acquire(&legacy_lock_named_database)
            .expect("legacy suffix is an independent database identity");
        assert_ne!(first.lock_path(), legacy_named.lock_path());
        assert_ne!(first.lock_path(), legacy_lock_named_database);

        let contract = spock_lang::compile("").expect("empty contract");
        let connection =
            spock_runtime::engine::open(&contract, Some(&legacy_lock_named_database), None)
                .expect("materialize database with historical lock suffix");
        assert!(first.lock_path().is_file());
        assert!(matches!(
            NamedStateLock::acquire(&database),
            Err(NamedStateLockError::Contended { .. })
        ));

        drop(connection);
        drop(legacy_named);
        drop(first);
        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    #[test]
    fn actual_lock_object_cannot_be_reopened_as_a_database() {
        let root = temp_root("reserved-namespace");
        std::fs::create_dir(&root).expect("temporary root");
        let database = root.join("world.sqlite");

        let first = NamedStateLock::acquire(&database).expect("first owner");
        let lock_object = first.lock_path().to_path_buf();
        let error = NamedStateLock::acquire(&lock_object)
            .expect_err("the lock namespace must never accept database paths");
        assert!(matches!(
            error,
            NamedStateLockError::ReservedDatabasePath { path } if path == lock_object
        ));
        assert!(matches!(
            NamedStateLock::acquire(&database),
            Err(NamedStateLockError::Contended { .. })
        ));

        drop(first);
        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    #[test]
    fn exclusive_lock_is_nonblocking_and_released_on_drop() {
        let root = temp_root("drop-release");
        let database = root.join("nested/world.sqlite");
        let expected_lock_path = named_state_lock_path(&database);

        let first = NamedStateLock::acquire(&database).expect("first owner");
        assert_eq!(first.database_path(), database);
        assert_eq!(
            first.resolved_database_path(),
            std::fs::canonicalize(root.join("nested"))
                .expect("canonical database parent")
                .join("world.sqlite")
        );
        assert_eq!(first.lock_path(), expected_lock_path);
        assert!(expected_lock_path.is_file());
        assert!(
            !database.exists(),
            "locking must precede database bootstrap"
        );

        assert!(matches!(
            NamedStateLock::acquire(&database),
            Err(NamedStateLockError::Contended { .. })
        ));

        drop(first);
        let later = NamedStateLock::acquire(&database).expect("ownership after drop");
        drop(later);
        assert!(
            expected_lock_path.exists(),
            "the lock file is not an ownership sentinel"
        );

        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }

    /// Helper selected explicitly by `killed_subprocess_releases_os_lock`.
    /// The ordinary test-harness invocation is a no-op.
    #[test]
    fn lock_holder_subprocess() {
        let Some(database) = std::env::var_os(CHILD_DATABASE_ENV) else {
            return;
        };
        let _lock = NamedStateLock::acquire(PathBuf::from(database)).expect("child lock");
        println!("SPOCK_HOST_LOCK_ACQUIRED");
        std::io::stdout().flush().expect("flush lock handshake");

        // Keep the lock live until the parent terminates this process. The
        // pipe avoids sleeps and makes the crash-release test deterministic.
        let mut release = String::new();
        std::io::stdin()
            .read_line(&mut release)
            .expect("read parent release");
    }

    #[test]
    fn killed_subprocess_releases_os_lock() {
        let root = temp_root("process-release");
        let database = root.join("world.sqlite");
        let current_test = std::env::current_exe().expect("current test executable");
        let mut child = Command::new(current_test)
            .args([
                "--exact",
                "named_state::tests::lock_holder_subprocess",
                "--nocapture",
            ])
            .env(CHILD_DATABASE_ENV, &database)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn lock holder");

        let stdout = child.stdout.take().expect("child stdout");
        let mut acquired = false;
        for line in BufReader::new(stdout).lines() {
            let line = line.expect("child output");
            if line.contains("SPOCK_HOST_LOCK_ACQUIRED") {
                acquired = true;
                break;
            }
        }
        assert!(acquired, "child did not report lock acquisition");
        assert!(matches!(
            NamedStateLock::acquire(&database),
            Err(NamedStateLockError::Contended { .. })
        ));

        child.kill().expect("terminate lock holder without Drop");
        let status = child.wait().expect("reap lock holder");
        assert!(!status.success());

        let recovered =
            NamedStateLock::acquire(&database).expect("OS released lock after process death");
        drop(recovered);
        std::fs::remove_dir_all(root).expect("remove temporary tree");
    }
}
