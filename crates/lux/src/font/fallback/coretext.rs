//! The CoreText calls behind `locate`. Workers only: CoreText may read
//! font files while it answers.

use std::ffi::c_void;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

type CFTypeRef = *const c_void;
type CFIndex = isize;

#[repr(C)]
struct CFRange {
    location: CFIndex,
    length: CFIndex,
}

const UTF8: u32 = 0x0800_0100;
/// `kCTFontUIFontUserFixedPitch`: the user's fixed-pitch font (Menlo or SF
/// Mono).
const USER_FIXED_PITCH: u32 = 1;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithBytes(
        alloc: CFTypeRef,
        bytes: *const u8,
        len: CFIndex,
        encoding: u32,
        external: u8,
    ) -> CFTypeRef;
    fn CFStringGetLength(string: CFTypeRef) -> CFIndex;
    fn CFStringGetCString(string: CFTypeRef, buf: *mut u8, size: CFIndex, encoding: u32) -> u8;
    fn CFURLGetFileSystemRepresentation(
        url: CFTypeRef,
        resolve: u8,
        buf: *mut u8,
        len: CFIndex,
    ) -> u8;
    fn CFRelease(object: CFTypeRef);
}

#[link(name = "CoreText", kind = "framework")]
unsafe extern "C" {
    static kCTFontURLAttribute: CFTypeRef;
    fn CTFontCreateWithName(name: CFTypeRef, size: f64, matrix: *const c_void) -> CFTypeRef;
    fn CTFontCreateUIFontForLanguage(kind: u32, size: f64, language: CFTypeRef) -> CFTypeRef;
    fn CTFontCopyFamilyName(font: CFTypeRef) -> CFTypeRef;
    fn CTFontCreateForString(font: CFTypeRef, string: CFTypeRef, range: CFRange) -> CFTypeRef;
    fn CTFontGetGlyphsForCharacters(
        font: CFTypeRef,
        chars: *const u16,
        glyphs: *mut u16,
        count: CFIndex,
    ) -> u8;
    fn CTFontCopyPostScriptName(font: CFTypeRef) -> CFTypeRef;
    fn CTFontCopyAttribute(font: CFTypeRef, attribute: CFTypeRef) -> CFTypeRef;
}

/// Owns one +1 CoreFoundation reference.
struct Owned(CFTypeRef);

impl Owned {
    fn new(raw: CFTypeRef) -> Option<Self> {
        (!raw.is_null()).then_some(Self(raw))
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a non-null object this wrapper owns once.
        unsafe { CFRelease(self.0) }
    }
}

fn cf_string(text: &str) -> Option<Owned> {
    // SAFETY: `text` is valid UTF-8 for `len` bytes; the call copies it.
    Owned::new(unsafe {
        CFStringCreateWithBytes(
            std::ptr::null(),
            text.as_ptr(),
            text.len() as CFIndex,
            UTF8,
            0,
        )
    })
}

fn rust_string(string: &Owned) -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: `buf` is writable for its length; CoreFoundation writes a
    // NUL-terminated string into it or fails.
    let ok = unsafe { CFStringGetCString(string.0, buf.as_mut_ptr(), buf.len() as CFIndex, UTF8) };
    if ok == 0 {
        return None;
    }
    let end = buf.iter().position(|b| *b == 0)?;
    String::from_utf8(buf[..end].to_vec()).ok()
}

/// The font CoreText starts its cascade from: `family` when it is an
/// installed family. A face Lux registered from its own file (a bundled
/// font) is unknown to CoreText, which would then answer with some
/// unrelated default; the system monospace font stands in for it.
fn base_font(family: &str) -> Option<Owned> {
    if let Some(name) = cf_string(family) {
        // SAFETY: `name` is a live CFString; a null matrix means identity.
        let font = Owned::new(unsafe { CTFontCreateWithName(name.0, 16.0, std::ptr::null()) });
        if let Some(font) = font {
            // SAFETY: `font` is live; the copy follows the Create rule.
            let found = Owned::new(unsafe { CTFontCopyFamilyName(font.0) })
                .and_then(|name| rust_string(&name));
            if found.is_some_and(|found| found.eq_ignore_ascii_case(family)) {
                return Some(font);
            }
        }
    }
    // SAFETY: a constant font kind; a null language means the user's.
    Owned::new(unsafe { CTFontCreateUIFontForLanguage(USER_FIXED_PITCH, 16.0, std::ptr::null()) })
}

/// The file of the face CoreText picks to draw `ch` after `family`, or
/// `None` when no installed face has a glyph for it.
pub(super) fn face_path_for(family: &str, ch: char) -> Option<PathBuf> {
    let base = base_font(family)?;
    let mut utf8 = [0u8; 4];
    let text = cf_string(ch.encode_utf8(&mut utf8))?;
    // SAFETY: `text` is a live CFString.
    let length = unsafe { CFStringGetLength(text.0) };
    let range = CFRange {
        location: 0,
        length,
    };
    // SAFETY: both objects are live and `range` lies inside `text`.
    let font = Owned::new(unsafe { CTFontCreateForString(base.0, text.0, range) })?;

    let mut units = [0u16; 2];
    let units = ch.encode_utf16(&mut units);
    let mut glyphs = [0u16; 2];
    // SAFETY: both buffers hold `units.len()` (1 or 2) elements.
    let covered = unsafe {
        CTFontGetGlyphsForCharacters(
            font.0,
            units.as_ptr(),
            glyphs.as_mut_ptr(),
            units.len() as CFIndex,
        )
    };
    if covered == 0 {
        return None;
    }
    // SAFETY: `font` is live; the copy follows the Create rule.
    let postscript = Owned::new(unsafe { CTFontCopyPostScriptName(font.0) })?;
    if rust_string(&postscript).as_deref() == Some("LastResort") {
        return None;
    }
    // SAFETY: `font` is live and the attribute key is a CoreText constant.
    let url = Owned::new(unsafe { CTFontCopyAttribute(font.0, kCTFontURLAttribute) })?;
    let mut path = [0u8; 1024];
    // SAFETY: `path` is writable for its length.
    let ok = unsafe {
        CFURLGetFileSystemRepresentation(url.0, 1, path.as_mut_ptr(), path.len() as CFIndex)
    };
    if ok == 0 {
        return None;
    }
    let end = path.iter().position(|b| *b == 0)?;
    Some(PathBuf::from(std::ffi::OsStr::from_bytes(&path[..end])))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn family_of(font: &Owned) -> Option<String> {
        // SAFETY: `font` is live; the copy follows the Create rule.
        Owned::new(unsafe { CTFontCopyFamilyName(font.0) }).and_then(|name| rust_string(&name))
    }

    #[test]
    fn an_unknown_family_cascades_from_the_system_monospace_font() {
        let system = base_font("")
            .and_then(|f| family_of(&f))
            .expect("system monospace");
        let unknown = base_font("Lux Bundled Font That Is Not Installed")
            .and_then(|f| family_of(&f))
            .expect("a base font");
        assert_eq!(unknown, system);
        let menlo = base_font("Menlo").and_then(|f| family_of(&f));
        assert_eq!(menlo.as_deref(), Some("Menlo"));
        assert!(face_path_for("Lux Bundled Font That Is Not Installed", 'ส').is_some());
    }
}
