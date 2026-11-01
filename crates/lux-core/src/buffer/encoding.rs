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
    /// UTF-16 LE with no BOM. Chosen only when one byte parity is mostly NUL
    /// and the other endianness is a much worse decode. Ambiguous input is
    /// not guessed.
    Utf16LeNoBom,
    /// UTF-16 BE with no BOM. Same confidence rule as [`Encoding::Utf16LeNoBom`].
    Utf16BeNoBom,
    /// Windows-1252. Used when the bytes are not UTF-8 and include a byte in
    /// `0x80..=0x9F` (the range where 1252 differs from Latin-1).
    Windows1252,
    /// Fallback for bytes that are not UTF-8 and have no `0x80..=0x9F` byte.
    /// Those bytes mean the same thing in Latin-1 and Windows-1252.
    Latin1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    /// The content looks like a binary file (a NUL byte was found), so it
    /// isn't opened as text.
    Binary,
    /// A BOM declares UTF-8/UTF-16, but the payload is malformed. Refuse
    /// rather than discard bytes or insert replacement characters on open.
    InvalidEncoding,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Binary => write!(f, "file looks binary, refusing to decode as text"),
            DecodeError::InvalidEncoding => {
                write!(f, "file has malformed Unicode for its declared encoding")
            }
        }
    }
}

impl Error for DecodeError {}

/// Detects the encoding of `bytes` and decodes it to a `String`.
///
/// Order: UTF-16 with a BOM; a confident UTF-16-without-BOM guess; the
/// binary check (NUL bytes that did not look like UTF-16); a UTF-8 BOM;
/// well-formed UTF-8; Windows-1252 when a byte in `0x80..=0x9F` is present;
/// otherwise Latin-1. A guess that is not confident stays binary instead of
/// being decoded as Latin-1, which would corrupt the file on save.
pub fn decode(bytes: &[u8]) -> Result<(String, Encoding), DecodeError> {
    if let Some(found) = decode_marked(bytes)? {
        return Ok(found);
    }
    if binary_detect::looks_binary(bytes) {
        return Err(DecodeError::Binary);
    }
    decode_text(bytes)
}

fn decode_marked(bytes: &[u8]) -> Result<Option<(String, Encoding)>, DecodeError> {
    if let Some(rest) = bytes.strip_prefix(&UTF16LE_BOM) {
        return Ok(Some((
            decode_utf16(rest, u16::from_le_bytes)?,
            Encoding::Utf16Le,
        )));
    }
    if let Some(rest) = bytes.strip_prefix(&UTF16BE_BOM) {
        return Ok(Some((
            decode_utf16(rest, u16::from_be_bytes)?,
            Encoding::Utf16Be,
        )));
    }
    Ok(try_utf16_without_bom(bytes))
}

fn decode_text(bytes: &[u8]) -> Result<(String, Encoding), DecodeError> {
    if let Some(rest) = bytes.strip_prefix(&UTF8_BOM) {
        return Ok((
            std::str::from_utf8(rest)
                .map_err(|_| DecodeError::InvalidEncoding)?
                .to_owned(),
            Encoding::Utf8Bom,
        ));
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok((text.to_owned(), Encoding::Utf8));
    }
    Ok(decode_legacy(bytes))
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
    if let Some(found) = decode_marked(&bytes)? {
        return Ok(found);
    }
    if binary_detect::looks_binary(&bytes) {
        return Err(DecodeError::Binary);
    }
    if bytes.starts_with(&UTF8_BOM) {
        return Ok((
            std::str::from_utf8(&bytes[UTF8_BOM.len()..])
                .map_err(|_| DecodeError::InvalidEncoding)?
                .to_owned(),
            Encoding::Utf8Bom,
        ));
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok((text, Encoding::Utf8)),
        Err(e) => Ok(decode_legacy(e.as_bytes())),
    }
}

/// True when `encode` can write `text` without substituting a character.
pub fn can_encode(text: &str, encoding: Encoding) -> bool {
    match encoding {
        Encoding::Utf8
        | Encoding::Utf8Bom
        | Encoding::Utf16Le
        | Encoding::Utf16Be
        | Encoding::Utf16LeNoBom
        | Encoding::Utf16BeNoBom => true,
        Encoding::Latin1 => text.chars().all(|c| (c as u32) <= 0xFF),
        Encoding::Windows1252 => text.chars().all(|c| encode_windows1252_byte(c).is_some()),
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
        Encoding::Utf16LeNoBom => encode_utf16(text, &[], u16::to_le_bytes),
        Encoding::Utf16BeNoBom => encode_utf16(text, &[], u16::to_be_bytes),
        Encoding::Windows1252 => text
            .chars()
            .map(|c| encode_windows1252_byte(c).unwrap_or(b'?'))
            .collect(),
        Encoding::Latin1 => text
            .chars()
            .map(|c| if (c as u32) <= 0xFF { c as u8 } else { b'?' })
            .collect(),
    }
}

fn decode_utf16(bytes: &[u8], from_bytes: fn([u8; 2]) -> u16) -> Result<String, DecodeError> {
    let (pairs, remainder) = bytes.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(DecodeError::InvalidEncoding);
    }
    let units = pairs.iter().map(|pair| from_bytes(*pair));
    char::decode_utf16(units)
        .map(|r| r.map_err(|_| DecodeError::InvalidEncoding))
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

fn decode_legacy(bytes: &[u8]) -> (String, Encoding) {
    if bytes.iter().any(|b| (0x80..=0x9F).contains(b)) {
        (decode_windows1252(bytes), Encoding::Windows1252)
    } else {
        (decode_latin1(bytes), Encoding::Latin1)
    }
}

