//! Multi-document tab session for the desktop shell (G05).
//!
//! Wraps [`lux_ui::tabs::TabBar`] and parks inactive/closed documents.
//! The active document lives in `App` fields and is exchanged through the
//! methods below (always returning the `DocumentState` `App` should hold).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_ui::tabs::{TabBar, TabId};

use crate::syntax::SyntaxHighlight;

/// Height reserved at the top of the window for the tab strip.
pub const TAB_STRIP_HEIGHT: f32 = 28.0;
/// Height reserved at the bottom for the status bar.
pub const STATUS_BAR_HEIGHT: f32 = 22.0;
/// Hit target width for the tab close control on the right of each tab.
pub const TAB_CLOSE_WIDTH: f32 = 18.0;

/// Per-tab editor state (active copy lives on `App` while that tab is front).
pub struct DocumentState {
    pub buffer: Buffer,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub read_only: bool,
    pub cursors: MultiCursor,
    pub goal_column: usize,
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub syntax: Option<SyntaxHighlight>,
}

impl DocumentState {
    pub fn untitled(buffer: Buffer) -> Self {
        Self::from_parts(buffer, None, false, false)
    }

    pub fn from_parts(
        buffer: Buffer,
        path: Option<PathBuf>,
        read_only: bool,
        dirty: bool,
    ) -> Self {
        let syntax = path.as_ref().and_then(|p| SyntaxHighlight::for_path(p));
        if let Some(hl) = &syntax {
            hl.set_text(buffer.text());
        }
        Self {
            buffer,
            path,
            dirty,
            read_only,
            cursors: MultiCursor::new(Selection::cursor(Position::document_start())),
            goal_column: 0,
            scroll_x: 0.0,
            scroll_y: 0.0,
            syntax,
        }
    }

    pub fn tab_title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");
        if self.dirty {
            format!("• {name}")
        } else {
            name.to_string()
        }
    }
}

/// Tab strip + parked documents. The active tab's bytes live on `App`.
pub struct TabSession {
    bar: TabBar,
    parked: HashMap<TabId, DocumentState>,
    closed: HashMap<TabId, DocumentState>,
    active: TabId,
}

impl TabSession {
    pub fn bootstrap(title: impl Into<String>) -> Self {
        let mut bar = TabBar::new();
        let active = bar.open(title);
        Self {
            bar,
            parked: HashMap::new(),
            closed: HashMap::new(),
            active,
        }
    }

    pub fn bar(&self) -> &TabBar {
        &self.bar
    }

    pub fn active_id(&self) -> TabId {
        self.active
    }

    pub fn set_active_title(&mut self, title: impl Into<String>) {
        let _ = self.bar.set_title(self.active, title);
    }

    /// Parks `current` under the active tab and opens a new active tab.
    /// Caller installs a fresh document into `App` for the new tab.
    pub fn open_new(&mut self, current: DocumentState, new_title: impl Into<String>) {
        self.parked.insert(self.active, current);
        self.active = self.bar.open(new_title);
    }

    /// If `path` is parked in another tab, parks `current` and returns that
    /// document to load into `App`. Otherwise returns `current` unchanged
    /// via [`FocusExisting::NotOpen`].
    pub fn focus_existing(
        &mut self,
        path: &Path,
        current: DocumentState,
    ) -> FocusExisting {
        let Some(existing) = self.find_parked_path(path) else {
            return FocusExisting::NotOpen(current);
        };
        self.parked.insert(self.active, current);
        let doc = self
            .parked
            .remove(&existing)
            .expect("found parked path must resolve");
        let _ = self.bar.activate(existing);
        self.active = existing;
        FocusExisting::Switched(doc)
    }

    fn find_parked_path(&self, path: &Path) -> Option<TabId> {
        self.parked
            .iter()
            .find(|(_, doc)| doc.path.as_deref() == Some(path))
            .map(|(id, _)| *id)
    }

    /// Parks `current`, activates `id`, returns that tab's document.
    /// On failure returns `current` unchanged via `Err`.
    pub fn switch_to(
        &mut self,
        id: TabId,
        current: DocumentState,
    ) -> SwitchResult {
        if id == self.active || self.bar.tabs().iter().all(|t| t.id != id) {
            return SwitchResult::Unchanged(current);
        }
        self.parked.insert(self.active, current);
        let doc = self
            .parked
            .remove(&id)
            .expect("inactive open tab must be parked");
        let _ = self.bar.activate(id);
        self.active = id;
        SwitchResult::Changed(doc)
    }

