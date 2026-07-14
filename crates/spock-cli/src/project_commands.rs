//! Reusable command boundaries for framework-project operations.
//!
//! Target selection, checking, and filesystem mutation live here rather than
//! in the Clap presentation layer. An explicit `.spock` target keeps the
//! historical language-file check; every other accepted check target resolves
//! to a validated framework project.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use spock_host::{ProjectCheckFailure, ProjectCheckReport};
use spock_project::{
    adoption_plan, load_project, minimal_uhura_client_template, parse_manifest, resolve_target,
    scaffold_plan, Diagnostics, NormalizedRelativePath, ProjectInventory, ProjectLayout,
    ProjectName, ResolvedTarget, MANIFEST_FILE,
};
use thiserror::Error;

use crate::{
    apply_write_plan, full_load_check, ApplyError, CheckSummary, FileProgram, ProgramLoadError,
    RootPolicy,
};

/// A successful polymorphic `spock check` result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckTargetSummary {
    /// The historical one-file language/runtime proof.
    File {
        path: PathBuf,
        summary: CheckSummary,
    },
    /// A manifest, backend, client, and currently provable link check.
    Project {
        root: PathBuf,
        project_name: String,
        report: ProjectCheckReport,
    },
}

impl CheckTargetSummary {
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::File { path, .. } => path,
            Self::Project { root, .. } => root,
        }
    }
}

impl fmt::Display for CheckTargetSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { summary, .. } => summary.fmt(formatter),
            Self::Project {
                project_name,
                report,
                ..
            } => {
                write!(
                    formatter,
                    "ok: project `{project_name}` — {} table(s), {} record(s), {} fn(s), {} seed row(s)",
                    report.backend.tables,
                    report.backend.records,
                    report.backend.functions,
                    report.backend.seed_rows,
                )?;
                match &report.client {
                    Some(client) => write!(
                        formatter,
                        ", {} preview(s), {} replay-derived preview(s)",
                        client.preview_count, client.replay_derived_count,
                    )?,
                    None => formatter.write_str(", backend only")?,
                }
                write!(
                    formatter,
                    ", {} unchecked link(s), {} warning(s)",
                    report.unchecked_links,
                    report.warnings.len(),
                )
            }
        }
    }
}

