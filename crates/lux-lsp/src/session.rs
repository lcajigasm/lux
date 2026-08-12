//! Lifecycle of a single language server process: spawn, capability
//! handshake, crash-triggered restart, and clean shutdown.

use std::collections::{BTreeMap, HashMap};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::lsp_types::notification::{Progress, PublishDiagnostics};
use async_lsp::lsp_types::request::{
    InlayHintRefreshRequest, RegisterCapability, SemanticTokensRefresh, WorkDoneProgressCreate,
    WorkspaceConfiguration,
};
use async_lsp::lsp_types::{
    ClientCapabilities, CodeActionCapabilityResolveSupport, CodeActionClientCapabilities,
    CodeActionKindLiteralSupport, CodeActionLiteralSupport, CompletionClientCapabilities,
    CompletionItemCapability, CompletionItemCapabilityResolveSupport, Diagnostic, InitializeParams,
    InitializedParams, NumberOrString, ProgressParams, ProgressParamsValue,
    PublishDiagnosticsParams, ServerCapabilities, TextDocumentClientCapabilities, Url,
    WindowClientCapabilities, WorkDoneProgress, WorkspaceFolder,
};
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router::Router;
use async_lsp::{LanguageServer, MainLoop, ServerSocket};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tower::ServiceBuilder;

use crate::discovery::{self, ServerSpec};
use crate::framing;

/// A restart is attempted this many times before a session is marked
/// permanently [`ServerStatus::Failed`].
const MAX_RESTARTS: u32 = 3;
const SHUTDOWN_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const SHUTDOWN_WAIT_TIMEOUT: Duration = Duration::from_secs(3);

/// Real, observable state of one language server process.
#[derive(Debug, Clone, Default)]
pub struct ServerState {
    pub status: ServerStatus,
    pub capabilities: Option<ServerCapabilities>,
    /// In-flight `$/progress` operations, keyed by their token.
    pub progress: HashMap<String, ProgressInfo>,
    /// Monotonic counter of every `$/progress` notification received,
    /// regardless of whether the operation it belongs to has since ended --
    /// a race-free way to confirm progress reporting is wired up.
    pub progress_events_seen: u32,
    /// OS process id of the currently running server, if any.
    pub pid: Option<u32>,
    /// Total number of times this session has restarted after a crash.
    pub restarts: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ServerStatus {
    #[default]
    Starting,
    Initializing,
    Running,
    ShuttingDown,
    Stopped,
    Crashed {
        restarts: u32,
    },
    Failed {
        reason: String,
    },
}

impl ServerStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, ServerStatus::Stopped | ServerStatus::Failed { .. })
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProgressInfo {
    pub title: Option<String>,
    pub message: Option<String>,
    pub percentage: Option<u32>,
}

/// A managed language server: one process, one language, one project root.
pub struct ServerSession {
    language_id: String,
    root: PathBuf,
    state_rx: watch::Receiver<ServerState>,
    socket_rx: watch::Receiver<Option<ServerSocket>>,
    diagnostics_rx: watch::Receiver<Arc<BTreeMap<Url, Vec<Diagnostic>>>>,
    stop_tx: StdMutex<Option<oneshot::Sender<()>>>,
    supervisor: JoinHandle<()>,
}

impl ServerSession {
    /// Spawn a session directly for an arbitrary [`ServerSpec`], bypassing
    /// [`crate::ServerRegistry`]'s fixed, known-language lookup -- the
    /// primitive the registry itself is built on, for a caller that needs
    /// a language server outside `KNOWN_SERVERS` (or, in this crate's own
    /// tests, a scripted fake one).
    pub fn spawn(spec: ServerSpec, root: PathBuf) -> Self {
        let (state_tx, state_rx) = watch::channel(ServerState::default());
        let (socket_tx, socket_rx) = watch::channel(None);
        let (diagnostics_tx, diagnostics_rx) = watch::channel(Arc::new(BTreeMap::new()));
        let (stop_tx, stop_rx) = oneshot::channel();
        let supervisor = tokio::spawn(supervise(
            spec,
            root.clone(),
            state_tx,
            socket_tx,
            diagnostics_tx,
            stop_rx,
        ));
        Self {
            language_id: spec.language_id.to_string(),
            root,
            state_rx,
            socket_rx,
            diagnostics_rx,
            stop_tx: StdMutex::new(Some(stop_tx)),
            supervisor,
        }
    }

