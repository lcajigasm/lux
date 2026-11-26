use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use super::*;

#[test]
fn load_faces_reads_each_distinct_face_once_and_skips_unresolved_chars() {
    let dir = std::env::temp_dir().join(format!("lux-fallback-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a = dir.join("a.ttf");
    let b = dir.join("b.ttf");
    let reads = Arc::new(Mutex::new(Vec::<PathBuf>::new()));
    let (pa, pb) = (a.clone(), b.clone());
    let locate = move |ch: char| match ch {
        'x' | 'y' => Some(pa.clone()),
        'z' => Some(pb.clone()),
        _ => None,
    };
    let log = reads.clone();
    let read = move |path: &Path| {
        log.lock().unwrap().push(path.to_path_buf());
        Ok(path.to_string_lossy().as_bytes().to_vec())
    };
    let faces = load_faces(&['x', 'y', 'q', 'z'], &[], &locate, &read);
    let keys: Vec<_> = faces.iter().map(|f| f.key.clone()).collect();
    assert_eq!(keys, vec![a.display().to_string(), b.display().to_string()]);
    assert_eq!(*reads.lock().unwrap(), vec![a.clone(), b.clone()]);

    // A later batch whose face the layout context already holds reads
    // nothing and returns nothing: no second copy of the bytes.
    let again = load_faces(&['y'], &keys, &locate, &read);
    assert!(again.is_empty());
    assert_eq!(reads.lock().unwrap().len(), 2);
}

#[test]
fn nothing_is_requested_before_the_coverage_faces_are_registered() {
    let Some((_, bytes)) = crate::font::load_system_monospace_font() else {
        return;
    };
    let mut layout = lux_render::TextLayoutContext::new();
    let family = layout.register_font(&bytes).expect("editor face");
    let style = lux_render::LineStyle {
        family: &family,
        font_size: 16.0,
        ligatures: false,
    };
    layout.layout_line(&style, "ሰላም", &[]);
    assert!(take_job(&mut layout).is_none());
}

/// Thai, Arabic, Devanagari, Hebrew, Ethiopic and math symbols are
/// outside the fixed coverage list. After
/// one round trip through the worker, every character has a real glyph,
/// and only the worker thread read a font file.
#[cfg(target_os = "macos")]
#[test]
fn scripts_outside_the_coverage_list_get_real_glyphs_from_a_worker() {
    let Some((_, bytes)) = crate::font::load_system_monospace_font() else {
        panic!("macOS has a system monospace face");
    };
    let mut layout = lux_render::TextLayoutContext::new();
    let family = layout.register_font(&bytes).expect("editor face");
    let coverage = std::thread::spawn(crate::font::read_coverage_faces)
        .join()
        .unwrap();
    crate::font::install_coverage_faces(&mut layout, &coverage);
    assert!(layout.has_coverage_fonts());

    let line = "Thai สวัสดีครับ Arabic مرحبا بالعالم Devanagari नमस्ते दुनिया \
        Hebrew שלום Ethiopic ሰላም Math ∮ ⨁ 𝔸 ⟦x⟧";
    let style = lux_render::LineStyle {
        family: &family,
        font_size: 16.0,
        ligatures: false,
    };
    let notdef = |layout: &mut lux_render::TextLayoutContext| -> Vec<String> {
        let shaped = layout.layout_line(&style, line, &[]);
        shaped
            .clusters()
            .into_iter()
            .filter(|c| {
                let text = &line[c.byte_range.clone()];
                !text.trim().is_empty()
                    && (c.glyph_id == Some(0) || c.extra_glyphs.iter().any(|g| g.0 == 0))
            })
            .map(|c| line[c.byte_range].to_owned())
            .collect()
    };

    let before = notdef(&mut layout);
    assert!(
        !before.is_empty(),
        "the editor face alone has no Thai, Arabic or Devanagari"
    );

    let ui_thread = std::thread::current().id();
    let readers = Arc::new(Mutex::new(Vec::new()));
    let job = take_job(&mut layout)
        .expect("missing characters become a job")
        .with_family(&family);
    let log = readers.clone();
    let faces = std::thread::spawn(move || {
        job.run(&|path| {
            log.lock().unwrap().push(std::thread::current().id());
            std::fs::read(path)
        })
    })
    .join()
    .unwrap();
    assert!(!faces.is_empty(), "CoreText names a face for these scripts");
    assert!(!readers.lock().unwrap().is_empty());
    assert!(readers.lock().unwrap().iter().all(|id| *id != ui_thread));

    install(&mut layout, faces);
    let after = notdef(&mut layout);
    assert!(after.is_empty(), "still .notdef after fallback: {after:?}");
    assert!(take_job(&mut layout).is_none());
}
