//! macOS: dialogs as sheets attached to the editor window.
//!
//! `rfd`'s blocking calls on macOS run `-[NSSavePanel runModal]` (or
//! `-[NSAlert runModal]`) on the main thread, a nested modal run loop that
//! holds the main thread until the panel closes. The async variants with a
//! parent window instead call `beginSheetModalForWindow:completionHandler:`
//! on the main thread and return at once, so winit's `[NSApp run]` keeps
//! dispatching events, timers and redraws while the sheet is up. The worker
//! that asked for the dialog parks until the completion handler wakes it.
//!
//! Nothing here runs on the main thread: a sheet requested from the main
//! thread would park the thread its completion handler needs. Those calls
//! return `None` and the caller falls back to the blocking dialog.

use std::future::Future;
use std::path::PathBuf;
use std::pin::pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};
use std::thread::Thread;

use crate::dialog::{FileDialogOptions, SaveDecision};

unsafe extern "C" {
    fn pthread_main_np() -> i32;
}

fn on_main_thread() -> bool {
    // SAFETY: no arguments, no preconditions; returns 1 on the main thread.
    unsafe { pthread_main_np() == 1 }
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

/// Window-attached panel and alert templates. Empty when there is no
/// window to attach to.
#[derive(Default)]
pub(super) struct Sheets {
    files: Mutex<Option<rfd::AsyncFileDialog>>,
    alert: Mutex<Option<rfd::AsyncMessageDialog>>,
}

impl Sheets {
    pub(super) fn with_parent(window: &winit::window::Window) -> Self {
        Self {
            files: Mutex::new(Some(rfd::AsyncFileDialog::new().set_parent(window))),
            alert: Mutex::new(Some(rfd::AsyncMessageDialog::new().set_parent(window))),
        }
    }

    fn files(&self, options: &FileDialogOptions) -> Option<rfd::AsyncFileDialog> {
        if on_main_thread() {
            return None;
        }
        let mut dialog = self
            .files
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()?;
        if let Some(title) = &options.title {
            dialog = dialog.set_title(title.as_str());
        }
        if let Some(dir) = &options.starting_directory {
            dialog = dialog.set_directory(dir);
        }
        for (name, extensions) in &options.filters {
            dialog = dialog.add_filter(name.as_str(), extensions);
        }
        Some(dialog)
    }

    fn alert(&self) -> Option<rfd::AsyncMessageDialog> {
        if on_main_thread() {
            return None;
        }
        self.alert.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub(super) fn pick_file(&self, options: &FileDialogOptions) -> Option<Option<PathBuf>> {
        let dialog = self.files(options)?;
        Some(block_on(dialog.pick_file()).map(|file| file.path().to_path_buf()))
    }

    pub(super) fn pick_files(&self, options: &FileDialogOptions) -> Option<Vec<PathBuf>> {
        let dialog = self.files(options)?;
        let picked = block_on(dialog.pick_files()).unwrap_or_default();
        Some(picked.iter().map(|f| f.path().to_path_buf()).collect())
    }

    pub(super) fn pick_folder(&self, options: &FileDialogOptions) -> Option<Option<PathBuf>> {
        let dialog = self.files(options)?;
        Some(block_on(dialog.pick_folder()).map(|file| file.path().to_path_buf()))
    }

    pub(super) fn save_file(&self, options: &FileDialogOptions) -> Option<Option<PathBuf>> {
        let dialog = self.files(options)?;
        Some(block_on(dialog.save_file()).map(|file| file.path().to_path_buf()))
    }

    pub(super) fn confirm_save(&self, document: &str) -> Option<SaveDecision> {
        let dialog = self
            .alert()?
            .set_title("Unsaved changes")
            .set_description(format!("Save changes to {document} before continuing?"))
            .set_buttons(rfd::MessageButtons::YesNoCancel);
        Some(match block_on(dialog.show()) {
            rfd::MessageDialogResult::Yes => SaveDecision::Save,
            rfd::MessageDialogResult::No => SaveDecision::Discard,
            _ => SaveDecision::Cancel,
        })
    }

    pub(super) fn confirm_trash(&self, item: &str) -> Option<bool> {
        let dialog = self
            .alert()?
            .set_level(rfd::MessageLevel::Warning)
            .set_title("Move to Trash")
            .set_description(format!("Move \"{item}\" to the Trash?"))
            .set_buttons(rfd::MessageButtons::OkCancel);
        Some(matches!(
            block_on(dialog.show()),
            rfd::MessageDialogResult::Ok | rfd::MessageDialogResult::Yes
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
