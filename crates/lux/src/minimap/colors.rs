//! The editor's highlight tokens as minimap stroke colors. Reads the tokens
//! the background highlighter already produced; never parses.

use crate::app::SyntaxState;
use lux_ui::minimap::{ColorRun, LineColors};

pub(crate) struct SyntaxColors<'a>(pub(crate) &'a SyntaxState);

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
        let tokens = &self.0.tokens;
        let first = tokens.partition_point(|token| token.end_byte <= start);
        for token in tokens[first..]
            .iter()
            .take_while(|token| token.start_byte < end)
        {
            let c = self.0.theme.color_for_token(token);
            out.push(ColorRun {
                start: token.start_byte,
                end: token.end_byte,
                rgb: [c.r, c.g, c.b],
            });
        }
    }
}
