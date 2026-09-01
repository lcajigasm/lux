//! Scrollback navigation, regex search, and selection/copy over the VT
//! grid. `alacritty_terminal` already implements all three —
//! `Term::scroll_display` moves the viewport into the history
//! `Config::scrolling_history` already keeps, `term::search` runs real
//! DFA-compiled regex search across the whole buffer (viewport +
//! scrollback), and `Selection` + `Term::selection_to_string` extract
//! copyable text — this module is a thin, `Pty`-shaped surface over that
//! existing machinery, not a reimplementation of any of it.

use alacritty_terminal::grid::{Dimensions, Scroll as GridScroll};
use alacritty_terminal::index::Side;
use alacritty_terminal::selection::Selection;

pub use alacritty_terminal::index::{Column, Direction as SearchDirection, Line, Point};
pub use alacritty_terminal::selection::SelectionType;
pub use alacritty_terminal::term::search::{Match, RegexSearch};

use crate::pty::Pty;

/// How far to move the viewport into the terminal's scrollback.
#[derive(Debug, Clone, Copy)]
pub enum Scroll {
    /// Move by `n` lines: positive scrolls up into history, negative
    /// scrolls back down toward the live output.
    Lines(i32),
    PageUp,
    PageDown,
    /// Jump to the oldest scrollback line.
    Top,
    /// Jump back to the live, unscrolled viewport.
    Bottom,
}

/// How much scrollback exists, and how far the viewport is currently
/// scrolled into it (`display_offset == 0` means live/unscrolled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbackPosition {
    pub history_lines: usize,
    pub display_offset: usize,
}

impl Pty {
    /// Move the viewport into scrollback, or back toward live output.
    pub fn scroll(&self, scroll: Scroll) {
        let grid_scroll = match scroll {
            Scroll::Lines(n) => GridScroll::Delta(n),
            Scroll::PageUp => GridScroll::PageUp,
            Scroll::PageDown => GridScroll::PageDown,
            Scroll::Top => GridScroll::Top,
            Scroll::Bottom => GridScroll::Bottom,
        };
        self.term.lock().scroll_display(grid_scroll);
    }

    /// Current scrollback depth and viewport position within it.
    pub fn scrollback_position(&self) -> ScrollbackPosition {
        let term = self.term.lock();
        let grid = term.grid();
        ScrollbackPosition {
            history_lines: grid.total_lines() - grid.screen_lines(),
            display_offset: grid.display_offset(),
        }
    }

    /// Find the next match for `search` starting at `origin` and moving
    /// `direction`, across the whole buffer (scrollback included, not
    /// just the visible viewport), scrolling the match into view.
    pub fn search(
        &self,
        search: &mut RegexSearch,
        origin: Point,
        direction: SearchDirection,
    ) -> Option<Match> {
        let mut term = self.term.lock();
        let range = term.search_next(search, origin, direction, Side::Left, None)?;
        term.scroll_to_point(*range.start());
        Some(range)
    }

    /// Select the text from `start` to `end` (inclusive) and return it —
    /// the terminal's "select and copy." Replaces any prior selection.
    pub fn select(&self, ty: SelectionType, start: Point, end: Point) -> Option<String> {
        let mut term = self.term.lock();
        let mut selection = Selection::new(ty, start, Side::Left);
        selection.update(end, Side::Right);
        term.selection = Some(selection);
        term.selection_to_string()
    }

    /// Clear the current selection, if any.
    pub fn clear_selection(&self) {
        self.term.lock().selection = None;
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::{CommandBuilder, PtySize};

    fn small_size() -> PtySize {
        PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if condition() {
                return true;
            }
            if Instant::now() >= deadline {
                return condition();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn scroll_reveals_history_the_live_viewport_no_longer_shows() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args([
            "-c",
            "i=1; while [ $i -le 100 ]; do echo line-$i; i=$((i + 1)); done",
        ]);
        let pty = crate::Pty::spawn(cmd, small_size()).expect("spawn real shell");

        assert!(
            wait_until(Duration::from_secs(5), || pty
                .snapshot()
                .contains("line-100")),
            "shell did not finish printing:\n{}",
            pty.snapshot()
        );
        assert!(
            !pty.snapshot().contains("line-1\n"),
            "line-1 should already have scrolled out of the live viewport:\n{}",
            pty.snapshot()
        );

        let position = pty.scrollback_position();
        assert!(
            position.history_lines > 0,
            "expected real scrollback history to exist"
        );
        assert_eq!(position.display_offset, 0, "should start live, unscrolled");

        pty.scroll(Scroll::Top);
        assert!(
            pty.scrollback_position().display_offset > 0,
            "scrolling to Top should move the viewport into history"
        );
        assert!(
            pty.snapshot().contains("line-1"),
            "scrolled-back viewport should show early history:\n{}",
            pty.snapshot()
        );

        pty.scroll(Scroll::Bottom);
        assert_eq!(
            pty.scrollback_position().display_offset,
            0,
            "Bottom returns to live output"
        );
        assert!(pty.snapshot().contains("line-100"));
    }

    #[test]
    fn search_finds_a_match_that_has_scrolled_into_history() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args([
            "-c",
            "i=1; while [ $i -le 100 ]; do \
             if [ $i -eq 5 ]; then echo needle-marker; else echo line-$i; fi; \
             i=$((i + 1)); done",
        ]);
        let pty = crate::Pty::spawn(cmd, small_size()).expect("spawn real shell");

        assert!(wait_until(Duration::from_secs(5), || pty
            .snapshot()
            .contains("line-100")));
        assert!(
            !pty.snapshot().contains("needle-marker"),
            "the marker should already be out of the live viewport:\n{}",
            pty.snapshot()
        );

        let mut search = RegexSearch::new("needle-marker").expect("valid regex");
        let live_bottom = Point::new(Line(0), Column(0));
        let found = pty
            .search(&mut search, live_bottom, SearchDirection::Left)
            .expect("should find the marker in scrollback");

        let text = pty
            .select(SelectionType::Simple, *found.start(), *found.end())
            .expect("match range should be selectable");
        assert_eq!(text, "needle-marker");

        assert!(
            pty.scrollback_position().display_offset > 0,
            "search should have scrolled the match into view"
        );
    }

    #[test]
    fn select_extracts_exact_text_and_clear_selection_drops_it() {
        let mut cmd = CommandBuilder::new("/bin/sh");
        cmd.args(["-c", "printf 'hello world'"]);
        let pty = crate::Pty::spawn(cmd, small_size()).expect("spawn real shell");

        assert!(wait_until(Duration::from_secs(5), || pty
            .snapshot()
            .contains("hello world")));

        let start = Point::new(Line(0), Column(0));
        let end = Point::new(Line(0), Column(79));
        let text = pty
            .select(SelectionType::Simple, start, end)
            .expect("selection should have text");
        assert_eq!(text, "hello world");

        pty.clear_selection();
        assert!(pty.term.lock().selection.is_none());
    }
}
