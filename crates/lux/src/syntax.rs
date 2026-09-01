//! Off-UI-thread syntax highlighting for the desktop shell (G04).
//!
//! Owns a [`BackgroundHighlighter`] when the open path maps to a known
//! language, submits parse jobs after buffer loads/edits, and drains
//! finished token lists via non-blocking `try_recv` so the UI thread never
//! waits on tree-sitter. Unknown extensions keep `None` — no invented
//! colors.

use std::path::Path;

use lux_syntax::theme::{Color as SyntaxColor, SyntaxTheme, LUX_DARK};
use lux_syntax::{BackgroundHighlighter, Job, LanguageId, Token};

/// Live highlight state for the current buffer. Absent when the path has
/// no grammar (honest fallback: paint with the chrome foreground).
pub struct SyntaxHighlight {
    worker: BackgroundHighlighter,
    tokens: Vec<Token>,
    theme: &'static SyntaxTheme,
    #[allow(dead_code)] // retained for diagnostics / future STATUS rows
    language: LanguageId,
}

impl SyntaxHighlight {
    /// Starts a background worker for `path`'s language, if we ship a
    /// grammar for its extension. Does not submit text yet — call
    /// [`Self::set_text`] after the buffer is loaded.
    pub fn for_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        let language = LanguageId::from_extension(ext)?;
        let worker = BackgroundHighlighter::spawn(language);
        println!("Lux: syntax highlighting ({language:?})");
        Some(Self {
            worker,
            tokens: Vec::new(),
            theme: &LUX_DARK,
            language,
        })
    }


    /// Queues a full reparse+highlight of `text` on the worker thread.
    pub fn set_text(&self, text: String) {
        self.worker.submit(Job::SetText(text));
    }

    /// Drains finished highlight results. Returns `true` when tokens
    /// changed (caller should request a redraw).
    pub fn poll(&mut self) -> bool {
        let mut updated = false;
        while let Some(tokens) = self.worker.try_recv() {
            self.tokens = tokens;
            updated = true;
        }
        updated
    }

    /// Theme color covering `byte`, or `None` when no token covers it
    /// (paint uses the chrome foreground in that case).
    pub fn color_at(&self, byte: usize) -> Option<SyntaxColor> {
        color_at_byte(&self.tokens, self.theme, byte)
    }
}

/// Resolves the most specific token covering `byte` and maps it through
/// `theme`. Exposed for deterministic tests without a GPU.
pub fn color_at_byte(
    tokens: &[Token],
    theme: &SyntaxTheme,
    byte: usize,
) -> Option<SyntaxColor> {
    let idx = tokens.partition_point(|t| t.end_byte <= byte);
    let mut best: Option<&Token> = None;
    for token in tokens.iter().skip(idx) {
        if token.start_byte > byte {
            break;
        }
        if token.start_byte <= byte && byte < token.end_byte {
            best = Some(match best {
                Some(prev) if span_len(token) < span_len(prev) => token,
                Some(prev) => prev,
                None => token,
            });
        }
    }
    // Also check a few tokens before partition_point — overlapping captures
    // can start earlier than the first with end_byte > byte.
    let start = idx.saturating_sub(8);
    for token in &tokens[start..idx] {
        if token.start_byte <= byte && byte < token.end_byte {
            best = Some(match best {
                Some(prev) if span_len(token) < span_len(prev) => token,
                Some(prev) => prev,
                None => token,
            });
        }
    }
    best.map(|t| theme.color_for_token(t))
}

fn span_len(token: &Token) -> usize {
    token.end_byte.saturating_sub(token.start_byte)
}

/// Converts a syntax theme color to the linear RGBA the glyph pipeline uses.
pub fn to_rgba(color: SyntaxColor) -> [f32; 4] {
    [
        color.r as f32 / 255.0,
        color.g as f32 / 255.0,
        color.b as f32 / 255.0,
        1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use lux_syntax::Document;

    #[test]
    fn rust_keyword_fn_gets_lux_dark_keyword_color() {
        let source = "fn main() {}\n";
        let mut doc = Document::new(LanguageId::Rust);
        doc.set_text(source);
        let tokens = doc.highlight(source);
        // Byte 0 of "fn" must be a keyword (or keyword.*) capture.
        let color = color_at_byte(&tokens, &LUX_DARK, 0).expect("fn is highlighted");
        assert_eq!(color, LUX_DARK.color_for_scope("keyword"));
    }

    #[test]
    fn rust_string_gets_lux_dark_string_color() {
        let source = "fn main() { let s = \"hi\"; }\n";
        let mut doc = Document::new(LanguageId::Rust);
        doc.set_text(source);
        let tokens = doc.highlight(source);
        let quote = source.find('"').expect("string literal");
        let color = color_at_byte(&tokens, &LUX_DARK, quote).expect("string is highlighted");
        assert_eq!(color, LUX_DARK.color_for_scope("string"));
    }

    #[test]
    fn unknown_extension_yields_no_highlighter() {
        assert!(SyntaxHighlight::for_path(Path::new("notes.txt")).is_none());
        assert!(SyntaxHighlight::for_path(Path::new("main.rs")).is_some());
    }

    #[test]
    fn background_worker_delivers_tokens_off_caller_thread() {
        let mut hl = SyntaxHighlight::for_path(Path::new("lib.rs")).expect("Rust");
        hl.set_text("fn answer() -> i32 { 42 }\n".into());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !hl.poll() {
            assert!(
                std::time::Instant::now() < deadline,
                "worker should finish without the caller blocking on recv"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(!hl.tokens.is_empty());
        assert_eq!(
            color_at_byte(&hl.tokens, &LUX_DARK, 0),
            Some(LUX_DARK.color_for_scope("keyword"))
        );
    }
}
