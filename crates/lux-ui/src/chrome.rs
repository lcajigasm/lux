//! App chrome: focus, locale, motion, theme, UI scale.

use crate::focus::FocusRing;
use crate::i18n::Locale;
use crate::motion::Motion;
use crate::theme::Theme;

#[derive(Debug, Clone)]
pub struct Chrome {
    pub focus: FocusRing,
    pub locale: Locale,
    pub motion: Motion,
    pub theme: Theme,
    pub ui_scale: f32,
}

impl Default for Chrome {
    fn default() -> Self {
        Self {
            focus: FocusRing::new(),
            locale: Locale::En,
            motion: Motion::default(),
            theme: Theme::LUX_DARK,
            ui_scale: 1.0,
        }
    }
}

impl Chrome {
    pub fn set_os_scale(&mut self, factor: f32) {
        self.ui_scale = factor.clamp(0.5, 4.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_scale_is_clamped() {
        let mut c = Chrome::default();
        c.set_os_scale(99.0);
        assert_eq!(c.ui_scale, 4.0);
        c.set_os_scale(0.1);
        assert_eq!(c.ui_scale, 0.5);
    }
}
