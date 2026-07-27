//! Fuzzes exactly what the phase plan asks for: buffer loading and edit
//! application. `arbitrary` turns the raw byte input into structured
//! initial file bytes plus a sequence of insert/delete operations (rather
//! than hand-parsing the fuzzer's bytes), whose offsets are clamped into
//! the buffer's *current* valid range at apply time — the same clamping
//! pattern `tests/undo_proptest.rs` uses, so a generated case is always a
//! legal edit and any panic found here is a real bug, not an artifact of
//! an out-of-bounds test harness.
//!
//! No explicit assertions: the deliverable's bar is "30 minutes, no crash,
//! no panic" (LUX_PROMPT_MAESTRO.md, Fase 1 acceptance criteria) — the
//! harness itself, running under libFuzzer, is the check.

#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use lux_core::buffer::Buffer;
use lux_core::edit::Edit;

#[derive(Debug, Arbitrary)]
enum FuzzOp {
    Insert { char_idx: usize, text: String },
    Delete { char_idx: usize, len: usize },
}

#[derive(Debug, Arbitrary)]
struct FuzzInput {
    initial: Vec<u8>,
    ops: Vec<FuzzOp>,
}

fuzz_target!(|input: FuzzInput| {
    let Ok(mut buffer) = Buffer::from_bytes(&input.initial) else {
        // Binary content or another legitimate decode rejection — not a bug.
        return;
    };

    for op in input.ops {
        match op {
            FuzzOp::Insert { char_idx, text } => {
                let len_chars = buffer.len_chars();
                let idx = char_idx % (len_chars + 1);
                Edit::insert(idx, text).apply(&mut buffer);
            }
            FuzzOp::Delete { char_idx, len } => {
                let len_chars = buffer.len_chars();
                if len_chars == 0 {
                    continue;
                }
                let idx = char_idx % len_chars;
                let max_len = len_chars - idx;
                let del_len = len % max_len + 1;
                Edit::delete(&buffer, idx, del_len).apply(&mut buffer);
            }
        }
    }

    // Touch the result so a corrupted rope would still surface here even
    // if the last op happened to be a no-op.
    let _ = buffer.text();
});
