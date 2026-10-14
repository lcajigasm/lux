//! RTL: Hebrew/Arabic (and related) first-strong characters flip layout.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
}

pub fn direction_for(text: &str) -> Direction {
    for c in text.chars() {
        match c {
            '\u{0590}'..='\u{05FF}'
            | '\u{0600}'..='\u{06FF}'
            | '\u{0700}'..='\u{074F}'
            | '\u{0750}'..='\u{077F}'
            | '\u{08A0}'..='\u{08FF}'
            | '\u{FB50}'..='\u{FDFF}'
            | '\u{FE70}'..='\u{FEFF}' => return Direction::Rtl,
            'A'..='Z' | 'a'..='z' => return Direction::Ltr,
            _ => {}
        }
    }
    Direction::Ltr
}

/// Logical column 0 is the start of the line: left in LTR, right in RTL.
pub fn start_edge_x(direction: Direction, viewport_width: f32) -> f32 {
    match direction {
        Direction::Ltr => 0.0,
        Direction::Rtl => viewport_width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hebrew_and_arabic_are_rtl_latin_is_ltr() {
        assert_eq!(direction_for("שלום"), Direction::Rtl);
        assert_eq!(direction_for("مرحبا"), Direction::Rtl);
        assert_eq!(direction_for("hello"), Direction::Ltr);
        assert_eq!(start_edge_x(Direction::Rtl, 100.0), 100.0);
        assert_eq!(start_edge_x(Direction::Ltr, 100.0), 0.0);
    }

    #[test]
    fn mixed_bidirectional_text() {
        // Leading RTL character determines line direction
        assert_eq!(direction_for("שלום world 123"), Direction::Rtl);
        assert_eq!(direction_for("123 مرحبا"), Direction::Rtl);
        // Leading LTR character
        assert_eq!(direction_for("hello שלום"), Direction::Ltr);
    }
}
