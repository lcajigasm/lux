//! The `winit` 0.30.13-backed implementation of the platform trait, for
//! macOS, Windows, and Linux (ADR 0004). UIKit (Phase 12) and Android/JNI
//! (Phase 13) backends live in sibling modules once they exist; nothing
//! above `lux-platform` should reach into this module directly.

mod app;
mod clipboard;
mod context;
mod convert;
mod dialog;
#[cfg(target_os = "macos")]
mod dialog_sheet;
#[cfg(target_os = "macos")]
mod macos_title_bar;
mod run;
mod window;

pub use run::run;
