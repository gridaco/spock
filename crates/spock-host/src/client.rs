use thiserror::Error;
use uhura_host::{
    build_candidate, CandidateDiagnostics, CandidateSummary, ClientCandidate as UhuraCandidate,
    Host as UhuraHost, ProjectSourceSnapshot, PublicationReport, RouteRequest, RouteResponse,
    WebAssets,
};

use crate::{Fingerprint, ObservedRevision};

/// Content identity of the exact Uhura snapshot consumed by a client build.
///
/// Uhura remains responsible for enumerating and capturing its semantic
/// inputs. The framework only converts that subsystem-owned identity into its
/// status vocabulary.
#[must_use]
pub fn client_source_fingerprint(snapshot: &ProjectSourceSnapshot) -> Fingerprint {
    Fingerprint::new(snapshot.fingerprint().stable_id())
}

/// One complete off-path Uhura build, bound to the framework observation that
/// requested it.
///
/// `source_revision` is intentionally independent from `observed_revision`:
/// backend-only observations advance the framework clock without creating a
/// hole in Uhura's consecutive publication clock.
pub struct PreparedClient {
    observed_revision: ObservedRevision,
    source_revision: u64,
    source_fingerprint: Fingerprint,
    summary: CandidateSummary,
    candidate: UhuraCandidate,
}

impl PreparedClient {
    fn build(
        snapshot: &ProjectSourceSnapshot,
        observed_revision: ObservedRevision,
        source_revision: u64,
    ) -> Self {
        let source_fingerprint = client_source_fingerprint(snapshot);
        let candidate = build_candidate(snapshot, source_revision);
        let summary = candidate.summary();
        Self {
            observed_revision,
            source_revision,
            source_fingerprint,
            summary,
            candidate,
        }
    }

    #[must_use]
    pub const fn observed_revision(&self) -> ObservedRevision {
        self.observed_revision
    }

    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    #[must_use]
    pub fn source_fingerprint(&self) -> &Fingerprint {
        &self.source_fingerprint
    }

    #[must_use]
    pub const fn summary(&self) -> CandidateSummary {
        self.summary
    }

    #[must_use]
    pub fn diagnostics(&self) -> CandidateDiagnostics<'_> {
        self.candidate.diagnostics()
    }
}

/// The last Uhura Play generation that successfully replaced served client
/// artifacts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveClientBinding {
    pub observed_revision: ObservedRevision,
    pub source_revision: u64,
    pub source_fingerprint: Fingerprint,
    pub play_generation: u64,
}

/// Result of publishing one Editor/Play attempt into the listenerless Uhura
/// host.
///
/// An invalid attempt still advances Editor diagnostics and the Uhura source
/// revision. `active` continues to name the last successful Play generation,
/// if one exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientPublication {
    pub observed_revision: ObservedRevision,
    pub source_fingerprint: Fingerprint,
    pub report: PublicationReport,
    pub active: Option<ActiveClientBinding>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ClientHostError {
    #[error(
        "client candidate from observed revision {candidate} is stale; newest observed revision is {newest}"
    )]
    StaleCandidate {
        candidate: ObservedRevision,
        newest: ObservedRevision,
    },
    #[error(
        "client candidate repeats or precedes published observed revision {published}; received {received}"
    )]
    ObservationOrder {
        published: ObservedRevision,
        received: ObservedRevision,
    },
    #[error(
        "client candidate has Uhura source revision {received}; expected the next consecutive revision {expected}"
    )]
    PublicationOrder { expected: u64, received: u64 },
    #[error("Uhura host publication failed: {0}")]
    Uhura(String),
}

/// Framework-owned publication state around the reusable listenerless Uhura
/// host.
///
/// This type does not observe the filesystem or decide whether a candidate is
/// the newest project observation. The future `dev` coordinator supplies that
/// eligibility fact at publication time, while this boundary enforces it and
/// translates between the two revision domains.
pub struct ClientHost {
    host: UhuraHost,
    latest: ClientPublication,
}

