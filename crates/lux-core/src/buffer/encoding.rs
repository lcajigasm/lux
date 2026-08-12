use std::error::Error;
use std::fmt;

use crate::binary_detect;

const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
const UTF16LE_BOM: [u8; 2] = [0xFF, 0xFE];
const UTF16BE_BOM: [u8; 2] = [0xFE, 0xFF];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    /// Fallback guess for bytes that are neither UTF-8 nor UTF-16-with-BOM.
    /// Every byte sequence is valid Latin-1, so this never fails to decode —
    /// it's a last resort, not a confirmed detection.
    Latin1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The content looks like a binary file (a NUL byte was found), so it
    /// isn't opened as text.
    Binary,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Binary => write!(f, "file looks binary, refusing to decode as text"),
        }
    }
}

impl Error for DecodeError {}

/// Detects the encoding of `bytes` and decodes it to a `String`. UTF-16 BOMs
/// are checked before the binary heuristic, since UTF-16 text legitimately
/// contains NUL bytes (the high byte of every ASCII-range code unit) that
/// would otherwise look binary. Order: UTF-16 BOMs; then the binary check;
/// then a UTF-8 BOM; then well-formed UTF-8 without a BOM; then Latin-1 as
/// the final fallback, which always succeeds.
pub fn decode(bytes: &[u8]) -> Result<(String, Encoding), DecodeError> {
    if let Some(rest) = bytes.strip_prefix(&UTF16LE_BOM) {
        return Ok((decode_utf16(rest, u16::from_le_bytes), Encoding::Utf16Le));
    }
    if let Some(rest) = bytes.strip_prefix(&UTF16BE_BOM) {
        return Ok((decode_utf16(rest, u16::from_be_bytes), Encoding::Utf16Be));
    }
    if binary_detect::looks_binary(bytes) {
        return Err(DecodeError::Binary);
    }
    if let Some(rest) = bytes.strip_prefix(&UTF8_BOM) {
        return Ok((
            String::from_utf8_lossy(rest).into_owned(),
            Encoding::Utf8Bom,
        ));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok((text.to_owned(), Encoding::Utf8));
    }
    Ok((decode_latin1(bytes), Encoding::Latin1))
}

/// Like [`decode`], but takes ownership of `bytes` so the common
/// no-BOM-valid-UTF-8 case can reuse the same allocation as the returned
/// `String` (`String::from_utf8`) instead of copying it — the fix for the
/// "open a 100 MB file" RSS budget documented in `docs/PERFORMANCE.md`
/// (three simultaneous full copies of the file at peak: the raw bytes, the
/// decoded `String`, and `ropey`'s own chunk storage). Every other path
/// (a BOM, invalid UTF-8) still allocates a fresh `String`, same as
/// `decode` — only the dominant plain-UTF-8 case skips the copy.
pub fn decode_owned(bytes: Vec<u8>) -> Result<(String, Encoding), DecodeError> {
    if bytes.starts_with(&UTF16LE_BOM) {
        return Ok((
            decode_utf16(&bytes[UTF16LE_BOM.len()..], u16::from_le_bytes),
            Encoding::Utf16Le,
        ));
    }
    if bytes.starts_with(&UTF16BE_BOM) {
        return Ok((
            decode_utf16(&bytes[UTF16BE_BOM.len()..], u16::from_be_bytes),
            Encoding::Utf16Be,
        ));
    }
    if binary_detect::looks_binary(&bytes) {
        return Err(DecodeError::Binary);
    }
    if bytes.starts_with(&UTF8_BOM) {
        return Ok((
            String::from_utf8_lossy(&bytes[UTF8_BOM.len()..]).into_owned(),
            Encoding::Utf8Bom,
        ));
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok((text, Encoding::Utf8)),
        Err(e) => Ok((decode_latin1(e.as_bytes()), Encoding::Latin1)),
    }
}

