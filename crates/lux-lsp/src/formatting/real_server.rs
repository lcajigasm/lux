//! End-to-end tests against a real `rust-analyzer` (no fake/mock server,
//! and rust-analyzer's own formatting delegates to a real `rustfmt`, so
//! this is real formatting output, not a stub). Lives inside
//! `formatting/`, not `tests/`, for the same reason as the other Phase 7
//! deliverables' `real_server` modules: it needs helpers that reach into
//! `pub(crate)` session internals shared with `DocumentSync`.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_lsp::lsp_types::{FormattingOptions, Position, Range, TextEdit as LspTextEdit};

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

fn default_options() -> FormattingOptions {
    FormattingOptions {
        tab_size: 4,
        insert_spaces: true,
        ..Default::default()
    }
}

/// Retries on any JSON-RPC response error (not just `CONTENT_MODIFIED`) --
/// same reasoning as the rename deliverable's real-server helpers: a
/// freshly loaded crate can answer a hard error for a moment while
/// rust-analyzer is still building its crate graph, before formatting is
/// meaningfully available. A real transport error still fails fast.
async fn format_until(
    formatting: &Formatting,
    uri: &Url,
    predicate: impl Fn(&[LspTextEdit]) -> bool,
    budget: Duration,
) -> Vec<LspTextEdit> {
    tokio::time::timeout(budget, async {
        loop {
            match formatting.document(uri, default_options()).await {
                Ok(edits) if predicate(&edits) => return edits,
                Ok(_) => {}
                Err(FormattingError::Request(async_lsp::Error::Response(_))) => {}
                Err(err) => panic!("formatting request failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for a real formatting result"))
}

#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn document_formatting_produces_real_rustfmt_output() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let formatting = Formatting::new(session.clone());
    let uri = lib_rs_uri(root.path());

    let messy = "fn add(a:i32,b:i32)->i32{a+b}\n";
    sync.open(uri.clone(), "rust", messy.to_string())
        .expect("didOpen should succeed");

    let edits = format_until(
        &formatting,
        &uri,
        |edits| !edits.is_empty(),
        Duration::from_secs(60),
    )
    .await;
    sync.change(&uri, edits.into_iter().map(TextEdit::from).collect())
        .expect("applying the formatting edits should succeed");

    let formatted = sync.text(&uri).unwrap();
    assert_ne!(
        formatted, messy,
        "expected formatting to actually change the document"
    );
    assert!(
        formatted.contains("fn add(a: i32, b: i32) -> i32 {"),
        "expected real rustfmt spacing in the signature, got: {formatted}"
    );
    assert!(
        formatted.contains("    a + b"),
        "expected the body on its own indented line, got: {formatted}"
    );

    session.stop().await;
}

/// rustfmt's range formatting is explicitly unstable upstream: a stable
/// rust-analyzer refuses it outright with "rustfmt range formatting is
/// unstable ... opt-in ... using a nightly build of rustfmt and setting
/// `rustfmt.rangeFormatting.enable`" -- a rust-analyzer-specific
/// initialization option, not a general LSP client capability, and out of
/// this deliverable's scope to force on (it would tie the whole client to
/// one server's nightly-only config, not something a general LSP client
/// should assume). So this test asserts the *real, honest* outcome for
/// whatever this rust-analyzer actually advertises, rather than assuming
/// range formatting always succeeds: when the server claims support, it
/// must genuinely reformat only the requested range; when it doesn't (the
/// common case on a stable toolchain, confirmed via the real
/// `documentRangeFormattingProvider` capability), the request must still
/// round-trip cleanly -- an empty result or the server's own real error,
/// never a client-side panic or hang.
#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn range_formatting_round_trips_against_the_real_servers_actual_capability() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let formatting = Formatting::new(session.clone());
    let uri = lib_rs_uri(root.path());

    let messy = "fn add(a:i32,b:i32)->i32{a+b}\npub fn one() -> i32 { 1 }\n";
    sync.open(uri.clone(), "rust", messy.to_string())
        .expect("didOpen should succeed");

    let supports_range_formatting = matches!(
        session
            .watch()
            .borrow()
            .capabilities
            .as_ref()
            .and_then(|c| c.document_range_formatting_provider.clone()),
        Some(async_lsp::lsp_types::OneOf::Left(true)) | Some(async_lsp::lsp_types::OneOf::Right(_))
    );

    let first_line = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 30,
        },
    };
    let result = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            match formatting.range(&uri, first_line, default_options()).await {
                Err(FormattingError::Request(async_lsp::Error::Response(resp)))
                    if resp.code == async_lsp::ErrorCode::CONTENT_MODIFIED => {}
                other => return other,
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for a real range formatting response"));

    if supports_range_formatting {
        let edits = result
            .expect("range formatting should succeed when the server advertises support for it");
        assert!(
            !edits.is_empty(),
            "expected the server to actually produce edits for the range it claims to support"
        );
        sync.change(&uri, edits.into_iter().map(TextEdit::from).collect())
            .expect("applying the range formatting edits should succeed");
        let text = sync.text(&uri).unwrap();
        assert!(
            text.contains("fn add(a: i32, b: i32) -> i32 {"),
            "expected the requested range to be reformatted, got: {text}"
        );
        assert!(
            text.contains("pub fn one() -> i32 { 1 }"),
            "expected the untouched second line to stay exactly as it was, got: {text}"
        );
    } else {
        match result {
            Ok(edits) => assert!(
                edits.is_empty(),
                "expected no edits from a server that doesn't advertise range formatting support"
            ),
            Err(FormattingError::Request(_)) => {}
            Err(err) => panic!("expected a clean not-connected error at worst, got: {err}"),
        }
    }

    session.stop().await;
}

#[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
#[tokio::test]
async fn format_and_save_applies_formatting_before_saving() {
    let root = rust_project_root();
    let session = running_session(root.path()).await;
    let sync = DocumentSync::new(session.clone());
    let formatting = Formatting::new(session.clone());
    let uri = lib_rs_uri(root.path());

    let messy = "fn add(a:i32,b:i32)->i32{a+b}\n";
    sync.open(uri.clone(), "rust", messy.to_string())
        .expect("didOpen should succeed");

    // Same readiness concern as the other tests here: retry the whole
    // format-and-save flow until it lands, rather than assuming the very
    // first attempt (which could race a freshly spawned server still
    // loading its crate graph) succeeds.
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            match formatting
                .format_and_save(&sync, &uri, default_options())
                .await
            {
                Ok(()) if sync.text(&uri).unwrap() != messy => return,
                Ok(()) => {}
                Err(FormatAndSaveError::Formatting(FormattingError::Request(
                    async_lsp::Error::Response(_),
                ))) => {}
                Err(err) => panic!("format_and_save failed: {err}"),
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!("timed out waiting for format_and_save to actually format the document")
    });

    let saved = sync.text(&uri).unwrap();
    assert!(
        saved.contains("fn add(a: i32, b: i32) -> i32 {"),
        "expected the saved content to be the *formatted* text, got: {saved}"
    );

    session.stop().await;
}
