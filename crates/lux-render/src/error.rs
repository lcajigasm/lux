//! The error type shared by every fallible operation in this crate.

use std::fmt;

#[derive(Debug)]
pub enum RenderError {
    /// No GPU adapter satisfying our requirements was found.
    Adapter(String),
    /// The logical device/queue could not be created from the chosen adapter.
    Device(String),
    /// A `wgpu::Surface` could not be created for the given window.
    Surface(String),
    /// The surface reported no texture format we can render into.
    NoCompatibleSurfaceFormat,
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Adapter(msg) => write!(f, "no compatible GPU adapter: {msg}"),
            RenderError::Device(msg) => write!(f, "failed to request a GPU device: {msg}"),
            RenderError::Surface(msg) => write!(f, "failed to create a surface: {msg}"),
            RenderError::NoCompatibleSurfaceFormat => {
                write!(f, "the surface reported no compatible texture format")
            }
        }
    }
}

impl std::error::Error for RenderError {}
