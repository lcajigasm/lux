use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    Tabs,
    Spaces,
}

/// Detected indentation style and width, e.g. "4 spaces" or "tabs".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Indent {
    pub style: IndentStyle,
    pub width: u8,
}

impl Indent {
    /// Detects indent style by counting lines that start with a tab vs.
    /// lines that start with spaces, and detects the space width by voting
    /// on the most common increase in leading-space count between a line and
    /// the previous indented line (the classic "guess indentation" heuristic
    /// editors use). Defaults to 4 spaces when nothing indented is found.
    pub fn detect(text: &str) -> Indent {
        let mut tabs = 0usize;
        let mut spaces = 0usize;
        let mut width_votes: HashMap<usize, usize> = HashMap::new();
        let mut prev_space_indent = 0usize;

        for line in text.lines() {
            if line.starts_with('\t') {
                tabs += 1;
                prev_space_indent = 0;
                continue;
            }
            let leading_spaces = line.chars().take_while(|&c| c == ' ').count();
            if leading_spaces == 0 {
                prev_space_indent = 0;
                continue;
            }
            spaces += 1;
            let delta = leading_spaces.saturating_sub(prev_space_indent);
            if delta > 0 {
                *width_votes.entry(delta).or_insert(0) += 1;
            }
            prev_space_indent = leading_spaces;
        }

        let style = if tabs >= spaces && tabs > 0 {
            IndentStyle::Tabs
        } else {
            IndentStyle::Spaces
        };
        let width = match style {
            IndentStyle::Tabs => 1,
            IndentStyle::Spaces => width_votes
                .into_iter()
                .max_by_key(|&(_, count)| count)
                .map_or(4, |(width, _)| width as u8),
        };
        Indent { style, width }
    }

    /// True when at least one line has leading whitespace, so [`detect`]
    /// saw a real sample. A flat file is false and the editor may use the
    /// indent settings instead.
    pub fn has_sample(text: &str) -> bool {
        text.lines()
            .any(|line| matches!(line.chars().next(), Some(' ' | '\t')))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tabs() {
        let text = "fn main() {\n\tlet x = 1;\n\tlet y = 2;\n}\n";
        let indent = Indent::detect(text);
        assert_eq!(indent.style, IndentStyle::Tabs);
        assert_eq!(indent.width, 1);
    }

    #[test]
    fn detects_four_spaces() {
        let text = "fn main() {\n    let x = 1;\n    let y = 2;\n}\n";
        let indent = Indent::detect(text);
        assert_eq!(indent.style, IndentStyle::Spaces);
        assert_eq!(indent.width, 4);
    }

    #[test]
    fn detects_two_spaces() {
        let text = "if true {\n  a();\n  b();\n}\n";
        let indent = Indent::detect(text);
        assert_eq!(indent.style, IndentStyle::Spaces);
        assert_eq!(indent.width, 2);
    }

    #[test]
    fn detects_nested_indent_delta() {
        let text = "a\n  b\n    c\n  d\n";
        let indent = Indent::detect(text);
        assert_eq!(indent.style, IndentStyle::Spaces);
        assert_eq!(indent.width, 2);
    }

    #[test]
    fn defaults_to_four_spaces_when_flat() {
        let indent = Indent::detect("a\nb\nc\n");
        assert_eq!(indent.style, IndentStyle::Spaces);
        assert_eq!(indent.width, 4);
    }
}
