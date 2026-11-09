//! Incremental document synchronization: `didOpen`/`didChange`/`didSave`/
//! `didClose`, sending `textDocument/didChange` as the ranges that actually
//! changed -- not the whole document -- whenever the server negotiated
//! incremental sync, with a correct full-document fallback otherwise.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex as StdMutex, Weak};

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams,
    DidSaveTextDocumentParams, Position, Range, ServerCapabilities, TextDocumentContentChangeEvent,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncSaveOptions, Url, VersionedTextDocumentIdentifier,
};

use crate::session::ServerSession;

/// A single edit to apply to an open document: replace `range` with
/// `new_text`. `range` uses UTF-16 code unit columns, per the LSP spec's
/// default position encoding.
#[derive(Debug, Clone)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

impl From<async_lsp::lsp_types::TextEdit> for TextEdit {
    fn from(edit: async_lsp::lsp_types::TextEdit) -> Self {
        Self {
            range: edit.range,
            new_text: edit.new_text,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("document {0} is not open")]
    NotOpen(Url),
    #[error("failed to notify the language server: {0}")]
    Notify(#[from] async_lsp::Error),
}

struct OpenDocument {
    language_id: String,
    version: i32,
    text: String,
}

/// Tracks one session's open documents and speaks `didOpen`/`didChange`/
/// `didSave`/`didClose` to it. Survives a crash restart: once the
/// underlying [`ServerSession`] reconnects, every still-open document is
/// replayed as a fresh `didOpen` (the new server process has no memory of
/// the old one's state).
pub struct DocumentSync {
    session: Arc<ServerSession>,
    documents: StdMutex<HashMap<Url, OpenDocument>>,
}

impl DocumentSync {
    pub fn new(session: Arc<ServerSession>) -> Arc<Self> {
        let this = Arc::new(Self {
            session,
            documents: StdMutex::new(HashMap::new()),
        });
        crate::runtime::spawn(replay_on_reconnect(Arc::downgrade(&this)));
        this
    }

    /// Send `didOpen` and start tracking `uri` for subsequent `didChange`.
    pub fn open(
        &self,
        uri: Url,
        language_id: impl Into<String>,
        text: String,
    ) -> Result<(), SyncError> {
        let language_id = language_id.into();
        let mut socket = self.session.socket().ok_or(SyncError::NotConnected)?;
        socket.did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: language_id.clone(),
                version: 0,
                text: text.clone(),
            },
        })?;
        self.documents.lock().expect("documents poisoned").insert(
            uri,
            OpenDocument {
                language_id,
                version: 0,
                text,
            },
        );
        Ok(())
    }

    /// Apply `edits` locally and send them on as `didChange`: one
    /// incremental range per edit when the server negotiated
    /// [`TextDocumentSyncKind::INCREMENTAL`], or the whole resulting
    /// document in a single change event for [`TextDocumentSyncKind::FULL`].
    pub fn change(&self, uri: &Url, mut edits: Vec<TextEdit>) -> Result<(), SyncError> {
        if edits.is_empty() {
            return Ok(());
        }
        let mut socket = self.session.socket().ok_or(SyncError::NotConnected)?;
        let sync_kind = self.negotiated_change_kind();
        sort_edits_for_application(&mut edits);

        let params = {
            let mut documents = self.documents.lock().expect("documents poisoned");
            let doc = documents
                .get_mut(uri)
                .ok_or_else(|| SyncError::NotOpen(uri.clone()))?;
            for edit in &edits {
                doc.text = apply_edit(&doc.text, edit);
            }
            doc.version += 1;
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: doc.version,
                },
                content_changes: content_changes_for(sync_kind, &edits, &doc.text),
            }
        };
        socket.did_change(params)?;
        Ok(())
    }

    /// Apply a set of edits to *multiple* already-open documents as one
    /// atomic unit: either every targeted file's edits land (and are sent
    /// to the server) or, if any target document is not currently open,
    /// none are -- nothing is mutated until every target has been
    /// confirmed open. Used by `code_actions` to apply a `WorkspaceEdit`
    /// transactionally, including edits spanning several files.
    pub fn apply_all(&self, edits_by_uri: &BTreeMap<Url, Vec<TextEdit>>) -> Result<(), SyncError> {
        if edits_by_uri.is_empty() {
            return Ok(());
        }
        let mut socket = self.session.socket().ok_or(SyncError::NotConnected)?;
        let sync_kind = self.negotiated_change_kind();
        let mut documents = self.documents.lock().expect("documents poisoned");

        // Sorted once up front so both the local fold below and the
        // `content_changes` sent to the server use the exact same,
        // position-safe order (see `sort_edits_for_application`).
        let mut sorted_edits: BTreeMap<&Url, Vec<TextEdit>> = BTreeMap::new();
        for (uri, edits) in edits_by_uri {
            let mut edits = edits.clone();
            sort_edits_for_application(&mut edits);
            sorted_edits.insert(uri, edits);
        }

        // Phase 1: validate every target is open and compute its new text
        // -- no mutation yet, so a single missing document aborts the
        // whole call before anything changes.
        let mut computed = Vec::with_capacity(sorted_edits.len());
        for (&uri, edits) in &sorted_edits {
            let current = &documents
                .get(uri)
                .ok_or_else(|| SyncError::NotOpen(uri.clone()))?
                .text;
            let mut new_text = current.clone();
            for edit in edits {
                new_text = apply_edit(&new_text, edit);
            }
            computed.push((uri.clone(), new_text));
        }

        // Phase 2: commit -- every entry is guaranteed present, since
        // phase 1 already checked.
        for (uri, new_text) in computed {
            let doc = documents.get_mut(&uri).expect("checked open in phase 1");
            doc.text = new_text;
            doc.version += 1;
            let params = DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: doc.version,
                },
                content_changes: content_changes_for(sync_kind, &sorted_edits[&uri], &doc.text),
            };
            let _ = socket.did_change(params);
        }
        Ok(())
    }

    /// The current known text of an open document, if any -- e.g. to
    /// confirm a `code_actions::apply` actually landed.
    pub fn text(&self, uri: &Url) -> Option<String> {
        self.documents
            .lock()
            .expect("documents poisoned")
            .get(uri)
            .map(|doc| doc.text.clone())
    }

    /// Send `didSave`, including the current text only if the server asked
    /// for it (`TextDocumentSyncOptions::save::include_text`).
    pub fn save(&self, uri: &Url) -> Result<(), SyncError> {
        let mut socket = self.session.socket().ok_or(SyncError::NotConnected)?;
        let want_text = self.save_wants_text();
        let text = {
            let documents = self.documents.lock().expect("documents poisoned");
            let doc = documents
                .get(uri)
                .ok_or_else(|| SyncError::NotOpen(uri.clone()))?;
            want_text.then(|| doc.text.clone())
        };
        socket.did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            text,
        })?;
        Ok(())
    }

    /// Send `didClose` and stop tracking `uri`.
    pub fn close(&self, uri: &Url) -> Result<(), SyncError> {
        let mut socket = self.session.socket().ok_or(SyncError::NotConnected)?;
        self.documents
            .lock()
            .expect("documents poisoned")
            .remove(uri)
            .ok_or_else(|| SyncError::NotOpen(uri.clone()))?;
        socket.did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        })?;
        Ok(())
    }

    pub fn is_open(&self, uri: &Url) -> bool {
        self.documents
            .lock()
            .expect("documents poisoned")
            .contains_key(uri)
    }

    fn negotiated_change_kind(&self) -> TextDocumentSyncKind {
        change_kind_for(self.capabilities().as_ref())
    }

    fn save_wants_text(&self) -> bool {
        save_wants_text_for(self.capabilities().as_ref())
    }

    fn capabilities(&self) -> Option<ServerCapabilities> {
        self.session.watch().borrow().capabilities.clone()
    }

    fn replay_open_documents(&self) {
        let Some(mut socket) = self.session.socket() else {
            return;
        };
        let mut documents = self.documents.lock().expect("documents poisoned");
        for (uri, doc) in documents.iter_mut() {
            doc.version = 0;
            let _ = socket.did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: doc.language_id.clone(),
                    version: 0,
                    text: doc.text.clone(),
                },
            });
        }
    }
}