    /// Closes `id`. Always returns the document `App` should hold afterwards.
    pub fn close(&mut self, id: TabId, current: DocumentState) -> DocumentState {
        if self.bar.tabs().iter().all(|t| t.id != id) {
            return current;
        }
        let closing_active = id == self.active;
        if closing_active {
            self.parked.insert(id, current);
            if !self.bar.close(id) {
                return self.parked.remove(&id).expect("just parked");
            }
            if let Some(doc) = self.parked.remove(&id) {
                self.closed.insert(id, doc);
            }
        } else {
            if !self.bar.close(id) {
                return current;
            }
            if let Some(doc) = self.parked.remove(&id) {
                self.closed.insert(id, doc);
            }
            return current;
        }
        if let Some(tab_id) = self.bar.active().map(|t| t.id) {
            self.active = tab_id;
            self.parked
                .remove(&tab_id)
                .expect("neighbor tab must be parked")
        } else {
            let doc = DocumentState::untitled(
                Buffer::from_bytes(b"").expect("empty buffer always decodes"),
            );
            let title = doc.tab_title();
            self.active = self.bar.open(title);
            doc
        }
    }

    pub fn reopen_last_closed(
        &mut self,
        current: DocumentState,
    ) -> SwitchResult {
        let Some(id) = self.bar.reopen_last_closed() else {
            return SwitchResult::Unchanged(current);
        };
        self.parked.insert(self.active, current);
        self.active = id;
        let doc = self.closed.remove(&id).unwrap_or_else(|| {
            DocumentState::untitled(Buffer::from_bytes(b"").expect("empty buffer always decodes"))
        });
        let title = doc.tab_title();
        let _ = self.bar.set_title(id, title);
        SwitchResult::Changed(doc)
    }

    pub fn activate_next(
        &mut self,
        current: DocumentState,
    ) -> SwitchResult {
        let tabs = self.bar.tabs();
        if tabs.len() < 2 {
            return SwitchResult::Unchanged(current);
        }
        let idx = tabs.iter().position(|t| t.id == self.active).unwrap_or(0);
        let next = tabs[(idx + 1) % tabs.len()].id;
        self.switch_to(next, current)
    }

    pub fn activate_prev(
        &mut self,
        current: DocumentState,
    ) -> SwitchResult {
        let tabs = self.bar.tabs();
        if tabs.len() < 2 {
            return SwitchResult::Unchanged(current);
        }
        let idx = tabs.iter().position(|t| t.id == self.active).unwrap_or(0);
        let prev = tabs[(idx + tabs.len() - 1) % tabs.len()].id;
        self.switch_to(prev, current)
    }
}

/// Result of tab switch / cycle / reopen helpers that must not drop `current`.
pub enum SwitchResult {
    Changed(DocumentState),
    Unchanged(DocumentState),
}

/// Result of [`TabSession::focus_existing`].
pub enum FocusExisting {
    Switched(DocumentState),
    NotOpen(DocumentState),
}

pub fn language_label(path: Option<&Path>) -> String {
    path.and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .and_then(lux_syntax::LanguageId::from_extension)
        .map(|id| format!("{id:?}"))
        .unwrap_or_else(|| "Plain Text".to_string())
}

/// Which part of the tab strip a click landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabHit {
    Activate(TabId),
    Close(TabId),
}

