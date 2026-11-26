//! Switching the editor face when `editor.fontFamily` changes (ADR 0020).
//!
//! The UI thread never reads a font file. A face not seen before is read
//! by a `ui_io` worker; the UI thread keeps drawing the current face until
//! the worker reports back, then registers the bytes and swaps. A face
//! loaded once stays registered, so switching back to it is immediate.

use swash::FontRef;

use super::bundled::{self, SYSTEM};
use crate::app::App;

/// A face that has been registered with the editor's text layout.
#[derive(Clone)]
struct Face {
    name: &'static str,
    family: String,
    font: FontRef<'static>,
}

/// Which editor face is in use, which one a worker is reading, and every
/// face registered so far.
pub(crate) struct EditorFonts {
    applied: &'static str,
    pending: Option<&'static str>,
    loaded: Vec<Face>,
}

impl Default for EditorFonts {
    fn default() -> Self {
        Self {
            applied: SYSTEM,
            pending: None,
            loaded: Vec::new(),
        }
    }
}

impl EditorFonts {
    /// The canonical name of the face `App::new` was built with.
    pub(crate) fn started_with(&mut self, name: &'static str) {
        self.applied = name;
    }

    #[cfg(test)]
    pub(crate) fn applied(&self) -> &'static str {
        self.applied
    }

    fn cached(&self, name: &str) -> Option<Face> {
        self.loaded.iter().find(|face| face.name == name).cloned()
    }
}

/// Bytes a worker read for `requested`. `resolved` differs when the
/// requested file was missing and the system face was read instead.
pub(crate) struct LoadedFont {
    requested: &'static str,
    resolved: &'static str,
    source: String,
    bytes: Vec<u8>,
}

/// Reads the face for `name`, or the system face when it is missing.
/// Disk I/O: workers and pre-event-loop startup only.
pub(crate) fn read_face(name: &'static str) -> Option<LoadedFont> {
    if let Some((source, bytes)) = bundled::read_named(name) {
        return Some(LoadedFont {
            requested: name,
            resolved: name,
            source,
            bytes,
        });
    }
    eprintln!("Lux: font {name:?} not found; using the system face");
    let (source, bytes) = super::load_system_monospace_font()?;
    Some(LoadedFont {
        requested: name,
        resolved: SYSTEM,
        source,
        bytes,
    })
}

/// The startup face for a raw `editor.fontFamily` value: its canonical
/// name, where it came from, and its bytes. Runs before the event loop.
pub(crate) fn startup_face(raw: Option<&str>) -> Option<(&'static str, String, Vec<u8>)> {
    let name = requested_name(raw);
    read_face(name).map(|loaded| (loaded.requested, loaded.source, loaded.bytes))
}

fn requested_name(raw: Option<&str>) -> &'static str {
    bundled::canonical(raw).unwrap_or_else(|| {
        eprintln!("Lux: editor.fontFamily {raw:?} is not available; using the system face");
        SYSTEM
    })
}

impl App {
    /// Applies a raw `editor.fontFamily` value. Never reads the disk: a
    /// face not loaded yet is read on a worker and installed later.
    pub(crate) fn apply_editor_font(&mut self, raw: Option<&str>) {
        let name = requested_name(raw);
        let fonts = &mut self.atlas_state.fonts;
        if fonts.applied == name {
            // Also drops a read still running for another family.
            fonts.pending = None;
            return;
        }
        if fonts.pending == Some(name) {
            return;
        }
        fonts.pending = None;
        if let Some(face) = fonts.cached(name) {
            self.use_face(face);
            return;
        }
        fonts.pending = Some(name);
        crate::ui_io::spawn(self, None, move || {
            Some(crate::ui_io::Done::EditorFont(name, read_face(name)))
        });
    }

    /// Registers what a worker read for `requested` and draws with it. A
    /// result for a request that was replaced meanwhile, found or not, is
    /// dropped and leaves the newer request pending.
    pub(crate) fn install_editor_font(
        &mut self,
        requested: &'static str,
        loaded: Option<LoadedFont>,
    ) {
        if self.atlas_state.fonts.pending != Some(requested) {
            return;
        }
        self.atlas_state.fonts.pending = None;
        let Some(loaded) = loaded else {
            return;
        };
        let face = match self.atlas_state.fonts.cached(loaded.resolved) {
            Some(face) => face,
            None => {
                let bytes: &'static [u8] = Box::leak(loaded.bytes.into_boxed_slice());
                let Some(font) = FontRef::from_index(bytes, 0) else {
                    return;
                };
                let Some(family) = self.editor_view.text_layout_mut().register_font(bytes) else {
                    return;
                };
                eprintln!("Lux: editor font {} ({family})", loaded.source);
                Face {
                    name: loaded.resolved,
                    family,
                    font,
                }
            }
        };
        self.use_face(Face {
            name: loaded.requested,
            ..face.clone()
        });
        self.remember(face);
    }

    fn use_face(&mut self, face: Face) {
        let current = Face {
            name: self.atlas_state.fonts.applied,
            family: self.editor_view.family().to_string(),
            font: self.font,
        };
        self.remember(current);
        self.editor_view.set_family(face.family.clone());
        self.font = face.font;
        self.atlas_state.fonts.applied = face.name;
        self.remember(face);
        self.scheduler.invalidate_all();
        if let Some(gpu) = &self.gpu_window {
            gpu.window.request_redraw();
        }
    }

    fn remember(&mut self, face: Face) {
        let fonts = &mut self.atlas_state.fonts;
        if fonts.cached(face.name).is_none() {
            fonts.loaded.push(face);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_failed_older_read_leaves_the_newer_request_pending() {
        let mut app = crate::document::tests::app();
        app.atlas_state.fonts.pending = Some("Fira Code");
        app.install_editor_font("JetBrains Mono", None);
        assert_eq!(app.atlas_state.fonts.pending, Some("Fira Code"));
        app.install_editor_font("Fira Code", None);
        assert_eq!(app.atlas_state.fonts.pending, None);
    }
}
