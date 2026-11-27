//! Geometry inside the bottom dock, shared by its painter and its clicks.
//!
//! The dock rectangle itself comes from [`crate::frame_layout`]. Everything
//! here is relative to it, so the dock can move (sidebar shown or hidden,
//! find bar open) without the hit targets drifting from the paint.

/// Height of the tab strip at the top of the dock, in points.
pub(crate) const HEADER_HEIGHT: f32 = 26.0;
/// Gap between the Cancel button and the dock's right edge, in points.
pub(crate) const EDGE_GAP: f32 = 8.0;
const ROW_FIRST: f32 = 40.0;
const ROW_STEP: f32 = 24.0;
const CANCEL_WIDTH: f32 = 60.0;
const CONTROL_HEIGHT: f32 = 18.0;

/// Top of the content area under the tab strip.
pub(crate) fn grid_top(dock: [f32; 4], scale: f32) -> f32 {
    dock[1] + HEADER_HEIGHT * scale
}

/// Top of the `index`th task row in the Tasks view.
pub(crate) fn task_row_top(dock: [f32; 4], index: usize, scale: f32) -> f32 {
    grid_top(dock, scale) + (ROW_FIRST + ROW_STEP * index as f32) * scale
}

/// The Cancel button of a task row: right-aligned, `EDGE_GAP` from the
/// dock's right edge, whatever the label's length.
pub(crate) fn cancel_rect(dock: [f32; 4], index: usize, scale: f32) -> [f32; 4] {
    let width = CANCEL_WIDTH * scale;
    [
        dock[0] + dock[2] - EDGE_GAP * scale - width,
        task_row_top(dock, index, scale) - 2.0 * scale,
        width,
        CONTROL_HEIGHT * scale,
    ]
}

/// The status pill sits `EDGE_GAP` after the label and never runs into
/// the Cancel button.
pub(crate) fn status_pill_rect(
    dock: [f32; 4],
    index: usize,
    label_right: f32,
    pill_width: f32,
    scale: f32,
) -> [f32; 4] {
    let cancel = cancel_rect(dock, index, scale);
    let x = (label_right + EDGE_GAP * scale).min(cancel[0] - EDGE_GAP * scale - pill_width);
    [x, cancel[1], pill_width, CONTROL_HEIGHT * scale]
}

/// Which task row's Cancel button holds `(x, y)`.
pub(crate) fn cancel_hit(dock: [f32; 4], rows: usize, scale: f32, x: f32, y: f32) -> Option<usize> {
    (0..rows).find(|&index| {
        let [rx, ry, rw, rh] = cancel_rect(dock, index, scale);
        x >= rx && x < rx + rw && y >= ry && y < ry + rh
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_is_right_aligned_with_an_eight_point_gap_at_both_scales() {
        for scale in [1.0, 2.0] {
            let dock = [220.0 * scale, 400.0 * scale, 780.0 * scale, 200.0 * scale];
            let cancel = cancel_rect(dock, 0, scale);
            assert_eq!(cancel[0] + cancel[2], dock[0] + dock[2] - 8.0 * scale);
            let pill = status_pill_rect(dock, 0, dock[0] + 180.0 * scale, 70.0 * scale, scale);
            assert_eq!(pill[0], dock[0] + 188.0 * scale, "pill follows the label");
            assert!(pill[0] + pill[2] <= cancel[0] - 8.0 * scale);
            assert_eq!(
                cancel_hit(dock, 1, scale, cancel[0] + 1.0, cancel[1] + 1.0),
                Some(0)
            );
            assert_eq!(
                cancel_hit(dock, 1, scale, pill[0] + 1.0, pill[1] + 1.0),
                None
            );
        }
    }

    #[test]
    fn a_long_label_does_not_push_the_pill_under_cancel() {
        let dock = [0.0, 0.0, 400.0, 200.0];
        let cancel = cancel_rect(dock, 0, 1.0);
        let pill = status_pill_rect(dock, 0, 390.0, 70.0, 1.0);
        assert!(pill[0] + pill[2] <= cancel[0] - 8.0);
    }
}
