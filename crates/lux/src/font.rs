//! Real system monospace font loading. Bundled developer fonts are in
//! `bundled`, and switching the editor face is in `editor` (ADR 0020).
//!
//! `fontique`'s system-discovery feature is off workspace-wide (see
//! `lux-render/Cargo.toml`'s own doc comment: it pulls a `fontconfig`
//! version that conflicts with `lux-platform`'s Wayland CSD dependency).
//! Reading a known system font file's bytes directly and registering them
//! via `TextLayoutContext::register_font` sidesteps that conflict entirely
//! — the same bytes-registration path the test suite's bundled Ahem fixture
//! already uses, just pointed at a real, legible system file instead.

use std::path::PathBuf;

pub(crate) mod bundled;
pub(crate) mod editor;

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

/// Faces that cover scripts the primary monospace file does not: Han,
/// kana, Hangul, and color emoji. Missing paths are skipped. The editor
/// still shapes with the primary family; these are fallbacks only.
const COVERAGE_FACES: &[(&str, &[[u8; 4]], bool)] = &[
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
const HANI: &[[u8; 4]] = &[*b"Hani", *b"Hira", *b"Kana"];
const HANG: &[[u8; 4]] = &[*b"Hang"];
const CJK: &[[u8; 4]] = &[*b"Hani", *b"Hira", *b"Kana", *b"Hang"];

/// A coverage face read from disk, ready to register on the UI thread.
pub(crate) struct CoverageFace {
    pub(crate) bytes: Vec<u8>,
    pub(crate) scripts: &'static [[u8; 4]],
    pub(crate) emoji: bool,
}

/// Reads every coverage face that exists. Disk I/O: call it on a worker.
pub(crate) fn read_coverage_faces() -> Vec<CoverageFace> {
    COVERAGE_FACES
        .iter()
        .filter_map(|(path, scripts, emoji)| {
            std::fs::read(path).ok().map(|bytes| CoverageFace {
                bytes,
                scripts,
                emoji: *emoji,
            })
        })
        .collect()
}

/// Registers faces that `read_coverage_faces` returned. No disk access.
pub(crate) fn install_coverage_faces(
    layout: &mut lux_render::TextLayoutContext,
    faces: &[CoverageFace],
) {
    for face in faces {
        layout.register_coverage_font(&face.bytes, face.scripts, face.emoji);
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

/// The UI face, read once on a worker and handed to the UI thread.
///
/// Only `load` reads the disk, and only a worker calls it. The UI thread
/// uses `ready`, which never waits: `OnceLock::get` returns `None` while
/// another thread is still running the initializer. Rendering also waits
/// for `installed`, set once the face is registered with the editor's text
/// layout, so a frame never names a family the layout does not know yet.
struct UiFaceCell {
    face: std::sync::OnceLock<Option<UiFace>>,
    installed: std::sync::atomic::AtomicBool,
}

impl UiFaceCell {
    const fn new() -> Self {
        Self {
            face: std::sync::OnceLock::new(),
            installed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Reads the first candidate that parses. Disk I/O: workers only.
    fn load(&self, read: impl Fn(&str) -> std::io::Result<Vec<u8>>) {
        self.face.get_or_init(|| {
            for path in UI_CANDIDATES {
                let Ok(bytes) = read(path) else {
                    continue;
                };
                if let Some(face) = parse_ui_face(path, bytes) {
                    return Some(face);
                }
            }
            eprintln!("Lux: no system UI font found; selectors use the editor face");
            None
        });
    }

    /// The loaded face, or `None` while the worker is still reading it.
    fn loaded(&self) -> Option<&UiFace> {
        self.face.get().and_then(Option::as_ref)
    }

    /// The face rendering may use: loaded and registered with the layout.
    fn ready(&self) -> Option<&UiFace> {
        if self.installed.load(std::sync::atomic::Ordering::Acquire) {
            self.loaded()
        } else {
            None
        }
    }

    fn install(&self, layout: &mut lux_render::TextLayoutContext) {
        let Some(face) = self.loaded() else {
            return;
        };
        if layout.register_font(face.bytes).is_some() {
            self.installed
                .store(true, std::sync::atomic::Ordering::Release);
        }
    }
}

fn parse_ui_face(path: &str, bytes: Vec<u8>) -> Option<UiFace> {
    let leaked: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let font = swash::FontRef::from_index(leaked, 0)?;
    let mut layout = lux_render::TextLayoutContext::new();
    let family = layout.register_font(leaked)?;
    let family: &'static str = Box::leak(family.into_boxed_str());
    eprintln!("Lux: UI font {path} ({family})");
    Some(UiFace {
        font,
        family,
        bytes: leaked,
    })
}

static UI_FACE: UiFaceCell = UiFaceCell::new();

/// Reads the system UI face into its cache. Disk I/O: call it on a worker,
/// then `install_ui_font` on the UI thread once the worker reports back.
pub(crate) fn load_ui_font() {
    UI_FACE.load(|path| std::fs::read(path));
}

/// Registers the loaded UI face with `layout`. No disk access; does nothing
/// until `load_ui_font` has finished on its worker.
pub(crate) fn install_ui_font(layout: &mut lux_render::TextLayoutContext) {
    UI_FACE.install(layout);
}

/// The face selectors and settings draw with. Uses the editor face until
/// the UI face has been loaded and installed, and on machines that have
/// none of the known UI files. Never reads the disk and never waits.
pub(crate) fn ui_font<'a>(
    fallback: swash::FontRef<'a>,
    fallback_family: &str,
) -> (swash::FontRef<'a>, String) {
    pick_ui_font(&UI_FACE, fallback, fallback_family)
}

fn pick_ui_font<'a>(
    cell: &UiFaceCell,
    fallback: swash::FontRef<'a>,
    fallback_family: &str,
) -> (swash::FontRef<'a>, String) {
    match cell.ready() {
        Some(face) => (face.font, face.family.to_string()),
        None => (fallback, fallback_family.to_string()),
    }
}

#[cfg(test)]
mod tests;
