//! End-to-end tests against a real `rust-analyzer` (no fake/mock server).
//! Lives inside `rename/`, not `tests/`, for the same reason as the other
//! Phase 7 deliverables' `real_server` modules: it needs helpers that
//! reach into `pub(crate)` session internals shared with `DocumentSync`.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::Position;

use super::*;
use crate::registry::ServerRegistry;
use crate::session::{ServerState, ServerStatus};
use crate::sync::DocumentSync;

/// Both `src/lib.rs` and `src/other.rs` are written to disk up front, not
/// just opened via `didOpen` -- rust-analyzer resolves `pub mod other;` to
/// a file path as part of loading the crate graph, so the module needs to
/// genuinely exist on disk the same way it would in a real editor session
/// (open a project that's already there, not one invented purely in an
/// LSP overlay).
fn rust_project_root() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/lib.rs"), lib_rs_text()).unwrap();
    std::fs::write(dir.path().join("src/other.rs"), other_rs_text()).unwrap();
    dir
}

fn src_uri(root: &Path, name: &str) -> Url {
    Url::from_file_path(root.canonicalize().unwrap().join("src").join(name)).unwrap()
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

/// `lib.rs` and `other.rs`: a struct defined in one file, used (return
/// type, associated-function call, struct literal) across both -- a
/// genuine cross-file rename target, with several occurrences within
/// `other.rs` itself to also exercise same-file multi-hunk ordering.
///
/// lib.rs:
/// ```text
/// 0 pub mod other;
/// 1
/// 2 pub fn make() -> other::Point {
/// 3     other::Point::new(1, 2)
/// 4 }
/// ```
///
/// other.rs:
/// ```text
/// 0 pub struct Point {
/// 1     pub x: i32,
/// 2     pub y: i32,
/// 3 }
/// 4
/// 5 impl Point {
/// 6     pub fn new(x: i32, y: i32) -> Point {
/// 7         Point { x, y }
/// 8     }
/// 9 }
/// ```
fn lib_rs_text() -> String {
    "pub mod other;\n\npub fn make() -> other::Point {\n    other::Point::new(1, 2)\n}\n"
        .to_string()
}

fn other_rs_text() -> String {
    "pub struct Point {\n    pub x: i32,\n    pub y: i32,\n}\n\nimpl Point {\n    pub fn new(x: i32, y: i32) -> Point {\n        Point { x, y }\n    }\n}\n".to_string()
}

/// Retries on *any* JSON-RPC response error, not just `CONTENT_MODIFIED`:
/// a freshly loaded two-file crate can answer "no references found" (a
/// hard error, not the usual "still indexing" signal) for a moment while
/// rust-analyzer is still building its crate graph. A real transport
/// error (not a response error at all) still fails fast.
async fn rename_edit_until(
    rename: &Rename,
    uri: &Url,
    position: Position,
    new_name: &str,
    budget: Duration,
) -> WorkspaceEdit {
    tokio::time::timeout(budget, async {
        loop {
            match rename.rename(uri, position, new_name).await {
                Ok(Some(edit)) => return edit,
                Ok(None) => {}
                Err(RenameError::Request(async_lsp::Error::Response(_))) => {}
                Err(err) => panic!("rename request failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for a real cross-file rename edit"))
}

#[tokio::test]
async fn prepare_validates_the_real_symbol_at_the_cursor() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let rename = Rename::new(session.clone());
    let other_uri = src_uri(root.path(), "other.rs");

    sync.open(other_uri.clone(), "rust", other_rs_text())
        .expect("didOpen should succeed");
    sync.open(src_uri(root.path(), "lib.rs"), "rust", lib_rs_text())
        .expect("didOpen should succeed");

    // "Point" in `pub struct Point {`, other.rs line 0, chars 11..16.
    let point_name = Position {
        line: 0,
        character: 13,
    };
    let prepared = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            match rename.prepare(&other_uri, point_name).await {
                Ok(Some(response)) => return response,
                Ok(None) => {}
                Err(RenameError::Request(async_lsp::Error::Response(_))) => {}
                Err(err) => panic!("prepareRename failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for a real prepareRename response"));

    let range = match prepared {
        PrepareRenameResponse::Range(range) => range,
        PrepareRenameResponse::RangeWithPlaceholder { range, placeholder } => {
            assert_eq!(
                placeholder, "Point",
                "expected the real current name as the placeholder"
            );
            range
        }
        PrepareRenameResponse::DefaultBehavior { .. } => {
            panic!("expected a concrete range for a real, renameable struct")
        }
    };
    assert_eq!(range.start.line, 0);

    session.stop().await;
}

#[tokio::test]
async fn cross_file_rename_previews_and_applies_or_rejects_per_hunk() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let rename = Rename::new(session.clone());
    let other_uri = src_uri(root.path(), "other.rs");
    let lib_uri = src_uri(root.path(), "lib.rs");

    sync.open(other_uri.clone(), "rust", other_rs_text())
        .expect("didOpen should succeed");
    sync.open(lib_uri.clone(), "rust", lib_rs_text())
        .expect("didOpen should succeed");

    let point_name = Position {
        line: 0,
        character: 13,
    };
    let edit = rename_edit_until(
        &rename,
        &other_uri,
        point_name,
        "Coord",
        Duration::from_secs(60),
    )
    .await;

    let rename_preview = preview(&edit);
    assert!(
        rename_preview.file_operations.is_empty(),
        "expected a pure text-edit rename, not a file rename"
    );
    let files = rename_preview.files();
    assert!(
        files.contains(&&other_uri),
        "expected other.rs among the renamed files"
    );
    assert!(
        files.contains(&&lib_uri),
        "expected lib.rs among the renamed files -- this must be a real cross-file rename"
    );
    assert!(
        rename_preview.hunks.len() >= 4,
        "expected several hunks across both files, got {}",
        rename_preview.hunks.len()
    );

    // Accept only other.rs's hunks -- lib.rs must stay untouched, proving
    // real per-hunk (not per-file, not all-or-nothing) selection.
    let other_only: HashSet<usize> = rename_preview
        .hunks
        .iter()
        .filter(|h| h.uri == other_uri)
        .map(|h| h.id)
        .collect();
    assert!(!other_only.is_empty());
    rename
        .apply(&sync, &rename_preview, &other_only)
        .expect("applying only other.rs's hunks should succeed");

    let other_text = sync.text(&other_uri).unwrap();
    assert!(
        other_text.contains("Coord"),
        "expected other.rs to show the renamed symbol, got: {other_text}"
    );
    assert!(
        !other_text.contains("Point"),
        "expected every `Point` occurrence in other.rs to be renamed, got: {other_text}"
    );

    let lib_text_after_partial = sync.text(&lib_uri).unwrap();
    assert_eq!(
        lib_text_after_partial,
        lib_rs_text(),
        "expected lib.rs's rejected hunks to leave it completely untouched"
    );

    // Now accept the rest (lib.rs's hunks) -- the remainder of the same
    // preview, applied in a second, independent call.
    let lib_only: HashSet<usize> = rename_preview
        .hunks
        .iter()
        .filter(|h| h.uri == lib_uri)
        .map(|h| h.id)
        .collect();
    assert!(!lib_only.is_empty());
    rename
        .apply(&sync, &rename_preview, &lib_only)
        .expect("applying the remaining hunks should succeed");

    let lib_text_after_full = sync.text(&lib_uri).unwrap();
    assert!(
        lib_text_after_full.contains("Coord"),
        "expected lib.rs to show the renamed symbol after accepting its hunks, got: {lib_text_after_full}"
    );
    assert!(
        !lib_text_after_full.contains("Point"),
        "expected every `Point` occurrence in lib.rs to be renamed, got: {lib_text_after_full}"
    );

    session.stop().await;
}