async fn replay_on_reconnect(sync: Weak<DocumentSync>) {
    let mut socket_rx = match sync.upgrade() {
        Some(strong) => strong.session.watch_socket(),
        None => return,
    };
    // A freshly cloned receiver's "last seen" baseline is inherited from
    // `ServerSession`'s own never-consulted `socket_rx`, which is stuck at
    // the channel's construction-time value -- without this, the very
    // first `changed()` below would spuriously fire for whatever the
    // *current* value already is (e.g. the first successful connect,
    // wrongly read as a "reconnect"). Marking it seen now makes `changed()`
    // only fire on a genuine transition from here on.
    socket_rx.borrow_and_update();
    loop {
        if socket_rx.changed().await.is_err() {
            return;
        }
        let Some(strong) = sync.upgrade() else {
            return;
        };
        if socket_rx.borrow().is_some() {
            strong.replay_open_documents();
        }
    }
}

/// The `didChange` content-change shape to use, per the server's negotiated
/// `textDocument/didChange` capability. Defaults to `FULL` (always valid,
/// just less efficient) when the server didn't specify -- never guesses
/// `INCREMENTAL` without the server asking for it.
fn change_kind_for(capabilities: Option<&ServerCapabilities>) -> TextDocumentSyncKind {
    match capabilities.and_then(|c| c.text_document_sync.clone()) {
        Some(TextDocumentSyncCapability::Kind(kind)) => kind,
        Some(TextDocumentSyncCapability::Options(options)) => {
            options.change.unwrap_or(TextDocumentSyncKind::FULL)
        }
        None => TextDocumentSyncKind::FULL,
    }
}

