//! When to write the session between launches and quits (ADR 0019).
//!
//! The UI thread calls [`note_change`] on a meaningful change: an edit, a
//! caret move, a tab or layout change. That only stores a timestamp. A
//! helper thread, started on demand, sleeps until the changes have been
//! quiet for [`QUIET`] and then wakes the event loop through the platform
//! waker. The loop answers in `background_event` with one snapshot, so a
//! burst of typing costs one snapshot and one write, never one per key.
//! Nothing runs while nothing changes.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Quiet period after the last change before the session is written.
pub(crate) const QUIET: Duration = Duration::from_millis(750);

type Waker = Arc<dyn Fn() + Send + Sync>;

struct State {
    last_change: Option<Instant>,
    running: bool,
    waker: Option<Waker>,
}

pub(crate) struct Debounce {
    state: Mutex<State>,
    due: AtomicBool,
}

/// The editor's one debouncer.
pub(crate) static SESSION: Debounce = Debounce::new();

impl Debounce {
    pub(crate) const fn new() -> Self {
        Debounce {
            state: Mutex::new(State {
                last_change: None,
                running: false,
                waker: None,
            }),
            due: AtomicBool::new(false),
        }
    }

    pub(crate) fn register_waker(&self, waker: Waker) {
        self.lock().waker = Some(waker);
    }

    /// Records a change. Without a registered waker (tests, before the
    /// window exists) there is no loop to wake, so nothing is scheduled.
    pub(crate) fn note_change(&'static self) {
        let mut state = self.lock();
        if state.waker.is_none() {
            return;
        }
        state.last_change = Some(Instant::now());
        if state.running {
            return;
        }
        state.running = true;
        let spawned = std::thread::Builder::new()
            .name("lux-session-debounce".into())
            .spawn(move || self.wait_for_quiet());
        if spawned.is_err() {
            state.running = false;
        }
    }

    /// True once per quiet period: the caller should write the session now.
    pub(crate) fn take_due(&self) -> bool {
        self.due.swap(false, Ordering::AcqRel)
    }

    fn wait_for_quiet(&self) {
        loop {
            let mut state = self.lock();
            let due_at = state.last_change.map_or_else(Instant::now, |at| at + QUIET);
            let now = Instant::now();
            if now >= due_at {
                state.last_change = None;
                state.running = false;
                let waker = state.waker.clone();
                drop(state);
                self.due.store(true, Ordering::Release);
                if let Some(waker) = waker {
                    waker();
                }
                return;
            }
            drop(state);
            std::thread::sleep(due_at - now);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}
