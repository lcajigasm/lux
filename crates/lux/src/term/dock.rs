//! Geometry inside the bottom dock, shared by its painter and its clicks.
//!
//! The dock rectangle itself comes from [`crate::frame_layout`]. Everything
//! here is relative to it, so the dock can move (sidebar shown or hidden,
//! find bar open) without the hit targets drifting from the paint.

/// Height of the tab strip at the top of the dock, in points.
pub(crate) const HEADER_HEIGHT: f32 = 26.0;

/// Top of the content area under the tab strip.
pub(crate) fn grid_top(dock: [f32; 4], scale: f32) -> f32 {
    dock[1] + HEADER_HEIGHT * scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_starts_under_the_tab_strip_at_both_scales() {
        for scale in [1.0, 2.0] {
            let dock = [220.0 * scale, 400.0 * scale, 780.0 * scale, 200.0 * scale];
            assert_eq!(grid_top(dock, scale), dock[1] + 26.0 * scale);
        }
    }
}
