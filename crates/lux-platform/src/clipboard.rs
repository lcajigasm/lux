//! The system clipboard, text only — Lux does not read or write rich
//! content (images, HTML) anywhere in the editor today, so the trait does
//! not model it.

use crate::PlatformError;

pub trait Clipboard {
    fn read_text(&self) -> Result<String, PlatformError>;
    fn write_text(&self, text: &str) -> Result<(), PlatformError>;
}
