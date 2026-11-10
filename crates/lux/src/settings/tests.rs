//! Integration tests for settings configuration, hot reload, validation,
//! completion, and importers (Goal G22).

use super::*;

#[test]
fn settings_lifecycle_and_validation() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.json");

    let state = SettingsState::new(path.clone());
    assert!(path.exists());
    assert!(state.is_settings_path(&path));

    let valid_json = r#"{"editor.fontSize": 15.0}"#;
    let diags = lux_config::validate(valid_json);
    assert!(diags.is_empty());

    let invalid_json = r#"{"editor.fontSize": "not_a_number"}"#;
    let diags = lux_config::validate(invalid_json);
    assert!(!diags.is_empty());
}

#[test]
fn settings_completion_produces_candidates() {
    let text = r#"{"editor."#;
    let items = lux_config::completion::complete(text, 9);
    assert!(!items.is_empty());
    assert!(items.iter().any(|i| i.label.starts_with("editor.")));
}

#[test]
fn settings_importers_and_panel_options() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.json");

    let mut state = SettingsState::new(path);
    state.open_settings_ui();
    assert!(state.panel.is_some());

    {
        let panel = state.panel.as_ref().unwrap();
        assert_eq!(panel.items.len(), 14);
        assert_eq!(panel.items[0].key, "editor.fontSize");
        assert_eq!(panel.items[1].key, "editor.lineHeight");
        assert_eq!(
            panel.categories(),
            vec!["All", "Editor", "Appearance", "Keys", "Files", "Git", "CLI"]
        );
        assert!(
            panel
                .items
                .iter()
                .any(|item| { item.category == "Git" && matches!(item.kind, SettingKind::Info) })
        );
        assert!(panel.items.iter().any(|item| item.category == "CLI"));
    }
    let panel = state.panel.as_mut().unwrap();
    panel.query = "history".into();
    let visible = panel.visible_indices();
    assert_eq!(visible.len(), 1);
    assert_eq!(panel.items[visible[0]].category, "Git");
}

#[test]
fn theme_reload_keeps_the_buffer_and_scales_line_height() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{ "editor.fontSize": 13.0, "editor.lineHeight": 1.4, "workbench.colorTheme": "monokai" }"#,
    )
    .unwrap();
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.ui.settings = SettingsState::new(path);
    app.set_display_scale(2.0);
    app.buffer =
        lux_core::buffer::Buffer::from_bytes(b"fn main() {\n    let value = 1;\n}\n").unwrap();
    app.documents.path = Some(std::path::PathBuf::from("src/main.rs"));
    app.documents
        .set_language(Some(lux_syntax::LanguageId::Rust));
    app.documents.saved_version = app.history.state_id();
    let text = app.buffer.text();
    app.confirm_theme("One Dark");
    assert_eq!(app.buffer.text(), text);
    assert_eq!(
        app.documents.path.as_deref(),
        Some(std::path::Path::new("src/main.rs"))
    );
    assert_eq!(app.documents.language(), Some(lux_syntax::LanguageId::Rust));
    assert!(!app.document_dirty());
    let metrics = app.editor_view.metrics();
    assert!(
        (metrics.font_size - 26.0).abs() < 0.1,
        "{}",
        metrics.font_size
    );
    assert!(
        (metrics.line_height - 13.0 * 1.4 * 2.0).abs() < 0.1,
        "{}",
        metrics.line_height
    );
    assert!(metrics.line_height > metrics.font_size);
    assert_eq!(app.chrome.theme.name, "One Dark");
    app.cursors = lux_core::multicursor::MultiCursor::new(lux_core::selection::Selection::cursor(
        lux_core::position::Position::from_char(&app.buffer, 8),
    ));
    app.viewport.scroll_y = 120.0;
    app.viewport.scroll_x = 16.0;
    for name in crate::color_scheme::theme_setting_options() {
        app.confirm_theme(name);
        assert_eq!(app.buffer.text(), text, "{name}");
        assert!(!app.document_dirty(), "{name}");
        assert_eq!(
            app.documents.language(),
            Some(lux_syntax::LanguageId::Rust),
            "{name}"
        );
        let metrics = app.editor_view.metrics();
        assert!(
            (metrics.font_size - 26.0).abs() < 0.1,
            "{name} font {}",
            metrics.font_size
        );
        assert!(
            metrics.line_height > metrics.font_size,
            "{name} line {}",
            metrics.line_height
        );
        assert_eq!(app.cursors.primary().head.char, 8, "{name}");
        assert!((app.viewport.scroll_y - 120.0).abs() < 0.01, "{name}");
        assert!((app.viewport.scroll_x - 16.0).abs() < 0.01, "{name}");
    }
    app.confirm_theme("Solarized Light");
    assert_eq!(app.chrome.theme.name, "Solarized Light");
    assert_eq!(app.buffer.text(), text);
    assert_eq!(app.cursors.primary().head.char, 8);
    let path = app.ui.settings.settings_path.clone();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("Solarized Light"), "{saved}");
    let mut next = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let settings = SettingsState::new(path);
    settings.apply_to_app(&mut next);
    assert_eq!(next.chrome.theme.name, "Solarized Light");
}

#[test]
fn word_wrap_defaults_off_and_the_toggle_persists() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("settings.json");
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    assert!(!app.editor_view.metrics().word_wrap);
    app.ui.settings = SettingsState::new(path.clone());
    app.toggle_word_wrap();
    assert!(app.editor_view.metrics().word_wrap);
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains("\"editor.wordWrap\": true"), "{saved}");
    let mut next = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let settings = SettingsState::new(path);
    settings.apply_to_app(&mut next);
    assert!(next.editor_view.metrics().word_wrap);
}

#[test]
fn the_default_line_height_is_a_multiplier_not_a_pixel_height() {
    let (size, line) = physical_text_metrics(13.0, 1.4, 1.0);
    assert_eq!(size, 13.0);
    assert!((line - 18.2).abs() < 0.01, "{line}");
    let (size, line) = physical_text_metrics(13.0, 1.4, 2.0);
    assert_eq!(size, 26.0);
    assert!((line - 36.4).abs() < 0.01, "{line}");
    let (_size, line) = physical_text_metrics(16.0, 22.0, 1.0);
    assert_eq!(line, 22.0);
}
