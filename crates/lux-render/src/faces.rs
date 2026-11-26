//! Fallback faces registered after startup.
//!
//! Two kinds:
//! - Coverage faces (Han, kana, Hangul, color emoji) known ahead of time,
//!   keyed by script or by the emoji generic family.
//! - Dynamic faces, chosen by the platform for characters no registered
//!   face covers. Layout records those characters (they shape to glyph 0,
//!   `.notdef`); the app asks the OS for a face per character on a worker,
//!   reads it there, and registers the bytes here. Dynamic faces follow the
//!   primary family in every line's family list, so they apply whatever
//!   script the run resolves to (symbols and math are script Common and
//!   would miss a script-keyed fallback).

use std::borrow::Cow;
use std::collections::BTreeSet;

use fontique::Blob;
use parley::{FontFamily, FontFamilyName, Layout, PositionedLayoutItem};

use crate::text_layout::{NoBrush, TextLayoutContext};

/// Characters seen as `.notdef` and the dynamic faces registered so far.
#[derive(Default)]
pub(crate) struct DynamicFallback {
    /// Family names in registration order, appended after the primary.
    families: Vec<String>,
    /// Keys (file paths) of faces already registered, so one face is read
    /// into the collection once.
    keys: Vec<String>,
    /// Every character already reported; never reported twice.
    seen: BTreeSet<char>,
    /// Reported, not yet taken by the app.
    pending: Vec<char>,
    coverage: bool,
}

impl DynamicFallback {
    /// The family value for one line: the primary alone, or the primary
    /// followed by every dynamic face.
    pub(crate) fn family<'a>(&'a self, primary: &'a str) -> FontFamily<'a> {
        if self.families.is_empty() {
            return FontFamily::named(primary);
        }
        let mut list = Vec::with_capacity(self.families.len() + 1);
        list.push(FontFamilyName::named(primary));
        list.extend(self.families.iter().map(|f| FontFamilyName::named(f)));
        FontFamily::List(Cow::Owned(list))
    }

    /// Records non-emoji characters that shaped to glyph 0. ASCII lines are
    /// skipped: the primary monospace face always covers them.
    pub(crate) fn scan(&mut self, layout: &Layout<NoBrush>, text: &str) {
        if text.is_ascii() {
            return;
        }
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                for cluster in run.run().clusters() {
                    if cluster.is_emoji() || cluster.glyphs().any(|g| g.id != 0) {
                        continue;
                    }
                    let ch = cluster.source_char();
                    if ch.is_whitespace() || ch.is_control() || !self.seen.insert(ch) {
                        continue;
                    }
                    self.pending.push(ch);
                }
            }
        }
    }
}

impl TextLayoutContext {
    /// Registers another face and appends it as a script or emoji fallback.
    /// The primary monospace file stays the default; this only fills gaps
    /// (Han, Hiragana, Katakana, Hangul, emoji) it cannot draw.
    ///
    /// Characters reported missing before this face arrived are forgotten,
    /// so the next layout reports only what is still uncovered.
    pub fn register_coverage_font(
        &mut self,
        font_bytes: &[u8],
        scripts: &[[u8; 4]],
        emoji: bool,
    ) -> bool {
        let blob = Blob::from(font_bytes.to_vec());
        let registered = self.font_context.collection.register_fonts(blob, None);
        if registered.is_empty() {
            return false;
        }
        let ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
        for script in scripts {
            let key = fontique::FallbackKey::new(fontique::Script::from_bytes(*script), None);
            self.font_context
                .collection
                .append_fallbacks(key, ids.iter().copied());
        }
        if emoji {
            self.font_context
                .collection
                .append_generic_families(fontique::GenericFamily::Emoji, ids.into_iter());
        }
        let fallback = &mut self.fallback;
        fallback.coverage = true;
        fallback.seen.clear();
        fallback.pending.clear();
        true
    }

    /// Whether any coverage face has been registered. Until then, missing
    /// characters may still be covered by a face that is being read.
    pub fn has_coverage_fonts(&self) -> bool {
        self.fallback.coverage
    }

    /// Characters that shaped to `.notdef` since the last call, each
    /// reported once for the life of this context.
    pub fn take_missing_chars(&mut self) -> Vec<char> {
        std::mem::take(&mut self.fallback.pending)
    }

    /// Registers a face the platform chose for missing characters and puts
    /// its families after the primary in every later layout. `key` (the
    /// file path) dedupes: a face already registered is not added again.
    /// Returns whether the face is now available.
    pub fn register_fallback_face(&mut self, key: &str, font_bytes: &[u8]) -> bool {
        if self.fallback.keys.iter().any(|k| k == key) {
            return true;
        }
        let blob = Blob::from(font_bytes.to_vec());
        let registered = self.font_context.collection.register_fonts(blob, None);
        if registered.is_empty() {
            return false;
        }
        for (id, _) in registered {
            let Some(name) = self.font_context.collection.family_name(id) else {
                continue;
            };
            if !self.fallback.families.iter().any(|f| f == name) {
                self.fallback.families.push(name.to_owned());
            }
        }
        self.fallback.keys.push(key.to_owned());
        true
    }
}

/// Keeps one process-lifetime copy of a shaped font's bytes so
/// [`swash::FontRef`] can outlive the `parley` layout that owned the blob.
pub(crate) fn leaked_font_bytes(blob: &Blob<u8>) -> &'static [u8] {
    use std::sync::Mutex;
    static CACHE: Mutex<Vec<(u64, &'static [u8])>> = Mutex::new(Vec::new());
    let id = blob.id();
    let mut cache = CACHE.lock().expect("font byte cache");
    if let Some((_, bytes)) = cache.iter().find(|(cached, _)| *cached == id) {
        return bytes;
    }
    let owned = blob.clone();
    let ptr = owned.data().as_ptr();
    let len = owned.data().len();
    // The forgotten Arc is never dropped, so this allocation stays for the
    // process. That is the same lifetime the primary monospace face already
    // has (`Box::leak` in `App::new`).
    std::mem::forget(owned);
    // SAFETY: `owned` was the last Arc we hold and it was forgotten, so the
    // allocation `ptr` addresses is not freed. `len` is that allocation's
    // length. The slice is therefore valid for the rest of the process.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    cache.push((id, bytes));
    bytes
}

#[cfg(test)]
mod tests;
