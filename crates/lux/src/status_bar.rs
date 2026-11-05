//! Status bar state, telemetry formatting, and click handling for the desktop window.

use lux_core::buffer::{Buffer, Encoding, Eol, Indent, IndentStyle};
use lux_core::position::Position;
use lux_syntax::LanguageId;
use lux_ui::status_bar::{self, StatusBar};

pub(crate) const STATUS_BAR_HEIGHT: f32 = 24.0;
pub(crate) const STATUS_BAR_PADDING: f32 = 12.0;
pub(crate) const STATUS_BAR_ITEM_SPACING: f32 = 16.0;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct Rect {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl Rect {
    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct StatusBarLayout {
    pub(crate) bar_rect: Rect,
    pub(crate) cursor_rect: Rect,
    pub(crate) branch_rect: Rect,
    pub(crate) indent_rect: Rect,
    pub(crate) encoding_rect: Rect,
    pub(crate) eol_rect: Rect,
    pub(crate) language_rect: Rect,
}

#[derive(Debug, Default)]
pub(crate) struct StatusBarState {
    pub(crate) layout: StatusBarLayout,
}

pub(crate) fn language_name(lang: Option<LanguageId>) -> &'static str {
    lang.map(LanguageId::name).unwrap_or("Plain Text")
}

pub(crate) fn build_status_bar(
    buffer: &Buffer,
    cursor: Position,
    language: Option<LanguageId>,
    selected_chars: usize,
    branch: Option<&str>,
) -> StatusBar {
    status_bar::build(
        buffer,
        cursor,
        language_name(language),
        selected_chars,
        branch,
    )
}

pub(crate) fn selected_chars(selection: lux_core::selection::Selection) -> usize {
    if selection.is_empty() {
        0
    } else {
        let (start, end) = selection.ordered();
        end.char.saturating_sub(start.char)
    }
}

pub(crate) fn toggle_eol(buffer: &mut Buffer) -> Eol {
    let next = match buffer.eol() {
        Eol::Lf => Eol::Crlf,
        Eol::Crlf => Eol::Lf,
        Eol::Cr => Eol::Lf,
    };
    buffer.set_eol(next);
    next
}

pub(crate) fn toggle_indent(buffer: &mut Buffer) -> Indent {
    let current = buffer.indent();
    let next = match current.style {
        IndentStyle::Spaces if current.width == 4 => Indent {
            style: IndentStyle::Spaces,
            width: 2,
        },
        IndentStyle::Spaces => Indent {
            style: IndentStyle::Tabs,
            width: 1,
        },
        IndentStyle::Tabs => Indent {
            style: IndentStyle::Spaces,
            width: 4,
        },
    };
    buffer.set_indent(next);
    next
}

pub(crate) fn toggle_encoding(buffer: &mut Buffer) -> Encoding {
    let next = match buffer.encoding() {
        Encoding::Utf8 => Encoding::Latin1,
        _ => Encoding::Utf8,
    };
    buffer.set_encoding(next);
    next
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_name_maps_known_languages() {
        assert_eq!(language_name(Some(LanguageId::Rust)), "Rust");
        assert_eq!(language_name(Some(LanguageId::Python)), "Python");
        assert_eq!(language_name(None), "Plain Text");
    }

    #[test]
    fn rect_contains_tests_points() {
        let r = Rect {
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 30.0,
        };
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(35.0, 35.0));
        assert!(r.contains(60.0, 50.0));
        assert!(!r.contains(9.0, 20.0));
        assert!(!r.contains(61.0, 35.0));
        assert!(!r.contains(35.0, 19.0));
        assert!(!r.contains(35.0, 51.0));
    }

    #[test]
    fn toggle_eol_cycles_between_lf_and_crlf() {
        let mut buf = Buffer::from_bytes(b"hello\n").unwrap();
        assert_eq!(buf.eol(), Eol::Lf);
        assert_eq!(toggle_eol(&mut buf), Eol::Crlf);
        assert_eq!(buf.eol(), Eol::Crlf);
        assert_eq!(toggle_eol(&mut buf), Eol::Lf);
        assert_eq!(buf.eol(), Eol::Lf);
    }

    #[test]
    fn toggle_indent_cycles_4spaces_2spaces_tabs() {
        let mut buf = Buffer::from_bytes(b"    hello\n").unwrap();
        assert_eq!(
            buf.indent(),
            Indent {
                style: IndentStyle::Spaces,
                width: 4
            }
        );
        let n1 = toggle_indent(&mut buf);
        assert_eq!(
            n1,
            Indent {
                style: IndentStyle::Spaces,
                width: 2
            }
        );
        let n2 = toggle_indent(&mut buf);
        assert_eq!(
            n2,
            Indent {
                style: IndentStyle::Tabs,
                width: 1
            }
        );
        let n3 = toggle_indent(&mut buf);
        assert_eq!(
            n3,
            Indent {
                style: IndentStyle::Spaces,
                width: 4
            }
        );
    }

    #[test]
    fn toggle_encoding_switches_utf8_and_latin1() {
        let mut buf = Buffer::from_bytes(b"hello\n").unwrap();
        assert_eq!(buf.encoding(), Encoding::Utf8);
        assert_eq!(toggle_encoding(&mut buf), Encoding::Latin1);
        assert_eq!(buf.encoding(), Encoding::Latin1);
        assert_eq!(toggle_encoding(&mut buf), Encoding::Utf8);
        assert_eq!(buf.encoding(), Encoding::Utf8);
    }

    #[test]
    fn app_status_bar_clicks_dispatch_toggles() {
        let font_bytes = crate::font::load_system_monospace_font()
            .map(|(_, b)| b)
            .unwrap();
        let mut app = crate::app::App::new(font_bytes);
        app.ui.status_bar.layout = StatusBarLayout {
            bar_rect: Rect {
                x: 0.0,
                y: 676.0,
                width: 1000.0,
                height: 24.0,
            },
            cursor_rect: Rect {
                x: 12.0,
                y: 676.0,
                width: 80.0,
                height: 24.0,
            },
            branch_rect: Rect {
                x: 100.0,
                y: 676.0,
                width: 48.0,
                height: 24.0,
            },
            indent_rect: Rect {
                x: 700.0,
                y: 676.0,
                width: 60.0,
                height: 24.0,
            },
            encoding_rect: Rect {
                x: 780.0,
                y: 676.0,
                width: 50.0,
                height: 24.0,
            },
            eol_rect: Rect {
                x: 850.0,
                y: 676.0,
                width: 40.0,
                height: 24.0,
            },
            language_rect: Rect {
                x: 910.0,
                y: 676.0,
                width: 60.0,
                height: 24.0,
            },
        };

        // Click EOL
        assert_eq!(app.buffer.eol(), Eol::Lf);
        app.handle_status_bar_click(860.0, 680.0);
        assert_eq!(app.buffer.eol(), Eol::Crlf);

        // Click Indent
        assert_eq!(app.buffer.indent().style, IndentStyle::Spaces);
        assert_eq!(app.buffer.indent().width, 4);
        app.handle_status_bar_click(720.0, 680.0);
        assert_eq!(app.buffer.indent().width, 2);

        // Click Encoding
        assert_eq!(app.buffer.encoding(), Encoding::Utf8);
        app.handle_status_bar_click(790.0, 680.0);
        assert_eq!(app.buffer.encoding(), Encoding::Latin1);

        app.handle_status_bar_click(40.0, 680.0);
        assert!(app.ui.goto_anything.is_some());
        assert_eq!(app.ui.goto_anything.as_ref().unwrap().query.as_str(), ":");
        app.ui.git.branch_name = Some("main".into());
        app.handle_status_bar_click(110.0, 680.0);
        assert_eq!(app.documents.message.as_deref(), Some("Git branch main"));
        assert_eq!(
            app.chrome.focus.current(),
            lux_ui::focus::FocusTarget::Editor
        );
    }
}
