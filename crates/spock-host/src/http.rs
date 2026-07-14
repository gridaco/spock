use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::json;
use spock_runtime::error::ApiError;
use spock_runtime::generation::BackendGeneration;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::CorsLayer;
use uhura_host::{EventStreamPoll, RequestMethod, RouteBody, RouteRequest, RouteResponse};

use crate::{
    classify_route, BackendGenerationId, ClientHost, GenerationCoordinator, HostMode,
    ProjectEventHub, ProjectEventStreamPoll, ProjectGenerationId, ProjectStatus, RouteOwner,
};

pub const HOST_ENVIRONMENT_PROTOCOL: &str = "spock-host-environment/1";
const STREAM_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// One active backend, optional client publication service, and stable
/// host-session status/event state.
///
/// The backend generation is immutable. Development observation can mutate
/// only the coordinator and client publication behind their narrow locks.
pub struct FrameworkSession {
    backend: Arc<BackendGeneration>,
    publication: Arc<RwLock<PublicationState>>,
    events: Arc<ProjectEventHub>,
    stream_shutdown: Arc<AtomicBool>,
}

pub(crate) struct PublicationState {
    pub coordinator: GenerationCoordinator,
    pub client: Option<ClientHost>,
}

impl FrameworkSession {
    #[must_use]
    pub(crate) fn new(
        backend: BackendGeneration,
        client: Option<ClientHost>,
        coordinator: GenerationCoordinator,
    ) -> Self {
        Self {
            backend: Arc::new(backend),
            publication: Arc::new(RwLock::new(PublicationState {
                coordinator,
                client,
            })),
            events: Arc::new(ProjectEventHub::default()),
            stream_shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    #[must_use]
    pub fn backend(&self) -> Arc<BackendGeneration> {
        Arc::clone(&self.backend)
    }

    #[must_use]
    pub(crate) fn publication(&self) -> Arc<RwLock<PublicationState>> {
        Arc::clone(&self.publication)
    }

    #[must_use]
    pub(crate) fn events(&self) -> Arc<ProjectEventHub> {
        Arc::clone(&self.events)
    }

    pub(crate) fn shutdown_streams(&self) {
        self.stream_shutdown.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn status(&self) -> ProjectStatus {
        self.publication
            .read()
            .expect("project publication lock")
            .coordinator
            .status()
    }

    /// Build the one-origin router without binding a listener.
    pub fn router(&self) -> Result<Router, spock_runtime::http::StartupError> {
        let authority = self.backend.authority_router()?;
        let graphql_available =
            !self.backend.contract().tables.is_empty() || !self.backend.contract().fns.is_empty();

        let status_state = self.publication();
        let environment_state = self.publication();
        let health_state = self.publication();
        let project_events = self.events();
        let publication = self.publication();
        let client_configured = publication
            .read()
            .expect("project publication lock")
            .client
            .is_some();
        let event_shutdown = Arc::clone(&self.stream_shutdown);
        let client_shutdown = Arc::clone(&self.stream_shutdown);

        let framework =
            Router::new()
                .route(
                    "/~project/status",
                    get(move || async move {
                        (
                            [(CACHE_CONTROL, HeaderValue::from_static("no-store"))],
                            Json(
                                status_state
                                    .read()
                                    .expect("project publication lock")
                                    .coordinator
                                    .status(),
                            ),
                        )
                    }),
                )
                .route(
                    "/~project/environment",
                    get(move || async move {
                        let status = environment_state
                            .read()
                            .expect("project publication lock")
                            .coordinator
                            .status();
                        (
                            [(CACHE_CONTROL, HeaderValue::from_static("no-store"))],
                            Json(HostEnvironment::from_status(&status, graphql_available)),
                        )
                    }),
                )
                .route(
                    "/~project/events",
                    get(move || {
                        let stream = project_events.subscribe();
                        let shutdown = Arc::clone(&event_shutdown);
                        async move { project_event_response(stream, shutdown) }
                    })
                    .head(|| async { event_method_error() }),
                )
                .route(
                    "/~health",
                    get(move || async move {
                        let status = health_state
                            .read()
                            .expect("project publication lock")
                            .coordinator
                            .status();
                        let code = if status.health.ready {
                            StatusCode::OK
                        } else {
                            StatusCode::SERVICE_UNAVAILABLE
                        };
                        (
                            code,
                            [(CACHE_CONTROL, HeaderValue::from_static("no-store"))],
                            Json(json!({
                                "ok": status.health.ready,
                                "ready": status.health.ready,
                                "degraded": status.health.degraded,
                            })),
                        )
                    }),
                )
                .fallback(move |request: Request| {
                    let publication = Arc::clone(&publication);
                    let shutdown = Arc::clone(&client_shutdown);
                    async move {
                        combined_fallback(request, publication, client_configured, shutdown).await
                    }
                });

        Ok(authority.merge(framework).layer(CorsLayer::permissive()))
    }
}

#[derive(Serialize)]
struct HostEnvironment {
    protocol: &'static str,
    mode: HostMode,
    project_generation_id: ProjectGenerationId,
    backend_generation_id: BackendGenerationId,
    authority: AuthorityEnvironment,
}

#[derive(Serialize)]
struct AuthorityEnvironment {
    graphql_path: Option<&'static str>,
    rpc_path: &'static str,
    storage_path: &'static str,
}

impl HostEnvironment {
    fn from_status(status: &ProjectStatus, graphql_available: bool) -> Self {
        Self {
            protocol: HOST_ENVIRONMENT_PROTOCOL,
            mode: status.mode,
            project_generation_id: status.active_project.generation_id,
            backend_generation_id: status.active_project.backend_generation_id,
            authority: AuthorityEnvironment {
                graphql_path: graphql_available.then_some("/graphql/v1"),
                rpc_path: "/rest/v1/rpc",
                storage_path: "/storage/v1",
            },
        }
    }
}

async fn combined_fallback(
    request: Request,
    publication: Arc<RwLock<PublicationState>>,
    client_configured: bool,
    stream_shutdown: Arc<AtomicBool>,
) -> Response {
    let path = request.uri().path().to_string();
    match classify_route(&path, client_configured) {
        RouteOwner::Client => route_client(request, &publication, stream_shutdown),
        RouteOwner::Framework if path == "/" && !client_configured => {
            if matches!(
                *request.method(),
                axum::http::Method::GET | axum::http::Method::HEAD
            ) {
                Redirect::temporary("/~studio").into_response()
            } else {
                let mut response = (
                    StatusCode::METHOD_NOT_ALLOWED,
                    Json(json!({
                        "error": {
                            "code": "bad_request",
                            "kind": "bad_request",
                            "table": null,
                            "fields": [],
                            "message": "the backend-only project root accepts GET and HEAD",
                        }
                    })),
                )
                    .into_response();
                response
                    .headers_mut()
                    .insert("allow", HeaderValue::from_static("GET, HEAD"));
                response
            }
        }
        RouteOwner::Framework
        | RouteOwner::Authority
        | RouteOwner::ProtocolNotFound
        | RouteOwner::NotFound => protocol_not_found(&path),
    }
}

fn protocol_not_found(path: &str) -> Response {
    ApiError::not_found(format!("no such path: {path}")).into_response()
}

fn route_client(
    request: Request,
    publication: &RwLock<PublicationState>,
    stream_shutdown: Arc<AtomicBool>,
) -> Response {
    let method = match *request.method() {
        axum::http::Method::GET => RequestMethod::Get,
        axum::http::Method::HEAD => RequestMethod::Head,
        _ => RequestMethod::Other,
    };
    let url = request
        .uri()
        .path_and_query()
        .map_or_else(|| request.uri().path(), |value| value.as_str());
    let publication = publication.read().expect("project publication lock");
    let Some(client) = &publication.client else {
        return protocol_not_found(request.uri().path());
    };
    let response = client.route(RouteRequest { method, url });
    drop(publication);
    uhura_response(response, stream_shutdown)
}

fn uhura_response(response: RouteResponse, stream_shutdown: Arc<AtomicBool>) -> Response {
    let status = StatusCode::from_u16(response.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut builder = Response::builder().status(status);
    for (name, value) in response.headers {
        let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
            return internal_transport_error("Uhura returned an invalid response header name");
        };
        let Ok(value) = HeaderValue::from_str(&value) else {
            return internal_transport_error("Uhura returned an invalid response header value");
        };
        builder = builder.header(name, value);
    }

    let body = match response.body {
        RouteBody::Bytes(bytes) => Body::from(bytes.into_inner()),
        RouteBody::Events(stream) => {
            let (sender, receiver) = mpsc::channel::<Result<Bytes, Infallible>>(1);
            tokio::spawn(async move {
                loop {
                    if sender.is_closed() || stream_shutdown.load(Ordering::Acquire) {
                        break;
                    }
                    match stream.try_next_frame() {
                        EventStreamPoll::Frame(frame) => {
                            match sender.try_send(Ok(Bytes::from(frame))) {
                                Ok(()) | Err(TrySendError::Full(_)) => {}
                                Err(TrySendError::Closed(_)) => break,
                            }
                        }
                        EventStreamPoll::Timeout => {
                            tokio::time::sleep(STREAM_POLL_INTERVAL).await;
                        }
                        EventStreamPoll::Closed => break,
                    }
                }
            });
            Body::from_stream(ReceiverStream::new(receiver))
        }
    };
    builder
        .body(body)
        .unwrap_or_else(|_| internal_transport_error("could not build the Uhura response"))
}

fn project_event_response(
    stream: crate::ProjectEventStream,
    stream_shutdown: Arc<AtomicBool>,
) -> Response {
    let (sender, receiver) = mpsc::channel::<Result<Bytes, Infallible>>(1);
    tokio::spawn(async move {
        loop {
            if sender.is_closed() || stream_shutdown.load(Ordering::Acquire) {
                break;
            }
            match stream.try_next_frame() {
                ProjectEventStreamPoll::Frame(frame) => {
                    match sender.try_send(Ok(Bytes::from(frame))) {
                        Ok(()) | Err(TrySendError::Full(_)) => {}
                        Err(TrySendError::Closed(_)) => break,
                    }
                }
                ProjectEventStreamPoll::Timeout => {
                    tokio::time::sleep(STREAM_POLL_INTERVAL).await;
                }
                ProjectEventStreamPoll::Closed => break,
            }
        }
    });

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/event-stream; charset=utf-8")
        .header(CACHE_CONTROL, "no-store")
        .body(Body::from_stream(ReceiverStream::new(receiver)))
        .unwrap_or_else(|_| internal_transport_error("could not build the project event response"))
}

fn event_method_error() -> Response {
    let mut response = (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(json!({
            "error": {
                "code": "bad_request",
                "kind": "bad_request",
                "table": null,
                "fields": [],
                "message": "/~project/events requires GET",
            }
        })),
    )
        .into_response();
    response
        .headers_mut()
        .insert("allow", HeaderValue::from_static("GET"));
    response
}

fn internal_transport_error(message: &'static str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": {
                "code": "internal",
                "kind": "internal",
                "table": null,
                "fields": [],
                "message": message,
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use axum::body::to_bytes;
    use axum::http::Request;
    use spock_runtime::generation::CapturedBackend;
    use tower::ServiceExt;

    use super::*;
    use crate::{Fingerprint, GenerationCoordinator};

    fn backend_only() -> FrameworkSession {
        session_with_source("")
    }

    fn session_with_source(source: &str) -> FrameworkSession {
        let backend =
            BackendGeneration::from_captured(CapturedBackend::new(source, BTreeMap::new()), None)
                .expect("backend generation");
        let coordinator = GenerationCoordinator::activated(
            HostMode::Start,
            Fingerprint::new("backend"),
            Fingerprint::new("topology"),
            None,
            "world-1",
        );
        FrameworkSession::new(backend, None, coordinator)
    }

    async fn response_json(response: Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body bytes");
        serde_json::from_slice(&bytes).expect("JSON body")
    }

    #[tokio::test]
    async fn backend_only_root_redirects_and_unknown_protocol_paths_are_json() {
        let router = backend_only().router().expect("router");
        let root = router
            .clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(root.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(root.headers()["location"], "/~studio");

        let unknown = router
            .clone()
            .oneshot(
                Request::get("/api/not-a-client-route")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
        assert_eq!(response_json(unknown).await["error"]["code"], "not_found");

        let post_root = router
            .clone()
            .oneshot(Request::post("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(post_root.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(post_root.headers()["allow"], "GET, HEAD");

        let event_head = router
            .oneshot(
                Request::head("/~project/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(event_head.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(event_head.headers()["allow"], "GET");
    }

    #[tokio::test]
    async fn encoded_client_protocol_spelling_cannot_fall_through_to_spa_html() {
        let session = backend_only();
        let response = combined_fallback(
            Request::get("/api%2Feditor/state")
                .body(Body::empty())
                .unwrap(),
            session.publication(),
            true,
            Arc::new(AtomicBool::new(false)),
        )
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(!bytes
            .windows(b"<!doctype".len())
            .any(|window| { window.eq_ignore_ascii_case(b"<!doctype") }));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["error"]["code"],
            "not_found"
        );
    }

    #[tokio::test]
    async fn fixed_status_environment_health_and_empty_contract_share_one_router() {
        let router = backend_only().router().expect("router");
        for path in [
            "/~project/status",
            "/~project/environment",
            "/~health",
            "/~contract",
        ] {
            let response = router
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            if path != "/~contract" {
                assert_eq!(response.headers()[CACHE_CONTROL], "no-store", "{path}");
            }
        }

        let environment = router
            .clone()
            .oneshot(
                Request::get("/~project/environment")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let environment = response_json(environment).await;
        assert_eq!(environment["protocol"], HOST_ENVIRONMENT_PROTOCOL);
        assert_eq!(environment["mode"], "start");
        assert!(environment["authority"]["graphql_path"].is_null());

        let graphql = router
            .oneshot(Request::get("/graphql/v1").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(graphql.status(), StatusCode::NOT_FOUND);
        assert_eq!(response_json(graphql).await["error"]["kind"], "not_found");
    }

    #[tokio::test]
    async fn a_non_empty_contract_advertises_graphql() {
        let router = session_with_source("table note { key id: uuid = auto }\n")
            .router()
            .expect("router");
        let environment = router
            .oneshot(
                Request::get("/~project/environment")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response_json(environment).await["authority"]["graphql_path"],
            "/graphql/v1"
        );
    }
}
