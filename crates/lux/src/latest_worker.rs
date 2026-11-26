//! A background writer where only the newest job matters: recovery
//! snapshots and session saves. The UI thread hands over a cheap snapshot
//! and returns; a worker thread, started on demand, writes it. A job that
//! arrives while one is being written replaces any job still waiting, so
//! a burst of edits costs one write, not one per keystroke. No thread
//! lingers when there is nothing to write.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct LatestWorker<T: Send + 'static> {
    name: &'static str,
    slot: Mutex<Option<T>>,
    /// Held while a job is taken and written, so [`Self::run_now`] can
    /// never be overtaken by an older job finishing after it.
    writing: Mutex<()>,
    running: AtomicBool,
    run: fn(T),
}

impl<T: Send + 'static> LatestWorker<T> {
    pub(crate) const fn new(name: &'static str, run: fn(T)) -> Self {
        LatestWorker {
            name,
            slot: Mutex::new(None),
            writing: Mutex::new(()),
            running: AtomicBool::new(false),
            run,
        }
    }

    /// Queues `job`, replacing any job not yet started.
    pub(crate) fn submit(&'static self, job: T) {
        if let Ok(mut slot) = self.slot.lock() {
            *slot = Some(job);
        }
        if self.running.swap(true, Ordering::AcqRel) {
            return;
        }
        let spawned = std::thread::Builder::new()
            .name(self.name.into())
            .spawn(move || self.drain());
        if spawned.is_err() {
            self.running.store(false, Ordering::Release);
        }
    }

    /// Runs `write` on the calling thread in place of the worker,
    /// discarding any job still waiting and waiting for the one in flight,
    /// and returns what `write` returns. For the quit path, where the
    /// process must not exit before the newest state is on disk and the
    /// caller needs to know whether it got there.
    pub(crate) fn run_now<R>(&self, write: impl FnOnce() -> R) -> R {
        let _writing = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(mut slot) = self.slot.lock() {
            slot.take();
        }
        write()
    }

    fn drain(&'static self) {
        loop {
            let writing = self.writing.lock().unwrap_or_else(|e| e.into_inner());
            let job = self.slot.lock().ok().and_then(|mut slot| slot.take());
            let Some(job) = job else {
                drop(writing);
                self.running.store(false, Ordering::Release);
                // A job queued between the take and the store found the
                // flag still set and started no thread. Pick it up here.
                let queued = self.slot.lock().ok().is_some_and(|slot| slot.is_some());
                if queued && !self.running.swap(true, Ordering::AcqRel) {
                    continue;
                }
                return;
            };
            (self.run)(job);
            drop(writing);
        }
    }

    /// True while a job is queued or being written.
    #[cfg(test)]
    pub(crate) fn busy(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static SEEN: AtomicUsize = AtomicUsize::new(0);
    static LAST: AtomicUsize = AtomicUsize::new(0);

    fn record(job: usize) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        SEEN.fetch_add(1, Ordering::SeqCst);
        LAST.store(job, Ordering::SeqCst);
    }

    static WORKER: LatestWorker<usize> = LatestWorker::new("lux-test-latest", record);

    #[test]
    fn a_burst_ends_with_the_newest_job_written() {
        for job in 1..=50 {
            WORKER.submit(job);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while WORKER.busy() || LAST.load(Ordering::SeqCst) != 50 {
            assert!(std::time::Instant::now() < deadline, "worker never settled");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(SEEN.load(Ordering::SeqCst) < 50, "jobs were not coalesced");
        WORKER.submit(60);
        WORKER.run_now(|| record(70));
        assert_eq!(LAST.load(Ordering::SeqCst), 70, "run_now was overtaken");
    }
}
