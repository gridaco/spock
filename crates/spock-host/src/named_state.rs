//! Process ownership for an explicitly named database world.
//!
//! The lock file is only the stable object on which the operating system holds
//! an advisory lock. Its contents and existence carry no ownership meaning:
//! there is no PID record to inspect and no stale sentinel to delete.

use std::ffi::OsString;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use thiserror::Error;

const LOCK_SUFFIX: &str = ".spock.lock";

/// Derive the advisory-lock path for one named `--db` value.
///
/// The suffix is appended rather than replacing the extension, so
/// `world.db` and `world.sqlite` cannot accidentally share a lock. Appending
/// through `OsString` also preserves paths that are not UTF-8.
#[must_use]
pub fn named_state_lock_path(database_path: &Path) -> PathBuf {
    let mut lock_path = OsString::from(database_path.as_os_str());
    lock_path.push(LOCK_SUFFIX);
    PathBuf::from(lock_path)
}

/// Failures before a host owns its named database world.
#[derive(Debug, Error)]
pub enum NamedStateLockError {
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
        let lock_path = named_state_lock_path(&database_path);

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

    #[must_use]
    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
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
    fn lock_path_is_deterministic_and_extension_preserving() {
        assert_eq!(
            named_state_lock_path(Path::new("state/world.sqlite")),
            PathBuf::from("state/world.sqlite.spock.lock")
        );
        assert_ne!(
            named_state_lock_path(Path::new("state/world.sqlite")),
            named_state_lock_path(Path::new("state/world.db"))
        );
    }

    #[test]
    fn exclusive_lock_is_nonblocking_and_released_on_drop() {
        let root = temp_root("drop-release");
        let database = root.join("nested/world.sqlite");
        let expected_lock_path = named_state_lock_path(&database);

        let first = NamedStateLock::acquire(&database).expect("first owner");
        assert_eq!(first.database_path(), database);
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
