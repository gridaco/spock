//! Race-safe filesystem application for [`spock_project::WritePlan`].
//!
//! Planning remains mutation-free in `spock-project`. This module owns the
//! imperative boundary used by `spock new` and `spock init`: it creates every
//! file with `create_new`, treats `spock.toml` as the final commit marker, and
//! rolls back only paths created by the current invocation.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use spock_project::{PlanKind, WritePlan, MANIFEST_FILE};

static NEXT_QUARANTINE: AtomicU64 = AtomicU64::new(0);

/// Filesystem policy for the root of a write plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootPolicy {
    /// `spock new`: the destination itself must not exist.
    NewDestination,
    /// `spock init`: the adoption root must already be a real directory.
    ExistingAdoptionRoot,
}

impl RootPolicy {
    fn accepts(self, kind: PlanKind) -> bool {
        matches!(
            (self, kind),
            (Self::NewDestination, PlanKind::Scaffold)
                | (Self::ExistingAdoptionRoot, PlanKind::Adopt)
        )
    }
}

/// The operation that failed while applying a plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplyStage {
    ValidatePolicy,
    ValidateRoot,
    CreateRoot,
    CreateDirectory,
    CreateFile,
    RecordOwnership,
    WriteFile,
}

impl fmt::Display for ApplyStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let description = match self {
            Self::ValidatePolicy => "validate write-plan policy",
            Self::ValidateRoot => "validate project root",
            Self::CreateRoot => "create project root",
            Self::CreateDirectory => "create project directory",
            Self::CreateFile => "create project file",
            Self::RecordOwnership => "record project path ownership",
            Self::WriteFile => "write project file",
        };
        f.write_str(description)
    }
}

/// Kind of invocation-created path considered during rollback.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreatedPathKind {
    File,
    Directory,
}

/// One invocation-created path that could not be removed during rollback.
#[derive(Debug)]
pub struct RollbackResidual {
    path: PathBuf,
    kind: CreatedPathKind,
    error: io::Error,
}

impl RollbackResidual {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn kind(&self) -> CreatedPathKind {
        self.kind
    }

    pub fn error(&self) -> &io::Error {
        &self.error
    }
}

/// Result of the best-effort rollback performed after an apply failure.
#[derive(Debug, Default)]
pub struct RollbackReport {
    residuals: Vec<RollbackResidual>,
}

impl RollbackReport {
    /// True when every invocation-created path is gone.
    pub fn is_complete(&self) -> bool {
        self.residuals.is_empty()
    }

    /// Invocation-created paths that still exist or could not be inspected.
    pub fn residuals(&self) -> &[RollbackResidual] {
        &self.residuals
    }
}

/// Successful filesystem effects of one plan application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplySummary {
    root: PathBuf,
    created_files: Vec<PathBuf>,
    created_directories: Vec<PathBuf>,
}

impl ApplySummary {
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Files in creation order. `spock.toml` is always last.
    pub fn created_files(&self) -> &[PathBuf] {
        &self.created_files
    }

    /// Directories in parent-before-child creation order.
    pub fn created_directories(&self) -> &[PathBuf] {
        &self.created_directories
    }
}

/// A failed plan application together with its rollback outcome.
#[derive(Debug)]
pub struct ApplyError {
    stage: ApplyStage,
    path: PathBuf,
    source: io::Error,
    rollback: RollbackReport,
}

