//! Error and warning colors for the build output in the dock.
//!
//! A line the build's `file_regex` matches (or, for a build system without
//! one, a line holding a `file:line` location, the same fallback F4 uses)
//! is painted in the error color, or the warning color when it mentions a
//! warning. Both colors are adjusted to stay readable on the theme's
//! background.

use lux_term::detect_links;

use super::paint::color;
use super::panel::readable_ink;
use crate::build::BuildState;

const ERROR: [f32; 4] = [0.93, 0.33, 0.31, 1.0];
const WARNING: [f32; 4] = [0.91, 0.67, 0.20, 1.0];

pub(crate) struct LineMarks<'a> {
    regex: Option<&'a regex::Regex>,
}

impl<'a> LineMarks<'a> {
    pub(crate) fn new(build: &'a BuildState) -> Self {
        Self {
            regex: build.output_regex.as_ref(),
        }
    }

    fn matches(&self, line: &str) -> bool {
        match self.regex {
            Some(regex) => regex.is_match(line),
            None => !detect_links(line).is_empty(),
        }
    }

    /// The ink for a whole output line, or `None` to keep its own colors.
    pub(crate) fn ink(&self, line: &str, theme: &lux_ui::Theme) -> Option<[f32; 4]> {
        if !self.matches(line) {
            return None;
        }
        let base = if line.to_ascii_lowercase().contains("warning") {
            WARNING
        } else {
            ERROR
        };
        Some(readable_ink(base, color(theme.background)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_regex_lines_get_readable_error_and_warning_inks() {
        let build = BuildState {
            output_regex: regex::Regex::new(r"^(\S+):(\d+):(\d+)").ok(),
            ..BuildState::default()
        };
        let marks = LineMarks::new(&build);
        for theme in [lux_ui::Theme::LUX_DARK, lux_ui::Theme::LUX_LIGHT] {
            let error = marks.ink("src/a.c:3:1: error: bad", &theme).unwrap();
            let warning = marks.ink("src/a.c:4:1: Warning: odd", &theme).unwrap();
            assert_ne!(error, warning);
            let bg = color(theme.background);
            for ink in [error, warning] {
                assert!(super::super::panel::contrast_ratio(ink, bg) >= 4.5);
            }
            assert_eq!(marks.ink("Compiling a v0.1.0", &theme), None);
            assert_eq!(
                marks.ink("  in src/a.c:3:1", &theme),
                None,
                "regex is anchored"
            );
        }
    }

    #[test]
    fn without_a_file_regex_location_lines_are_marked() {
        let build = BuildState::default();
        let marks = LineMarks::new(&build);
        let theme = lux_ui::Theme::LUX_DARK;
        assert!(marks.ink("  --> src/main.rs:2:5", &theme).is_some());
        assert_eq!(marks.ink("   Compiling demo v0.1.0", &theme), None);
    }
}
