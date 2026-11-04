//! Cross-platform window/input/IME/clipboard trait, with a winit-backed
//! desktop implementation (ADR 0004).
//!
//! `lux-ui` and every crate above it must program against the traits in this
//! module, never against `winit` directly. That boundary is what lets Phase
//! 12/13 add hand-written UIKit and Android/JNI backends later without
//! touching a single line above this crate.

pub mod android;
pub mod capability;
pub mod clipboard;
pub mod desktop;
pub mod dialog;
pub mod error;
pub mod event;
pub mod ime;
pub mod uikit;
pub mod window;

pub use capability::{child_processes_allowed, pty_and_lsp_ui_shipped};

pub use clipboard::Clipboard;
pub use dialog::{FileDialogOptions, FileDialogs};
pub use error::PlatformError;
pub use event::{
    ColorScheme, ElementState, Ime, Key, KeyEvent, Modifiers, MouseButton, NamedKey, PhysicalKey,
    PlatformEvent, ScrollDelta, Touch, TouchPhase,
};
pub use ime::{ImeComposition, ImeUpdate};
pub use window::{PlatformWindow, SafeAreaInsets, WindowAttributes, WindowId};

/// The callbacks a platform backend drives. Implemented by whatever owns the
/// application's state (today, that will be `lux-ui`'s driver); the backend
/// (desktop/winit today, UIKit/Android later) calls these, never the other
/// way around.
///
/// `resumed`/`suspended` are split from `surfaces_available`/`surfaces_lost`
/// on purpose, matching the shape `winit` 0.31 is moving to: on desktop a
/// resumed app always has a surface, but on Android a resumed `Activity` can
/// exist before its `SurfaceView` is ready, and the reverse can happen on
/// backgrounding. The desktop backend calls both pairs back-to-back today;
/// a future Android backend is what actually needs the gap between them.
pub trait PlatformEventHandler {
    /// A worker has queued results. Drain the application's result channel.
    fn background_event(&mut self, _ctx: &dyn PlatformContext) {}
    /// The application became active and may now create windows.
    fn resumed(&mut self, ctx: &dyn PlatformContext);

    /// Surfaces (and therefore GPU rendering) can be created. Defaults to
    /// doing nothing, since most backends can render as soon as they resume.
    fn surfaces_available(&mut self, ctx: &dyn PlatformContext) {
        let _ = ctx;
    }

    /// Surfaces must be dropped; do not touch the GPU device until
    /// `surfaces_available` fires again.
    fn surfaces_lost(&mut self, ctx: &dyn PlatformContext) {
        let _ = ctx;
    }

    /// The application was backgrounded and must stop doing work.
    fn suspended(&mut self, ctx: &dyn PlatformContext) {
        let _ = ctx;
    }

    /// A window received an input or lifecycle event.
    fn window_event(&mut self, ctx: &dyn PlatformContext, window: WindowId, event: PlatformEvent);

    /// All queued events have been dispatched; a good place to do per-frame
    /// work that isn't tied to a specific event.
    fn about_to_wait(&mut self, ctx: &dyn PlatformContext) {
        let _ = ctx;
    }

    /// The application is shutting down.
    fn exiting(&mut self, ctx: &dyn PlatformContext) {
        let _ = ctx;
    }
}

/// What a `PlatformEventHandler` can do back to the platform: create
/// windows, reach the clipboard and file dialogs, and ask to exit.
pub trait PlatformContext {
    /// Thread-safe event-loop notification, usable after this callback ends.
    fn background_waker(&self) -> std::sync::Arc<dyn Fn() + Send + Sync>;

    /// Owned native dialog service for a worker while the main loop runs.
    fn background_dialogs(&self) -> std::sync::Arc<dyn FileDialogs>;
    /// Creates a new window with the given attributes.
    fn create_window(
        &self,
        attrs: WindowAttributes,
    ) -> Result<Box<dyn PlatformWindow>, PlatformError>;

    /// The system clipboard.
    fn clipboard(&self) -> &dyn Clipboard;

    /// Native file/folder picker dialogs.
    fn file_dialogs(&self) -> &dyn FileDialogs;

    /// Requests that the run loop exit after the current callback returns.
    fn exit(&self);

    /// Asks the platform to wake this application up at (or after) `at`
    /// even if no real input event arrives before then — what a scheduled
    /// repaint (e.g. `lux_render::schedule::RepaintScheduler::request_repaint_at`,
    /// whose own doc comment calls this out as "a later loop") needs to
    /// actually run: without it, an app that only repaints in response to
    /// dispatched events would never see a timer-only wakeup like the next
    /// cursor-blink transition. A lower bound on timing, not a guarantee of
    /// exact delivery, matching the platform's own event loop's contract.
    /// Callers must not treat this as a recurring poll — request the next
    /// wakeup once per actual timer need, never on a fixed period
    /// independent of real state (golden rule #5).
    fn request_wakeup(&self, at: std::time::Instant);
}