/// Failure from polymorphic `spock check` target selection or checking.
#[derive(Debug, Error)]
pub enum CheckTargetError {
    #[error(transparent)]
    ProjectTopology(#[from] Diagnostics),
    #[error(transparent)]
    FileLoad(#[from] ProgramLoadError),
    #[error("error: {source}")]
    FileLoadProof {
        path: PathBuf,
        #[source]
        source: anyhow::Error,
    },
    #[error(transparent)]
    ProjectCheck(#[from] ProjectCheckFailure),
}

/// Select and fully check either an explicit `.spock` file or a project.
///
/// File mode deliberately calls the same [`FileProgram`] and
/// [`full_load_check`] boundary used by the legacy CLI. In project mode the
/// framework manifest is loaded first, then every configured subsystem is
/// checked without binding a listener or touching named state.
pub fn check_target(
    target: Option<&Path>,
    cwd: &Path,
) -> Result<CheckTargetSummary, CheckTargetError> {
    if let Some(path) = target.filter(|path| has_spock_extension(path)) {
        let program = FileProgram::load_from_cwd(path, cwd)?;
        let summary =
            full_load_check(program.contract(), program.source_dir()).map_err(|source| {
                CheckTargetError::FileLoadProof {
                    path: path.to_path_buf(),
                    source,
                }
            })?;
        return Ok(CheckTargetSummary::File {
            path: path.to_path_buf(),
            summary,
        });
    }

    match resolve_target(target, cwd)? {
        ResolvedTarget::SpockFile(path) => {
            // `resolve_target` selects file mode only by the extension checked
            // above. Keep this fallback defensive for future resolver modes.
            let program = FileProgram::load(&path)?;
            let summary =
                full_load_check(program.contract(), program.source_dir()).map_err(|source| {
                    CheckTargetError::FileLoadProof {
                        path: path.clone(),
                        source,
                    }
                })?;
            Ok(CheckTargetSummary::File { path, summary })
        }
        ResolvedTarget::Project(root) => {
            let layout = load_project(&root)?;
            let project_name = layout.manifest.project().as_str().to_string();
            let mut report = spock_host::check_project(&layout)?;
            deduplicate_warnings(&mut report);
            Ok(CheckTargetSummary::Project {
                root: layout.root,
                project_name,
                report,
            })
        }
    }
}

/// Failure while selecting a framework project for `start` or `dev`.
#[derive(Debug, Error)]
pub enum ResolveProjectForServeError {
    #[error(transparent)]
    ProjectTopology(#[from] Diagnostics),
    #[error("`{}` selects standalone `.spock` file mode; run `spock run` with that file instead", path.display())]
    StandaloneFile { path: PathBuf },
}

/// Resolve and validate a project target for a framework serving command.
///
/// `start` and `dev` never silently reinterpret a `.spock` file as a project;
/// their error points at the retained standalone `spock run` escape hatch.
pub fn resolve_project_for_serve(
    target: Option<&Path>,
    cwd: &Path,
) -> Result<ProjectLayout, ResolveProjectForServeError> {
    if let Some(path) = target.filter(|path| has_spock_extension(path)) {
        return Err(ResolveProjectForServeError::StandaloneFile {
            path: path.to_path_buf(),
        });
    }

    match resolve_target(target, cwd)? {
        ResolvedTarget::Project(root) => Ok(load_project(&root)?),
        ResolvedTarget::SpockFile(path) => {
            Err(ResolveProjectForServeError::StandaloneFile { path })
        }
    }
}

fn has_spock_extension(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("spock")
}

fn deduplicate_warnings(report: &mut ProjectCheckReport) {
    let mut seen = BTreeSet::new();
    report
        .warnings
        .retain(|warning| seen.insert(warning.clone()));
}

/// Whether a successful project write created a new root or adopted one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectWriteOperation {
    New,
    Init,
}

/// Exact, presentation-independent effects of `spock new` or `spock init`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectWriteSummary {
    pub operation: ProjectWriteOperation,
    pub root: PathBuf,
    pub project_name: String,
    pub includes_client: bool,
    pub created_files: Vec<PathBuf>,
    pub created_directories: Vec<PathBuf>,
}

impl fmt::Display for ProjectWriteSummary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let verb = match self.operation {
            ProjectWriteOperation::New => "created",
            ProjectWriteOperation::Init => "initialized",
        };
        let shape = if self.includes_client {
            "full-stack"
        } else {
            "backend-only"
        };
        write!(
            formatter,
            "{verb} {shape} project `{}` at {}",
            self.project_name,
            self.root.display(),
        )
    }
}

/// A rejected `spock new NAME` before any path is created.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid project name `{name}`: {reason}; NAME must be one safe path component")]
pub struct NewProjectNameError {
    pub name: String,
    pub reason: String,
}

