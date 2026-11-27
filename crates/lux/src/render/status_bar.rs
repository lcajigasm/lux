//! Rendering logic for the interactive status bar at the bottom of the desktop window.

use swash::FontRef;
use swash::scale::ScaleContext;

use super::panel::chrome_text::{self, ChromePen};
use crate::render::paint::color;
use crate::status_bar::{
    Rect, STATUS_BAR_HEIGHT, STATUS_BAR_ITEM_SPACING, STATUS_BAR_PADDING, StatusBarLayout,
    build_status_bar,
};
use lux_render::{GlyphAtlas, GlyphInstance, QuadInstance};
use lux_ui::editor_view::EditorView;

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_status_bar(
    window_width: f32,
    window_height: f32,
    scale_factor: f32,
    stroke: f32,
    font: FontRef<'static>,
    editor_view: &mut EditorView,
    atlas: &mut GlyphAtlas,
    atlas_dirty: &mut bool,
    scale_cx: &mut ScaleContext,
    theme: &lux_ui::Theme,
    buffer: &lux_core::buffer::Buffer,
    cursor: lux_core::selection::Selection,
    language_id: Option<lux_syntax::LanguageId>,
    is_recording_macro: bool,
    branch: Option<&str>,
    quad_instances: &mut Vec<QuadInstance>,
    glyph_instances: &mut Vec<GlyphInstance>,
) -> StatusBarLayout {
    let bar_height = STATUS_BAR_HEIGHT * scale_factor;
    let bar_y = window_height - bar_height;
    let mid = bar_y + bar_height * 0.5;
    let size = chrome_text::STATUS_PT * scale_factor;
    let mut pen = ChromePen::new(
        editor_view,
        atlas,
        atlas_dirty,
        scale_cx,
        font,
        glyph_instances,
    );

    quad_instances.push(QuadInstance::solid(
        [0.0, bar_y, window_width, bar_height],
        color(theme.chrome.status_bar),
    ));
    quad_instances.push(QuadInstance::solid(
        [0.0, bar_y, window_width, stroke],
        color(theme.chrome.border),
    ));

    let status = build_status_bar(
        buffer,
        cursor.head,
        language_id,
        crate::status_bar::selected_chars(cursor),
        branch,
    );
    let ink = chrome_text::status_ink(theme);
    let spacing = STATUS_BAR_ITEM_SPACING * scale_factor;
    let hit = |x: f32, w: f32| Rect {
        x: x - 4.0 * stroke,
        y: bar_y,
        width: w + 8.0 * stroke,
        height: bar_height,
    };
    let mut layout = StatusBarLayout {
        bar_rect: Rect {
            x: 0.0,
            y: bar_y,
            width: window_width,
            height: bar_height,
        },
        ..Default::default()
    };

    // Left: cursor position, branch, macro recording.
    let mut left = STATUS_BAR_PADDING * scale_factor;
    let width = pen.draw(size, &status.cursor.label, left, mid, ink);
    layout.cursor_rect = hit(left, width);
    left += width + spacing;
    if let Some(branch_item) = &status.branch {
        let width = pen.draw(size, &branch_item.label, left, mid, ink);
        layout.branch_rect = hit(left, width);
        left += width + spacing;
    }
    if is_recording_macro {
        let recording =
            chrome_text::sidebar_label([0.95, 0.28, 0.28, 1.0], color(theme.chrome.status_bar));
        pen.draw(size, "● Recording Macro", left, mid, recording);
    }

    // Right, from the edge inwards: language, line endings, encoding, indent.
    let mut right = window_width - STATUS_BAR_PADDING * scale_factor;
    let mut place = |label: &str| {
        let width = pen.width(size, 0.0, label);
        let x = right - width;
        pen.draw(size, label, x, mid, ink);
        right = x - spacing;
        hit(x, width)
    };
    layout.language_rect = place(&status.language.label);
    layout.eol_rect = place(&status.eol.label);
    layout.encoding_rect = place(&status.encoding.label);
    layout.indent_rect = place(&status.indent.label);
    layout
}
