//! Keyboard focus order across chrome panels. F6 (and the API) cycles;
//! Escape returns to the editor. No trap: every panel is reachable and
//! the editor is always one step away.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusTarget {
    #[default]
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

    pub fn set_focus(&mut self, target: FocusTarget) {
        if let Some(idx) = ORDER.iter().position(|&t| t == target) {
            self.current = idx;
        }
    }

    pub fn is_focused(&self, target: FocusTarget) -> bool {
        self.current() == target
    }

    pub fn focus_next_visible(&mut self, is_visible: impl Fn(FocusTarget) -> bool) -> FocusTarget {
        let initial = self.current;
        for step in 1..=ORDER.len() {
            let candidate = (initial + step) % ORDER.len();
            if is_visible(ORDER[candidate]) {
                self.current = candidate;
                return self.current();
            }
        }
        self.current()
    }

    pub fn focus_prev_visible(&mut self, is_visible: impl Fn(FocusTarget) -> bool) -> FocusTarget {
        let initial = self.current;
        for step in 1..=ORDER.len() {
            let candidate = (initial + ORDER.len() * 2 - step) % ORDER.len();
            if is_visible(ORDER[candidate]) {
                self.current = candidate;
                return self.current();
            }
        }
        self.current()
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

    #[test]
    fn f6_skips_hidden_panels() {
        let mut ring = FocusRing::new();
        // Only Editor, FileTree, Terminal are visible
        let visible = |t| {
            matches!(
                t,
                FocusTarget::Editor | FocusTarget::FileTree | FocusTarget::Terminal
            )
        };

        assert_eq!(ring.current(), FocusTarget::Editor);
        assert_eq!(ring.focus_next_visible(visible), FocusTarget::FileTree);
        assert_eq!(ring.focus_next_visible(visible), FocusTarget::Terminal);
        assert_eq!(ring.focus_next_visible(visible), FocusTarget::Editor);

        // Backward
        assert_eq!(ring.focus_prev_visible(visible), FocusTarget::Terminal);
        assert_eq!(ring.focus_prev_visible(visible), FocusTarget::FileTree);
        assert_eq!(ring.focus_prev_visible(visible), FocusTarget::Editor);
    }

    #[test]
    fn set_focus_and_is_focused() {
        let mut ring = FocusRing::new();
        assert!(ring.is_focused(FocusTarget::Editor));
        ring.set_focus(FocusTarget::Terminal);
        assert!(ring.is_focused(FocusTarget::Terminal));
        assert_eq!(ring.current(), FocusTarget::Terminal);
    }
}
