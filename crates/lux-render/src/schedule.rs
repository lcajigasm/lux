//! Event-driven repaint scheduling and dirty-region tracking — Phase 2
//! deliverable 8: "Loop de render con presupuesto: sin repintado continuo
//! cuando no hay cambios; repintado dirigido por eventos e invalidación
//! por regiones."
//!
//! [`RepaintScheduler`] is the policy a render loop consults instead of
//! painting every frame unconditionally: something (an edit, a cursor
//! move, a resize, a blink timer) must explicitly invalidate a region or
//! request a timed wakeup before [`RepaintScheduler::needs_repaint`] ever
//! returns `true`. There is no `if now - last_paint > interval` anywhere
//! here — that would be exactly the polling golden rule #5 forbids; the
//! timer support ([`RepaintScheduler::request_repaint_at`]) only *records*
//! when a repaint would be due, driven by whatever real timer/event-loop
//! wakeup the caller already has (`lux-platform`'s event loop, once this is
//! wired into one — that wiring is a later loop, this crate doesn't depend
//! on `lux-platform` at all, see `docs/ARCHITECTURE.md`).

use std::time::Instant;

/// A rectangular region of the render target that needs repainting, in
/// physical pixels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DirtyRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl DirtyRect {
    /// The smallest rectangle containing both `self` and `other` — used to
    /// coalesce many small regions into one when there are too many to
    /// track individually (see [`RepaintScheduler::MAX_REGIONS`]).
    pub fn union(&self, other: &DirtyRect) -> DirtyRect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.width).max(other.x + other.width);
        let y1 = (self.y + self.height).max(other.y + other.height);
        DirtyRect {
            x: x0,
            y: y0,
            width: x1 - x0,
            height: y1 - y0,
        }
    }

    /// Whether `(x, y)` falls within this rectangle — mainly useful for
    /// asserting a coalesced union still covers a point a smaller,
    /// now-merged-away rectangle used to cover.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// Tracks whether the next frame needs to be painted at all, and if so,
/// which regions — never both empty and "needs paint," never continuously
/// true absent some real invalidation.
#[derive(Default)]
pub struct RepaintScheduler {
    dirty: Vec<DirtyRect>,
    full_repaint: bool,
    next_timer: Option<Instant>,
}

impl RepaintScheduler {
    /// Above this many pending small regions, tracking them individually
    /// stops paying for itself; they're coalesced into their union instead.
    const MAX_REGIONS: usize = 16;

    pub fn new() -> Self {
        Self::default()
    }

    /// Marks `rect` as needing repaint (an edit, a cursor move, a
    /// selection change...).
    pub fn invalidate(&mut self, rect: DirtyRect) {
        if self.full_repaint {
            return;
        }
        self.dirty.push(rect);
        if self.dirty.len() > Self::MAX_REGIONS {
            let merged = self
                .dirty
                .drain(..)
                .reduce(|a, b| a.union(&b))
                .expect("just pushed at least one region above");
            self.dirty.push(merged);
        }
    }

    /// Marks the whole viewport dirty (resize, theme change) — cheaper
    /// than tracking a region that happens to cover everything.
    pub fn invalidate_all(&mut self) {
        self.dirty.clear();
        self.full_repaint = true;
    }

    /// Schedules a repaint at (or after) `at` even if nothing else
    /// changes — e.g. the next cursor-blink phase transition. Still
    /// event-driven, not polling: the caller's own real timer is what
    /// wakes the event loop and asks `needs_repaint` again; this only
    /// records the earliest instant that answer should flip to `true`.
    pub fn request_repaint_at(&mut self, at: Instant) {
        self.next_timer = Some(match self.next_timer {
            Some(existing) => existing.min(at),
            None => at,
        });
    }

    /// Whether a repaint is due at `now` — because something invalidated a
    /// region or the whole frame, or because a previously requested timer
    /// has elapsed.
    pub fn needs_repaint(&self, now: Instant) -> bool {
        self.full_repaint || !self.dirty.is_empty() || self.next_timer.is_some_and(|t| now >= t)
    }

