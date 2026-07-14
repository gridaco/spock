use std::fmt;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;
use sha2::{Digest, Sha256};
use spock_project::ProjectLayout;
use spock_runtime::generation::{BackendGeneration, BackendGenerationError};
use uhura_host::{build_candidate, capture_project_snapshot, ProjectSourceSnapshot};

use crate::{
    client_source_fingerprint, locate_uhura_assets, observe_backend, AssetError,
    BackendDiagnostics, BackendObservation, ClientHost, ClientHostError, Fingerprint,
    FrameworkSession, GenerationCoordinator, HostMode, NamedStateLock, NamedStateLockError,
    UhuraAssetRoots,
};

const STABLE_CAPTURE_ATTEMPTS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectComponent {
    Manifest,
    Backend,
    Client,
    Link,
}

impl fmt::Display for ProjectComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Manifest => "manifest",
            Self::Backend => "backend",
            Self::Client => "client",
            Self::Link => "link",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCheckDiagnostic {
    pub component: ProjectComponent,
    pub message: String,
}

impl fmt::Display for ProjectCheckDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.component, self.message)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectCheckFailure {
    diagnostics: Vec<ProjectCheckDiagnostic>,
}

impl ProjectCheckFailure {
    #[must_use]
    pub fn diagnostics(&self) -> &[ProjectCheckDiagnostic] {
        &self.diagnostics
    }

    fn push(&mut self, component: ProjectComponent, message: impl Into<String>) {
        self.diagnostics.push(ProjectCheckDiagnostic {
            component,
            message: message.into(),
        });
    }

    fn sort(&mut self) {
        self.diagnostics.sort_by(|left, right| {
            left.component
                .cmp(&right.component)
                .then_with(|| left.message.cmp(&right.message))
        });
    }
}

