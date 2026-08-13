//! Code actions and quick fixes, applying the resulting `WorkspaceEdit`
//! transactionally -- including edits spanning several files, all or
//! nothing.

use std::collections::BTreeMap;
use std::sync::Arc;

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    CodeAction, CodeActionContext, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, Diagnostic, DocumentChangeOperation, DocumentChanges, OneOf,
    PartialResultParams, Range, ResourceOp, TextDocumentEdit, TextDocumentIdentifier,
    TextEdit as LspTextEdit, Url, WorkDoneProgressParams, WorkspaceEdit,
};

use crate::session::ServerSession;
use crate::sync::{DocumentSync, SyncError, TextEdit};

#[derive(Debug, thiserror::Error)]
pub enum CodeActionsError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("code action request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(
        "workspace edit targets {0}, which this client cannot create/rename/delete (only edits to already-open documents can be applied) -- nothing was applied"
    )]
    UnsupportedFileOperation(Url),
}

/// One file-level change a `WorkspaceEdit` can describe, normalized so a
/// caller can preview the whole edit -- which files, what changes -- before
/// committing any of it.
#[derive(Debug, Clone)]
pub enum FileChange {
    Edit { uri: Url, edits: Vec<LspTextEdit> },
    Create { uri: Url },
    Rename { old_uri: Url, new_uri: Url },
    Delete { uri: Url },
}

pub struct CodeActions {
    session: Arc<ServerSession>,
}

