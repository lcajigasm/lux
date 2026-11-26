//! Wake-up, progress and cancellation behavior of `find_in_files_notify`.

use super::*;
use std::fs;
use std::sync::mpsc;
use std::time::Duration;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lux-search-notify-{tag}-{:?}-{}",
        std::thread::current().id(),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(&dir).unwrap()
}

#[test]
fn notify_drives_the_stream_to_completion_without_blocking_reads() {
    let dir = temp_dir("drive");
    for i in 0..300 {
        let body = if i % 3 == 0 {
            "needle\nhay\nneedle\n"
        } else {
            "hay\n"
        };
        fs::write(dir.join(format!("f{i}.txt")), body).unwrap();
    }
    let (wake_tx, wake_rx) = mpsc::channel::<()>();
    let notify: Notify = Arc::new(move || {
        let _ = wake_tx.send(());
    });
    let mut stream =
        find_in_files_notify(&dir, "needle", &SearchOptions::default(), Some(notify)).unwrap();
    while !stream.is_finished() {
        wake_rx
            .recv_timeout(Duration::from_secs(20))
            .expect("the worker wakes the consumer until the search ends");
        stream.poll();
    }
    assert_eq!(stream.matches().len(), 200);
    assert_eq!(stream.files_searched(), 300);
    // A file's matches arrive as one contiguous batch.
    let mut seen = std::collections::HashSet::new();
    let mut last: Option<&Path> = None;
    for m in stream.matches() {
        if last != Some(m.path.as_path()) {
            assert!(seen.insert(m.path.clone()), "{:?} split", m.path);
            last = Some(m.path.as_path());
        }
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn files_searched_counts_files_without_matches() {
    let dir = temp_dir("count");
    fs::write(dir.join("a.txt"), "needle\n").unwrap();
    fs::write(dir.join("b.txt"), "nothing\n").unwrap();
    fs::write(dir.join("c.txt"), "nothing\n").unwrap();
    let mut stream = find_in_files(&dir, "needle", &SearchOptions::default()).unwrap();
    stream.wait_until_done();
    assert_eq!(stream.matches().len(), 1);
    assert_eq!(stream.files_searched(), 3);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn cancelled_stream_stops_waking() {
    let dir = temp_dir("cancel");
    for i in 0..200 {
        fs::write(dir.join(format!("f{i}.txt")), "needle\n").unwrap();
    }
    let (wake_tx, wake_rx) = mpsc::channel::<()>();
    let notify: Notify = Arc::new(move || {
        let _ = wake_tx.send(());
    });
    let stream =
        find_in_files_notify(&dir, "needle", &SearchOptions::default(), Some(notify)).unwrap();
    drop(stream);
    // Wakes sent before the drop may be queued; none arrive afterwards
    // once the workers have observed the cancel and the channel closes.
    while wake_rx.recv_timeout(Duration::from_secs(20)).is_ok() {}
    fs::remove_dir_all(&dir).ok();
}