/// Unicode scalar for each Windows-1252 byte `0x80 + index`. Undefined bytes
/// keep the C1 control of the same value so the byte still round-trips.
const WINDOWS_1252: [char; 32] = [
    '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}', '\u{017D}', '\u{008F}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}', '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
];

fn decode_windows1252(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if (0x80..=0x9F).contains(&b) {
                WINDOWS_1252[(b - 0x80) as usize]
            } else {
                b as char
            }
        })
        .collect()
}

fn encode_windows1252_byte(c: char) -> Option<u8> {
    let u = c as u32;
    if u <= 0x7F || (0xA0..=0xFF).contains(&u) {
        return Some(u as u8);
    }
    WINDOWS_1252
        .iter()
        .position(|&mapped| mapped == c)
        .map(|index| 0x80 + index as u8)
}

/// UTF-16 without a BOM, only when one byte of each pair is usually NUL
/// (the ASCII-range pattern) and that endianness decodes to text. Pure CJK
/// UTF-16 has no NULs, so it is left alone rather than guessed.
fn try_utf16_without_bom(bytes: &[u8]) -> Option<(String, Encoding)> {
    if bytes.len() < 8 || !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units = bytes.len() / 2;
    let even_nuls = bytes.iter().step_by(2).filter(|b| **b == 0).count();
    let odd_nuls = bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count();
    let mostly = |nuls: usize| nuls * 4 >= units * 3;
    let rare = |nuls: usize| nuls * 4 < units;
    let (from_bytes, encoding) = if mostly(odd_nuls) && rare(even_nuls) {
        (
            u16::from_le_bytes as fn([u8; 2]) -> u16,
            Encoding::Utf16LeNoBom,
        )
    } else if mostly(even_nuls) && rare(odd_nuls) {
        (
            u16::from_be_bytes as fn([u8; 2]) -> u16,
            Encoding::Utf16BeNoBom,
        )
    } else {
        return None;
    };
    let text = decode_utf16(bytes, from_bytes).ok()?;
    if text_score(&text) <= 0 {
        return None;
    }
    Some((text, encoding))
}

fn text_score(text: &str) -> i32 {
    let mut score = 0i32;
    for c in text.chars() {
        if matches!(c, '\n' | '\r' | '\t' | ' ') {
            score += 1;
        } else if c.is_control() {
            score -= 4;
        } else {
            score += 2;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_bom_payloads_are_rejected_without_silent_data_loss() {
        for bytes in [
            vec![0xef, 0xbb, 0xbf, 0xff], // Invalid UTF-8 after BOM.
            vec![0xff, 0xfe, 0x61],       // Partial UTF-16 code unit.
            vec![0xff, 0xfe, 0x00, 0xd8], // Unpaired high surrogate.
            vec![0xfe, 0xff, 0xdc, 0x00], // Unpaired low surrogate.
        ] {
            assert_eq!(decode(&bytes), Err(DecodeError::InvalidEncoding));
            assert_eq!(decode_owned(bytes), Err(DecodeError::InvalidEncoding));
        }
    }

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
            Encoding::Utf16LeNoBom,
            Encoding::Utf16BeNoBom,
        ] {
            let original = "Hello, world! Ünïcödé 😀";
            let bytes = encode(original, encoding);
            let (decoded, detected) = decode(&bytes).unwrap();
            assert_eq!(decoded, original);
            assert_eq!(detected, encoding);
        }
    }

    #[test]
    fn windows1252_smart_quote_is_not_a_c1_control() {
        let bytes = [b'A', 0x93, b'B'];
        let (text, encoding) = decode(&bytes).unwrap();
        assert_eq!(encoding, Encoding::Windows1252);
        assert_eq!(text, "A\u{201C}B");
        assert_eq!(encode(&text, encoding), bytes);
        assert!(can_encode(&text, encoding));
    }

    #[test]
    fn utf16_without_bom_round_trips_and_does_not_gain_a_bom() {
        let text = "Hola ñ\n";
        for encoding in [Encoding::Utf16LeNoBom, Encoding::Utf16BeNoBom] {
            let bytes = encode(text, encoding);
            assert_ne!(&bytes[..2], &UTF16LE_BOM);
            assert_ne!(&bytes[..2], &UTF16BE_BOM);
            let (decoded, detected) = decode(&bytes).unwrap();
            assert_eq!(decoded, text);
            assert_eq!(detected, encoding);
            assert_eq!(encode(&decoded, detected), bytes);
        }
    }

    #[test]
    fn odd_length_nuls_stay_binary() {
        assert_eq!(decode(&[0x48, 0x00, 0x69]), Err(DecodeError::Binary));
    }

    #[test]
    fn fixture_files_round_trip_byte_for_byte() {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/compare/fixtures");
        for name in [
            "characters.txt",
            "sample-utf8-bom.bin",
            "sample-latin1.bin",
            "sample-windows-1252.bin",
            "sample-utf16-le-bom.bin",
            "sample-utf16-be-bom.bin",
            "sample-utf16-le-nobom.bin",
            "sample-utf16-be-nobom.bin",
        ] {
            let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
            let (text, encoding) = decode(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(can_encode(&text, encoding), "{name} {encoding:?}");
            assert_eq!(encode(&text, encoding), bytes, "{name} {encoding:?}");
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
            encode("Hi ñ", Encoding::Utf16LeNoBom),
            encode("Hi ñ", Encoding::Utf16BeNoBom),
            vec![b'c', b'a', 0xE9, b'f', b'e'],
            vec![0x93],
            vec![0x00, 0x01, 0x02, b'a', b'b'],
        ];
        for bytes in cases {
            assert_eq!(decode(&bytes), decode_owned(bytes));
        }
    }
}