/// Whether `didSave` should include the document's full text, per
/// `TextDocumentSyncOptions::save::include_text`.
fn save_wants_text_for(capabilities: Option<&ServerCapabilities>) -> bool {
    let Some(TextDocumentSyncCapability::Options(options)) =
        capabilities.and_then(|c| c.text_document_sync.clone())
    else {
        return false;
    };
    match options.save {
        Some(TextDocumentSyncSaveOptions::SaveOptions(save)) => save.include_text.unwrap_or(false),
        Some(TextDocumentSyncSaveOptions::Supported(_)) | None => false,
    }
}

/// LSP `TextEdit` ranges within one file are always relative to the
/// *original* document, never to a state some earlier edit in the same
/// batch already produced. Applying them in array order via naive
/// sequential folding is only safe if none of them shift line/column
/// positions -- which a rename touching the same file more than once
/// routinely does. Sorting by descending start position first (last edit
/// in the document applied first) keeps every not-yet-applied edit's
/// coordinates valid: at the moment it's applied, nothing at or before
/// its own range has been touched yet.
fn sort_edits_for_application(edits: &mut [TextEdit]) {
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.range.start));
}

/// Build the `didChange` content-change list for `edits`: one range per
/// edit for [`TextDocumentSyncKind::INCREMENTAL`], or the whole
/// `full_text_after` in a single event for anything else (including
/// `FULL`, the safe default).
fn content_changes_for(
    sync_kind: TextDocumentSyncKind,
    edits: &[TextEdit],
    full_text_after: &str,
) -> Vec<TextDocumentContentChangeEvent> {
    if sync_kind == TextDocumentSyncKind::FULL {
        vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: full_text_after.to_string(),
        }]
    } else {
        edits
            .iter()
            .cloned()
            .map(|edit| TextDocumentContentChangeEvent {
                range: Some(edit.range),
                range_length: None,
                text: edit.new_text,
            })
            .collect()
    }
}

/// Splice `edit` into `text`, resolving `edit.range` as UTF-16 code unit
/// columns within 0-indexed lines, exactly as the LSP spec's default
/// position encoding requires.
fn apply_edit(text: &str, edit: &TextEdit) -> String {
    let start = position_to_byte_offset(text, edit.range.start);
    let end = position_to_byte_offset(text, edit.range.end);
    let mut result = String::with_capacity(text.len() - (end - start) + edit.new_text.len());
    result.push_str(&text[..start]);
    result.push_str(&edit.new_text);
    result.push_str(&text[end..]);
    result
}

