//! Markdown inline highlighting and a structured preview of a parsed note.
//! The block grammar stays the one already vendored; inline emphasis, code
//! spans, and links come from `markdown_inline`.

use std::cell::RefCell;

use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

use crate::{Document, LanguageId, Token};

/// One run of inline text inside a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSpan {
    Text(String),
    Strong(String),
    Emphasis(String),
    Code(String),
    Link { label: String, destination: String },
}

/// One block in a Markdown preview. Built from the real parse tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewBlock {
    Heading { level: u8, text: String },
    Paragraph { spans: Vec<PreviewSpan> },
    Code { info: String, text: String },
    ListItem { text: String },
    Quote { text: String },
    Rule,
}

pub(crate) struct InlineHighlighter {
    parser: Parser,
    query: Query,
}

impl InlineHighlighter {
    pub(crate) fn new() -> Self {
        let mut parser = Parser::new();
        let language = inline_language();
        parser
            .set_language(&language)
            .expect("markdown inline grammar ABI must match tree-sitter");
        let query = Query::new(
            &language,
            include_str!("../queries/markdown_inline/highlights.scm"),
        )
        .expect("markdown inline highlights.scm must match its grammar");
        Self { parser, query }
    }

    pub(crate) fn tokens(&mut self, source: &str, base: usize) -> Vec<Token> {
        let Some(tree) = self.parser.parse(source, None) else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        let mut tokens = Vec::new();
        let mut captures = cursor.captures(&self.query, tree.root_node(), source.as_bytes());
        while let Some((mat, index)) = captures.next() {
            let capture = mat.captures[*index];
            tokens.push(Token {
                start_byte: base + capture.node.start_byte(),
                end_byte: base + capture.node.end_byte(),
                kind: self.query.capture_names()[capture.index as usize].to_string(),
            });
        }
        tokens
    }
}

pub(crate) fn inline_for(language: LanguageId) -> Option<RefCell<InlineHighlighter>> {
    if language == LanguageId::Markdown {
        Some(RefCell::new(InlineHighlighter::new()))
    } else {
        None
    }
}

/// Adds inline captures for every `inline` node. Block tokens stay; the
/// painter uses the first token that covers a byte, and inline captures
/// start at or after the block token.
pub(crate) fn append_inline_tokens(
    inline: &RefCell<InlineHighlighter>,
    root: Node,
    source: &str,
    tokens: &mut Vec<Token>,
) {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "inline" {
            let start = node.start_byte();
            let end = node.end_byte().min(source.len());
            if start < end {
                tokens.extend(inline.borrow_mut().tokens(&source[start..end], start));
            }
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
}

/// Structured preview of `doc`, which must already have been parsed.
#[must_use]
pub fn preview_document(doc: &Document, source: &str) -> Vec<PreviewBlock> {
    let Some(tree) = doc.tree() else {
        return Vec::new();
    };
    let mut blocks = Vec::new();
    collect(tree.root_node(), source, &mut blocks);
    blocks
}

fn collect(node: Node, source: &str, blocks: &mut Vec<PreviewBlock>) {
    match node.kind() {
        "atx_heading" => blocks.push(PreviewBlock::Heading {
            level: heading_level(node),
            text: inline_text(node, source),
        }),
        "setext_heading" => blocks.push(PreviewBlock::Heading {
            level: if node
                .child_by_field_name("underline")
                .is_some_and(|n| n.kind() == "setext_h2_underline")
                || child_kind(node, "setext_h2_underline")
            {
                2
            } else {
                1
            },
            text: inline_text(node, source),
        }),
        "fenced_code_block" | "indented_code_block" => blocks.push(PreviewBlock::Code {
            info: child_text(node, source, "info_string"),
            text: fence_body(node, source),
        }),
        "thematic_break" => blocks.push(PreviewBlock::Rule),
        "list_item" => blocks.push(PreviewBlock::ListItem {
            text: plain_text(node, source),
        }),
        "block_quote" => blocks.push(PreviewBlock::Quote {
            text: plain_text(node, source),
        }),
        "paragraph" => {
            if let Some(inline) = find_kind(node, "inline") {
                blocks.push(PreviewBlock::Paragraph {
                    spans: spans(inline, source),
                });
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect(child, source, blocks);
            }
        }
    }
}

fn spans(inline: Node, source: &str) -> Vec<PreviewSpan> {
    let mut highlighter = InlineHighlighter::new();
    let text = slice(source, inline);
    let Some(tree) = highlighter.parser.parse(&text, None) else {
        return vec![PreviewSpan::Text(text)];
    };
    let mut out = Vec::new();
    let mut cursor = tree.root_node().walk();
    let mut covered = 0usize;
    for child in tree.root_node().children(&mut cursor) {
        if child.start_byte() > covered {
            let gap = text[covered..child.start_byte()].to_string();
            if !gap.is_empty() {
                out.push(PreviewSpan::Text(gap));
            }
        }
        match child.kind() {
            "strong_emphasis" => out.push(PreviewSpan::Strong(inner_text(&text, child))),
            "emphasis" => out.push(PreviewSpan::Emphasis(inner_text(&text, child))),
            "code_span" => out.push(PreviewSpan::Code(inner_text(&text, child))),
            "inline_link" => out.push(PreviewSpan::Link {
                label: field_or_kind(&text, child, "link_text", "link_text"),
                destination: field_or_kind(&text, child, "link_destination", "link_destination"),
            }),
            _ => {
                let raw = text[child.start_byte()..child.end_byte()].to_string();
                if !raw.trim().is_empty() {
                    out.push(PreviewSpan::Text(raw));
                }
            }
        }
        covered = child.end_byte();
    }
    if covered < text.len() {
        let rest = text[covered..].to_string();
        if !rest.is_empty() {
            out.push(PreviewSpan::Text(rest));
        }
    }
    if out.is_empty() {
        out.push(PreviewSpan::Text(text));
    }
    out
}

fn inner_text(source: &str, node: Node) -> String {
    let mut cursor = node.walk();
    let mut parts = Vec::new();
    for child in node.children(&mut cursor) {
        if child.kind().ends_with("delimiter") || child.kind() == "code_span_delimiter" {
            continue;
        }
        if matches!(child.kind(), "link_text" | "link_destination") {
            continue;
        }
        if child.child_count() == 0 && !child.kind().contains("delimiter") {
            parts.push(source[child.start_byte()..child.end_byte()].to_string());
        } else if child.kind() != "emphasis_delimiter" {
            let nested = source[child.start_byte()..child.end_byte()].to_string();
            let trimmed = nested.trim_matches('*').trim_matches('_').trim_matches('`');
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }
    }
    if parts.is_empty() {
        source[node.start_byte()..node.end_byte()]
            .trim_matches('*')
            .trim_matches('_')
            .trim_matches('`')
            .to_string()
    } else {
        parts.concat()
    }
}

fn field_or_kind(source: &str, node: Node, field: &str, kind: &str) -> String {
    if let Some(child) = node.child_by_field_name(field) {
        return slice(source, child);
    }
    find_kind(node, kind)
        .map(|child| slice(source, child))
        .unwrap_or_default()
}

fn heading_level(node: Node) -> u8 {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "atx_h1_marker" => return 1,
            "atx_h2_marker" => return 2,
            "atx_h3_marker" => return 3,
            "atx_h4_marker" => return 4,
            "atx_h5_marker" => return 5,
            "atx_h6_marker" => return 6,
            _ => {}
        }
    }
    1
}

