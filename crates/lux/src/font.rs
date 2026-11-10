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
/// Faces that cover scripts the primary monospace file does not: Han,
/// kana, Hangul, and color emoji. Missing paths are skipped. The editor
/// still shapes with the primary family; these are fallbacks only.
pub fn install_coverage_fallbacks(layout: &mut lux_render::TextLayoutContext) {
    const HANI: &[[u8; 4]] = &[*b"Hani", *b"Hira", *b"Kana"];
    const HANG: &[[u8; 4]] = &[*b"Hang"];
    const CJK: &[[u8; 4]] = &[*b"Hani", *b"Hira", *b"Kana", *b"Hang"];
    const FACES: &[(&str, &[[u8; 4]], bool)] = &[
        ("/System/Library/Fonts/PingFang.ttc", HANI, false),
        ("/System/Library/Fonts/Hiragino Sans GB.ttc", HANI, false),
        ("/System/Library/Fonts/AppleSDGothicNeo.ttc", HANG, false),
        ("/System/Library/Fonts/Apple Color Emoji.ttc", &[], true),
        (
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
            CJK,
            false,
        ),
        (
            "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
            &[],
            true,
        ),
    ];
    for (path, scripts, emoji) in FACES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        layout.register_coverage_font(&bytes, scripts, *emoji);
    }
}

pub fn load_system_monospace_font() -> Option<(String, Vec<u8>)> {
    candidates().into_iter().find_map(|path| {
        std::fs::read(&path)
            .ok()
            .filter(|bytes| swash::FontRef::from_index(bytes, 0).is_some())
            .map(|bytes| (path.display().to_string(), bytes))
    })
}

/// SF Pro on macOS, then Helvetica, then a sans face on Linux and Windows.
/// The editor keeps its monospace file. Selectors and settings use this one.
#[cfg(target_os = "macos")]
const UI_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/SFNSText.ttf",
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/Geneva.ttf",
    "/System/Library/Fonts/Supplemental/Arial.ttf",
];

#[cfg(target_os = "windows")]
const UI_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\segoeui.ttf",
    r"C:\Windows\Fonts\arial.ttf",
];

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
const UI_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
];

struct UiFace {
    font: swash::FontRef<'static>,
    family: &'static str,
    bytes: &'static [u8],
}

fn ui_face() -> Option<&'static UiFace> {
    use std::sync::OnceLock;
    static FACE: OnceLock<Option<UiFace>> = OnceLock::new();
    FACE.get_or_init(|| {
        for path in UI_CANDIDATES {
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
            let Some(font) = swash::FontRef::from_index(leaked, 0) else {
                continue;
            };
            let mut layout = lux_render::TextLayoutContext::new();
            let Some(family) = layout.register_font(leaked) else {
                continue;
            };
            let family: &'static str = Box::leak(family.into_boxed_str());
            eprintln!("Lux: UI font {path} ({family})");
            return Some(UiFace {
                font,
                family,
                bytes: leaked,
            });
        }
        eprintln!("Lux: no system UI font found; selectors use the editor face");
        None
    })
    .as_ref()
}

/// Registers the system UI face with `layout` so selector text can shape it.
pub(crate) fn install_ui_font(layout: &mut lux_render::TextLayoutContext) {
    let Some(face) = ui_face() else {
        return;
    };
    let _ = layout.register_font(face.bytes);
}

/// The face selectors and settings draw with. Falls back to the editor font
/// when the machine has none of the known UI files.
pub(crate) fn ui_font(
    fallback: swash::FontRef<'static>,
    fallback_family: &str,
) -> (swash::FontRef<'static>, String) {
    match ui_face() {
        Some(face) => (face.font, face.family.to_string()),
        None => (fallback, fallback_family.to_string()),
    }
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
