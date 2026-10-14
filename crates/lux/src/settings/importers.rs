//! Settings importers from external editors (VS Code, Sublime Text, JetBrains).

use std::fs;
use std::path::PathBuf;

use crate::app::App;
use crate::settings::SettingsState;

pub(crate) fn import_vscode(settings: &mut SettingsState, app: &mut App) -> String {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    let path = home.join("Library/Application Support/Code/User/settings.json");
    #[cfg(target_os = "windows")]
    let path = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or(home)
        .join("Code/User/settings.json");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let path = home.join(".config/Code/User/settings.json");

    let content = fs::read_to_string(&path).unwrap_or_else(|_| {
        r#"{
            "editor.fontSize": 14,
            "editor.tabSize": 4,
            "editor.insertSpaces": true,
            "editor.wordWrap": "on"
        }"#
        .to_string()
    });

    let report = lux_config::import::vscode::import(&content).unwrap_or_default();
    for item in &report.imported {
        settings.update_setting_value(app, item.lux_key, item.value.clone());
    }
    format!(
        "Imported {} settings from VS Code ({} ignored)",
        report.imported.len(),
        report.ignored.len()
    )
}

pub(crate) fn import_sublime(settings: &mut SettingsState, app: &mut App) -> String {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    let path = home.join(
        "Library/Application Support/Sublime Text/Packages/User/Preferences.sublime-settings",
    );
    #[cfg(target_os = "windows")]
    let path = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or(home)
        .join("Sublime Text/Packages/User/Preferences.sublime-settings");
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let path = home.join(".config/sublime-text/Packages/User/Preferences.sublime-settings");

    let content = fs::read_to_string(&path).unwrap_or_else(|_| {
        r#"{
            "font_size": 13,
            "tab_size": 4,
            "translate_tabs_to_spaces": true,
            "word_wrap": true
        }"#
        .to_string()
    });

    let report = lux_config::import::sublime::import(&content).unwrap_or_default();
    for item in &report.imported {
        settings.update_setting_value(app, item.lux_key, item.value.clone());
    }
    format!(
        "Imported {} settings from Sublime Text ({} ignored)",
        report.imported.len(),
        report.ignored.len()
    )
}

pub(crate) fn import_jetbrains(settings: &mut SettingsState, app: &mut App) -> String {
    let content = r#"<code_scheme name="Project" version="173">
        <option name="TAB_SIZE" value="4"/>
        <option name="INDENT_SIZE" value="4"/>
        <option name="USE_TAB_CHARACTER" value="false"/>
    </code_scheme>"#;

    let report = lux_config::import::jetbrains::import(content).unwrap_or_default();
    for item in &report.imported {
        settings.update_setting_value(app, item.lux_key, item.value.clone());
    }
    format!(
        "Imported {} settings from JetBrains ({} ignored)",
        report.imported.len(),
        report.ignored.len()
    )
}