fn position_to_byte_offset(text: &str, position: Position) -> usize {
    let mut byte_offset = 0;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        if line_index as u32 == position.line {
            let mut utf16_units = 0u32;
            for (offset_in_line, ch) in line.char_indices() {
                if utf16_units >= position.character {
                    return byte_offset + offset_in_line;
                }
                utf16_units += ch.len_utf16() as u32;
            }
            return byte_offset + line.trim_end_matches('\n').len();
        }
        byte_offset += line.len();
    }
    byte_offset
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    fn range(start: Position, end: Position) -> Range {
        Range { start, end }
    }

    fn apply_edits_in_order(text: &str, edits: &mut [TextEdit]) -> String {
        sort_edits_for_application(edits);
        let mut result = text.to_string();
        for edit in edits.iter() {
            result = apply_edit(&result, edit);
        }
        result
    }

    #[test]
    fn multiple_edits_on_one_line_apply_correctly_even_when_an_earlier_one_shifts_length() {
        // Two occurrences of "x" on the same line, as a rename touching the
        // same file more than once would produce. Renaming the *first* "x"
        // to the longer "index" shifts every later column on the line;
        // applying edits last-in-document-first (rather than naive array
        // order) is what keeps the second edit's original coordinates
        // valid.
        let text = "let x = x + 1;\n";
        let mut edits = vec![
            TextEdit {
                range: range(pos(0, 4), pos(0, 5)),
                new_text: "index".into(),
            },
            TextEdit {
                range: range(pos(0, 8), pos(0, 9)),
                new_text: "index".into(),
            },
        ];
        assert_eq!(
            apply_edits_in_order(text, &mut edits),
            "let index = index + 1;\n"
        );
    }

    #[test]
    fn edit_order_in_the_input_array_does_not_change_the_result() {
        // Same two edits as above, listed in the opposite array order --
        // `sort_edits_for_application` normalizes this, so the outcome
        // must be identical either way.
        let text = "let x = x + 1;\n";
        let mut edits = vec![
            TextEdit {
                range: range(pos(0, 8), pos(0, 9)),
                new_text: "index".into(),
            },
            TextEdit {
                range: range(pos(0, 4), pos(0, 5)),
                new_text: "index".into(),
            },
        ];
        assert_eq!(
            apply_edits_in_order(text, &mut edits),
            "let index = index + 1;\n"
        );
    }

    #[test]
    fn applies_a_single_line_replacement() {
        let text = "fn add(a: i32, b: i32) -> i32 { a + b }\n";
        let edit = TextEdit {
            range: range(pos(0, 3), pos(0, 6)),
            new_text: "sum".into(),
        };
        assert_eq!(
            apply_edit(text, &edit),
            "fn sum(a: i32, b: i32) -> i32 { a + b }\n"
        );
    }

    #[test]
    fn applies_an_insertion_at_a_zero_width_range() {
        let text = "fn add() {}\n";
        let edit = TextEdit {
            range: range(pos(0, 9), pos(0, 9)),
            new_text: "/* body */".into(),
        };
        assert_eq!(apply_edit(text, &edit), "fn add() /* body */{}\n");
    }

    #[test]
    fn applies_a_multi_line_replacement() {
        let text = "line one\nline two\nline three\n";
        let edit = TextEdit {
            range: range(pos(1, 5), pos(2, 4)),
            new_text: "TWO".into(),
        };
        assert_eq!(apply_edit(text, &edit), "line one\nline TWO three\n");
    }

    #[test]
    fn resolves_utf16_columns_past_a_multi_byte_character() {
        // "héllo" -- 'é' is 2 bytes in UTF-8 but 1 UTF-16 code unit, so the
        // "llo" replacement's start column (2) must land right after 'é' in
        // UTF-16 terms, i.e. UTF-8 byte offset 3, not byte offset 2 (which
        // would land inside 'é's own 2-byte UTF-8 encoding).
        let text = "héllo\n";
        let edit = TextEdit {
            range: range(pos(0, 2), pos(0, 5)),
            new_text: "LLO".into(),
        };
        assert_eq!(apply_edit(text, &edit), "héLLO\n");
    }

    fn sync_options(
        change: Option<TextDocumentSyncKind>,
        save: Option<TextDocumentSyncSaveOptions>,
    ) -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                async_lsp::lsp_types::TextDocumentSyncOptions {
                    open_close: Some(true),
                    change,
                    save,
                    ..Default::default()
                },
            )),
            ..Default::default()
        }
    }

    #[test]
    fn reads_incremental_from_a_bare_kind_capability() {
        let caps = ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(
                TextDocumentSyncKind::INCREMENTAL,
            )),
            ..Default::default()
        };
        assert_eq!(
            change_kind_for(Some(&caps)),
            TextDocumentSyncKind::INCREMENTAL
        );
    }

    #[test]
    fn reads_incremental_from_sync_options() {
        let caps = sync_options(Some(TextDocumentSyncKind::INCREMENTAL), None);
        assert_eq!(
            change_kind_for(Some(&caps)),
            TextDocumentSyncKind::INCREMENTAL
        );
    }

    #[test]
    fn defaults_to_full_when_unspecified() {
        assert_eq!(change_kind_for(None), TextDocumentSyncKind::FULL);
        let caps = sync_options(None, None);
        assert_eq!(change_kind_for(Some(&caps)), TextDocumentSyncKind::FULL);
    }

    #[test]
    fn save_text_follows_include_text_flag() {
        assert!(!save_wants_text_for(None));
        let no_flag = sync_options(None, Some(TextDocumentSyncSaveOptions::Supported(true)));
        assert!(!save_wants_text_for(Some(&no_flag)));
        let wants_text = sync_options(
            None,
            Some(TextDocumentSyncSaveOptions::SaveOptions(
                async_lsp::lsp_types::SaveOptions {
                    include_text: Some(true),
                },
            )),
        );
        assert!(save_wants_text_for(Some(&wants_text)));
    }
}

/// End-to-end tests against a real `rust-analyzer` -- see `sync/real_server.rs`.
#[cfg(test)]
mod real_server;