impl ApplyError {
    pub fn stage(&self) -> ApplyStage {
        self.stage
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn io_error(&self) -> &io::Error {
        &self.source
    }

    pub fn rollback(&self) -> &RollbackReport {
        &self.rollback
    }
}

impl fmt::Display for ApplyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "could not {} `{}`: {}",
            self.stage,
            self.path.display(),
            self.source
        )?;
        if !self.rollback.is_complete() {
            write!(f, "; rollback left")?;
            for residual in self.rollback.residuals() {
                write!(f, " `{}` ({})", residual.path().display(), residual.error())?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ApplyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Apply a pure project write plan without overwriting any existing file.
///
/// Non-manifest files are created first. `spock.toml` is created last so its
/// presence means all preceding template writes succeeded. A later conflict
/// triggers a best-effort rollback consisting exclusively of non-recursive
/// removals for paths this invocation actually created.
pub fn apply_write_plan(
    plan: &WritePlan,
    root_policy: RootPolicy,
) -> Result<ApplySummary, ApplyError> {
    apply_write_plan_inner(plan, root_policy, |_| {})
}

fn apply_write_plan_inner<F>(
    plan: &WritePlan,
    root_policy: RootPolicy,
    mut after_write: F,
) -> Result<ApplySummary, ApplyError>
where
    F: FnMut(&Path),
{
    let mut journal = CreationJournal::default();

    if !root_policy.accepts(plan.kind) {
        let source = io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "root policy {root_policy:?} does not accept a {:?} plan",
                plan.kind
            ),
        );
        return Err(failed(
            ApplyStage::ValidatePolicy,
            &plan.root,
            source,
            journal,
        ));
    }

    let root_result = match root_policy {
        RootPolicy::NewDestination => create_new_root(&plan.root, &mut journal),
        RootPolicy::ExistingAdoptionRoot => validate_existing_root(&plan.root),
    };
    if let Err(failure) = root_result {
        return Err(failed(failure.stage, failure.path, failure.error, journal));
    }
    let root = match PinnedRoot::open(&plan.root) {
        Ok(root) => root,
        Err(error) => {
            return Err(failed(ApplyStage::ValidateRoot, &plan.root, error, journal));
        }
    };

    let mut writes = plan.writes().iter().collect::<Vec<_>>();
    writes.sort_by(|left, right| {
        let left_is_manifest = left.relative_path.as_str() == MANIFEST_FILE;
        let right_is_manifest = right.relative_path.as_str() == MANIFEST_FILE;
        left_is_manifest
            .cmp(&right_is_manifest)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });

    for write in writes {
        let relative_parent = write
            .relative_path
            .as_path()
            .parent()
            .expect("a planned file always has a parent");
        let parent = match ensure_relative_directories(&root, relative_parent, &mut journal) {
            Ok(parent) => parent,
            Err(failure) => {
                return Err(failed(failure.stage, failure.path, failure.error, journal));
            }
        };

        let destination = plan.root.join(write.relative_path.as_path());
        let file_name = write
            .relative_path
            .as_path()
            .file_name()
            .expect("a planned file has a file name");
        let (mut file, anchor) = match parent.create_new_file(file_name) {
            Ok(created) => created,
            Err(error) => {
                return Err(failed(ApplyStage::CreateFile, destination, error, journal));
            }
        };
        // Retain an open handle until success or rollback. Comparing a current
        // directory entry to this live file object avoids inode/file-index
        // reuse after an editor atomically replaces the path.
        let identity = match file.metadata() {
            Ok(metadata) => entry_identity(&metadata),
            Err(error) => {
                journal
                    .files
                    .push(JournalEntry::file(destination.clone(), file, None, anchor));
                return Err(failed(
                    ApplyStage::RecordOwnership,
                    destination,
                    error,
                    journal,
                ));
            }
        };

        if let Err(error) = file.write_all(&write.contents) {
            journal.files.push(JournalEntry::file(
                destination.clone(),
                file,
                identity,
                anchor,
            ));
            return Err(failed(ApplyStage::WriteFile, destination, error, journal));
        }
        journal.files.push(JournalEntry::file(
            destination.clone(),
            file,
            identity,
            anchor,
        ));
        after_write(&destination);
    }

    if let Err(error) = root.validate() {
        return Err(failed(ApplyStage::ValidateRoot, &plan.root, error, journal));
    }

    let CreationJournal { files, directories } = journal;
    Ok(ApplySummary {
        root: plan.root.clone(),
        created_files: files.into_iter().map(|entry| entry.path).collect(),
        created_directories: directories.into_iter().map(|entry| entry.path).collect(),
    })
}

#[derive(Debug)]
struct OperationFailure {
    stage: ApplyStage,
    path: PathBuf,
    error: io::Error,
}

fn create_new_root(root: &Path, journal: &mut CreationJournal) -> Result<(), OperationFailure> {
    match fs::symlink_metadata(root) {
        Ok(_) => {
            return Err(OperationFailure {
                stage: ApplyStage::CreateRoot,
                path: root.to_path_buf(),
                error: io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "new project destination already exists",
                ),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(OperationFailure {
                stage: ApplyStage::CreateRoot,
                path: root.to_path_buf(),
                error,
            });
        }
    }

    if let Some(parent) = nonempty_parent(root) {
        ensure_directory_tree(parent, journal)?;
    }

    fs::create_dir(root).map_err(|error| OperationFailure {
        stage: ApplyStage::CreateRoot,
        path: root.to_path_buf(),
        error,
    })?;
    record_created_directory(root, journal)
}

fn validate_existing_root(root: &Path) -> Result<(), OperationFailure> {
    let metadata = fs::symlink_metadata(root).map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: root.to_path_buf(),
        error,
    })?;
    if !metadata.file_type().is_dir() {
        return Err(OperationFailure {
            stage: ApplyStage::ValidateRoot,
            path: root.to_path_buf(),
            error: io::Error::new(
                io::ErrorKind::NotADirectory,
                "adoption root is not a real directory",
            ),
        });
    }
    Ok(())
}

fn nonempty_parent(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

/// Ensure a possibly-outside-root ancestor path for a new destination.
/// Existing ancestors terminate recursion; only directories created while
/// unwinding are journaled.
fn ensure_directory_tree(
    directory: &Path,
    journal: &mut CreationJournal,
) -> Result<(), OperationFailure> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => return require_real_directory(directory, &metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(OperationFailure {
                stage: ApplyStage::CreateDirectory,
                path: directory.to_path_buf(),
                error,
            });
        }
    }

    if let Some(parent) = nonempty_parent(directory) {
        ensure_directory_tree(parent, journal)?;
    }
    create_directory_if_missing(directory, journal)
}

