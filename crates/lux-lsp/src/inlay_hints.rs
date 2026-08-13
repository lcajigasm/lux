//! Inlay hints: inferred-type and parameter-name annotations the server
//! wants rendered inline in the editor (not actually part of the source).

use std::sync::Arc;

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    InlayHint as LspInlayHint, InlayHintKind, InlayHintLabel, InlayHintParams, Position, Range,
    TextDocumentIdentifier, Url, WorkDoneProgressParams,
};

use crate::session::ServerSession;

#[derive(Debug, thiserror::Error)]
pub enum InlayHintsError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("inlay hints request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

/// One inlay hint, with its label flattened to plain text -- the server's
/// two possible shapes (a plain string, or an array of
/// `InlayHintLabelPart`s for interactive/composite labels) collapsed into
/// one string, since rendering individually clickable label parts is a
/// `lux-ui` concern this headless client doesn't need to model yet.
#[derive(Debug, Clone)]
pub struct InlayHintEntry {
    pub position: Position,
    pub text: String,
    pub kind: Option<InlayHintKind>,
}

pub struct InlayHints {
    session: Arc<ServerSession>,
}

impl InlayHints {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    /// Inlay hints for the given visible `range` of `uri`, per spec --
    /// servers compute hints for a range, not a whole (potentially huge)
    /// document.
    pub async fn inlay_hints(
        &self,
        uri: &Url,
        range: Range,
    ) -> Result<Vec<InlayHintEntry>, InlayHintsError> {
        let mut socket = self.session.socket().ok_or(InlayHintsError::NotConnected)?;
        let response = socket
            .inlay_hint(InlayHintParams {
                work_done_progress_params: WorkDoneProgressParams::default(),
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range,
            })
            .await?;
        Ok(response
            .unwrap_or_default()
            .into_iter()
            .map(flatten_hint)
            .collect())
    }
}

fn flatten_hint(hint: LspInlayHint) -> InlayHintEntry {
    InlayHintEntry {
        position: hint.position,
        text: flatten_label(hint.label),
        kind: hint.kind,
    }
}

fn flatten_label(label: InlayHintLabel) -> String {
    match label {
        InlayHintLabel::String(text) => text,
        InlayHintLabel::LabelParts(parts) => parts
            .into_iter()
            .map(|part| part.value)
            .collect::<Vec<_>>()
            .join(""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_lsp::lsp_types::InlayHintLabelPart;

    #[test]
    fn flattens_a_plain_string_label() {
        assert_eq!(
            flatten_label(InlayHintLabel::String(": i32".into())),
            ": i32"
        );
    }

    #[test]
    fn flattens_label_parts_by_concatenating_their_values() {
        let parts = vec![
            InlayHintLabelPart {
                value: "x".into(),
                ..Default::default()
            },
            InlayHintLabelPart {
                value: ": i32".into(),
                ..Default::default()
            },
        ];
        assert_eq!(flatten_label(InlayHintLabel::LabelParts(parts)), "x: i32");
    }
}

#[cfg(test)]
mod real_server;
