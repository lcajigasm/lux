//! Signature help: the active signature and its active parameter, with
//! `ParameterLabel::LabelOffsets` resolved against the signature's own
//! label text (UTF-16 code unit offsets, per the LSP spec).

use std::sync::Arc;

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    ParameterLabel, Position, SignatureHelp, SignatureHelpParams, SignatureInformation,
    TextDocumentIdentifier, TextDocumentPositionParams, Url, WorkDoneProgressParams,
};

use crate::session::ServerSession;

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("signature help request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

pub struct Signature {
    session: Arc<ServerSession>,
}

impl Signature {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    pub async fn signature_help(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Option<SignatureHelp>, SignatureError> {
        let mut socket = self.session.socket().ok_or(SignatureError::NotConnected)?;
        let response = socket
            .signature_help(SignatureHelpParams {
                context: None,
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await?;
        Ok(response)
    }
}

/// The signature a UI should show first: `active_signature`'s index, or
/// the first signature when the server didn't pick one.
pub fn active_signature(help: &SignatureHelp) -> Option<&SignatureInformation> {
    help.active_signature
        .and_then(|index| help.signatures.get(index as usize))
        .or_else(|| help.signatures.first())
}

/// The active parameter's own label text, resolving a `LabelOffsets` pair
/// against `signature.label` if that's the shape the server sent (rather
/// than a plain `Simple` string). `active_parameter` on `SignatureHelp`
/// falls back to the signature's own `active_parameter` when unset, per
/// spec.
pub fn active_parameter_label<'a>(
    help: &SignatureHelp,
    signature: &'a SignatureInformation,
) -> Option<&'a str> {
    let index = help.active_parameter.or(signature.active_parameter)?;
    let parameter = signature.parameters.as_ref()?.get(index as usize)?;
    match &parameter.label {
        ParameterLabel::Simple(text) => Some(text.as_str()),
        ParameterLabel::LabelOffsets([start, end]) => {
            let start = utf16_offset_to_byte(&signature.label, *start);
            let end = utf16_offset_to_byte(&signature.label, *end);
            signature.label.get(start..end)
        }
    }
}

/// Resolve a UTF-16 code unit offset into `text` (assumed single-line, as
/// a signature label always is) to a UTF-8 byte offset.
fn utf16_offset_to_byte(text: &str, utf16_offset: u32) -> usize {
    let mut utf16_units = 0u32;
    for (byte_index, ch) in text.char_indices() {
        if utf16_units >= utf16_offset {
            return byte_index;
        }
        utf16_units += ch.len_utf16() as u32;
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(label: &str, parameters: Vec<ParameterLabel>) -> SignatureInformation {
        SignatureInformation {
            label: label.into(),
            documentation: None,
            parameters: Some(
                parameters
                    .into_iter()
                    .map(|label| async_lsp::lsp_types::ParameterInformation {
                        label,
                        documentation: None,
                    })
                    .collect(),
            ),
            active_parameter: None,
        }
    }

    #[test]
    fn active_signature_uses_the_advertised_index() {
        let help = SignatureHelp {
            signatures: vec![signature("a()", vec![]), signature("b()", vec![])],
            active_signature: Some(1),
            active_parameter: None,
        };
        assert_eq!(active_signature(&help).unwrap().label, "b()");
    }

    #[test]
    fn active_signature_falls_back_to_the_first_when_unset() {
        let help = SignatureHelp {
            signatures: vec![signature("only()", vec![])],
            active_signature: None,
            active_parameter: None,
        };
        assert_eq!(active_signature(&help).unwrap().label, "only()");
    }

    #[test]
    fn active_parameter_label_resolves_a_simple_label() {
        let sig = signature(
            "add(a, b)",
            vec![
                ParameterLabel::Simple("a".into()),
                ParameterLabel::Simple("b".into()),
            ],
        );
        let help = SignatureHelp {
            signatures: vec![sig.clone()],
            active_signature: Some(0),
            active_parameter: Some(1),
        };
        assert_eq!(active_parameter_label(&help, &sig), Some("b"));
    }

    #[test]
    fn active_parameter_label_resolves_utf16_offsets_past_a_multi_byte_character() {
        // "café(a: i32)" -- 'é' is 2 bytes in UTF-8 but 1 UTF-16 unit, so
        // the "a" parameter's offsets (5, 6) must land after "café(", not
        // partway through 'é's own encoding.
        let label = "café(a: i32)";
        let sig = signature(label, vec![ParameterLabel::LabelOffsets([5, 6])]);
        let help = SignatureHelp {
            signatures: vec![sig.clone()],
            active_signature: Some(0),
            active_parameter: Some(0),
        };
        assert_eq!(active_parameter_label(&help, &sig), Some("a"));
    }

    #[test]
    fn active_parameter_falls_back_to_the_signatures_own_value() {
        let mut sig = signature("f(x)", vec![ParameterLabel::Simple("x".into())]);
        sig.active_parameter = Some(0);
        let help = SignatureHelp {
            signatures: vec![sig.clone()],
            active_signature: Some(0),
            active_parameter: None,
        };
        assert_eq!(active_parameter_label(&help, &sig), Some("x"));
    }

    #[test]
    fn no_active_parameter_returns_none() {
        let sig = signature("f()", vec![]);
        let help = SignatureHelp {
            signatures: vec![sig.clone()],
            active_signature: Some(0),
            active_parameter: None,
        };
        assert_eq!(active_parameter_label(&help, &sig), None);
    }
}

#[cfg(test)]
mod real_server;
