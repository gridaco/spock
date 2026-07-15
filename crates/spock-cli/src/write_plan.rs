//! Race-safe filesystem application for [`spock_project::WritePlan`].
//!
//! Planning remains mutation-free in `spock-project`. This module owns the
//! imperative boundary used by `spock new` and `spock init`: it creates every
//! file with `create_new`, treats `spock.toml` as the final commit marker, and
//! handles failure without touching pre-existing paths. Unix rollback removes
//! exact invocation-owned files through retained parent handles but preserves
//! created directories because `mkdir` cannot atomically return an ownership
//! handle. Windows deliberately preserves and reports every known creation
//! because the available rename APIs cannot safely restore without replacement.

#[cfg(windows)]
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
#[cfg(not(unix))]
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
#[cfg(not(windows))]
use std::sync::atomic::{AtomicU64, Ordering};

use spock_project::{
    is_ignored_inventory_directory, Diagnostic, DiagnosticCode, Diagnostics, InventoryEntryKind,
    NormalizedRelativePath, PlanKind, ProjectInventory, ProjectResult, WritePlan, MANIFEST_FILE,
};

#[cfg(not(windows))]
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

/// A live filesystem capability retained between project inventory and apply.
///
/// The public project commands use this lease so planning and mutation cannot
/// silently select different directories at the same pathname. On Windows the
/// retained directory handle also denies delete sharing, preventing rename or
/// replacement for the lifetime of the lease.
#[derive(Debug)]
pub(crate) struct PreparedWriteRoot(PinnedRoot);

impl PreparedWriteRoot {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        PinnedRoot::open(path).map(Self)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0.path
    }

    pub(crate) fn validate(&self) -> io::Result<()> {
        self.0.validate()
    }

    /// Inventory exactly the retained directory used by the later apply.
    pub(crate) fn inventory(&self) -> ProjectResult<ProjectInventory> {
        scan_prepared_inventory(&self.0)
    }
}

fn inventory_io(
    root: &Path,
    relative: &Path,
    action: &str,
    error: impl fmt::Display,
) -> Diagnostics {
    Diagnostics::one(
        Diagnostic::new(DiagnosticCode::Io, format!("could not {action}: {error}"))
            .at_path(root.join(relative)),
    )
}

fn normalized_inventory_path(
    root: &Path,
    relative: &Path,
) -> ProjectResult<NormalizedRelativePath> {
    let mut segments = Vec::new();
    for component in relative.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(Diagnostic::new(
                DiagnosticCode::InvalidManifestPath,
                "scanned path cannot be represented in a project manifest",
            )
            .at_path(root.join(relative))
            .into());
        };
        let Some(segment) = segment.to_str() else {
            return Err(Diagnostic::new(
                DiagnosticCode::InvalidManifestPath,
                "scanned path is not UTF-8 and cannot be represented in a project manifest",
            )
            .at_path(root.join(relative))
            .into());
        };
        segments.push(segment);
    }
    NormalizedRelativePath::file(&segments.join("/")).map_err(|error| {
        Diagnostics::one(
            Diagnostic::new(
                DiagnosticCode::InvalidManifestPath,
                format!("scanned path is not a valid project path: {error}"),
            )
            .at_path(root.join(relative)),
        )
    })
}

#[cfg(unix)]
fn scan_prepared_inventory(root: &PinnedRoot) -> ProjectResult<ProjectInventory> {
    let mut entries = Vec::new();
    scan_unix_inventory_directory(&root.directory, Path::new(""), &root.path, &mut entries)?;
    ProjectInventory::from_entries(root.path.clone(), entries)
}

#[cfg(unix)]
fn scan_unix_inventory_directory(
    directory: &fs::File,
    relative_parent: &Path,
    root: &Path,
    entries: &mut Vec<(NormalizedRelativePath, InventoryEntryKind)>,
) -> ProjectResult<()> {
    use std::os::unix::ffi::OsStrExt;

    let reader = rustix::fs::Dir::read_from(directory)
        .map_err(|error| inventory_io(root, relative_parent, "scan directory", error))?;
    let mut names = reader
        .map(|entry| {
            let entry = entry.map_err(|error| {
                inventory_io(root, relative_parent, "scan directory entry", error)
            })?;
            Ok(OsStr::from_bytes(entry.file_name().to_bytes()).to_os_string())
        })
        .collect::<ProjectResult<Vec<_>>>()?;
    names.retain(|name| name != "." && name != "..");
    names.sort();

    for name in names {
        let relative = relative_parent.join(&name);
        let metadata = rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|error| inventory_io(root, &relative, "inspect directory entry", error))?;
        let kind = match rustix::fs::FileType::from_raw_mode(metadata.st_mode) {
            rustix::fs::FileType::Directory => InventoryEntryKind::Directory,
            rustix::fs::FileType::Symlink => InventoryEntryKind::Symlink,
            _ => InventoryEntryKind::File,
        };
        let normalized = normalized_inventory_path(root, &relative)?;
        let ignored = kind == InventoryEntryKind::Directory
            && normalized
                .file_name()
                .is_some_and(is_ignored_inventory_directory);
        entries.push((normalized, kind));
        if kind == InventoryEntryKind::Directory && !ignored {
            let child = open_directory_at(directory, &name)
                .map_err(|error| inventory_io(root, &relative, "open directory", error))?;
            scan_unix_inventory_directory(&child, &relative, root, entries)?;
        }
    }
    Ok(())
}

#[cfg(windows)]
fn scan_prepared_inventory(root: &PinnedRoot) -> ProjectResult<ProjectInventory> {
    let mut entries = Vec::new();
    scan_windows_inventory_directory(&root.directory, Path::new(""), &root.path, &mut entries)?;
    ProjectInventory::from_entries(root.path.clone(), entries)
}

