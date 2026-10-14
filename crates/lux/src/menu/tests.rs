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
}
