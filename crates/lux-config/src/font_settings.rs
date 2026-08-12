//! Typography settings (Phase 5 deliverable 5): family, size, line height,
//! ligatures — resolved from a [`LayeredConfig`]'s merged settings, so
//! project- and folder-level overrides (already real, from the `layering`
//! loop's [`Layer::Project`]/[`Layer::Folder`]) apply to typography the
//! same way they apply to every other setting, with no separate override
//! mechanism invented just for fonts. The four keys below use the same
//! flat, dotted-key convention `import`'s `ImportedSetting::lux_key`
//! already established (`editor.fontFamily` and `editor.fontSize` are the
//! very keys `import::sublime`/`import::vscode` produce), so a value one
//! of those importers reported as imported lands directly at the key this
//! module reads back — no translation step in between.
//!
//! [`Layer::Project`]: crate::layering::Layer::Project
//! [`Layer::Folder`]: crate::layering::Layer::Folder

use crate::layering::LayeredConfig;

pub const FONT_FAMILY_KEY: &str = "editor.fontFamily";
pub const FONT_SIZE_KEY: &str = "editor.fontSize";
pub const LINE_HEIGHT_KEY: &str = "editor.lineHeight";
pub const FONT_LIGATURES_KEY: &str = "editor.fontLigatures";

/// Every field is `None` when no layer sets it — unconfigured typography
/// is a normal outcome, not an error, the same convention `editorconfig`
/// already uses.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FontSettings {
    pub family: Option<String>,
    /// Points, not pixels — matches `editor.fontSize`'s real meaning in
    /// both VS Code and Sublime. `f64`, not an integer: `line_height` is
    /// routinely a fractional multiplier (`1.5`) in real editor configs,
    /// and there's no reason to make `size` a stricter type than that for
    /// a settings surface that already stores everything as `f64` once it
    /// reaches JSON.
    pub size: Option<f64>,
    pub line_height: Option<f64>,
    pub ligatures: Option<bool>,
}

/// Resolves typography settings from `config`'s current merged view —
/// `Default` → `User` → `Project` → `Folder` → `Language`, the same
/// precedence every other setting in `config` already resolves through.
/// A key present but of the wrong JSON type (e.g. `editor.fontSize` set to
/// a string) resolves to `None` for that field rather than erroring —
/// consistent with how `editorconfig::resolve` treats an unparseable
/// property value, and for the same reason: one malformed typography
/// setting shouldn't make every other one unusable.
pub fn resolve(config: &LayeredConfig) -> FontSettings {
    let settings = config.resolved();
    FontSettings {
        family: settings
            .get(FONT_FAMILY_KEY)
            .and_then(|v| v.as_str())
            .map(str::to_string),
        size: settings.get(FONT_SIZE_KEY).and_then(|v| v.as_f64()),
        line_height: settings.get(LINE_HEIGHT_KEY).and_then(|v| v.as_f64()),
        ligatures: settings.get(FONT_LIGATURES_KEY).and_then(|v| v.as_bool()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layering::Layer;
    use std::fs;
    use std::path::PathBuf;
    use std::thread;

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-config-font-settings-test-{label}-{:?}-{}",
            thread::current().id(),
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &std::path::Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn resolves_all_four_properties_from_the_default_layer() {
        let config = LayeredConfig::new(
            r#"{
                "editor.fontFamily": "Menlo",
                "editor.fontSize": 13,
                "editor.lineHeight": 1.5,
                "editor.fontLigatures": true
            }"#,
        )
        .unwrap();

        let fonts = resolve(&config);
        assert_eq!(fonts.family.as_deref(), Some("Menlo"));
        assert_eq!(fonts.size, Some(13.0));
        assert_eq!(fonts.line_height, Some(1.5));
        assert_eq!(fonts.ligatures, Some(true));
    }

    #[test]
    fn unconfigured_properties_resolve_to_none() {
        let config = LayeredConfig::new(r#"{}"#).unwrap();
        assert_eq!(resolve(&config), FontSettings::default());
    }

    #[test]
    fn a_project_override_wins_over_the_default_family() {
        let dir = temp_dir("project-override");
        let mut config = LayeredConfig::new(r#"{"editor.fontFamily": "Menlo"}"#).unwrap();
        config
            .set_layer_path(
                Layer::Project,
                write(
                    &dir,
                    "project.jsonc",
                    r#"{"editor.fontFamily": "Fira Code"}"#,
                ),
            )
            .unwrap();

        assert_eq!(resolve(&config).family.as_deref(), Some("Fira Code"));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_folder_override_wins_over_a_project_override_for_the_same_property() {
        let dir = temp_dir("folder-over-project");
        let mut config = LayeredConfig::new(r#"{"editor.fontSize": 12}"#).unwrap();
        config
            .set_layer_path(
                Layer::Project,
                write(&dir, "project.jsonc", r#"{"editor.fontSize": 14}"#),
            )
            .unwrap();
        config
            .set_layer_path(
                Layer::Folder,
                write(&dir, "folder.jsonc", r#"{"editor.fontSize": 16}"#),
            )
            .unwrap();

        assert_eq!(resolve(&config).size, Some(16.0));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_folder_override_of_one_property_does_not_clobber_others() {
        let dir = temp_dir("partial-override");
        let mut config = LayeredConfig::new(
            r#"{"editor.fontFamily": "Menlo", "editor.fontSize": 12, "editor.fontLigatures": false}"#,
        )
        .unwrap();
        // The folder's own `.editorconfig`-adjacent settings file only
        // cares about this one project's preferred size — everything else
        // should still come from the default layer underneath it.
        config
            .set_layer_path(
                Layer::Folder,
                write(&dir, "folder.jsonc", r#"{"editor.fontSize": 20}"#),
            )
            .unwrap();

        let fonts = resolve(&config);
        assert_eq!(fonts.size, Some(20.0));
        assert_eq!(fonts.family.as_deref(), Some("Menlo"));
        assert_eq!(fonts.ligatures, Some(false));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clearing_the_folder_override_falls_back_to_the_project_value() {
        let dir = temp_dir("clear-fallback");
        let mut config = LayeredConfig::new(r#"{"editor.fontSize": 12}"#).unwrap();
        config
            .set_layer_path(
                Layer::Project,
                write(&dir, "project.jsonc", r#"{"editor.fontSize": 14}"#),
            )
            .unwrap();
        config
            .set_layer_path(
                Layer::Folder,
                write(&dir, "folder.jsonc", r#"{"editor.fontSize": 16}"#),
            )
            .unwrap();
        assert_eq!(resolve(&config).size, Some(16.0));

        config.clear_layer(Layer::Folder);
        assert_eq!(resolve(&config).size, Some(14.0));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_property_of_the_wrong_json_type_resolves_to_none_not_an_error() {
        let config = LayeredConfig::new(r#"{"editor.fontSize": "fourteen"}"#).unwrap();
        assert_eq!(resolve(&config).size, None);
    }
}