#[cfg(windows)]
fn scan_windows_inventory_directory(
    directory: &cap_std::fs::Dir,
    relative_parent: &Path,
    root: &Path,
    entries: &mut Vec<(NormalizedRelativePath, InventoryEntryKind)>,
) -> ProjectResult<()> {
    use cap_fs_ext::DirExt as _;

    let reader = directory
        .entries()
        .map_err(|error| inventory_io(root, relative_parent, "scan directory", error))?;
    let mut children = reader
        .map(|entry| {
            let entry = entry.map_err(|error| {
                inventory_io(root, relative_parent, "scan directory entry", error)
            })?;
            let name = entry.file_name();
            let kind = entry.file_type().map_err(|error| {
                inventory_io(
                    root,
                    &relative_parent.join(&name),
                    "inspect directory entry",
                    error,
                )
            })?;
            Ok((name, kind))
        })
        .collect::<ProjectResult<Vec<_>>>()?;
    children.sort_by(|left, right| left.0.cmp(&right.0));

    for (name, file_type) in children {
        let relative = relative_parent.join(&name);
        let kind = if file_type.is_symlink() {
            InventoryEntryKind::Symlink
        } else if file_type.is_dir() {
            InventoryEntryKind::Directory
        } else {
            InventoryEntryKind::File
        };
        let normalized = normalized_inventory_path(root, &relative)?;
        let ignored = kind == InventoryEntryKind::Directory
            && normalized
                .file_name()
                .is_some_and(is_ignored_inventory_directory);
        entries.push((normalized, kind));
        if kind == InventoryEntryKind::Directory && !ignored {
            let child = directory
                .open_dir_nofollow(&name)
                .map_err(|error| inventory_io(root, &relative, "open directory", error))?;
            scan_windows_inventory_directory(&child, &relative, root, entries)?;
        }
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn scan_prepared_inventory(root: &PinnedRoot) -> ProjectResult<ProjectInventory> {
    root.validate().map_err(|error| {
        inventory_io(&root.path, Path::new(""), "validate adoption root", error)
    })?;
    ProjectInventory::scan(&root.path)
}

#[derive(Debug)]
pub(crate) enum PreparedWriteTarget {
    NewChild {
        parent: PreparedWriteRoot,
        child: OsString,
    },
    Existing(PreparedWriteRoot),
}

impl PreparedWriteTarget {
    pub(crate) fn new_child(parent: PreparedWriteRoot, child: impl Into<OsString>) -> Self {
        Self::NewChild {
            parent,
            child: child.into(),
        }
    }

    pub(crate) fn existing(root: PreparedWriteRoot) -> Self {
        Self::Existing(root)
    }

    fn policy(&self) -> RootPolicy {
        match self {
            Self::NewChild { .. } => RootPolicy::NewDestination,
            Self::Existing(_) => RootPolicy::ExistingAdoptionRoot,
        }
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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
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
/// triggers a platform-specific best-effort rollback. Unix removes exact
/// invocation-owned files through retained parent handles and reports created
/// directories as residuals. Windows performs no rollback mutation and reports
/// every known creation as a residual. These conservative rules avoid deleting
/// or overwriting an entry concurrently installed under a created name.
pub fn apply_write_plan(
    plan: &WritePlan,
    root_policy: RootPolicy,
) -> Result<ApplySummary, ApplyError> {
    apply_write_plan_inner(plan, root_policy, |_| {})
}

pub(crate) fn apply_prepared_write_plan(
    plan: &WritePlan,
    target: PreparedWriteTarget,
) -> Result<ApplySummary, ApplyError> {
    let policy = target.policy();
    apply_write_plan_inner_with_target(plan, policy, Some(target), |_| {})
}

fn apply_write_plan_inner<F>(
    plan: &WritePlan,
    root_policy: RootPolicy,
    after_write: F,
) -> Result<ApplySummary, ApplyError>
where
    F: FnMut(&Path),
{
    apply_write_plan_inner_with_target(plan, root_policy, None, after_write)
}

fn apply_write_plan_inner_with_target<F>(
    plan: &WritePlan,
    root_policy: RootPolicy,
    prepared_target: Option<PreparedWriteTarget>,
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

    let root = match prepare_root(&plan.root, root_policy, prepared_target, &mut journal) {
        Ok(root) => root,
        Err(failure) => {
            return Err(failed(failure.stage, failure.path, failure.error, journal));
        }
    };
    let mut directory_cache = DirectoryCache::new();

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
        let parent = match ensure_relative_directories(
            &root,
            relative_parent,
            &mut journal,
            &mut directory_cache,
        ) {
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
        let identity = match journal_identity_from_file(&file) {
            Ok(identity) => identity,
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

fn prepare_root(
    root: &Path,
    root_policy: RootPolicy,
    prepared_target: Option<PreparedWriteTarget>,
    journal: &mut CreationJournal,
) -> Result<PinnedRoot, OperationFailure> {
    if let Some(prepared_target) = prepared_target {
        return match prepared_target {
            PreparedWriteTarget::Existing(PreparedWriteRoot(prepared)) => {
                if root_policy != RootPolicy::ExistingAdoptionRoot || prepared.path != root {
                    return Err(prepared_target_mismatch(root));
                }
                prepared.validate().map_err(|error| OperationFailure {
                    stage: ApplyStage::ValidateRoot,
                    path: root.to_path_buf(),
                    error,
                })?;
                Ok(prepared)
            }
            PreparedWriteTarget::NewChild {
                parent: PreparedWriteRoot(parent),
                child,
            } => {
                let expected = parent.path.join(&child);
                let mut components = Path::new(&child).components();
                let is_normal_child = matches!(
                    (components.next(), components.next()),
                    (Some(std::path::Component::Normal(_)), None)
                );
                if root_policy != RootPolicy::NewDestination || expected != root || !is_normal_child
                {
                    return Err(prepared_target_mismatch(root));
                }
                create_new_root_from_prepared_parent(root, &child, parent, journal)
            }
        };
    }

    match root_policy {
        RootPolicy::NewDestination => create_new_root(root, journal),
        RootPolicy::ExistingAdoptionRoot => {
            validate_existing_root(root)?;
            PinnedRoot::open(root).map_err(|error| OperationFailure {
                stage: ApplyStage::ValidateRoot,
                path: root.to_path_buf(),
                error,
            })
        }
    }
}

fn prepared_target_mismatch(root: &Path) -> OperationFailure {
    OperationFailure {
        stage: ApplyStage::ValidatePolicy,
        path: root.to_path_buf(),
        error: io::Error::new(
            io::ErrorKind::InvalidInput,
            "prepared filesystem target does not match the write plan root and kind",
        ),
    }
}

#[cfg(windows)]
fn create_new_root_from_prepared_parent(
    root: &Path,
    child: &OsStr,
    parent: PinnedRoot,
    journal: &mut CreationJournal,
) -> Result<PinnedRoot, OperationFailure> {
    parent.validate().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: parent.path.clone(),
        error,
    })?;
    let created = create_windows_directory_at(&parent.directory, child).map_err(|error| {
        OperationFailure {
            stage: ApplyStage::CreateRoot,
            path: root.to_path_buf(),
            error,
        }
    })?;
    let directory = retain_windows_created_directory(
        &parent.directory,
        child,
        root.to_path_buf(),
        created,
        journal,
    )?;
    let identity =
        windows_identity_from_directory(&directory).map_err(|error| OperationFailure {
            stage: ApplyStage::RecordOwnership,
            path: root.to_path_buf(),
            error,
        })?;
    let PinnedRoot {
        directory: parent_directory,
        mut ancestor_guards,
        ..
    } = parent;
    ancestor_guards.push(parent_directory);
    let pinned = PinnedRoot {
        directory,
        identity,
        path: root.to_path_buf(),
        ancestor_guards,
    };
    pinned.validate().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: root.to_path_buf(),
        error,
    })?;
    Ok(pinned)
}

#[cfg(unix)]
fn create_new_root_from_prepared_parent(
    root: &Path,
    child: &OsStr,
    parent: PinnedRoot,
    journal: &mut CreationJournal,
) -> Result<PinnedRoot, OperationFailure> {
    parent.validate().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: parent.path.clone(),
        error,
    })?;
    create_directory_at(&parent.directory, child).map_err(|error| OperationFailure {
        stage: ApplyStage::CreateRoot,
        path: root.to_path_buf(),
        error,
    })?;
    // Journal immediately after mkdirat. The retained child handle is opened
    // in a second syscall, so rollback must preserve this directory even when
    // that open or any following validation fails.
    journal
        .directories
        .push(JournalEntry::directory_without_identity(
            root.to_path_buf(),
            None,
        ));
    let directory =
        open_directory_at(&parent.directory, child).map_err(|error| OperationFailure {
            stage: ApplyStage::CreateRoot,
            path: root.to_path_buf(),
            error,
        })?;
    parent.validate().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: parent.path.clone(),
        error,
    })?;
    let metadata = directory.metadata().map_err(|error| OperationFailure {
        stage: ApplyStage::RecordOwnership,
        path: root.to_path_buf(),
        error,
    })?;
    let identity = entry_identity(&metadata).expect("Unix directory identity");
    let pinned = PinnedRoot {
        directory,
        identity,
        path: root.to_path_buf(),
    };
    pinned.validate().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: root.to_path_buf(),
        error,
    })?;
    Ok(pinned)
}

