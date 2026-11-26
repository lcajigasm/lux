//! The latest-wins lane of `ui_io`: one long-lived worker thread and a
//! one-job mailbox. Posting replaces a job still waiting there, so a burst
//! of requests (holding ↓ in Goto Anything) runs only the newest one; the
//! jobs it replaced are dropped without running.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

use super::{Gate, Task};

#[derive(Default)]
struct Mailbox {
    job: Option<Task>,
    closed: bool,
}

#[derive(Default)]
struct Slot {
    mailbox: Mutex<Mailbox>,
    ready: Condvar,
}

fn lock(slot: &Slot) -> MutexGuard<'_, Mailbox> {
    slot.mailbox.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The owner's side of the lane. The worker starts on the first post and
/// stops when this is dropped.
#[derive(Default)]
pub(super) struct Latest {
    slot: Option<Arc<Slot>>,
}

impl Latest {
    /// Puts `task` in the mailbox. `Ok(true)` when the mailbox was empty,
    /// so one more result will arrive; `Ok(false)` when `task` replaced a
    /// waiting job, whose result now never arrives.
    pub(super) fn post(&mut self, task: Task, gate: &Arc<Gate>) -> std::io::Result<bool> {
        let slot = match &self.slot {
            Some(slot) => slot.clone(),
            None => {
                let slot = Arc::new(Slot::default());
                let (worker, gate) = (slot.clone(), gate.clone());
                std::thread::Builder::new()
                    .name("lux-ui-io-latest".into())
                    .spawn(move || run(&worker, &gate))?;
                self.slot = Some(slot.clone());
                slot
            }
        };
        let fresh = lock(&slot).job.replace(task).is_none();
        slot.ready.notify_one();
        Ok(fresh)
    }

    /// Drops the waiting job, if any. `true` when one was dropped.
    pub(super) fn clear(&mut self) -> bool {
        self.slot
            .as_ref()
            .is_some_and(|slot| lock(slot).job.take().is_some())
    }
}

impl Drop for Latest {
    fn drop(&mut self) {
        if let Some(slot) = &self.slot {
            let mut mailbox = lock(slot);
            mailbox.closed = true;
            mailbox.job = None;
            drop(mailbox);
            slot.ready.notify_all();
        }
    }
}

fn run(slot: &Slot, gate: &Gate) {
    loop {
        let mut mailbox = lock(slot);
        while mailbox.job.is_none() && !mailbox.closed {
            mailbox = slot
                .ready
                .wait(mailbox)
                .unwrap_or_else(PoisonError::into_inner);
        }
        if mailbox.closed {
            return;
        }
        drop(mailbox);
        // Tests close the gate here, before the job is taken, so the jobs
        // posted meanwhile replace one another.
        gate.wait();
        let job = lock(slot).job.take();
        if let Some(job) = job {
            job();
        }
    }
}
