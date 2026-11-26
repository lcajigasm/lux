//! A background writer where only the newest job matters: recovery
//! snapshots and session saves. The UI thread hands over a cheap snapshot
//! and returns; a worker thread, started on demand, writes it. A job that
//! arrives while one is being written replaces any job still waiting, so
//! a burst of edits costs one write, not one per keystroke. No thread
//! lingers when there is nothing to write. A keyed worker replaces only a
//! waiting job with the same key, so writes to different files never
//! drop each other.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct LatestWorker<T: Send + 'static> {
    name: &'static str,
    /// Waiting jobs, oldest first; at most one per key.
    slot: Mutex<Vec<T>>,
    /// Held while a job is taken and written, so [`Self::run_now`] can
    /// never be overtaken by an older job finishing after it.
    writing: Mutex<()>,
    running: AtomicBool,
    run: fn(T),
    /// Whether two jobs write the same thing, so the newer replaces the older.
    same: fn(&T, &T) -> bool,
}

impl<T: Send + 'static> LatestWorker<T> {
    pub(crate) const fn new(name: &'static str, run: fn(T)) -> Self {
        Self::keyed(name, run, |_, _| true)
    }

    /// A worker that keeps the newest waiting job per key, as `same` tells.
    pub(crate) const fn keyed(name: &'static str, run: fn(T), same: fn(&T, &T) -> bool) -> Self {
        LatestWorker {
            name,
            slot: Mutex::new(Vec::new()),
            writing: Mutex::new(()),
            running: AtomicBool::new(false),
            run,
            same,
        }
    }

    /// Queues `job`, replacing a job with the same key not yet started.
    pub(crate) fn submit(&'static self, job: T) {
        if let Ok(mut slot) = self.slot.lock() {
            match slot.iter().position(|waiting| (self.same)(waiting, &job)) {
                Some(index) => slot[index] = job,
                None => slot.push(job),
            }
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

    /// Writes `job` with `write` on the calling thread in place of the
    /// worker, after the job in flight. A waiting job with `job`'s key is
    /// discarded; waiting jobs with other keys are written first, here, so
    /// none is lost. Returns what `write` returns. For the quit path, where
    /// the process must not exit before the newest state is on disk and
    /// the caller needs to know whether it got there.
    pub(crate) fn run_now<R>(&self, job: T, write: impl FnOnce(T) -> R) -> R {
        let _writing = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        let waiting = match self.slot.lock() {
            Ok(mut slot) => std::mem::take(&mut *slot),
            Err(_) => Vec::new(),
        };
        for other in waiting {
            if !(self.same)(&other, &job) {
                (self.run)(other);
            }
        }
        write(job)
    }

    fn drain(&'static self) {
        loop {
            let writing = self.writing.lock().unwrap_or_else(|e| e.into_inner());
            let job = self
                .slot
                .lock()
                .ok()
                .and_then(|mut slot| (!slot.is_empty()).then(|| slot.remove(0)));
            let Some(job) = job else {
                drop(writing);
                self.running.store(false, Ordering::Release);
                // A job queued between the take and the store found the
                // flag still set and started no thread. Pick it up here.
                let queued = self.slot.lock().ok().is_some_and(|slot| !slot.is_empty());
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
        WORKER.run_now(70, record);
        assert_eq!(LAST.load(Ordering::SeqCst), 70, "run_now was overtaken");
    }

    static KEYED_SEEN: std::sync::Mutex<Vec<(u8, u32)>> = std::sync::Mutex::new(Vec::new());

    fn record_keyed(job: (u8, u32)) {
        std::thread::sleep(std::time::Duration::from_millis(20));
        KEYED_SEEN.lock().unwrap().push(job);
    }

    static KEYED: LatestWorker<(u8, u32)> =
        LatestWorker::keyed("lux-test-keyed", record_keyed, |a, b| a.0 == b.0);

    /// A queued write for one file survives a newer write for another
    /// file, and each file still ends with its newest job.
    #[test]
    fn a_keyed_worker_keeps_the_newest_job_per_key() {
        for job in [(1, 1), (1, 2), (2, 1), (1, 3), (2, 2)] {
            KEYED.submit(job);
        }
        // The quit path: (3, 2) replaces a waiting (3, 1), and a waiting
        // job for key 4 is written first rather than dropped.
        KEYED.submit((3, 1));
        KEYED.submit((4, 1));
        KEYED.run_now((3, 2), record_keyed);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while KEYED.busy() {
            assert!(std::time::Instant::now() < deadline, "worker never settled");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let seen = KEYED_SEEN.lock().unwrap().clone();
        let last = |key: u8| seen.iter().rev().find(|job| job.0 == key).copied();
        assert_eq!(last(1), Some((1, 3)), "{seen:?}");
        assert_eq!(last(2), Some((2, 2)), "{seen:?}");
        assert_eq!(last(4), Some((4, 1)), "{seen:?}");
        assert_eq!(last(3), Some((3, 2)), "{seen:?}");
    }
}