#[cfg(not(any(unix, windows)))]
fn create_new_root_from_prepared_parent(
    root: &Path,
    _child: &OsStr,
    parent: PinnedRoot,
    journal: &mut CreationJournal,
) -> Result<PinnedRoot, OperationFailure> {
    parent.validate().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: parent.path.clone(),
        error,
    })?;
    let result = create_new_root(root, journal);
    parent.validate().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: parent.path.clone(),
        error,
    })?;
    result
}

#[cfg(not(windows))]
fn create_new_root(
    root: &Path,
    journal: &mut CreationJournal,
) -> Result<PinnedRoot, OperationFailure> {
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
    record_created_directory(root, journal)?;
    PinnedRoot::open(root).map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: root.to_path_buf(),
        error,
    })
}

#[cfg(windows)]
fn create_new_root(
    root: &Path,
    journal: &mut CreationJournal,
) -> Result<PinnedRoot, OperationFailure> {
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

    // Walk upward without mutating until one real ancestor can be pinned.
    // Every missing component is then created relative to the retained handle;
    // the newly created root handle itself becomes `PinnedRoot` and is never
    // reopened through its mutable pathname.
    let mut missing = Vec::<(PathBuf, OsString)>::new();
    let mut existing = root.to_path_buf();
    loop {
        match fs::symlink_metadata(&existing) {
            Ok(metadata) => {
                require_real_directory(&existing, &metadata)?;
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let name = existing.file_name().ok_or_else(|| OperationFailure {
                    stage: ApplyStage::CreateRoot,
                    path: root.to_path_buf(),
                    error: io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "new project destination has no creatable path component",
                    ),
                })?;
                missing.push((existing.clone(), name.to_os_string()));
                existing = existing
                    .parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."));
            }
            Err(error) => {
                return Err(OperationFailure {
                    stage: ApplyStage::CreateDirectory,
                    path: existing,
                    error,
                });
            }
        }
    }

    let pinned_ancestor = PinnedRoot::open(&existing).map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: existing.clone(),
        error,
    })?;
    let PinnedRoot {
        directory: mut current,
        mut ancestor_guards,
        ..
    } = pinned_ancestor;

    let missing_count = missing.len();
    for (index, (path, name)) in missing.into_iter().rev().enumerate() {
        ancestor_guards.push(current.try_clone().map_err(|error| OperationFailure {
            stage: ApplyStage::RecordOwnership,
            path: path.clone(),
            error,
        })?);
        let created = match create_windows_directory_at(&current, &name) {
            Ok(created) => created,
            Err(error)
                if error.kind() == io::ErrorKind::AlreadyExists && index + 1 != missing_count =>
            {
                use cap_fs_ext::DirExt as _;

                current = current
                    .open_dir_nofollow(&name)
                    .map_err(|error| OperationFailure {
                        stage: ApplyStage::CreateDirectory,
                        path,
                        error,
                    })?;
                continue;
            }
            Err(error) => {
                return Err(OperationFailure {
                    stage: if index + 1 == missing_count {
                        ApplyStage::CreateRoot
                    } else {
                        ApplyStage::CreateDirectory
                    },
                    path,
                    error,
                });
            }
        };
        current = retain_windows_created_directory(&current, &name, path, created, journal)?;
    }

    let identity = windows_identity_from_directory(&current).map_err(|error| OperationFailure {
        stage: ApplyStage::RecordOwnership,
        path: root.to_path_buf(),
        error,
    })?;
    let pinned = PinnedRoot {
        directory: current,
        identity,
        path: root.to_path_buf(),
        ancestor_guards,
    };
    pinned.validate().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: root.to_path_buf(),
        error,
    })?;
    Ok(pinned)
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

