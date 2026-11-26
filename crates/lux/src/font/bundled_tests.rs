//! Every bundled family loads, registers under its own name, and shapes
//! ASCII and ligature sequences; packaging puts the files and licenses
//! where the binary looks for them.

use std::path::{Path, PathBuf};

use super::*;

/// Families whose `calt` turns `->`, `!=`, `=>` into ligature glyphs.
const WITH_LIGATURES: &[&str] = &["JetBrains Mono", "Fira Code", "Cascadia Code"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn repo_bytes(font: &BundledFont) -> Vec<u8> {
    let (_, bytes) = read_bundled(font, &[repo().join("fonts")])
        .unwrap_or_else(|| panic!("{} is missing from fonts/", font.file));
    bytes
}

fn glyphs(
    cx: &mut lux_render::TextLayoutContext,
    family: &str,
    text: &str,
    ligatures: bool,
) -> Vec<u32> {
    let style = lux_render::LineStyle {
        family,
        font_size: 16.0,
        ligatures,
    };
    let layout = cx.layout_line(&style, text, &[]);
    layout
        .clusters()
        .iter()
        .flat_map(|cluster| {
            cluster
                .glyph_id
                .into_iter()
                .chain(cluster.extra_glyphs.iter().map(|(id, _, _)| *id))
        })
        .collect()
}

#[test]
fn each_bundled_family_registers_under_its_own_name_and_shapes_ascii() {
    for font in BUNDLED_FONTS {
        let bytes = repo_bytes(font);
        let mut cx = lux_render::TextLayoutContext::new();
        let family = cx.register_font(&bytes).expect("registers");
        assert_eq!(family, font.family);
        let ascii = "The quick brown fox {}[]()<>;:'\"`~ 0123456789 +-*/%&|^!?=";
        let ids = glyphs(&mut cx, &family, ascii, true);
        assert_eq!(
            ids.len(),
            ascii.chars().count(),
            "{family}: one glyph per char"
        );
        assert!(
            ids.iter().all(|id| *id != 0),
            "{family}: .notdef in {ids:?}"
        );
    }
}

#[test]
fn ligature_families_change_shape_with_the_setting_and_the_others_do_not() {
    for font in BUNDLED_FONTS {
        let bytes = repo_bytes(font);
        let mut cx = lux_render::TextLayoutContext::new();
        let family = cx.register_font(&bytes).expect("registers");
        for sequence in ["->", "!=", "=>"] {
            let on = glyphs(&mut cx, &family, sequence, true);
            let off = glyphs(&mut cx, &family, sequence, false);
            assert!(on.iter().chain(&off).all(|id| *id != 0));
            if WITH_LIGATURES.contains(&font.family) {
                assert_ne!(on, off, "{family} {sequence}: ligatures on must differ");
            } else {
                assert_eq!(on, off, "{family} {sequence}: no ligature expected");
            }
        }
    }
}

#[test]
fn every_option_is_canonical_and_names_match_without_case() {
    assert_eq!(canonical(None), Some(SYSTEM));
    assert_eq!(canonical(Some("  ")), Some(SYSTEM));
    assert_eq!(canonical(Some("jetbrains mono")), Some("JetBrains Mono"));
    assert_eq!(canonical(Some("Comic Sans MS")), None);
    for option in FAMILY_OPTIONS {
        assert_eq!(canonical(Some(option)), Some(*option));
    }
    for font in BUNDLED_FONTS {
        assert!(FAMILY_OPTIONS.contains(&font.family), "{}", font.family);
    }
}

#[test]
fn install_fonts_ships_every_face_and_license_where_the_bundle_looks() {
    let temp = tempfile::tempdir().unwrap();
    let app = temp.path().join("Lux.app/Contents");
    let status = std::process::Command::new(repo().join("scripts/install-fonts.sh"))
        .arg(app.join("Resources/fonts"))
        .status()
        .expect("install-fonts.sh runs");
    assert!(status.success());
    let fonts = app.join("Resources/fonts");
    assert!(fonts.join("NOTICE.md").is_file());
    assert!(fonts.join("SHA256SUMS").is_file());
    for exe in ["MacOS/lux", "SharedSupport/bin/lux"] {
        // `..` resolves through the real directory, as in a real bundle.
        std::fs::create_dir_all(app.join(exe).parent().unwrap()).unwrap();
        let dirs = font_dirs(Some(&app.join(exe)));
        for font in BUNDLED_FONTS {
            let family_dir = Path::new(font.file).parent().unwrap();
            let license = std::fs::read_to_string(fonts.join(family_dir).join("OFL.txt")).unwrap();
            assert!(license.contains("SIL Open Font License"), "{}", font.family);
            let (source, bytes) = read_bundled(font, &dirs).expect("found in bundle");
            assert!(source.starts_with(&*app.to_string_lossy()), "{source}");
            assert_eq!(bytes, repo_bytes(font));
        }
    }
}

#[test]
fn every_packager_installs_the_fonts() {
    for script in [
        "scripts/stage-lux-app.sh",
        "scripts/package-macos.sh",
        "scripts/package-deb.sh",
        "scripts/package.sh",
    ] {
        let text = std::fs::read_to_string(repo().join(script)).unwrap();
        assert!(text.contains("install-fonts.sh"), "{script}");
    }
}

#[test]
fn the_deb_and_dist_layout_is_found_from_bin() {
    let dirs = font_dirs(Some(Path::new("/usr/bin/lux")));
    assert!(dirs.contains(&PathBuf::from("/usr/bin/../share/lux/fonts")));
}
