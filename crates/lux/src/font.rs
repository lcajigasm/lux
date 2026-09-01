//! Real system monospace font loading.
//!
//! No font ships bundled with the editor yet, and `fontique`'s system-
//! discovery feature is off workspace-wide (see `lux-render/Cargo.toml`:
//! it pulls a `fontconfig` native binding that conflicts with
//! `lux-platform`'s Wayland CSD dependency via `links = "fontconfig"`).
//! Reading known system font files' bytes and registering them via
//! `TextLayoutContext::register_font` sidesteps that conflict — the same
//! bytes-registration path the test suite's bundled Ahem fixture uses.
//!
//! On Linux, after the fixed candidate list, we may ask `fc-match` for a
//! monospace file path (one-shot at startup, not a poll loop). That uses
//! the system `fontconfig` CLI without linking `yeslogic-fontconfig-sys`.

use std::path::Path;

#[cfg(target_os = "windows")]
use std::path::PathBuf;

/// macOS system monospace faces (order = preference).
#[cfg(target_os = "macos")]
const PLATFORM_MONOSPACE_CANDIDATES: &[&str] = &[
    "/System/Library/Fonts/SFNSMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Monaco.ttf",
    "/System/Library/Fonts/Supplemental/Andale Mono.ttf",
];

/// Common Linux distro paths (Debian/Ubuntu, Fedora/Arch layouts).
#[cfg(target_os = "linux")]
const PLATFORM_MONOSPACE_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansMono-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoMono-Regular.ttf",
    "/usr/share/fonts/truetype/ubuntu/UbuntuMono-R.ttf",
    "/usr/share/fonts/truetype/jetbrains-mono/JetBrainsMono-Regular.ttf",
    "/usr/share/fonts/truetype/cascadia/CascadiaMono.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/TTF/LiberationMono-Regular.ttf",
    "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
];

/// Windows system + Cascadia installs under `%WINDIR%\Fonts`.
#[cfg(target_os = "windows")]
const PLATFORM_MONOSPACE_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\cascadiamono.ttf",
    r"C:\Windows\Fonts\CascadiaMono.ttf",
    r"C:\Windows\Fonts\consola.ttf",
    r"C:\Windows\Fonts\cour.ttf",
    r"C:\Windows\Fonts\lucon.ttf",
];

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
const PLATFORM_MONOSPACE_CANDIDATES: &[&str] = &[];

fn read_if_parseable(path: &Path) -> Option<(String, Vec<u8>)> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    // Reject files swash cannot open (wrong format / corrupt) before the
    // app treats them as a success and panics later in shaping.
    swash::FontRef::from_index(&bytes, 0)?;
    Some((path.display().to_string(), bytes))
}

fn load_from_candidate_list(candidates: &[&str]) -> Option<(String, Vec<u8>)> {
    candidates
        .iter()
        .find_map(|path| read_if_parseable(Path::new(path)))
}

/// Windows: also probe `%WINDIR%\Fonts` and the per-user fonts directory
/// when the hardcoded drive letter differs (non-`C:` installs).
#[cfg(target_os = "windows")]
fn windows_dynamic_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        let fonts = PathBuf::from(windir).join("Fonts");
        for name in [
            "cascadiamono.ttf",
            "CascadiaMono.ttf",
            "consola.ttf",
            "cour.ttf",
            "lucon.ttf",
        ] {
            out.push(fonts.join(name));
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let fonts = PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts");
        for name in ["CascadiaMono.ttf", "cascadiamono.ttf"] {
            out.push(fonts.join(name));
        }
    }
    out
}

#[cfg(target_os = "windows")]
fn load_windows_extra() -> Option<(String, Vec<u8>)> {
    windows_dynamic_candidates()
        .into_iter()
        .find_map(|path| read_if_parseable(&path))
}

/// Linux: one-shot `fc-match` for a monospace face file. Avoids linking
/// `fontconfig` into the Rust graph (ADR/DEPENDENCIES conflict).
#[cfg(target_os = "linux")]
fn load_via_fc_match() -> Option<(String, Vec<u8>)> {
    let output = std::process::Command::new("fc-match")
        .args(["-f", "%{file}", "monospace:charset=ASCII"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    if path.is_empty() {
        return None;
    }
    read_if_parseable(Path::new(path))
}

/// Returns the first platform monospace font that exists and parses, plus
/// the path used (for the startup log). `None` only if every strategy
/// fails — a real, reportable failure, not a silent blank render.
pub fn load_system_monospace_font() -> Option<(String, Vec<u8>)> {
    if let Some(hit) = load_from_candidate_list(PLATFORM_MONOSPACE_CANDIDATES) {
        return Some(hit);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(hit) = load_windows_extra() {
            return Some(hit);
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(hit) = load_via_fc_match() {
            return Some(hit);
        }
    }
    None
}

/// Message for `expect` / user-facing startup failure (platform-aware).
pub fn missing_font_message() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "no known macOS system monospace font was found — see \
         crates/lux/src/font.rs candidate list"
    }
    #[cfg(target_os = "linux")]
    {
        "no known Linux system monospace font was found — install \
         fonts-dejavu-mono / fonts-liberation / fonts-noto-mono, or ensure \
         `fc-match` can resolve monospace; see crates/lux/src/font.rs"
    }
    #[cfg(target_os = "windows")]
    {
        "no known Windows monospace font was found under %WINDIR%\\Fonts \
         (tried Cascadia Mono, Consolas, Courier New, Lucida Console); see \
         crates/lux/src/font.rs"
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        "no system monospace font loader is configured for this OS — see \
         crates/lux/src/font.rs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_parseable_font_on_this_host_or_skips_when_impossible() {
        let Some((path, bytes)) = load_system_monospace_font() else {
            // Mobile/other hosts without desktop fonts: honest skip.
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            panic!(
                "expected a system monospace font on desktop OS ({})",
                std::env::consts::OS
            );
            #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
            return;
        };
        assert!(!bytes.is_empty(), "font file at {path} must not be empty");
        assert!(
            swash::FontRef::from_index(&bytes, 0).is_some(),
            "font bytes from {path} must parse"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_candidate_list_includes_dejavu_path() {
        assert!(
            PLATFORM_MONOSPACE_CANDIDATES
                .iter()
                .any(|p| p.ends_with("DejaVuSansMono.ttf")),
            "Linux list must prefer DejaVu Sans Mono"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_candidate_list_includes_consolas() {
        assert!(
            PLATFORM_MONOSPACE_CANDIDATES
                .iter()
                .any(|p| p.ends_with("consola.ttf")),
            "Windows list must include Consolas"
        );
    }
}
