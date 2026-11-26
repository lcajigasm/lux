//! The inline name field for New File, New Folder, and Rename. It edits a
//! `QueryField`, so it has a caret, a selection, arrows, Home/End, and the
//! panel clipboard chords. Rename opens with the base name selected, so
//! typing replaces `service` in `service.rs` and keeps the extension.

use lux_platform::{Key, KeyEvent, NamedKey, PlatformContext};
use lux_ui::file_tree::{NodeId, NodeKind};

use super::folders::Slot;
use super::{SidebarState, worker};
use crate::app::App;
use crate::field::QueryField;

#[derive(Debug, Clone)]
pub(crate) enum InlineInputMode {
    CreateFile { parent_id: NodeId },
    CreateFolder { parent_id: NodeId },
    Rename { node_id: NodeId },
}

#[derive(Debug, Clone)]
pub(crate) struct InlineInputState {
    pub(crate) mode: InlineInputMode,
    /// Which open folder: 0 is the header folder, then `extra` in order.
    pub(crate) root: usize,
    pub(crate) field: QueryField,
}

impl InlineInputState {
    /// The node whose row shows the field: the parent for a new entry.
    pub(crate) fn anchor(&self) -> NodeId {
        match self.mode {
            InlineInputMode::CreateFile { parent_id }
            | InlineInputMode::CreateFolder { parent_id } => parent_id,
            InlineInputMode::Rename { node_id } => node_id,
        }
    }
}

/// End of the base name: the last dot that is not the first character.
/// `.gitignore` and `Makefile` select whole; `a.tar.gz` selects `a.tar`.
pub(crate) fn base_name_end(name: &str) -> usize {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[..dot].chars().count(),
        _ => name.chars().count(),
    }
}

/// Opens the field for `mode` in folder `root`.
pub(crate) fn begin(sidebar: &mut SidebarState, root: usize, mode: InlineInputMode) {
    let field = match mode {
        InlineInputMode::Rename { node_id } => {
            let name = sidebar
                .tree_at(root)
                .and_then(|tree| tree.node_path(node_id))
                .and_then(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut field = QueryField::from_text(name.clone());
            field.select_range(0, base_name_end(&name));
            field
        }
        _ => QueryField::new(),
    };
    sidebar.context_menu = None;
    sidebar.inline_input = Some(InlineInputState { mode, root, field });
}

/// The painted row that carries the field. A new entry under a root whose
/// own row is hidden (the header folder) uses that folder's first row.
pub(crate) fn slot_index(input: &InlineInputState, slots: &[Slot]) -> Option<usize> {
    let anchor = input.anchor();
    slots
        .iter()
        .position(|slot| slot.root == input.root && slot.id == anchor)
        .or_else(|| slots.iter().position(|slot| slot.root == input.root))
}

/// Keys while the field is open. `true` when the key was consumed.
pub(crate) fn handle_key(app: &mut App, ctx: &dyn PlatformContext, event: &KeyEvent) -> bool {
    let shift = app.modifiers.shift;
    let chord = app.modifiers.meta || app.modifiers.control || app.modifiers.alt;
    let Some(input) = app
        .ui
        .sidebar
        .as_mut()
        .and_then(|sidebar| sidebar.inline_input.as_mut())
    else {
        return false;
    };
    let field = &mut input.field;
    match &event.key {
        Key::Named(NamedKey::Enter) => return commit(app, ctx),
        Key::Named(NamedKey::Backspace) => {
            field.pop();
        }
        Key::Named(NamedKey::Delete) => field.delete_forward(),
        Key::Named(NamedKey::ArrowLeft) => field.move_left(shift),
        Key::Named(NamedKey::ArrowRight) => field.move_right(shift),
        Key::Named(NamedKey::Home) | Key::Named(NamedKey::ArrowUp) => field.move_home(shift),
        Key::Named(NamedKey::End) | Key::Named(NamedKey::ArrowDown) => field.move_end(shift),
        Key::Named(NamedKey::Space) if !chord => field.push_str(" "),
        Key::Character(text) if !chord => field.push_str(text),
        _ => return false,
    }
    app.scheduler.invalidate_all();
    true
}

/// Enter: hands the disk work to the sidebar worker. The tree changes when
/// the result arrives.
pub(crate) fn commit(app: &mut App, _ctx: &dyn PlatformContext) -> bool {
    let waker = app.ui.session.waker();
    let Some(sidebar) = app.ui.sidebar.as_mut() else {
        return false;
    };
    let Some(input) = sidebar.inline_input.take() else {
        return false;
    };
    app.scheduler.invalidate_all();
    let name = input.field.as_str().trim().to_string();
    if name.is_empty() {
        return true;
    }
    let Some(tree) = sidebar.tree_at(input.root) else {
        return true;
    };
    let job = match input.mode {
        InlineInputMode::CreateFile { parent_id } | InlineInputMode::CreateFolder { parent_id } => {
            let kind = if matches!(input.mode, InlineInputMode::CreateFile { .. }) {
                NodeKind::File
            } else {
                NodeKind::Directory
            };
            let Some(parent) = tree.node_path(parent_id) else {
                return true;
            };
            worker::Job::Create {
                parent: parent.to_path_buf(),
                name,
                kind,
            }
        }
        InlineInputMode::Rename { node_id } => {
            let (Some(from), Some(kind)) = (tree.node_path(node_id), tree.node_kind(node_id))
            else {
                return true;
            };
            if from.file_name().is_some_and(|old| old == name.as_str()) {
                return true;
            }
            worker::Job::Rename {
                from: from.to_path_buf(),
                name,
                kind,
            }
        }
    };
    app.ui.session.file_ops.submit(job, waker);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_base_name_stops_before_the_last_extension() {
        assert_eq!(base_name_end("service.rs"), 7);
        assert_eq!(base_name_end("a.tar.gz"), 5);
        assert_eq!(base_name_end(".gitignore"), 10);
        assert_eq!(base_name_end("Makefile"), 8);
        assert_eq!(base_name_end("ñame.txt"), 4);
    }
}
