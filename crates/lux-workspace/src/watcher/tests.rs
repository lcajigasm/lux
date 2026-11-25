use super::*;
use std::fs;
use std::thread;

const DEBOUNCE: Duration = Duration::from_millis(150);
const RECV_TIMEOUT: Duration = Duration::from_secs(5);
// How long a settle wait blocks for before deciding nothing more is
// coming — see `drain_until_quiet`/`settle`.
const SETTLE_TIMEOUT: Duration = Duration::from_millis(2000);

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lux-workspace-watcher-test-{:?}-{}",
        thread::current().id(),
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    // Canonicalized so paths built from this (`dir.join(...)`) compare
    // equal to what `FileWatcher` reports — see `normalize`'s doc
    // comment for why that's not automatic on macOS.
    fs::canonicalize(&dir).unwrap()
}

/// Drains whatever batches keep arriving until a full `recv` call times
/// out with nothing new — i.e. the channel has gone quiet. `watch()`'s
/// historical-replay burst (FSEvents (macOS) and other native backends
/// report at least "this directory exists" right after `watch()`
/// returns) and `unwatch()`'s OS-level teardown both have latency that
/// varies a lot by machine (a loaded CI runner is not a quiet laptop);
/// waiting for actual quiescence instead of guessing a fixed sleep
/// duration is what makes these tests reliable across both. Bounded by
/// `SETTLE_TIMEOUT` per `recv` call, not a polling loop — each call
/// blocks on the same real, event-driven channel every other test uses.
fn settle(watcher: &mut FileWatcher) {
    loop {
        let batch = watcher.recv(SETTLE_TIMEOUT);
        if batch.changes.is_empty() && batch.errors.is_empty() {
            break;
        }
    }
}

fn watch_dir(dir: &Path) -> FileWatcher {
    let mut watcher = FileWatcher::new(DEBOUNCE).unwrap();
    watcher.watch(dir).unwrap();
    settle(&mut watcher);
    watcher
}

#[test]
#[cfg(unix)]
fn scan_known_does_not_recurse_into_a_symlink_cycle() {
    let dir = temp_dir();
    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();
    // A symlink inside `sub` pointing back at `dir` — a real cycle a
    // naive `Path::is_dir()`-following recursion would loop on forever.
    std::os::unix::fs::symlink(&dir, sub.join("back_to_root")).unwrap();

    let mut known = HashSet::new();
    scan_known(&dir, &mut known);
    // Terminates at all is the actual assertion; the symlink itself is
    // recorded as a leaf, not descended into.
    assert!(known.contains(&sub.join("back_to_root")));

    fs::remove_dir_all(&dir).ok();
}

fn wait_for(watcher: &mut FileWatcher, mut matches: impl FnMut(&FileChange) -> bool) -> FileChange {
    let deadline = std::time::Instant::now() + RECV_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        assert!(
            !remaining.is_zero(),
            "timed out waiting for the expected change"
        );
        let batch = watcher.recv(remaining);
        if let Some(change) = batch.changes.into_iter().find(|c| matches(c)) {
            return change;
        }
    }
}

