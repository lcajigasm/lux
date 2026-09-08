//! A requested frame stays dirty until presentation succeeds. Transient
//! acquisition failure gets one delayed retry; occlusion waits for an OS event.

use lux_render::RepaintScheduler;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
pub(crate) enum Outcome {
    Presented,
    Retry,
    Hidden,
    Failed,
}

#[derive(Default)]
pub(crate) struct Presentation {
    hidden: bool,
    retry_at: Option<Instant>,
}

impl Presentation {
    pub(crate) fn ready(&self, now: Instant) -> bool {
        !self.hidden && self.retry_at.is_none_or(|at| now >= at)
    }
    pub(crate) fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        self.retry_at = None;
    }
    pub(crate) fn finish(
        &mut self,
        outcome: Outcome,
        scheduler: &mut RepaintScheduler,
        now: Instant,
    ) -> Option<Instant> {
        match outcome {
            Outcome::Presented => {
                scheduler.take_dirty_regions();
                self.retry_at = None;
            }
            Outcome::Retry => {
                let at = now + Duration::from_millis(16);
                self.retry_at = Some(at);
                scheduler.request_repaint_at(at);
                return Some(at);
            }
            Outcome::Hidden | Outcome::Failed => self.set_hidden(true),
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_failed_first_frame_keeps_the_document_dirty_until_presented() {
        let now = Instant::now();
        let mut state = Presentation::default();
        let mut scheduler = RepaintScheduler::new();
        scheduler.invalidate_all();
        let retry = state.finish(Outcome::Retry, &mut scheduler, now).unwrap();
        assert!(scheduler.needs_repaint(now));
        assert!(!state.ready(now));
        assert!(state.ready(retry));
        state.finish(Outcome::Hidden, &mut scheduler, retry);
        assert!(!state.ready(retry + Duration::from_secs(5)));
        assert!(scheduler.needs_repaint(retry));
        state.set_hidden(false);
        assert!(state.ready(retry));
        state.finish(Outcome::Presented, &mut scheduler, retry);
        assert!(!scheduler.needs_repaint(retry));
    }
}
