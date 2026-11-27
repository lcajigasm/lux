//! macOS: dialogs as sheets attached to the editor window.
//!
//! `rfd`'s blocking calls on macOS run `-[NSSavePanel runModal]` (or
//! `-[NSAlert runModal]`) on the main thread, a nested modal run loop that
//! holds the main thread until the panel closes. The async variants with a
//! parent window instead call `beginSheetModalForWindow:completionHandler:`
//! and return at once, so winit's `[NSApp run]` keeps dispatching events,
//! timers and redraws while the sheet is up. The worker that asked for the
//! dialog parks until the completion handler wakes it.
//!
//! No AppKit call happens on the worker. The editor window's content view
//! is retained on the main thread when the dialog is requested
//! (`ctx.background_dialogs()`), so the handle cannot dangle if the window
//! closes meanwhile. The worker then runs the whole `rfd` setup, which
//! resolves `[view window]`, builds the panel and begins the sheet, on the
//! main thread with `dispatch_sync_f`, and only parks on the returned
//! future itself. The view is released on the main thread too.
//!
//! A sheet requested from the main thread would park the thread its
//! completion handler needs. Those calls return `None` and the caller
//! falls back to the blocking dialog.

use std::ffi::{c_char, c_void};
use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::ptr::NonNull;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::Thread;

use raw_window_handle::{
    AppKitWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    RawWindowHandle, WindowHandle,
};

use crate::dialog::{FileDialogOptions, SaveDecision};

#[repr(C)]
struct DispatchQueue {
    _private: [u8; 0],
}

type Work = unsafe extern "C" fn(*mut c_void);

unsafe extern "C" {
    fn pthread_main_np() -> i32;
    static _dispatch_main_q: DispatchQueue;
    fn dispatch_sync_f(queue: *const DispatchQueue, context: *mut c_void, work: Work);
    fn dispatch_async_f(queue: *const DispatchQueue, context: *mut c_void, work: Work);
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_retain(object: *mut c_void) -> *mut c_void;
    fn objc_release(object: *mut c_void);
    fn sel_registerName(name: *const c_char) -> *const c_void;
    /// Called only with this exact signature (an object-returning,
    /// argument-less selector), as arm64 requires.
    fn objc_msgSend(receiver: *mut c_void, selector: *const c_void) -> *mut c_void;
}

fn on_main_thread() -> bool {
    // SAFETY: no arguments, no preconditions; returns 1 on the main thread.
    unsafe { pthread_main_np() == 1 }
}

/// Runs `work` on the main thread and waits for it. The main thread never
/// waits on a worker (CLAUDE.md rule 6), so this cannot deadlock while the
/// event loop runs. A panic inside `work` is resumed here, on the worker.
fn on_main<R: Send, F: FnOnce() -> R + Send>(work: F) -> R {
    struct Call<F, R> {
        work: Option<F>,
        out: Option<std::thread::Result<R>>,
    }
    unsafe extern "C" fn trampoline<F: FnOnce() -> R, R>(context: *mut c_void) {
        // SAFETY: `context` is the `Call` below, alive and exclusively ours
        // for the duration of the synchronous dispatch.
        let call = unsafe { &mut *context.cast::<Call<F, R>>() };
        if let Some(work) = call.work.take() {
            call.out = Some(std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)));
        }
    }
    let mut call = Call {
        work: Some(work),
        out: None,
    };
    // SAFETY: `_dispatch_main_q` is the process's main queue; `call` lives
    // until `dispatch_sync_f` returns, which is after `trampoline` ran.
    unsafe {
        dispatch_sync_f(
            &raw const _dispatch_main_q,
            (&raw mut call).cast(),
            trampoline::<F, R>,
        );
    }
    match call.out.expect("the main queue ran the dialog setup") {
        Ok(value) => value,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

/// The editor window's content view, retained on the main thread.
struct Parent(NonNull<c_void>);

// SAFETY: the view is only messaged on the main thread (`on_main`), and it
// is released there (`Drop`). Retain counts are thread-safe.
unsafe impl Send for Parent {}
// SAFETY: as above; `&Parent` only exposes the pointer to main-thread code.
unsafe impl Sync for Parent {}

impl Parent {
    /// Main thread only, which is where `ctx.background_dialogs()` runs.
    fn of(window: &winit::window::Window) -> Option<Self> {
        if !on_main_thread() {
            return None;
        }
        let RawWindowHandle::AppKit(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        // SAFETY: winit's live content view; the +1 is balanced in `Drop`.
        let view = unsafe { objc_retain(handle.ns_view.as_ptr()) };
        NonNull::new(view).map(Self)
    }

    /// Main thread only: whether the view still sits in a window. `rfd`
    /// panics on a view without one.
    fn in_window(&self) -> bool {
        debug_assert!(on_main_thread());
        // SAFETY: a retained NSView, messaged on the main thread with the
        // argument-less, object-returning `window` selector.
        let window = unsafe { objc_msgSend(self.0.as_ptr(), sel_registerName(c"window".as_ptr())) };
        !window.is_null()
    }
}

impl Drop for Parent {
    fn drop(&mut self) {
        unsafe extern "C" fn release(view: *mut c_void) {
            // SAFETY: the +1 taken in `Parent::of`, released once.
            unsafe { objc_release(view) }
        }
        // SAFETY: the main queue outlives the process's windows; the
        // pointer stays valid until `release` balances the retain.
        unsafe { dispatch_async_f(&raw const _dispatch_main_q, self.0.as_ptr(), release) }
    }
}

impl HasWindowHandle for Parent {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = RawWindowHandle::AppKit(AppKitWindowHandle::new(self.0));
        // SAFETY: the view is retained for as long as `self` lives.
        Ok(unsafe { WindowHandle::borrow_raw(raw) })
    }
}

impl HasDisplayHandle for Parent {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::appkit())
    }
}

