//! Tab strip and status-bar wiring for [`App`] (G05).
//!
//! Kept in its own file so `app.rs` stays under the 600-line limit.

use std::path::PathBuf;

use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::{ElementState, Key, MouseButton, NamedKey, PlatformEvent};
use lux_ui::tabs::TabId;

use crate::app::App;
use crate::tab_session::{
    hit_test_tab_strip, DocumentState, FocusExisting, SwitchResult, TabHit,
};

impl App {
    /// Snapshot the active `App` document fields into a [`DocumentState`].
    pub(crate) fn take_document(&mut self) -> DocumentState {
        DocumentState {
            buffer: std::mem::replace(
                &mut self.buffer,
                Buffer::from_bytes(b"").expect("empty buffer always decodes"),
            ),
            path: self.path.take(),
            dirty: self.dirty,
            read_only: self.read_only,
            cursors: std::mem::replace(
                &mut self.cursors,
                MultiCursor::new(Selection::cursor(Position::document_start())),
            ),
            goal_column: self.goal_column,
            scroll_x: self.viewport.scroll_x,
            scroll_y: self.viewport.scroll_y,
            syntax: self.syntax.take(),
        }
    }

    /// Install a document into the active `App` fields and refresh chrome.
    pub(crate) fn install_document(&mut self, doc: DocumentState) {
        self.buffer = doc.buffer;
        self.path = doc.path;
        self.dirty = doc.dirty;
        self.read_only = doc.read_only;
        self.cursors = doc.cursors;
        self.goal_column = doc.goal_column;
        self.viewport.scroll_x = doc.scroll_x;
        self.viewport.scroll_y = doc.scroll_y;
        self.syntax = doc.syntax;
        self.sync_tab_title();
        self.refresh_title();
        self.scheduler.invalidate_all();
    }

    pub(crate) fn sync_tab_title(&mut self) {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");
        let title = if self.dirty {
            format!("• {name}")
        } else {
            name.to_string()
        };
        self.tabs.set_active_title(title);
    }


    /// Open `path` in a new tab, or focus an existing tab for that path.
    pub(crate) fn open_path_in_tab(&mut self, path: PathBuf) {
        match crate::document::open_path(&path) {
            Ok(opened) => {
                println!("Lux: opened {}", opened.path.display());
                if opened.read_only {
                    println!(
                        "Lux: {} is huge — opened read-only (no fake preview)",
                        opened.path.display()
                    );
                }
                if self.path.as_ref() == Some(&opened.path) {
                    return;
                }
                let new_doc = DocumentState::from_parts(
                    opened.buffer,
                    Some(opened.path.clone()),
                    opened.read_only,
                    false,
                );
                let current = self.take_document();
                match self.tabs.focus_existing(&opened.path, current) {
                    FocusExisting::Switched(doc) => {
                        drop(new_doc);
                        self.install_document(doc);
                    }
                    FocusExisting::NotOpen(current) => {
                        let title = new_doc.tab_title();
                        self.tabs.open_new(current, title);
                        self.install_document(new_doc);
                        println!("Lux: tabs open={}", self.tabs.bar().tabs().len());
                    }
                }
            }
            Err(err) => eprintln!("Lux: {err}"),
        }
    }

    pub(crate) fn close_tab(&mut self, id: TabId) {
        let current = self.take_document();
        let doc = self.tabs.close(id, current);
        self.install_document(doc);
        println!("Lux: tabs open={}", self.tabs.bar().tabs().len());
    }

    pub(crate) fn close_active_tab(&mut self) {
        let id = self.tabs.active_id();
        self.close_tab(id);
    }

    pub(crate) fn switch_tab(&mut self, id: TabId) {
        let current = self.take_document();
        match self.tabs.switch_to(id, current) {
            SwitchResult::Changed(doc) | SwitchResult::Unchanged(doc) => {
                self.install_document(doc)
            }
        }
    }

    pub(crate) fn cycle_tab(&mut self, forward: bool) {
        let current = self.take_document();
        let result = if forward {
            self.tabs.activate_next(current)
        } else {
            self.tabs.activate_prev(current)
        };
        match result {
            SwitchResult::Changed(doc) | SwitchResult::Unchanged(doc) => {
                self.install_document(doc)
            }
        }
    }

    pub(crate) fn reopen_closed_tab(&mut self) {
        let current = self.take_document();
        match self.tabs.reopen_last_closed(current) {
            SwitchResult::Changed(doc) => {
                self.install_document(doc);
                println!(
                    "Lux: reopened tab; tabs open={}",
                    self.tabs.bar().tabs().len()
                );
            }
            SwitchResult::Unchanged(doc) => self.install_document(doc),
        }
    }

    pub(crate) fn handle_chrome_pointer(&mut self, event: &PlatformEvent) -> bool {
        match event {
            PlatformEvent::MouseMoved(pos) => {
                self.pointer = (pos.x, pos.y);
                false
            }
            PlatformEvent::MouseButton {
                button: MouseButton::Left,
                state: ElementState::Pressed,
            } => {
                let (x, y) = self.pointer;
                let width = self.viewport.width;
                if let Some(hit) = hit_test_tab_strip(self.tabs.bar(), width, x as f32, y as f32) {
                    match hit {
                        TabHit::Activate(id) => self.switch_tab(id),
                        TabHit::Close(id) => self.close_tab(id),
                    }
                    true
                } else {
                    false
                }
            }
            PlatformEvent::MouseButton {
                button: MouseButton::Middle,
                state: ElementState::Pressed,
            } => {
                let (x, y) = self.pointer;
                let width = self.viewport.width;
                if let Some(TabHit::Activate(id) | TabHit::Close(id)) =
                    hit_test_tab_strip(self.tabs.bar(), width, x as f32, y as f32)
                {
                    self.close_tab(id);
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Ctrl/Cmd+W close, Ctrl/Cmd+Tab next, Ctrl/Cmd+Shift+Tab prev,
    /// Ctrl/Cmd+Shift+T reopen — hard-wired until the keymap grows these
    /// commands (G06 palette will surface them).
    pub(crate) fn handle_tab_chord(&mut self, key: &Key) -> bool {
        if !self.chord_primary() {
            return false;
        }
        match key {
            Key::Character(ch) if ch.eq_ignore_ascii_case("w") && !self.modifiers.shift => {
                self.close_active_tab();
                true
            }
            Key::Character(ch) if ch.eq_ignore_ascii_case("t") && self.modifiers.shift => {
                self.reopen_closed_tab();
                true
            }
            Key::Named(NamedKey::Tab) if self.modifiers.shift => {
                self.cycle_tab(false);
                true
            }
            Key::Named(NamedKey::Tab) => {
                self.cycle_tab(true);
                true
            }
            _ => false,
        }
    }
}
