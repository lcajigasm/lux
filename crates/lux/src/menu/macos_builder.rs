//! macOS Cocoa AppKit native menu bar builder (T03).

use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly, msg_send, sel};
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_foundation::NSString;

use super::MenuAction;
use super::macos::get_target;

pub(crate) fn add_item(
    mtm: MainThreadMarker,
    menu: &NSMenu,
    title: &str,
    key: &str,
    action: Option<MenuAction>,
) -> Retained<NSMenuItem> {
    let target = get_target();
    let ns_title = NSString::from_str(title);
    let ns_key = NSString::from_str(key);
    let sel = if action.is_some() {
        Some(sel!(onMenuAction:))
    } else {
        None
    };
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &ns_title,
            sel,
            &ns_key,
        )
    };
    if let Some(act) = action {
        item.setTag(act.tag());
        unsafe {
            let _: () = msg_send![&*item, setTarget: &**target];
        }
    }
    menu.addItem(&item);
    item
}

pub(crate) fn add_item_with_mask(
    mtm: MainThreadMarker,
    menu: &NSMenu,
    title: &str,
    key: &str,
    mask: usize,
    action: Option<MenuAction>,
) -> Retained<NSMenuItem> {
    let item = add_item(mtm, menu, title, key, action);
    unsafe {
        let _: () = msg_send![&*item, setKeyEquivalentModifierMask: mask];
    }
    item
}

pub(crate) fn add_system_item(
    mtm: MainThreadMarker,
    menu: &NSMenu,
    title: &str,
    key: &str,
    action: objc2::runtime::Sel,
) -> Retained<NSMenuItem> {
    let item = add_item(mtm, menu, title, key, None);
    unsafe {
        let _: () = msg_send![&*item, setAction: action];
    }
    item
}

pub(crate) fn add_actions(
    mtm: MainThreadMarker,
    menu: &NSMenu,
    items: &[(&str, &str, MenuAction)],
) {
    for &(title, key, act) in items {
        add_item(mtm, menu, title, key, Some(act));
    }
}

pub(crate) fn add_separator(mtm: MainThreadMarker, menu: &NSMenu) {
    let item = NSMenuItem::separatorItem(mtm);
    menu.addItem(&item);
}

pub(crate) fn add_submenu(
    mtm: MainThreadMarker,
    parent: &NSMenu,
    title: &str,
    items: &[(&str, &str, usize, MenuAction)],
) {
    let sub_item = NSMenuItem::new(mtm);
    let sub = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
    for &(t, k, m, act) in items {
        if m == 0 {
            add_item(mtm, &sub, t, k, Some(act));
        } else {
            add_item_with_mask(mtm, &sub, t, k, m, Some(act));
        }
    }
    sub_item.setTitle(&NSString::from_str(title));
    parent.addItem(&sub_item);
    parent.setSubmenu_forItem(Some(&sub), &sub_item);
}

