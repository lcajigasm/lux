//! File-name detection, line comments, and the Markdown preview chord.

use std::path::Path;

use lux_core::buffer::Buffer;
use lux_syntax::LanguageId;

use super::tests::{app, command};
use crate::document::detect_language;

#[test]
fn names_extensions_and_shebangs_pick_the_grammar() {
    let make = Buffer::from_bytes(b"all:\n\t@echo hi\n").unwrap();
    assert_eq!(
        detect_language(Some(Path::new("Makefile")), &make),
        Some(LanguageId::Make)
    );
    let docker = Buffer::from_bytes(b"FROM alpine\n").unwrap();
    assert_eq!(
        detect_language(Some(Path::new("Dockerfile")), &docker),
        Some(LanguageId::Dockerfile)
    );
    let php = Buffer::from_bytes(b"<?php echo 1;\n").unwrap();
    assert_eq!(
        detect_language(Some(Path::new("index.php")), &php),
        Some(LanguageId::Php)
    );
    let ruby = Buffer::from_bytes(b"#!/usr/bin/env ruby\nputs 1\n").unwrap();
    assert_eq!(detect_language(None, &ruby), Some(LanguageId::Ruby));
    let ignore = Buffer::from_bytes(b"target\n").unwrap();
    assert_eq!(
        detect_language(Some(Path::new(".gitignore")), &ignore),
        Some(LanguageId::Gitignore)
    );
}

#[test]
fn an_untitled_buffer_stays_plain_text() {
    let php = Buffer::from_bytes(b"#!/usr/bin/env php <?php echo str_repeat('x', 400);\n").unwrap();
    let tab = crate::document::Documents::default()
        .active_tab
        .expect("default tab");
    let untitled = crate::document::DocumentEntry::new(tab, None, php.clone());
    assert!(untitled.language.is_none());
    let saved =
        crate::document::DocumentEntry::new(tab, Some(std::path::PathBuf::from("index.php")), php);
    assert_eq!(saved.language, Some(LanguageId::Php));

    let mut app = app("#!/usr/bin/env php\n");
    assert!(app.documents.language().is_none());
    app.update_syntax_on_open();
    assert!(app.documents.language().is_none());
    app.set_language_mode(Some(LanguageId::Php));
    assert_eq!(app.documents.language(), Some(LanguageId::Php));
}

#[test]
fn new_languages_comment_with_their_line_prefix() {
    assert_eq!(LanguageId::Php.line_comment(), Some("//"));
    assert_eq!(LanguageId::Ruby.line_comment(), Some("#"));
    assert_eq!(LanguageId::Sql.line_comment(), Some("--"));
    assert_eq!(LanguageId::Lua.line_comment(), Some("--"));
    assert_eq!(LanguageId::CSharp.line_comment(), Some("//"));
    assert_eq!(LanguageId::Haskell.line_comment(), Some("--"));
    assert_eq!(LanguageId::Erlang.line_comment(), Some("%"));
    assert_eq!(LanguageId::Xml.line_comment(), None);

    let mut php = app("echo 1;\n");
    php.documents.entries[0].language = Some(LanguageId::Php);
    command(&mut php, "/", false);
    assert!(
        php.buffer.text().contains("//"),
        "cmd+/ did not comment PHP: {}",
        php.buffer.text()
    );
}

#[test]
fn shift_command_v_toggles_the_markdown_preview() {
    let mut note = app("# Title\n\n**bold** and [a](https://example.com)\n");
    note.documents.entries[0].language = Some(LanguageId::Markdown);
    command(&mut note, "v", true);
    assert!(note.documents.entries[0].markdown_preview);
    command(&mut note, "v", true);
    assert!(!note.documents.entries[0].markdown_preview);

    let source = "# Title\n\n- item\n\n```\ncode\n```\n\n[a](https://example.com)\n";
    let summary = crate::markdown_preview::summary(source);
    assert!(summary.contains("heading:Title"), "{summary}");
    assert!(summary.contains("list:item"), "{summary}");
    assert!(summary.contains("code:code"), "{summary}");
    assert!(summary.contains("link"), "{summary}");
    let edited = source.replacen("# Title", "# Title LIVE", 1);
    let again = crate::markdown_preview::summary(&edited);
    assert!(again.contains("heading:Title LIVE"), "{again}");

    let mut rust = app("fn main() {}\n");
    rust.documents.entries[0].language = Some(LanguageId::Rust);
    command(&mut rust, "v", true);
    assert!(!rust.documents.entries[0].markdown_preview);
    assert_eq!(
        rust.documents.message.as_deref(),
        Some("Markdown preview needs a Markdown file")
    );
}
