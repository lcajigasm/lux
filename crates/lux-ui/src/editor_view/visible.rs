//! Which lines a viewport shows and whether the caret blinks on.

use std::ops::Range;
use std::time::Duration;

use super::Viewport;

/// The first and last (exclusive) line indices intersecting `viewport`,
/// clamped to `total_lines` — the whole point of this deliverable: nothing
/// outside this range gets shaped.
pub(super) fn visible_range(
    viewport: &Viewport,
    line_height: f32,
    total_lines: usize,
) -> Range<usize> {
    if total_lines == 0 || viewport.height <= 0.0 {
        return 0..0;
    }
    let line_height = line_height.max(1.0);
    let first = ((viewport.scroll_y.max(0.0) / line_height).floor() as usize).min(total_lines - 1);
    // +1 so a line only partially scrolled into view at the bottom edge is
    // still shaped (better than a visible gap while scrolling).
    let visible_count = (viewport.height / line_height).ceil() as usize + 1;
    let last = first.saturating_add(visible_count).min(total_lines);
    first..last
}

/// Whether a blinking caret is in its "on" phase at `elapsed` — a pure
/// function of time, not a timer: the actual blink redraw is driven by a
/// future event/timer in the real render loop (golden rule #5, no
/// polling), not by this function, which only answers "on or off right
/// now" for a given instant.
pub(super) fn cursor_blink_visible(elapsed: Duration, interval: Duration) -> bool {
    if interval.is_zero() {
        return true;
    }
    let interval_ms = interval.as_millis().max(1);
    (elapsed.as_millis() / interval_ms).is_multiple_of(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_range_clamps_to_document_length() {
        let viewport = Viewport {
            width: 800.0,
            height: 40.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        assert_eq!(visible_range(&viewport, 20.0, 3), 0..3);
    }

    #[test]
    fn visible_range_scrolls_the_window_forward() {
        let viewport = Viewport {
            width: 800.0,
            height: 40.0,
            scroll_x: 0.0,
            scroll_y: 100.0,
        };
        // 100px / 20px line height = line 5 onward, 40px/20px + 1 = 3 lines.
        assert_eq!(visible_range(&viewport, 20.0, 1000), 5..8);
    }

    #[test]
    fn visible_range_of_an_empty_document_is_empty() {
        let viewport = Viewport {
            width: 800.0,
            height: 40.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        };
        assert_eq!(visible_range(&viewport, 20.0, 0), 0..0);
    }

    #[test]
    fn blink_alternates_every_interval() {
        let interval = Duration::from_millis(500);
        assert!(cursor_blink_visible(Duration::from_millis(0), interval));
        assert!(cursor_blink_visible(Duration::from_millis(499), interval));
        assert!(!cursor_blink_visible(Duration::from_millis(500), interval));
        assert!(!cursor_blink_visible(Duration::from_millis(999), interval));
        assert!(cursor_blink_visible(Duration::from_millis(1000), interval));
    }

    #[test]
    fn blink_is_always_visible_when_interval_is_zero() {
        assert!(cursor_blink_visible(
            Duration::from_millis(12345),
            Duration::ZERO
        ));
    }
}
