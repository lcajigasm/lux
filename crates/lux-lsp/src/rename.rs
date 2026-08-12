//! Cross-file rename via `textDocument/rename`, with a multi-file preview
//! and per-hunk accept/reject before applying -- never string replacement.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    Position, PrepareRenameResponse, RenameParams, TextDocumentIdentifier,
    TextDocumentPositionParams, TextEdit as LspTextEdit, Url, WorkDoneProgressParams,
    WorkspaceEdit,
};

use crate::code_actions::{self, FileChange};
use crate::session::ServerSession;
use crate::sync::{DocumentSync, SyncError, TextEdit};

#[derive(Debug, thiserror::Error)]
pub enum RenameError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("rename request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum RenameApplyError {
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(
        "rename edit also creates/renames/deletes {0}, which this client cannot apply (only text edits to already-open documents can be)"
    )]
    UnsupportedFileOperation(Url),
}

/// One selectable edit within a multi-file rename preview -- one file, one
/// occurrence of the renamed symbol -- addressable by `id` so a caller can
/// accept or reject it individually before applying.
#[derive(Debug, Clone)]
pub struct RenameHunk {
    pub id: usize,
    pub uri: Url,
    pub edit: LspTextEdit,
}

/// A normalized, previewable rename: every hunk the server's
/// `WorkspaceEdit` describes, plus any file-level operations it also
/// requested (e.g. renaming a module's file to match) -- kept separate
/// since this client can preview but not apply those (see
/// [`Rename::apply`]).
#[derive(Debug, Clone, Default)]
pub struct RenamePreview {
    pub hunks: Vec<RenameHunk>,
    pub file_operations: Vec<FileChange>,
}

impl RenamePreview {
    /// The distinct files this rename touches with text edits, in order.
    pub fn files(&self) -> Vec<&Url> {
        let mut seen = Vec::new();
        for hunk in &self.hunks {
            if !seen.contains(&&hunk.uri) {
                seen.push(&hunk.uri);
            }
        }
        seen
    }
}

pub struct Rename {
    session: Arc<ServerSession>,
}

impl Rename {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    /// Validate the symbol at `position` can be renamed, via
    /// `textDocument/prepareRename` -- real editors call this before
    /// showing a rename UI, both to reject an invalid target early and to
    /// get the current name as the input's starting value.
    pub async fn prepare(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Option<PrepareRenameResponse>, RenameError> {
        let mut socket = self.session.socket().ok_or(RenameError::NotConnected)?;
        Ok(socket
            .prepare_rename(TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            })
            .await?)
    }