#[cfg(unix)]
#[derive(Debug)]
struct PinnedRoot {
    directory: fs::File,
    identity: EntryIdentity,
    path: PathBuf,
}

#[cfg(unix)]
impl PinnedRoot {
    fn open(path: &Path) -> io::Result<Self> {
        let directory = open_directory_path(path)?;
        let identity = entry_identity(&directory.metadata()?).expect("Unix directory identity");
        let root = Self {
            directory,
            identity,
            path: path.to_path_buf(),
        };
        root.validate()?;
        Ok(root)
    }

    fn validate(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.file_type().is_dir()
            || entry_identity(&metadata) != Some(self.identity)
            || entry_identity(&self.directory.metadata()?) != Some(self.identity)
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "project root identity changed while applying the write plan",
            ));
        }
        Ok(())
    }

    fn cursor(&self) -> io::Result<DirectoryCursor> {
        self.validate()?;
        Ok(DirectoryCursor {
            directory: self.directory.try_clone()?,
            path: self.path.clone(),
        })
    }
}

#[cfg(unix)]
#[derive(Debug)]
struct DirectoryCursor {
    directory: fs::File,
    path: PathBuf,
}

#[cfg(unix)]
impl DirectoryCursor {
    fn create_new_file(&self, name: &OsStr) -> io::Result<(fs::File, Option<PathAnchor>)> {
        let descriptor = rustix::fs::openat(
            &self.directory,
            name,
            rustix::fs::OFlags::WRONLY
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::EXCL
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o666),
        )?;
        let file = fs::File::from(descriptor);
        let anchor = PathAnchor {
            parent: self.directory.try_clone()?,
            name: name.to_os_string(),
        };
        Ok((file, Some(anchor)))
    }
}

#[cfg(not(unix))]
#[derive(Debug)]
struct PinnedRoot {
    directory: fs::File,
    identity: Option<EntryIdentity>,
    path: PathBuf,
}

#[cfg(not(unix))]
impl PinnedRoot {
    fn open(path: &Path) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "project root is not a real directory",
            ));
        }
        let directory = open_directory_path(path)?;
        let identity = entry_identity(&directory.metadata()?);
        let root = Self {
            directory,
            identity,
            path: path.to_path_buf(),
        };
        root.validate()?;
        Ok(root)
    }

    fn validate(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.file_type().is_dir()
            || self.identity.is_none()
            || entry_identity(&metadata) != self.identity
            || entry_identity(&self.directory.metadata()?) != self.identity
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "project root identity changed while applying the write plan",
            ));
        }
        Ok(())
    }

    fn cursor(&self) -> io::Result<DirectoryCursor> {
        self.validate()?;
        Ok(DirectoryCursor {
            root: self.directory.try_clone()?,
            root_identity: self.identity,
            root_path: self.path.clone(),
            path: self.path.clone(),
        })
    }
}

#[cfg(not(unix))]
#[derive(Debug)]
struct DirectoryCursor {
    root: fs::File,
    root_identity: Option<EntryIdentity>,
    root_path: PathBuf,
    path: PathBuf,
}

#[cfg(not(unix))]
impl DirectoryCursor {
    fn validate_root(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.root_path)?;
        if !metadata.file_type().is_dir()
            || self.root_identity.is_none()
            || entry_identity(&metadata) != self.root_identity
            || entry_identity(&self.root.metadata()?) != self.root_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "project root identity changed while applying the write plan",
            ));
        }
        Ok(())
    }

    fn create_new_file(&self, name: &OsStr) -> io::Result<(fs::File, Option<PathAnchor>)> {
        self.validate_root()?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path.join(name))?;
        self.validate_root()?;
        Ok((file, None))
    }
}

#[cfg(unix)]
fn ensure_relative_directories(
    root: &PinnedRoot,
    relative: &Path,
    journal: &mut CreationJournal,
) -> Result<DirectoryCursor, OperationFailure> {
    let mut current = root.cursor().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: root.path.clone(),
        error,
    })?;
    for component in relative.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(invalid_planned_parent(&root.path, relative));
        };
        let path = current.path.join(segment);
        match open_directory_at(&current.directory, segment) {
            Ok(directory) => current = DirectoryCursor { directory, path },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let created = match create_directory_at(&current.directory, segment) {
                    Ok(()) => true,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                    Err(error) => {
                        return Err(OperationFailure {
                            stage: ApplyStage::CreateDirectory,
                            path,
                            error,
                        });
                    }
                };
                let directory =
                    open_directory_at(&current.directory, segment).map_err(|error| {
                        OperationFailure {
                            stage: ApplyStage::CreateDirectory,
                            path: path.clone(),
                            error,
                        }
                    })?;
                if created {
                    let metadata = directory.metadata().map_err(|error| OperationFailure {
                        stage: ApplyStage::RecordOwnership,
                        path: path.clone(),
                        error,
                    })?;
                    journal.directories.push(JournalEntry::directory(
                        path.clone(),
                        &metadata,
                        Some(PathAnchor {
                            parent: current.directory.try_clone().map_err(|error| {
                                OperationFailure {
                                    stage: ApplyStage::RecordOwnership,
                                    path: path.clone(),
                                    error,
                                }
                            })?,
                            name: segment.to_os_string(),
                        }),
                    ));
                }
                current = DirectoryCursor { directory, path };
            }
            Err(error) => {
                return Err(OperationFailure {
                    stage: ApplyStage::CreateDirectory,
                    path,
                    error,
                });
            }
        }
    }
    Ok(current)
}

