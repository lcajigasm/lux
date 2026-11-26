//! The settings page, the settings file saved from a tab, and the importers
//! never touch the disk on the UI thread: each call returns first and the
//! write or read lands when its worker reports back.

use lux_platform::event::{ElementState, KeyEvent};
use lux_platform::{Key, NamedKey, PlatformContext, ScrollDelta};

use super::tests::{Context, Dialogs};
use crate::app::App;
use crate::document::tests::app;
use crate::settings::{BUILT, SettingKind, SettingsState};

fn with_settings(dir: &tempfile::TempDir, ctx: &Context) -> (App, std::path::PathBuf) {
    let path = dir.path().join("settings.json");
    let mut app = app();
    app.ui.settings = SettingsState::new(path.clone());
    app.ui.dialogs.jobs.set_wake(ctx.background_waker());
    (app, path)
}

fn key(key: NamedKey) -> KeyEvent {
    KeyEvent {
        key: Key::Named(key),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
        physical: None,
    }
}

fn resolved(app: &App, key: &str) -> serde_json::Value {
    app.ui.settings.config.lock().unwrap().resolved()[key].clone()
}

#[test]
fn page_keys_clicks_and_wheel_never_rebuild_the_settings_state() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, _) = with_settings(&dir, &ctx);
    app.ui.settings.open_settings_ui();
    let built = BUILT.with(|built| built.get());

    for named in [NamedKey::ArrowDown, NamedKey::ArrowUp, NamedKey::PageDown] {
        assert!(crate::settings::handle_key(&mut app, &key(named)));
    }
    crate::settings::scroll_settings(&mut app, ScrollDelta::Lines { x: 0.0, y: -1.0 });
    crate::settings::handle_click(&mut app, 1.0, 1.0);
    let settings = app.ui.settings.settings_path.clone();
    crate::lsp::sync_document_change(&mut app, &settings, "{\"editor.fontSize\": 12}");
    app.ui.settings.open_settings_ui();
    app.ui.settings.open_settings_ui();

    assert_eq!(BUILT.with(|built| built.get()), built);
}

#[test]
fn a_page_edit_shows_at_once_and_writes_on_a_worker() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, path) = with_settings(&dir, &ctx);
    app.ui.settings.open_settings_ui();
    let ligatures = app
        .ui
        .settings
        .panel
        .as_ref()
        .unwrap()
        .items
        .iter()
        .position(|item| item.key == "editor.fontLigatures")
        .unwrap();
    app.ui.settings.panel.as_mut().unwrap().selected_index = ligatures;
    let before = std::fs::read_to_string(&path).unwrap();

    let hold = app.ui.dialogs.jobs.hold();
    crate::settings::handle_key(&mut app, &key(NamedKey::Enter));
    assert!(matches!(
        app.ui.settings.panel.as_ref().unwrap().items[ligatures].kind,
        SettingKind::Boolean { value: false }
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    assert_eq!(app.ui.dialogs.jobs.in_flight(), 1);

    hold.release();
    ctx.settle(&mut app, 1);
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        written.contains(r#""editor.fontLigatures": false"#),
        "{written}"
    );
    assert_eq!(resolved(&app, "editor.fontLigatures"), false);
    assert!(!app.editor_view.metrics().ligatures);
}

#[test]
fn saving_the_settings_tab_reloads_the_layer_on_a_worker() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, path) = with_settings(&dir, &ctx);
    std::fs::write(&path, r#"{"editor.wordWrap": true}"#).unwrap();

    let hold = app.ui.dialogs.jobs.hold();
    crate::lsp::sync_document_save(&mut app, &path);
    assert_eq!(resolved(&app, "editor.wordWrap"), false, "read inline");

    hold.release();
    ctx.settle(&mut app, 1);
    assert_eq!(resolved(&app, "editor.wordWrap"), true);
    assert!(app.editor_view.metrics().word_wrap);
}

