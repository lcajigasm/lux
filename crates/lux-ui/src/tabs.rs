//! Tabs (`LUX_PROMPT_MAESTRO.md` Phase 3 deliverable 9, tab half):
//! reordering (the primitive a drag & drop UI moves a tab with), pinning,
//! closing, and reopening the most recently closed tab. No interactive UI
//! exists yet in `lux-ui` (same state as every other Phase 3 deliverable
//! so far), so this is the state machine a tab strip widget would drive —
//! mouse/drag event handling itself is separate, not-started work.

/// Opaque handle to an open tab. Minted by [`TabBar::open`]; never
/// caller-chosen, so two tabs can never collide on id by construction —
/// unlike `command_palette::Command`, a tab id has no need to be a stable,
/// externally-referenced string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(u64);

/// One open tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub pinned: bool,
}

/// A strip of tabs: display order (what drag & drop reorders), which one
/// is active, and a LIFO stack of closed tabs for "reopen last closed".
#[derive(Debug, Default)]
pub struct TabBar {
    tabs: Vec<Tab>,
    active: Option<TabId>,
    closed_stack: Vec<Tab>,
    next_id: u64,
    scroll: f32,
}

impl TabBar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Every open tab, in display order.
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    pub fn set_scroll(&mut self, value: f32) {
        self.scroll = value.max(0.0);
    }

    pub fn active(&self) -> Option<&Tab> {
        self.active.and_then(|id| self.find(id))
    }

    fn find(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    /// Opens a new, unpinned tab at the end of the strip and activates it.
    pub fn open(&mut self, title: impl Into<String>) -> TabId {
        let id = TabId(self.next_id);
        self.next_id += 1;
        self.tabs.push(Tab {
            id,
            title: title.into(),
            pinned: false,
        });
        self.active = Some(id);
        id
    }

    /// Closes `id`, pushing it onto the reopen stack. If it was the active
    /// tab, activates its right neighbor, or its left neighbor if it was
    /// last, or nothing if it was the only tab. `false` if `id` isn't open.
    pub fn close(&mut self, id: TabId) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        let removed = self.tabs.remove(index);
        if self.active == Some(id) {
            self.active = self
                .tabs
                .get(index)
                .or_else(|| index.checked_sub(1).and_then(|i| self.tabs.get(i)))
                .map(|t| t.id);
        }
        self.closed_stack.push(removed);
        true
    }

    /// Reopens the most recently closed tab (LIFO — closing A then B and
    /// reopening twice gives B back first, then A), appended at the end of
    /// the strip and activated. `None` if nothing has been closed.
    pub fn reopen_last_closed(&mut self) -> Option<TabId> {
        let tab = self.closed_stack.pop()?;
        let id = tab.id;
        self.tabs.push(tab);
        self.active = Some(id);
        Some(id)
    }

    /// Pins `id`, moving it to the end of the already-pinned group (so
    /// pinned tabs stay clustered at the start of the strip in the order
    /// they were pinned). `false` if `id` isn't open.
    pub fn pin(&mut self, id: TabId) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        if self.tabs[index].pinned {
            return true;
        }
        let mut tab = self.tabs.remove(index);
        tab.pinned = true;
        let insert_at = self.tabs.iter().take_while(|t| t.pinned).count();
        self.tabs.insert(insert_at, tab);
        true
    }

    /// Unpins `id`, moving it to the start of the unpinned group (right
    /// after whatever's still pinned). `false` if `id` isn't open.
    pub fn unpin(&mut self, id: TabId) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        if !self.tabs[index].pinned {
            return true;
        }
        let mut tab = self.tabs.remove(index);
        tab.pinned = false;
        let insert_at = self.tabs.iter().take_while(|t| t.pinned).count();
        self.tabs.insert(insert_at, tab);
        true
    }

    /// Moves `id` to `to_index` in the strip — the primitive a drag & drop
    /// gesture reports its drop position through. Doesn't enforce the
    /// pinned/unpinned grouping `pin`/`unpin` maintain; a manual drag can
    /// freely interleave, same as most editors allow. `false` if `id`
    /// isn't open.
    pub fn move_tab(&mut self, id: TabId, to_index: usize) -> bool {
        let Some(from) = self.index_of(id) else {
            return false;
        };
        let to_index = to_index.min(self.tabs.len() - 1);
        let tab = self.tabs.remove(from);
        self.tabs.insert(to_index, tab);
        true
    }

    /// Activates `id` (e.g. clicking it). `false` if `id` isn't open.
    pub fn activate(&mut self, id: TabId) -> bool {
        if self.find(id).is_none() {
            return false;
        }
        self.active = Some(id);
        true
    }

    /// Updates the title of tab `id`. Returns `false` if `id` isn't open.
    pub fn set_title(&mut self, id: TabId, title: impl Into<String>) -> bool {
        let Some(index) = self.index_of(id) else {
            return false;
        };
        self.tabs[index].title = title.into();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(bar: &TabBar) -> Vec<&str> {
        bar.tabs().iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn open_appends_and_activates() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        let b = bar.open("b.rs");
        assert_eq!(titles(&bar), vec!["a.rs", "b.rs"]);
        assert_eq!(bar.active().unwrap().id, b);
        assert_ne!(a, b);
    }

    #[test]
    fn close_activates_the_right_neighbor() {
        let mut bar = TabBar::new();
        bar.open("a.rs");
        let b = bar.open("b.rs");
        let c = bar.open("c.rs");
        bar.activate(b);
        assert!(bar.close(b));
        assert_eq!(titles(&bar), vec!["a.rs", "c.rs"]);
        assert_eq!(bar.active().unwrap().id, c);
    }

    #[test]
    fn close_last_tab_activates_the_left_neighbor() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        let b = bar.open("b.rs");
        bar.activate(b);
        assert!(bar.close(b));
        assert_eq!(titles(&bar), vec!["a.rs"]);
        assert_eq!(bar.active().unwrap().id, a);
    }

    #[test]
    fn close_only_tab_leaves_nothing_active() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        assert!(bar.close(a));
        assert!(bar.tabs().is_empty());
        assert_eq!(bar.active(), None);
    }

    #[test]
    fn close_unrelated_tab_does_not_change_active() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        let b = bar.open("b.rs");
        bar.activate(a);
        assert!(bar.close(b));
        assert_eq!(bar.active().unwrap().id, a);
    }

    #[test]
    fn close_unknown_id_returns_false() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        bar.close(a);
        assert!(!bar.close(a));
    }

    #[test]
    fn reopen_last_closed_restores_in_lifo_order() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        let b = bar.open("b.rs");
        bar.close(a);
        bar.close(b);

        let reopened_first = bar.reopen_last_closed().unwrap();
        assert_eq!(reopened_first, b);
        assert_eq!(titles(&bar), vec!["b.rs"]);

        let reopened_second = bar.reopen_last_closed().unwrap();
        assert_eq!(reopened_second, a);
        assert_eq!(titles(&bar), vec!["b.rs", "a.rs"]);
    }

    #[test]
    fn reopen_with_nothing_closed_returns_none() {
        let mut bar = TabBar::new();
        assert_eq!(bar.reopen_last_closed(), None);
    }

    #[test]
    fn pin_clusters_pinned_tabs_at_the_start() {
        let mut bar = TabBar::new();
        bar.open("a.rs");
        let b = bar.open("b.rs");
        bar.open("c.rs");
        assert!(bar.pin(b));
        assert_eq!(titles(&bar), vec!["b.rs", "a.rs", "c.rs"]);
        assert!(bar.tabs()[0].pinned);
    }

    #[test]
    fn pin_twice_is_a_stable_no_op() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        bar.open("b.rs");
        assert!(bar.pin(a));
        assert!(bar.pin(a));
        assert_eq!(titles(&bar), vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn unpin_moves_tab_to_the_start_of_the_unpinned_group() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        let b = bar.open("b.rs");
        bar.open("c.rs");
        bar.pin(a);
        bar.pin(b);
        assert_eq!(titles(&bar), vec!["a.rs", "b.rs", "c.rs"]);

        assert!(bar.unpin(a));
        assert_eq!(titles(&bar), vec!["b.rs", "a.rs", "c.rs"]);
        assert!(!bar.tabs().iter().find(|t| t.id == a).unwrap().pinned);
    }

    #[test]
    fn move_tab_reorders_by_id() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        bar.open("b.rs");
        bar.open("c.rs");
        assert!(bar.move_tab(a, 2));
        assert_eq!(titles(&bar), vec!["b.rs", "c.rs", "a.rs"]);
    }

    #[test]
    fn move_tab_clamps_out_of_range_index_to_the_end() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        bar.open("b.rs");
        assert!(bar.move_tab(a, 999));
        assert_eq!(titles(&bar), vec!["b.rs", "a.rs"]);
    }

    #[test]
    fn move_unknown_id_returns_false() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        bar.close(a);
        assert!(!bar.move_tab(a, 0));
    }

    #[test]
    fn activate_switches_the_active_tab() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        bar.open("b.rs");
        assert!(bar.activate(a));
        assert_eq!(bar.active().unwrap().id, a);
    }

    #[test]
    fn activate_unknown_id_returns_false() {
        let mut bar = TabBar::new();
        let a = bar.open("a.rs");
        bar.close(a);
        assert!(!bar.activate(a));
    }

    #[test]
    fn set_title_updates_existing_tab() {
        let mut bar = TabBar::new();
        let a = bar.open("untitled");
        assert!(bar.set_title(a, "saved.rs"));
        assert_eq!(titles(&bar), vec!["saved.rs"]);
        bar.close(a);
        assert!(!bar.set_title(a, "other.rs"));
    }
}