impl fmt::Display for ProjectCheckFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            diagnostic.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for ProjectCheckFailure {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendCheckSummary {
    pub tables: usize,
    pub records: usize,
    pub functions: usize,
    pub seed_rows: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientCheckSummary {
    pub source_id: String,
    pub preview_count: usize,
    pub replay_derived_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectCheckReport {
    pub backend: BackendCheckSummary,
    pub client: Option<ClientCheckSummary>,
    /// Application-owned provider code is an explicit adapter seam in v1.
    pub unchecked_links: usize,
    pub warnings: Vec<ProjectCheckDiagnostic>,
}

/// Check every project component without binding, watching, loading browser
/// assets, acquiring a named-state lock, or touching a named database.
pub fn check_project(layout: &ProjectLayout) -> Result<ProjectCheckReport, ProjectCheckFailure> {
    let mut failures = ProjectCheckFailure::default();
    let mut warnings = Vec::new();

    let backend = match observe_backend(layout).into_captured_backend() {
        Ok(captured) => match BackendGeneration::from_captured(captured, None) {
            Ok(generation) => match generation.authority_router() {
                Ok(_) => Some(BackendCheckSummary {
                    tables: generation.contract().tables.len(),
                    records: generation.contract().records.len(),
                    functions: generation.contract().fns.len(),
                    seed_rows: generation.contract().seed.len(),
                }),
                Err(error) => {
                    failures.push(ProjectComponent::Backend, error.to_string());
                    None
                }
            },
            Err(error) => {
                failures.push(ProjectComponent::Backend, error.to_string());
                None
            }
        },
        Err(diagnostics) => {
            for diagnostic in diagnostics.iter() {
                failures.push(ProjectComponent::Backend, diagnostic.to_string());
            }
            None
        }
    };

    let client = match &layout.client {
        None => None,
        Some(client_layout) => match capture_stable_client(client_layout.root.absolute()) {
            Ok(snapshot) => {
                let candidate = build_candidate(&snapshot, 1);
                let summary = candidate.summary();
                let diagnostics = candidate.diagnostics();
                collect_uhura_diagnostics(diagnostics.editor, &mut failures, &mut warnings);
                collect_uhura_diagnostics(diagnostics.play, &mut failures, &mut warnings);
                if summary.editor_current && summary.play_ok {
                    Some(ClientCheckSummary {
                        source_id: candidate.source_id(),
                        preview_count: summary.preview_count.unwrap_or(0),
                        replay_derived_count: summary.replay_derived_count.unwrap_or(0),
                    })
                } else {
                    if !failures
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.component == ProjectComponent::Client)
                    {
                        failures.push(
                            ProjectComponent::Client,
                            "Uhura candidate was rejected without a structured diagnostic",
                        );
                    }
                    None
                }
            }
            Err(message) => {
                failures.push(ProjectComponent::Client, message);
                None
            }
        },
    };

    if layout.client.is_some() {
        warnings.push(ProjectCheckDiagnostic {
            component: ProjectComponent::Link,
            message: "application-owned provider adapter code remains unchecked".to_string(),
        });
    }

    if !failures.diagnostics.is_empty() {
        failures.sort();
        return Err(failures);
    }

    Ok(ProjectCheckReport {
        backend: backend.expect("failure returned when backend summary is absent"),
        client,
        unchecked_links: usize::from(layout.client.is_some()),
        warnings,
    })
}

fn collect_uhura_diagnostics(
    envelope: &Value,
    failures: &mut ProjectCheckFailure,
    warnings: &mut Vec<ProjectCheckDiagnostic>,
) {
    let Some(diagnostics) = envelope.get("diagnostics").and_then(Value::as_array) else {
        return;
    };
    for diagnostic in diagnostics {
        let code = diagnostic
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("UHURA");
        let message = diagnostic
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("client check diagnostic");
        let rendered = format!("{code}: {message}");
        if diagnostic.get("severity").and_then(Value::as_str) == Some("error") {
            failures.push(ProjectComponent::Client, rendered);
        } else {
            warnings.push(ProjectCheckDiagnostic {
                component: ProjectComponent::Client,
                message: rendered,
            });
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error("backend input capture failed:\n{0}")]
    BackendCapture(BackendDiagnostics),
    #[error("backend generation failed: {0}")]
    BackendGeneration(#[from] BackendGenerationError),
    #[error("backend route construction failed: {0}")]
    BackendRoutes(#[from] spock_runtime::http::StartupError),
    #[error("client capture failed: {0}")]
    ClientCapture(String),
    #[error("configured client is invalid:\n{0}")]
    ClientInvalid(String),
    #[error(transparent)]
    Assets(#[from] AssetError),
    #[error(transparent)]
    ClientHost(#[from] ClientHostError),
    #[error(transparent)]
    NamedState(#[from] NamedStateLockError),
}

pub(crate) struct PreparedProject {
    pub layout: Arc<ProjectLayout>,
    pub session: Arc<FrameworkSession>,
    pub active_topology: Fingerprint,
    pub active_backend: BackendObservation,
    pub _named_state_lock: Option<NamedStateLock>,
}

pub(crate) fn prepare_project(
    layout: ProjectLayout,
    mode: HostMode,
    database_path: Option<&Path>,
    asset_roots: Option<UhuraAssetRoots>,
) -> Result<PreparedProject, HostError> {
    let active_topology = topology_fingerprint(&layout.manifest_path);
    let active_backend = observe_backend(&layout);
    let captured = active_backend
        .captured_backend()
        .cloned()
        .ok_or_else(|| HostError::BackendCapture(active_backend.diagnostics().clone()))?;

    // Prove the full backend load in memory before a client failure or missing
    // asset can cause a named database to be touched.
    let validation_backend = BackendGeneration::from_captured(captured.clone(), None)?;
    let _ = validation_backend.authority_router()?;

    let (initial_client, client_diagnostics, web) = if let Some(client_layout) = &layout.client {
        let snapshot = capture_stable_client(client_layout.root.absolute())
            .map_err(HostError::ClientCapture)?;
        let candidate = build_candidate(&snapshot, 1);
        let summary = candidate.summary();
        let diagnostic_text = candidate_diagnostic_text(&candidate);
        if mode == HostMode::Start && (!summary.editor_current || !summary.play_ok) {
            return Err(HostError::ClientInvalid(diagnostic_text));
        }
        let roots = match asset_roots {
            Some(roots) => roots,
            None => locate_uhura_assets()?,
        };
        let web = roots.load()?;
        (Some(snapshot), Some(diagnostic_text), Some(web))
    } else {
        (None, None, None)
    };

    let (named_state_lock, backend) = match database_path {
        Some(path) => {
            let lock = NamedStateLock::acquire(path)?;
            let backend = BackendGeneration::from_captured(captured, Some(path))?;
            (Some(lock), backend)
        }
        None => (None, validation_backend),
    };

    let client_fingerprint = initial_client.as_ref().map(client_source_fingerprint);
    let mut coordinator = GenerationCoordinator::activated(
        mode,
        active_backend.fingerprint().clone(),
        active_topology.clone(),
        client_fingerprint.clone(),
        active_backend.fingerprint().as_str(),
    );

    let client_host = match (web, initial_client.as_ref()) {
        (Some(web), Some(snapshot)) => {
            let revision = coordinator.observed_revision();
            coordinator
                .begin_client_attempt(revision)
                .expect("initial revision is newest");
            let (host, publication) = ClientHost::activate(web, snapshot, revision)?;
            if publication.report.editor_current && publication.report.play_ok {
                coordinator
                    .publish_client(
                        revision,
                        publication.report.source_revision,
                        client_fingerprint
                            .clone()
                            .expect("configured client has a fingerprint"),
                    )
                    .expect("initial client attempt was started");
            } else {
                coordinator
                    .reject_client(
                        revision,
                        vec![client_diagnostics
                            .unwrap_or_else(|| "Uhura client candidate was rejected".to_string())],
                    )
                    .expect("initial client attempt was started");
            }
            Some(host)
        }
        (None, None) => None,
        _ => unreachable!("client snapshot and browser assets are prepared together"),
    };

    Ok(PreparedProject {
        layout: Arc::new(layout),
        session: Arc::new(FrameworkSession::new(backend, client_host, coordinator)),
        active_topology,
        active_backend,
        _named_state_lock: named_state_lock,
    })
}

pub(crate) fn capture_stable_client(root: &Path) -> Result<ProjectSourceSnapshot, String> {
    let mut previous = capture_project_snapshot(root);
    for _ in 1..STABLE_CAPTURE_ATTEMPTS {
        let current = capture_project_snapshot(root);
        if current.fingerprint() == previous.fingerprint() {
            return Ok(current);
        }
        previous = current;
    }
    Err(format!(
        "client inputs under {} did not remain unchanged across {STABLE_CAPTURE_ATTEMPTS} consecutive captures",
        root.display()
    ))
}

#[must_use]
pub fn topology_fingerprint(manifest_path: &Path) -> Fingerprint {
    let mut hasher = Sha256::new();
    hasher.update(b"spock-project-topology/1\0");
    match fs::symlink_metadata(manifest_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            hasher.update(b"symlink\0");
            match fs::read_link(manifest_path) {
                Ok(target) => {
                    hash_field(&mut hasher, target.as_os_str().to_string_lossy().as_bytes())
                }
                Err(error) => hash_field(&mut hasher, error.to_string().as_bytes()),
            }
        }
        Ok(metadata) if metadata.is_file() => {
            hasher.update(b"file\0");
            match fs::read(manifest_path) {
                Ok(bytes) => hash_field(&mut hasher, &bytes),
                Err(error) => hash_field(&mut hasher, error.to_string().as_bytes()),
            }
        }
        Ok(metadata) if metadata.is_dir() => hasher.update(b"directory\0"),
        Ok(_) => hasher.update(b"other\0"),
        Err(error) => {
            hasher.update(b"unavailable\0");
            hash_field(&mut hasher, error.to_string().as_bytes());
        }
    }
    Fingerprint::new(hex_digest(hasher.finalize().as_slice()))
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
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

fn candidate_diagnostic_text(candidate: &uhura_host::ClientCandidate) -> String {
    let diagnostics = candidate.diagnostics();
    let values = [diagnostics.editor, diagnostics.play]
        .into_iter()
        .filter(|value| !value.is_null())
        .map(|value| serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()))
        .collect::<Vec<_>>();
    if values.is_empty() {
        "Uhura client candidate has no diagnostics".to_string()
    } else {
        values.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use spock_project::{load_project_from, ProjectManifest};
    use tempfile::tempdir;

    use super::*;

    fn write_project(root: &Path, backend: &str, with_client: bool) -> ProjectLayout {
        fs::create_dir_all(root.join("backend")).unwrap();
        fs::write(root.join("backend/app.spock"), backend).unwrap();
        if with_client {
            for file in spock_project::minimal_uhura_client_template().files() {
                let path = root.join("client").join(file.path().as_path());
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(path, file.contents()).unwrap();
            }
        }
        let manifest = ProjectManifest::new(
            "demo",
            "backend",
            "app.spock",
            with_client.then_some("client"),
        )
        .unwrap();
        fs::write(root.join("spock.toml"), manifest.to_toml_string()).unwrap();
        load_project_from(root).unwrap()
    }

    #[test]
    fn project_check_accepts_backend_only_and_empty_full_stack_projects() {
        let backend = tempdir().unwrap();
        let backend_layout = write_project(backend.path(), "", false);
        let report = check_project(&backend_layout).unwrap();
        assert_eq!(
            report.backend,
            BackendCheckSummary {
                tables: 0,
                records: 0,
                functions: 0,
                seed_rows: 0,
            }
        );
        assert!(report.client.is_none());

        let full = tempdir().unwrap();
        let full_layout = write_project(full.path(), "", true);
        let report = check_project(&full_layout).unwrap();
        assert!(report.client.is_some());
        assert_eq!(report.unchecked_links, 1);
    }

    #[test]
    fn project_check_aggregates_backend_and_client_failures() {
        let temp = tempdir().unwrap();
        let layout = write_project(temp.path(), "table broken {", true);
        fs::write(
            temp.path().join("client/app/home/page.uhura"),
            "not valid uhura",
        )
        .unwrap();

        let failure = check_project(&layout).unwrap_err();
        assert!(failure
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.component == ProjectComponent::Backend));
        assert!(failure
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.component == ProjectComponent::Client));
    }

    #[test]
    fn topology_identity_changes_for_bytes_and_unsafe_entry_kinds() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("spock.toml");
        fs::write(&path, "version = 1\n").unwrap();
        let first = topology_fingerprint(&path);
        fs::write(&path, "version = 2\n").unwrap();
        let second = topology_fingerprint(&path);
        assert_ne!(first, second);
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert_ne!(second, topology_fingerprint(&path));
    }

    #[test]
    fn candidate_diagnostic_text_is_not_empty_for_invalid_snapshot() {
        let temp = tempdir().unwrap();
        let snapshot = capture_project_snapshot(temp.path());
        let candidate = build_candidate(&snapshot, 1);
        let rendered = candidate_diagnostic_text(&candidate);
        assert!(rendered.contains("uhura-diagnostics"), "{rendered}");
    }

    #[test]
    fn in_memory_prepare_never_creates_named_state() {
        let temp = tempdir().unwrap();
        let layout = write_project(temp.path(), "", false);
        let prepared = prepare_project(layout, HostMode::Start, None, None).unwrap();
        assert!(prepared._named_state_lock.is_none());
        assert_eq!(prepared.session.status().mode, HostMode::Start);
        assert_eq!(
            prepared
                .session
                .backend()
                .input_fingerprint()
                .unwrap()
                .as_str(),
            prepared.active_backend.fingerprint().as_str()
        );
    }
}