    /// Takes the pending regions and clears all pending state — call
    /// exactly once per frame actually painted, never speculatively, or a
    /// real invalidation could be lost. `None` means "repaint everything"
    /// (explicitly requested, or too many small regions accumulated to
    /// track — see [`Self::MAX_REGIONS`]); `Some(regions)` (possibly
    /// empty, if only a timer fired) means only those regions changed.
    pub fn take_dirty_regions(&mut self) -> Option<Vec<DirtyRect>> {
        let full = std::mem::take(&mut self.full_repaint);
        self.next_timer = None;
        if full {
            None
        } else {
            Some(std::mem::take(&mut self.dirty))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> DirtyRect {
        DirtyRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn a_fresh_scheduler_needs_no_repaint() {
        let scheduler = RepaintScheduler::new();
        assert!(!scheduler.needs_repaint(Instant::now()));
    }

    #[test]
    fn invalidating_a_region_makes_a_repaint_due() {
        let mut scheduler = RepaintScheduler::new();
        scheduler.invalidate(rect(0.0, 0.0, 10.0, 10.0));
        assert!(scheduler.needs_repaint(Instant::now()));
    }

    #[test]
    fn invalidate_all_reports_as_a_full_repaint_with_no_region_list() {
        let mut scheduler = RepaintScheduler::new();
        scheduler.invalidate(rect(0.0, 0.0, 10.0, 10.0));
        scheduler.invalidate_all();
        assert!(scheduler.needs_repaint(Instant::now()));
        assert_eq!(
            scheduler.take_dirty_regions(),
            None,
            "a full repaint clears the region list, not returns it"
        );
    }

    #[test]
    fn taking_dirty_regions_clears_pending_state() {
        let mut scheduler = RepaintScheduler::new();
        scheduler.invalidate(rect(1.0, 2.0, 3.0, 4.0));
        let regions = scheduler.take_dirty_regions();
        assert_eq!(regions, Some(vec![rect(1.0, 2.0, 3.0, 4.0)]));
        assert!(
            !scheduler.needs_repaint(Instant::now()),
            "state must be cleared after being taken, or the same repaint would be redone forever"
        );
    }

    #[test]
    fn many_small_invalidations_coalesce_instead_of_growing_unbounded() {
        let mut scheduler = RepaintScheduler::new();
        for i in 0..64 {
            scheduler.invalidate(rect(i as f32, 0.0, 1.0, 1.0));
        }
        let regions = scheduler.take_dirty_regions().expect("not a full repaint");
        assert!(
            regions.len() <= RepaintScheduler::MAX_REGIONS,
            "must have coalesced well before {} individual regions, got {}",
            64,
            regions.len()
        );
        // The coalesced union must still cover every original small rect —
        // spot-check the first and last.
        assert!(regions.iter().any(|r| r.contains(0.5, 0.5)));
        assert!(regions.iter().any(|r| r.contains(63.5, 0.5)));
    }

    #[test]
    fn a_timer_request_is_not_due_before_its_time_arrives() {
        let mut scheduler = RepaintScheduler::new();
        let now = Instant::now();
        scheduler.request_repaint_at(now + Duration::from_millis(500));
        assert!(!scheduler.needs_repaint(now));
    }

    #[test]
    fn a_timer_request_is_due_once_its_time_arrives() {
        let mut scheduler = RepaintScheduler::new();
        let now = Instant::now();
        let at = now + Duration::from_millis(10);
        scheduler.request_repaint_at(at);
        assert!(scheduler.needs_repaint(at));
        assert!(scheduler.needs_repaint(at + Duration::from_millis(1)));
    }

    #[test]
    fn the_earliest_of_two_timer_requests_wins() {
        let mut scheduler = RepaintScheduler::new();
        let now = Instant::now();
        scheduler.request_repaint_at(now + Duration::from_secs(10));
        scheduler.request_repaint_at(now + Duration::from_millis(1)); // earlier
        assert!(
            scheduler.needs_repaint(now + Duration::from_millis(2)),
            "the earlier of the two requests must be the one that governs"
        );
    }

    #[test]
    fn taking_dirty_regions_also_clears_a_pending_timer() {
        let mut scheduler = RepaintScheduler::new();
        let now = Instant::now();
        scheduler.request_repaint_at(now);
        scheduler.take_dirty_regions();
        assert!(!scheduler.needs_repaint(now + Duration::from_secs(1)));
    }
}