pub(crate) fn build_all_menus(mtm: MainThreadMarker, main_menu: &NSMenu) {
    // 1. Lux App Menu
    let app_menu_item = NSMenuItem::new(mtm);
    let app_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Lux"));
    add_item(mtm, &app_menu, "About Lux", "", Some(MenuAction::About));
    add_separator(mtm, &app_menu);
    add_item(
        mtm,
        &app_menu,
        "Preferences...",
        ",",
        Some(MenuAction::Preferences),
    );
    add_item(
        mtm,
        &app_menu,
        "Color Scheme...",
        "",
        Some(MenuAction::SelectColorTheme),
    );
    add_separator(mtm, &app_menu);
    add_system_item(mtm, &app_menu, "Hide Lux", "h", sel!(hide:));
    let hide_others =
        add_item_with_mask(mtm, &app_menu, "Hide Others", "h", 1048576 | 524288, None);
    unsafe {
        let _: () = msg_send![&*hide_others, setAction: sel!(hideOtherApplications:)];
    }
    add_separator(mtm, &app_menu);
    add_system_item(mtm, &app_menu, "Quit Lux", "q", sel!(terminate:));
    main_menu.addItem(&app_menu_item);
    main_menu.setSubmenu_forItem(Some(&app_menu), &app_menu_item);

    // 2. File Menu
    let file_menu_item = NSMenuItem::new(mtm);
    let file_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("File"));
    add_item(mtm, &file_menu, "New File", "n", Some(MenuAction::NewFile));
    add_item(
        mtm,
        &file_menu,
        "Open File...",
        "o",
        Some(MenuAction::OpenFile),
    );
    add_item_with_mask(
        mtm,
        &file_menu,
        "Open Workspace Folder...",
        "O",
        1048576 | 131072,
        Some(MenuAction::OpenFolder),
    );
    add_separator(mtm, &file_menu);
    add_item(mtm, &file_menu, "Save", "s", Some(MenuAction::Save));
    add_item_with_mask(
        mtm,
        &file_menu,
        "Save As...",
        "S",
        1048576 | 131072,
        Some(MenuAction::SaveAs),
    );
    add_separator(mtm, &file_menu);
    add_item(
        mtm,
        &file_menu,
        "Close Tab",
        "w",
        Some(MenuAction::CloseTab),
    );
    main_menu.addItem(&file_menu_item);
    main_menu.setSubmenu_forItem(Some(&file_menu), &file_menu_item);

    // 3. Edit Menu
    let edit_menu_item = NSMenuItem::new(mtm);
    let edit_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Edit"));
    add_item(mtm, &edit_menu, "Undo", "z", Some(MenuAction::Undo));
    add_item_with_mask(
        mtm,
        &edit_menu,
        "Redo",
        "Z",
        1048576 | 131072,
        Some(MenuAction::Redo),
    );
    add_separator(mtm, &edit_menu);
    add_actions(
        mtm,
        &edit_menu,
        &[
            ("Cut", "x", MenuAction::Cut),
            ("Copy", "c", MenuAction::Copy),
            ("Paste", "v", MenuAction::Paste),
            ("Select All", "a", MenuAction::SelectAll),
        ],
    );
    add_separator(mtm, &edit_menu);
    add_item(
        mtm,
        &edit_menu,
        "Toggle Line Comment",
        "/",
        Some(MenuAction::ToggleComment),
    );
    add_separator(mtm, &edit_menu);
    add_item_with_mask(
        mtm,
        &edit_menu,
        "Duplicate Line",
        "D",
        1048576 | 131072,
        Some(MenuAction::DuplicateLine),
    );
    add_item_with_mask(
        mtm,
        &edit_menu,
        "Delete Line",
        "K",
        1048576 | 131072,
        Some(MenuAction::DeleteLine),
    );
    add_actions(
        mtm,
        &edit_menu,
        &[
            ("Join Lines", "j", MenuAction::JoinLines),
            ("Swap Line Up", "", MenuAction::SwapLineUp),
            ("Swap Line Down", "", MenuAction::SwapLineDown),
        ],
    );
    add_separator(mtm, &edit_menu);
    add_item_with_mask(
        mtm,
        &edit_menu,
        "Fold",
        "[",
        1048576 | 524288,
        Some(MenuAction::Fold),
    );
    add_item_with_mask(
        mtm,
        &edit_menu,
        "Unfold",
        "]",
        1048576 | 524288,
        Some(MenuAction::Unfold),
    );
    add_actions(
        mtm,
        &edit_menu,
        &[
            ("Fold All", "", MenuAction::FoldAll),
            ("Unfold All", "", MenuAction::UnfoldAll),
        ],
    );
    main_menu.addItem(&edit_menu_item);
    main_menu.setSubmenu_forItem(Some(&edit_menu), &edit_menu_item);

    // 4. Selection Menu
    let sel_menu_item = NSMenuItem::new(mtm);
    let sel_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Selection"));
    add_actions(
        mtm,
        &sel_menu,
        &[
            ("Add Next Occurrence", "d", MenuAction::AddNextOccurrence),
            ("Skip Occurrence", "", MenuAction::SkipOccurrence),
        ],
    );
    add_item_with_mask(
        mtm,
        &sel_menu,
        "Select All Occurrences",
        "g",
        1048576 | 262144,
        Some(MenuAction::SelectAllOccurrences),
    );
    add_separator(mtm, &sel_menu);
    add_item_with_mask(
        mtm,
        &sel_menu,
        "Split Into Lines",
        "L",
        1048576 | 131072,
        Some(MenuAction::SplitIntoLines),
    );
    add_actions(
        mtm,
        &sel_menu,
        &[
            ("Add Cursor Above", "", MenuAction::AddCursorAbove),
            ("Add Cursor Below", "", MenuAction::AddCursorBelow),
        ],
    );
    add_separator(mtm, &sel_menu);
    add_item(
        mtm,
        &sel_menu,
        "Expand Selection to Line",
        "l",
        Some(MenuAction::ExpandToLine),
    );
    add_item_with_mask(
        mtm,
        &sel_menu,
        "Expand Selection to Scope",
        " ",
        1048576 | 131072,
        Some(MenuAction::ExpandToScope),
    );
    add_item_with_mask(
        mtm,
        &sel_menu,
        "Expand Selection to Tag",
        "A",
        1048576 | 131072,
        Some(MenuAction::ExpandToTag),
    );
    add_item_with_mask(
        mtm,
        &sel_menu,
        "Jump to Matching Bracket",
        "m",
        262144,
        Some(MenuAction::JumpToMatchingBracket),
    );

    main_menu.addItem(&sel_menu_item);
    main_menu.setSubmenu_forItem(Some(&sel_menu), &sel_menu_item);

    // 5. Find Menu
    let find_menu_item = NSMenuItem::new(mtm);
    let find_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Find"));
    add_item(mtm, &find_menu, "Find...", "f", Some(MenuAction::Find));
    add_item(
        mtm,
        &find_menu,
        "Find Next",
        "g",
        Some(MenuAction::FindNext),
    );
    add_item_with_mask(
        mtm,
        &find_menu,
        "Find Previous",
        "G",
        1048576 | 131072,
        Some(MenuAction::FindPrev),
    );
    add_item_with_mask(
        mtm,
        &find_menu,
        "Next Result",
        "\u{F707}",
        0,
        Some(MenuAction::FindNextResult),
    );
    add_item_with_mask(
        mtm,
        &find_menu,
        "Previous Result",
        "\u{F707}",
        131072,
        Some(MenuAction::FindPrevResult),
    );
    add_item_with_mask(
        mtm,
        &find_menu,
        "Replace...",
        "F",
        1048576 | 524288,
        Some(MenuAction::Replace),
    );
    add_separator(mtm, &find_menu);
    add_item_with_mask(
        mtm,
        &find_menu,
        "Find in Files...",
        "F",
        1048576 | 131072,
        Some(MenuAction::FindInFiles),
    );
    add_item_with_mask(
        mtm,
        &find_menu,
        "Replace in Files...",
        "H",
        1048576 | 131072,
        Some(MenuAction::ReplaceInFiles),
    );
    main_menu.addItem(&find_menu_item);
    main_menu.setSubmenu_forItem(Some(&find_menu), &find_menu_item);

    super::macos_view_menus::build_view_and_tool_menus(mtm, main_menu);
}