#[test]
fn detects_a_created_file() {
    let dir = temp_dir();
    let mut watcher = watch_dir(&dir);

    let file = dir.join("new.txt");
    fs::write(&file, "hi").unwrap();

    let change = wait_for(&mut watcher, |c| c.path == file);
    assert_eq!(change.kind, ChangeKind::Created);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn detects_a_removed_file() {
    let dir = temp_dir();
    let file = dir.join("gone.txt");
    fs::write(&file, "hi").unwrap();
    let mut watcher = watch_dir(&dir);

    fs::remove_file(&file).unwrap();

    let change = wait_for(&mut watcher, |c| {
        c.path == file && c.kind == ChangeKind::Removed
    });
    assert_eq!(change.kind, ChangeKind::Removed);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn modifying_a_clean_file_reports_reload() {
    let dir = temp_dir();
    let file = dir.join("clean.txt");
    fs::write(&file, "before").unwrap();
    let mut watcher = watch_dir(&dir);

    fs::write(&file, "after").unwrap();

    let change = wait_for(&mut watcher, |c| {
        c.path == file && c.kind == ChangeKind::Reload
    });
    assert_eq!(change.kind, ChangeKind::Reload);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn modifying_a_dirty_file_reports_conflict() {
    let dir = temp_dir();
    let file = dir.join("dirty.txt");
    fs::write(&file, "before").unwrap();
    let mut watcher = watch_dir(&dir);
    watcher.mark_dirty(file.clone());

    fs::write(&file, "changed externally").unwrap();

    let change = wait_for(&mut watcher, |c| {
        c.path == file && c.kind == ChangeKind::Conflict
    });
    assert_eq!(change.kind, ChangeKind::Conflict);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn mark_clean_reverts_to_reload_classification() {
    let dir = temp_dir();
    let file = dir.join("was_dirty.txt");
    fs::write(&file, "before").unwrap();
    let mut watcher = watch_dir(&dir);
    watcher.mark_dirty(file.clone());
    assert!(watcher.is_dirty(&file));

    watcher.mark_clean(&file);
    assert!(!watcher.is_dirty(&file));

    fs::write(&file, "after").unwrap();
    let change = wait_for(&mut watcher, |c| {
        c.path == file && c.kind == ChangeKind::Reload
    });
    assert_eq!(change.kind, ChangeKind::Reload);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn try_recv_is_non_blocking_and_empty_with_no_changes() {
    let dir = temp_dir();
    let mut watcher = watch_dir(&dir);
    let batch = watcher.try_recv();
    assert!(batch.changes.is_empty());
    assert!(batch.errors.is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn recv_returns_empty_batch_on_timeout_with_no_changes() {
    let dir = temp_dir();
    let mut watcher = watch_dir(&dir);
    let batch = watcher.recv(Duration::from_millis(200));
    assert!(batch.changes.is_empty());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn unwatch_stops_reporting_changes_from_that_root() {
    let dir = temp_dir();
    let mut watcher = watch_dir(&dir);
    watcher.unwatch(&dir).unwrap();
    // `unwatch` returning doesn't guarantee the OS has actually torn
    // down the underlying watch stream yet — settle the same way
    // `watch_dir` does, so a write made right after this doesn't race
    // a not-yet-finished teardown on a slower machine.
    settle(&mut watcher);

    fs::write(dir.join("after_unwatch.txt"), "hi").unwrap();

    let batch = watcher.recv(Duration::from_millis(500));
    assert!(batch.changes.is_empty());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_external_edit_invokes_the_registered_waker() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let dir = temp_dir();
    let file = dir.join("wake.txt");
    fs::write(&file, "a").unwrap();
    let mut watcher = watch_dir(&dir);
    let hit = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&hit);
    watcher.set_waker(Arc::new(move || flag.store(true, Ordering::SeqCst)));

    fs::write(&file, "b").unwrap();
    let deadline = std::time::Instant::now() + RECV_TIMEOUT;
    while !hit.load(Ordering::SeqCst) {
        assert!(
            std::time::Instant::now() < deadline,
            "the waker was not called after an external edit"
        );
        thread::sleep(Duration::from_millis(20));
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn is_watch_limit_reached_identifies_the_max_files_watch_error() {
    let limit_err = notify::Error::new(notify::ErrorKind::MaxFilesWatch);
    assert!(is_watch_limit_reached(&limit_err));

    let other_err = notify::Error::new(notify::ErrorKind::WatchNotFound);
    assert!(!is_watch_limit_reached(&other_err));
}

#[test]
fn dirty_tracking_is_independent_per_path() {
    let dir = temp_dir();
    let a = dir.join("a.txt");
    let b = dir.join("b.txt");
    fs::write(&a, "a").unwrap();
    fs::write(&b, "b").unwrap();
    let mut watcher = watch_dir(&dir);

    watcher.mark_dirty(a.clone());
    assert!(watcher.is_dirty(&a));
    assert!(!watcher.is_dirty(&b));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_created_directory_is_reported_as_one() {
    let dir = temp_dir().join("dir-kind");
    fs::create_dir_all(&dir).unwrap();
    let mut watcher = watch_dir(&dir);

    let sub = dir.join("sub");
    fs::create_dir(&sub).unwrap();
    let change = wait_for(&mut watcher, |c| c.path == sub);
    assert_eq!(change.kind, ChangeKind::Created);
    assert!(change.is_dir);

    let file = dir.join("plain.txt");
    fs::write(&file, "x").unwrap();
    let change = wait_for(&mut watcher, |c| c.path == file);
    assert!(!change.is_dir);

    fs::remove_dir_all(&dir).ok();
}

/// Two roots, the second added on a worker thread. The waker runs once
/// that watch is in place, and a file created there is reported.
#[test]
fn a_detached_watch_reports_changes_in_a_second_root() {
    let base = temp_dir().join("two-roots");
    let first = base.join("first");
    let second = base.join("second");
    fs::create_dir_all(&first).unwrap();
    fs::create_dir_all(&second).unwrap();
    let mut watcher = watch_dir(&first);
    let (tx, rx) = std::sync::mpsc::channel();
    watcher.set_waker(Arc::new(move || {
        let _ = tx.send(());
    }));
    watcher.watch_detached(&second);
    rx.recv_timeout(RECV_TIMEOUT)
        .expect("the detached watch wakes the caller");
    settle(&mut watcher);

    let file = second.join("arrived.txt");
    fs::write(&file, "hi").unwrap();
    let change = wait_for(&mut watcher, |c| c.path == file);
    assert_eq!(change.kind, ChangeKind::Created);

    fs::remove_dir_all(&base).ok();
}

#[test]
fn a_detached_watch_on_a_missing_root_reports_an_error() {
    let mut watcher = FileWatcher::new(DEBOUNCE).unwrap();
    watcher.watch_detached(temp_dir().join("does-not-exist"));
    let batch = watcher.recv(RECV_TIMEOUT);
    assert!(
        batch.errors.iter().any(|e| e.contains("cannot watch")),
        "{:?}",
        batch.errors
    );
}

/// A root added and closed at once stays closed: the watch and the
/// unwatch run on one worker in the order they were asked. The second
/// root's watch, asked last, is in place only after both.
#[test]
fn a_root_closed_right_after_it_was_added_is_not_watched() {
    let base = temp_dir().join("add-then-close");
    let closed = base.join("closed");
    let kept = base.join("kept");
    fs::create_dir_all(&closed).unwrap();
    fs::create_dir_all(&kept).unwrap();
    let mut watcher = FileWatcher::new(DEBOUNCE).unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    watcher.set_waker(Arc::new(move || {
        let _ = tx.send(());
    }));
    watcher.watch_detached(&closed);
    watcher.unwatch_detached(&closed);
    watcher.watch_detached(&kept);
    for _ in 0..2 {
        rx.recv_timeout(RECV_TIMEOUT)
            .expect("each detached watch wakes the caller");
    }
    settle(&mut watcher);

    fs::write(closed.join("ignored.txt"), "no").unwrap();
    let file = kept.join("seen.txt");
    fs::write(&file, "yes").unwrap();
    let mut changes = Vec::new();
    let deadline = std::time::Instant::now() + RECV_TIMEOUT;
    while !changes.iter().any(|c: &FileChange| c.path == file) {
        assert!(std::time::Instant::now() < deadline, "{changes:?}");
        changes.extend(watcher.recv(RECV_TIMEOUT).changes);
    }
    loop {
        let batch = watcher.recv(SETTLE_TIMEOUT);
        if batch.changes.is_empty() {
            break;
        }
        changes.extend(batch.changes);
    }
    assert!(
        !changes.iter().any(|c| c.path.starts_with(&closed)),
        "{changes:?}"
    );

    fs::remove_dir_all(&base).ok();
}
