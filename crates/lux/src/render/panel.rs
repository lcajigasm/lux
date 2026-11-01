//! Rounded selector surfaces. Radius 0 is not used here: these panels are
//! the command palette, goto, theme picker, and completion list.

use lux_render::QuadInstance;

pub(crate) fn push_rounded_surface(
    quads: &mut Vec<QuadInstance>,
    rect: [f32; 4],
    radius: f32,
    fill: [f32; 4],
    border: [f32; 4],
    shadow: f32,
    stroke: f32,
) {
    if shadow > 0.0 {
        quads.push(QuadInstance::rounded(
            [rect[0] + shadow * 0.4, rect[1] + shadow, rect[2], rect[3]],
            [0.0, 0.0, 0.0, 0.45],
            radius,
        ));
    }
    quads.push(QuadInstance::rounded(rect, border, radius));
    let inset = stroke.max(1.0);
    quads.push(QuadInstance::rounded(
        [
            rect[0] + inset,
            rect[1] + inset,
            (rect[2] - 2.0 * inset).max(1.0),
            (rect[3] - 2.0 * inset).max(1.0),
        ],
        fill,
        (radius - inset).max(0.0),
    ));
}

/// One moving selection row. `visual_index` is fractional while the
/// highlight is easing between rows.
#[allow(clippy::too_many_arguments)]
pub(crate) fn push_moving_row(
    quads: &mut Vec<QuadInstance>,
    left: f32,
    rows_top: f32,
    width: f32,
    row_height: f32,
    visual_index: f32,
    start_idx: usize,
    visible: usize,
    fill: [f32; 4],
    accent: [f32; 4],
    scale: f32,
) {
    if visible == 0 {
        return;
    }
    let y = rows_top + (visual_index - start_idx as f32) * row_height;
    let bottom = rows_top + visible as f32 * row_height;
    if y + row_height <= rows_top || y >= bottom {
        return;
    }
    let inset = 6.0 * scale;
    let radius = 4.0 * scale;
    quads.push(QuadInstance::rounded(
        [
            left + inset,
            y + scale,
            (width - 2.0 * inset).max(1.0),
            (row_height - 2.0 * scale).max(1.0),
        ],
        fill,
        radius,
    ));
    quads.push(QuadInstance::rounded(
        [
            left + inset,
            y + 4.0 * scale,
            3.0 * scale,
            (row_height - 8.0 * scale).max(1.0),
        ],
        accent,
        1.5 * scale,
    ));
}
