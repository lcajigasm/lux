//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `code_actions/`, not `tests/`, for the same reason as the
//! other Phase 7 deliverables' `real_server` modules: it needs helpers
//! that reach into `pub(crate)` session internals shared with
//! `DocumentSync`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::{CodeActionOrCommand, Position};

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

#[tokio::test]
async fn a_real_code_action_edit_is_resolved_and_applied() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let actions = CodeActions::new(session.clone());
    let uri = lib_rs_uri(root.path());

    let original = "pub fn one() -> i32 {\n    1\n}\n";
    sync.open(uri.clone(), "rust", original.to_string())
        .expect("didOpen should succeed");

    // The `1` literal on line 1 -- rust-analyzer offers "Extract into
    // variable" for a selected expression regardless of diagnostics, so
    // this doesn't depend on any particular lint/config being enabled.
    let literal_range = Range {
        start: Position {
            line: 1,
            character: 4,
        },
        end: Position {
            line: 1,
            character: 5,
        },
    };
    let edit = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let response = match actions.code_actions(&uri, literal_range, Vec::new()).await {
                Ok(response) => response,
                Err(CodeActionsError::Request(async_lsp::Error::Response(resp)))
                    if resp.code == async_lsp::ErrorCode::CONTENT_MODIFIED =>
                {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
                Err(err) => panic!("code action request failed: {err}"),
            };
            let candidate = response.into_iter().find_map(|a| match a {
                CodeActionOrCommand::CodeAction(action)
                    if action.title.to_lowercase().contains("extract") =>
                {
                    Some(action)
                }
                _ => None,
            });
            // Only some code actions carry their `WorkspaceEdit` directly;
            // others need resolving, and even a resolve can come back
            // still-empty while rust-analyzer is mid-indexing -- retry the
            // whole request in that case rather than failing on a partial
            // answer.
            if let Some(action) = candidate
                && let Ok(resolved) = actions.resolve(action).await
                && let Some(edit) = resolved.edit
            {
                return edit;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!("timed out waiting for a resolvable 'extract into variable' code action")
    });

    let files = preview(&edit);
    assert!(
        !files.is_empty(),
        "expected the preview to list at least one file change"
    );
    assert!(
        matches!(&files[0], FileChange::Edit { uri: file_uri, .. } if *file_uri == uri),
        "expected the edit to target the fixture file"
    );

    actions
        .apply(&sync, &edit)
        .expect("apply should succeed for an edit to an already-open document");

    let new_text = sync.text(&uri).expect("document should still be open");
    assert_ne!(
        new_text, original,
        "expected the code action to actually change the document"
    );
    assert!(
        new_text.contains("let "),
        "expected the extracted variable's `let` binding, got: {new_text}"
    );

    session.stop().await;
}

#[tokio::test]
async fn apply_all_is_transactional_across_multiple_files() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());

    let uri_a =
        Url::from_file_path(root.path().canonicalize().unwrap().join("src/lib.rs")).unwrap();
    std::fs::write(
        root.path().join("src/other.rs"),
        "pub fn two() -> i32 { 2 }\n",
    )
    .unwrap();
    let uri_b =
        Url::from_file_path(root.path().canonicalize().unwrap().join("src/other.rs")).unwrap();

    sync.open(
        uri_a.clone(),
        "rust",
        "pub fn one() -> i32 { 1 }\n".to_string(),
    )
    .unwrap();
    sync.open(
        uri_b.clone(),
        "rust",
        "pub fn two() -> i32 { 2 }\n".to_string(),
    )
    .unwrap();

    // Success case: both open files get edited together.
    let mut edits_by_uri: BTreeMap<Url, Vec<TextEdit>> = BTreeMap::new();
    edits_by_uri.insert(
        uri_a.clone(),
        vec![TextEdit {
            range: whole_first_line(),
            new_text: "pub fn one_renamed() -> i32 { 1 }\n".into(),
        }],
    );
    edits_by_uri.insert(
        uri_b.clone(),
        vec![TextEdit {
            range: whole_first_line(),
            new_text: "pub fn two_renamed() -> i32 { 2 }\n".into(),
        }],
    );
    sync.apply_all(&edits_by_uri)
        .expect("applying edits to two open documents should succeed");
    assert!(sync.text(&uri_a).unwrap().contains("one_renamed"));
    assert!(sync.text(&uri_b).unwrap().contains("two_renamed"));

    // Failure case: one target isn't open -- neither already-open document
    // may be touched, proving the "all or nothing" guarantee for real,
    // not just for the single-file case.
    let before_a = sync.text(&uri_a).unwrap();
    let before_b = sync.text(&uri_b).unwrap();
    let unopened =
        Url::from_file_path(root.path().canonicalize().unwrap().join("src/missing.rs")).unwrap();
    let mut partial: BTreeMap<Url, Vec<TextEdit>> = BTreeMap::new();
    partial.insert(
        uri_a.clone(),
        vec![TextEdit {
            range: whole_first_line(),
            new_text: "pub fn should_not_apply() {}\n".into(),
        }],
    );
    partial.insert(
        unopened.clone(),
        vec![TextEdit {
            range: whole_first_line(),
            new_text: "unreachable".into(),
        }],
    );

    match sync.apply_all(&partial) {
        Err(SyncError::NotOpen(u)) => assert_eq!(u, unopened),
        other => panic!("expected NotOpen for the unopened target, got {other:?}"),
    }
    assert_eq!(
        sync.text(&uri_a).unwrap(),
        before_a,
        "the open document must be unchanged after a rejected transactional apply"
    );
    assert_eq!(
        sync.text(&uri_b).unwrap(),
        before_b,
        "the other open document must be unchanged too"
    );

    session.stop().await;
}

fn whole_first_line() -> Range {
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 1,
            character: 0,
        },
    }
}