/// Hit-test the tab strip. Tabs share the strip width equally.
pub fn hit_test_tab_strip(bar: &TabBar, strip_width: f32, x: f32, y: f32) -> Option<TabHit> {
    if !(0.0..TAB_STRIP_HEIGHT).contains(&y) || strip_width <= 0.0 {
        return None;
    }
    let tabs = bar.tabs();
    if tabs.is_empty() {
        return None;
    }
    let tab_w = strip_width / tabs.len() as f32;
    let index = ((x / tab_w).floor() as usize).min(tabs.len() - 1);
    let tab = &tabs[index];
    let local_x = x - tab_w * index as f32;
    if local_x >= tab_w - TAB_CLOSE_WIDTH {
        Some(TabHit::Close(tab.id))
    } else {
        Some(TabHit::Activate(tab.id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(name: &str, body: &str) -> DocumentState {
        DocumentState::from_parts(
            Buffer::from_bytes(body.as_bytes()).unwrap(),
            Some(PathBuf::from(name)),
            false,
            false,
        )
    }

    fn open_another(session: &mut TabSession, active: &mut DocumentState, next: DocumentState) {
        let title = next.tab_title();
        let old = std::mem::replace(active, next);
        session.open_new(old, title);
    }

    #[test]
    fn open_second_file_adds_a_tab() {
        let mut active = doc("a.rs", "fn a() {}");
        let mut session = TabSession::bootstrap(active.tab_title());
        open_another(&mut session, &mut active, doc("b.rs", "fn b() {}"));
        assert_eq!(session.bar().tabs().len(), 2);
        assert_eq!(active.path.as_deref(), Some(Path::new("b.rs")));
    }

    #[test]
    fn focus_existing_switches_without_duplicating() {
        let mut active = doc("/tmp/a.rs", "a");
        let mut session = TabSession::bootstrap(active.tab_title());
        open_another(&mut session, &mut active, doc("/tmp/b.rs", "b"));
        match session.focus_existing(
            Path::new("/tmp/a.rs"),
            std::mem::replace(&mut active, doc("tmp", "")),
        ) {
            FocusExisting::Switched(restored) => active = restored,
            FocusExisting::NotOpen(_) => panic!("a.rs should be parked"),
        }
        assert_eq!(session.bar().tabs().len(), 2);
        assert_eq!(active.path.as_deref(), Some(Path::new("/tmp/a.rs")));
        assert!(active.buffer.text().contains('a'));
    }

    #[test]
    fn close_active_loads_neighbor() {
        let mut active = doc("a.rs", "aaa");
        let mut session = TabSession::bootstrap(active.tab_title());
        open_another(&mut session, &mut active, doc("b.rs", "bbb"));
        open_another(&mut session, &mut active, doc("c.rs", "ccc"));
        let id = session.active_id();
        active = session.close(id, active);
        assert_eq!(session.bar().tabs().len(), 2);
        let text = active.buffer.text();
        assert!(text.contains("bbb") || text.contains("aaa"));
    }

    #[test]
    fn closing_last_tab_mints_untitled() {
        let active = doc("solo.rs", "x");
        let mut session = TabSession::bootstrap(active.tab_title());
        let id = session.active_id();
        let active = session.close(id, active);
        assert_eq!(session.bar().tabs().len(), 1);
        assert!(active.path.is_none());
    }

    #[test]
    fn reopen_restores_bytes() {
        let mut active = doc("a.rs", "keep");
        let mut session = TabSession::bootstrap(active.tab_title());
        open_another(&mut session, &mut active, doc("gone.rs", "restore-me"));
        let id = session.active_id();
        active = session.close(id, active);
        active = match session.reopen_last_closed(active) {
            SwitchResult::Changed(doc) => doc,
            SwitchResult::Unchanged(_) => panic!("reopen"),
        };
        assert!(active.buffer.text().contains("restore-me"));
    }

    #[test]
    fn hit_test_distinguishes_activate_and_close() {
        let mut active = doc("a.rs", "a");
        let mut session = TabSession::bootstrap(active.tab_title());
        open_another(&mut session, &mut active, doc("b.rs", "b"));
        let w = 200.0;
        assert_eq!(
            hit_test_tab_strip(session.bar(), w, 10.0, 5.0),
            Some(TabHit::Activate(session.bar().tabs()[0].id))
        );
        assert_eq!(
            hit_test_tab_strip(session.bar(), w, 99.0, 5.0),
            Some(TabHit::Close(session.bar().tabs()[0].id))
        );
        assert_eq!(
            hit_test_tab_strip(session.bar(), w, 110.0, 5.0),
            Some(TabHit::Activate(session.bar().tabs()[1].id))
        );
    }

    #[test]
    fn language_label_uses_extension_or_plain_text() {
        assert_eq!(language_label(Some(Path::new("main.rs"))), "Rust");
        assert_eq!(language_label(Some(Path::new("notes.txt"))), "Plain Text");
        assert_eq!(language_label(None), "Plain Text");
    }
}
