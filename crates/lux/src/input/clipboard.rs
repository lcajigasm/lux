//! Copy, cut and paste with per-caret pieces, as Sublime Text does them.
//!
//! The system clipboard only carries one string. Copying N selections
//! writes them joined by the document's line ending, and also keeps the N
//! pieces here together with a fingerprint of the string they produced. A
//! later paste with N carets distributes piece i to caret i, but only while
//! the system clipboard still holds exactly that string; anything copied
//! elsewhere in the meantime is pasted as plain text.

use std::hash::{DefaultHasher, Hash, Hasher};

use lux_core::edit_ops;
use lux_platform::Clipboard;

use crate::app::App;

/// The pieces of the last multi-selection copy made by this editor.
#[derive(Default)]
pub(crate) struct ClipboardMemory {
    pieces: Vec<String>,
    fingerprint: Option<(u64, usize)>,
}

fn fingerprint(text: &str) -> (u64, usize) {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    (hasher.finish(), text.len())
}

impl ClipboardMemory {
    fn remember(&mut self, pieces: Vec<String>, written: &str) {
        self.fingerprint = Some(fingerprint(written));
        self.pieces = pieces;
    }

    /// The remembered pieces, if `text` is still the string they produced.
    fn pieces_for(&self, text: &str) -> Option<&[String]> {
        (self.fingerprint == Some(fingerprint(text))).then_some(self.pieces.as_slice())
    }
}

/// How a paste spreads `text` over `carets` carets: one piece per caret in
/// document order, or `None` for the whole text at every caret.
fn distribution(memory: &ClipboardMemory, text: &str, carets: usize) -> Option<Vec<String>> {
    if carets < 2 {
        return None;
    }
    if let Some(pieces) = memory.pieces_for(text) {
        return (pieces.len() == carets).then(|| pieces.to_vec());
    }
    // Text copied outside Lux: one line per caret when the counts agree.
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_owned())
        .collect();
    (lines.len() == carets).then_some(lines)
}

impl App {
    /// Copies every non-empty selection in document order. Several
    /// selections are joined with the document's own line ending, and their
    /// pieces are remembered for a per-caret paste.
    pub(crate) fn copy_to_clipboard(&mut self, clipboard: Option<&dyn Clipboard>) -> bool {
        let Some(clipboard) = clipboard else {
            return false;
        };
        let pieces: Vec<String> = self
            .cursors
            .cursors()
            .iter()
            .filter(|selection| !selection.is_empty())
            .map(|selection| self.buffer.text_range(selection.char_range()))
            .collect();
        if pieces.is_empty() {
            return false;
        }
        let text = pieces.join(self.buffer.eol().as_str());
        match clipboard.write_text(&text) {
            Ok(()) => {
                self.carets.clipboard.remember(pieces, &text);
                true
            }
            Err(error) => {
                self.documents.message = Some(error.to_string());
                false
            }
        }
    }

    /// Copies the selections, then deletes them as one undo step.
    pub(crate) fn cut_to_clipboard(&mut self, clipboard: Option<&dyn Clipboard>) {
        if self.editing_blocked() || self.buffer.is_huge_file_mode() {
            return;
        }
        if self.copy_to_clipboard(clipboard) {
            self.replace_selections(self.cursors.clone(), "");
        }
    }

    /// Reads the system clipboard and pastes it at every caret.
    pub(crate) fn paste_from_clipboard(&mut self, clipboard: Option<&dyn Clipboard>) {
        if self.editing_blocked() || self.buffer.is_huge_file_mode() {
            return;
        }
        let Some(clipboard) = clipboard else {
            return;
        };
        match clipboard.read_text() {
            Ok(text) => self.paste_text(&text),
            Err(error) => self.documents.message = Some(error.to_string()),
        }
    }

    /// Pastes `text`: piece i at caret i when the carets match the pieces
    /// of the copy that produced `text` (or its lines, for foreign text),
    /// otherwise the whole text at every caret.
    pub(crate) fn paste_text(&mut self, text: &str) {
        if text.is_empty() || self.editing_blocked() || self.buffer.is_huge_file_mode() {
            return;
        }
        let Some(pieces) = distribution(&self.carets.clipboard, text, self.cursors.count()) else {
            self.insert_at_cursor(text);
            return;
        };
        self.snippet_session = None;
        // `apply_to_each` visits carets back to front, so count down to
        // hand out the pieces in document order.
        let mut index = pieces.len();
        self.apply_edit_operation(self.cursors.clone(), |buffer, selection| {
            index = index.saturating_sub(1);
            edit_ops::insert_text(buffer, selection, &pieces[index])
        });
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn remembered_pieces_only_apply_to_the_text_they_produced() {
        let mut memory = ClipboardMemory::default();
        memory.remember(vec!["a\nb".into(), "c".into()], "a\nb\nc");
        assert_eq!(
            distribution(&memory, "a\nb\nc", 2),
            Some(vec!["a\nb".into(), "c".into()])
        );
        // Three carets, two pieces: the whole text goes everywhere, even
        // though the text happens to have three lines.
        assert_eq!(distribution(&memory, "a\nb\nc", 3), None);
        // Foreign text falls back to its lines.
        assert_eq!(
            distribution(&memory, "x\r\ny", 2),
            Some(vec!["x".into(), "y".into()])
        );
        assert_eq!(distribution(&memory, "a\nb\nc", 1), None);
    }
}
