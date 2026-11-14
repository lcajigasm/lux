//! Structured preview of a parsed HTML document.
//!
//! Headings, paragraphs, links, and preformatted text are enough for the
//! side pane. Script and style bodies stay out of the preview.

use tree_sitter::Node;

use crate::{Document, PreviewBlock, PreviewSpan};

/// Blocks for `doc`, which must already have been parsed as HTML.
#[must_use]
pub fn preview_html(doc: &Document, source: &str) -> Vec<PreviewBlock> {
    let Some(tree) = doc.tree() else {
        return Vec::new();
    };
    let mut blocks = Vec::new();
    collect(tree.root_node(), source, &mut blocks);
    blocks
}

fn collect(node: Node, source: &str, blocks: &mut Vec<PreviewBlock>) {
    if !is_element(node.kind()) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            collect(child, source, blocks);
        }
        return;
    }
    let tag = tag_name(node, source);
    if tag == "script" || tag == "style" {
        return;
    }
    match tag.as_str() {
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let text = visible_text(node, source);
            if !text.is_empty() {
                let level = tag.as_bytes()[1] - b'0';
                blocks.push(PreviewBlock::Heading { level, text });
            }
        }
        "p" | "a" => {
            let spans = inline_spans(node, source);
            if !spans.is_empty() {
                blocks.push(PreviewBlock::Paragraph { spans });
            }
        }
        "pre" | "code" => {
            let text = visible_text(node, source);
            if !text.is_empty() {
                blocks.push(PreviewBlock::Code {
                    info: String::new(),
                    text,
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

fn inline_spans(node: Node, source: &str) -> Vec<PreviewSpan> {
    let mut spans = Vec::new();
    push_spans(node, source, &mut spans);
    spans
}

fn push_spans(node: Node, source: &str, spans: &mut Vec<PreviewSpan>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "text" => {
                let text = slice(source, child);
                if !text.is_empty() {
                    spans.push(PreviewSpan::Text(text));
                }
            }
            "start_tag" | "end_tag" | "self_closing_tag" => {}
            kind if is_element(kind) && tag_name(child, source) == "a" => {
                let label = visible_text(child, source);
                if !label.is_empty() {
                    spans.push(PreviewSpan::Link {
                        label,
                        destination: attr(child, source, "href"),
                    });
                }
            }
            kind if is_element(kind) => push_spans(child, source, spans),
            _ => {}
        }
    }
}

fn is_element(kind: &str) -> bool {
    matches!(kind, "element" | "script_element" | "style_element")
}

fn tag_name(node: Node, source: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(child.kind(), "start_tag" | "self_closing_tag")
            && let Some(name) = find_kind(child, "tag_name")
        {
            return slice(source, name).to_ascii_lowercase();
        }
    }
    String::new()
}

fn attr(node: Node, source: &str, want: &str) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if !matches!(child.kind(), "start_tag" | "self_closing_tag") {
            continue;
        }
        let mut attrs = child.walk();
        for attr in child.children(&mut attrs) {
            if attr.kind() != "attribute" {
                continue;
            }
            let Some(name) = find_kind(attr, "attribute_name") else {
                continue;
            };
            if slice(source, name) != want {
                continue;
            }
            let Some(value) = find_kind(attr, "attribute_value") else {
                continue;
            };
            return unquote(&slice(source, value));
        }
    }
    String::new()
}

fn visible_text(node: Node, source: &str) -> String {
    let mut out = String::new();
    push_text(node, source, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_text(node: Node, source: &str, out: &mut String) {
    if node.kind() == "text" {
        out.push_str(&slice(source, node));
        return;
    }
    if matches!(node.kind(), "start_tag" | "end_tag" | "self_closing_tag") {
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        push_text(child, source, out);
    }
}

fn find_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return Some(child);
        }
        if let Some(found) = find_kind(child, kind) {
            return Some(found);
        }
    }
    None
}

fn slice(source: &str, node: Node) -> String {
    source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or("")
        .to_string()
}

fn unquote(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LanguageId;

    #[test]
    fn a_simple_page_yields_a_heading_a_paragraph_and_a_link() {
        let source = "<h1>Hello</h1>\n<p>A paragraph with a <a href=\"https://example.com\">link</a>.</p>\n<pre>code</pre>\n";
        let mut doc = Document::new(LanguageId::Html);
        doc.set_text(source);
        let blocks = preview_html(&doc, source);
        let heading = blocks.iter().find_map(|block| match block {
            PreviewBlock::Heading { text, level } => Some((text.as_str(), *level)),
            _ => None,
        });
        assert_eq!(heading, Some(("Hello", 1)), "{blocks:?}\n{}", dump(source));
        let paragraph = blocks.iter().find_map(|block| match block {
            PreviewBlock::Paragraph { spans } => Some(spans),
            _ => None,
        });
        let Some(spans) = paragraph else {
            panic!("no paragraph\n{}", dump(source));
        };
        assert!(
            spans.iter().any(
                |span| matches!(span, PreviewSpan::Link { label, destination }
                    if label == "link" && destination == "https://example.com")
            ),
            "{spans:?}\n{}",
            dump(source)
        );
        assert!(
            blocks
                .iter()
                .any(|block| matches!(block, PreviewBlock::Code { text, .. } if text == "code")),
            "{blocks:?}\n{}",
            dump(source)
        );
    }

    fn dump(source: &str) -> String {
        let mut doc = Document::new(LanguageId::Html);
        doc.set_text(source);
        let Some(tree) = doc.tree() else {
            return "no tree".to_string();
        };
        let mut out = String::new();
        walk(tree.root_node(), source, &mut out, 0);
        out
    }

    fn walk(node: Node, source: &str, out: &mut String, depth: usize) {
        let snippet: String = source
            .get(node.start_byte()..node.end_byte())
            .unwrap_or("")
            .chars()
            .take(24)
            .collect();
        out.push_str(&" ".repeat(depth));
        out.push_str(node.kind());
        out.push_str(" [");
        out.push_str(&snippet);
        out.push_str("]\n");
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            walk(child, source, out, depth + 2);
        }
    }
}
