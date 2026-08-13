//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `hover/`, not `tests/`, for the same reason as the other
//! Phase 7 deliverables' `real_server` modules: it needs `pub(crate)`
//! session internals shared with `DocumentSync`.

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

/// Same small fixture `navigation`'s real-server tests use: a `Point`
/// struct, its constructor, and a caller.
fn document_text() -> String {
    "pub struct Point {\n    pub x: i32,\n    pub y: i32,\n}\n\nimpl Point {\n    pub fn new(x: i32, y: i32) -> Point {\n        Point { x, y }\n    }\n}\n\npub fn make() -> Point {\n    Point::new(1, 2)\n}\n\npub fn use_it() -> i32 {\n    let p = make();\n    p.x\n}\n".to_string()
}

async fn hover_text_until(
    hover: &Hover,
    uri: &Url,
    position: Position,
    mut predicate: impl FnMut(&str) -> bool,
    budget: Duration,
) -> HoverText {
    tokio::time::timeout(budget, async {
        loop {
            match hover.hover(uri, position).await {
                Ok(Some(text)) if predicate(&text.text) => return text,
                Ok(_) => {}
                Err(HoverError::Request(async_lsp::Error::Response(resp)))
                    if resp.code == async_lsp::ErrorCode::CONTENT_MODIFIED => {}
                Err(err) => panic!("hover request failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for a useful hover result"))
}

#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn hover_shows_the_real_function_signature() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let hover = Hover::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");

    // `make` call site inside `use_it`, line 16: "    let p = make();".
    let call_site = Position {
        line: 16,
        character: 13,
    };
    let hovered = hover_text_until(
        &hover,
        &uri,
        call_site,
        |text| text.contains("make"),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        hovered.text.contains("fn make"),
        "expected hover to show `make`'s real signature, got: {}",
        hovered.text
    );
    assert!(
        hovered.text.contains("Point"),
        "expected hover to mention the real return type `Point`, got: {}",
        hovered.text
    );

    session.stop().await;
}
