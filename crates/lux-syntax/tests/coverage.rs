//! Every compiled-in grammar loads its highlight query, and Markdown preview
//! reads the real parse tree (headings, emphasis, links, fenced code).

use lux_syntax::{Document, LanguageId, PreviewBlock, PreviewSpan, preview_document};

#[test]
fn every_language_highlight_query_loads() {
    for language in LanguageId::ALL {
        let mut doc = Document::new(*language);
        doc.set_text("x\n");
        let _ = doc.highlight("x\n");
    }
}

#[test]
fn markdown_preview_reads_headings_emphasis_links_and_code() {
    let source = include_str!("fixtures/markdown/sample.md");
    let mut doc = Document::new(LanguageId::Markdown);
    doc.set_text(source);
    let tokens = doc.highlight(source);
    assert!(
        tokens.iter().any(|token| token.kind == "text.strong"),
        "bold is not highlighted: {tokens:?}"
    );
    assert!(
        tokens.iter().any(|token| token.kind == "text.uri"),
        "link destination is not highlighted: {tokens:?}"
    );

    let blocks = preview_document(&doc, source);
    assert!(
        blocks.iter().any(|block| matches!(
            block,
            PreviewBlock::Heading { level: 1, text } if text.contains("Sample")
        )),
        "heading missing: {blocks:?}"
    );
    assert!(
        blocks.iter().any(|block| matches!(
            block,
            PreviewBlock::Paragraph { spans }
                if spans.iter().any(|span| matches!(span, PreviewSpan::Strong(text) if text.contains("Markdown")))
                    && spans.iter().any(|span| matches!(span, PreviewSpan::Link { label, destination } if label.contains("link") && destination.contains("example.com")))
                    && spans.iter().any(|span| matches!(span, PreviewSpan::Code(text) if text.contains("inline code")))
        )),
        "paragraph spans missing: {blocks:?}"
    );
    assert!(
        blocks.iter().any(
            |block| matches!(block, PreviewBlock::Code { text, .. } if text.contains("fn main"))
        ),
        "code block missing: {blocks:?}"
    );
}
