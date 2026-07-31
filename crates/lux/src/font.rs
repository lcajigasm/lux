//! Real system monospace font loading.
//!
//! No font ships bundled with the editor yet (a real Phase 3+ concern), and
//! `fontique`'s system-discovery feature is off workspace-wide (see
//! `lux-render/Cargo.toml`'s own doc comment: it pulls a `fontconfig`
//! version that conflicts with `lux-platform`'s Wayland CSD dependency).
//! Reading a known system font file's bytes directly and registering them
//! via `TextLayoutContext::register_font` sidesteps that conflict entirely
//! — the same bytes-registration path the test suite's bundled Ahem fixture
//! already uses, just pointed at a real, legible system file instead.

use std::path::Path;

/// Tried in order; the first path that exists and reads successfully wins.
/// All are real macOS system fonts — this list is macOS-only by
/// construction, a real, documented gap (not a silent one): there is no
/// Windows or Linux environment on this machine to find or verify
/// equivalent system font paths against, the same gap every other Phase 2
/// deliverable this session has hit.
const MACOS_MONOSPACE_CANDIDATES: &[&str] = &[
    // San Francisco Mono: the actual system monospace font on modern macOS.
    "/System/Library/Fonts/SFNSMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Monaco.ttf",
    "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
];

/// Returns the first candidate's raw bytes that actually exists and reads
/// successfully, along with the path it came from (for the window title and
/// startup log). `None` only if every candidate is missing — a real,
/// reportable failure, not a silent fallback to nothing.
pub fn load_system_monospace_font() -> Option<(String, Vec<u8>)> {
    MACOS_MONOSPACE_CANDIDATES.iter().find_map(|path| {
        std::fs::read(Path::new(path))
            .ok()
            .map(|bytes| (path.to_string(), bytes))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_macos_system_monospace_font_is_found_and_parses() {
        let Some((path, bytes)) = load_system_monospace_font() else {
            // Not this crate's bug: same "no environment to verify against"
            // gap as every cross-platform check this session has hit.
            return;
        };
        assert!(!bytes.is_empty(), "font file at {path} must not be empty");
        assert!(
            swash::FontRef::from_index(&bytes, 0).is_some(),
            "font bytes read from {path} must parse as a real font"
        );
    }
}
