//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `sync/`, not `tests/`, because it needs `pub(crate)` access
//! to `ServerSession::socket()` to issue a raw `hover` request as a
//! verification oracle -- proving edits actually landed server-side, not
//! just that a notification was sent without erroring.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::{
    HoverContents, HoverParams, MarkupContent, TextDocumentPositionParams, WorkDoneProgressParams,
};
use async_lsp::{ErrorCode, ServerSocket};

use super::*;
use crate::registry::ServerRegistry;
use crate::session::{ServerState, ServerStatus};

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

async fn wait_for(
    session: &ServerSession,
    mut predicate: impl FnMut(&ServerState) -> bool,
    budget: Duration,
) -> ServerState {
    tokio::time::timeout(budget, async {
        let mut rx = session.watch();
        loop {
            {
                let state = rx.borrow();
                if predicate(&state) {
                    return state.clone();
                }
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

/// Retry `hover` until it returns non-empty markup, tolerating
/// `None`/`CONTENT_MODIFIED` while rust-analyzer is still indexing.
async fn hover_markup(
    socket: &mut ServerSocket,
    uri: &Url,
    position: Position,
    budget: Duration,
) -> String {
    tokio::time::timeout(budget, async {
        loop {
            match socket
                .hover(HoverParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: TextDocumentIdentifier { uri: uri.clone() },
                        position,
                    },
                    work_done_progress_params: WorkDoneProgressParams::default(),
                })
                .await
            {
                Ok(Some(hover)) => {
                    if let HoverContents::Markup(MarkupContent { value, .. }) = hover.contents
                        && !value.is_empty()
                    {
                        return value;
                    }
                }
                Ok(None) => {}
                Err(async_lsp::Error::Response(resp))
                    if resp.code == ErrorCode::CONTENT_MODIFIED => {}
                Err(err) => panic!("hover request failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for a useful hover result"))
}

/// `ServerRegistry` canonicalizes the session root before using it as the
/// workspace URI (symlinks like macOS's `/var` -> `/private/var` would
/// otherwise make the file's URI disagree with what the server thinks its
/// own project root is), so the file URI built here must match.
fn lib_rs_uri(root: &Path) -> Url {
    Url::from_file_path(root.canonicalize().unwrap().join("src/lib.rs")).unwrap()
}

#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn incremental_change_is_reflected_by_the_server() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(
        uri.clone(),
        "rust",
        "pub fn one() -> i32 { 1 }\n".to_string(),
    )
    .expect("didOpen should succeed");

    // Baseline: hover resolves the document as opened, before any edit.
    let mut socket = session.socket().expect("session should still be connected");
    let baseline = hover_markup(
        &mut socket,
        &uri,
        Position {
            line: 0,
            character: 8,
        },
        Duration::from_secs(60),
    )
    .await;
    assert!(
        baseline.contains("fn one"),
        "expected hover to show the opened function, got: {baseline}"
    );

    // Confirm against the real server's own advertised capability, rather
    // than assuming it -- proves the negotiation is real.
    let caps = session
        .watch()
        .borrow()
        .capabilities
        .clone()
        .expect("capabilities recorded after handshake");
    assert_eq!(
        change_kind_for(Some(&caps)),
        TextDocumentSyncKind::INCREMENTAL,
        "expected rust-analyzer to negotiate incremental sync"
    );

    // A zero-width insert at EOF -- an incremental range, never the whole
    // document.
    let insert_at = Position {
        line: 1,
        character: 0,
    };
    sync.change(
        &uri,
        vec![TextEdit {
            range: Range {
                start: insert_at,
                end: insert_at,
            },
            new_text: "pub fn two() -> i32 { 2 }\n".into(),
        }],
    )
    .expect("didChange should succeed");

    let mut socket = session.socket().expect("session should still be connected");
    let hover = hover_markup(
        &mut socket,
        &uri,
        Position {
            line: 1,
            character: 8,
        },
        Duration::from_secs(60),
    )
    .await;
    assert!(
        hover.contains("fn two"),
        "expected hover to show the appended function, got: {hover}"
    );

    sync.close(&uri).expect("didClose should succeed");
    assert!(!sync.is_open(&uri));

    session.stop().await;
}

#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn save_and_close_round_trip_and_close_forbids_further_use() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(
        uri.clone(),
        "rust",
        "pub fn one() -> i32 { 1 }\n".to_string(),
    )
    .unwrap();
    sync.save(&uri).expect("didSave should succeed while open");
    sync.close(&uri).expect("didClose should succeed");

    match sync.save(&uri) {
        Err(SyncError::NotOpen(err_uri)) => assert_eq!(err_uri, uri),
        other => panic!("expected NotOpen after close, got {other:?}"),
    }
    match sync.change(
        &uri,
        vec![TextEdit {
            range: Range::default(),
            new_text: "x".into(),
        }],
    ) {
        Err(SyncError::NotOpen(err_uri)) => assert_eq!(err_uri, uri),
        other => panic!("expected NotOpen after close, got {other:?}"),
    }

    session.stop().await;
}

// `libc::kill`/`SIGKILL` are POSIX-only -- Windows has no equivalent
// signal, so this test (already manual-only, see #[ignore] below) simply
// doesn't exist there rather than needing a different termination
// mechanism it wouldn't be testing anyway.
#[cfg(unix)]
#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn reopens_documents_after_a_crash_restart() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let uri = lib_rs_uri(root.path());

    // The in-memory edit below is never written to disk, so the only way a
    // post-restart hover can see "fn two" is if `DocumentSync` itself
    // replayed `didOpen` with this text to the new process.
    let text = "pub fn one() -> i32 { 1 }\npub fn two() -> i32 { 2 }\n";
    sync.open(uri.clone(), "rust", text.to_string()).unwrap();

    let pid = session
        .watch()
        .borrow()
        .pid
        .expect("running session should expose a pid");
    let killed = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
    assert_eq!(killed, 0, "failed to signal rust-analyzer (pid {pid})");

    wait_for(
        &session,
        |s| s.restarts >= 1 && matches!(s.status, ServerStatus::Running),
        Duration::from_secs(60),
    )
    .await;

    let mut socket = session
        .socket()
        .expect("session should be connected again after restart");
    let hover = hover_markup(
        &mut socket,
        &uri,
        Position {
            line: 1,
            character: 8,
        },
        Duration::from_secs(60),
    )
    .await;
    assert!(
        hover.contains("fn two"),
        "expected the replayed document to be recognized, got: {hover}"
    );

    session.stop().await;
}
