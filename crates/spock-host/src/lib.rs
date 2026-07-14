//! Coordination and hosting for one Spock framework project.
//!
//! The pure generation state machine lives independently from filesystem and
//! HTTP adapters so its activation laws can be exhaustively tested. Listener,
//! observer, and subsystem adapters are layered on this crate.

mod client;
mod generation;
mod named_state;
mod routing;

pub use client::{
    client_source_fingerprint, ActiveClientBinding, ClientHost, ClientHostError, ClientPublication,
    PreparedClient,
};
pub use generation::{
    BackendFreshness, BackendGenerationId, CandidateError, ClientAttempt, ClientAttemptState,
    ClientFreshness, ClientGenerationId, EditorFreshness, Fingerprint, GenerationCoordinator,
    HealthStatus, HostMode, Observation, ObservationDisposition, ObservedRevision,
    ProjectGenerationId, ProjectStatus,
};
pub use named_state::{named_state_lock_path, NamedStateLock, NamedStateLockError};
pub use routing::{classify_route, RouteOwner};
