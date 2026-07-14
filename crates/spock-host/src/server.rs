use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use spock_project::ProjectLayout;
use tokio::task::JoinHandle;
use uhura_host::ProjectSourceSnapshot;

use crate::project::{capture_stable_client, prepare_project, PreparedProject};
use crate::{
    client_source_fingerprint, observe_backend, topology_fingerprint, BackendFreshness,
    BackendObservation, Fingerprint, HostError, HostMode, Observation, ObservationDisposition,
    UhuraAssetRoots,
};

const COHERENT_FRAME_ATTEMPTS: usize = 4;

#[derive(Clone, Debug)]
pub struct ServeOptions {
    pub bind: SocketAddr,
    pub database_path: Option<PathBuf>,
    pub asset_roots: Option<UhuraAssetRoots>,
    pub poll_interval: Duration,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4000),
            database_path: None,
            asset_roots: None,
            poll_interval: Duration::from_millis(250),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostNotice {
    DevelopmentPolicy,
    Listening {
        address: SocketAddr,
        client_configured: bool,
    },
    ClientBuilding {
        observed_revision: u64,
    },
    ClientPublished {
        observed_revision: u64,
        source_revision: u64,
        play_generation: u64,
    },
    ClientRejected {
        observed_revision: u64,
        diagnostics: Vec<String>,
        serving_last_good: bool,
    },
    BackendRestartRequired {
        changed_inputs: Vec<String>,
        diagnostics: Vec<String>,
    },
    BackendReverted,
    ObserverError {
        message: String,
    },
}

#[derive(Clone)]
pub struct HostNoticeSink(Arc<dyn Fn(HostNotice) + Send + Sync>);

impl HostNoticeSink {
    pub fn new(callback: impl Fn(HostNotice) + Send + Sync + 'static) -> Self {
        Self(Arc::new(callback))
    }

    fn emit(&self, notice: HostNotice) {
        (self.0)(notice);
    }
}

