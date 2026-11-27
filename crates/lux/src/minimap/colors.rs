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
    /// The pane's token generation (`PaneTokens::generation`), which
    /// changes on every replacement. The vector's address and length do
    /// not: a freed allocation can be reused with the same length.
    pub(crate) generation: u64,
    pub(crate) theme: &'static SyntaxTheme,
    pub(crate) buffer_id: u64,
}

impl LineColors for TokenColors<'_> {
    /// New tokens for the pane, a theme switch, or another document
    /// changes it. Never 0, which the model uses for "no colors".
    fn stamp(&self) -> u64 {
        let theme = std::ptr::from_ref(self.theme) as usize as u64;
        (self.generation.wrapping_mul(0x9E37_79B9_7F4A_7C15)
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

#[cfg(test)]
mod tests {
    use super::TokenColors;
    use crate::pane_view::PaneTokens;
    use lux_ui::minimap::LineColors;

    fn stamp(tokens: &PaneTokens) -> u64 {
        TokenColors {
            tokens: tokens.as_slice(),
            generation: tokens.generation(),
            theme: lux_syntax::theme::by_name("Monokai").expect("bundled theme"),
            buffer_id: 7,
        }
        .stamp()
    }

    /// The same (empty, unallocated) vector before and after: equal
    /// address and length, yet the stamp must change.
    #[test]
    fn replaced_tokens_change_the_stamp_even_at_the_same_address() {
        let mut tokens = PaneTokens::default();
        tokens.replace(Vec::new());
        let (before, ptr) = (stamp(&tokens), tokens.as_slice().as_ptr());
        tokens.replace(Vec::new());
        assert_eq!(tokens.as_slice().as_ptr(), ptr);
        assert_ne!(stamp(&tokens), before);
    }

    #[test]
    fn unchanged_tokens_keep_their_stamp() {
        let mut tokens = PaneTokens::default();
        tokens.replace(Vec::new());
        assert_eq!(stamp(&tokens), stamp(&tokens.clone()));
    }
}
