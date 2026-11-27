//! A worker hands its result over before it looks for the event loop's
//! waker, so a waker registered at any moment still finds the result in
//! the channel when its wake-up runs `poll`.

use std::time::Duration;

use super::{Done, spawn};
use crate::document::tests::app;

/// The test holds the waker slot. A worker that reads the slot before
/// sending blocks there with its result unsent; one that sends first has
/// its result in the channel while it waits for the slot.
#[test]
fn a_worker_sends_its_result_before_reading_the_waker() {
    let mut app = app();
    let slot = app.ui.dialogs.jobs.wake.clone();
    let held = slot.lock().unwrap();
    spawn(&mut app, None, || Some(Done::UiFont));
    let received = app
        .ui
        .dialogs
        .jobs
        .receiver
        .recv_timeout(Duration::from_secs(5));
    drop(held);
    assert!(
        matches!(received, Ok(Some(Done::UiFont))),
        "the result was not sent before the waker was read"
    );
}
