//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `inlay_hints/`, not `tests/`, for the same reason as the
//! other Phase 7 deliverables' `real_server` modules.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::Position;

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

/// Same fixture as the other real-server tests in this phase, plus one
/// line rust-analyzer reliably annotates without any special config: a
/// `let` binding with no explicit type, which gets a real inferred-type
/// inlay hint (`: Point`) -- unlike parameter-name hints, which some
/// rust-analyzer defaults suppress for single-argument or "obvious" calls.
fn document_text() -> String {
    "pub struct Point {\n    pub x: i32,\n    pub y: i32,\n}\n\nimpl Point {\n    pub fn new(x: i32, y: i32) -> Point {\n        Point { x, y }\n    }\n}\n\npub fn make() -> Point {\n    Point::new(1, 2)\n}\n\npub fn use_it() -> i32 {\n    let p = make();\n    p.x\n}\n".to_string()
}

async fn inlay_hints_until(
    client: &InlayHints,
    uri: &Url,
    range: Range,
    mut predicate: impl FnMut(&[InlayHintEntry]) -> bool,
    budget: Duration,
) -> Vec<InlayHintEntry> {
    tokio::time::timeout(budget, async {
        loop {
            match client.inlay_hints(uri, range).await {
                Ok(hints) if predicate(&hints) => return hints,
                Ok(_) => {}
                Err(InlayHintsError::Request(async_lsp::Error::Response(resp)))
                    if resp.code == async_lsp::ErrorCode::CONTENT_MODIFIED => {}
                Err(err) => panic!("inlay hints request failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for a useful inlay hints result"))
}

#[tokio::test]
async fn inlay_hints_show_the_real_inferred_type() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let client = InlayHints::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");

    let whole_document = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 19,
            character: 0,
        },
    };
    let hints = inlay_hints_until(
        &client,
        &uri,
        whole_document,
        |hints| hints.iter().any(|h| h.position.line == 16),
        Duration::from_secs(60),
    )
    .await;

    // Line 16: "    let p = make();" -- `p`'s inferred type is `Point`,
    // with no explicit annotation in the source.
    let type_hint = hints
        .iter()
        .find(|h| h.position.line == 16)
        .expect("expected an inlay hint on the `let p = make();` line");
    assert!(
        type_hint.text.contains("Point"),
        "expected the inferred-type hint to mention `Point`, got: {}",
        type_hint.text
    );

    session.stop().await;
}
