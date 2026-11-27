//! The editor's highlight tokens as minimap stroke colors. Reads the tokens
//! the background highlighter already produced; never parses.

use crate::app::SyntaxState;
use lux_syntax::Token;
use lux_syntax::theme::SyntaxTheme;
use lux_ui::minimap::{ColorRun, LineColors};

pub(crate) struct SyntaxColors<'a>(pub(crate) &'a SyntaxState);

/// A split group's cached tokens, for a group that is not the focused
/// one and so has no live highlighter of its own.
pub(crate) struct TokenColors<'a> {
    pub(crate) tokens: &'a [Token],
    pub(crate) theme: &'static SyntaxTheme,
    pub(crate) buffer_id: u64,
}

impl LineColors for TokenColors<'_> {
    /// A group's tokens are replaced wholesale (a new allocation) when it
    /// is laid out or loses focus, so their address and length change.
    fn stamp(&self) -> u64 {
        let tokens = self.tokens.as_ptr() as usize as u64;
        let theme = std::ptr::from_ref(self.theme) as usize as u64;
        (tokens.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (self.tokens.len() as u64).rotate_left(17)
            ^ theme
            ^ self.buffer_id.rotate_left(32))
            | 1
    }

    fn runs(&self, start: usize, end: usize, out: &mut Vec<ColorRun>) {
        push_runs(self.tokens, self.theme, start, end, out);
    }
}

fn push_runs(
    tokens: &[Token],
    theme: &SyntaxTheme,
    start: usize,
    end: usize,
    out: &mut Vec<ColorRun>,
) {
    let first = tokens.partition_point(|token| token.end_byte <= start);
    for token in tokens[first..]
        .iter()
        .take_while(|token| token.start_byte < end)
    {
        let c = theme.color_for_token(token);
        out.push(ColorRun {
            start: token.start_byte,
            end: token.end_byte,
            rgb: [c.r, c.g, c.b],
        });
    }
}

impl LineColors for SyntaxColors<'_> {
    /// A new parse result, a theme switch, or another document changes it.
    /// Never 0, which the model uses for "no colors".
    fn stamp(&self) -> u64 {
        let theme = std::ptr::from_ref(self.0.theme) as usize as u64;
        (self.0.generation.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ theme
            ^ self.0.buffer_id.rotate_left(32))
            | 1
    }

    fn runs(&self, start: usize, end: usize, out: &mut Vec<ColorRun>) {
        push_runs(&self.0.tokens, self.0.theme, start, end, out);
    }
}