#[cfg(not(unix))]
fn ensure_relative_directories(
    root: &PinnedRoot,
    relative: &Path,
    journal: &mut CreationJournal,
) -> Result<DirectoryCursor, OperationFailure> {
    let mut current = root.cursor().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: root.path.clone(),
        error,
    })?;
    for component in relative.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(invalid_planned_parent(&root.path, relative));
        };
        current.validate_root().map_err(|error| OperationFailure {
            stage: ApplyStage::ValidateRoot,
            path: root.path.clone(),
            error,
        })?;
        current.path.push(segment);
        match fs::symlink_metadata(&current.path) {
            Ok(metadata) => require_real_directory(&current.path, &metadata)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                create_directory_if_missing(&current.path, journal)?;
            }
            Err(error) => {
                return Err(OperationFailure {
                    stage: ApplyStage::CreateDirectory,
                    path: current.path,
                    error,
                });
            }
        }
    }
    Ok(current)
}

fn invalid_planned_parent(root: &Path, relative: &Path) -> OperationFailure {
    OperationFailure {
        stage: ApplyStage::CreateDirectory,
        path: root.join(relative),
        error: io::Error::new(
            io::ErrorKind::InvalidInput,
            "planned parent is not a normalized relative path",
        ),
    }
}

fn create_directory_if_missing(
    directory: &Path,
    journal: &mut CreationJournal,
) -> Result<(), OperationFailure> {
    match fs::create_dir(directory) {
        Ok(()) => record_created_directory(directory, journal),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            // Another creator won the race. Accept only a real directory and
            // do not journal it: this invocation does not own that path.
            let metadata = fs::symlink_metadata(directory).map_err(|error| OperationFailure {
                stage: ApplyStage::CreateDirectory,
                path: directory.to_path_buf(),
                error,
            })?;
            require_real_directory(directory, &metadata)
        }
        Err(error) => Err(OperationFailure {
            stage: ApplyStage::CreateDirectory,
            path: directory.to_path_buf(),
            error,
        }),
    }
}

fn require_real_directory(
    directory: &Path,
    metadata: &fs::Metadata,
) -> Result<(), OperationFailure> {
    if metadata.file_type().is_dir() {
        Ok(())
    } else {
        Err(OperationFailure {
            stage: ApplyStage::CreateDirectory,
            path: directory.to_path_buf(),
            error: io::Error::new(
                io::ErrorKind::NotADirectory,
                "path exists but is not a real directory",
            ),
        })
    }
}

#[cfg(unix)]
fn open_directory_path(path: &Path) -> io::Result<fs::File> {
    let descriptor = rustix::fs::openat(
        rustix::fs::CWD,
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    Ok(fs::File::from(descriptor))
}

#[cfg(windows)]
fn open_directory_path(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(not(any(unix, windows)))]
fn open_directory_path(path: &Path) -> io::Result<fs::File> {
    fs::File::open(path)
}

#[cfg(unix)]
fn open_directory_at(parent: &fs::File, name: &OsStr) -> io::Result<fs::File> {
    let descriptor = rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?;
    Ok(fs::File::from(descriptor))
}

#[cfg(unix)]
fn create_directory_at(parent: &fs::File, name: &OsStr) -> io::Result<()> {
    Ok(rustix::fs::mkdirat(
        parent,
        name,
        rustix::fs::Mode::from_raw_mode(0o777),
    )?)
}

#[cfg(unix)]
fn create_quarantine_at(parent: &fs::File) -> io::Result<(OsString, fs::File)> {
    loop {
        let name = quarantine_name();
        match rustix::fs::mkdirat(parent, &name, rustix::fs::Mode::from_raw_mode(0o700)) {
            Ok(()) => {
                return open_directory_at(parent, &name).map(|directory| (name, directory));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(unix)]
fn rename_at(
    from_parent: &fs::File,
    from: &OsStr,
    to_parent: &fs::File,
    to: &OsStr,
) -> io::Result<()> {
    Ok(rustix::fs::renameat(from_parent, from, to_parent, to)?)
}

#[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
fn rename_noreplace_at(
    from_parent: &fs::File,
    from: &OsStr,
    to_parent: &fs::File,
    to: &OsStr,
) -> io::Result<()> {
    Ok(rustix::fs::renameat_with(
        from_parent,
        from,
        to_parent,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )?)
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android", target_os = "macos"))
))]
fn rename_noreplace_at(
    _from_parent: &fs::File,
    _from: &OsStr,
    _to_parent: &fs::File,
    _to: &OsStr,
) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic no-replace restore is unavailable on this Unix platform",
    ))
}

