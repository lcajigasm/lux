use super::{MenuAction, poll_actions, post_action};

#[test]
fn test_menu_action_tag_roundtrip() {
    let all_actions = [
        MenuAction::About,
        MenuAction::Preferences,
        MenuAction::NewFile,
        MenuAction::OpenFile,
        MenuAction::OpenFolder,
        MenuAction::Save,
        MenuAction::SaveAs,
        MenuAction::CloseTab,
        MenuAction::CloneViewToGroup,
        MenuAction::NewViewIntoFile,
        MenuAction::Undo,
        MenuAction::Redo,
        MenuAction::Cut,
        MenuAction::Copy,
        MenuAction::Paste,
        MenuAction::SelectAll,
        MenuAction::ToggleComment,
        MenuAction::DuplicateLine,
        MenuAction::DeleteLine,
        MenuAction::JoinLines,
        MenuAction::SwapLineUp,
        MenuAction::SwapLineDown,
        MenuAction::AddNextOccurrence,
        MenuAction::SelectAllOccurrences,
        MenuAction::SkipOccurrence,
        MenuAction::SplitIntoLines,
        MenuAction::AddCursorAbove,
        MenuAction::AddCursorBelow,
        MenuAction::ExpandToLine,
        MenuAction::ExpandToScope,
        MenuAction::ExpandToTag,
        MenuAction::Find,
        MenuAction::Replace,
        MenuAction::FindInFiles,
        MenuAction::ReplaceInFiles,
        MenuAction::ToggleSidebar,
        MenuAction::ToggleMinimap,
        MenuAction::ToggleTerminal,
        MenuAction::ChangeLanguageMode,
        MenuAction::NextTab,
        MenuAction::PrevTab,
        MenuAction::ToggleWordWrap,
        MenuAction::SplitRight,
        MenuAction::SplitDown,
        MenuAction::CloseSplit,
        MenuAction::GotoAnything,
        MenuAction::CommandPalette,
        MenuAction::Fold,
        MenuAction::Unfold,
        MenuAction::FoldAll,
        MenuAction::UnfoldAll,
        MenuAction::JumpToMatchingBracket,
        MenuAction::SaveAll,
        MenuAction::ReopenClosedTab,
        MenuAction::NewWindow,
        MenuAction::ClearRecent,
    ];

    for action in all_actions {
        let tag = action.tag();
        assert_eq!(
            MenuAction::from_tag(tag),
            Some(action),
            "Tag {tag} failed roundtrip"
        );
    }
}

#[test]
fn test_invalid_tag_returns_none() {
    assert_eq!(MenuAction::from_tag(0), None);
    assert_eq!(MenuAction::from_tag(-1), None);
    assert_eq!(MenuAction::from_tag(200), None);
    assert_eq!(MenuAction::from_tag(209), None);
    assert_eq!(MenuAction::from_tag(9999), None);
}

#[test]
fn test_menu_queue_fifo() {
    // Empty whatever might be in the queue
    let _ = poll_actions();

    post_action(MenuAction::NewFile);
    post_action(MenuAction::Save);
    post_action(MenuAction::CloseTab);

    let actions = poll_actions();
    assert_eq!(
        actions,
        vec![MenuAction::NewFile, MenuAction::Save, MenuAction::CloseTab]
    );

    // Subsequent poll is empty
    assert!(poll_actions().is_empty());

    super::queue::post_recent(2);
    assert_eq!(super::queue::poll_recent(), vec![2]);
    assert!(super::queue::poll_recent().is_empty());
}

use crate::app::App;
use lux_platform::dialog::SaveDecision;
use lux_platform::{FileDialogOptions, FileDialogs, PlatformContext};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[test]
fn test_waker_invocation_on_post_action() {
    let _ = poll_actions();
    let woken = Arc::new(AtomicBool::new(false));
    let woken_clone = Arc::clone(&woken);
    super::register_waker(Arc::new(move || {
        woken_clone.store(true, Ordering::SeqCst);
    }));

    post_action(MenuAction::About);
    assert!(
        woken.load(Ordering::SeqCst),
        "Waker must be called when action is posted"
    );
    let actions = poll_actions();
    assert_eq!(actions, vec![MenuAction::About]);
}

struct TestCtx {
    folder_to_pick: Option<PathBuf>,
}

impl PlatformContext for TestCtx {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        unimplemented!()
    }
    fn create_window(
        &self,
        _: lux_platform::WindowAttributes,
    ) -> Result<Box<dyn lux_platform::PlatformWindow>, lux_platform::PlatformError> {
        unimplemented!()
    }
    fn clipboard(&self) -> &dyn lux_platform::Clipboard {
        unimplemented!()
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        self
    }
    fn exit(&self) {}
    fn request_wakeup(&self, _: std::time::Instant) {}
}

impl FileDialogs for TestCtx {
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Cancel
    }
    fn pick_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        self.folder_to_pick.clone()
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<PathBuf> {
        None
    }
}

#[test]
fn test_handle_menu_action_about() {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let ctx = TestCtx {
        folder_to_pick: None,
    };
    assert!(app.ui.dialogs.about.is_none());

    super::handle_menu_action(&mut app, &ctx, MenuAction::About);
    assert!(app.ui.dialogs.about.is_some());
}

#[test]
fn test_handle_menu_action_open_folder() {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let temp = tempfile::tempdir().unwrap();
    let ctx = TestCtx {
        folder_to_pick: Some(temp.path().to_path_buf()),
    };

    super::handle_menu_action(&mut app, &ctx, MenuAction::OpenFolder);
    assert!(app.ui.sidebar.is_some());
    assert_eq!(
        app.ui.sidebar.as_ref().unwrap().workspace_root,
        Some(temp.path().to_path_buf())
    );
}

#[test]
fn a_menu_comment_and_the_keymap_comment_run_once() {
    use lux_core::buffer::Buffer;
    use lux_syntax::LanguageId;

    super::chords::clear_claim();
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(b"alpha\n").unwrap();
    app.documents.entries[0].language = Some(LanguageId::Rust);
    let ctx = TestCtx {
        folder_to_pick: None,
    };
    super::handle_menu_action(&mut app, &ctx, MenuAction::ToggleComment);
    crate::input::dispatch::execute_keymap_command(&mut app, "toggle_comment", None, None);
    let text = app.buffer.text();
    assert!(text.contains("//"), "{text}");
    assert!(text.contains("alpha"), "{text}");
}

#[test]
fn command_palette_stays_open_when_the_menu_and_the_key_both_arrive() {
    super::chords::clear_claim();
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let ctx = TestCtx {
        folder_to_pick: None,
    };
    super::handle_menu_action(&mut app, &ctx, MenuAction::CommandPalette);
    assert!(app.ui.command_palette.is_some());
    crate::input::dispatch::execute_keymap_command(&mut app, "command_palette", None, None);
    assert!(
        app.ui.command_palette.is_some(),
        "the keymap delivery closed the palette the menu had just opened"
    );
}

#[test]
fn quick_open_stays_open_when_the_key_arrives_before_the_menu() {
    super::chords::clear_claim();
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    let ctx = TestCtx {
        folder_to_pick: None,
    };
    crate::input::dispatch::execute_keymap_command(&mut app, "quick_open", None, None);
    assert!(app.ui.goto_anything.is_some());
    super::handle_menu_action(&mut app, &ctx, MenuAction::GotoAnything);
    assert!(
        app.ui.goto_anything.is_some(),
        "the menu delivery closed Goto Anything the key had just opened"
    );
}
