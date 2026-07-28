use std::sync::Mutex;

use crate::{Clipboard, PlatformError};

pub(super) struct ArboardClipboard(Mutex<arboard::Clipboard>);

impl ArboardClipboard {
    pub(super) fn new() -> Result<Self, PlatformError> {
        arboard::Clipboard::new()
            .map(|clipboard| ArboardClipboard(Mutex::new(clipboard)))
            .map_err(|e| PlatformError::Clipboard(e.to_string()))
    }
}

impl Clipboard for ArboardClipboard {
    fn read_text(&self) -> Result<String, PlatformError> {
        self.0
            .lock()
            .unwrap()
            .get_text()
            .map_err(|e| PlatformError::Clipboard(e.to_string()))
    }

    fn write_text(&self, text: &str) -> Result<(), PlatformError> {
        self.0
            .lock()
            .unwrap()
            .set_text(text.to_string())
            .map_err(|e| PlatformError::Clipboard(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_text_through_the_real_system_clipboard() {
        let clipboard = match ArboardClipboard::new() {
            Ok(clipboard) => clipboard,
            // No clipboard service available in this environment (e.g. a
            // headless CI runner with no display server) — not this crate's
            // bug to report, so skip rather than fail.
            Err(_) => return,
        };
        let payload = "lux-platform clipboard round-trip \u{1F980}";
        if clipboard.write_text(payload).is_err() {
            return;
        }
        assert_eq!(clipboard.read_text().unwrap(), payload);
    }
}