#[cfg(unix)]
fn identity_at(parent: &fs::File, name: &OsStr) -> io::Result<EntryIdentity> {
    let metadata = rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
    Ok(EntryIdentity {
        volume: stat_identity_part(metadata.st_dev)?,
        file: stat_identity_part(metadata.st_ino)?,
    })
}

#[cfg(unix)]
fn stat_identity_part<T>(value: T) -> io::Result<u64>
where
    T: TryInto<u64>,
    T::Error: fmt::Display,
{
    value.try_into().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("filesystem identity does not fit u64: {error}"),
        )
    })
}

#[cfg(unix)]
fn unlink_at(parent: &fs::File, name: &OsStr, kind: CreatedPathKind) -> io::Result<()> {
    let flags = match kind {
        CreatedPathKind::File => rustix::fs::AtFlags::empty(),
        CreatedPathKind::Directory => rustix::fs::AtFlags::REMOVEDIR,
    };
    Ok(rustix::fs::unlinkat(parent, name, flags)?)
}

#[cfg(unix)]
fn remove_quarantine_at(parent: &fs::File, name: &OsStr) {
    let _ = unlink_at(parent, name, CreatedPathKind::Directory);
}

#[derive(Debug)]
struct PathAnchor {
    #[cfg(unix)]
    parent: fs::File,
    #[cfg(unix)]
    name: OsString,
}

#[cfg(unix)]
fn path_anchor(path: &Path) -> io::Result<PathAnchor> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "created path has no parent"))?;
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "created path has no file name")
    })?;
    Ok(PathAnchor {
        parent: open_directory_path(parent)?,
        name: name.to_os_string(),
    })
}

#[cfg(not(unix))]
fn path_anchor(_path: &Path) -> io::Result<PathAnchor> {
    Ok(PathAnchor {})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EntryIdentity {
    volume: u64,
    file: u64,
}

#[cfg(unix)]
fn entry_identity(metadata: &fs::Metadata) -> Option<EntryIdentity> {
    use std::os::unix::fs::MetadataExt;

    Some(EntryIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(windows)]
fn entry_identity(metadata: &fs::Metadata) -> Option<EntryIdentity> {
    use std::os::windows::fs::MetadataExt;

    Some(EntryIdentity {
        volume: u64::from(metadata.volume_serial_number()?),
        file: metadata.file_index()?,
    })
}

#[cfg(not(any(unix, windows)))]
fn entry_identity(_metadata: &fs::Metadata) -> Option<EntryIdentity> {
    None
}

#[derive(Debug)]
struct JournalEntry {
    path: PathBuf,
    identity: Option<EntryIdentity>,
    file: Option<fs::File>,
    anchor: Option<PathAnchor>,
}

impl JournalEntry {
    fn directory(path: PathBuf, metadata: &fs::Metadata, anchor: Option<PathAnchor>) -> Self {
        Self {
            path,
            identity: entry_identity(metadata),
            file: None,
            anchor,
        }
    }

    fn file(
        path: PathBuf,
        file: fs::File,
        identity: Option<EntryIdentity>,
        anchor: Option<PathAnchor>,
    ) -> Self {
        Self {
            path,
            identity,
            file: Some(file),
            anchor,
        }
    }

    fn directory_without_identity(path: PathBuf, anchor: Option<PathAnchor>) -> Self {
        Self {
            path,
            identity: None,
            file: None,
            anchor,
        }
    }

    fn expected_identity(&self) -> io::Result<Option<EntryIdentity>> {
        match &self.file {
            Some(file) => file.metadata().map(|metadata| entry_identity(&metadata)),
            None => Ok(self.identity),
        }
    }
}

fn record_created_directory(
    directory: &Path,
    journal: &mut CreationJournal,
) -> Result<(), OperationFailure> {
    let anchor = path_anchor(directory).map_err(|error| {
        journal
            .directories
            .push(JournalEntry::directory_without_identity(
                directory.to_path_buf(),
                None,
            ));
        OperationFailure {
            stage: ApplyStage::RecordOwnership,
            path: directory.to_path_buf(),
            error,
        }
    })?;
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            journal.directories.push(JournalEntry::directory(
                directory.to_path_buf(),
                &metadata,
                Some(anchor),
            ));
            Ok(())
        }
        Err(error) => {
            journal
                .directories
                .push(JournalEntry::directory_without_identity(
                    directory.to_path_buf(),
                    Some(anchor),
                ));
            Err(OperationFailure {
                stage: ApplyStage::RecordOwnership,
                path: directory.to_path_buf(),
                error,
            })
        }
    }
}

