//! Keyboard focus order across chrome panels. F6 (and the API) cycles;
//! Escape returns to the editor. No trap: every panel is reachable and
//! the editor is always one step away.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Editor,
    CommandPalette,
    Search,
    FileTree,
    Git,
    Terminal,
    Problems,
}

const ORDER: &[FocusTarget] = &[
    FocusTarget::Editor,
    FocusTarget::CommandPalette,
    FocusTarget::Search,
    FocusTarget::FileTree,
    FocusTarget::Git,
    FocusTarget::Terminal,
    FocusTarget::Problems,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusRing {
    current: usize,
}

impl Default for FocusRing {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusRing {
    pub fn new() -> Self {
        Self { current: 0 }
    }

    pub fn current(&self) -> FocusTarget {
        ORDER[self.current]
    }

    pub fn focus_next(&mut self) -> FocusTarget {
        self.current = (self.current + 1) % ORDER.len();
        self.current()
    }

    pub fn focus_prev(&mut self) -> FocusTarget {
        self.current = (self.current + ORDER.len() - 1) % ORDER.len();
        self.current()
    }

    pub fn escape_to_editor(&mut self) -> FocusTarget {
        self.current = 0;
        FocusTarget::Editor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f6_reaches_every_panel_and_escape_untraps() {
        let mut ring = FocusRing::new();
        assert_eq!(ring.current(), FocusTarget::Editor);
        let mut seen = vec![ring.current()];
        for _ in 0..ORDER.len() - 1 {
            seen.push(ring.focus_next());
        }
        assert_eq!(seen.len(), ORDER.len());
        assert_eq!(ring.focus_next(), FocusTarget::Editor);
        ring.focus_next();
        assert_ne!(ring.current(), FocusTarget::Editor);
        assert_eq!(ring.escape_to_editor(), FocusTarget::Editor);
    }
}
