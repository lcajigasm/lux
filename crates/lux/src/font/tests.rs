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

#[test]
fn rendering_before_the_ui_font_worker_finishes_uses_the_editor_face_without_waiting() {
    use std::sync::mpsc;
    use std::sync::{Arc, Mutex};

    let Some((_, bytes)) = load_system_monospace_font() else {
        return;
    };
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    let fallback = swash::FontRef::from_index(bytes, 0).expect("editor face parses");

    let cell: &'static UiFaceCell = Box::leak(Box::new(UiFaceCell::new()));
    let readers = Arc::new(Mutex::new(Vec::new()));
    let (entered_tx, entered) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let worker = {
        let readers = readers.clone();
        std::thread::spawn(move || {
            cell.load(move |_| {
                readers.lock().unwrap().push(std::thread::current().id());
                let _ = entered_tx.send(());
                let _ = released.recv();
                Ok(bytes.to_vec())
            });
        })
    };

    // The worker is inside the initializer and holds it until released.
    entered.recv().unwrap();
    let started = std::time::Instant::now();
    let (_, family) = pick_ui_font(cell, fallback, "Editor");
    assert_eq!(family, "Editor");
    assert!(started.elapsed() < std::time::Duration::from_millis(100));

    release.send(()).unwrap();
    worker.join().unwrap();
    assert!(
        readers
            .lock()
            .unwrap()
            .iter()
            .all(|id| *id != std::thread::current().id()),
        "the UI thread must never read the font file"
    );

    // Loaded but not yet registered with the layout: still the editor face.
    let (_, family) = pick_ui_font(cell, fallback, "Editor");
    assert_eq!(family, "Editor");

    let mut layout = lux_render::TextLayoutContext::new();
    cell.install(&mut layout);
    let (_, family) = pick_ui_font(cell, fallback, "Editor");
    assert_eq!(family, cell.loaded().expect("face loaded").family);
}
