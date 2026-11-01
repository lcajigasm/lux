//! App chrome: focus, locale, motion, theme, UI scale.

use crate::focus::FocusRing;
use crate::i18n::Locale;
use crate::motion::Motion;
use crate::theme::Theme;

/// OS light/dark appearance. The editor theme follows this only while the
/// user setting is `system` (see `theme_follows_system`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SystemScheme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone)]
pub struct Chrome {
    pub focus: FocusRing,
    pub locale: Locale,
    pub motion: Motion,
    pub theme: Theme,
    /// Last scheme the platform reported. Dark until one arrives.
    pub system_scheme: SystemScheme,
    /// When set, `theme` is Lux Dark or Lux Light for `system_scheme`.
    pub theme_follows_system: bool,
    pub ui_scale: f32,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            focus: FocusRing::new(),
            locale: Locale::En,
            motion: Motion::default(),
            theme: Theme::LUX_DARK,
            system_scheme: SystemScheme::Dark,
            theme_follows_system: true,
            ui_scale: 1.0,
        }
    }
}

impl Chrome {
    pub fn set_os_scale(&mut self, factor: f32) {
        self.ui_scale = factor.clamp(0.5, 4.0);
    }

    /// `system` and `auto` track the OS. A named theme does not.
    pub fn follows_system(setting: &str) -> bool {
        let normalized = setting
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "");
        matches!(normalized.as_str(), "system" | "auto")
    }

    pub fn theme_name_for_scheme(scheme: SystemScheme) -> &'static str {
        match scheme {
            SystemScheme::Dark => "lux-dark",
            SystemScheme::Light => "lux-light",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn os_scale_is_clamped() {
        let mut c = Chrome::default();
        c.set_os_scale(99.0);
        assert_eq!(c.ui_scale, 4.0);
        c.set_os_scale(0.1);
        assert_eq!(c.ui_scale, 0.5);
    }

    #[test]
    fn system_setting_follows_the_os_and_a_named_theme_does_not() {
        assert!(Chrome::follows_system("system"));
        assert!(Chrome::follows_system("Auto"));
        assert!(!Chrome::follows_system("lux-light"));
        assert_eq!(
            Chrome::theme_name_for_scheme(SystemScheme::Light),
            "lux-light"
        );
        assert_eq!(
            Chrome::theme_name_for_scheme(SystemScheme::Dark),
            "lux-dark"
        );
        let chrome = Chrome::default();
        assert!(chrome.theme_follows_system);
        assert_eq!(chrome.system_scheme, SystemScheme::Dark);
        assert_eq!(chrome.theme, Theme::LUX_DARK);
    }
}
