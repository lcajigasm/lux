//! The smallest invertible primitive a `Transaction` is built from: insert
//! text at a char offset, or delete a run of text (recording exactly what
//! was removed, so the edit can invert itself without touching the buffer
//! again). Higher-level editing commands — delete word, duplicate line, and
//! so on — are out of scope here; they compose these two primitives. This
//! module only has to guarantee every edit it produces can be undone
//! exactly, which is what `Transaction`/`History` (in `crate::history`)
//! build on.

use crate::buffer::Buffer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    Insert { char_idx: usize, text: String },
    Delete { char_idx: usize, text: String },
}

impl Edit {
    pub fn insert(char_idx: usize, text: impl Into<String>) -> Edit {
        Edit::Insert {
            char_idx,
            text: text.into(),
        }
    }

    /// Deletes `len` chars starting at `char_idx`. Reads the text it's about
    /// to remove from `buffer` up front, so the resulting `Edit` carries
    /// everything it needs to invert itself later.
    pub fn delete(buffer: &Buffer, char_idx: usize, len: usize) -> Edit {
        Edit::Delete {
            char_idx,
            text: buffer.text_range(char_idx..char_idx + len),
        }
    }

    /// Applies this edit to `buffer`.
    pub fn apply(&self, buffer: &mut Buffer) {
        match self {
            Edit::Insert { char_idx, text } => buffer.insert(*char_idx, text),
            Edit::Delete { char_idx, text } => {
                buffer.delete(*char_idx..*char_idx + text.chars().count());
            }
        }
    }

    /// The edit that undoes this one: an insert's inverse is deleting back
    /// what it inserted; a delete's inverse is re-inserting what it removed.
    pub fn invert(&self) -> Edit {
        match self {
            Edit::Insert { char_idx, text } => Edit::Delete {
                char_idx: *char_idx,
                text: text.clone(),
            },
            Edit::Delete { char_idx, text } => Edit::Insert {
                char_idx: *char_idx,
                text: text.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_then_apply_places_the_text() {
        let mut buffer = Buffer::from_bytes(b"helloworld").unwrap();
        Edit::insert(5, " ").apply(&mut buffer);
        assert_eq!(buffer.text(), "hello world");
    }

    #[test]
    fn delete_captures_the_removed_text() {
        let buffer = Buffer::from_bytes(b"hello world").unwrap();
        let edit = Edit::delete(&buffer, 5, 6);
        assert_eq!(
            edit,
            Edit::Delete {
                char_idx: 5,
                text: " world".to_string()
            }
        );
    }

    #[test]
    fn delete_then_apply_removes_the_text() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let edit = Edit::delete(&buffer, 5, 6);
        edit.apply(&mut buffer);
        assert_eq!(buffer.text(), "hello");
    }

    #[test]
    fn invert_of_insert_deletes_it_back_out() {
        let mut buffer = Buffer::from_bytes(b"helloworld").unwrap();
        let edit = Edit::insert(5, " ");
        edit.apply(&mut buffer);
        edit.invert().apply(&mut buffer);
        assert_eq!(buffer.text(), "helloworld");
    }

    #[test]
    fn invert_of_delete_reinserts_it() {
        let mut buffer = Buffer::from_bytes(b"hello world").unwrap();
        let original = buffer.text();
        let edit = Edit::delete(&buffer, 5, 6);
        edit.apply(&mut buffer);
        edit.invert().apply(&mut buffer);
        assert_eq!(buffer.text(), original);
    }

    #[test]
    fn invert_round_trips_with_multibyte_text() {
        let mut buffer = Buffer::from_bytes("café ☕".as_bytes()).unwrap();
        let original = buffer.text();
        let edit = Edit::delete(&buffer, 3, 3); // "é ☕" -> removes 3 chars
        edit.apply(&mut buffer);
        edit.invert().apply(&mut buffer);
        assert_eq!(buffer.text(), original);
    }
}