#[cfg(not(windows))]
fn nonempty_parent(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

/// Ensure a possibly-outside-root ancestor path for a new destination.
/// Existing ancestors terminate recursion; only directories created while
/// unwinding are journaled.
#[cfg(not(windows))]
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

#[cfg(not(windows))]
#[derive(Debug)]
struct DirectoryCache;

#[cfg(not(windows))]
impl DirectoryCache {
    fn new() -> Self {
        Self
    }
}

#[cfg(windows)]
#[derive(Debug, Default)]
struct DirectoryCache {
    directories: BTreeMap<PathBuf, cap_std::fs::Dir>,
}

#[cfg(windows)]
impl DirectoryCache {
    fn new() -> Self {
        Self::default()
    }
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
        // Duplicate the parent capability before the mutating openat. If the
        // process is out of descriptors, fail before creating a file that the
        // caller cannot journal through its retained parent anchor.
        let parent = self.directory.try_clone()?;
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
            parent,
            name: name.to_os_string(),
        };
        Ok((file, Some(anchor)))
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct PinnedRoot {
    directory: cap_std::fs::Dir,
    identity: EntryIdentity,
    path: PathBuf,
    // `spock new` may create a missing chain below the nearest existing
    // ancestor. Keep every ancestor locked until the plan commits so moving a
    // parent cannot carry the pinned project outside the requested pathname.
    ancestor_guards: Vec<cap_std::fs::Dir>,
}

#[cfg(windows)]
impl PinnedRoot {
    fn open(path: &Path) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "project root is not a real directory",
            ));
        }
        let directory = cap_std::fs::Dir::from_std_file(open_directory_path(path)?);
        let identity = windows_identity_from_directory(&directory)?;
        let root = Self {
            directory,
            identity,
            path: path.to_path_buf(),
            ancestor_guards: Vec::new(),
        };
        root.validate()?;
        Ok(root)
    }

    fn validate(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if !metadata.file_type().is_dir()
            || !windows_path_matches_identity(&self.path, &self.identity)?
            || windows_identity_from_directory(&self.directory)? != self.identity
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
        let root = self.directory.try_clone()?;
        Ok(DirectoryCursor {
            directory: root.try_clone()?,
            root_identity: windows_identity_from_directory(&root)?,
            root,
            root_path: self.path.clone(),
        })
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct DirectoryCursor {
    directory: cap_std::fs::Dir,
    root: cap_std::fs::Dir,
    root_identity: EntryIdentity,
    root_path: PathBuf,
}

#[cfg(windows)]
impl DirectoryCursor {
    fn validate_root(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.root_path)?;
        if !metadata.file_type().is_dir()
            || !windows_path_matches_identity(&self.root_path, &self.root_identity)?
            || windows_identity_from_directory(&self.root)? != self.root_identity
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "project root identity changed while applying the write plan",
            ));
        }
        Ok(())
    }

    fn create_new_file(&self, name: &OsStr) -> io::Result<(fs::File, Option<PathAnchor>)> {
        use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
        use cap_std::fs::OpenOptionsExt as _;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ, FILE_SHARE_WRITE,
        };

        self.validate_root()?;
        let mut options = cap_std::fs::OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No)
            // `create_new` already guarantees the final component did not
            // exist. Supplying the no-follow flag explicitly prevents
            // cap-std from performing a fallible metadata probe after the
            // successful create syscall, so the returned handle can always be
            // journaled by the caller before any later fallible operation.
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
        let file = self.directory.open_with(name, &options)?.into_std();
        Ok((file, None))
    }
}

#[cfg(not(any(unix, windows)))]
#[derive(Debug)]
struct PinnedRoot {
    directory: fs::File,
    identity: Option<EntryIdentity>,
    path: PathBuf,
}

#[cfg(not(any(unix, windows)))]
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

#[cfg(not(any(unix, windows)))]
#[derive(Debug)]
struct DirectoryCursor {
    root: fs::File,
    root_identity: Option<EntryIdentity>,
    root_path: PathBuf,
    path: PathBuf,
}

#[cfg(not(any(unix, windows)))]
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
    _cache: &mut DirectoryCache,
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
                if created {
                    // `mkdirat` has already mutated the filesystem and does
                    // not return a handle. Record the logical creation before
                    // any fallible open or metadata call. Directory rollback
                    // is deliberately non-mutating because ownership cannot be
                    // proven across this gap.
                    journal
                        .directories
                        .push(JournalEntry::directory_without_identity(path.clone(), None));
                }
                let directory =
                    open_directory_at(&current.directory, segment).map_err(|error| {
                        OperationFailure {
                            stage: ApplyStage::CreateDirectory,
                            path: path.clone(),
                            error,
                        }
                    })?;
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

