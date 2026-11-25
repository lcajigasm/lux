//! A tab keeps its own syntax, diagnostics, and gutter marks.

use lux_platform::dialog::SaveDecision;
use lux_syntax::LanguageId;

use crate::document::Request;
use crate::document::tests::{Context, app};

fn open_settings(app: &mut crate::app::App, ctx: &Context) {
    crate::settings::open_settings_json(app, ctx);
    ctx.complete_io(app);
    ctx.complete(app);
}

#[test]
fn closing_settings_json_leaves_plain_text_without_its_view() {
    let dir = tempfile::tempdir().unwrap();
    let settings_path = dir.path().join("settings.json");
    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    app.insert_at_cursor("asdf");
    app.ui.settings = crate::settings::SettingsState::new(settings_path);
    open_settings(&mut app, &ctx);

    assert!(app.buffer.text().contains("editor.fontSize"));
    assert_eq!(app.documents.language(), Some(LanguageId::Json));
    assert!(
        app.syntax
            .as_ref()
            .is_some_and(|syntax| syntax.language == LanguageId::Json && !syntax.tokens.is_empty()),
        "settings JSON must be highlighted"
    );
    assert!(
        !app.ui.lsp.diagnostics.is_empty(),
        "settings JSON must report a diagnostic"
    );

    let settings_tab = app.documents.active_tab.expect("settings tab");
    app.close_tab(&ctx, settings_tab);
    ctx.complete(&mut app);

    assert_eq!(app.buffer.text(), "asdf");
    assert_eq!(app.documents.language(), None, "status language");
    assert!(
        app.syntax.is_none(),
        "plain text kept a highlighter for {:?}",
        app.syntax.as_ref().map(|syntax| syntax.language)
    );
    let lines: Vec<u32> = app
        .ui
        .lsp
        .diagnostics
        .iter()
        .map(|item| item.range.start.line)
        .collect();
    assert!(
        lines.is_empty(),
        "diagnostics leaked onto plain text: {lines:?}"
    );
    assert!(app.ui.lsp.completion_popup.is_none());
    assert!(app.ui.lsp.hover_popup.is_none());
    assert!(
        app.ui.git.markers.is_empty(),
        "gutter marks leaked: {:?}",
        app.ui.git.markers
    );
}

#[test]
fn switching_languages_keeps_each_buffers_view() {
    let dir = tempfile::tempdir().unwrap();
    let json = dir.path().join("data.json");
    let markdown = dir.path().join("note.md");
    let html = dir.path().join("page.html");
    let plain = dir.path().join("note.txt");
    std::fs::write(&json, "{\"a\": 1}\n").unwrap();
    std::fs::write(&markdown, "# Title\n\n- item\n").unwrap();
    std::fs::write(&html, "<h1>Hello</h1>\n").unwrap();
    std::fs::write(&plain, "asdf\n").unwrap();

    let ctx = Context::new(None, None, SaveDecision::Discard);
    let mut app = app();
    app.ui.settings = crate::settings::SettingsState::new(dir.path().join("settings.json"));
    app.insert_at_cursor("asdf");
    let plain_tab = app.documents.active_tab.expect("untitled");

    for path in [&json, &markdown, &html, &plain] {
        app.request_document(&ctx, Request::Open(Some(path.clone())));
        ctx.complete(&mut app);
    }
    open_settings(&mut app, &ctx);
    let settings_tab = app.documents.active_tab.expect("settings");

    let expect = [
        (settings_tab, Some(LanguageId::Json), true),
        (plain_tab, None, false),
    ];
    for (tab, language, diagnostics) in expect {
        assert!(app.switch_to_tab(tab));
        assert_eq!(app.documents.language(), language);
        assert_eq!(app.syntax.as_ref().map(|syntax| syntax.language), language);
        assert_eq!(!app.ui.lsp.diagnostics.is_empty(), diagnostics);
    }

    let tabs = app.documents.entries.clone();
    for entry in &tabs {
        let Some(path) = &entry.path else { continue };
        let language = entry.language;
        assert!(app.switch_to_tab(entry.tab_id));
        assert_eq!(app.documents.language(), language);
        assert_eq!(
            app.syntax.as_ref().map(|syntax| syntax.language),
            language,
            "highlighter followed {}",
            path.display()
        );
        let settings = app.ui.settings.is_settings_path(path);
        assert_eq!(
            !app.ui.lsp.diagnostics.is_empty(),
            settings,
            "diagnostics followed {}",
            path.display()
        );
        if language == Some(LanguageId::Markdown) || language == Some(LanguageId::Html) {
            crate::markdown_preview::toggle(&mut app);
            assert!(app.documents.active_entry().unwrap().markdown_preview);
        }
    }

    assert!(app.switch_to_tab(plain_tab));
    assert_eq!(app.documents.language(), None);
    assert!(app.syntax.is_none());
    assert!(app.ui.lsp.diagnostics.is_empty());
    assert!(
        app.documents
            .active_entry()
            .is_some_and(|entry| !entry.markdown_preview)
    );
}
