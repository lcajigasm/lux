//! Formatting: whole document, a range, and format-on-save, with a
//! formatter configuration resolved per language and per workspace with
//! clear precedence (an explicit per-language override wins; otherwise
//! the workspace default applies).

use std::collections::HashMap;
use std::sync::Arc;

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    DocumentFormattingParams, DocumentRangeFormattingParams, FormattingOptions, Range,
    TextDocumentIdentifier, TextEdit as LspTextEdit, Url, WorkDoneProgressParams,
};

use crate::session::ServerSession;
use crate::sync::{DocumentSync, SyncError, TextEdit};

#[derive(Debug, thiserror::Error)]
pub enum FormattingError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("formatting request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum FormatAndSaveError {
    #[error(transparent)]
    Formatting(#[from] FormattingError),
    #[error(transparent)]
    Sync(#[from] SyncError),
}

/// Per-language formatting behavior: the options sent to the server, and
/// whether to format automatically before `didSave`.
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageFormattingConfig {
    pub options: FormattingOptions,
    pub format_on_save: bool,
}

impl Default for LanguageFormattingConfig {
    fn default() -> Self {
        Self {
            options: FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                ..Default::default()
            },
            format_on_save: false,
        }
    }
}

/// Formatting configuration resolved with clear precedence: an explicit
/// per-language override wins; otherwise the workspace default applies.
#[derive(Debug, Clone, Default)]
pub struct FormattingConfig {
    pub workspace_default: LanguageFormattingConfig,
    pub per_language: HashMap<String, LanguageFormattingConfig>,
}

impl FormattingConfig {
    pub fn for_language(&self, language_id: &str) -> &LanguageFormattingConfig {
        self.per_language
            .get(language_id)
            .unwrap_or(&self.workspace_default)
    }
}

pub struct Formatting {
    session: Arc<ServerSession>,
}

impl Formatting {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    /// Format the whole document, per `textDocument/formatting`.
    pub async fn document(
        &self,
        uri: &Url,
        options: FormattingOptions,
    ) -> Result<Vec<LspTextEdit>, FormattingError> {
        let mut socket = self.session.socket().ok_or(FormattingError::NotConnected)?;
        let response = socket
            .formatting(DocumentFormattingParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                options,
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await?;
        Ok(response.unwrap_or_default())
    }

    /// Format only `range`, per `textDocument/rangeFormatting`.
    pub async fn range(
        &self,
        uri: &Url,
        range: Range,
        options: FormattingOptions,
    ) -> Result<Vec<LspTextEdit>, FormattingError> {
        let mut socket = self.session.socket().ok_or(FormattingError::NotConnected)?;
        let response = socket
            .range_formatting(DocumentRangeFormattingParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range,
                options,
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await?;
        Ok(response.unwrap_or_default())
    }

    /// Format the whole document, apply the result through `sync`, then
    /// send `didSave` -- real format-on-save: what actually gets saved is
    /// the *formatted* text, not a pre-format snapshot with formatting
    /// applied as an afterthought.
    pub async fn format_and_save(
        &self,
        sync: &DocumentSync,
        uri: &Url,
        options: FormattingOptions,
    ) -> Result<(), FormatAndSaveError> {
        let edits = self.document(uri, options).await?;
        if !edits.is_empty() {
            sync.change(uri, edits.into_iter().map(TextEdit::from).collect())?;
        }
        sync.save(uri)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_config_falls_back_to_the_workspace_default() {
        let config = FormattingConfig {
            workspace_default: LanguageFormattingConfig {
                format_on_save: true,
                ..Default::default()
            },
            per_language: HashMap::new(),
        };
        assert!(config.for_language("rust").format_on_save);
    }

    #[test]
    fn a_per_language_override_wins_over_the_workspace_default() {
        let mut config = FormattingConfig {
            workspace_default: LanguageFormattingConfig {
                format_on_save: false,
                ..Default::default()
            },
            per_language: HashMap::new(),
        };
        config.per_language.insert(
            "rust".into(),
            LanguageFormattingConfig {
                options: FormattingOptions {
                    tab_size: 2,
                    insert_spaces: true,
                    ..Default::default()
                },
                format_on_save: true,
            },
        );

        let rust = config.for_language("rust");
        assert!(
            rust.format_on_save,
            "expected the per-language override, not the workspace default"
        );
        assert_eq!(rust.options.tab_size, 2);

        // A language with no override still gets the workspace default.
        let python = config.for_language("python");
        assert!(!python.format_on_save);
        assert_eq!(python.options.tab_size, 4);
    }

    #[test]
    fn default_language_config_uses_sane_baseline_options() {
        let default = LanguageFormattingConfig::default();
        assert_eq!(default.options.tab_size, 4);
        assert!(default.options.insert_spaces);
        assert!(
            !default.format_on_save,
            "format-on-save should be opt-in, not silently on by default"
        );
    }
}

#[cfg(test)]
mod real_server;