impl CodeActions {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    pub async fn code_actions(
        &self,
        uri: &Url,
        range: Range,
        diagnostics: Vec<Diagnostic>,
    ) -> Result<Vec<CodeActionOrCommand>, CodeActionsError> {
        let mut socket = self
            .session
            .socket()
            .ok_or(CodeActionsError::NotConnected)?;
        let response = socket
            .code_action(CodeActionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range,
                context: CodeActionContext {
                    diagnostics,
                    only: None,
                    trigger_kind: None,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await?;
        Ok(response.unwrap_or_default())
    }

    /// Lazily resolve an action's `edit` if it doesn't already carry one
    /// and the server advertised `codeActionProvider.resolveProvider` --
    /// some servers return code actions title-only until resolved.
    pub async fn resolve(&self, action: CodeAction) -> Result<CodeAction, CodeActionsError> {
        if action.edit.is_some() || !self.resolve_supported() {
            return Ok(action);
        }
        let mut socket = self
            .session
            .socket()
            .ok_or(CodeActionsError::NotConnected)?;
        Ok(socket.code_action_resolve(action).await?)
    }

    /// Apply a `WorkspaceEdit` transactionally: text edits to
    /// currently-open documents (potentially several) are applied all or
    /// nothing via `sync`. A `WorkspaceEdit` that also creates, renames,
    /// or deletes a file is rejected outright -- this client only owns
    /// already-open in-memory documents, not the filesystem, so applying
    /// just the text-edit part while silently dropping a file operation
    /// would be less than what the edit actually asked for.
    pub fn apply(&self, sync: &DocumentSync, edit: &WorkspaceEdit) -> Result<(), ApplyError> {
        let edits_by_uri = text_edits_only(preview(edit))?;
        sync.apply_all(&edits_by_uri)?;
        Ok(())
    }

    fn resolve_supported(&self) -> bool {
        matches!(
            self.session.watch().borrow().capabilities.as_ref().and_then(|c| c.code_action_provider.as_ref()),
            Some(CodeActionProviderCapability::Options(options)) if options.resolve_provider == Some(true)
        )
    }
}

/// Normalize a `WorkspaceEdit` into a previewable, ordered list of
/// per-file changes. Per spec, `document_changes` takes precedence over
/// the older, simpler `changes` map when a server sends both.
pub fn preview(edit: &WorkspaceEdit) -> Vec<FileChange> {
    if let Some(document_changes) = &edit.document_changes {
        return match document_changes {
            DocumentChanges::Edits(edits) => edits.iter().map(file_change_from_edit).collect(),
            DocumentChanges::Operations(ops) => {
                ops.iter().map(file_change_from_operation).collect()
            }
        };
    }
    let Some(changes) = &edit.changes else {
        return Vec::new();
    };
    let mut sorted: Vec<_> = changes.iter().collect();
    sorted.sort_by(|(a, _), (b, _)| a.as_str().cmp(b.as_str()));
    sorted
        .into_iter()
        .map(|(uri, edits)| FileChange::Edit {
            uri: uri.clone(),
            edits: edits.clone(),
        })
        .collect()
}

/// The text-edit-only subset of a normalized preview, as the shape
/// [`crate::sync::DocumentSync::apply_all`] takes -- `Err` (with nothing
/// converted) the moment a resource operation appears, since this client
/// cannot honor it.
fn text_edits_only(changes: Vec<FileChange>) -> Result<BTreeMap<Url, Vec<TextEdit>>, ApplyError> {
    let mut by_uri: BTreeMap<Url, Vec<TextEdit>> = BTreeMap::new();
    for change in changes {
        match change {
            FileChange::Edit { uri, edits } => {
                by_uri
                    .entry(uri)
                    .or_default()
                    .extend(edits.into_iter().map(TextEdit::from));
            }
            FileChange::Create { uri } | FileChange::Delete { uri } => {
                return Err(ApplyError::UnsupportedFileOperation(uri));
            }
            FileChange::Rename { old_uri, .. } => {
                return Err(ApplyError::UnsupportedFileOperation(old_uri));
            }
        }
    }
    Ok(by_uri)
}

fn file_change_from_edit(edit: &TextDocumentEdit) -> FileChange {
    let edits = edit
        .edits
        .iter()
        .map(|e| match e {
            OneOf::Left(text_edit) => text_edit.clone(),
            OneOf::Right(annotated) => annotated.text_edit.clone(),
        })
        .collect();
    FileChange::Edit {
        uri: edit.text_document.uri.clone(),
        edits,
    }
}

fn file_change_from_operation(op: &DocumentChangeOperation) -> FileChange {
    match op {
        DocumentChangeOperation::Edit(edit) => file_change_from_edit(edit),
        DocumentChangeOperation::Op(ResourceOp::Create(create)) => FileChange::Create {
            uri: create.uri.clone(),
        },
        DocumentChangeOperation::Op(ResourceOp::Rename(rename)) => FileChange::Rename {
            old_uri: rename.old_uri.clone(),
            new_uri: rename.new_uri.clone(),
        },
        DocumentChangeOperation::Op(ResourceOp::Delete(delete)) => FileChange::Delete {
            uri: delete.uri.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri(name: &str) -> Url {
        Url::parse(&format!("file:///{name}")).unwrap()
    }

    fn edit(text: &str) -> LspTextEdit {
        LspTextEdit {
            range: Range::default(),
            new_text: text.into(),
        }
    }

    #[test]
    fn preview_normalizes_the_legacy_changes_map_in_sorted_order() {
        let mut changes = std::collections::HashMap::new();
        changes.insert(uri("b.rs"), vec![edit("b")]);
        changes.insert(uri("a.rs"), vec![edit("a")]);
        let workspace_edit = WorkspaceEdit {
            changes: Some(changes),
            ..Default::default()
        };

        let files = preview(&workspace_edit);
        let uris: Vec<&str> = files
            .iter()
            .map(|f| match f {
                FileChange::Edit { uri, .. } => uri.path(),
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(uris, vec!["/a.rs", "/b.rs"]);
    }

    #[test]
    fn preview_normalizes_document_changes_edits() {
        let workspace_edit = WorkspaceEdit {
            document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                text_document: async_lsp::lsp_types::OptionalVersionedTextDocumentIdentifier {
                    uri: uri("a.rs"),
                    version: Some(3),
                },
                edits: vec![OneOf::Left(edit("x"))],
            }])),
            ..Default::default()
        };
        let files = preview(&workspace_edit);
        match &files[..] {
            [FileChange::Edit { uri, edits }] => {
                assert_eq!(uri.path(), "/a.rs");
                assert_eq!(edits.len(), 1);
            }
            other => panic!("expected one Edit, got {other:?}"),
        }
    }

    #[test]
    fn preview_prefers_document_changes_over_the_legacy_map() {
        let mut changes = std::collections::HashMap::new();
        changes.insert(uri("legacy.rs"), vec![edit("legacy")]);
        let workspace_edit = WorkspaceEdit {
            changes: Some(changes),
            document_changes: Some(DocumentChanges::Operations(vec![
                DocumentChangeOperation::Op(ResourceOp::Create(async_lsp::lsp_types::CreateFile {
                    uri: uri("new.rs"),
                    options: None,
                    annotation_id: None,
                })),
            ])),
            ..Default::default()
        };
        let files = preview(&workspace_edit);
        assert!(matches!(&files[..], [FileChange::Create { uri }] if uri.path() == "/new.rs"));
    }

    #[test]
    fn preview_normalizes_resource_operations() {
        let workspace_edit = WorkspaceEdit {
            document_changes: Some(DocumentChanges::Operations(vec![
                DocumentChangeOperation::Op(ResourceOp::Rename(async_lsp::lsp_types::RenameFile {
                    old_uri: uri("old.rs"),
                    new_uri: uri("new.rs"),
                    options: None,
                    annotation_id: None,
                })),
                DocumentChangeOperation::Op(ResourceOp::Delete(async_lsp::lsp_types::DeleteFile {
                    uri: uri("gone.rs"),
                    options: None,
                })),
            ])),
            ..Default::default()
        };
        let files = preview(&workspace_edit);
        assert!(
            matches!(&files[0], FileChange::Rename { old_uri, new_uri } if old_uri.path() == "/old.rs" && new_uri.path() == "/new.rs")
        );
        assert!(matches!(&files[1], FileChange::Delete { uri } if uri.path() == "/gone.rs"));
    }

    #[test]
    fn text_edits_only_rejects_a_workspace_edit_with_any_resource_operation() {
        let changes = vec![
            FileChange::Edit {
                uri: uri("a.rs"),
                edits: vec![edit("a")],
            },
            FileChange::Create { uri: uri("b.rs") },
        ];
        match text_edits_only(changes) {
            Err(ApplyError::UnsupportedFileOperation(u)) => assert_eq!(u.path(), "/b.rs"),
            other => panic!("expected UnsupportedFileOperation, got {other:?}"),
        }
    }

    #[test]
    fn text_edits_only_merges_edits_for_the_same_file() {
        let changes = vec![
            FileChange::Edit {
                uri: uri("a.rs"),
                edits: vec![edit("first")],
            },
            FileChange::Edit {
                uri: uri("a.rs"),
                edits: vec![edit("second")],
            },
        ];
        let by_uri = text_edits_only(changes).unwrap();
        assert_eq!(by_uri[&uri("a.rs")].len(), 2);
    }
}

#[cfg(test)]
mod real_server;