/// Failure while planning or atomically applying `spock new`/`spock init`.
#[derive(Debug, Error)]
pub enum ProjectWriteError {
    #[error(transparent)]
    InvalidName(#[from] NewProjectNameError),
    #[error("could not resolve {role} `{}`: {source}", path.display())]
    ResolveDirectory {
        role: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{role} `{}` is not a directory", path.display())]
    NotDirectory { role: &'static str, path: PathBuf },
    #[error(transparent)]
    ProjectPlan(#[from] Diagnostics),
    #[error(transparent)]
    Apply(#[from] ApplyError),
}

/// Create the canonical project as a direct child of `cwd`.
///
/// Full stack is the default; `backend_only` is the explicit opt-out. The
/// destination is preflighted as empty and then created with the race-safe
/// new-destination policy.
pub fn create_project(
    cwd: &Path,
    name: &str,
    backend_only: bool,
) -> Result<ProjectWriteSummary, ProjectWriteError> {
    validate_new_project_name(name)?;
    let cwd = canonical_directory(cwd, "working directory")?;
    let destination = cwd.join(name);
    let client = (!backend_only).then(minimal_uhura_client_template);
    let plan = scaffold_plan(&destination, name, client.as_ref())?;
    let inventory = ProjectInventory::empty(destination);
    plan.preflight(&inventory)?;
    apply_project_plan(
        &plan,
        RootPolicy::NewDestination,
        ProjectWriteOperation::New,
    )
}

/// Adopt exactly `target`, or `cwd` when no target is supplied.
///
/// Unlike project discovery, initialization does not walk to a parent
/// manifest: it inventories the selected directory itself and lets the pure
/// adoption planner reject existing projects, ambiguity, and conflicts before
/// the race-safe apply boundary runs.
pub fn init_project(
    target: Option<&Path>,
    cwd: &Path,
) -> Result<ProjectWriteSummary, ProjectWriteError> {
    let cwd = canonical_directory(cwd, "working directory")?;
    let selected = match target {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => cwd.join(path),
        None => cwd,
    };
    let inventory = ProjectInventory::scan(&selected)?;
    let plan = adoption_plan(&inventory, None)?;
    plan.preflight(&inventory)?;
    apply_project_plan(
        &plan,
        RootPolicy::ExistingAdoptionRoot,
        ProjectWriteOperation::Init,
    )
}

fn validate_new_project_name(name: &str) -> Result<(), NewProjectNameError> {
    ProjectName::parse(name).map_err(|reason| NewProjectNameError {
        name: name.to_string(),
        reason,
    })?;

    let path = NormalizedRelativePath::file(name).map_err(|error| NewProjectNameError {
        name: name.to_string(),
        reason: error.to_string(),
    })?;
    if !path.parent().is_project_root() {
        return Err(NewProjectNameError {
            name: name.to_string(),
            reason: "nested paths are not allowed".to_string(),
        });
    }
    Ok(())
}

fn canonical_directory(path: &Path, role: &'static str) -> Result<PathBuf, ProjectWriteError> {
    let canonical =
        fs::canonicalize(path).map_err(|source| ProjectWriteError::ResolveDirectory {
            role,
            path: path.to_path_buf(),
            source,
        })?;
    if !canonical.is_dir() {
        return Err(ProjectWriteError::NotDirectory {
            role,
            path: canonical,
        });
    }
    Ok(canonical)
}

fn apply_project_plan(
    plan: &spock_project::WritePlan,
    policy: RootPolicy,
    operation: ProjectWriteOperation,
) -> Result<ProjectWriteSummary, ProjectWriteError> {
    let manifest_write = plan
        .write(MANIFEST_FILE)
        .expect("project plans always include their manifest commit marker");
    let manifest_source = std::str::from_utf8(&manifest_write.contents)
        .expect("project planners emit a UTF-8 manifest");
    let manifest = parse_manifest(manifest_source)?;
    let project_name = manifest.project().as_str().to_string();
    let includes_client = manifest.client().is_some();

    let applied = apply_write_plan(plan, policy)?;
    Ok(ProjectWriteSummary {
        operation,
        root: applied.root().to_path_buf(),
        project_name,
        includes_client,
        created_files: applied.created_files().to_vec(),
        created_directories: applied.created_directories().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            loop {
                let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "spock-project-commands-{}-{id}",
                    std::process::id()
                ));
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
    fn explicit_spock_check_keeps_the_file_load_proof_and_summary() {
        let temporary = TestDirectory::new();
        let source = temporary.path().join("standalone.spock");
        fs::write(
            &source,
            "table user { key id: uuid = auto\n name: text }\n\
             seed { user { name: \"Ada\" } }\n",
        )
        .unwrap();

        let result = check_target(Some(Path::new("standalone.spock")), temporary.path()).unwrap();

        assert_eq!(
            result,
            CheckTargetSummary::File {
                path: PathBuf::from("standalone.spock"),
                summary: CheckSummary {
                    tables: 1,
                    records: 0,
                    functions: 0,
                    unchecked_escapes: 0,
                    seed_rows: 1,
                },
            }
        );
        assert_eq!(
            result.to_string(),
            "ok: 1 table(s), 0 record(s), 0 fn(s), 1 seed row(s)"
        );
    }

    #[test]
    fn omitted_check_target_checks_the_nearest_project() {
        let temporary = TestDirectory::new();
        let created = create_project(temporary.path(), "demo", true).unwrap();
        let nested = created.root.join("backend");

        let checked = check_target(None, &nested).unwrap();

        let CheckTargetSummary::Project {
            root,
            project_name,
            report,
        } = checked
        else {
            panic!("omitted target did not select project mode");
        };
        assert_eq!(root, created.root);
        assert_eq!(project_name, "demo");
        assert_eq!(report.backend.tables, 0);
        assert!(report.client.is_none());
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn framework_serve_rejects_file_mode_with_run_guidance() {
        let temporary = TestDirectory::new();
        let error =
            resolve_project_for_serve(Some(Path::new("api.spock")), temporary.path()).unwrap_err();

        assert!(matches!(
            error,
            ResolveProjectForServeError::StandaloneFile { .. }
        ));
        let rendered = error.to_string();
        assert!(rendered.contains("spock run"), "{rendered}");
        assert!(rendered.contains("api.spock"), "{rendered}");
    }

    #[test]
    fn new_defaults_to_the_embedded_full_stack_template() {
        let temporary = TestDirectory::new();

        let summary = create_project(temporary.path(), "demo", false).unwrap();

        assert_eq!(summary.operation, ProjectWriteOperation::New);
        assert_eq!(summary.project_name, "demo");
        assert!(summary.includes_client);
        assert!(summary.root.join("backend/app.spock").is_file());
        for file in minimal_uhura_client_template().files() {
            assert_eq!(
                fs::read(summary.root.join("client").join(file.path().as_path())).unwrap(),
                file.contents(),
            );
        }
        assert_eq!(
            summary.created_files.last(),
            Some(&summary.root.join(MANIFEST_FILE)),
            "manifest remains the final commit marker"
        );
    }

    #[test]
    fn backend_only_new_omits_client_topology() {
        let temporary = TestDirectory::new();

        let summary = create_project(temporary.path(), "authority", true).unwrap();

        assert!(!summary.includes_client);
        assert!(!summary.root.join("client").exists());
        let manifest = fs::read_to_string(summary.root.join(MANIFEST_FILE)).unwrap();
        assert!(!manifest.contains("[client]"));
    }

    #[test]
    fn unsafe_new_names_fail_without_filesystem_effects() {
        let temporary = TestDirectory::new();

        for name in [
            "",
            ".",
            "..",
            "../escape",
            "nested/name",
            "nested\\name",
            "C:",
        ] {
            let error = create_project(temporary.path(), name, false).unwrap_err();
            assert!(matches!(error, ProjectWriteError::InvalidName(_)), "{name}");
        }

        assert_eq!(fs::read_dir(temporary.path()).unwrap().count(), 0);
    }

    #[test]
    fn init_adopts_the_exact_selected_directory_without_moving_sources() {
        let temporary = TestDirectory::new();
        let adoption = temporary.path().join("existing");
        fs::create_dir(&adoption).unwrap();
        fs::write(adoption.join("main.spock"), "").unwrap();
        fs::write(adoption.join("owned.txt"), "keep\n").unwrap();

        let summary = init_project(Some(Path::new("existing")), temporary.path()).unwrap();

        assert_eq!(summary.operation, ProjectWriteOperation::Init);
        assert_eq!(summary.root, fs::canonicalize(&adoption).unwrap());
        assert_eq!(
            fs::read_to_string(adoption.join("owned.txt")).unwrap(),
            "keep\n"
        );
        assert!(adoption.join("main.spock").is_file());
        assert!(!adoption.join("backend/app.spock").exists());
        let manifest = fs::read_to_string(adoption.join(MANIFEST_FILE)).unwrap();
        assert!(manifest.contains("root = \".\""), "{manifest}");
        assert!(manifest.contains("entry = \"main.spock\""), "{manifest}");
    }

    #[test]
    fn init_of_uhura_only_root_adds_the_required_empty_backend_and_checks() {
        let temporary = TestDirectory::new();
        for file in minimal_uhura_client_template().files() {
            let destination = temporary.path().join(file.path().as_path());
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, file.contents()).unwrap();
        }

        let summary = init_project(None, temporary.path()).unwrap();

        assert!(summary.includes_client);
        assert!(temporary.path().join("backend/app.spock").is_file());
        let checked = check_target(None, temporary.path()).unwrap();
        let CheckTargetSummary::Project { report, .. } = checked else {
            panic!("adopted root did not select project mode");
        };
        assert!(report.client.is_some());
        assert_eq!(report.unchecked_links, 1);
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn init_ambiguity_fails_before_writing_a_manifest() {
        let temporary = TestDirectory::new();
        fs::write(temporary.path().join("one.spock"), "").unwrap();
        fs::write(temporary.path().join("two.spock"), "").unwrap();

        let error = init_project(None, temporary.path()).unwrap_err();

        let ProjectWriteError::ProjectPlan(diagnostics) = error else {
            panic!("ambiguity did not remain a structured project diagnostic");
        };
        assert_eq!(
            diagnostics.iter().next().map(|diagnostic| diagnostic.code),
            Some(spock_project::DiagnosticCode::AmbiguousBackend),
        );
        assert!(!temporary.path().join(MANIFEST_FILE).exists());
    }
}