fn inline_text(node: Node, source: &str) -> String {
    find_kind(node, "inline")
        .or_else(|| find_kind(node, "paragraph"))
        .map(|child| slice(source, child).trim().to_string())
        .unwrap_or_else(|| plain_text(node, source))
}

fn plain_text(node: Node, source: &str) -> String {
    slice(source, node)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Fence text with the grammar's wrapping newline removed. `trim` would
/// also eat the first line's indent, and the preview would show it flush.
fn fence_body(node: Node, source: &str) -> String {
    let raw = find_kind(node, "code_fence_content")
        .map(|child| slice(source, child))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| slice(source, node));
    let mut text = raw.as_str();
    if let Some(rest) = text.strip_prefix("\r\n") {
        text = rest;
    } else if let Some(rest) = text.strip_prefix('\n') {
        text = rest;
    }
    if let Some(rest) = text.strip_suffix("\r\n") {
        text = rest;
    } else if let Some(rest) = text.strip_suffix('\n') {
        text = rest;
    }
    text.to_string()
}

fn child_text(node: Node, source: &str, kind: &str) -> String {
    find_kind(node, kind)
        .map(|child| slice(source, child).trim().to_string())
        .unwrap_or_default()
}

fn child_kind(node: Node, kind: &str) -> bool {
    find_kind(node, kind).is_some()
}

fn find_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn slice(source: &str, node: Node) -> String {
    let end = node.end_byte().min(source.len());
    let start = node.start_byte().min(end);
    source[start..end].to_string()
}

fn inline_language() -> tree_sitter::Language {
    unsafe extern "C" {
        fn tree_sitter_markdown_inline() -> *const ();
    }
    tree_sitter::Language::new(unsafe {
        tree_sitter_language::LanguageFn::from_raw(tree_sitter_markdown_inline)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fenced_block_keeps_indent_on_every_line() {
        let source = "```rust\n    fn main() {\n        let value = 1;\n    }\n```\n";
        let mut doc = Document::new(LanguageId::Markdown);
        doc.set_text(source);
        let blocks = preview_document(&doc, source);
        let text = blocks.iter().find_map(|block| match block {
            PreviewBlock::Code { text, .. } => Some(text.as_str()),
            _ => None,
        });
        assert_eq!(
            text,
            Some("    fn main() {\n        let value = 1;\n    }"),
            "fence body dropped indent: {text:?}"
        );
    }
}