#[derive(Debug, Default)]
struct CreationJournal {
    files: Vec<JournalEntry>,
    directories: Vec<JournalEntry>,
}

fn failed(
    stage: ApplyStage,
    path: impl Into<PathBuf>,
    source: io::Error,
    journal: CreationJournal,
) -> ApplyError {
    ApplyError {
        stage,
        path: path.into(),
        source,
        rollback: journal.rollback(),
    }
}

impl CreationJournal {
    fn rollback(self) -> RollbackReport {
        let mut residuals = Vec::new();

        for entry in self.files.into_iter().rev() {
            rollback_entry(entry, CreatedPathKind::File, &mut residuals);
        }
        for entry in self.directories.into_iter().rev() {
            rollback_entry(entry, CreatedPathKind::Directory, &mut residuals);
        }

        RollbackReport { residuals }
    }
}

fn rollback_entry(
    entry: JournalEntry,
    kind: CreatedPathKind,
    residuals: &mut Vec<RollbackResidual>,
) {
    let expected_identity = match entry.expected_identity() {
        Ok(identity) => identity,
        Err(error) => {
            residuals.push(RollbackResidual {
                path: entry.path,
                kind,
                error,
            });
            return;
        }
    };
    if expected_identity.is_none() {
        residuals.push(RollbackResidual {
            path: entry.path,
            kind,
            error: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "created path has no stable identity; preserved it during rollback",
            ),
        });
        return;
    }

    #[cfg(unix)]
    if entry.anchor.is_some() {
        rollback_anchored_unix(entry, kind, expected_identity, residuals);
        return;
    }

    rollback_quarantined_path(entry, kind, expected_identity, residuals);
}

fn quarantine_name() -> OsString {
    OsString::from(format!(
        ".spock-rollback-{}-{}",
        std::process::id(),
        NEXT_QUARANTINE.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(unix)]
fn rollback_anchored_unix(
    entry: JournalEntry,
    kind: CreatedPathKind,
    expected_identity: Option<EntryIdentity>,
    residuals: &mut Vec<RollbackResidual>,
) {
    let anchor = entry.anchor.as_ref().expect("checked anchored entry");
    let (quarantine_name, quarantine) = match create_quarantine_at(&anchor.parent) {
        Ok(quarantine) => quarantine,
        Err(error) => {
            residuals.push(RollbackResidual {
                path: entry.path,
                kind,
                error,
            });
            return;
        }
    };
    let quarantine_entry = OsStr::new("entry");
    let quarantine_path = entry
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&quarantine_name)
        .join(quarantine_entry);

    if let Err(error) = rename_at(&anchor.parent, &anchor.name, &quarantine, quarantine_entry) {
        remove_quarantine_at(&anchor.parent, &quarantine_name);
        if error.kind() != io::ErrorKind::NotFound {
            residuals.push(RollbackResidual {
                path: entry.path,
                kind,
                error,
            });
        }
        return;
    }

    let current_identity = identity_at(&quarantine, quarantine_entry);
    if current_identity
        .as_ref()
        .is_ok_and(|identity| Some(*identity) == expected_identity)
    {
        let removal = unlink_at(&quarantine, quarantine_entry, kind);
        if let Err(error) = removal {
            match rename_noreplace_at(
                &quarantine,
                quarantine_entry,
                &anchor.parent,
                &anchor.name,
            ) {
                Ok(()) => {
                    remove_quarantine_at(&anchor.parent, &quarantine_name);
                    residuals.push(RollbackResidual {
                        path: entry.path,
                        kind,
                        error,
                    });
                }
                Err(restore_error) => residuals.push(RollbackResidual {
                    path: quarantine_path,
                    kind,
                    error: io::Error::new(
                        restore_error.kind(),
                        format!(
                            "could not remove invocation-owned path or restore it from rollback quarantine ({error}; {restore_error})"
                        ),
                    ),
                }),
            }
            return;
        }
        remove_quarantine_at(&anchor.parent, &quarantine_name);
        return;
    }

    let identity_error = current_identity.err();
    match rename_noreplace_at(&quarantine, quarantine_entry, &anchor.parent, &anchor.name) {
        Ok(()) => {
            remove_quarantine_at(&anchor.parent, &quarantine_name);
            residuals.push(RollbackResidual {
                path: entry.path,
                kind,
                error: identity_error.unwrap_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "path identity changed after creation; restored concurrent replacement",
                    )
                }),
            });
        }
        Err(restore_error) => {
            residuals.push(RollbackResidual {
                path: quarantine_path,
                kind,
                error: io::Error::new(
                    restore_error.kind(),
                    format!(
                        "path identity changed; preserved replacement in rollback quarantine ({restore_error})"
                    ),
                ),
            });
        }
    }
}

