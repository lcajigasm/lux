//! Reading a session back (ADR 0019): tabs with their drafts, carets and
//! selections, scroll, undo history, and the split layout.
//!
//! Nothing here runs on the UI thread. [`start`] reads the session file on
//! a `ui_io` worker; when it lands (`restore_apply::begin`) one worker
//! thread reads the tabs in session order, each file with its undo
//! history, and sends each tab as soon as it is read. The window is shown
//! and usable meanwhile; `restore_apply` adds each tab as it lands.

use std::path::{Path, PathBuf};

use lux_core::buffer::Buffer;
use lux_core::history::{History, codec};
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_platform::PlatformContext;
use lux_workspace::session::{CursorState, Session, TabState};
use lux_workspace::session_store;

use crate::app::App;
use crate::ui_io::Done;

/// A session file read from disk, with the folder to open.
pub(crate) struct Plan {
    pub(super) session: Session,
    pub(super) history_dir: PathBuf,
    /// The session's first folder, when it still exists.
    pub(super) root: Option<PathBuf>,
}

/// One tab read from disk, ready to become a document entry.
pub(crate) struct RestoredTab {
    pub(super) state: TabState,
    pub(super) buffer: Buffer,
    /// The saved language, when it names one (`Some(None)` is Plain Text).
    pub(super) language: Option<Option<lux_syntax::LanguageId>>,
    pub(super) history: Option<History>,
    pub(super) cursors: Option<MultiCursor>,
}

/// What a restore worker hands back.
pub(crate) enum Landed {
    Plan {
        generation: u64,
        plan: Option<Box<Plan>>,
    },
    Tab {
        generation: u64,
        index: usize,
        tab: Option<Box<RestoredTab>>,
    },
}

/// Starts restoring the session for `root` (`None`: the folderless
/// window). Returns at once; see the module docs.
pub(crate) fn start(app: &mut App, ctx: Option<&dyn PlatformContext>, root: Option<PathBuf>) {
    let data_dir = app.ui.session.data_dir.clone();
    let generation = super::restore_apply::next_generation(app);
    crate::ui_io::spawn(app, ctx, move || {
        let file = match &data_dir {
            Some(dir) => super::location::readable_session_file_in(dir, root.as_deref()),
            None => super::location::readable_session_file(root.as_deref()),
        };
        let plan = file.and_then(|file| read_plan(&file).ok().flatten());
        Some(Done::Session(Landed::Plan {
            generation,
            plan: plan.map(Box::new),
        }))
    });
}

/// Reads the session file and checks its folder. Disk I/O; workers only.
pub(super) fn read_plan(read_from: &Path) -> std::io::Result<Option<Plan>> {
    let session = Session::load(read_from)?;
    if session.folders.is_empty() && session.tabs.is_empty() {
        return Ok(None);
    }
    let root = session.folders.first().filter(|f| f.exists()).cloned();
    Ok(Some(Plan {
        history_dir: session_store::history_dir(read_from),
        session,
        root,
    }))
}

/// Reads one tab's text and undo history. Disk I/O; workers only.
pub(super) fn read_tab(state: TabState, history_dir: &Path) -> Option<RestoredTab> {
    let buffer = match (&state.draft_text, &state.path) {
        (Some(draft), _) => Buffer::from_bytes(draft.as_bytes()).ok()?,
        (None, Some(path)) if path.exists() => Buffer::open(path).ok()?,
        _ => return None,
    };
    // A pathless tab stays Plain Text until the user picks a language.
    let language = state
        .path
        .as_ref()
        .and(state.language.as_ref())
        .and_then(|name| {
            crate::language_picker::all_languages()
                .into_iter()
                .find(|(_, language)| language == name)
        })
        .map(|(id, _)| id);
    // The history only comes back when it was recorded against exactly
    // this text; a file changed on disk since then restores without undo.
    let history = state
        .history
        .as_deref()
        .and_then(|name| session_store::read_history(history_dir, name))
        .and_then(|bytes| codec::decode(&bytes, &buffer));
    let cursors = multi_cursor(&buffer, &state.cursors);
    Some(RestoredTab {
        state,
        buffer,
        language,
        history,
        cursors,
    })
}

/// Reads and applies a whole session on the calling thread, through the
/// same apply steps the workers feed. Tests only.
#[cfg(test)]
pub(crate) fn restore_session(app: &mut App, read_from: &Path) -> std::io::Result<bool> {
    let Some(plan) = read_plan(read_from)? else {
        return Ok(false);
    };
    let generation = super::restore_apply::next_generation(app);
    let Some((generation, tabs, history_dir)) = super::restore_apply::begin(app, generation, plan)
    else {
        return Ok(false);
    };
    let mut restored = false;
    for (index, state) in tabs.into_iter().enumerate() {
        let tab = read_tab(state, &history_dir).map(Box::new);
        if let Some(any) = super::restore_apply::apply_tab(app, generation, index, tab) {
            restored = any;
        }
    }
    Ok(restored)
}

/// Every caret with its selection, clamped to the restored text.
pub(super) fn multi_cursor(buffer: &Buffer, cursors: &[CursorState]) -> Option<MultiCursor> {
    if cursors.is_empty() {
        return None;
    }
    let end = buffer.len_chars();
    let sels = cursors
        .iter()
        .map(|c| {
            let head = Position::from_char(buffer, c.char_offset.min(end));
            let anchor = c
                .anchor
                .map_or(head, |a| Position::from_char(buffer, a.min(end)));
            Selection::new(anchor, head)
        })
        .collect();
    Some(MultiCursor::from_cursors(sels))
}
