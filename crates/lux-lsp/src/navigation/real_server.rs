//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `navigation/`, not `tests/`, for the same reason as the
//! other Phase 7 deliverables' `real_server` modules: it needs
//! `pub(crate)` session internals shared with `DocumentSync`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyItem, CallHierarchyOutgoingCall, Location, Position,
};

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

/// A small but real crate: a struct with a constructor, and two functions
/// with a genuine caller/callee relationship -- enough surface for
/// definition, references, implementations, type definition, document and
/// workspace symbols, and call hierarchy, all against real semantics.
///
/// ```text
/// 0  pub struct Point {
/// 1      pub x: i32,
/// 2      pub y: i32,
/// 3  }
/// 4
/// 5  impl Point {
/// 6      pub fn new(x: i32, y: i32) -> Point {
/// 7          Point { x, y }
/// 8      }
/// 9  }
/// 10
/// 11 pub fn make() -> Point {
/// 12     Point::new(1, 2)
/// 13 }
/// 14
/// 15 pub fn use_it() -> i32 {
/// 16     let p = make();
/// 17     p.x
/// 18 }
/// ```
fn document_text() -> String {
    "pub struct Point {\n    pub x: i32,\n    pub y: i32,\n}\n\nimpl Point {\n    pub fn new(x: i32, y: i32) -> Point {\n        Point { x, y }\n    }\n}\n\npub fn make() -> Point {\n    Point::new(1, 2)\n}\n\npub fn use_it() -> i32 {\n    let p = make();\n    p.x\n}\n".to_string()
}

/// Retry a navigation call until `predicate` accepts the result, tolerating
/// an empty/incomplete answer while rust-analyzer is still indexing, and
/// `CONTENT_MODIFIED` (rust-analyzer's "not ready yet, ask again" reply --
/// the same one `async-lsp`'s own reference client retries on).
async fn retry_until<T, F, Fut>(
    mut request: F,
    mut predicate: impl FnMut(&T) -> bool,
    budget: Duration,
) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, NavigationError>>,
{
    tokio::time::timeout(budget, async {
        loop {
            match request().await {
                Ok(value) if predicate(&value) => return value,
                Ok(_) => {}
                Err(NavigationError::Request(async_lsp::Error::Response(resp)))
                    if resp.code == async_lsp::ErrorCode::CONTENT_MODIFIED => {}
                Err(err) => panic!("navigation request failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for the expected navigation result"))
}

#[tokio::test]
async fn goto_and_references_resolve_real_locations() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let navigation = Navigation::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");

    // `make` call site inside `use_it`, line 16: "    let p = make();".
    let call_site = Position {
        line: 16,
        character: 13,
    };
    let def = retry_until(
        || navigation.definition(&uri, call_site),
        |locs: &Vec<Location>| !locs.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        def.iter().any(|l| l.range.start.line == 11),
        "expected `definition` to resolve to `make`'s definition at line 11, got: {def:?}"
    );

    // References to the `Point` type, from its own definition, line 0.
    let point_name = Position {
        line: 0,
        character: 12,
    };
    let refs = retry_until(
        || navigation.references(&uri, point_name, true),
        |locs: &Vec<Location>| locs.len() >= 3,
        Duration::from_secs(60),
    )
    .await;
    assert!(
        refs.len() >= 3,
        "expected multiple references to `Point`, got: {refs:?}"
    );

    // Implementations of `Point` -- the `impl Point` block at line 5.
    let implementations = retry_until(
        || navigation.implementations(&uri, point_name),
        |locs: &Vec<Location>| !locs.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        implementations.iter().any(|l| l.range.start.line == 5),
        "expected an implementation at line 5, got: {implementations:?}"
    );

    // Type definition of `p` (a `Point`), line 17: "    p.x".
    let p_usage = Position {
        line: 17,
        character: 4,
    };
    let type_def = retry_until(
        || navigation.type_definition(&uri, p_usage),
        |locs: &Vec<Location>| !locs.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        type_def.iter().any(|l| l.range.start.line == 0),
        "expected type definition to resolve to `Point` at line 0, got: {type_def:?}"
    );

    // `declaration` has no Rust-specific meaning distinct from `definition`
    // (no header/source split); just confirm the request round-trips.
    retry_until(
        || navigation.declaration(&uri, call_site),
        |_: &Vec<Location>| true,
        Duration::from_secs(60),
    )
    .await;

    session.stop().await;
}

#[tokio::test]
async fn document_and_workspace_symbols_list_real_symbols() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let navigation = Navigation::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");

    let symbols = retry_until(
        || navigation.document_symbols(&uri),
        |entries: &Vec<SymbolEntry>| !entries.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    let mut names: Vec<String> = Vec::new();
    fn collect(entries: &[SymbolEntry], into: &mut Vec<String>) {
        for entry in entries {
            into.push(entry.name.clone());
            collect(&entry.children, into);
        }
    }
    collect(&symbols, &mut names);
    for expected in ["Point", "make", "use_it"] {
        assert!(
            names.iter().any(|n| n == expected),
            "expected document symbol `{expected}` among: {names:?}"
        );
    }

    let matches = retry_until(
        || navigation.workspace_symbols("make"),
        |entries: &Vec<WorkspaceSymbolEntry>| !entries.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        matches.iter().any(|s| s.name == "make"),
        "expected a workspace symbol named `make`, got: {:?}",
        matches.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    session.stop().await;
}

#[tokio::test]
async fn call_hierarchy_walks_a_real_caller_and_callee() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let navigation = Navigation::new(session.clone());
    let uri = lib_rs_uri(root.path());

    sync.open(uri.clone(), "rust", document_text())
        .expect("didOpen should succeed");

    // `make`'s own name, line 11: "pub fn make() -> Point {".
    let make_name = Position {
        line: 11,
        character: 8,
    };
    let roots = retry_until(
        || navigation.prepare_call_hierarchy(&uri, make_name),
        |items: &Vec<CallHierarchyItem>| !items.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    let make_item = roots
        .into_iter()
        .find(|item| item.name == "make")
        .expect("expected a call hierarchy item for `make`");

    let incoming = retry_until(
        || navigation.incoming_calls(make_item.clone()),
        |calls: &Vec<CallHierarchyIncomingCall>| !calls.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        incoming.iter().any(|call| call.from.name == "use_it"),
        "expected `use_it` to call `make`, got: {:?}",
        incoming.iter().map(|c| &c.from.name).collect::<Vec<_>>()
    );

    let outgoing = retry_until(
        || navigation.outgoing_calls(make_item.clone()),
        |calls: &Vec<CallHierarchyOutgoingCall>| !calls.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    assert!(
        outgoing.iter().any(|call| call.to.name.contains("new")),
        "expected `make` to call `Point::new`, got: {:?}",
        outgoing.iter().map(|c| &c.to.name).collect::<Vec<_>>()
    );

    session.stop().await;
}
