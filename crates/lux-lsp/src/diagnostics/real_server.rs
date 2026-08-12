//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `diagnostics/`, not `tests/`, for the same reason as
//! `sync/real_server.rs`: it needs helpers that reach into `pub(crate)`
//! session internals.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::{DiagnosticSeverity, Position, Range};

use super::*;
use crate::registry::ServerRegistry;
use crate::session::{ServerState, ServerStatus};
use crate::sync::{DocumentSync, TextEdit};

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

/// See `sync/real_server.rs`'s identical helper: the session canonicalizes
/// its root, so the file URI built here must match.
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

/// Wait until `predicate` holds for the diagnostics of `uri`, event-driven
/// via the diagnostics `watch` channel.
async fn wait_for_document_diagnostics(
    diagnostics: &Diagnostics,
    uri: &Url,
    mut predicate: impl FnMut(&[async_lsp::lsp_types::Diagnostic]) -> bool,
    budget: Duration,
) -> Vec<async_lsp::lsp_types::Diagnostic> {
    tokio::time::timeout(budget, async {
        let mut rx = diagnostics.watch();
        loop {
            let current = rx.borrow().get(uri).cloned().unwrap_or_default();
            if predicate(&current) {
                return current;
            }
            rx.changed()
                .await
                .expect("diagnostics channel closed unexpectedly");
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for expected diagnostics"))
}

#[tokio::test]
async fn publishes_a_real_error_then_clears_it_once_fixed() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let diagnostics = Diagnostics::new(session.clone());
    let uri = lib_rs_uri(root.path());

    // A syntax error -- reported straight from parsing, unconditionally,
    // with no dependency on semantic analysis, crate-graph loading, or
    // `cargo check`/flycheck being finished or even enabled.
    sync.open(
        uri.clone(),
        "rust",
        "pub fn broken( -> i32 { 1 }\n".to_string(),
    )
    .expect("didOpen should succeed");

    let broken = wait_for_document_diagnostics(
        &diagnostics,
        &uri,
        |ds| !ds.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        !broken.is_empty(),
        "expected rust-analyzer to report a diagnostic for the syntax error"
    );
    let severities: Vec<_> = broken
        .iter()
        .map(|d| d.severity.unwrap_or(DiagnosticSeverity::ERROR))
        .collect();
    assert!(
        severities.contains(&DiagnosticSeverity::ERROR),
        "expected an error-severity diagnostic, got: {severities:?}"
    );

    // Confirm the query layer surfaces the same thing `for_document` and
    // `summary`/`filtered` would report, not just the raw watch channel.
    assert_eq!(diagnostics.for_document(&uri).len(), broken.len());
    assert!(diagnostics.summary().errors >= 1);
    assert!(!diagnostics.filtered(DiagnosticSeverity::ERROR).is_empty());

    // Fix it via an incremental edit covering the whole (single) line --
    // proves diagnostics are *continuous*, not a one-shot snapshot from
    // `didOpen`.
    let whole_document = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 1,
            character: 0,
        },
    };
    sync.change(
        &uri,
        vec![TextEdit {
            range: whole_document,
            new_text: "pub fn fixed() -> i32 { 1 }\n".into(),
        }],
    )
    .expect("didChange should succeed");

    let fixed = wait_for_document_diagnostics(
        &diagnostics,
        &uri,
        |ds| ds.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        fixed.is_empty(),
        "expected the diagnostic to clear once the reference was fixed"
    );
    assert!(diagnostics.for_document(&uri).is_empty());

    session.stop().await;
}
