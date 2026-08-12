//! A defensive guard on the LSP wire framing (`Content-Length: N\r\n\r\n`
//! plus `N` body bytes), sitting between a language server's raw stdout
//! and `async-lsp`'s own message parser.
//!
//! Found by this phase's fuzzer (`fuzz/fuzz_targets/lsp_stream_fuzz.rs`):
//! `async-lsp` 0.2.4's `Message::read` takes the `Content-Length` value
//! straight from the header and does `vec![0u8; content_len]` with no
//! upper bound before ever reading a body byte. A single short header
//! line claiming an enormous length aborts the whole process on an
//! oversized-allocation failure -- not the language server crashing, but
//! *this client* crashing because of what the server (buggy or hostile)
//! sent. `Message::read` is a private method with no public hook to cap
//! it directly, so this module re-parses just the framing layer (not the
//! JSON-RPC payload -- that's still entirely `async-lsp`'s job) as a
//! transparent pass-through: forward well-formed frames unchanged, refuse
//! anything claiming more than [`MAX_MESSAGE_BYTES`] before ever
//! allocating for it.

use std::io;
use std::pin::Pin;

use futures::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt};
use futures::stream::{self, TryStreamExt};

/// No real LSP response approaches this size; a claim larger than it is
/// treated as a malformed or hostile stream, not a legitimately large
/// message.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

/// A single frame's header block is always a handful of short lines; a
/// header that never terminates is the same class of problem as an
/// oversized `Content-Length`, just via an unbounded local buffer instead
/// of an unbounded allocation.
const MAX_HEADER_BYTES: usize = 64 * 1024;

/// Wrap a reader of the raw LSP wire stream so a claimed `Content-Length`
/// over [`MAX_MESSAGE_BYTES`] ends the stream with a clean I/O error
/// instead of reaching `async-lsp`'s unbounded allocation. Well-formed
/// frames, and any frame without a parseable `Content-Length` at all
/// (left for `async-lsp`'s own stricter parser to reject correctly),
/// pass through byte-for-byte unchanged.
pub fn guard_stream<R>(reader: R) -> Pin<Box<dyn AsyncRead + Send>>
where
    R: AsyncBufRead + Unpin + Send + 'static,
{
    Box::pin(
        stream::unfold(Some(reader), |state| async move {
            let mut reader = state?;
            match read_frame(&mut reader).await {
                FrameResult::Frame(bytes) => Some((Ok(bytes), Some(reader))),
                FrameResult::Eof => None,
                FrameResult::Err(err) => Some((Err(err), None)),
            }
        })
        .into_async_read(),
    )
}

enum FrameResult {
    Frame(Vec<u8>),
    Eof,
    Err(io::Error),
}

async fn read_frame<R: AsyncBufRead + Unpin>(reader: &mut R) -> FrameResult {
    let mut header = Vec::new();
    let mut content_length = None;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) if header.is_empty() => return FrameResult::Eof,
            Ok(0) => {
                return FrameResult::Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "LSP stream ended in the middle of a header block",
                ));
            }
            Ok(_) => {}
            Err(err) => return FrameResult::Err(err),
        }
        header.extend_from_slice(line.as_bytes());
        if header.len() > MAX_HEADER_BYTES {
            return FrameResult::Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "LSP header block exceeds the maximum accepted size",
            ));
        }
        if line == "\r\n" {
            break;
        }
        // Matched byte-for-byte against async-lsp's own header-line rule
        // (`Message::read`: a strict "\r\n" terminator, exactly ": "
        // between name and value) -- a looser match here could see a
        // *different* line as "the" Content-Length than async-lsp does,
        // which is exactly how this guard's first version missed a
        // dangerous value: a later, malformed "Content-Length"-looking
        // line reset this guard's parsed value back to "none found" while
        // async-lsp's own stricter parser still read the frame's real,
        // huge value from the earlier, well-formed line.
        let Some((name, value)) = line.strip_suffix("\r\n").and_then(|l| l.split_once(": ")) else {
            continue;
        };
        if name.eq_ignore_ascii_case("Content-Length") {
            match value.parse::<usize>() {
                Ok(value) => content_length = Some(value),
                // async-lsp itself errors out immediately on an
                // unparseable Content-Length rather than continuing --
                // matched here so a later line's value can never mask
                // this one.
                Err(_) => {
                    return FrameResult::Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unparseable Content-Length header value",
                    ));
                }
            }
        }
    }

    let Some(len) = content_length else {
        // No parseable Content-Length: let async-lsp's own stricter
        // header parser reject this frame with its real protocol error.
        return FrameResult::Frame(header);
    };
    if len > MAX_MESSAGE_BYTES {
        return FrameResult::Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Content-Length {len} exceeds the {MAX_MESSAGE_BYTES}-byte maximum accepted message size"
            ),
        ));
    }

    let mut body = vec![0u8; len];
    if let Err(err) = reader.read_exact(&mut body).await {
        return FrameResult::Err(err);
    }
    header.extend_from_slice(&body);
    FrameResult::Frame(header)
}

