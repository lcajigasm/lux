//! A compact, versioned binary encoding of a [`History`], so undo and redo
//! survive a restart (hot exit).
//!
//! The encoding is bound to the exact text it was recorded against: the
//! header carries a hash and the char length of the buffer. [`decode`]
//! refuses a history whose buffer has changed since — a file edited on
//! disk while the editor was closed — instead of replaying edits at
//! offsets that no longer mean anything. A trailing checksum rejects a
//! damaged file.
//!
//! Layout (integers are LEB128 varints unless noted):
//!
//! ```text
//! "LUXH" | version: u8 | content_hash: u64 LE | content_chars
//! | state | next_state | undo_count | redo_count | entries… | checksum: u64 LE
//! entry = before | after | edit_count | edits… | sels_before | sels_after
//! edit  = kind: u8 (0 insert, 1 delete) | char_idx | byte_len | utf-8 bytes
//! sels  = count | (anchor, head)…   position = byte | char | line | column
//! ```
//!
//! Size is bounded: [`encode`] drops the oldest undo steps (then the
//! furthest redo steps) until the result fits the caller's budget. The
//! remaining steps still undo exactly; only the oldest ones are lost.

use super::{History, HistoryEntry, Revision, Transaction};
use crate::buffer::Buffer;
use crate::edit::Edit;
use crate::position::Position;
use crate::selection::Selection;

const MAGIC: &[u8; 4] = b"LUXH";
/// Bump when the layout changes. [`decode`] rejects any other version.
pub const FORMAT_VERSION: u8 = 1;
/// Per-buffer ceiling used by session persistence: 4 MiB.
pub const DEFAULT_BUDGET: usize = 4 << 20;

/// FNV-1a over the buffer's UTF-8 bytes. Stable across runs and Rust
/// versions, unlike `DefaultHasher`.
pub fn content_hash(buffer: &Buffer) -> u64 {
    let mut hash = Fnv::new();
    for chunk in buffer.rope().chunks() {
        hash.write(chunk.as_bytes());
    }
    hash.finish()
}

/// Encodes `history`, recorded against `buffer`'s current text, within
/// `budget` bytes. Returns `None` only when not even an empty history fits.
pub fn encode(history: &History, buffer: &Buffer, budget: usize) -> Option<Vec<u8>> {
    let undo: Vec<Vec<u8>> = history.undo_stack.iter().map(encode_entry).collect();
    let redo: Vec<Vec<u8>> = history.redo_stack.iter().map(encode_entry).collect();
    let mut header = Vec::with_capacity(64);
    header.extend_from_slice(MAGIC);
    header.push(FORMAT_VERSION);
    header.extend_from_slice(&content_hash(buffer).to_le_bytes());
    put(&mut header, buffer.len_chars() as u64);
    put(&mut header, history.state);
    put(&mut header, history.next_state);

    // Oldest undo steps sit at the front of `undo`; the furthest redo
    // steps at the front of `redo`. Drop from those ends first.
    let fixed = header.len() + 2 * 10 + 8;
    let mut total: usize = fixed + undo.iter().chain(&redo).map(Vec::len).sum::<usize>();
    let (mut undo_from, mut redo_from) = (0, 0);
    while total > budget {
        if undo_from < undo.len() {
            total -= undo[undo_from].len();
            undo_from += 1;
        } else if redo_from < redo.len() {
            total -= redo[redo_from].len();
            redo_from += 1;
        } else {
            return None;
        }
    }

    let mut out = header;
    put(&mut out, (undo.len() - undo_from) as u64);
    put(&mut out, (redo.len() - redo_from) as u64);
    for entry in undo[undo_from..].iter().chain(&redo[redo_from..]) {
        out.extend_from_slice(entry);
    }
    let checksum = Fnv::of(&out);
    out.extend_from_slice(&checksum.to_le_bytes());
    Some(out)
}