    /// The live handle to talk to the running server, if it's currently
    /// connected. Used by `sync` to send `textDocument/*` notifications.
    pub(crate) fn socket(&self) -> Option<ServerSocket> {
        self.socket_rx.borrow().clone()
    }

    /// Subscribe to connect/disconnect transitions -- event-driven, no
    /// polling. Used by `sync` to replay `didOpen` after a crash restart.
    pub(crate) fn watch_socket(&self) -> watch::Receiver<Option<ServerSocket>> {
        self.socket_rx.clone()
    }

    /// The current `publishDiagnostics` state, one full replacement list
    /// per document, keyed by its URI. Cheap to call: an `Arc` clone, not a
    /// deep copy. Used by `diagnostics` to build the filterable/navigable
    /// problems view.
    pub(crate) fn diagnostics(&self) -> Arc<BTreeMap<Url, Vec<Diagnostic>>> {
        self.diagnostics_rx.borrow().clone()
    }

    /// Subscribe to diagnostics updates -- event-driven, no polling.
    pub(crate) fn watch_diagnostics(&self) -> watch::Receiver<Arc<BTreeMap<Url, Vec<Diagnostic>>>> {
        self.diagnostics_rx.clone()
    }

    pub fn language_id(&self) -> &str {
        &self.language_id
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn status(&self) -> ServerStatus {
        self.state_rx.borrow().status.clone()
    }

    pub fn is_terminal(&self) -> bool {
        self.status().is_terminal()
    }

    /// Subscribe to state changes -- event-driven, no polling.
    pub fn watch(&self) -> watch::Receiver<ServerState> {
        self.state_rx.clone()
    }

    /// Request a clean shutdown (`shutdown` + `exit`) and wait for it to
    /// finish. Idempotent: safe to call more than once, or on a session
    /// that already stopped or failed on its own.
    pub async fn stop(&self) {
        if let Some(tx) = self.stop_tx.lock().expect("stop_tx poisoned").take() {
            let _ = tx.send(());
        }
        let mut rx = self.watch();
        while !rx.borrow().status.is_terminal() {
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Drop for ServerSession {
    fn drop(&mut self) {
        // Best-effort safety net if `stop()` was never called: aborting the
        // supervisor drops its child `async_process::Child`, which was
        // spawned with `kill_on_drop(true)`.
        self.supervisor.abort();
    }
}

async fn supervise(
    spec: ServerSpec,
    root: PathBuf,
    state_tx: watch::Sender<ServerState>,
    socket_tx: watch::Sender<Option<ServerSocket>>,
    diagnostics_tx: watch::Sender<Arc<BTreeMap<Url, Vec<Diagnostic>>>>,
    mut stop_rx: oneshot::Receiver<()>,
) {
    let mut restarts: u32 = 0;
    loop {
        state_tx.send_modify(|s| s.status = ServerStatus::Starting);

        let Some(binary) = discovery::discover_command(spec.command) else {
            state_tx.send_modify(|s| {
                s.status = ServerStatus::Failed {
                    reason: format!("`{}` not found on PATH", spec.command),
                };
            });
            return;
        };

        let spawned = async_process::Command::new(&binary)
            .args(spec.args)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(err) => {
                state_tx.send_modify(|s| {
                    s.status = ServerStatus::Failed {
                        reason: format!("spawn failed: {err}"),
                    }
                });
                return;
            }
        };
        let pid = child.id();
        let stdout = child.stdout.take().expect("piped stdout");
        let stdin = child.stdin.take().expect("piped stdin");
        // Guarded, not the server's raw stdout: async-lsp's own message
        // parser has no cap on an attacker- or bug-claimed Content-Length
        // before allocating for it (found by this phase's fuzzer -- see
        // `framing`'s doc comment).
        let guarded_stdout = framing::guard_stream(futures::io::BufReader::new(stdout));

        let (mainloop, mut server) = MainLoop::new_client(|_server| {
            let mut router = Router::new(RouterState {
                state_tx: state_tx.clone(),
                diagnostics_tx: diagnostics_tx.clone(),
            });
            router
                .notification::<Progress>(|st, params| {
                    record_progress(&st.state_tx, params);
                    ControlFlow::Continue(())
                })
                .notification::<PublishDiagnostics>(|st, params| {
                    record_diagnostics(&st.diagnostics_tx, params);
                    ControlFlow::Continue(())
                })
                .unhandled_notification(|_, _| ControlFlow::Continue(()))
                .request::<WorkDoneProgressCreate, _>(|_, _| async { Ok(()) })
                .request::<RegisterCapability, _>(|_, _| async { Ok(()) })
                .request::<SemanticTokensRefresh, _>(|_, _| async { Ok(()) })
                .request::<InlayHintRefreshRequest, _>(|_, _| async { Ok(()) })
                .request::<WorkspaceConfiguration, _>(|_, params| async move {
                    Ok(vec![serde_json::Value::Null; params.items.len()])
                });

            ServiceBuilder::new()
                .layer(async_lsp::tracing::TracingLayer::default())
                .layer(CatchUnwindLayer::default())
                .layer(ConcurrencyLayer::default())
                .service(router)
        });
        let mut mainloop_task: JoinHandle<Result<(), async_lsp::Error>> =
            tokio::spawn(async move { mainloop.run_buffered(guarded_stdout, stdin).await });

        state_tx.send_modify(|s| {
            s.status = ServerStatus::Initializing;
            s.pid = Some(pid);
        });

        let root_uri = Url::from_file_path(&root).expect("root must be an absolute path");
        // `root_uri` is spec-deprecated in favor of `workspace_folders`, but
        // still sent alongside it: typescript-language-server 5.3.0 (one of
        // Phase 7's five required servers) only resolves its `typescript`
        // dependency from `root_uri` and fails the whole handshake without
        // it, `workspace_folders` alone isn't enough in practice -- found
        // via `crates/lux-lsp/tests/multi_server.rs`.
        #[allow(deprecated)]
        let init = server
            .initialize(InitializeParams {
                root_uri: Some(root_uri.clone()),
                workspace_folders: Some(vec![WorkspaceFolder {
                    uri: root_uri,
                    name: "root".into(),
                }]),
                capabilities: ClientCapabilities {
                    window: Some(WindowClientCapabilities {
                        work_done_progress: Some(true),
                        ..Default::default()
                    }),
                    text_document: Some(TextDocumentClientCapabilities {
                        completion: Some(CompletionClientCapabilities {
                            completion_item: Some(CompletionItemCapability {
                                snippet_support: Some(true),
                                resolve_support: Some(CompletionItemCapabilityResolveSupport {
                                    properties: vec!["documentation".into(), "detail".into()],
                                }),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        code_action: Some(CodeActionClientCapabilities {
                            code_action_literal_support: Some(CodeActionLiteralSupport {
                                code_action_kind: CodeActionKindLiteralSupport {
                                    value_set: vec![
                                        "quickfix".into(),
                                        "refactor".into(),
                                        "refactor.extract".into(),
                                        "refactor.inline".into(),
                                        "refactor.rewrite".into(),
                                        "source".into(),
                                        "source.organizeImports".into(),
                                        "source.fixAll".into(),
                                    ],
                                },
                            }),
                            data_support: Some(true),
                            resolve_support: Some(CodeActionCapabilityResolveSupport {
                                properties: vec!["edit".into()],
                            }),
                            is_preferred_support: Some(true),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await;

        let capabilities = match init {
            Ok(result) => result.capabilities,
            Err(err) => {
                let _ = child.kill();
                let _ = mainloop_task.await;
                restarts += 1;
                if restarts > MAX_RESTARTS {
                    state_tx.send_modify(|s| {
                        s.status = ServerStatus::Failed {
                            reason: format!("initialize failed: {err}"),
                        };
                        s.pid = None;
                    });
                    return;
                }
                state_tx.send_modify(|s| {
                    s.status = ServerStatus::Crashed { restarts };
                    s.restarts = restarts;
                    s.pid = None;
                });
                continue;
            }
        };
        let _ = server.initialized(InitializedParams {});
        state_tx.send_modify(|s| {
            s.status = ServerStatus::Running;
            s.capabilities = Some(capabilities.clone());
        });
        let _ = socket_tx.send(Some(server.clone()));

        tokio::select! {
            _ = &mut mainloop_task => {
                let _ = socket_tx.send(None);
                let _ = diagnostics_tx.send(Arc::new(BTreeMap::new()));
                restarts += 1;
                if restarts > MAX_RESTARTS {
                    state_tx.send_modify(|s| {
                        s.status = ServerStatus::Failed {
                            reason: format!("server exited unexpectedly {restarts} time(s)"),
                        };
                        s.pid = None;
                    });
                    return;
                }
                state_tx.send_modify(|s| {
                    s.status = ServerStatus::Crashed { restarts };
                    s.restarts = restarts;
                    s.pid = None;
                });
            }
            _ = &mut stop_rx => {
                let _ = socket_tx.send(None);
                let _ = diagnostics_tx.send(Arc::new(BTreeMap::new()));
                state_tx.send_modify(|s| {
                    s.status = ServerStatus::ShuttingDown;
                });
                let _ = tokio::time::timeout(SHUTDOWN_REQUEST_TIMEOUT, server.shutdown(())).await;
                let _ = server.exit(());
                if tokio::time::timeout(SHUTDOWN_WAIT_TIMEOUT, &mut mainloop_task).await.is_err() {
                    let _ = child.kill();
                    let _ = mainloop_task.await;
                }
                state_tx.send_modify(|s| {
                    s.status = ServerStatus::Stopped;
                    s.pid = None;
                });
                return;
            }
        }
    }
}

struct RouterState {
    state_tx: watch::Sender<ServerState>,
    diagnostics_tx: watch::Sender<Arc<BTreeMap<Url, Vec<Diagnostic>>>>,
}

/// `publishDiagnostics` always replaces the *full* diagnostic set for its
/// document -- never a delta -- per the LSP spec. An empty list means the
/// server has nothing left to report for that document, so it's removed
/// rather than kept around as a stale empty entry.
fn record_diagnostics(
    diagnostics_tx: &watch::Sender<Arc<BTreeMap<Url, Vec<Diagnostic>>>>,
    params: PublishDiagnosticsParams,
) {
    diagnostics_tx.send_modify(|current| {
        let map = Arc::make_mut(current);
        if params.diagnostics.is_empty() {
            map.remove(&params.uri);
        } else {
            map.insert(params.uri, params.diagnostics);
        }
    });
}

fn record_progress(state_tx: &watch::Sender<ServerState>, params: ProgressParams) {
    let key = match &params.token {
        NumberOrString::Number(n) => n.to_string(),
        NumberOrString::String(s) => s.clone(),
    };
    state_tx.send_modify(|state| {
        state.progress_events_seen = state.progress_events_seen.saturating_add(1);
        match params.value {
            ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(begin)) => {
                state.progress.insert(
                    key,
                    ProgressInfo {
                        title: Some(begin.title),
                        message: begin.message,
                        percentage: begin.percentage,
                    },
                );
            }
            ProgressParamsValue::WorkDone(WorkDoneProgress::Report(report)) => {
                let entry = state.progress.entry(key).or_default();
                if report.message.is_some() {
                    entry.message = report.message;
                }
                if report.percentage.is_some() {
                    entry.percentage = report.percentage;
                }
            }
            ProgressParamsValue::WorkDone(WorkDoneProgress::End(_)) => {
                state.progress.remove(&key);
            }
        }
    });
}