fn rollback_quarantined_path(
    entry: JournalEntry,
    kind: CreatedPathKind,
    expected_identity: Option<EntryIdentity>,
    residuals: &mut Vec<RollbackResidual>,
) {
    let parent = entry.path.parent().unwrap_or_else(|| Path::new("."));
    let (quarantine_path, quarantined_entry) = loop {
        let quarantine_path = parent.join(quarantine_name());
        match fs::create_dir(&quarantine_path) {
            Ok(()) => break (quarantine_path.clone(), quarantine_path.join("entry")),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                residuals.push(RollbackResidual {
                    path: entry.path,
                    kind,
                    error,
                });
                return;
            }
        }
    };

    if let Err(error) = fs::rename(&entry.path, &quarantined_entry) {
        let _ = fs::remove_dir(&quarantine_path);
        if error.kind() != io::ErrorKind::NotFound {
            residuals.push(RollbackResidual {
                path: entry.path,
                kind,
                error,
            });
        }
        return;
    }

    let current_identity = fs::symlink_metadata(&quarantined_entry)
        .ok()
        .and_then(|metadata| entry_identity(&metadata));
    if current_identity == expected_identity {
        let removal = match kind {
            CreatedPathKind::File => fs::remove_file(&quarantined_entry),
            CreatedPathKind::Directory => fs::remove_dir(&quarantined_entry),
        };
        if let Err(error) = removal {
            #[cfg(windows)]
            if fs::rename(&quarantined_entry, &entry.path).is_ok() {
                let _ = fs::remove_dir(&quarantine_path);
                residuals.push(RollbackResidual {
                    path: entry.path,
                    kind,
                    error,
                });
                return;
            }
            residuals.push(RollbackResidual {
                path: quarantined_entry,
                kind,
                error,
            });
            return;
        }
        let _ = fs::remove_dir(quarantine_path);
        return;
    }

    // `rename` does not replace a destination on Windows. Other non-Unix
    // targets are intentionally conservative: if restoration cannot be proven
    // exclusive, leave the replacement in quarantine and report its location.
    #[cfg(windows)]
    if fs::rename(&quarantined_entry, &entry.path).is_ok() {
        let _ = fs::remove_dir(&quarantine_path);
        residuals.push(RollbackResidual {
            path: entry.path,
            kind,
            error: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "path identity changed after creation; restored concurrent replacement",
            ),
        });
        return;
    }

    residuals.push(RollbackResidual {
        path: quarantined_entry,
        kind,
        error: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "path identity changed; preserved replacement in rollback quarantine",
        ),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use spock_project::{adoption_plan, scaffold_plan, ProjectInventory};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            loop {
                let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir()
                    .join(format!("spock-write-plan-{}-{id}", std::process::id()));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("could not create test directory: {error}"),
                }
            }
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn applies_scaffold_with_manifest_last_and_reports_exact_effects() {
        let temporary = TestDirectory::new();
        let destination = temporary.path().join("demo");
        let plan = scaffold_plan(&destination, "demo", None).unwrap();

        let summary = apply_write_plan(&plan, RootPolicy::NewDestination).unwrap();

        assert_eq!(summary.root(), destination);
        assert_eq!(
            summary.created_files(),
            [
                destination.join("backend/app.spock"),
                destination.join(MANIFEST_FILE),
            ]
        );
        assert_eq!(
            summary.created_directories(),
            [destination.clone(), destination.join("backend")]
        );
        assert!(destination.join("backend/app.spock").is_file());
        assert!(destination.join(MANIFEST_FILE).is_file());
    }

    #[test]
    fn new_destination_may_create_and_reports_missing_parent_directories() {
        let temporary = TestDirectory::new();
        let parent = temporary.path().join("nested");
        let destination = parent.join("demo");
        let plan = scaffold_plan(&destination, "demo", None).unwrap();

        let summary = apply_write_plan(&plan, RootPolicy::NewDestination).unwrap();

        assert_eq!(
            summary.created_directories(),
            [parent, destination.clone(), destination.join("backend"),]
        );
    }

    #[test]
    fn existing_new_destination_is_never_modified() {
        let temporary = TestDirectory::new();
        let destination = temporary.path().join("demo");
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("owned.txt"), "keep").unwrap();
        let plan = scaffold_plan(&destination, "demo", None).unwrap();

        let error = apply_write_plan(&plan, RootPolicy::NewDestination).unwrap_err();

        assert_eq!(error.stage(), ApplyStage::CreateRoot);
        assert_eq!(error.io_error().kind(), io::ErrorKind::AlreadyExists);
        assert!(error.rollback().is_complete());
        assert_eq!(
            fs::read_to_string(destination.join("owned.txt")).unwrap(),
            "keep"
        );
        assert!(!destination.join(MANIFEST_FILE).exists());
    }

    #[test]
    fn adoption_conflict_rolls_back_created_files_but_preserves_racer_file() {
        let temporary = TestDirectory::new();
        let inventory = ProjectInventory::scan(temporary.path()).unwrap();
        let plan = adoption_plan(&inventory, Some("demo")).unwrap();
        let racer_manifest = plan.root.join(MANIFEST_FILE);
        fs::write(&racer_manifest, "racer-owned\n").unwrap();

        let error = apply_write_plan(&plan, RootPolicy::ExistingAdoptionRoot).unwrap_err();

        assert_eq!(error.stage(), ApplyStage::CreateFile);
        assert_eq!(error.path(), racer_manifest);
        assert_eq!(error.io_error().kind(), io::ErrorKind::AlreadyExists);
        assert!(error.rollback().is_complete());
        assert_eq!(fs::read_to_string(racer_manifest).unwrap(), "racer-owned\n");
        assert!(!plan.root.join("backend/app.spock").exists());
        assert!(!plan.root.join("backend").exists());
    }

    #[test]
    fn rollback_preserves_a_file_replaced_after_this_invocation_created_it() {
        let temporary = TestDirectory::new();
        let inventory = ProjectInventory::scan(temporary.path()).unwrap();
        let plan = adoption_plan(&inventory, Some("demo")).unwrap();
        fs::write(plan.root.join(MANIFEST_FILE), "racer-owned\n").unwrap();
        let backend = plan.root.join("backend/app.spock");
        let moved_invocation_file = temporary.path().join("invocation-file-moved-away");

        let error = apply_write_plan_inner(&plan, RootPolicy::ExistingAdoptionRoot, |written| {
            if written == backend {
                fs::rename(written, &moved_invocation_file).unwrap();
                fs::write(written, "replacement-owned-by-another-writer\n").unwrap();
            }
        })
        .unwrap_err();

        assert_eq!(
            fs::read_to_string(&backend).unwrap(),
            "replacement-owned-by-another-writer\n"
        );
        assert!(moved_invocation_file.is_file());
        assert!(error.rollback().residuals().iter().any(|residual| {
            residual.kind() == CreatedPathKind::File && residual.path() == backend
        }));
    }

    #[test]
    fn rollback_reports_a_directory_that_gained_concurrent_content() {
        let temporary = TestDirectory::new();
        let inventory = ProjectInventory::scan(temporary.path()).unwrap();
        let plan = adoption_plan(&inventory, Some("demo")).unwrap();
        fs::write(plan.root.join(MANIFEST_FILE), "racer-owned\n").unwrap();
        let concurrent_file = plan.root.join("backend/concurrent.txt");

        let error = apply_write_plan_inner(&plan, RootPolicy::ExistingAdoptionRoot, |written| {
            if written.ends_with("backend/app.spock") {
                fs::write(&concurrent_file, "keep").unwrap();
            }
        })
        .unwrap_err();

        assert_eq!(error.rollback().residuals().len(), 1);
        let residual = &error.rollback().residuals()[0];
        assert_eq!(residual.kind(), CreatedPathKind::Directory);
        assert_eq!(residual.path(), plan.root.join("backend"));
        assert_eq!(fs::read_to_string(concurrent_file).unwrap(), "keep");
        assert!(!plan.root.join("backend/app.spock").exists());
    }

    #[cfg(unix)]
    #[test]
    fn writes_remain_confined_to_the_pinned_root_after_path_replacement() {
        use std::os::unix::fs::symlink;

        let temporary = TestDirectory::new();
        let temporary_root = fs::canonicalize(temporary.path()).unwrap();
        let project = temporary_root.join("project");
        let moved_project = temporary_root.join("moved-project");
        let replacement_target = temporary_root.join("replacement-target");
        fs::create_dir(&project).unwrap();
        fs::create_dir(&replacement_target).unwrap();
        let inventory = ProjectInventory::scan(&project).unwrap();
        let plan = adoption_plan(&inventory, Some("demo")).unwrap();
        let error = apply_write_plan_inner(&plan, RootPolicy::ExistingAdoptionRoot, |written| {
            if written.ends_with(MANIFEST_FILE) {
                fs::rename(&project, &moved_project).unwrap();
                symlink(&replacement_target, &project).unwrap();
            }
        })
        .unwrap_err();

        assert_eq!(error.stage(), ApplyStage::ValidateRoot);
        assert!(error.rollback().is_complete());
        assert!(!moved_project.join("backend/app.spock").exists());
        assert!(!moved_project.join(MANIFEST_FILE).exists());
        assert!(!moved_project.join("backend").exists());
        assert!(fs::read_dir(&replacement_target).unwrap().next().is_none());
    }

    #[test]
    fn root_policy_mismatch_fails_before_mutation() {
        let temporary = TestDirectory::new();
        let destination = temporary.path().join("demo");
        let plan = scaffold_plan(&destination, "demo", None).unwrap();

        let error = apply_write_plan(&plan, RootPolicy::ExistingAdoptionRoot).unwrap_err();

        assert_eq!(error.stage(), ApplyStage::ValidatePolicy);
        assert_eq!(error.io_error().kind(), io::ErrorKind::InvalidInput);
        assert!(error.rollback().is_complete());
        assert!(!destination.exists());
    }
}