    /// Request the rename itself: every reference to the symbol at
    /// `position`, across however many files the server determines,
    /// renamed to `new_name`.
    pub async fn rename(
        &self,
        uri: &Url,
        position: Position,
        new_name: &str,
    ) -> Result<Option<WorkspaceEdit>, RenameError> {
        let mut socket = self.session.socket().ok_or(RenameError::NotConnected)?;
        Ok(socket
            .rename(RenameParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                new_name: new_name.to_string(),
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await?)
    }

    /// Apply exactly the hunks whose id is in `accepted`, across however
    /// many files they span, as one atomic unit -- rejected hunks are
    /// simply never sent. A rename that also requests a file
    /// create/rename/delete is refused outright (nothing applied): this
    /// client only owns already-open in-memory documents, not the
    /// filesystem, so silently dropping that half of the edit would be
    /// less than what it actually asked for.
    pub fn apply(
        &self,
        sync: &DocumentSync,
        preview: &RenamePreview,
        accepted: &HashSet<usize>,
    ) -> Result<(), RenameApplyError> {
        let edits_by_uri = hunks_to_edit_map(preview, accepted)?;
        sync.apply_all(&edits_by_uri)?;
        Ok(())
    }
}

/// The pure half of [`Rename::apply`]: reject up front (nothing built) if
/// the preview also carries a file operation, otherwise build the edit map
/// for exactly the accepted hunks.
fn hunks_to_edit_map(
    preview: &RenamePreview,
    accepted: &HashSet<usize>,
) -> Result<BTreeMap<Url, Vec<TextEdit>>, RenameApplyError> {
    if let Some(op) = preview.file_operations.first() {
        let uri = match op {
            FileChange::Create { uri } | FileChange::Delete { uri } => uri.clone(),
            FileChange::Rename { old_uri, .. } => old_uri.clone(),
            FileChange::Edit { uri, .. } => uri.clone(),
        };
        return Err(RenameApplyError::UnsupportedFileOperation(uri));
    }
    let mut edits_by_uri: BTreeMap<Url, Vec<TextEdit>> = BTreeMap::new();
    for hunk in &preview.hunks {
        if accepted.contains(&hunk.id) {
            edits_by_uri
                .entry(hunk.uri.clone())
                .or_default()
                .push(hunk.edit.clone().into());
        }
    }
    Ok(edits_by_uri)
}

/// Normalize a rename's `WorkspaceEdit` into a previewable, per-hunk list.
/// Reuses `code_actions::preview`'s `WorkspaceEdit` normalization (the
/// same shape, whichever of its forms the server used) and flattens the
/// edit-carrying files into individually addressable hunks.
pub fn preview(edit: &WorkspaceEdit) -> RenamePreview {
    let mut result = RenamePreview::default();
    let mut next_id = 0;
    for change in code_actions::preview(edit) {
        match change {
            FileChange::Edit { uri, edits } => {
                for edit in edits {
                    result.hunks.push(RenameHunk {
                        id: next_id,
                        uri: uri.clone(),
                        edit,
                    });
                    next_id += 1;
                }
            }
            other => result.file_operations.push(other),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri(name: &str) -> Url {
        Url::parse(&format!("file:///{name}")).unwrap()
    }

    fn edit(text: &str) -> LspTextEdit {
        LspTextEdit {
            range: async_lsp::lsp_types::Range::default(),
            new_text: text.into(),
        }
    }

    fn edits_map(edit: &WorkspaceEdit) -> std::collections::HashMap<Url, Vec<LspTextEdit>> {
        match &edit.changes {
            Some(changes) => changes.clone(),
            None => Default::default(),
        }
    }

    fn workspace_edit(files: Vec<(Url, Vec<LspTextEdit>)>) -> WorkspaceEdit {
        WorkspaceEdit {
            changes: Some(files.into_iter().collect()),
            ..Default::default()
        }
    }

    #[test]
    fn preview_flattens_every_file_into_individually_addressable_hunks() {
        let we = workspace_edit(vec![
            (uri("a.rs"), vec![edit("one"), edit("two")]),
            (uri("b.rs"), vec![edit("three")]),
        ]);
        let rename_preview = preview(&we);
        assert_eq!(rename_preview.hunks.len(), 3);
        let ids: Vec<usize> = rename_preview.hunks.iter().map(|h| h.id).collect();
        assert_eq!(ids, vec![0, 1, 2], "expected stable, distinct hunk ids");
        assert_eq!(rename_preview.files(), vec![&uri("a.rs"), &uri("b.rs")]);
        assert!(rename_preview.file_operations.is_empty());
        // Sanity: the map we built from actually matches what preview saw.
        assert_eq!(edits_map(&we).len(), 2);
    }

    #[test]
    fn preview_separates_file_operations_from_edit_hunks() {
        let we = WorkspaceEdit {
            document_changes: Some(async_lsp::lsp_types::DocumentChanges::Operations(vec![
                async_lsp::lsp_types::DocumentChangeOperation::Op(
                    async_lsp::lsp_types::ResourceOp::Rename(async_lsp::lsp_types::RenameFile {
                        old_uri: uri("old.rs"),
                        new_uri: uri("new.rs"),
                        options: None,
                        annotation_id: None,
                    }),
                ),
            ])),
            ..Default::default()
        };
        let rename_preview = preview(&we);
        assert!(rename_preview.hunks.is_empty());
        assert_eq!(rename_preview.file_operations.len(), 1);
    }

    #[test]
    fn hunks_to_edit_map_rejects_a_rename_that_also_touches_the_filesystem() {
        let mut rename_preview = RenamePreview::default();
        rename_preview.file_operations.push(FileChange::Rename {
            old_uri: uri("old.rs"),
            new_uri: uri("new.rs"),
        });

        match hunks_to_edit_map(&rename_preview, &HashSet::new()) {
            Err(RenameApplyError::UnsupportedFileOperation(u)) => assert_eq!(u, uri("old.rs")),
            other => panic!("expected UnsupportedFileOperation, got {other:?}"),
        }
    }

    #[test]
    fn hunks_to_edit_map_only_includes_accepted_ids() {
        let we = workspace_edit(vec![(uri("a.rs"), vec![edit("keep"), edit("skip")])]);
        let rename_preview = preview(&we);
        let accepted: HashSet<usize> = [rename_preview.hunks[0].id].into_iter().collect();

        let edits_by_uri = hunks_to_edit_map(&rename_preview, &accepted).unwrap();
        assert_eq!(edits_by_uri[&uri("a.rs")].len(), 1);
        assert_eq!(edits_by_uri[&uri("a.rs")][0].new_text, "keep");
    }

    #[test]
    fn hunks_to_edit_map_with_no_accepted_ids_applies_nothing() {
        let we = workspace_edit(vec![(uri("a.rs"), vec![edit("skip")])]);
        let rename_preview = preview(&we);
        let edits_by_uri = hunks_to_edit_map(&rename_preview, &HashSet::new()).unwrap();
        assert!(edits_by_uri.is_empty());
    }
}

#[cfg(test)]
mod real_server;
