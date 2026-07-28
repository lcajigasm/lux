//! The error type shared by every trait in this crate.

use std::fmt;

#[derive(Debug)]
pub enum PlatformError {
    /// The system clipboard could not be read or written.
    Clipboard(String),
    /// A window or surface could not be created.
    Window(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlatformError::Clipboard(msg) => write!(f, "clipboard error: {msg}"),
            PlatformError::Window(msg) => write!(f, "window error: {msg}"),
        }
    }
}

impl std::error::Error for PlatformError {}