struct Unpark(Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

/// Parks the calling (worker) thread until `future` completes. The sheet's
/// completion handler is the only waker, so this never spins.
fn block_on<F: Future>(future: F) -> F::Output {
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::thread::park();
    }
}

/// The window sheets attach to. Empty when there is no window to attach to.
#[derive(Default)]
pub(super) struct Sheets {
    parent: Option<Parent>,
}

/// Main thread only (inside `on_main`).
fn files(parent: &Parent, options: &FileDialogOptions) -> rfd::AsyncFileDialog {
    let mut dialog = rfd::AsyncFileDialog::new();
    if parent.in_window() {
        dialog = dialog.set_parent(parent);
    }
    if let Some(title) = &options.title {
        dialog = dialog.set_title(title.as_str());
    }
    if let Some(dir) = &options.starting_directory {
        dialog = dialog.set_directory(dir);
    }
    for (name, extensions) in &options.filters {
        dialog = dialog.add_filter(name.as_str(), extensions);
    }
    dialog
}

/// Main thread only (inside `on_main`).
fn alert(parent: &Parent) -> rfd::AsyncMessageDialog {
    let dialog = rfd::AsyncMessageDialog::new();
    if parent.in_window() {
        dialog.set_parent(parent)
    } else {
        dialog
    }
}

impl Sheets {
    /// Retains the window's view. Call on the main thread.
    pub(super) fn with_parent(window: &winit::window::Window) -> Self {
        Self {
            parent: Parent::of(window),
        }
    }

    /// The parent, when this call may show a sheet: from a worker only.
    fn parent(&self) -> Option<&Parent> {
        if on_main_thread() {
            return None;
        }
        self.parent.as_ref()
    }

    pub(super) fn pick_file(&self, options: &FileDialogOptions) -> Option<Option<PathBuf>> {
        let parent = self.parent()?;
        let picked = on_main(|| files(parent, options).pick_file());
        Some(block_on(picked).map(|file| file.path().to_path_buf()))
    }

    pub(super) fn pick_files(&self, options: &FileDialogOptions) -> Option<Vec<PathBuf>> {
        let parent = self.parent()?;
        let picked = block_on(on_main(|| files(parent, options).pick_files()));
        Some(
            picked
                .unwrap_or_default()
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect(),
        )
    }

    pub(super) fn pick_folder(&self, options: &FileDialogOptions) -> Option<Option<PathBuf>> {
        let parent = self.parent()?;
        let picked = on_main(|| files(parent, options).pick_folder());
        Some(block_on(picked).map(|file| file.path().to_path_buf()))
    }

    pub(super) fn save_file(&self, options: &FileDialogOptions) -> Option<Option<PathBuf>> {
        let parent = self.parent()?;
        let picked = on_main(|| files(parent, options).save_file());
        Some(block_on(picked).map(|file| file.path().to_path_buf()))
    }

    pub(super) fn confirm_save(&self, document: &str) -> Option<SaveDecision> {
        let parent = self.parent()?;
        let shown = on_main(|| {
            alert(parent)
                .set_title("Unsaved changes")
                .set_description(format!("Save changes to {document} before continuing?"))
                .set_buttons(rfd::MessageButtons::YesNoCancel)
                .show()
        });
        Some(match block_on(shown) {
            rfd::MessageDialogResult::Yes => SaveDecision::Save,
            rfd::MessageDialogResult::No => SaveDecision::Discard,
            _ => SaveDecision::Cancel,
        })
    }

    pub(super) fn confirm_trash(&self, item: &str) -> Option<bool> {
        let parent = self.parent()?;
        let shown = on_main(|| {
            alert(parent)
                .set_level(rfd::MessageLevel::Warning)
                .set_title("Move to Trash")
                .set_description(format!("Move \"{item}\" to the Trash?"))
                .set_buttons(rfd::MessageButtons::OkCancel)
                .show()
        });
        Some(matches!(
            block_on(shown),
            rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn block_on_parks_until_another_thread_wakes_it() {
        let slot: Arc<Mutex<(Option<u32>, Option<Waker>)>> = Arc::default();
        let remote = slot.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            let mut guard = remote.lock().unwrap();
            guard.0 = Some(7);
            if let Some(waker) = guard.1.take() {
                waker.wake();
            }
        });
        let value = block_on(std::future::poll_fn(|cx| {
            let mut guard = slot.lock().unwrap();
            match guard.0 {
                Some(v) => Poll::Ready(v),
                None => {
                    guard.1 = Some(cx.waker().clone());
                    Poll::Pending
                }
            }
        }));
        assert_eq!(value, 7);
    }

    #[test]
    fn without_a_parent_window_every_call_falls_back() {
        let sheets = Sheets::default();
        let options = FileDialogOptions::default();
        assert!(sheets.pick_file(&options).is_none());
        assert!(sheets.pick_files(&options).is_none());
        assert!(sheets.pick_folder(&options).is_none());
        assert!(sheets.save_file(&options).is_none());
        assert!(sheets.confirm_save("a.txt").is_none());
        assert!(sheets.confirm_trash("a.txt").is_none());
    }
}