impl ClientHost {
    /// Build and publish Uhura revision 1 before a public listener is bound.
    pub fn activate(
        web: WebAssets,
        snapshot: &ProjectSourceSnapshot,
        observed_revision: ObservedRevision,
    ) -> Result<(Self, ClientPublication), ClientHostError> {
        let prepared = PreparedClient::build(snapshot, observed_revision, 1);
        let source_fingerprint = prepared.source_fingerprint.clone();
        let (host, report) =
            UhuraHost::new(web, prepared.candidate).map_err(ClientHostError::Uhura)?;
        let latest = publication(observed_revision, source_fingerprint, report, None);
        Ok((
            Self {
                host,
                latest: latest.clone(),
            },
            latest,
        ))
    }

    /// Build the next candidate without mutating the served publication.
    #[must_use]
    pub fn prepare(
        &self,
        snapshot: &ProjectSourceSnapshot,
        observed_revision: ObservedRevision,
    ) -> PreparedClient {
        PreparedClient::build(snapshot, observed_revision, self.host.source_revision() + 1)
    }

    /// Publish only a candidate belonging to the newest framework observation.
    ///
    /// The caller passes the current observation after any off-path build has
    /// completed. This second check makes an older result permanently
    /// ineligible even when it finishes after newer work.
    pub fn publish(
        &mut self,
        prepared: PreparedClient,
        newest_observed_revision: ObservedRevision,
    ) -> Result<ClientPublication, ClientHostError> {
        if prepared.observed_revision != newest_observed_revision {
            return Err(ClientHostError::StaleCandidate {
                candidate: prepared.observed_revision,
                newest: newest_observed_revision,
            });
        }
        if prepared.observed_revision <= self.latest.observed_revision {
            return Err(ClientHostError::ObservationOrder {
                published: self.latest.observed_revision,
                received: prepared.observed_revision,
            });
        }

        let expected = self.host.source_revision() + 1;
        if prepared.source_revision != expected {
            return Err(ClientHostError::PublicationOrder {
                expected,
                received: prepared.source_revision,
            });
        }

        let report = self
            .host
            .publish(prepared.candidate)
            .map_err(ClientHostError::Uhura)?;
        let latest = publication(
            prepared.observed_revision,
            prepared.source_fingerprint,
            report,
            self.latest.active.clone(),
        );
        self.latest = latest.clone();
        Ok(latest)
    }

    #[must_use]
    pub fn latest_publication(&self) -> &ClientPublication {
        &self.latest
    }

    #[must_use]
    pub fn active_client(&self) -> Option<&ActiveClientBinding> {
        self.latest.active.as_ref()
    }

    /// Delegate one transport-neutral request to the subsystem host. The
    /// combined Axum adapter can translate the returned bytes or event stream
    /// without giving Uhura its own listener.
    #[must_use]
    pub fn route(&self, request: RouteRequest<'_>) -> RouteResponse {
        self.host.route(request)
    }
}