/// Encodes `text` back to bytes for the given encoding — the inverse of
/// `decode`, used when saving so a file round-trips in its original form.
pub fn encode(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => {
            let mut out = UTF8_BOM.to_vec();
            out.extend_from_slice(text.as_bytes());
            out
        }
        Encoding::Utf16Le => encode_utf16(text, &UTF16LE_BOM, u16::to_le_bytes),
        Encoding::Utf16Be => encode_utf16(text, &UTF16BE_BOM, u16::to_be_bytes),
        // ponytail: lossy for code points above U+00FF (replaced with '?');
        // Latin-1 can't represent them. Upgrade if round-tripping non-Latin-1
        // text through a Latin-1-detected file ever matters.
        Encoding::Latin1 => text
            .chars()
            .map(|c| if c as u32 <= 0xFF { c as u8 } else { b'?' })
            .collect(),
    }
}

fn decode_utf16(bytes: &[u8], from_bytes: fn([u8; 2]) -> u16) -> String {
    let units = bytes
        .chunks_exact(2)
        .map(|pair| from_bytes([pair[0], pair[1]]));
    char::decode_utf16(units)
        .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

fn encode_utf16(text: &str, bom: &[u8], to_bytes: fn(u16) -> [u8; 2]) -> Vec<u8> {
    let mut out = bom.to_vec();
    for unit in text.encode_utf16() {
        out.extend_from_slice(&to_bytes(unit));
    }
    out
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_plain_utf8() {
        let (text, encoding) = decode("héllo".as_bytes()).unwrap();
        assert_eq!(text, "héllo");
        assert_eq!(encoding, Encoding::Utf8);
    }

    #[test]
    fn detects_utf8_bom() {
        let mut bytes = UTF8_BOM.to_vec();
        bytes.extend_from_slice("hi".as_bytes());
        let (text, encoding) = decode(&bytes).unwrap();
        assert_eq!(text, "hi");
        assert_eq!(encoding, Encoding::Utf8Bom);
    }

    #[test]
    fn detects_and_decodes_utf16_le() {
        let bytes = encode("hi 😀", Encoding::Utf16Le);
        let (text, encoding) = decode(&bytes).unwrap();
        assert_eq!(text, "hi 😀");
        assert_eq!(encoding, Encoding::Utf16Le);
    }

    #[test]
    fn detects_and_decodes_utf16_be() {
        let bytes = encode("hi 😀", Encoding::Utf16Be);
        let (text, encoding) = decode(&bytes).unwrap();
        assert_eq!(text, "hi 😀");
        assert_eq!(encoding, Encoding::Utf16Be);
    }

    #[test]
    fn falls_back_to_latin1_for_invalid_utf8() {
        // 0xE9 is 'é' in Latin-1 but not valid on its own as UTF-8.
        let bytes = [b'c', b'a', 0xE9, b'f', b'e'];
        let (text, encoding) = decode(&bytes).unwrap();
        assert_eq!(text, "caéfe");
        assert_eq!(encoding, Encoding::Latin1);
    }

    #[test]
    fn rejects_binary_content() {
        let bytes = [0x00, 0x01, 0x02, b'a', b'b'];
        assert_eq!(decode(&bytes), Err(DecodeError::Binary));
    }

    #[test]
    fn round_trips_each_encoding() {
        for encoding in [
            Encoding::Utf8,
            Encoding::Utf8Bom,
            Encoding::Utf16Le,
            Encoding::Utf16Be,
        ] {
            let original = "Hello, world! Ünïcödé 😀";
            let bytes = encode(original, encoding);
            let (decoded, detected) = decode(&bytes).unwrap();
            assert_eq!(decoded, original);
            assert_eq!(detected, encoding);
        }
    }

    #[test]
    fn latin1_round_trips_within_its_range() {
        let original = "café";
        let bytes = encode(original, Encoding::Latin1);
        let (decoded, detected) = decode(&bytes).unwrap();
        assert_eq!(decoded, original);
        assert_eq!(detected, Encoding::Latin1);
    }

    #[test]
    fn decode_owned_agrees_with_decode_on_every_case() {
        let cases: Vec<Vec<u8>> = vec![
            "héllo".as_bytes().to_vec(),
            encode("hi", Encoding::Utf8Bom),
            encode("hi 😀", Encoding::Utf16Le),
            encode("hi 😀", Encoding::Utf16Be),
            vec![b'c', b'a', 0xE9, b'f', b'e'],
            vec![0x00, 0x01, 0x02, b'a', b'b'],
        ];
        for bytes in cases {
            assert_eq!(decode(&bytes), decode_owned(bytes));
        }
    }
}
