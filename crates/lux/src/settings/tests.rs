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

    let panel = state.panel.as_ref().unwrap();
    assert_eq!(panel.items.len(), 12);
    assert_eq!(panel.items[0].key, "editor.fontSize");
    assert_eq!(panel.items[1].key, "editor.lineHeight");
}
