//! Reduced-motion preference: when set, the UI must not blink or animate.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Motion {
    pub reduced: bool,
}

impl Motion {
    pub fn cursor_blinks(self) -> bool {
        !self.reduced
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduced_motion_disables_cursor_blink() {
        assert!(Motion { reduced: false }.cursor_blinks());
        assert!(!Motion { reduced: true }.cursor_blinks());
    }
}