#[test]
fn an_import_reads_and_writes_on_a_worker() {
    use crate::settings::importers::{Source, import_from};
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, path) = with_settings(&dir, &ctx);
    let source = dir.path().join("vscode.json");
    std::fs::write(
        &source,
        r#"{ // comment
            "editor.fontSize": 17, "editor.unknownThing": 1 }"#,
    )
    .unwrap();
    let before = std::fs::read_to_string(&path).unwrap();

    let hold = app.ui.dialogs.jobs.hold();
    import_from(&mut app, Some(&ctx), Source::VsCode, source);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    assert!(app.ui.announcement.is_none());

    hold.release();
    ctx.settle(&mut app, 1);
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains(r#""editor.fontSize": 17"#), "{written}");
    assert_eq!(resolved(&app, "editor.fontSize").as_f64(), Some(17.0));
    let report = app.ui.announcement.clone().unwrap();
    assert!(
        report.starts_with("Imported 1 settings from VS Code"),
        "{report}"
    );
}

#[test]
fn a_missing_import_source_writes_nothing() {
    use crate::settings::importers::{Source, import_from};
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, path) = with_settings(&dir, &ctx);
    let before = std::fs::read_to_string(&path).unwrap();

    import_from(
        &mut app,
        Some(&ctx),
        Source::Sublime,
        dir.path().join("missing.sublime-settings"),
    );
    ctx.settle(&mut app, 1);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
    let report = app.ui.announcement.clone().unwrap();
    assert!(
        report.starts_with("No Sublime Text settings imported"),
        "{report}"
    );
}

#[test]
fn a_broken_settings_file_is_never_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, path) = with_settings(&dir, &ctx);
    let broken = "{\n  \"editor.fontSize\": 15,\n  \"editor.tabWidth\": \n}";
    std::fs::write(&path, broken).unwrap();

    crate::settings::set_setting(&mut app, "editor.wordWrap", serde_json::json!(true));
    ctx.settle(&mut app, 1);

    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
    let message = app.documents.message.clone().unwrap();
    assert!(message.starts_with("Settings not saved:"), "{message}");
    assert!(message.contains("line"), "{message}");
}

#[test]
fn a_settings_write_keeps_comments() {
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, path) = with_settings(&dir, &ctx);
    std::fs::write(
        &path,
        "{\n  // my font\n  \"editor.fontSize\": 15,\n  \"editor.wordWrap\": false, // wrap\n}",
    )
    .unwrap();

    crate::settings::set_setting(&mut app, "editor.wordWrap", serde_json::json!(true));
    ctx.settle(&mut app, 1);

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(written.contains("// my font"), "{written}");
    assert!(written.contains("// wrap"), "{written}");
    assert!(written.contains(r#""editor.wordWrap": true"#), "{written}");
    assert_eq!(resolved(&app, "editor.fontSize").as_f64(), Some(15.0));
    assert_eq!(resolved(&app, "editor.wordWrap"), true);
    assert!(app.documents.message.is_none());
}

#[test]
fn an_import_into_a_broken_settings_file_reports_the_error() {
    use crate::settings::importers::{Source, import_from};
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, path) = with_settings(&dir, &ctx);
    std::fs::write(&path, "{ \"editor.fontSize\": ").unwrap();
    let source = dir.path().join("vscode.json");
    std::fs::write(&source, r#"{ "editor.fontSize": 17 }"#).unwrap();

    import_from(&mut app, Some(&ctx), Source::VsCode, source);
    ctx.settle(&mut app, 1);

    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "{ \"editor.fontSize\": "
    );
    let report = app.ui.announcement.clone().unwrap();
    assert!(
        report.starts_with("No VS Code settings imported:"),
        "{report}"
    );
}

#[cfg(unix)]
#[test]
fn a_failed_settings_write_is_reported() {
    use crate::settings::importers::{Source, import_from};
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let ctx = Context::new(Dialogs::default());
    let (mut app, path) = with_settings(&dir, &ctx);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).unwrap();
    if std::fs::OpenOptions::new().write(true).open(&path).is_ok() {
        return; // Running as a user that ignores file permissions.
    }
    let source = dir.path().join("vscode.json");
    std::fs::write(&source, r#"{ "editor.fontSize": 17 }"#).unwrap();

    import_from(&mut app, Some(&ctx), Source::VsCode, source);
    ctx.settle(&mut app, 1);
    let report = app.ui.announcement.clone().unwrap();
    assert!(report.contains("cannot write"), "{report}");

    crate::settings::set_setting(&mut app, "editor.wordWrap", serde_json::json!(true));
    ctx.settle(&mut app, 1);
    let message = app.documents.message.clone().unwrap();
    assert!(message.contains("cannot write"), "{message}");
}
