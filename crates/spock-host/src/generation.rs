use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const STATUS_PROTOCOL: &str = "spock-project-status/1";

macro_rules! numeric_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

numeric_id!(ObservedRevision);
numeric_id!(BackendGenerationId);
numeric_id!(ClientGenerationId);
numeric_id!(ProjectGenerationId);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Fingerprint(String);

impl Fingerprint {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostMode {
    Start,
    Dev,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendFreshness {
    Active,
    RestartRequired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientFreshness {
    Absent,
    Building,
    Active,
    RejectedLastGood,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorFreshness {
    Current,
    Stale,
    ColdInvalid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ClientAttemptState {
    Building,
    Published,
    Rejected,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClientAttempt {
    pub observed_revision: ObservedRevision,
    pub state: ClientAttemptState,
    pub diagnostics: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub topology: Fingerprint,
    pub backend: Fingerprint,
    pub client: Option<Fingerprint>,
    pub changed_backend_inputs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObservationDisposition {
    NoChange,
    Changed {
        revision: ObservedRevision,
        backend: BackendFreshness,
        client_changed: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HealthStatus {
    pub ready: bool,
    pub degraded: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BackendStatus {
    pub generation_id: BackendGenerationId,
    pub world_id: String,
    pub freshness: BackendFreshness,
    pub active_source_fingerprint: Fingerprint,
    pub observed_source_fingerprint: Fingerprint,
    pub active_topology_fingerprint: Fingerprint,
    pub observed_topology_fingerprint: Fingerprint,
    pub changed_inputs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActiveClientStatus {
    pub generation_id: ClientGenerationId,
    pub source_revision: ObservedRevision,
    pub artifact_fingerprint: Fingerprint,
    pub backend_generation_id: BackendGenerationId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClientStatus {
    pub freshness: ClientFreshness,
    pub active: Option<ActiveClientStatus>,
    pub latest_attempt: Option<ClientAttempt>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActiveProjectStatus {
    pub generation_id: ProjectGenerationId,
    pub backend_generation_id: BackendGenerationId,
    pub client_generation_id: Option<ClientGenerationId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObservedStatus {
    pub revision: ObservedRevision,
    pub topology_fingerprint: Fingerprint,
    pub backend_fingerprint: Fingerprint,
    pub client_fingerprint: Option<Fingerprint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectStatus {
    pub protocol: String,
    pub mode: HostMode,
    pub observed: ObservedStatus,
    pub active_project: ActiveProjectStatus,
    pub backend: BackendStatus,
    pub client: ClientStatus,
    pub editor: EditorFreshness,
    pub health: HealthStatus,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum CandidateError {
    #[error(
        "client candidate revision {candidate} is stale; newest observed revision is {observed}"
    )]
    Stale {
        candidate: ObservedRevision,
        observed: ObservedRevision,
    },
    #[error("client candidate revision {0} was not started")]
    NotStarted(ObservedRevision),
}

#[derive(Clone, Debug)]
struct ActiveClient {
    generation_id: ClientGenerationId,
    source_revision: ObservedRevision,
    artifact_fingerprint: Fingerprint,
    backend_generation_id: BackendGenerationId,
}

/// Pure session state for fixed and watched project generations.
///
/// There is intentionally no method that replaces the active backend. A
/// process restart constructs another coordinator and another backend world.
#[derive(Clone, Debug)]
pub struct GenerationCoordinator {
    mode: HostMode,
    observed_revision: ObservedRevision,
    active_backend_id: BackendGenerationId,
    active_world_id: String,
    active_backend: Fingerprint,
    active_topology: Fingerprint,
    observed_backend: Fingerprint,
    observed_topology: Fingerprint,
    observed_client: Option<Fingerprint>,
    backend_freshness: BackendFreshness,
    changed_backend_inputs: Vec<String>,
    client_configured: bool,
    client_freshness: ClientFreshness,
    editor_freshness: EditorFreshness,
    latest_attempt: Option<ClientAttempt>,
    active_client: Option<ActiveClient>,
    project_generation_id: ProjectGenerationId,
    next_client_generation_id: u64,
}

impl GenerationCoordinator {
    #[must_use]
    pub fn activated(
        mode: HostMode,
        backend_fingerprint: Fingerprint,
        topology_fingerprint: Fingerprint,
        client_fingerprint: Option<Fingerprint>,
        world_id: impl Into<String>,
    ) -> Self {
        let client_configured = client_fingerprint.is_some();
        Self {
            mode,
            observed_revision: ObservedRevision(1),
            active_backend_id: BackendGenerationId(1),
            active_world_id: world_id.into(),
            active_backend: backend_fingerprint.clone(),
            active_topology: topology_fingerprint.clone(),
            observed_backend: backend_fingerprint,
            observed_topology: topology_fingerprint,
            observed_client: client_fingerprint,
            backend_freshness: BackendFreshness::Active,
            changed_backend_inputs: Vec::new(),
            client_configured,
            client_freshness: if client_configured {
                ClientFreshness::Building
            } else {
                ClientFreshness::Absent
            },
            editor_freshness: if client_configured {
                EditorFreshness::ColdInvalid
            } else {
                EditorFreshness::Current
            },
            latest_attempt: None,
            active_client: None,
            project_generation_id: ProjectGenerationId(1),
            next_client_generation_id: 1,
        }
    }

    #[must_use]
    pub const fn observed_revision(&self) -> ObservedRevision {
        self.observed_revision
    }

    #[must_use]
    pub const fn active_backend_generation(&self) -> BackendGenerationId {
        self.active_backend_id
    }

    /// Record one coherently captured filesystem state.
    ///
    /// TODO(RFD-0023): replace restart-required with off-path backend candidate
    /// construction and an explicit activation policy after development-world
    /// semantics are accepted. Never reopen or mutate the active world here.
    pub fn observe(&mut self, observation: Observation) -> ObservationDisposition {
        if self.observed_topology == observation.topology
            && self.observed_backend == observation.backend
            && self.observed_client == observation.client
        {
            return ObservationDisposition::NoChange;
        }

        let previous_client = self.observed_client.clone();
        self.observed_revision = ObservedRevision(self.observed_revision.0 + 1);
        self.observed_topology = observation.topology;
        self.observed_backend = observation.backend;
        self.observed_client = observation.client;

        let backend_matches = self.observed_topology == self.active_topology
            && self.observed_backend == self.active_backend;
        self.backend_freshness = if backend_matches {
            self.changed_backend_inputs.clear();
            BackendFreshness::Active
        } else {
            self.changed_backend_inputs = observation.changed_backend_inputs;
            BackendFreshness::RestartRequired
        };

        ObservationDisposition::Changed {
            revision: self.observed_revision,
            backend: self.backend_freshness,
            client_changed: previous_client != self.observed_client,
        }
    }

    pub fn begin_client_attempt(
        &mut self,
        revision: ObservedRevision,
    ) -> Result<(), CandidateError> {
        self.require_newest(revision)?;
        self.latest_attempt = Some(ClientAttempt {
            observed_revision: revision,
            state: ClientAttemptState::Building,
            diagnostics: Vec::new(),
        });
        self.client_freshness = ClientFreshness::Building;
        Ok(())
    }

    pub fn publish_client(
        &mut self,
        revision: ObservedRevision,
        artifact_fingerprint: Fingerprint,
    ) -> Result<ClientGenerationId, CandidateError> {
        self.require_started_newest(revision)?;
        let generation_id = ClientGenerationId(self.next_client_generation_id);
        self.next_client_generation_id += 1;
        self.active_client = Some(ActiveClient {
            generation_id,
            source_revision: revision,
            artifact_fingerprint,
            backend_generation_id: self.active_backend_id,
        });
        self.latest_attempt = Some(ClientAttempt {
            observed_revision: revision,
            state: ClientAttemptState::Published,
            diagnostics: Vec::new(),
        });
        self.client_freshness = ClientFreshness::Active;
        self.editor_freshness = EditorFreshness::Current;
        self.project_generation_id = ProjectGenerationId(self.project_generation_id.0 + 1);
        Ok(generation_id)
    }

    pub fn reject_client(
        &mut self,
        revision: ObservedRevision,
        diagnostics: Vec<String>,
    ) -> Result<(), CandidateError> {
        self.require_started_newest(revision)?;
        self.latest_attempt = Some(ClientAttempt {
            observed_revision: revision,
            state: ClientAttemptState::Rejected,
            diagnostics,
        });
        self.client_freshness = ClientFreshness::RejectedLastGood;
        self.editor_freshness = if self.active_client.is_some() {
            EditorFreshness::Stale
        } else {
            EditorFreshness::ColdInvalid
        };
        Ok(())
    }

    #[must_use]
    pub fn status(&self) -> ProjectStatus {
        let active_client = self
            .active_client
            .as_ref()
            .map(|client| ActiveClientStatus {
                generation_id: client.generation_id,
                source_revision: client.source_revision,
                artifact_fingerprint: client.artifact_fingerprint.clone(),
                backend_generation_id: client.backend_generation_id,
            });
        ProjectStatus {
            protocol: STATUS_PROTOCOL.to_owned(),
            mode: self.mode,
            observed: ObservedStatus {
                revision: self.observed_revision,
                topology_fingerprint: self.observed_topology.clone(),
                backend_fingerprint: self.observed_backend.clone(),
                client_fingerprint: self.observed_client.clone(),
            },
            active_project: ActiveProjectStatus {
                generation_id: self.project_generation_id,
                backend_generation_id: self.active_backend_id,
                client_generation_id: active_client.as_ref().map(|client| client.generation_id),
            },
            backend: BackendStatus {
                generation_id: self.active_backend_id,
                world_id: self.active_world_id.clone(),
                freshness: self.backend_freshness,
                active_source_fingerprint: self.active_backend.clone(),
                observed_source_fingerprint: self.observed_backend.clone(),
                active_topology_fingerprint: self.active_topology.clone(),
                observed_topology_fingerprint: self.observed_topology.clone(),
                changed_inputs: self.changed_backend_inputs.clone(),
            },
            client: ClientStatus {
                freshness: if self.client_configured {
                    self.client_freshness
                } else {
                    ClientFreshness::Absent
                },
                active: active_client,
                latest_attempt: self.latest_attempt.clone(),
            },
            editor: self.editor_freshness,
            health: HealthStatus {
                ready: true,
                degraded: self.backend_freshness == BackendFreshness::RestartRequired
                    || self.client_freshness == ClientFreshness::RejectedLastGood,
            },
        }
    }

    fn require_newest(&self, revision: ObservedRevision) -> Result<(), CandidateError> {
        if revision == self.observed_revision {
            Ok(())
        } else {
            Err(CandidateError::Stale {
                candidate: revision,
                observed: self.observed_revision,
            })
        }
    }

    fn require_started_newest(&self, revision: ObservedRevision) -> Result<(), CandidateError> {
        self.require_newest(revision)?;
        match &self.latest_attempt {
            Some(attempt)
                if attempt.observed_revision == revision
                    && attempt.state == ClientAttemptState::Building =>
            {
                Ok(())
            }
            _ => Err(CandidateError::NotStarted(revision)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint(value: &str) -> Fingerprint {
        Fingerprint::new(value)
    }

    fn coordinator() -> GenerationCoordinator {
        GenerationCoordinator::activated(
            HostMode::Dev,
            fingerprint("backend-a"),
            fingerprint("topology-a"),
            Some(fingerprint("client-a")),
            "world-a",
        )
    }

    fn observation(backend: &str, topology: &str, client: &str) -> Observation {
        Observation {
            topology: fingerprint(topology),
            backend: fingerprint(backend),
            client: Some(fingerprint(client)),
            changed_backend_inputs: vec!["backend/app.spock".to_owned()],
        }
    }

    #[test]
    fn identical_observation_is_a_no_op() {
        let mut coordinator = coordinator();
        assert_eq!(
            coordinator.observe(observation("backend-a", "topology-a", "client-a")),
            ObservationDisposition::NoChange
        );
        assert_eq!(coordinator.observed_revision(), ObservedRevision(1));
    }

    #[test]
    fn backend_change_requires_restart_without_replacing_any_active_identity() {
        let mut coordinator = coordinator();
        let backend_id = coordinator.active_backend_generation();
        let project_id = coordinator.status().active_project.generation_id;

        assert_eq!(
            coordinator.observe(observation("backend-b", "topology-a", "client-a")),
            ObservationDisposition::Changed {
                revision: ObservedRevision(2),
                backend: BackendFreshness::RestartRequired,
                client_changed: false,
            }
        );

        let status = coordinator.status();
        assert_eq!(status.backend.generation_id, backend_id);
        assert_eq!(status.active_project.generation_id, project_id);
        assert_eq!(
            status.backend.active_source_fingerprint,
            fingerprint("backend-a")
        );
        assert_eq!(
            status.backend.observed_source_fingerprint,
            fingerprint("backend-b")
        );
        assert!(status.health.ready);
        assert!(status.health.degraded);
    }

    #[test]
    fn topology_and_backend_must_both_revert_before_restart_required_clears() {
        let mut coordinator = coordinator();
        coordinator.observe(observation("backend-b", "topology-b", "client-a"));
        coordinator.observe(observation("backend-a", "topology-b", "client-a"));
        assert_eq!(
            coordinator.status().backend.freshness,
            BackendFreshness::RestartRequired
        );

        coordinator.observe(observation("backend-a", "topology-a", "client-a"));
        let status = coordinator.status();
        assert_eq!(status.backend.freshness, BackendFreshness::Active);
        assert!(status.backend.changed_inputs.is_empty());
        assert_eq!(status.backend.generation_id, BackendGenerationId(1));
    }

    #[test]
    fn client_publication_is_bound_to_the_active_backend() {
        let mut coordinator = coordinator();
        coordinator
            .begin_client_attempt(ObservedRevision(1))
            .unwrap();
        let client_id = coordinator
            .publish_client(ObservedRevision(1), fingerprint("artifact-a"))
            .unwrap();
        let status = coordinator.status();
        assert_eq!(status.client.freshness, ClientFreshness::Active);
        assert_eq!(
            status.client.active.as_ref().unwrap().generation_id,
            client_id
        );
        assert_eq!(
            status.client.active.as_ref().unwrap().backend_generation_id,
            BackendGenerationId(1)
        );
        assert_eq!(status.active_project.client_generation_id, Some(client_id));
    }

    #[test]
    fn invalid_client_retains_last_good_and_keeps_attempt_identity_separate() {
        let mut coordinator = coordinator();
        coordinator
            .begin_client_attempt(ObservedRevision(1))
            .unwrap();
        let good = coordinator
            .publish_client(ObservedRevision(1), fingerprint("artifact-a"))
            .unwrap();

        coordinator.observe(observation("backend-a", "topology-a", "client-b"));
        coordinator
            .begin_client_attempt(ObservedRevision(2))
            .unwrap();
        coordinator
            .reject_client(ObservedRevision(2), vec!["UH0001".to_owned()])
            .unwrap();

        let status = coordinator.status();
        assert_eq!(status.client.freshness, ClientFreshness::RejectedLastGood);
        assert_eq!(status.client.active.unwrap().generation_id, good);
        assert_eq!(
            status.client.latest_attempt,
            Some(ClientAttempt {
                observed_revision: ObservedRevision(2),
                state: ClientAttemptState::Rejected,
                diagnostics: vec!["UH0001".to_owned()],
            })
        );
        assert_eq!(status.editor, EditorFreshness::Stale);
    }

    #[test]
    fn a_newer_observation_permanently_makes_an_older_candidate_ineligible() {
        let mut coordinator = coordinator();
        coordinator
            .begin_client_attempt(ObservedRevision(1))
            .unwrap();
        coordinator.observe(observation("backend-a", "topology-a", "client-b"));
        assert_eq!(
            coordinator.publish_client(ObservedRevision(1), fingerprint("artifact-old")),
            Err(CandidateError::Stale {
                candidate: ObservedRevision(1),
                observed: ObservedRevision(2),
            })
        );
        assert!(coordinator.status().client.active.is_none());
    }

    #[test]
    fn client_can_publish_while_backend_restart_is_required() {
        let mut coordinator = coordinator();
        coordinator.observe(observation("backend-b", "topology-a", "client-b"));
        coordinator
            .begin_client_attempt(ObservedRevision(2))
            .unwrap();
        coordinator
            .publish_client(ObservedRevision(2), fingerprint("artifact-b"))
            .unwrap();

        let status = coordinator.status();
        assert_eq!(status.backend.freshness, BackendFreshness::RestartRequired);
        assert_eq!(status.client.freshness, ClientFreshness::Active);
        assert_eq!(
            status.client.active.unwrap().backend_generation_id,
            BackendGenerationId(1)
        );
    }

    #[test]
    fn status_protocol_and_attempted_generation_are_serialized() {
        let mut coordinator = coordinator();
        coordinator
            .begin_client_attempt(ObservedRevision(1))
            .unwrap();
        let value = serde_json::to_value(coordinator.status()).unwrap();
        assert_eq!(value["protocol"], STATUS_PROTOCOL);
        assert_eq!(value["client"]["freshness"], "building");
        assert_eq!(value["client"]["latest_attempt"]["state"], "building");
        assert_eq!(value["backend"]["generation_id"], 1);
    }
}
