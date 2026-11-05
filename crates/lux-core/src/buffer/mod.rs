//! The text buffer: a rope-backed in-memory document with atomic save,
//! encoding/line-ending/indent detection, and the raw insert/delete
//! primitives that [`crate::edit::Edit`] is built on. Higher-level editing
//! commands (delete word, duplicate line, ...) are out of scope here — they
//! compose these two primitives; this module only guarantees they're
//! correct and byte/char-index safe.

mod encoding;
mod eol;
mod indent;
mod lines;
mod save;

pub use encoding::{DecodeError, Encoding};
pub use eol::Eol;
pub use indent::{Indent, IndentStyle};

use crate::binary_detect;
use crate::huge_file::{self, HugeFilePolicy};
use ropey::Rope;
use std::fs;
use std::io;
use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Buffer {
    rope: Rope,
    eol: Eol,
    indent: Indent,
    /// `detect` saw a line with leading whitespace. Settings do not override it.
    indent_from_content: bool,
    /// The user or a project chose this indent. Settings do not override it.
    indent_explicit: bool,
    encoding: Encoding,
    huge_file_mode: bool,
}

impl Buffer {
    /// Decodes `bytes` (detecting encoding, refusing binary content) and
    /// detects its line-ending and indent style.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (text, encoding) = encoding::decode(bytes)?;
        Ok(Self::from_decoded(text, encoding))
    }

    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let bytes = fs::read(path)?;
        Self::from_owned_bytes(bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Same decode/detect logic as [`Self::from_bytes`], but takes
    /// ownership of `bytes` so [`encoding::decode_owned`] can skip a copy
    /// on the common plain-UTF-8 path — see that function's doc comment.
    /// `from_bytes` keeps its borrowing `&[u8]` signature, used by the
    /// hundreds of existing call sites across this crate that construct a
    /// `Buffer` from a byte-string literal; only `open`, the path the "open
    /// a 100 MB file" RSS budget actually measures, needs the owned
    /// fast path.
    fn from_owned_bytes(bytes: Vec<u8>) -> Result<Self, DecodeError> {
        let (text, encoding) = encoding::decode_owned(bytes)?;
        Ok(Self::from_decoded(text, encoding))
    }

    fn from_decoded(text: String, encoding: Encoding) -> Self {
        let indent_from_content = Indent::has_sample(&text);
        Buffer {
            rope: Rope::from_str(&text),
            eol: Eol::detect(&text),
            indent: Indent::detect(&text),
            indent_from_content,
            indent_explicit: false,
            encoding,
            huge_file_mode: false,
        }
    }

    /// Opens `path` the same way `open` does, but first checks — without
    /// reading the file's full content — whether it should be refused as
    /// binary or opened in huge-file mode (Phase 4 deliverable 8):
    ///
    /// 1. [`crate::binary_detect::looks_binary_path`] reads only a small
    ///    prefix; a binary file is rejected right there, the same
    ///    `DecodeError::Binary`/`InvalidData` error `open` would
    ///    eventually produce, but without first reading the rest of a
    ///    possibly huge binary file just to find out.
    /// 2. [`crate::huge_file::classify_path`] reads only the file's
    ///    metadata. A file at or above `policy.threshold_bytes` is still
    ///    opened (there's no partial/virtualized load in this rope-backed
    ///    `Buffer`), but [`Buffer::is_huge_file_mode`] comes back `true` —
    ///    the real, queryable signal a caller uses to open it read-only
    ///    and skip syntax highlighting. Enforcing "read-only" against
    ///    `insert`/`delete` themselves is deliberately not done here: both
    ///    are `Edit`/`Transaction`/`History`'s invertibility primitives,
    ///    and a silent no-op would desync a captured `Edit::Delete`'s text
    ///    from what actually happened to the rope — a real correctness
    ///    hazard in code this deliverable has no reason to touch. The flag
    ///    is real and correctly tracked; refusing edit commands against it
    ///    is the future editor-session layer's job, once one exists.
    pub fn open_with_policy(path: impl AsRef<Path>, policy: HugeFilePolicy) -> io::Result<Self> {
        let path = path.as_ref();
        if binary_detect::looks_binary_path(path)? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                DecodeError::Binary,
            ));
        }
        let huge = huge_file::classify_path(path, policy)?.is_huge();
        let mut buffer = Self::open(path)?;
        buffer.huge_file_mode = huge;
        Ok(buffer)
    }

    /// `true` if this buffer was opened via `open_with_policy` against a
    /// file at or above the policy's threshold. See `open_with_policy`'s
    /// doc comment for exactly what this does and doesn't enforce today.
    pub fn is_huge_file_mode(&self) -> bool {
        self.huge_file_mode
    }

    pub fn text(&self) -> String {
        self.rope.to_string()
    }

    pub fn eol(&self) -> Eol {
        self.eol
    }

    pub fn set_eol(&mut self, eol: Eol) {
        self.eol = eol;
    }

    pub fn indent(&self) -> Indent {
        self.indent
    }

    pub fn set_indent(&mut self, indent: Indent) {
        self.indent = indent;
        self.indent_explicit = true;
    }

    /// Changes the width without changing the style or locking the choice.
    pub fn set_indent_width(&mut self, width: u8) {
        self.indent.width = width.max(1);
    }

    /// Applies a settings fallback. A detected file and [`set_indent`] win.
    pub fn set_fallback_indent(&mut self, indent: Indent) {
        self.indent = indent;
    }

    pub fn indent_from_content(&self) -> bool {
        self.indent_from_content
    }

    pub fn indent_is_explicit(&self) -> bool {
        self.indent_explicit
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }

    pub fn set_encoding(&mut self, encoding: Encoding) {
        self.encoding = encoding;
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// The text in `char_range`.
    pub fn text_range(&self, char_range: Range<usize>) -> String {
        self.rope.slice(char_range).to_string()
    }

    /// The character at `char_idx`, if within bounds.
    #[must_use]
    pub fn char_at(&self, char_idx: usize) -> Option<char> {
        if char_idx < self.rope.len_chars() {
            Some(self.rope.char(char_idx))
        } else {
            None
        }
    }

    /// Inserts `text` at `char_idx`. The fundamental invertible primitive
    /// `Edit`/`Transaction`/`History` are built on — not itself a
    /// user-facing editing command.
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        self.rope.insert(char_idx, text);
    }

    /// Removes the text in `char_range` and returns it, so the caller can
    /// build the edit's inverse without a second read of the buffer.
    pub fn delete(&mut self, char_range: Range<usize>) -> String {
        let removed = self.text_range(char_range.clone());
        self.rope.remove(char_range);
        removed
    }

    /// Total number of bytes in the buffer, in O(1) time.
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Converts a char offset to a byte offset, in O(log N) time.
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx.min(self.rope.len_chars()))
    }

    /// Converts a byte offset to a char offset, in O(log N) time.
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_char(byte_idx.min(self.rope.len_bytes()))
    }

    /// Converts a byte offset to a line index, in O(log N) time.
    pub fn byte_to_line(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_line(byte_idx.min(self.rope.len_bytes()))
    }

    /// Converts a line index to its starting byte offset, in O(log N) time.
    pub fn line_to_byte(&self, line_idx: usize) -> usize {
        self.rope.line_to_byte(line_idx.min(self.rope.len_lines()))
    }

    /// Reference to the underlying rope for zero-copy chunk access and
    /// O(1) Arc cloning across worker thread boundaries.
    pub fn rope(&self) -> &Rope {
        &self.rope
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lux-core-buffer-test-{:?}",
            std::thread::current().id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn insert_places_text_at_the_given_char_index() {
        let mut buffer = Buffer::from_bytes(b"helloworld").unwrap();
        buffer.insert(5, " ");
        assert_eq!(buffer.text(), "hello world");
        assert_eq!(buffer.len_chars(), 11);
    }

    #[test]
    fn delete_removes_and_returns_the_range() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let removed = buffer.delete(5..11);
        assert_eq!(removed, " world");
        assert_eq!(buffer.text(), "hello");
    }

    #[test]
    fn text_range_reads_without_mutating() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        assert_eq!(buffer.text_range(6..11), "world");
        assert_eq!(buffer.text(), "hello world");
    }

    #[test]
    fn loads_from_bytes_and_detects_eol_and_indent() {
        let buffer = Buffer::from_bytes(b"fn main() {\r\n    let x = 1;\r\n}\r\n").unwrap();
        assert_eq!(buffer.text(), "fn main() {\r\n    let x = 1;\r\n}\r\n");
        assert_eq!(buffer.eol(), Eol::Crlf);
        assert_eq!(buffer.indent().style, IndentStyle::Spaces);
        assert_eq!(buffer.indent().width, 4);
        assert_eq!(buffer.encoding(), Encoding::Utf8);
        assert_eq!(buffer.len_lines(), 4);
    }

    #[test]
    fn refuses_binary_content() {
        let bytes = [0x00, 0xFF, 0x01, 0x02];
        assert_eq!(Buffer::from_bytes(&bytes).unwrap_err(), DecodeError::Binary);
    }

    #[test]
    fn open_reads_a_real_file() {
        let dir = temp_dir();
        let path = dir.join("open.txt");
        fs::write(&path, "hello\nworld\n").unwrap();

        let buffer = Buffer::open(&path).unwrap();
        assert_eq!(buffer.text(), "hello\nworld\n");
        assert_eq!(buffer.eol(), Eol::Lf);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn open_rejects_binary_file_as_invalid_data() {
        let dir = temp_dir();
        let path = dir.join("binary.bin");
        fs::write(&path, [0x00, 0x01, 0x02]).unwrap();

        let err = Buffer::open(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn open_with_policy_marks_a_file_at_or_above_the_threshold_as_huge() {
        let dir = temp_dir();
        let path = dir.join("huge.txt");
        fs::write(&path, vec![b'a'; 2000]).unwrap();

        let buffer = Buffer::open_with_policy(&path, HugeFilePolicy::new(1000)).unwrap();
        assert!(buffer.is_huge_file_mode());
        assert_eq!(buffer.len_chars(), 2000);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn open_with_policy_leaves_a_small_file_out_of_huge_file_mode() {
        let dir = temp_dir();
        let path = dir.join("small.txt");
        fs::write(&path, "hello").unwrap();

        let buffer = Buffer::open_with_policy(&path, HugeFilePolicy::default()).unwrap();
        assert!(!buffer.is_huge_file_mode());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn open_with_policy_rejects_a_binary_file_without_reading_it_fully() {
        let dir = temp_dir();
        let path = dir.join("binary.bin");
        // Larger than the huge-file threshold used here, so a correct
        // implementation must catch the binary content via the cheap
        // prefix check rather than accidentally succeeding because the
        // huge-file path skips the binary check.
        let mut content = vec![b'a'; 2000];
        content[10] = 0x00;
        fs::write(&path, &content).unwrap();

        let err = Buffer::open_with_policy(&path, HugeFilePolicy::new(1000)).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn open_with_policy_threshold_is_configurable() {
        let dir = temp_dir();
        let path = dir.join("medium.txt");
        fs::write(&path, vec![b'a'; 500]).unwrap();

        let lenient = Buffer::open_with_policy(&path, HugeFilePolicy::new(1_000_000)).unwrap();
        assert!(!lenient.is_huge_file_mode());

        let strict = Buffer::open_with_policy(&path, HugeFilePolicy::new(10)).unwrap();
        assert!(strict.is_huge_file_mode());

        fs::remove_file(&path).ok();
    }

    #[test]
    fn a_freshly_opened_buffer_is_not_in_huge_file_mode() {
        let buffer = Buffer::from_bytes(b"hello").unwrap();
        assert!(!buffer.is_huge_file_mode());
    }

    #[test]
    fn save_writes_content_and_round_trips_through_open() {
        let dir = temp_dir();
        let path = dir.join("save.txt");
        fs::remove_file(&path).ok();
        fs::remove_file(format!("{}.bak", path.display())).ok();

        let buffer = Buffer::from_bytes(b"one\ntwo\nthree\n").unwrap();
        buffer.save(&path).unwrap();

        let reopened = Buffer::open(&path).unwrap();
        assert_eq!(reopened.text(), "one\ntwo\nthree\n");

        fs::remove_file(&path).ok();
    }

    #[test]
    fn save_backs_up_previous_content_before_overwriting() {
        let dir = temp_dir();
        let path = dir.join("backup.txt");
        let backup_path = dir.join("backup.txt.bak");
        fs::remove_file(&path).ok();
        fs::remove_file(&backup_path).ok();

        fs::write(&path, "old content\n").unwrap();

        let buffer = Buffer::from_bytes(b"new content\n").unwrap();
        buffer.save(&path).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "new content\n");
        assert_eq!(fs::read_to_string(&backup_path).unwrap(), "old content\n");

        fs::remove_file(&path).ok();
        fs::remove_file(&backup_path).ok();
    }

    #[test]
    fn save_preserves_original_encoding() {
        let dir = temp_dir();
        let path = dir.join("utf16.txt");
        fs::remove_file(&path).ok();

        let original_bytes = encoding::encode("hola señor 😀\n", Encoding::Utf16Le);
        let buffer = Buffer::from_bytes(&original_bytes).unwrap();
        assert_eq!(buffer.encoding(), Encoding::Utf16Le);

        buffer.save(&path).unwrap();
        let saved_bytes = fs::read(&path).unwrap();
        assert_eq!(saved_bytes, original_bytes);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn buffer_setters_mutate_properties() {
        let mut buffer = Buffer::from_bytes(b"hello\n").unwrap();
        assert_eq!(buffer.eol(), Eol::Lf);
        buffer.set_eol(Eol::Crlf);
        assert_eq!(buffer.eol(), Eol::Crlf);

        buffer.set_indent(Indent {
            style: IndentStyle::Tabs,
            width: 1,
        });
        assert_eq!(buffer.indent().style, IndentStyle::Tabs);

        buffer.set_encoding(Encoding::Latin1);
        assert_eq!(buffer.encoding(), Encoding::Latin1);
    }
}