fn publication(
    observed_revision: ObservedRevision,
    source_fingerprint: Fingerprint,
    report: PublicationReport,
    previous_active: Option<ActiveClientBinding>,
) -> ClientPublication {
    let active = if report.play_ok {
        Some(ActiveClientBinding {
            observed_revision,
            source_revision: report.source_revision,
            source_fingerprint: source_fingerprint.clone(),
            play_generation: report.play_generation,
        })
    } else {
        previous_active
    };
    ClientPublication {
        observed_revision,
        source_fingerprint,
        report,
        active,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use uhura_host::{capture_project_snapshot, RequestMethod, RouteBody};

    use super::*;
    use crate::{GenerationCoordinator, HostMode, Observation};

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new(label: &str) -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "spock-host-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create temporary directory");
            Self(path)
        }
    }

    impl AsRef<Path> for TempDirectory {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn web_assets() -> (TempDirectory, WebAssets) {
        let root = TempDirectory::new("web");
        fs::create_dir_all(root.as_ref().join("assets")).unwrap();
        fs::write(
            root.as_ref().join("index.html"),
            r#"<!doctype html><script type="module" src="/assets/app.js"></script>"#,
        )
        .unwrap();
        fs::write(root.as_ref().join("assets/app.js"), "export {};\n").unwrap();
        let assets = WebAssets::from_frontend_directory(root.as_ref()).unwrap();
        (root, assets)
    }

    fn canonical_snapshot() -> ProjectSourceSnapshot {
        capture_project_snapshot(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../uhura/examples/instagram/client"),
        )
    }

    fn coordinator() -> GenerationCoordinator {
        GenerationCoordinator::activated(
            HostMode::Dev,
            Fingerprint::new("backend-a"),
            Fingerprint::new("topology-a"),
            Some(Fingerprint::new("client-a")),
            "world-a",
        )
    }

    fn advance_backend_only(coordinator: &mut GenerationCoordinator, name: &str) {
        coordinator.observe(Observation {
            topology: Fingerprint::new("topology-a"),
            backend: Fingerprint::new(name),
            client: Some(Fingerprint::new("client-a")),
            changed_backend_inputs: vec!["backend/app.spock".to_owned()],
            backend_diagnostics: Vec::new(),
        });
    }

    #[test]
    fn client_publication_clock_does_not_inherit_project_revision_gaps() {
        let (_web_root, web) = web_assets();
        let snapshot = canonical_snapshot();
        let mut coordinator = coordinator();
        let (mut host, initial) =
            ClientHost::activate(web, &snapshot, coordinator.observed_revision()).unwrap();
        assert_eq!(initial.report.source_revision, 1);
        assert!(initial.active.is_some());

        advance_backend_only(&mut coordinator, "backend-b");
        advance_backend_only(&mut coordinator, "backend-c");
        let observed_revision = coordinator.observed_revision();
        assert_eq!(observed_revision.get(), 3);

        let candidate = host.prepare(&snapshot, observed_revision);
        assert_eq!(candidate.observed_revision(), observed_revision);
        assert_eq!(candidate.source_revision(), 2);
        let published = host.publish(candidate, observed_revision).unwrap();
        assert_eq!(published.observed_revision, observed_revision);
        assert_eq!(published.report.source_revision, 2);
        assert_eq!(
            published.active.unwrap().observed_revision,
            observed_revision
        );
    }

    #[test]
    fn stale_result_is_rejected_before_it_can_advance_uhura_state() {
        let (_web_root, web) = web_assets();
        let snapshot = canonical_snapshot();
        let mut coordinator = coordinator();
        let (mut host, _) =
            ClientHost::activate(web, &snapshot, coordinator.observed_revision()).unwrap();

        advance_backend_only(&mut coordinator, "backend-b");
        let stale_revision = coordinator.observed_revision();
        let stale = host.prepare(&snapshot, stale_revision);
        advance_backend_only(&mut coordinator, "backend-c");
        let newest_revision = coordinator.observed_revision();
        let newest = host.prepare(&snapshot, newest_revision);

        assert_eq!(
            host.publish(stale, newest_revision),
            Err(ClientHostError::StaleCandidate {
                candidate: stale_revision,
                newest: newest_revision,
            })
        );
        assert_eq!(host.latest_publication().report.source_revision, 1);

        let publication = host.publish(newest, newest_revision).unwrap();
        assert_eq!(publication.report.source_revision, 2);
    }

    #[test]
    fn invalid_attempt_updates_editor_but_retains_last_good_play_binding() {
        let (_web_root, web) = web_assets();
        let valid = canonical_snapshot();
        let mut coordinator = coordinator();
        let initial_revision = coordinator.observed_revision();
        let (mut host, initial) = ClientHost::activate(web, &valid, initial_revision).unwrap();
        let active = initial.active.expect("valid example has Play artifacts");

        let invalid_root = TempDirectory::new("invalid-client");
        let invalid = capture_project_snapshot(invalid_root.as_ref());
        coordinator.observe(Observation {
            topology: Fingerprint::new("topology-a"),
            backend: Fingerprint::new("backend-a"),
            client: Some(client_source_fingerprint(&invalid)),
            changed_backend_inputs: Vec::new(),
            backend_diagnostics: Vec::new(),
        });
        let invalid_revision = coordinator.observed_revision();
        let candidate = host.prepare(&invalid, invalid_revision);
        let publication = host.publish(candidate, invalid_revision).unwrap();

        assert!(!publication.report.editor_current);
        assert!(!publication.report.play_ok);
        assert!(publication.report.has_good_play);
        assert_eq!(publication.active.as_ref(), Some(&active));
        assert_eq!(host.active_client(), Some(&active));

        let response = host.route(RouteRequest {
            method: RequestMethod::Get,
            url: "/api/play/ir.json",
        });
        assert_eq!(response.status, 200);
        assert!(matches!(response.body, RouteBody::Bytes(_)));
    }
}
