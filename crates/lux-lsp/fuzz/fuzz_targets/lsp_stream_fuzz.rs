//! Fuzzes the real JSON-RPC stream parsing this client actually depends
//! on for every byte a language server sends, driven through the same
//! two layers `lux-lsp`'s own `ServerSession` chains together in
//! production: `lux_lsp::guard_stream` (this crate's own defensive
//! Content-Length cap, `src/framing.rs`) wrapping `async-lsp`'s
//! `Message::read` (Content-Length header framing, then `serde_json`
//! deserialization into a `Message`), fed into `async_lsp::MainLoop::run`
//! against a minimal, do-nothing `Router` (no registered routes --
//! request/notification dispatch beyond parsing is out of this
//! deliverable's scope).
//!
//! This target originally drove `async-lsp` directly with no guard in
//! front of it. That version found a real bug within seconds: a short
//! header claiming an enormous `Content-Length` made `Message::read`
//! attempt an unbounded allocation and abort the process --
//! `async-lsp`'s own private parser has no cap, and there's no public
//! hook to add one from outside. `lux_lsp::guard_stream` is the fix,
//! and this target now exercises the exact chain production code runs,
//! so a regression in either layer -- the guard or `async-lsp` itself --
//! still surfaces here.
//!
//! No structured `Arbitrary` input generation: raw bytes fed straight in
//! as the wire stream is both the simplest harness that works and the
//! most honest one -- it doesn't pre-judge which malformed shapes "count"
//! (a hand-built generator would have to decide, for instance, how large
//! a bogus `Content-Length` value is "realistic" to test, which silently
//! narrows the search). libFuzzer's coverage-guided mutation (helped by a
//! couple of valid seed messages in `corpus/lsp_stream_fuzz/`) is capable
//! of discovering the "Content-Length: " header shape on its own.
//!
//! No explicit assertions: the deliverable's bar, like every other Phase 7
//! gate against this dependency, is "the real parser never panics or
//! aborts on attacker- or bug-controlled bytes" -- the harness itself,
//! running under libFuzzer with its default RSS/malloc limits (which
//! catch an unbounded attacker-controlled allocation exactly the way a
//! panic would be caught), is the check. A `Result::Err` from malformed
//! input (bad header, an oversized claim the guard rejects, truncated
//! body, invalid JSON, an unknown method) is the parser doing its job
//! correctly, not a bug.

#![no_main]

use async_lsp::MainLoop;
use async_lsp::router::Router;
use futures::executor::block_on;
use futures::io::{Cursor, sink};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let (main_loop, _socket) = MainLoop::new_client(|_server_socket| Router::new(()));
    let guarded_input = lux_lsp::guard_stream(Cursor::new(data.to_vec()));
    let output = sink();
    let _ = block_on(main_loop.run_buffered(guarded_input, output));
});
