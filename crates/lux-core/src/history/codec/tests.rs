use super::*;

fn cursor(buffer: &Buffer, char_idx: usize) -> Selection {
    Selection::cursor(Position::from_char(buffer, char_idx))
}

fn type_at(history: &mut History, buffer: &mut Buffer, at: usize, text: &str) {
    let before = cursor(buffer, at);
    let edit = Edit::insert(at, text);
    edit.apply(buffer);
    let after = cursor(buffer, at + text.chars().count());
    history.record(Transaction::new(vec![edit], vec![before], vec![after]));
}

fn edited() -> (History, Buffer) {
    let mut buffer = Buffer::from_bytes(b"").unwrap();
    let mut history = History::new();
    type_at(&mut history, &mut buffer, 0, "fn main() {}\n");
    type_at(&mut history, &mut buffer, 11, "ñ é 🦀");
    type_at(&mut history, &mut buffer, 0, "// x\n");
    let delete = Edit::delete(&buffer, 0, 3);
    let sel = Selection::new(
        Position::from_char(&buffer, 0),
        Position::from_char(&buffer, 3),
    );
    delete.apply(&mut buffer);
    history.record(Transaction::new(
        vec![delete],
        vec![sel],
        vec![cursor(&buffer, 0)],
    ));
    history.undo(&mut buffer);
    (history, buffer)
}

#[test]
fn round_trip_keeps_undo_redo_and_saved_state_ids() {
    let (history, buffer) = edited();
    let bytes = encode(&history, &buffer, DEFAULT_BUDGET).unwrap();
    let mut restored = decode(&bytes, &buffer).unwrap();
    assert_eq!(restored.state_id(), history.state_id());

    let mut a = buffer.clone();
    let mut b = buffer.clone();
    let mut original = history.clone();
    assert_eq!(restored.redo(&mut a), original.redo(&mut b));
    assert_eq!(a.text(), b.text());
    while let Some(sels) = original.undo(&mut b) {
        assert_eq!(restored.undo(&mut a), Some(sels));
        assert_eq!(a.text(), b.text());
    }
    assert!(!restored.can_undo());
    assert_eq!(a.text(), "");
}

#[test]
fn a_buffer_changed_since_encoding_discards_the_history() {
    let (history, buffer) = edited();
    let bytes = encode(&history, &buffer, DEFAULT_BUDGET).unwrap();
    let mut changed = buffer.clone();
    changed.insert(0, "x");
    assert!(decode(&bytes, &changed).is_none());
    let same_length = Buffer::from_bytes(buffer.text().replace('m', "n").as_bytes()).unwrap();
    assert!(decode(&bytes, &same_length).is_none());
}

#[test]
fn damaged_or_foreign_bytes_are_rejected() {
    let (history, buffer) = edited();
    let bytes = encode(&history, &buffer, DEFAULT_BUDGET).unwrap();
    for i in 0..bytes.len() {
        let mut bad = bytes.clone();
        bad[i] ^= 0x41;
        assert!(decode(&bad, &buffer).is_none(), "flip at {i}");
    }
    assert!(decode(&bytes[..bytes.len() - 1], &buffer).is_none());
    assert!(decode(b"", &buffer).is_none());
    let mut other_version = bytes[..bytes.len() - 8].to_vec();
    other_version[4] = FORMAT_VERSION + 1;
    let sum = Fnv::of(&other_version);
    other_version.extend_from_slice(&sum.to_le_bytes());
    assert!(decode(&other_version, &buffer).is_none());
}

#[test]
fn the_budget_drops_the_oldest_steps_first() {
    let mut buffer = Buffer::from_bytes(b"").unwrap();
    let mut history = History::new();
    for i in 0..200 {
        let at = buffer.len_chars();
        type_at(&mut history, &mut buffer, at, &format!("line {i}\n"));
    }
    let full = encode(&history, &buffer, DEFAULT_BUDGET).unwrap();
    let small = encode(&history, &buffer, full.len() / 4).unwrap();
    assert!(small.len() <= full.len() / 4);
    let mut restored = decode(&small, &buffer).unwrap();
    let mut steps = 0;
    while restored.undo(&mut buffer).is_some() {
        steps += 1;
    }
    assert!(steps > 0 && steps < 200);
    // The newest steps survived, so what remains is a prefix of the file.
    assert!(buffer.text().starts_with("line 0\n"));
    assert!(encode(&history, &buffer, 4).is_none());
}

#[test]
fn an_empty_history_round_trips() {
    let buffer = Buffer::from_bytes(b"plain").unwrap();
    let bytes = encode(&History::new(), &buffer, DEFAULT_BUDGET).unwrap();
    let restored = decode(&bytes, &buffer).unwrap();
    assert!(!restored.can_undo() && !restored.can_redo());
}

#[test]
fn revision_changes_on_every_mutation_and_survives_clone() {
    let mut buffer = Buffer::from_bytes(b"").unwrap();
    let mut history = History::new();
    let r0 = history.revision();
    type_at(&mut history, &mut buffer, 0, "a");
    let r1 = history.revision();
    assert_ne!(r0, r1);
    assert_eq!(history.clone().revision(), r1);
    history.undo(&mut buffer);
    assert_ne!(history.revision(), r1);
    let r2 = history.revision();
    history.redo(&mut buffer);
    assert_ne!(history.revision(), r2);
}
