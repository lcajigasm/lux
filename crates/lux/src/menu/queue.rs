//! Thread-safe menu action queue and immediate event-loop waker (ST-FIX-02).

use std::sync::{Arc, Mutex};

use super::MenuAction;

static MENU_QUEUE: Mutex<Vec<MenuAction>> = Mutex::new(Vec::new());
static WAKER: Mutex<Option<Arc<dyn Fn() + Send + Sync>>> = Mutex::new(None);

/// Registers the platform event loop waker (e.g. `PlatformContext::background_waker`).
pub(crate) fn register_waker(waker: Arc<dyn Fn() + Send + Sync>) {
    if let Ok(mut lock) = WAKER.lock() {
        *lock = Some(waker);
    }
}

/// Wakes the platform event loop immediately if a waker is registered.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn wake() {
    if let Ok(lock) = WAKER.lock()
        && let Some(waker) = lock.as_ref()
    {
        waker();
    }
}

/// Posts a menu action to the queue and immediately wakes the event loop.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn post_action(action: MenuAction) {
    if let Ok(mut queue) = MENU_QUEUE.lock() {
        queue.push(action);
    }
    wake();
}

/// Takes all pending menu actions from the queue.
pub(crate) fn poll_actions() -> Vec<MenuAction> {
    if let Ok(mut queue) = MENU_QUEUE.lock() {
        std::mem::take(&mut *queue)
    } else {
        Vec::new()
    }
}