#[cfg(test)]
mod tests {
    use futures::io::Cursor;

    use super::*;

    async fn drain(reader: impl AsyncRead + Unpin) -> io::Result<Vec<u8>> {
        let mut reader = reader;
        let mut out = Vec::new();
        reader.read_to_end(&mut out).await?;
        Ok(out)
    }

    #[tokio::test]
    async fn a_well_formed_frame_passes_through_unchanged() {
        let body = b"{\"jsonrpc\":\"2.0\",\"method\":\"x\"}";
        let msg = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut bytes = msg.into_bytes();
        bytes.extend_from_slice(body);

        let guarded = guard_stream(Cursor::new(bytes.clone()));
        let out = drain(guarded).await.expect("well-formed frame should pass");
        assert_eq!(out, bytes);
    }

    #[tokio::test]
    async fn multiple_frames_all_pass_through_in_order() {
        let make = |body: &[u8]| {
            let mut msg = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
            msg.extend_from_slice(body);
            msg
        };
        let mut bytes = make(b"{\"a\":1}");
        bytes.extend(make(b"{\"b\":2}"));

        let guarded = guard_stream(Cursor::new(bytes.clone()));
        let out = drain(guarded).await.expect("both frames should pass");
        assert_eq!(out, bytes);
    }

    #[tokio::test]
    async fn a_content_length_over_the_cap_is_rejected_before_allocating() {
        let bytes = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES + 1).into_bytes();

        let guarded = guard_stream(Cursor::new(bytes));
        let err = drain(guarded)
            .await
            .expect_err("oversized claim must be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn an_absurdly_large_content_length_is_rejected_the_same_way() {
        // The actual value the fuzzer found: a short header line, an
        // astronomically large claimed length.
        let bytes = b"Content-Length: 88888888888888\r\n\r\n".to_vec();

        let guarded = guard_stream(Cursor::new(bytes));
        let err = drain(guarded)
            .await
            .expect_err("a value this large must never reach an allocation");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn a_valid_huge_length_is_not_masked_by_a_later_malformed_header_line() {
        // Regression test for the fuzzer's second find: a well-formed,
        // huge Content-Length line followed by a header line that only
        // *looks* like Content-Length under a looser match. The first
        // version of this guard used a looser name/value split than
        // async-lsp's own strict one, so it saw the second line as
        // "another Content-Length" whose failed parse reset its tracked
        // value back to "none found" -- while async-lsp's own stricter
        // parser never recognized that second line as Content-Length at
        // all, so it still read the real value from the first line and
        // tried to allocate for it. Fits in a `usize` (so a naive
        // "does it parse" check misses it) but exceeds `isize::MAX`,
        // which is what actually crashed: `Vec`'s own capacity check.
        let bytes =
            b"Content-Length: 14743019325349691224\r\nContent-Length:ent-Length: 58\r\n\r\n"
                .to_vec();

        let guarded = guard_stream(Cursor::new(bytes));
        let err = drain(guarded)
            .await
            .expect_err("the real, huge value from the first line must still be caught");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn a_frame_with_no_content_length_passes_through_for_async_lsp_to_reject() {
        let bytes = b"X-Custom: 1\r\n\r\n".to_vec();

        let guarded = guard_stream(Cursor::new(bytes.clone()));
        let out = drain(guarded)
            .await
            .expect("a missing Content-Length is not this guard's job to reject");
        assert_eq!(out, bytes);
    }

    #[tokio::test]
    async fn clean_eof_between_frames_ends_the_stream_without_error() {
        let guarded = guard_stream(Cursor::new(Vec::<u8>::new()));
        let out = drain(guarded).await.expect("empty stream is a clean EOF");
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn an_unterminated_header_block_is_rejected() {
        let bytes = "Content-Length: ".repeat(10_000).into_bytes();

        let guarded = guard_stream(Cursor::new(bytes));
        let err = drain(guarded)
            .await
            .expect_err("a header block that never terminates must not grow unbounded");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }
}