/// Rebuilds a history encoded by [`encode`], or `None` when the bytes are
/// damaged, from another format version, or were recorded against text
/// other than `buffer`'s. The result never coalesces with the next edit:
/// a restart is an undo boundary.
pub fn decode(bytes: &[u8], buffer: &Buffer) -> Option<History> {
    let (body, checksum) = bytes.split_at_checked(bytes.len().checked_sub(8)?)?;
    if Fnv::of(body).to_le_bytes() != checksum {
        return None;
    }
    let mut r = Reader { bytes: body };
    if r.take(4)? != MAGIC || r.byte()? != FORMAT_VERSION {
        return None;
    }
    let hash = u64::from_le_bytes(r.take(8)?.try_into().ok()?);
    if r.var()? != buffer.len_chars() as u64 || hash != content_hash(buffer) {
        return None;
    }
    let state = r.var()?;
    let next_state = r.var()?;
    let undo_count = r.count()?;
    let redo_count = r.count()?;
    let mut undo_stack = Vec::with_capacity(undo_count);
    for _ in 0..undo_count {
        undo_stack.push(r.entry()?);
    }
    let mut redo_stack = Vec::with_capacity(redo_count);
    for _ in 0..redo_count {
        redo_stack.push(r.entry()?);
    }
    let consistent = undo_stack.last().is_none_or(|e| e.after == state)
        && redo_stack.last().is_none_or(|e| e.before == state)
        && next_state >= state;
    if !r.bytes.is_empty() || !consistent {
        return None;
    }
    Some(History {
        undo_stack,
        redo_stack,
        state,
        next_state,
        last_chained_at: None,
        revision: Revision::fresh(),
    })
}

fn encode_entry(entry: &HistoryEntry) -> Vec<u8> {
    let tx = &entry.transaction;
    let mut out = Vec::new();
    put(&mut out, entry.before);
    put(&mut out, entry.after);
    put(&mut out, tx.edits.len() as u64);
    for edit in &tx.edits {
        let (kind, idx, text) = match edit {
            Edit::Insert { char_idx, text } => (0u8, char_idx, text),
            Edit::Delete { char_idx, text } => (1u8, char_idx, text),
        };
        out.push(kind);
        put(&mut out, *idx as u64);
        put(&mut out, text.len() as u64);
        out.extend_from_slice(text.as_bytes());
    }
    for sels in [&tx.selections_before, &tx.selections_after] {
        put(&mut out, sels.len() as u64);
        for sel in sels {
            for pos in [sel.anchor, sel.head] {
                for v in [pos.byte, pos.char, pos.line, pos.column] {
                    put(&mut out, v as u64);
                }
            }
        }
    }
    out
}

fn put(out: &mut Vec<u8>, mut v: u64) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, rest) = self.bytes.split_at_checked(n)?;
        self.bytes = rest;
        Some(head)
    }

    fn byte(&mut self) -> Option<u8> {
        Some(self.take(1)?[0])
    }

    fn var(&mut self) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            v |= u64::from(b & 0x7f).checked_shl(shift)?;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }

    fn usize(&mut self) -> Option<usize> {
        usize::try_from(self.var()?).ok()
    }

    /// A length that cannot exceed the bytes left — a damaged count must
    /// not reserve gigabytes before the read fails.
    fn count(&mut self) -> Option<usize> {
        self.usize().filter(|&n| n <= self.bytes.len())
    }

    fn position(&mut self) -> Option<Position> {
        Some(Position {
            byte: self.usize()?,
            char: self.usize()?,
            line: self.usize()?,
            column: self.usize()?,
        })
    }

    fn selections(&mut self) -> Option<Vec<Selection>> {
        let n = self.count()?;
        let mut sels = Vec::with_capacity(n);
        for _ in 0..n {
            let anchor = self.position()?;
            sels.push(Selection {
                anchor,
                head: self.position()?,
            });
        }
        Some(sels)
    }

    fn entry(&mut self) -> Option<HistoryEntry> {
        let before = self.var()?;
        let after = self.var()?;
        let n = self.count()?;
        let mut edits = Vec::with_capacity(n);
        for _ in 0..n {
            let kind = self.byte()?;
            let char_idx = self.usize()?;
            let len = self.count()?;
            let text = std::str::from_utf8(self.take(len)?).ok()?.to_owned();
            edits.push(match kind {
                0 => Edit::Insert { char_idx, text },
                1 => Edit::Delete { char_idx, text },
                _ => return None,
            });
        }
        let selections_before = self.selections()?;
        let selections_after = self.selections()?;
        Some(HistoryEntry {
            transaction: Transaction::new(edits, selections_before, selections_after),
            before,
            after,
        })
    }
}

struct Fnv(u64);

impl Fnv {
    fn new() -> Fnv {
        Fnv(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }

    fn of(bytes: &[u8]) -> u64 {
        let mut f = Fnv::new();
        f.write(bytes);
        f.finish()
    }
}

#[cfg(test)]
mod tests;
