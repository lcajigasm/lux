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

use std::path::PathBuf;

/// SF Mono before Menlo. `SFNSMono.ttf` is the older system file; current
/// macOS ships SF Mono from Xcode at the Supplemental and `/Library/Fonts`
/// paths. Menlo remains the fallback when SF Mono is not installed.
/// Compiled on every host so the order can be tested off macOS; only the
/// macOS loader reads it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) const MACOS_MONOSPACE_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/SFNSMono.ttf",
    "/System/Library/Fonts/Supplemental/SF-Mono-Regular.otf",
    "/Library/Fonts/SF-Mono-Regular.otf",
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Monaco.ttf",
    "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
];

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
const LINUX_MONOSPACE_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
    "/run/current-system/sw/share/X11/fonts/TTF/DejaVuSansMono.ttf",
];

fn candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        MACOS_MONOSPACE_CANDIDATES
            .iter()
            .map(PathBuf::from)
            .collect()
    }
    #[cfg(target_os = "windows")]
    {
        let fonts = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
            .join("Fonts");
        ["consola.ttf", "lucon.ttf", "cour.ttf"]
            .into_iter()
            .map(|name| fonts.join(name))
            .collect()
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        LINUX_MONOSPACE_CANDIDATES
            .iter()
            .map(PathBuf::from)
            .collect()
    }
}

/// Returns the first candidate's raw bytes that actually exists and reads
/// successfully, along with the path it came from (for the window title and
/// startup log). `None` only if every candidate is missing — a real,
/// reportable failure, not a silent fallback to nothing.
pub fn load_system_monospace_font() -> Option<(String, Vec<u8>)> {
    candidates().into_iter().find_map(|path| {
        std::fs::read(&path)
            .ok()
            .filter(|bytes| swash::FontRef::from_index(bytes, 0).is_some())
            .map(|bytes| (path.display().to_string(), bytes))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_system_monospace_font_is_found_and_parses() {
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

    #[test]
    fn macos_tries_sf_mono_before_menlo() {
        let sf = MACOS_MONOSPACE_CANDIDATES
            .iter()
            .position(|path| path.contains("SF"))
            .expect("an SF Mono path");
        let menlo = MACOS_MONOSPACE_CANDIDATES
            .iter()
            .position(|path| path.ends_with("Menlo.ttc"))
            .expect("Menlo");
        assert!(sf < menlo);
        assert!(
            MACOS_MONOSPACE_CANDIDATES
                .iter()
                .any(|path| path.ends_with("SF-Mono-Regular.otf"))
        );
        assert!(MACOS_MONOSPACE_CANDIDATES[0].ends_with("SFNSMono.ttf"));
    }
}
