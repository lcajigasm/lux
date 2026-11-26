//! Dynamic font fallback: a face per character no registered face covers.
//!
//! The fixed coverage faces (`super::COVERAGE_FACES`) handle Han, kana,
//! Hangul and color emoji. Anything else (Thai, Arabic, Hebrew, Devanagari,
//! Ethiopic, rare symbols, math) shapes to `.notdef`. Layout records those
//! characters; after a frame, the UI thread hands them to a worker, which
//! asks CoreText for the face macOS itself would use
//! (`CTFontCreateForString` on the editor font), reads that face's file,
//! and sends the bytes back through `crate::ui_io`. The UI thread registers
//! them and repaints. Until then the characters draw as the primary face's
//! missing-glyph box.
//!
//! The editor reshapes every visible line each frame and keeps no layout
//! cache, so the repaint after registration is the re-layout: lines that
//! scrolled out are shaped with the new face when they come back.
//!
//! The UI thread never reads a font file and never calls CoreText here.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A face read on a worker, ready to register on the UI thread.
pub(crate) struct FallbackFace {
    /// File path; also the dedupe key in the layout context.
    pub(crate) key: String,
    pub(crate) bytes: Arc<Vec<u8>>,
}

type Reader<'a> = &'a dyn Fn(&Path) -> std::io::Result<Vec<u8>>;

/// Faces already read, by path, so another window or a later batch does not
/// read the same file again.
static READ: Mutex<Vec<(PathBuf, Arc<Vec<u8>>)>> = Mutex::new(Vec::new());

/// Worker side: resolves each character to a face file with `locate`, then
/// reads each distinct file once with `read`. Characters the system cannot
/// draw either (CoreText answers with LastResort) produce nothing.
fn load_faces(
    chars: &[char],
    locate: &dyn Fn(char) -> Option<PathBuf>,
    read: Reader<'_>,
) -> Vec<FallbackFace> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for &ch in chars {
        if let Some(path) = locate(ch)
            && !paths.contains(&path)
        {
            paths.push(path);
        }
    }
    let mut faces = Vec::new();
    for path in paths {
        let cached = READ.lock().ok().and_then(|read| {
            read.iter()
                .find(|(p, _)| *p == path)
                .map(|(_, b)| b.clone())
        });
        let bytes = match cached {
            Some(bytes) => bytes,
            None => match read(&path) {
                Ok(bytes) => {
                    let bytes = Arc::new(bytes);
                    if let Ok(mut cache) = READ.lock() {
                        cache.push((path.clone(), bytes.clone()));
                    }
                    bytes
                }
                Err(error) => {
                    eprintln!("Lux: fallback face {} unreadable: {error}", path.display());
                    continue;
                }
            },
        };
        faces.push(FallbackFace {
            key: path.display().to_string(),
            bytes,
        });
    }
    faces
}

/// UI side: registers faces a worker returned. No disk access.
pub(crate) fn install(layout: &mut lux_render::TextLayoutContext, faces: &[FallbackFace]) {
    for face in faces {
        if !layout.register_fallback_face(&face.key, &face.bytes) {
            eprintln!("Lux: fallback face {} did not parse", face.key);
        }
    }
}

/// Characters to resolve, built on the UI thread and run on a worker.
pub(crate) struct Job {
    chars: Vec<char>,
    family: String,
}

impl Job {
    /// The editor family CoreText starts its fallback search from.
    pub(crate) fn with_family(mut self, family: &str) -> Self {
        family.clone_into(&mut self.family);
        self
    }

    /// Worker side. `read` is `std::fs::read` in the app.
    pub(crate) fn run(self, read: Reader<'_>) -> Vec<FallbackFace> {
        load_faces(&self.chars, &|ch| locate(&self.family, ch), read)
    }
}

/// UI side, after each frame: the characters that newly shaped to
/// `.notdef`, as a job for a worker. Waits for the coverage faces first,
/// so a Han or emoji character on the first frame does not trigger a
/// second read of a face that is already on its way. Never touches disk.
pub(crate) fn take_job(layout: &mut lux_render::TextLayoutContext) -> Option<Job> {
    if !cfg!(target_os = "macos") || !layout.has_coverage_fonts() {
        return None;
    }
    let chars = layout.take_missing_chars();
    (!chars.is_empty()).then(|| Job {
        chars,
        family: String::new(),
    })
}

#[cfg(target_os = "macos")]
fn locate(family: &str, ch: char) -> Option<PathBuf> {
    coretext::face_path_for(family, ch)
}

#[cfg(not(target_os = "macos"))]
fn locate(_family: &str, _ch: char) -> Option<PathBuf> {
    None
}

#[cfg(target_os = "macos")]
mod coretext;

#[cfg(test)]
mod tests;
