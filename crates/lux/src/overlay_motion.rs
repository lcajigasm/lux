//! Short motion for selector overlays. The clock is the same repaint wakeup
//! the caret blink already uses: a frame that is still moving asks the
//! platform to wake once, 16 ms later. Reduced motion settles immediately.

use std::cell::Cell;
use std::time::{Duration, Instant};

const OPEN: Duration = Duration::from_millis(120);
const SLIDE: Duration = Duration::from_millis(90);

thread_local! {
    static REDUCED: Cell<bool> = const { Cell::new(false) };
    static ANIMATING: Cell<bool> = const { Cell::new(false) };
}

pub(crate) fn begin_frame(reduced: bool) {
    REDUCED.with(|cell| cell.set(reduced));
    ANIMATING.with(|cell| cell.set(false));
}

pub(crate) fn reduced_motion() -> bool {
    REDUCED.with(|cell| cell.get())
}

pub(crate) fn note_animating() {
    ANIMATING.with(|cell| cell.set(true));
}

pub(crate) fn take_animating() -> bool {
    ANIMATING.with(|cell| cell.replace(false))
}

#[derive(Clone, Debug)]
pub(crate) struct OverlayMotion {
    opened_at: Instant,
    from: f32,
    target: f32,
    started: Instant,
    current: f32,
}

impl OverlayMotion {
    pub(crate) fn open() -> Self {
        let now = Instant::now();
        Self {
            opened_at: now,
            from: 0.0,
            target: 0.0,
            started: now,
            current: 0.0,
        }
    }

    /// Vertical offset that eases to zero. Negative `distance` starts above
    /// the resting position. Reduced motion returns zero.
    pub(crate) fn open_offset(&self, now: Instant, reduced: bool, distance: f32) -> f32 {
        if reduced {
            return 0.0;
        }
        (1.0 - self.open_fade(now, false)) * distance
    }

    /// Opacity of the opening card. Zero at the first sample, one at 120 ms.
    /// Reduced motion is already settled.
    pub(crate) fn open_fade(&self, now: Instant, reduced: bool) -> f32 {
        if reduced {
            return 1.0;
        }
        ease(now, self.opened_at, OPEN)
    }

    /// Eases the highlight toward `index` from wherever it was last painted.
    pub(crate) fn row(&mut self, index: usize, now: Instant, reduced: bool) -> f32 {
        let target = index as f32;
        if reduced {
            self.current = target;
            self.target = target;
            self.from = target;
            return target;
        }
        if (self.target - target).abs() > 0.001 {
            self.from = self.current;
            self.target = target;
            self.started = now;
        }
        let t = ease(now, self.started, SLIDE);
        self.current = self.from + (self.target - self.from) * t;
        self.current
    }

    pub(crate) fn animating(&self, now: Instant, reduced: bool) -> bool {
        if reduced {
            return false;
        }
        let opening = now.saturating_duration_since(self.opened_at) < OPEN;
        let sliding = now.saturating_duration_since(self.started) < SLIDE
            && (self.current - self.target).abs() > 0.01;
        opening || sliding
    }
}

pub(crate) fn note_if_animating(motion: &OverlayMotion, now: Instant) {
    if motion.animating(now, reduced_motion()) {
        note_animating();
    }
}

fn ease(now: Instant, started: Instant, window: Duration) -> f32 {
    let t = now.saturating_duration_since(started).as_secs_f32() / window.as_secs_f32();
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_offset_settles_and_reduced_motion_skips_it() {
        let motion = OverlayMotion::open();
        let now = motion.opened_at;
        assert!((motion.open_offset(now, false, -8.0) + 8.0).abs() < 0.01);
        assert_eq!(motion.open_offset(now, true, -8.0), 0.0);
        let later = now + OPEN;
        assert!(motion.open_offset(later, false, -8.0).abs() < 0.01);
        assert!(motion.animating(now, false));
        assert!(!motion.animating(later, false));
        assert!(!motion.animating(now, true));
    }

    #[test]
    fn the_open_animation_fades_and_slides_across_three_frames() {
        assert!(OPEN <= Duration::from_millis(120));
        let motion = OverlayMotion::open();
        let start = motion.opened_at;
        let middle = start + Duration::from_millis(60);
        let end = start + OPEN;
        let offset = |at| motion.open_offset(at, false, -8.0);
        let fade = |at| motion.open_fade(at, false);
        assert!((offset(start) + 8.0).abs() < 0.01);
        assert!(offset(middle) > offset(start) && offset(middle) < -0.2);
        assert!(offset(end).abs() < 0.01);
        assert!(fade(start) < 0.05);
        assert!(fade(middle) > fade(start) && fade(middle) < 0.98);
        assert!((fade(end) - 1.0).abs() < 0.01);
        assert!(motion.animating(start, false));
        assert!(motion.animating(middle, false));
        assert!(!motion.animating(end, false));
        assert_eq!(motion.open_fade(start, true), 1.0);
        assert_eq!(motion.open_offset(start, true, -8.0), 0.0);
    }

    #[test]
    fn the_highlight_eases_toward_a_new_row() {
        let mut motion = OverlayMotion::open();
        let now = motion.opened_at + Duration::from_millis(200);
        assert_eq!(motion.row(0, now, false), 0.0);
        let started = motion.row(2, now, false);
        assert!(
            started < 0.5,
            "the first sample of a move stays near the old row"
        );
        assert!(motion.animating(now, false));
        let done = motion.row(2, now + SLIDE, false);
        assert!((done - 2.0).abs() < 0.01);
        assert!(!motion.animating(now + SLIDE, false));
        assert_eq!(motion.row(4, now, true), 4.0);
    }
}