impl Default for HostNoticeSink {
    fn default() -> Self {
        Self::new(|_| {})
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServeOutcome {
    pub local_address: SocketAddr,
}

#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    #[error(transparent)]
    Prepare(#[from] HostError),
    #[error("could not bind framework host at {address}: {source}")]
    Bind {
        address: SocketAddr,
        #[source]
        source: std::io::Error,
    },
    #[error("could not inspect bound framework listener: {0}")]
    LocalAddress(std::io::Error),
    #[error("could not start backend generation lifecycle: {0}")]
    BackendLifecycle(#[from] spock_runtime::generation::BackendLifecycleError),
    #[error("framework server failed: {0}")]
    Serve(std::io::Error),
    #[error("development observer failed: {0}")]
    Observer(#[from] tokio::task::JoinError),
}

/// Prepare, bind, and serve one fixed or watched framework project.
///
/// `start` and `dev` share the same preparation proof and one listener. The
/// development observer never constructs a second backend generation.
pub async fn serve_project<F>(
    layout: ProjectLayout,
    mode: HostMode,
    options: ServeOptions,
    notices: HostNoticeSink,
    shutdown: F,
) -> Result<ServeOutcome, ServeError>
where
    F: Future<Output = ()> + Send,
{
    let prepared = prepare_project(
        layout,
        mode,
        options.database_path.as_deref(),
        options.asset_roots,
    )?;
    let router = prepared.session.router().map_err(HostError::from)?;
    let listener = tokio::net::TcpListener::bind(options.bind)
        .await
        .map_err(|source| ServeError::Bind {
            address: options.bind,
            source,
        })?;
    let local_address = listener.local_addr().map_err(ServeError::LocalAddress)?;
    let lifecycle = prepared.session.backend().start_background_tasks()?;

    if mode == HostMode::Dev {
        notices.emit(HostNotice::DevelopmentPolicy);
    }
    notices.emit(HostNotice::Listening {
        address: local_address,
        client_configured: prepared.layout.client.is_some(),
    });

    let observer_stop = Arc::new(AtomicBool::new(false));
    let observer = if mode == HostMode::Dev {
        Some(spawn_observer(
            &prepared,
            options.poll_interval,
            Arc::clone(&observer_stop),
            notices.clone(),
        ))
    } else {
        None
    };

    let shutdown_session = Arc::clone(&prepared.session);
    let shutdown_observer = Arc::clone(&observer_stop);
    let server_result = serve_router_until_shutdown(listener, router, shutdown, move || {
        // Stop producing new observations and close host-owned streaming
        // bodies as soon as the listener begins graceful shutdown. Axum can
        // then drain every accepted connection instead of waiting forever on
        // SSE, while the backend generation and named-state lock stay alive.
        shutdown_observer.store(true, Ordering::Release);
        shutdown_session.shutdown_streams();
    })
    .await
    .map_err(ServeError::Serve);

    observer_stop.store(true, Ordering::Release);
    prepared.session.shutdown_streams();
    if let Some(observer) = observer {
        observer.await?;
    }
    lifecycle.shutdown().await;
    server_result?;

    // `prepared` deliberately remains alive through listener, observer, SSE,
    // and backend-task shutdown. Its final field owns the named-state lock,
    // which is released only after the session/database handles are dropped.
    drop(prepared);
    Ok(ServeOutcome { local_address })
}

async fn serve_router_until_shutdown<F, C>(
    listener: tokio::net::TcpListener,
    router: axum::Router,
    shutdown: F,
    on_shutdown: C,
) -> std::io::Result<()>
where
    F: Future<Output = ()> + Send,
    C: FnOnce(),
{
    let (graceful_tx, graceful_rx) = tokio::sync::oneshot::channel();
    let server = async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = graceful_rx.await;
            })
            .await
    };
    tokio::pin!(server);
    tokio::pin!(shutdown);
    tokio::select! {
        result = &mut server => result,
        () = &mut shutdown => {
            on_shutdown();
            let _ = graceful_tx.send(());
            server.await
        }
    }
}

fn spawn_observer(
    prepared: &PreparedProject,
    poll_interval: Duration,
    stop: Arc<AtomicBool>,
    notices: HostNoticeSink,
) -> JoinHandle<()> {
    let layout = Arc::clone(&prepared.layout);
    let session = Arc::clone(&prepared.session);
    let active_backend = prepared.active_backend.clone();
    let active_topology = prepared.active_topology.clone();

    tokio::task::spawn_blocking(move || {
        let mut force_client_build = false;
        while !stop.load(Ordering::Acquire) {
            std::thread::sleep(poll_interval);
            if stop.load(Ordering::Acquire) {
                break;
            }

            let frame = match capture_frame(&layout) {
                Ok(frame) => frame,
                Err(message) => {
                    notices.emit(HostNotice::ObserverError { message });
                    continue;
                }
            };
            let disposition = apply_frame(
                &session,
                &active_backend,
                &active_topology,
                &frame,
                &notices,
            );
            let client_changed = matches!(
                disposition,
                ObservationDisposition::Changed {
                    client_changed: true,
                    ..
                }
            );
            if !client_changed && !force_client_build {
                continue;
            }
            let Some(client_observation) = frame.client.as_ref() else {
                force_client_build = false;
                continue;
            };
            let client_host_configured = session
                .publication()
                .read()
                .expect("project publication lock")
                .client
                .is_some();
            if !client_host_configured {
                // A valid topology edit may add a client to a backend-only
                // active session. That requires process reconstruction; never
                // leave the absent client state stuck in `building`.
                force_client_build = false;
                continue;
            }
            force_client_build = false;

            let observed_revision = {
                let publication_state = session.publication();
                let mut publication = publication_state.write().expect("project publication lock");
                let revision = publication.coordinator.observed_revision();
                if let Err(error) = publication.coordinator.begin_client_attempt(revision) {
                    notices.emit(HostNotice::ObserverError {
                        message: error.to_string(),
                    });
                    continue;
                }
                revision
            };
            session.events().publish();
            notices.emit(HostNotice::ClientBuilding {
                observed_revision: observed_revision.get(),
            });

            if let Some(diagnostics) = client_observation.diagnostics() {
                let diagnostics = diagnostics.to_vec();
                let rejection = session
                    .publication()
                    .write()
                    .expect("project publication lock")
                    .coordinator
                    .reject_client(observed_revision, diagnostics.clone());
                match rejection {
                    Ok(()) => {
                        session.events().publish();
                        notices.emit(HostNotice::ClientRejected {
                            observed_revision: observed_revision.get(),
                            diagnostics,
                            serving_last_good: session.status().client.active.is_some(),
                        });
                    }
                    Err(error) => notices.emit(HostNotice::ObserverError {
                        message: error.to_string(),
                    }),
                }
                continue;
            }
            let snapshot = client_observation
                .snapshot()
                .expect("a client observation without diagnostics has a captured snapshot");

            let candidate = {
                let publication_state = session.publication();
                let publication = publication_state.read().expect("project publication lock");
                let Some(client) = &publication.client else {
                    continue;
                };
                client.prepare(snapshot, observed_revision)
            };
            let summary = candidate.summary();
            let source_fingerprint = candidate.source_fingerprint().clone();
            let diagnostics = prepared_client_diagnostics(&candidate);

            // The build may have overlapped another save. Re-observe every
            // subsystem before publication; a newer project revision makes
            // this result permanently ineligible.
            let latest = match capture_frame(&layout) {
                Ok(frame) => frame,
                Err(message) => {
                    notices.emit(HostNotice::ObserverError { message });
                    // The attempt is already visible as Building. Retry this
                    // same observed revision even when the next coherent
                    // frame has the same fingerprint, so capture instability
                    // cannot strand client status indefinitely.
                    force_client_build = true;
                    continue;
                }
            };
            let _ = apply_frame(
                &session,
                &active_backend,
                &active_topology,
                &latest,
                &notices,
            );
            let newest_revision = session
                .publication()
                .read()
                .expect("project publication lock")
                .coordinator
                .observed_revision();
            if newest_revision != observed_revision {
                force_client_build = true;
                continue;
            }

            let publication_result = {
                // Client artifact installation and status mutation share one
                // writer lock. Readers see entirely the old or new
                // publication; the invalidation is sent only after release.
                let publication_state = session.publication();
                let mut publication = publication_state.write().expect("project publication lock");
                let result = publication
                    .client
                    .as_mut()
                    .expect("configured client host")
                    .publish(candidate, newest_revision);
                match result.map_err(|error| error.to_string()) {
                    Ok(client_publication) => {
                        if summary.editor_current && summary.play_ok {
                            publication
                                .coordinator
                                .publish_client(
                                    newest_revision,
                                    client_publication.report.source_revision,
                                    source_fingerprint,
                                    diagnostics.clone(),
                                )
                                .map(|_| client_publication)
                                .map_err(|error| error.to_string())
                        } else {
                            publication
                                .coordinator
                                .reject_client(newest_revision, diagnostics.clone())
                                .map(|()| client_publication)
                                .map_err(|error| error.to_string())
                        }
                    }
                    Err(error) => Err(error),
                }
            };

            match publication_result {
                Ok(publication) => {
                    session.events().publish();
                    if publication.report.editor_current && publication.report.play_ok {
                        notices.emit(HostNotice::ClientPublished {
                            observed_revision: newest_revision.get(),
                            source_revision: publication.report.source_revision,
                            play_generation: publication.report.play_generation,
                        });
                    } else {
                        notices.emit(HostNotice::ClientRejected {
                            observed_revision: newest_revision.get(),
                            diagnostics,
                            serving_last_good: publication.report.has_good_play,
                        });
                    }
                }
                Err(error) => {
                    notices.emit(HostNotice::ObserverError {
                        message: error.to_string(),
                    });
                    // Publication is deterministic and revision-consuming.
                    // Retrying this same observation can only repeat an
                    // invariant failure (or violate ClientHost ordering); a
                    // later filesystem observation supplies a new attempt.
                }
            }
        }
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedLayout {
    layout: ProjectLayout,
    diagnostics: Vec<String>,
}

enum ClientObservation {
    Captured(ProjectSourceSnapshot),
    Invalid {
        fingerprint: Fingerprint,
        diagnostics: Vec<String>,
    },
}

impl ClientObservation {
    fn fingerprint(&self) -> Fingerprint {
        match self {
            Self::Captured(snapshot) => client_source_fingerprint(snapshot),
            Self::Invalid { fingerprint, .. } => fingerprint.clone(),
        }
    }

    fn snapshot(&self) -> Option<&ProjectSourceSnapshot> {
        match self {
            Self::Captured(snapshot) => Some(snapshot),
            Self::Invalid { .. } => None,
        }
    }

    fn diagnostics(&self) -> Option<&[String]> {
        match self {
            Self::Captured(_) => None,
            Self::Invalid { diagnostics, .. } => Some(diagnostics),
        }
    }
}

struct ObservationFrame {
    topology: Fingerprint,
    backend: BackendObservation,
    client: Option<ClientObservation>,
    topology_diagnostics: Vec<String>,
}

fn capture_frame(layout: &ProjectLayout) -> Result<ObservationFrame, String> {
    for _ in 0..COHERENT_FRAME_ATTEMPTS {
        let topology_before = topology_fingerprint(&layout.manifest_path);
        let observed_layout_before = resolve_observed_layout(layout);
        let backend_before = observe_backend(&observed_layout_before.layout);
        let client = observed_layout_before
            .layout
            .client
            .as_ref()
            .map(|client| capture_observed_client(&observed_layout_before.layout, client))
            .transpose()?;
        let observed_layout_after = resolve_observed_layout(layout);
        let backend_after = observe_backend(&observed_layout_after.layout);
        let topology_after = topology_fingerprint(&layout.manifest_path);

        // Re-parse and re-resolve the logical project on both sides of the
        // subsystem captures. This observes safe in-project symlink retargets,
        // prevents a cached canonical target from becoming a permanent watch
        // root, and rejects a frame assembled across a topology transition.
        if topology_before == topology_after
            && observed_layout_before == observed_layout_after
            && backend_before.fingerprint() == backend_after.fingerprint()
        {
            return Ok(ObservationFrame {
                topology: topology_after,
                backend: backend_after,
                client,
                topology_diagnostics: observed_layout_after.diagnostics,
            });
        }
    }
    Err(format!(
        "project inputs under {} did not remain unchanged across {COHERENT_FRAME_ATTEMPTS} coherent captures",
        layout.root.display()
    ))
}

fn resolve_observed_layout(active: &ProjectLayout) -> ObservedLayout {
    match spock_project::load_project_from(&active.root) {
        Ok(layout) => ObservedLayout {
            layout,
            diagnostics: Vec::new(),
        },
        Err(diagnostics) => ObservedLayout {
            layout: active.clone(),
            diagnostics: diagnostics.iter().map(ToString::to_string).collect(),
        },
    }
}

fn capture_observed_client(
    layout: &ProjectLayout,
    client: &spock_project::ClientLayout,
) -> Result<ClientObservation, String> {
    match spock_project::resolve_contained(&layout.root, client.root.relative()) {
        Ok(root) => capture_stable_client(root.absolute()).map(ClientObservation::Captured),
        Err(diagnostics) => {
            let diagnostics = diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            Ok(ClientObservation::Invalid {
                fingerprint: invalid_client_fingerprint(&diagnostics),
                diagnostics,
            })
        }
    }
}

fn invalid_client_fingerprint(diagnostics: &[String]) -> Fingerprint {
    let mut hasher = Sha256::new();
    hasher.update(b"spock-invalid-client-observation/1\0");
    hasher.update((diagnostics.len() as u64).to_be_bytes());
    for diagnostic in diagnostics {
        hasher.update((diagnostic.len() as u64).to_be_bytes());
        hasher.update(diagnostic.as_bytes());
    }
    Fingerprint::new(format!("{:x}", hasher.finalize()))
}

fn apply_frame(
    session: &crate::FrameworkSession,
    active_backend: &BackendObservation,
    active_topology: &Fingerprint,
    frame: &ObservationFrame,
    notices: &HostNoticeSink,
) -> ObservationDisposition {
    let mut changed_inputs = frame.backend.changed_inputs_since(active_backend);
    if &frame.topology != active_topology {
        changed_inputs.push("spock.toml".to_string());
        changed_inputs.sort();
        changed_inputs.dedup();
    }
    let mut backend_diagnostics = frame
        .backend
        .diagnostics()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let client_diagnostics = frame
        .client
        .as_ref()
        .and_then(ClientObservation::diagnostics)
        .unwrap_or_default();
    backend_diagnostics.extend(
        frame
            .topology_diagnostics
            .iter()
            .filter(|diagnostic| {
                !client_diagnostics
                    .iter()
                    .any(|client_diagnostic| client_diagnostic == *diagnostic)
            })
            .cloned(),
    );
    backend_diagnostics.sort();
    backend_diagnostics.dedup();
    let client = frame.client.as_ref().map(ClientObservation::fingerprint);

    let (before, disposition, after) = {
        let publication_state = session.publication();
        let mut publication = publication_state.write().expect("project publication lock");
        let before = publication.coordinator.status().backend.freshness;
        let disposition = publication.coordinator.observe(Observation {
            topology: frame.topology.clone(),
            backend: frame.backend.fingerprint().clone(),
            client,
            changed_backend_inputs: changed_inputs,
            backend_diagnostics: backend_diagnostics.clone(),
        });
        let after = publication.coordinator.status().backend.freshness;
        (before, disposition, after)
    };

    if !matches!(disposition, ObservationDisposition::NoChange) {
        session.events().publish();
    }
    match (before, after) {
        (BackendFreshness::Active, BackendFreshness::RestartRequired) => {
            let status = session.status();
            notices.emit(HostNotice::BackendRestartRequired {
                changed_inputs: status.backend.changed_inputs,
                diagnostics: backend_diagnostics,
            });
        }
        (BackendFreshness::RestartRequired, BackendFreshness::Active) => {
            notices.emit(HostNotice::BackendReverted);
        }
        _ => {}
    }
    disposition
}

fn prepared_client_diagnostics(candidate: &crate::PreparedClient) -> Vec<String> {
    let diagnostics = candidate.diagnostics();
    [diagnostics.editor, diagnostics.play]
        .into_iter()
        .filter(|value| !value.is_null())
        .map(|value| serde_json::to_string(value).unwrap_or_else(|_| value.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Instant;

    use axum::routing::get;
    use axum::Router;
    use spock_project::{
        load_project_from, minimal_uhura_client_template, scaffold_plan, ProjectManifest,
    };
    use tempfile::tempdir;

    use super::*;
    use crate::{ClientFreshness, ProjectStatus};

    const NETWORK_TEST_DEADLINE: Duration = Duration::from_secs(7);
    const NETWORK_TEST_POLL: Duration = Duration::from_millis(25);

    fn backend_project(root: &Path) -> ProjectLayout {
        fs::create_dir(root.join("backend")).unwrap();
        fs::write(root.join("backend/app.spock"), "").unwrap();
        fs::write(
            root.join("spock.toml"),
            ProjectManifest::new("demo", "backend", "app.spock", None)
                .unwrap()
                .to_toml_string(),
        )
        .unwrap();
        load_project_from(root).unwrap()
    }

    fn full_stack_project(root: &Path) -> ProjectLayout {
        let template = minimal_uhura_client_template();
        let plan = scaffold_plan(root, "demo", Some(&template)).unwrap();
        for write in plan.writes() {
            let path = root.join(write.relative_path.as_path());
            fs::create_dir_all(path.parent().expect("scaffold file parent")).unwrap();
            fs::write(path, &write.contents).unwrap();
        }
        load_project_from(root).unwrap()
    }

    fn write_client_template(root: &Path) {
        for file in minimal_uhura_client_template().files() {
            let path = root.join(file.path().as_path());
            fs::create_dir_all(path.parent().expect("client file parent")).unwrap();
            fs::write(path, file.contents()).unwrap();
        }
    }

    #[cfg(unix)]
    fn replace_symlink(link: &Path, target: &Path) {
        use std::os::unix::fs::symlink;

        let replacement = link.with_extension("next-link");
        let _ = fs::remove_file(&replacement);
        symlink(target, &replacement).unwrap();
        fs::rename(replacement, link).unwrap();
    }

    fn dummy_uhura_assets(root: &Path) -> UhuraAssetRoots {
        let web = root.join("web");
        let wasm = root.join("wasm");
        fs::create_dir_all(web.join("assets")).unwrap();
        fs::create_dir_all(&wasm).unwrap();
        fs::write(
            web.join("index.html"),
            r#"<!doctype html><script type="module" src="/assets/app.js"></script>"#,
        )
        .unwrap();
        fs::write(web.join("assets/app.js"), "export {};\n").unwrap();
        fs::write(wasm.join("uhura_wasm.js"), "export {};\n").unwrap();
        fs::write(wasm.join("uhura_wasm_bg.wasm"), b"wasm").unwrap();
        UhuraAssetRoots { web, wasm }
    }

    async fn status_until(
        client: &reqwest::Client,
        address: SocketAddr,
        deadline: Instant,
        description: &str,
        predicate: impl Fn(&ProjectStatus) -> bool,
    ) -> ProjectStatus {
        let url = format!("http://{address}/~project/status");
        loop {
            let last_observation = match client.get(&url).send().await {
                Ok(response) if response.status() == reqwest::StatusCode::OK => {
                    match response.json::<ProjectStatus>().await {
                        Ok(status) => {
                            if predicate(&status) {
                                return status;
                            }
                            format!("{status:#?}")
                        }
                        Err(error) => format!("invalid status JSON: {error}"),
                    }
                }
                Ok(response) => format!("status endpoint returned {}", response.status()),
                Err(error) => error.to_string(),
            };
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {description}; last observation: {last_observation}"
            );
            tokio::time::sleep(NETWORK_TEST_POLL).await;
        }
    }

    async fn ok_bytes(client: &reqwest::Client, address: SocketAddr, path: &str) -> Vec<u8> {
        let response = client
            .get(format!("http://{address}{path}"))
            .send()
            .await
            .unwrap_or_else(|error| panic!("GET {path} failed: {error}"));
        assert_eq!(response.status(), reqwest::StatusCode::OK, "GET {path}");
        response.bytes().await.unwrap().to_vec()
    }

    #[tokio::test]
    async fn graceful_shutdown_waits_for_an_accepted_request_to_finish() {
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let handler_entered = Arc::clone(&entered);
        let handler_release = Arc::clone(&release);
        let router = Router::new().route(
            "/slow",
            get(move || {
                let entered = Arc::clone(&handler_entered);
                let release = Arc::clone(&handler_release);
                async move {
                    entered.notify_one();
                    release.notified().await;
                    "finished"
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let (shutdown_started_tx, shutdown_started_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(serve_router_until_shutdown(
            listener,
            router,
            async move {
                let _ = shutdown_rx.await;
            },
            move || {
                let _ = shutdown_started_tx.send(());
            },
        ));
        let request = tokio::spawn(async move {
            reqwest::get(format!("http://{address}/slow"))
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        });

        tokio::time::timeout(Duration::from_secs(2), entered.notified())
            .await
            .expect("slow request entered its handler");
        shutdown_tx.send(()).unwrap();
        shutdown_started_rx
            .await
            .expect("graceful-shutdown callback ran");
        assert!(
            !server.is_finished(),
            "server returned while an accepted request was still active"
        );

        release.notify_one();
        assert_eq!(request.await.unwrap(), "finished");
        tokio::time::timeout(Duration::from_secs(2), server)
            .await
            .expect("server drained the completed request")
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn fixed_server_binds_ephemeral_port_and_releases_it_on_shutdown() {
        let temp = tempdir().unwrap();
        let layout = backend_project(temp.path());
        let options = ServeOptions {
            bind: "127.0.0.1:0".parse().unwrap(),
            ..ServeOptions::default()
        };
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let ready_tx = std::sync::Mutex::new(Some(ready_tx));
        let notices = HostNoticeSink::new(move |notice| {
            if let HostNotice::Listening { address, .. } = notice {
                if let Some(sender) = ready_tx.lock().unwrap().take() {
                    let _ = sender.send(address);
                }
            }
        });
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(serve_project(
            layout,
            HostMode::Start,
            options,
            notices,
            async move {
                let _ = shutdown_rx.await;
            },
        ));
        let address = ready_rx.await.unwrap();
        let client = reqwest::Client::new();
        let health = client
            .get(format!("http://{address}/~health"))
            .send()
            .await
            .unwrap();
        assert_eq!(health.status(), reqwest::StatusCode::OK);
        let events = client
            .get(format!("http://{address}/~project/events"))
            .send()
            .await
            .expect("open project event stream");
        assert_eq!(events.status(), reqwest::StatusCode::OK);

        shutdown_tx.send(()).unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .expect("open SSE stream did not prevent graceful shutdown")
                .unwrap()
                .unwrap()
                .local_address,
            address
        );
        drop(events);
        let rebound = tokio::net::TcpListener::bind(address).await.unwrap();
        drop(rebound);
    }

    #[tokio::test]
    async fn dev_server_keeps_one_port_and_last_good_generations_across_source_changes() {
        let project = tempdir().unwrap();
        let assets = tempdir().unwrap();
        let layout = full_stack_project(project.path());
        let options = ServeOptions {
            bind: "127.0.0.1:0".parse().unwrap(),
            asset_roots: Some(dummy_uhura_assets(assets.path())),
            poll_interval: NETWORK_TEST_POLL,
            ..ServeOptions::default()
        };
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let ready_tx = std::sync::Mutex::new(Some(ready_tx));
        let notices = HostNoticeSink::new(move |notice| {
            if let HostNotice::Listening { address, .. } = notice {
                if let Some(sender) = ready_tx.lock().unwrap().take() {
                    let _ = sender.send(address);
                }
            }
        });
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(serve_project(
            layout,
            HostMode::Dev,
            options,
            notices,
            async move {
                let _ = shutdown_rx.await;
            },
        ));
        let address = ready_rx.await.unwrap();
        let client = reqwest::Client::new();
        let deadline = Instant::now() + NETWORK_TEST_DEADLINE;

        let initial = status_until(
            &client,
            address,
            deadline,
            "the initial full-stack generation",
            |status| {
                status.backend.freshness == BackendFreshness::Active
                    && status.client.freshness == ClientFreshness::Active
            },
        )
        .await;
        let backend_generation = initial.backend.generation_id;
        let initial_observed_revision = initial.observed.revision;

        for path in [
            "/",
            "/play",
            "/api/editor/state",
            "/api/play/ir.json",
            "/~contract",
            "/~project/status",
            "/~health",
        ] {
            let _ = ok_bytes(&client, address, path).await;
        }
        let initial_play = ok_bytes(&client, address, "/api/play/ir.json").await;
        let initial_contract = ok_bytes(&client, address, "/~contract").await;

        let client_source = project.path().join("client/app/home/page.uhura");
        let original_client_source = fs::read(&client_source).unwrap();
        fs::write(&client_source, "this is not valid uhura\n").unwrap();
        let rejected = status_until(
            &client,
            address,
            deadline,
            "a rejected client candidate with the last good Play generation",
            |status| status.client.freshness == ClientFreshness::RejectedLastGood,
        )
        .await;
        assert!(rejected.observed.revision > initial_observed_revision);
        assert_eq!(rejected.backend.generation_id, backend_generation);
        assert_eq!(
            ok_bytes(&client, address, "/api/play/ir.json").await,
            initial_play,
            "a rejected edit must not replace the last good Play artifact"
        );

        fs::write(&client_source, original_client_source).unwrap();
        let restored = status_until(
            &client,
            address,
            deadline,
            "a restored active client generation",
            |status| {
                status.client.freshness == ClientFreshness::Active
                    && status.observed.revision > rejected.observed.revision
            },
        )
        .await;
        assert_eq!(restored.backend.generation_id, backend_generation);
        assert_eq!(
            restored
                .client
                .active
                .as_ref()
                .expect("restored active client")
                .observed_revision,
            restored.observed.revision
        );

        let backend_source = project.path().join("backend/app.spock");
        let original_backend_source = fs::read(&backend_source).unwrap();
        let mut changed_backend_source = original_backend_source.clone();
        changed_backend_source.extend_from_slice(b"// requires a restart\n");
        fs::write(&backend_source, changed_backend_source).unwrap();
        let restart_required = status_until(
            &client,
            address,
            deadline,
            "a backend restart-required observation",
            |status| status.backend.freshness == BackendFreshness::RestartRequired,
        )
        .await;
        assert_eq!(restart_required.backend.generation_id, backend_generation);
        assert_eq!(
            restart_required.active_project.backend_generation_id,
            backend_generation
        );
        assert_eq!(
            ok_bytes(&client, address, "/~contract").await,
            initial_contract,
            "backend observation must not replace the active generation"
        );

        fs::write(&backend_source, original_backend_source).unwrap();
        let reverted = status_until(
            &client,
            address,
            deadline,
            "the exact backend reversion",
            |status| status.backend.freshness == BackendFreshness::Active,
        )
        .await;
        assert_eq!(reverted.backend.generation_id, backend_generation);
        assert_eq!(
            ok_bytes(&client, address, "/~contract").await,
            initial_contract
        );

        shutdown_tx.send(()).unwrap();
        drop(client);
        let outcome = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("framework host shutdown timed out")
            .unwrap()
            .unwrap();
        assert_eq!(outcome.local_address, address);
        let rebound = tokio::net::TcpListener::bind(address).await.unwrap();
        drop(rebound);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn dev_observes_client_root_retargets_and_rejects_escapes_without_backend_swap() {
        use std::os::unix::fs::symlink;

        let project = tempdir().unwrap();
        let assets = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::create_dir(project.path().join("backend")).unwrap();
        fs::write(project.path().join("backend/app.spock"), "").unwrap();
        write_client_template(&project.path().join("client-a"));
        write_client_template(&project.path().join("client-b"));
        let retargeted_page = project.path().join("client-b/app/home/page.uhura");
        let retargeted_source = fs::read_to_string(&retargeted_page)
            .unwrap()
            .replace("Your app is running.", "The retargeted app is running.");
        fs::write(&retargeted_page, retargeted_source).unwrap();
        write_client_template(outside.path());
        symlink("client-a", project.path().join("client")).unwrap();
        fs::write(
            project.path().join("spock.toml"),
            ProjectManifest::new("demo", "backend", "app.spock", Some("client"))
                .unwrap()
                .to_toml_string(),
        )
        .unwrap();
        let layout = load_project_from(project.path()).unwrap();
        let options = ServeOptions {
            bind: "127.0.0.1:0".parse().unwrap(),
            asset_roots: Some(dummy_uhura_assets(assets.path())),
            poll_interval: NETWORK_TEST_POLL,
            ..ServeOptions::default()
        };
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let ready_tx = std::sync::Mutex::new(Some(ready_tx));
        let notices = HostNoticeSink::new(move |notice| {
            if let HostNotice::Listening { address, .. } = notice {
                if let Some(sender) = ready_tx.lock().unwrap().take() {
                    let _ = sender.send(address);
                }
            }
        });
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(serve_project(
            layout,
            HostMode::Dev,
            options,
            notices,
            async move {
                let _ = shutdown_rx.await;
            },
        ));
        let address = ready_rx.await.unwrap();
        let client = reqwest::Client::new();
        let initial = status_until(
            &client,
            address,
            Instant::now() + NETWORK_TEST_DEADLINE,
            "initial symlinked client generation",
            |status| status.client.freshness == ClientFreshness::Active,
        )
        .await;
        let backend_generation = initial.backend.generation_id;
        let initial_play = ok_bytes(&client, address, "/api/play/ir.json").await;

        replace_symlink(&project.path().join("client"), outside.path());
        let rejected = status_until(
            &client,
            address,
            Instant::now() + NETWORK_TEST_DEADLINE,
            "escaping client-root observation",
            |status| {
                status.backend.generation_id == backend_generation
                    && status.backend.freshness == BackendFreshness::Active
                    && status.backend.diagnostics.is_empty()
                    && status.client.freshness == ClientFreshness::RejectedLastGood
                    && status
                        .client
                        .latest_attempt
                        .as_ref()
                        .is_some_and(|attempt| {
                            attempt
                                .diagnostics
                                .iter()
                                .any(|diagnostic| diagnostic.contains("SPP011"))
                        })
            },
        )
        .await;
        assert_eq!(
            ok_bytes(&client, address, "/api/play/ir.json").await,
            initial_play,
            "an escaping retarget must retain the last-good client"
        );

        replace_symlink(&project.path().join("client"), Path::new("client-b"));
        let recovered = status_until(
            &client,
            address,
            Instant::now() + NETWORK_TEST_DEADLINE,
            "safe client-root retarget publication",
            |status| {
                status.backend.generation_id == backend_generation
                    && status.backend.freshness == BackendFreshness::Active
                    && status.client.freshness == ClientFreshness::Active
                    && status.observed.revision > rejected.observed.revision
            },
        )
        .await;
        assert_eq!(recovered.backend.generation_id, backend_generation);
        assert_ne!(
            ok_bytes(&client, address, "/api/play/ir.json").await,
            initial_play,
            "safe retarget must publish the newly resolved client tree"
        );

        shutdown_tx.send(()).unwrap();
        drop(client);
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("symlinked dev host shutdown timed out")
            .unwrap()
            .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn coherent_frames_re_resolve_backend_root_and_entry_symlinks_and_reject_escapes() {
        use std::os::unix::fs::symlink;

        let project = tempdir().unwrap();
        let outside = tempdir().unwrap();
        for directory in ["backend-a", "backend-b"] {
            fs::create_dir(project.path().join(directory)).unwrap();
        }
        fs::write(
            project.path().join("backend-a/source-a.spock"),
            "// active backend\n",
        )
        .unwrap();
        fs::write(
            project.path().join("backend-b/source-b.spock"),
            "table beta { key id: uuid = auto }\n",
        )
        .unwrap();
        fs::write(
            project.path().join("backend-b/source-c.spock"),
            "table gamma { key id: uuid = auto }\n",
        )
        .unwrap();
        symlink("source-a.spock", project.path().join("backend-a/app.spock")).unwrap();
        symlink("source-b.spock", project.path().join("backend-b/app.spock")).unwrap();
        symlink("backend-a", project.path().join("backend")).unwrap();
        fs::write(
            project.path().join("spock.toml"),
            ProjectManifest::new("demo", "backend", "app.spock", None)
                .unwrap()
                .to_toml_string(),
        )
        .unwrap();
        let layout = load_project_from(project.path()).unwrap();
        let prepared = prepare_project(layout, HostMode::Dev, None, None).unwrap();
        let backend_generation = prepared.session.status().backend.generation_id;
        assert!(prepared.session.backend().contract().tables.is_empty());

        replace_symlink(&project.path().join("backend"), Path::new("backend-b"));
        let root_retarget = capture_frame(&prepared.layout).unwrap();
        assert_eq!(
            root_retarget
                .backend
                .captured_backend()
                .expect("safe backend-root retarget")
                .source(),
            b"table beta { key id: uuid = auto }\n"
        );
        let first_observed = root_retarget.backend.fingerprint().clone();
        apply_frame(
            &prepared.session,
            &prepared.active_backend,
            &prepared.active_topology,
            &root_retarget,
            &HostNoticeSink::default(),
        );
        assert_eq!(
            prepared.session.status().backend.freshness,
            BackendFreshness::RestartRequired
        );
        assert_eq!(
            prepared.session.status().backend.generation_id,
            backend_generation
        );
        assert!(prepared.session.backend().contract().tables.is_empty());

        replace_symlink(
            &project.path().join("backend-b/app.spock"),
            Path::new("source-c.spock"),
        );
        let entry_retarget = capture_frame(&prepared.layout).unwrap();
        assert_eq!(
            entry_retarget
                .backend
                .captured_backend()
                .expect("safe backend-entry retarget")
                .source(),
            b"table gamma { key id: uuid = auto }\n"
        );
        assert_ne!(entry_retarget.backend.fingerprint(), &first_observed);
        apply_frame(
            &prepared.session,
            &prepared.active_backend,
            &prepared.active_topology,
            &entry_retarget,
            &HostNoticeSink::default(),
        );
        assert_eq!(
            prepared.session.status().backend.generation_id,
            backend_generation
        );
        assert!(prepared.session.backend().contract().tables.is_empty());

        fs::write(outside.path().join("app.spock"), "table escaped {}\n").unwrap();
        replace_symlink(&project.path().join("backend"), outside.path());
        let escaped = capture_frame(&prepared.layout).unwrap();
        assert!(!escaped.backend.is_valid());
        apply_frame(
            &prepared.session,
            &prepared.active_backend,
            &prepared.active_topology,
            &escaped,
            &HostNoticeSink::default(),
        );
        let status = prepared.session.status();
        assert_eq!(status.backend.generation_id, backend_generation);
        assert_eq!(status.backend.freshness, BackendFreshness::RestartRequired);
        assert!(status
            .backend
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.contains("SPH004") || diagnostic.contains("SPP011") }));
        assert!(prepared.session.backend().contract().tables.is_empty());

        fs::write(
            project.path().join("spock.toml"),
            ProjectManifest::new("demo", "backend-b", "app.spock", None)
                .unwrap()
                .to_toml_string(),
        )
        .unwrap();
        let manifest_retarget = capture_frame(&prepared.layout).unwrap();
        assert_eq!(
            manifest_retarget
                .backend
                .captured_backend()
                .expect("valid manifest path retarget")
                .source(),
            b"table gamma { key id: uuid = auto }\n"
        );
        assert_ne!(manifest_retarget.topology, prepared.active_topology);
        apply_frame(
            &prepared.session,
            &prepared.active_backend,
            &prepared.active_topology,
            &manifest_retarget,
            &HostNoticeSink::default(),
        );
        let status = prepared.session.status();
        assert_eq!(status.backend.generation_id, backend_generation);
        assert_eq!(status.backend.freshness, BackendFreshness::RestartRequired);
        assert!(status.backend.diagnostics.is_empty());
        assert!(prepared.session.backend().contract().tables.is_empty());
    }

    #[test]
    fn backend_observation_marks_restart_and_exact_reversion_without_runtime_work() {
        let temp = tempdir().unwrap();
        let layout = backend_project(temp.path());
        let prepared = prepare_project(layout, HostMode::Dev, None, None).unwrap();
        let notices = HostNoticeSink::default();

        fs::write(
            prepared.layout.root.join("backend/app.spock"),
            "// changed\n",
        )
        .unwrap();
        let changed = capture_frame(&prepared.layout).unwrap();
        apply_frame(
            &prepared.session,
            &prepared.active_backend,
            &prepared.active_topology,
            &changed,
            &notices,
        );
        assert_eq!(
            prepared.session.status().backend.freshness,
            BackendFreshness::RestartRequired
        );

        fs::write(prepared.layout.root.join("backend/app.spock"), "").unwrap();
        let reverted = capture_frame(&prepared.layout).unwrap();
        apply_frame(
            &prepared.session,
            &prepared.active_backend,
            &prepared.active_topology,
            &reverted,
            &notices,
        );
        assert_eq!(
            prepared.session.status().backend.freshness,
            BackendFreshness::Active
        );
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
