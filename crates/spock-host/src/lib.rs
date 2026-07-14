//! Coordination and hosting for one Spock framework project.
//!
//! The pure generation state machine lives independently from filesystem and
//! HTTP adapters so its activation laws can be exhaustively tested. Listener,
//! observer, and subsystem adapters are layered on this crate.

mod assets;
mod backend_capture;
mod client;
mod events;
mod generation;
mod http;
mod named_state;
mod project;
mod routing;
mod server;

pub use assets::{
    locate_uhura_assets, AssetError, UhuraAssetRoots, SPOCK_UHURA_WASM_DIST, SPOCK_UHURA_WEB_DIST,
};
pub use backend_capture::{
    capture_backend, observe_backend, BackendDiagnostic, BackendDiagnosticCode, BackendDiagnostics,
    BackendObservation,
};
pub use client::{
    client_source_fingerprint, ActiveClientBinding, ClientHost, ClientHostError, ClientPublication,
    PreparedClient,
};
pub use events::{
    ProjectEvent, ProjectEventHub, ProjectEventStream, ProjectEventStreamPoll,
    PROJECT_EVENT_PROTOCOL, PROJECT_STATUS_PATH,
};
pub use generation::{
    BackendFreshness, BackendGenerationId, CandidateError, ClientAttempt, ClientAttemptState,
    ClientFreshness, ClientGenerationId, EditorFreshness, Fingerprint, GenerationCoordinator,
    HealthStatus, HostMode, Observation, ObservationDisposition, ObservedRevision,
    ProjectGenerationId, ProjectStatus,
};
pub use http::{FrameworkSession, HOST_ENVIRONMENT_PROTOCOL};
pub use named_state::{named_state_lock_path, NamedStateLock, NamedStateLockError};
pub use project::{
    check_project, topology_fingerprint, BackendCheckSummary, ClientCheckSummary, HostError,
    ProjectCheckDiagnostic, ProjectCheckFailure, ProjectCheckReport, ProjectComponent,
};
pub use routing::{classify_route, RouteOwner};
pub use server::{
    serve_project, HostNotice, HostNoticeSink, ServeError, ServeOptions, ServeOutcome,
};
