//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `completion/`, not `tests/`, for the same reason as
//! `sync/real_server.rs` and `diagnostics/real_server.rs`: it needs
//! `pub(crate)` session internals shared with `DocumentSync`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::{Documentation, MarkupContent, Position};

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

/// Retry `complete()` until `predicate` accepts the result, tolerating an
/// empty or partial (e.g. postfix-snippets-only, before type-aware member
/// completions are ready) list while rust-analyzer is still indexing.
async fn complete_until(
    completion: &Completion,
    uri: &Url,
    position: Position,
    mut predicate: impl FnMut(&[CompletionItem]) -> bool,
    budget: Duration,
) -> Vec<CompletionItem> {
    tokio::time::timeout(budget, async {
        loop {
            match completion.complete(uri, position, None).await {
                Ok(items) if predicate(&items) => return items,
                Ok(_) | Err(CompletionError::Superseded) => {}
                Err(err) => panic!("completion request failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for the expected completion list"))
}

fn has_push(items: &[CompletionItem]) -> bool {
    items.iter().any(|i| i.label.starts_with("push("))
}

fn document_text() -> String {
    // Completing right after `v.` should list `Vec<i32>`'s inherent and
    // Deref-borrowed slice methods -- a large, well-known, reliably
    // populated completion set for testing sort order and resolve against.
    "pub fn one() -> i32 {\n    let v: Vec<i32> = Vec::new();\n    v.\n    1\n}\n".to_string()
}

#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn lists_and_sorts_real_completions() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let completion = Completion::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");

    // Position right after `v.` on line 1 (0-indexed): "    v." is 6 chars.
    let position = Position {
        line: 2,
        character: 6,
    };
    let items = complete_until(
        &completion,
        &uri,
        position,
        has_push,
        Duration::from_secs(60),
    )
    .await;

    assert!(
        items.iter().any(|i| i.label.starts_with("push(")),
        "expected `Vec::push` among the completions, got: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );

    // `sort_items` (unit-tested in isolation) is proven wired into the
    // real request path here: the returned order is exactly the
    // `sortText`-or-label order, non-decreasing end to end.
    let sort_keys: Vec<&str> = items
        .iter()
        .map(|i| i.sort_text.as_deref().unwrap_or(i.label.as_str()))
        .collect();
    let mut sorted = sort_keys.clone();
    sorted.sort();
    assert_eq!(
        sort_keys, sorted,
        "expected completions already in sortText/label order"
    );

    session.stop().await;
}

#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn resolve_fills_in_documentation_when_supported() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let completion = Completion::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");

    let caps = session
        .watch()
        .borrow()
        .capabilities
        .clone()
        .expect("capabilities recorded after handshake");
    let resolve_provider = caps
        .completion_provider
        .as_ref()
        .and_then(|cp| cp.resolve_provider)
        .unwrap_or(false);
    assert!(
        resolve_provider,
        "expected rust-analyzer to advertise completionProvider.resolveProvider"
    );

    let position = Position {
        line: 2,
        character: 6,
    };
    let items = complete_until(
        &completion,
        &uri,
        position,
        has_push,
        Duration::from_secs(60),
    )
    .await;
    let push = items
        .into_iter()
        .find(|i| i.label.starts_with("push("))
        .expect("expected a `push` completion item for Vec<i32>");
    let push_label = push.label.clone();

    let resolved = completion
        .resolve(push.clone())
        .await
        .expect("resolve should succeed");
    assert_eq!(resolved.label, push_label);
    let doc_text = match resolved.documentation {
        Some(Documentation::String(s)) => s,
        Some(Documentation::MarkupContent(MarkupContent { value, .. })) => value,
        None => String::new(),
    };
    assert!(
        !doc_text.trim().is_empty(),
        "expected resolve to fill in documentation for `push`"
    );

    session.stop().await;
}

#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn a_stale_request_is_superseded_by_a_newer_one() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let completion = Arc::new(Completion::new(session.clone()));
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");
    let position = Position {
        line: 2,
        character: 6,
    };

    // Warm up: wait for a real, fully type-aware, non-superseded result
    // first, so the race below isn't confused by rust-analyzer still
    // indexing.
    complete_until(
        &completion,
        &uri,
        position,
        has_push,
        Duration::from_secs(60),
    )
    .await;

    let stale_completion = completion.clone();
    let stale_uri = uri.clone();
    let stale_task =
        tokio::spawn(async move { stale_completion.complete(&stale_uri, position, None).await });
    // One cooperative yield gives the spawned task exactly one poll: enough
    // to run its synchronous prefix (bump the generation, dispatch the
    // request) and suspend awaiting the response, but nowhere near enough
    // wall-clock time for a real cross-process round trip to finish --
    // unlike a fixed sleep, which is either too short to let it start or
    // (as first tried) long enough for it to finish entirely, defeating
    // the race this test means to set up.
    tokio::task::yield_now().await;

    let fresh = completion
        .complete(&uri, position, None)
        .await
        .expect("the newer request should succeed");
    assert!(!fresh.is_empty());

    match stale_task.await.expect("stale task should not panic") {
        Err(CompletionError::Superseded) => {}
        other => panic!("expected the stale request to be superseded, got: {other:?}"),
    }

    session.stop().await;
}
