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

/// A face read on a worker, ready to register on the UI thread.
pub(crate) struct FallbackFace {
    /// File path; also the dedupe key in the layout context.
    pub(crate) key: String,
    /// Moved into the layout context's collection on install: the only
    /// copy of the face kept in memory.
    pub(crate) bytes: Vec<u8>,
}

type Reader<'a> = &'a dyn Fn(&Path) -> std::io::Result<Vec<u8>>;

/// Worker side: resolves each character to a face file with `locate`, then
/// reads each distinct file once with `read`. Files in `registered` (keys
/// the layout context already holds) are not read again. Characters the
/// system cannot draw either (CoreText answers with LastResort) produce
/// nothing.
fn load_faces(
    chars: &[char],
    registered: &[String],
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
        let key = path.display().to_string();
        if registered.contains(&key) {
            continue;
        }
        match read(&path) {
            Ok(bytes) => faces.push(FallbackFace { key, bytes }),
            Err(error) => {
                eprintln!("Lux: fallback face {} unreadable: {error}", path.display());
            }
        }
    }
    faces
}

/// UI side: registers faces a worker returned. No disk access.
pub(crate) fn install(layout: &mut lux_render::TextLayoutContext, faces: Vec<FallbackFace>) {
    for face in faces {
        if !layout.register_fallback_face(&face.key, face.bytes) {
            eprintln!("Lux: fallback face {} did not parse", face.key);
        }
    }
}

/// Characters to resolve, built on the UI thread and run on a worker.
pub(crate) struct Job {
    chars: Vec<char>,
    /// Keys of faces already registered when the job was built.
    registered: Vec<String>,
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
        load_faces(
            &self.chars,
            &self.registered,
            &|ch| locate(&self.family, ch),
            read,
        )
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
        registered: layout.fallback_face_keys().map(str::to_owned).collect(),
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
