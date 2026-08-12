//! A minimal, fully scripted LSP *server* -- the "servidor de mentira que
//! ejerza el protocolo" Phase 7's acceptance criteria names separately
//! from the fuzzer ("v0 no tenía ninguno" applies to both). Every other
//! integration test in this crate drives a real server (rust-analyzer et
//! al.); this binary exists to test the opposite direction precisely --
//! scenarios a real server can't be coerced into on demand, deterministically:
//! capturing exactly what `InitializeParams` this client actually sends
//! (a real server never reports that back), a `$/progress` sequence with
//! a known, fixed shape instead of whatever a real server happens to
//! emit, and a server that exits on its own rather than being killed.
//!
//! Speaks the real wire protocol for real (`async-lsp`'s own server-role
//! `MainLoop`, real Content-Length framing over real stdio) -- nothing
//! about the transport is faked, only the responses are scripted.
//! Spawned as a genuine subprocess by
//! `crates/lux-lsp/tests/fake_server_protocol.rs`, exactly the way a real
//! language server is spawned.

use std::ops::ControlFlow;
use std::time::Duration;

use async_lsp::lsp_types::notification::Progress;
use async_lsp::lsp_types::{
    HoverProviderCapability, InitializeParams, InitializeResult, NumberOrString, ProgressParams,
    ProgressParamsValue, ServerCapabilities, WorkDoneProgress, WorkDoneProgressBegin,
    WorkDoneProgressEnd, WorkDoneProgressReport,
};
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router::Router;
use async_lsp::server::LifecycleLayer;
use tower::ServiceBuilder;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (server, _client) = async_lsp::MainLoop::new_server(|client| {
        let mut router = Router::new(());
        router
            .request::<async_lsp::lsp_types::request::Initialize, _>(|_, params| async move {
                capture_params(&params);
                Ok(InitializeResult {
                    capabilities: ServerCapabilities {
                        hover_provider: Some(HoverProviderCapability::Simple(true)),
                        ..ServerCapabilities::default()
                    },
                    server_info: None,
                })
            })
            .notification::<async_lsp::lsp_types::notification::Initialized>(move |_, _| {
                let client = client.clone();
                tokio::spawn(async move {
                    // Sentinel files in the workspace root, not env vars:
                    // each test run gets its own root, so this needs no
                    // process-wide state shared (and racing) across
                    // `cargo test`'s concurrently-run test threads.
                    if std::path::Path::new("fake-lsp-send-progress").exists() {
                        send_scripted_progress(&client).await;
                    }
                    let exit_marker = std::path::Path::new("fake-lsp-exit-after-initialized");
                    if exit_marker.exists() {
                        // Consumed, not just checked: a restarted instance
                        // of this same binary must come back up and stay
                        // running, not crash-loop forever against the
                        // still-present marker.
                        let _ = std::fs::remove_file(exit_marker);
                        // A server that dies on its own -- not killed by a
                        // signal -- exercising a different code path than
                        // the SIGKILL-based crash test.
                        std::process::exit(1);
                    }
                });
                ControlFlow::Continue(())
            })
            .request::<async_lsp::lsp_types::request::HoverRequest, _>(|_, _| async move {
                // Configurable delay before responding -- lets a test prove
                // a slow, in-flight LSP request never blocks other
                // concurrent async work on the same executor thread (the
                // mechanism golden rule 6, "no I/O on the UI thread," and
                // Phase 7's frame-time acceptance criterion both depend
                // on), something a real server's actual response latency
                // can't be relied on to reproduce on demand.
                if let Ok(millis) = std::fs::read_to_string("fake-lsp-hover-delay-ms")
                    && let Ok(millis) = millis.trim().parse::<u64>()
                {
                    tokio::time::sleep(Duration::from_millis(millis)).await;
                }
                Ok(Some(async_lsp::lsp_types::Hover {
                    contents: async_lsp::lsp_types::HoverContents::Scalar(
                        async_lsp::lsp_types::MarkedString::String("fake hover text".into()),
                    ),
                    range: None,
                }))
            })
            .notification::<async_lsp::lsp_types::notification::DidOpenTextDocument>(|_, _| {
                ControlFlow::Continue(())
            })
            .notification::<async_lsp::lsp_types::notification::DidChangeTextDocument>(|_, _| {
                ControlFlow::Continue(())
            })
            .notification::<async_lsp::lsp_types::notification::DidCloseTextDocument>(|_, _| {
                ControlFlow::Continue(())
            });

        ServiceBuilder::new()
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .service(router)
    });

    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio().unwrap(),
        async_lsp::stdio::PipeStdout::lock_tokio().unwrap(),
    );

    let _ = server.run_buffered(stdin, stdout).await;
}

fn capture_params(params: &InitializeParams) {
    #[allow(deprecated)]
    let captured = serde_json::json!({
        "root_uri": params.root_uri,
        "workspace_folders": params.workspace_folders,
    });
    let _ = std::fs::write(
        "fake-lsp-initialize-capture.json",
        serde_json::to_vec(&captured).unwrap(),
    );
}

async fn send_scripted_progress(client: &async_lsp::ClientSocket) {
    let token = NumberOrString::String("fake-progress".into());
    let begin = ProgressParams {
        token: token.clone(),
        value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(WorkDoneProgressBegin {
            title: "Fake indexing".into(),
            cancellable: Some(false),
            message: Some("starting".into()),
            percentage: Some(0),
        })),
    };
    let report = ProgressParams {
        token: token.clone(),
        value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(WorkDoneProgressReport {
            cancellable: Some(false),
            message: Some("halfway".into()),
            percentage: Some(50),
        })),
    };
    let end = ProgressParams {
        token,
        value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
            message: Some("done".into()),
        })),
    };
    for params in [begin, report, end] {
        let _ = client.notify::<Progress>(params);
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
