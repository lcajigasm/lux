//! Type and ink of the window chrome: sidebar, status bar, bottom panel
//! header. Chrome is set in the system UI face (`font::ui_font`), like the
//! selectors and the settings page; the editor face stays in the editor.
//!
//! Sizes are points, multiplied by the display scale. The text stack
//! shapes one weight per family, so the sidebar header is drawn at the
//! regular weight with real letter spacing: there is no semibold.

use lux_render::{GlyphAtlas, GlyphInstance};
use lux_ui::editor_view::EditorView;
use swash::FontRef;
use swash::scale::ScaleContext;

use super::{contrast_ratio, distinct_fill, readable_ink};
use crate::render::paint::{color, push_glyph};

/// Sidebar row labels.
pub(crate) const SIDEBAR_PT: f32 = 13.0;
/// Sidebar header (the folder name, upper case).
pub(crate) const HEADER_PT: f32 = 11.0;
/// Extra space after each header letter, in em.
pub(crate) const HEADER_TRACKING_EM: f32 = 0.06;
/// Status bar items.
pub(crate) const STATUS_PT: f32 = 12.0;
/// Bottom panel tab labels.
pub(crate) const PANEL_TAB_PT: f32 = 12.0;
/// Opacity of the sidebar disclosure triangle over the foreground.
pub(crate) const CHEVRON_ALPHA: f32 = 0.55;

/// Shapes and draws chrome labels in the UI face.
pub(crate) struct ChromePen<'a> {
    pub editor_view: &'a mut EditorView,
    pub atlas: &'a mut GlyphAtlas,
    pub atlas_dirty: &'a mut bool,
    pub scale_cx: &'a mut ScaleContext,
    pub font: FontRef<'static>,
    pub family: String,
    pub glyphs: &'a mut Vec<GlyphInstance>,
}

impl<'a> ChromePen<'a> {
    /// A pen on the UI face, or on the editor face while the UI face has
    /// not been loaded (never waits for it).
    pub(crate) fn new(
        editor_view: &'a mut EditorView,
        atlas: &'a mut GlyphAtlas,
        atlas_dirty: &'a mut bool,
        scale_cx: &'a mut ScaleContext,
        editor_font: FontRef<'static>,
        glyphs: &'a mut Vec<GlyphInstance>,
    ) -> Self {
        let (font, family) = crate::font::ui_font(editor_font, editor_view.family());
        Self {
            editor_view,
            atlas,
            atlas_dirty,
            scale_cx,
            font,
            family,
            glyphs,
        }
    }

    /// Advance width of `text` at `size`, with `tracking` after each
    /// cluster but the last.
    pub(crate) fn width(&mut self, size: f32, tracking: f32, text: &str) -> f32 {
        let style = lux_render::LineStyle {
            family: &self.family,
            font_size: size,
            ligatures: false,
        };
        let line = self
            .editor_view
            .text_layout_mut()
            .layout_line(&style, text, &[]);
        let clusters = line.clusters().len();
        line.width() + tracking * clusters.saturating_sub(1) as f32
    }

    /// Draws `text` with its em box centered on `center_y`, starting at
    /// `x`. Returns the drawn width.
    pub(crate) fn draw(
        &mut self,
        size: f32,
        text: &str,
        x: f32,
        center_y: f32,
        ink: [f32; 4],
    ) -> f32 {
        self.draw_tracked(size, 0.0, text, x, center_y, ink)
    }

    /// [`ChromePen::draw`] with `tracking` pixels added after each cluster.
    pub(crate) fn draw_tracked(
        &mut self,
        size: f32,
        tracking: f32,
        text: &str,
        x: f32,
        center_y: f32,
        ink: [f32; 4],
    ) -> f32 {
        let style = lux_render::LineStyle {
            family: &self.family,
            font_size: size,
            ligatures: false,
        };
        let line = self
            .editor_view
            .text_layout_mut()
            .layout_line(&style, text, &[]);
        let origin_y = center_y - size * 0.5 + line.baseline();
        let mut count = 0usize;
        for (index, cluster) in line.clusters().into_iter().enumerate() {
            push_glyph(
                self.atlas,
                self.atlas_dirty,
                self.scale_cx,
                self.font,
                size,
                &cluster,
                x + index as f32 * tracking,
                origin_y,
                ink,
                self.glyphs,
            );
            count = index + 1;
        }
        line.width() + tracking * count.saturating_sub(1) as f32
    }
}

/// Fill of the selected sidebar row: the theme selection painted on the
/// sidebar, pushed away from the sidebar when the two would blend.
pub(crate) fn sidebar_selection(theme: &lux_ui::Theme) -> [f32; 4] {
    let painted = lux_ui::theme::composite(theme.selection, theme.chrome.sidebar);
    distinct_fill(color(painted), color(theme.chrome.sidebar))
}

/// Row label ink (foreground or a git status color) on `background`.
pub(crate) fn sidebar_label(ink: [f32; 4], background: [f32; 4]) -> [f32; 4] {
    readable_ink(ink, background)
}

/// Muted header ink on the sidebar.
pub(crate) fn sidebar_header(theme: &lux_ui::Theme) -> [f32; 4] {
    readable_ink(color(theme.muted), color(theme.chrome.sidebar))
}

/// Disclosure triangle: the label ink at [`CHEVRON_ALPHA`] over `under`,
/// made more opaque only as far as a 3:1 graphic contrast needs (One Dark
/// is the bundled theme that needs it).
pub(crate) fn chevron_ink(label: [f32; 4], under: [f32; 4]) -> [f32; 4] {
    let mut alpha = CHEVRON_ALPHA;
    while alpha < 1.0 && contrast_ratio(over_rgb(label, alpha, under), under) < 3.0 {
        alpha = (alpha + 0.05).min(1.0);
    }
    [label[0], label[1], label[2], alpha]
}

fn over_rgb(ink: [f32; 4], alpha: f32, under: [f32; 4]) -> [f32; 4] {
    [
        ink[0] * alpha + under[0] * (1.0 - alpha),
        ink[1] * alpha + under[1] * (1.0 - alpha),
        ink[2] * alpha + under[2] * (1.0 - alpha),
        1.0,
    ]
}

/// Folder and document outlines: muted, at least 3:1 (a graphic, not text).
pub(crate) fn outline_ink(theme: &lux_ui::Theme, background: [f32; 4]) -> [f32; 4] {
    let muted = color(theme.muted);
    if contrast_ratio(muted, background) >= 3.0 {
        muted
    } else {
        readable_ink(muted, background)
    }
}

/// Status bar item ink.
pub(crate) fn status_ink(theme: &lux_ui::Theme) -> [f32; 4] {
    readable_ink(color(theme.foreground), color(theme.chrome.status_bar))
}

/// Opaque color of `ink` drawn over `under` (straight alpha).
#[cfg(test)]
pub(crate) fn over(ink: [f32; 4], under: [f32; 4]) -> [f32; 4] {
    over_rgb(ink, ink[3], under)
}

#[cfg(test)]
#[path = "chrome_text_tests.rs"]
mod tests;
