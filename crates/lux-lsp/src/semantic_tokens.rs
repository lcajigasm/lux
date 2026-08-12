//! Semantic tokens: decodes the server's delta-encoded token stream into
//! absolute `(line, start_char, length)` spans with their real type/modifier
//! names resolved against the server's advertised legend.
//!
//! Combining these spans with tree-sitter's own highlighting (the master
//! prompt's "semantic tokens combinados con el resaltado de tree-sitter")
//! is a rendering-layer decision -- which span wins where they overlap --
//! that belongs to `lux-ui` once it wires up both `lux-syntax` and
//! `lux-lsp`, same as every other Phase 7 deliverable's headless-library
//! boundary. This module's job is the necessary piece before that's even
//! possible: turning the wire format into real, usable spans.

use std::sync::Arc;

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    SemanticToken, SemanticTokensLegend, SemanticTokensParams, SemanticTokensResult,
    SemanticTokensServerCapabilities, TextDocumentIdentifier, Url, WorkDoneProgressParams,
};

use crate::session::ServerSession;

#[derive(Debug, thiserror::Error)]
pub enum SemanticTokensError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("server did not advertise semantic tokens support")]
    NotSupported,
    #[error("semantic tokens request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

/// One semantic token at an absolute position, with its type/modifiers
/// resolved to their real names via the server's legend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedToken {
    pub line: u32,
    pub start_char: u32,
    pub length: u32,
    pub token_type: String,
    pub modifiers: Vec<String>,
}

pub struct SemanticTokensClient {
    session: Arc<ServerSession>,
}

impl SemanticTokensClient {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    /// The full document's semantic tokens, decoded to absolute positions.
    pub async fn full(&self, uri: &Url) -> Result<Vec<ResolvedToken>, SemanticTokensError> {
        let legend = self.legend().ok_or(SemanticTokensError::NotSupported)?;
        let mut socket = self
            .session
            .socket()
            .ok_or(SemanticTokensError::NotConnected)?;
        let response = socket
            .semantic_tokens_full(SemanticTokensParams {
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: async_lsp::lsp_types::PartialResultParams::default(),
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            })
            .await?;
        let raw = match response {
            Some(SemanticTokensResult::Tokens(tokens)) => tokens.data,
            Some(SemanticTokensResult::Partial(partial)) => partial.data,
            None => Vec::new(),
        };
        Ok(resolve_tokens(&raw, &legend))
    }

    fn legend(&self) -> Option<SemanticTokensLegend> {
        legend_from_capabilities(self.session.watch().borrow().capabilities.as_ref()?)
    }
}

fn legend_from_capabilities(
    capabilities: &async_lsp::lsp_types::ServerCapabilities,
) -> Option<SemanticTokensLegend> {
    match capabilities.semantic_tokens_provider.as_ref()? {
        SemanticTokensServerCapabilities::SemanticTokensOptions(options) => {
            Some(options.legend.clone())
        }
        SemanticTokensServerCapabilities::SemanticTokensRegistrationOptions(options) => {
            Some(options.semantic_tokens_options.legend.clone())
        }
    }
}

/// Decode the LSP delta encoding (each token's line/start are relative to
/// the *previous* token, restarting the column from zero whenever the line
/// advances) into absolute positions, and resolve each token's type index
/// and modifier bitset against the legend into real names.
fn resolve_tokens(raw: &[SemanticToken], legend: &SemanticTokensLegend) -> Vec<ResolvedToken> {
    let mut line = 0u32;
    let mut start_char = 0u32;
    let mut resolved = Vec::with_capacity(raw.len());
    for token in raw {
        if token.delta_line == 0 {
            start_char += token.delta_start;
        } else {
            line += token.delta_line;
            start_char = token.delta_start;
        }
        let token_type = legend
            .token_types
            .get(token.token_type as usize)
            .map(|t| t.as_str().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let modifiers = legend
            .token_modifiers
            .iter()
            .enumerate()
            .filter(|(bit, _)| token.token_modifiers_bitset & (1 << bit) != 0)
            .map(|(_, modifier)| modifier.as_str().to_string())
            .collect();
        resolved.push(ResolvedToken {
            line,
            start_char,
            length: token.length,
            token_type,
            modifiers,
        });
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_lsp::lsp_types::{SemanticTokenModifier, SemanticTokenType};

    fn legend() -> SemanticTokensLegend {
        SemanticTokensLegend {
            token_types: vec![SemanticTokenType::FUNCTION, SemanticTokenType::VARIABLE],
            token_modifiers: vec![
                SemanticTokenModifier::DECLARATION,
                SemanticTokenModifier::READONLY,
            ],
        }
    }

    fn raw(
        delta_line: u32,
        delta_start: u32,
        length: u32,
        token_type: u32,
        modifiers_bitset: u32,
    ) -> SemanticToken {
        SemanticToken {
            delta_line,
            delta_start,
            length,
            token_type,
            token_modifiers_bitset: modifiers_bitset,
        }
    }

    #[test]
    fn first_token_is_already_absolute() {
        let resolved = resolve_tokens(&[raw(2, 5, 4, 0, 0)], &legend());
        assert_eq!(resolved[0].line, 2);
        assert_eq!(resolved[0].start_char, 5);
    }

    #[test]
    fn same_line_deltas_accumulate_the_column() {
        let resolved = resolve_tokens(
            &[raw(0, 0, 3, 0, 0), raw(0, 4, 3, 0, 0), raw(0, 4, 3, 0, 0)],
            &legend(),
        );
        assert_eq!(
            resolved
                .iter()
                .map(|t| (t.line, t.start_char))
                .collect::<Vec<_>>(),
            vec![(0, 0), (0, 4), (0, 8)]
        );
    }

    #[test]
    fn a_new_line_resets_the_column_to_the_deltas_own_value() {
        let resolved = resolve_tokens(&[raw(0, 10, 3, 0, 0), raw(1, 2, 3, 0, 0)], &legend());
        assert_eq!((resolved[1].line, resolved[1].start_char), (1, 2));
    }

    #[test]
    fn resolves_token_type_and_modifier_names_from_the_legend() {
        let resolved = resolve_tokens(&[raw(0, 0, 3, 1, 0b11)], &legend());
        assert_eq!(resolved[0].token_type, "variable");
        assert_eq!(resolved[0].modifiers, vec!["declaration", "readonly"]);
    }

    #[test]
    fn an_out_of_range_token_type_falls_back_to_unknown_instead_of_panicking() {
        let resolved = resolve_tokens(&[raw(0, 0, 3, 99, 0)], &legend());
        assert_eq!(resolved[0].token_type, "unknown");
    }
}

#[cfg(test)]
mod real_server;