#[cfg(windows)]
fn ensure_relative_directories(
    root: &PinnedRoot,
    relative: &Path,
    journal: &mut CreationJournal,
    cache: &mut DirectoryCache,
) -> Result<DirectoryCursor, OperationFailure> {
    use cap_fs_ext::DirExt as _;

    let mut current = root.cursor().map_err(|error| OperationFailure {
        stage: ApplyStage::ValidateRoot,
        path: root.path.clone(),
        error,
    })?;
    let mut relative_cursor = PathBuf::new();
    for component in relative.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(invalid_planned_parent(&root.path, relative));
        };
        relative_cursor.push(segment);
        let path = root.path.join(&relative_cursor);

        current.validate_root().map_err(|error| OperationFailure {
            stage: ApplyStage::ValidateRoot,
            path: root.path.clone(),
            error,
        })?;

        let directory = if let Some(cached) = cache.directories.get(&relative_cursor) {
            cached.try_clone().map_err(|error| OperationFailure {
                stage: ApplyStage::RecordOwnership,
                path: path.clone(),
                error,
            })?
        } else {
            let directory = match current.directory.open_dir_nofollow(segment) {
                Ok(directory) => directory,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    match create_windows_directory_at(&current.directory, segment) {
                        Ok(created) => retain_windows_created_directory(
                            &current.directory,
                            segment,
                            path.clone(),
                            created,
                            journal,
                        )?,
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => current
                            .directory
                            .open_dir_nofollow(segment)
                            .map_err(|error| OperationFailure {
                                stage: ApplyStage::CreateDirectory,
                                path: path.clone(),
                                error,
                            })?,
                        Err(error) => {
                            return Err(OperationFailure {
                                stage: ApplyStage::CreateDirectory,
                                path,
                                error,
                            });
                        }
                    }
                }
                Err(error) => {
                    return Err(OperationFailure {
                        stage: ApplyStage::CreateDirectory,
                        path,
                        error,
                    });
                }
            };
            cache.directories.insert(
                relative_cursor.clone(),
                directory.try_clone().map_err(|error| OperationFailure {
                    stage: ApplyStage::RecordOwnership,
                    path: path.clone(),
                    error,
                })?,
            );
            directory
        };
        current = DirectoryCursor {
            directory,
            root: current.root.try_clone().map_err(|error| OperationFailure {
                stage: ApplyStage::RecordOwnership,
                path: path.clone(),
                error,
            })?,
            root_identity: windows_identity_from_directory(&current.root).map_err(|error| {
                OperationFailure {
                    stage: ApplyStage::RecordOwnership,
                    path: path.clone(),
                    error,
                }
            })?,
            root_path: current.root_path.clone(),
        };
    }
    Ok(current)
}

#[cfg(not(any(unix, windows)))]
fn ensure_relative_directories(
    root: &PinnedRoot,
    relative: &Path,
    journal: &mut CreationJournal,
    _cache: &mut DirectoryCache,
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

#[cfg(not(windows))]
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
    open_windows_entry_path(path)
}

#[cfg(windows)]
fn open_windows_entry_path(path: &Path) -> io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};

    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    OpenOptions::new()
        .read(true)
        // A retained directory handle is a filesystem capability on Windows.
        // Denying delete sharing prevents the directory from being renamed or
        // removed while descendant operations are resolved through it.
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

#[cfg(windows)]
fn create_windows_directory_at(parent: &cap_std::fs::Dir, name: &OsStr) -> io::Result<fs::File> {
    use fs_at::os::windows::OpenOptionsExt as _;
    use windows_sys::Wdk::Storage::FileSystem::{FILE_DIRECTORY_FILE, FILE_OPEN_REPARSE_POINT};
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_TRAVERSE, FILE_WRITE_ATTRIBUTES,
    };

    let parent = parent.try_clone()?.into_std_file();
    let mut options = fs_at::OpenOptions::default();
    options
        .create_new(true)
        .desired_access(
            FILE_LIST_DIRECTORY | FILE_READ_ATTRIBUTES | FILE_TRAVERSE | FILE_WRITE_ATTRIBUTES,
        )
        // `mkdir_at` performs a second reparse probe after creating on
        // Windows. Requesting FILE_CREATE through `open_at` gives us the
        // created directory handle in one operation, so every mutation can be
        // journaled even if later validation fails.
        .create_options(FILE_DIRECTORY_FILE | FILE_OPEN_REPARSE_POINT);
    options.open_at(&parent, name)
}

