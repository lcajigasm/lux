//! Developer fonts that ship next to the binary (ADR 0020), and the names
//! `editor.fontFamily` accepts.
//!
//! Only the selected family is ever read, and only on a worker or before
//! the event loop starts. Nothing here runs on the UI thread.

use std::path::{Path, PathBuf};

/// `editor.fontFamily` value for the platform's own monospace face.
pub(crate) const SYSTEM: &str = "system";

pub(crate) struct BundledFont {
    /// Family name as the font's `name` table spells it.
    pub(crate) family: &'static str,
    /// Path under the fonts directory.
    pub(crate) file: &'static str,
}

pub(crate) const BUNDLED_FONTS: &[BundledFont] = &[
    BundledFont {
        family: "JetBrains Mono",
        file: "JetBrainsMono/JetBrainsMono-Regular.ttf",
    },
    BundledFont {
        family: "Fira Code",
        file: "FiraCode/FiraCode-Regular.ttf",
    },
    BundledFont {
        family: "IBM Plex Mono",
        file: "IBMPlexMono/IBMPlexMono-Regular.ttf",
    },
    BundledFont {
        family: "Source Code Pro",
        file: "SourceCodePro/SourceCodePro-Regular.ttf",
    },
    BundledFont {
        family: "Cascadia Code",
        file: "CascadiaCode/CascadiaCode-Regular.ttf",
    },
];

/// System families the loader already knew before fonts were bundled,
/// each with the files tried in order.
#[cfg(target_os = "macos")]
const SYSTEM_FAMILIES: &[(&str, &[&str])] = &[
    (
        "SF Mono",
        &[
            "/System/Library/Fonts/SFNSMono.ttf",
            "/System/Library/Fonts/Supplemental/SF-Mono-Regular.otf",
            "/Library/Fonts/SF-Mono-Regular.otf",
        ],
    ),
    ("Menlo", &["/System/Library/Fonts/Menlo.ttc"]),
];
#[cfg(not(target_os = "macos"))]
const SYSTEM_FAMILIES: &[(&str, &[&str])] = &[];

/// What the Settings page offers, in order. Only families that are
/// installed where the binary runs: the macOS bundle and the Linux `.deb`
/// and tarball ship the bundled fonts; the Windows MSI does not, so Windows
/// offers the system face alone.
#[cfg(target_os = "macos")]
pub(crate) const FAMILY_OPTIONS: &[&str] = &[
    SYSTEM,
    "SF Mono",
    "Menlo",
    "JetBrains Mono",
    "Fira Code",
    "IBM Plex Mono",
    "Source Code Pro",
    "Cascadia Code",
];
#[cfg(not(any(target_os = "macos", windows)))]
pub(crate) const FAMILY_OPTIONS: &[&str] = &[
    SYSTEM,
    "JetBrains Mono",
    "Fira Code",
    "IBM Plex Mono",
    "Source Code Pro",
    "Cascadia Code",
];
#[cfg(windows)]
pub(crate) const FAMILY_OPTIONS: &[&str] = &[SYSTEM];

/// The canonical spelling of an `editor.fontFamily` value. Unset, empty,
/// and `"system"` are the system face. A CSS-style stack such as VS Code's
/// `"'JetBrains Mono', Menlo, monospace"` resolves to its first entry Lux
/// can load; the generic `monospace` is the system face. `None` when no
/// entry is a family Lux can load.
pub(crate) fn canonical(raw: Option<&str>) -> Option<&'static str> {
    let raw = raw.map(str::trim).unwrap_or("");
    if raw.is_empty() {
        return Some(SYSTEM);
    }
    let known = |name: &str| {
        FAMILY_OPTIONS
            .iter()
            .copied()
            .find(|option| option.eq_ignore_ascii_case(name))
    };
    known(raw).or_else(|| {
        raw.split(',')
            .map(|entry| entry.trim().trim_matches(|c| c == '\'' || c == '"').trim())
            .find_map(|entry| {
                known(entry).or_else(|| entry.eq_ignore_ascii_case("monospace").then_some(SYSTEM))
            })
    })
}

pub(crate) fn bundled(name: &str) -> Option<&'static BundledFont> {
    BUNDLED_FONTS
        .iter()
        .find(|font| font.family.eq_ignore_ascii_case(name))
}

/// Where the fonts directory can be, for a binary at `exe`, in lookup
/// order: inside a macOS bundle (from `Contents/MacOS` and from
/// `Contents/SharedSupport/bin`), the `.deb` and dist layout
/// (`bin/../share/lux/fonts`), then, in debug builds and tests only, the
/// repository for dev runs from `target/`. A release build never looks in
/// the source tree, so a package that forgot the fonts shows it.
pub(crate) fn font_dirs(exe: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(bin) = exe.and_then(Path::parent) {
        dirs.push(bin.join("../Resources/fonts"));
        dirs.push(bin.join("../../Resources/fonts"));
        dirs.push(bin.join("../share/lux/fonts"));
    }
    #[cfg(any(debug_assertions, test))]
    dirs.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fonts"));
    dirs
}

/// Reads the face for a canonical name: the bundled file, a known system
/// family, or the system default. Disk I/O: workers and pre-event-loop
/// startup only. `None` when nothing readable parses as a font.
pub(crate) fn read_named(name: &str) -> Option<(String, Vec<u8>)> {
    if name == SYSTEM {
        return super::load_system_monospace_font();
    }
    if let Some(font) = bundled(name) {
        let exe = std::env::current_exe().ok();
        return read_bundled(font, &font_dirs(exe.as_deref()));
    }
    let (_, paths) = SYSTEM_FAMILIES
        .iter()
        .find(|(family, _)| family.eq_ignore_ascii_case(name))?;
    read_first(paths.iter().map(PathBuf::from))
}

/// The first copy of `font` found in `dirs`. Disk I/O.
pub(crate) fn read_bundled(font: &BundledFont, dirs: &[PathBuf]) -> Option<(String, Vec<u8>)> {
    read_first(dirs.iter().map(|dir| dir.join(font.file)))
}

fn read_first(paths: impl IntoIterator<Item = PathBuf>) -> Option<(String, Vec<u8>)> {
    paths.into_iter().find_map(|path| {
        std::fs::read(&path)
            .ok()
            .filter(|bytes| swash::FontRef::from_index(bytes, 0).is_some())
            .map(|bytes| (path.display().to_string(), bytes))
    })
}

#[cfg(test)]
#[path = "bundled_tests.rs"]
mod tests;
