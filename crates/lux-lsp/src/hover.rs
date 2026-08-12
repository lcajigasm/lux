//! Hover: flattens the server's `MarkedString`/`MarkupContent` response
//! shapes into one plain text block plus the range it applies to.

use std::sync::Arc;

use async_lsp::LanguageServer;
use async_lsp::lsp_types::{
    Hover as LspHover, HoverContents, HoverParams, MarkedString, MarkupContent, Position, Range,
    TextDocumentIdentifier, TextDocumentPositionParams, Url, WorkDoneProgressParams,
};

use crate::session::ServerSession;

#[derive(Debug, thiserror::Error)]
pub enum HoverError {
    #[error("language server is not connected")]
    NotConnected,
    #[error("hover request failed: {0}")]
    Request(#[from] async_lsp::Error),
}

/// A hover result, flattened to plain/markdown text -- the server's three
/// possible shapes (`MarkedString` scalar, `MarkedString` array, or a
/// single `MarkupContent`) collapsed into one string.
#[derive(Debug, Clone)]
pub struct HoverText {
    pub text: String,
    pub range: Option<Range>,
}

pub struct Hover {
    session: Arc<ServerSession>,
}

impl Hover {
    pub fn new(session: Arc<ServerSession>) -> Self {
        Self { session }
    }

    pub async fn hover(
        &self,
        uri: &Url,
        position: Position,
    ) -> Result<Option<HoverText>, HoverError> {
        let mut socket = self.session.socket().ok_or(HoverError::NotConnected)?;
        let response = socket
            .hover(HoverParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
            .await?;
        Ok(response.map(flatten_hover))
    }
}

fn flatten_hover(hover: LspHover) -> HoverText {
    let text = match hover.contents {
        HoverContents::Scalar(marked) => marked_string_text(marked),
        HoverContents::Array(items) => items
            .into_iter()
            .map(marked_string_text)
            .collect::<Vec<_>>()
            .join("\n\n"),
        HoverContents::Markup(MarkupContent { value, .. }) => value,
    };
    HoverText {
        text,
        range: hover.range,
    }
}

fn marked_string_text(marked: MarkedString) -> String {
    match marked {
        MarkedString::String(text) => text,
        MarkedString::LanguageString(language_string) => language_string.value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_lsp::lsp_types::LanguageString;

    fn pos(line: u32, character: u32) -> Position {
        Position { line, character }
    }

    #[test]
    fn flattens_a_scalar_language_string() {
        let hover = LspHover {
            contents: HoverContents::Scalar(MarkedString::LanguageString(LanguageString {
                language: "rust".into(),
                value: "fn one() -> i32".into(),
            })),
            range: None,
        };
        assert_eq!(flatten_hover(hover).text, "fn one() -> i32");
    }

    #[test]
    fn flattens_an_array_by_joining_with_blank_lines() {
        let hover = LspHover {
            contents: HoverContents::Array(vec![
                MarkedString::String("first".into()),
                MarkedString::String("second".into()),
            ]),
            range: None,
        };
        assert_eq!(flatten_hover(hover).text, "first\n\nsecond");
    }

    #[test]
    fn flattens_markup_content_and_keeps_the_range() {
        let range = Range {
            start: pos(0, 0),
            end: pos(0, 5),
        };
        let hover = LspHover {
            contents: HoverContents::Markup(MarkupContent {
                kind: async_lsp::lsp_types::MarkupKind::Markdown,
                value: "**bold**".into(),
            }),
            range: Some(range),
        };
        let flattened = flatten_hover(hover);
        assert_eq!(flattened.text, "**bold**");
        assert_eq!(flattened.range, Some(range));
    }
}

#[cfg(test)]
mod real_server;