#[cfg(windows)]
fn retain_windows_created_directory(
    parent: &cap_std::fs::Dir,
    name: &OsStr,
    path: PathBuf,
    created: fs::File,
    journal: &mut CreationJournal,
) -> Result<cap_std::fs::Dir, OperationFailure> {
    use cap_fs_ext::DirExt as _;

    let identity = match windows_identity_from_file(&created) {
        Ok(identity) => identity,
        Err(error) => {
            journal
                .directories
                .push(JournalEntry::directory_without_identity_with_live_file(
                    path.clone(),
                    None,
                    created,
                ));
            return Err(OperationFailure {
                stage: ApplyStage::RecordOwnership,
                path,
                error,
            });
        }
    };
    let locked = match parent.open_dir_nofollow(name) {
        Ok(locked) => locked,
        Err(error) => {
            journal
                .directories
                .push(JournalEntry::directory_with_live_file(
                    path.clone(),
                    Some(identity),
                    None,
                    created,
                ));
            return Err(OperationFailure {
                stage: ApplyStage::RecordOwnership,
                path,
                error,
            });
        }
    };
    let locked_identity = match windows_identity_from_directory(&locked) {
        Ok(identity) => identity,
        Err(error) => {
            journal
                .directories
                .push(JournalEntry::directory_with_live_file(
                    path.clone(),
                    Some(identity),
                    None,
                    created,
                ));
            return Err(OperationFailure {
                stage: ApplyStage::RecordOwnership,
                path,
                error,
            });
        }
    };
    if locked_identity != identity {
        journal
            .directories
            .push(JournalEntry::directory_with_live_file(
                path.clone(),
                Some(identity),
                None,
                created,
            ));
        return Err(OperationFailure {
            stage: ApplyStage::RecordOwnership,
            path,
            error: io::Error::new(
                io::ErrorKind::AlreadyExists,
                "created directory identity changed before it could be pinned",
            ),
        });
    }

    let live = match locked.try_clone().map(cap_std::fs::Dir::into_std_file) {
        Ok(live) => live,
        Err(error) => {
            journal
                .directories
                .push(JournalEntry::directory_with_live_file(
                    path.clone(),
                    Some(identity),
                    None,
                    created,
                ));
            return Err(OperationFailure {
                stage: ApplyStage::RecordOwnership,
                path,
                error,
            });
        }
    };
    journal
        .directories
        .push(JournalEntry::directory_with_live_file(
            path,
            Some(identity),
            None,
            live,
        ));
    Ok(locked)
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

#[cfg(not(any(unix, windows)))]
fn path_anchor(_path: &Path) -> io::Result<PathAnchor> {
    Ok(PathAnchor {})
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EntryIdentity {
    volume: u64,
    file: u64,
}

#[cfg(windows)]
type EntryIdentity = same_file::Handle;

#[cfg(not(any(unix, windows)))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct EntryIdentity;

#[cfg(unix)]
fn entry_identity(metadata: &fs::Metadata) -> Option<EntryIdentity> {
    use std::os::unix::fs::MetadataExt;

    Some(EntryIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(not(any(unix, windows)))]
fn entry_identity(_metadata: &fs::Metadata) -> Option<EntryIdentity> {
    None
}

#[cfg(unix)]
fn journal_identity_from_file(file: &fs::File) -> io::Result<Option<EntryIdentity>> {
    file.metadata().map(|metadata| entry_identity(&metadata))
}

#[cfg(windows)]
fn windows_identity_from_file(file: &fs::File) -> io::Result<EntryIdentity> {
    same_file::Handle::from_file(file.try_clone()?)
}

#[cfg(windows)]
fn windows_identity_from_directory(directory: &cap_std::fs::Dir) -> io::Result<EntryIdentity> {
    let file = directory.try_clone()?.into_std_file();
    windows_identity_from_file(&file)
}

#[cfg(windows)]
fn windows_file_matches_identity(file: &fs::File, expected: &EntryIdentity) -> io::Result<bool> {
    windows_identity_from_file(file).map(|current| &current == expected)
}

#[cfg(windows)]
fn windows_path_matches_identity(path: &Path, expected: &EntryIdentity) -> io::Result<bool> {
    let current = open_windows_entry_path(path)?;
    windows_file_matches_identity(&current, expected)
}

#[cfg(windows)]
fn journal_identity_from_file(file: &fs::File) -> io::Result<Option<EntryIdentity>> {
    windows_identity_from_file(file).map(Some)
}

#[cfg(not(any(unix, windows)))]
fn journal_identity_from_file(file: &fs::File) -> io::Result<Option<EntryIdentity>> {
    file.metadata().map(|_| None)
}

#[cfg(unix)]
fn journal_identity_from_directory(
    _path: &Path,
    metadata: &fs::Metadata,
) -> io::Result<Option<EntryIdentity>> {
    Ok(entry_identity(metadata))
}

#[cfg(not(any(unix, windows)))]
fn journal_identity_from_directory(
    _path: &Path,
    metadata: &fs::Metadata,
) -> io::Result<Option<EntryIdentity>> {
    Ok(entry_identity(metadata))
}

#[cfg(all(windows, test))]
fn identity_from_path(path: &Path) -> io::Result<Option<EntryIdentity>> {
    let entry = open_windows_entry_path(path)?;
    windows_identity_from_file(&entry).map(Some)
}

#[cfg(not(any(unix, windows)))]
fn identity_from_path(path: &Path) -> io::Result<Option<EntryIdentity>> {
    fs::symlink_metadata(path).map(|metadata| entry_identity(&metadata))
}

#[derive(Debug)]
struct JournalEntry {
    path: PathBuf,
    #[cfg_attr(windows, allow(dead_code))]
    identity: Option<EntryIdentity>,
    _live_file: Option<fs::File>,
    #[cfg_attr(windows, allow(dead_code))]
    anchor: Option<PathAnchor>,
}

impl JournalEntry {
    #[cfg(not(windows))]
    fn directory(
        path: PathBuf,
        identity: Option<EntryIdentity>,
        anchor: Option<PathAnchor>,
    ) -> Self {
        Self {
            path,
            identity,
            _live_file: None,
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
            _live_file: Some(file),
            anchor,
        }
    }

    #[cfg(not(windows))]
    fn directory_without_identity(path: PathBuf, anchor: Option<PathAnchor>) -> Self {
        Self {
            path,
            identity: None,
            _live_file: None,
            anchor,
        }
    }

    #[cfg(windows)]
    fn directory_with_live_file(
        path: PathBuf,
        identity: Option<EntryIdentity>,
        anchor: Option<PathAnchor>,
        live_file: fs::File,
    ) -> Self {
        Self {
            path,
            identity,
            _live_file: Some(live_file),
            anchor,
        }
    }

    #[cfg(windows)]
    fn directory_without_identity_with_live_file(
        path: PathBuf,
        anchor: Option<PathAnchor>,
        live_file: fs::File,
    ) -> Self {
        Self::directory_with_live_file(path, None, anchor, live_file)
    }
}

#[cfg(not(windows))]
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
        Ok(metadata) => match journal_identity_from_directory(directory, &metadata) {
            Ok(identity) => {
                journal.directories.push(JournalEntry::directory(
                    directory.to_path_buf(),
                    identity,
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
        },
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
    #[cfg(windows)]
    {
        // Windows has no safe, stable, no-replace restore primitive in the
        // APIs used by this crate. In particular, `std::fs::rename` may replace
        // its destination. Preserve every known creation and report it rather
        // than risk removing or overwriting a concurrent replacement. The
        // recorded path is the logical creation path; another process may have
        // moved the entry after its retained handle is released.
        residuals.push(RollbackResidual {
            path: entry.path,
            kind,
            error: io::Error::other(
                "Windows rollback is intentionally non-mutating; the created entry was preserved and its recorded path may no longer be current",
            ),
        });
    }

    #[cfg(not(windows))]
    if kind == CreatedPathKind::Directory {
        // `mkdir`/`mkdirat` does not return a live handle. A concurrent actor
        // can move the new directory and install another directory before we
        // open and record its identity. Never remove a directory based on that
        // post-create observation: the replacement may belong to somebody
        // else. The logical path is still useful for cleanup guidance, though
        // it may no longer name the invocation-created directory.
        residuals.push(RollbackResidual {
            path: entry.path,
            kind,
            error: io::Error::other(
                "directory rollback is intentionally non-mutating because directory creation cannot be atomically bound to an ownership handle; the recorded path may no longer be current",
            ),
        });
        return;
    }

    #[cfg(not(windows))]
    if entry.identity.is_none() {
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
        rollback_anchored_unix(entry, kind, residuals);
    } else {
        residuals.push(RollbackResidual {
            path: entry.path,
            kind,
            error: io::Error::other(
                "created path lost its retained parent anchor; preserved it during rollback",
            ),
        });
    }

    #[cfg(not(any(unix, windows)))]
    rollback_quarantined_path(entry, kind, residuals);
}

#[cfg(not(windows))]
fn quarantine_name() -> OsString {
    OsString::from(format!(
        ".spock-rollback-{}-{}",
        std::process::id(),
        NEXT_QUARANTINE.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(unix)]
fn move_to_quarantine_at(parent: &fs::File, source: &OsStr) -> io::Result<OsString> {
    loop {
        let quarantine = quarantine_name();
        match rename_noreplace_at(parent, source, parent, &quarantine) {
            Ok(()) => return Ok(quarantine),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(unix)]
fn rollback_anchored_unix(
    entry: JournalEntry,
    kind: CreatedPathKind,
    residuals: &mut Vec<RollbackResidual>,
) {
    let anchor = entry.anchor.as_ref().expect("checked anchored entry");
    let quarantine_name = match move_to_quarantine_at(&anchor.parent, &anchor.name) {
        Ok(quarantine) => quarantine,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return,
        Err(error) => {
            residuals.push(RollbackResidual {
                path: entry.path,
                kind,
                error,
            });
            return;
        }
    };
    let quarantine_path = entry
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(&quarantine_name);

    let current_identity = identity_at(&anchor.parent, &quarantine_name);
    if current_identity
        .as_ref()
        .is_ok_and(|identity| entry.identity.as_ref() == Some(identity))
    {
        let removal = unlink_at(&anchor.parent, &quarantine_name, kind);
        if let Err(error) = removal {
            match rename_noreplace_at(
                &anchor.parent,
                &quarantine_name,
                &anchor.parent,
                &anchor.name,
            ) {
                Ok(()) => {
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
        return;
    }

    let identity_error = current_identity.err();
    match rename_noreplace_at(
        &anchor.parent,
        &quarantine_name,
        &anchor.parent,
        &anchor.name,
    ) {
        Ok(()) => {
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

#[cfg(not(any(unix, windows)))]
fn rollback_quarantined_path(
    entry: JournalEntry,
    kind: CreatedPathKind,
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

    let current_identity = identity_from_path(&quarantined_entry).ok().flatten();
    if current_identity.as_ref() == entry.identity.as_ref() {
        let removal = match kind {
            CreatedPathKind::File => fs::remove_file(&quarantined_entry),
            CreatedPathKind::Directory => fs::remove_dir(&quarantined_entry),
        };
        if let Err(error) = removal {
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
        assert_eq!(fs::read_to_string(racer_manifest).unwrap(), "racer-owned\n");
        #[cfg(not(windows))]
        {
            assert_eq!(error.rollback().residuals().len(), 1);
            let residual = &error.rollback().residuals()[0];
            assert_eq!(residual.kind(), CreatedPathKind::Directory);
            assert_eq!(residual.path(), plan.root.join("backend"));
            assert!(!plan.root.join("backend/app.spock").exists());
            assert!(plan.root.join("backend").is_dir());
        }
        #[cfg(windows)]
        {
            let residuals = error
                .rollback()
                .residuals()
                .iter()
                .map(|residual| (residual.path().to_path_buf(), residual.kind()))
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                residuals,
                std::collections::BTreeSet::from([
                    (plan.root.join("backend/app.spock"), CreatedPathKind::File,),
                    (plan.root.join("backend"), CreatedPathKind::Directory),
                ])
            );
            assert!(plan.root.join("backend/app.spock").is_file());
            assert!(plan.root.join("backend").is_dir());
            assert!(fs::read_dir(&plan.root).unwrap().all(|entry| {
                !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".spock-rollback-")
            }));
        }
    }

    #[cfg(not(windows))]
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

    #[cfg(windows)]
    #[test]
    fn windows_live_identity_distinguishes_a_replacement_from_the_retained_file() {
        let temporary = TestDirectory::new();
        let original = temporary.path().join("created.txt");
        let moved = temporary.path().join("created-moved-away.txt");
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&original)
            .unwrap();
        let original_identity = journal_identity_from_file(&file).unwrap().unwrap();

        fs::rename(&original, &moved).unwrap();
        fs::write(&original, "replacement-owned-by-another-writer\n").unwrap();

        let replacement_identity = identity_from_path(&original).unwrap().unwrap();
        let moved_identity = identity_from_path(&moved).unwrap().unwrap();
        assert_ne!(original_identity, replacement_identity);
        assert_eq!(original_identity, moved_identity);
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

        assert_eq!(fs::read_to_string(concurrent_file).unwrap(), "keep");
        #[cfg(not(windows))]
        {
            assert_eq!(error.rollback().residuals().len(), 1);
            let residual = &error.rollback().residuals()[0];
            assert_eq!(residual.kind(), CreatedPathKind::Directory);
            assert_eq!(residual.path(), plan.root.join("backend"));
            assert!(!plan.root.join("backend/app.spock").exists());
        }
        #[cfg(windows)]
        {
            assert_eq!(error.rollback().residuals().len(), 2);
            assert!(plan.root.join("backend/app.spock").is_file());
        }
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
        assert_eq!(error.rollback().residuals().len(), 1);
        assert_eq!(
            error.rollback().residuals()[0].path(),
            project.join("backend")
        );
        assert!(!moved_project.join("backend/app.spock").exists());
        assert!(!moved_project.join(MANIFEST_FILE).exists());
        assert!(moved_project.join("backend").is_dir());
        assert!(fs::read_dir(&replacement_target).unwrap().next().is_none());
    }

    #[cfg(windows)]
    #[test]
    fn windows_pinned_root_blocks_replacement_until_the_lease_is_dropped() {
        let temporary = TestDirectory::new();
        let project = temporary.path().join("project");
        let moved_project = temporary.path().join("moved-project");
        fs::create_dir(&project).unwrap();
        let root = PinnedRoot::open(&project).unwrap();

        let error = fs::rename(&project, &moved_project).unwrap_err();
        assert!(matches!(
            error.kind(),
            io::ErrorKind::PermissionDenied | io::ErrorKind::Other
        ));
        assert!(project.is_dir());

        drop(root);
        fs::rename(&project, &moved_project).unwrap();
        fs::create_dir(&project).unwrap();
        assert!(fs::read_dir(&project).unwrap().next().is_none());
        assert!(moved_project.is_dir());
    }

    #[test]
    fn prepared_target_mismatch_fails_before_mutation() {
        let temporary = TestDirectory::new();
        let destination = temporary.path().join("demo");
        let plan = scaffold_plan(&destination, "demo", None).unwrap();
        let parent = PreparedWriteRoot::open(temporary.path()).unwrap();

        let error =
            apply_prepared_write_plan(&plan, PreparedWriteTarget::new_child(parent, "different"))
                .unwrap_err();

        assert_eq!(error.stage(), ApplyStage::ValidatePolicy);
        assert_eq!(error.io_error().kind(), io::ErrorKind::InvalidInput);
        assert!(error.rollback().is_complete());
        assert!(!destination.exists());
    }

    #[cfg(unix)]
    #[test]
    fn prepared_inventory_reads_the_retained_root_across_a_path_aba() {
        let temporary = TestDirectory::new();
        let project = temporary.path().join("project");
        let moved_project = temporary.path().join("moved-project");
        fs::create_dir(&project).unwrap();
        fs::write(project.join("original.spock"), "").unwrap();
        let prepared = PreparedWriteRoot::open(&project).unwrap();

        fs::rename(&project, &moved_project).unwrap();
        fs::create_dir(&project).unwrap();
        fs::write(project.join("foreign.spock"), "").unwrap();
        let inventory = prepared.inventory().unwrap();

        fs::remove_dir_all(&project).unwrap();
        fs::rename(&moved_project, &project).unwrap();
        prepared.validate().unwrap();
        let paths = inventory
            .entries()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, ["original.spock"]);
    }

    #[cfg(windows)]
    #[test]
    fn windows_prepared_parent_and_created_entries_stay_leased_through_commit() {
        let temporary = TestDirectory::new();
        let workspace = temporary.path().join("workspace");
        let moved_workspace = temporary.path().join("moved-workspace");
        fs::create_dir(&workspace).unwrap();
        let parent = PreparedWriteRoot::open(&workspace).unwrap();
        assert!(fs::rename(&workspace, &moved_workspace).is_err());

        let destination = workspace.join("demo");
        let moved_root = workspace.join("moved-demo");
        let moved_backend = destination.join("moved-backend");
        let moved_file = destination.join("backend/moved-app.spock");
        let backend = destination.join("backend");
        let backend_file = backend.join("app.spock");
        let plan = scaffold_plan(&destination, "demo", None).unwrap();
        let mut root_rename_blocked = false;
        let mut directory_rename_blocked = false;
        let mut file_rename_blocked = false;

        let summary = apply_write_plan_inner_with_target(
            &plan,
            RootPolicy::NewDestination,
            Some(PreparedWriteTarget::new_child(parent, "demo")),
            |written| {
                if written == backend_file {
                    root_rename_blocked = fs::rename(&destination, &moved_root).is_err();
                    directory_rename_blocked = fs::rename(&backend, &moved_backend).is_err();
                    file_rename_blocked = fs::rename(&backend_file, &moved_file).is_err();
                }
            },
        )
        .unwrap();

        assert!(root_rename_blocked);
        assert!(directory_rename_blocked);
        assert!(file_rename_blocked);
        assert_eq!(summary.root(), destination);
        assert!(backend_file.is_file());
        assert!(destination.join(MANIFEST_FILE).is_file());
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
