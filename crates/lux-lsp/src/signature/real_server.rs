//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `signature/`, not `tests/`, for the same reason as the
//! other Phase 7 deliverables' `real_server` modules.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::{Position, SignatureHelp};

use super::*;
use crate::registry::ServerRegistry;
use crate::session::{ServerState, ServerStatus};
use crate::sync::DocumentSync;

fn rust_project_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn one() -> i32 { 1 }\n").unwrap();
    dir
}

fn lib_rs_uri(root: &Path) -> Url {
    Url::from_file_path(root.canonicalize().unwrap().join("src/lib.rs")).unwrap()
}

async fn wait_for(
    session: &ServerSession,
    mut predicate: impl FnMut(&ServerState) -> bool,
    budget: Duration,
) {
    tokio::time::timeout(budget, async {
        let mut rx = session.watch();
        loop {
            if predicate(&rx.borrow()) {
                return;
            }
            rx.changed()
                .await
                .expect("session state channel closed unexpectedly");
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for expected session state"))
}

async fn running_session(root: &Path) -> Arc<ServerSession> {
    let registry = ServerRegistry::new();
    let session = registry
        .get_or_start("rust", root)
        .await
        .expect("rust-analyzer should be discoverable on PATH");
    wait_for(
        &session,
        |s| matches!(s.status, ServerStatus::Running),
        Duration::from_secs(60),
    )
    .await;
    session
}

/// Same fixture as `navigation`/`hover`'s real-server tests.
fn document_text() -> String {
    "pub struct Point {\n    pub x: i32,\n    pub y: i32,\n}\n\nimpl Point {\n    pub fn new(x: i32, y: i32) -> Point {\n        Point { x, y }\n    }\n}\n\npub fn make() -> Point {\n    Point::new(1, 2)\n}\n\npub fn use_it() -> i32 {\n    let p = make();\n    p.x\n}\n".to_string()
}

async fn signature_help_until(
    signature: &Signature,
    uri: &Url,
    position: Position,
    mut predicate: impl FnMut(&SignatureHelp) -> bool,
    budget: Duration,
) -> SignatureHelp {
    tokio::time::timeout(budget, async {
        loop {
            match signature.signature_help(uri, position).await {
                Ok(Some(help)) if predicate(&help) => return help,
                Ok(_) => {}
                Err(SignatureError::Request(async_lsp::Error::Response(resp)))
                    if resp.code == async_lsp::ErrorCode::CONTENT_MODIFIED => {}
                Err(err) => panic!("signature help request failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for a useful signature help result"))
}

#[tokio::test]
async fn signature_help_shows_real_parameters_and_the_active_one() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let signature = Signature::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");

    // Inside `Point::new(1, 2)`'s argument list, right after the `(`, line
    // 12: "    Point::new(1, 2)".
    let inside_call = Position {
        line: 12,
        character: 15,
    };
    let help = signature_help_until(
        &signature,
        &uri,
        inside_call,
        |help| !help.signatures.is_empty(),
        Duration::from_secs(60),
    )
    .await;

    let active = active_signature(&help).expect("expected an active signature");
    assert!(
        active.label.contains("new"),
        "expected the active signature to be `Point::new`, got: {}",
        active.label
    );
    assert!(
        active.label.contains("x") && active.label.contains("y"),
        "expected the real parameter names in the label, got: {}",
        active.label
    );

    if let Some(label) = active_parameter_label(&help, active) {
        // rust-analyzer's parameter labels include the type annotation
        // (e.g. "x: i32"), not just the bare name.
        assert!(
            label.starts_with("x") || label.starts_with("y"),
            "expected the active parameter to be a real parameter (`x: i32` or `y: i32`), got: {label}"
        );
    }

    session.stop().await;
}
