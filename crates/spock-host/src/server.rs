use std::future::{Future, IntoFuture};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

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

    let server = axum::serve(listener, router).into_future();
    tokio::pin!(server);
    tokio::pin!(shutdown);
    let server_result = tokio::select! {
        result = &mut server => result.map_err(ServeError::Serve),
        () = &mut shutdown => Ok(()),
    };

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
            let Some(snapshot) = frame.client.as_ref() else {
                force_client_build = false;
                continue;
            };
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

struct ObservationFrame {
    topology: Fingerprint,
    backend: BackendObservation,
    client: Option<ProjectSourceSnapshot>,
}

fn capture_frame(layout: &ProjectLayout) -> Result<ObservationFrame, String> {
    for _ in 0..COHERENT_FRAME_ATTEMPTS {
        let topology_before = topology_fingerprint(&layout.manifest_path);
        let backend_before = observe_backend(layout);
        let client = layout
            .client
            .as_ref()
            .map(|client| capture_stable_client(client.root.absolute()))
            .transpose()?;
        let backend_after = observe_backend(layout);
        let topology_after = topology_fingerprint(&layout.manifest_path);

        // Capture the client between matching topology/backend boundaries.
        // Returning only this sandwiched state prevents one revision from
        // combining subsystem snapshots that never coexisted.
        if topology_before == topology_after
            && backend_before.fingerprint() == backend_after.fingerprint()
        {
            return Ok(ObservationFrame {
                topology: topology_after,
                backend: backend_after,
                client,
            });
        }
    }
    Err(format!(
        "project inputs under {} did not remain unchanged across {COHERENT_FRAME_ATTEMPTS} coherent captures",
        layout.root.display()
    ))
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
    let backend_diagnostics = frame
        .backend
        .diagnostics()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let client = frame.client.as_ref().map(client_source_fingerprint);

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
    use std::time::Instant;

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
        let health = reqwest::get(format!("http://{address}/~health"))
            .await
            .unwrap();
        assert_eq!(health.status(), reqwest::StatusCode::OK);

        shutdown_tx.send(()).unwrap();
        assert_eq!(task.await.unwrap().unwrap().local_address, address);
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
