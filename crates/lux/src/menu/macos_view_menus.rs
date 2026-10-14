//! macOS Cocoa AppKit View, Goto, Tools, Window, and Help menus builder (T03, ST12).

use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_foundation::NSString;

use super::MenuAction;
use super::macos_builder::{
    add_item, add_item_with_mask, add_separator, add_submenu, add_system_item,
};

pub(crate) fn build_view_and_tool_menus(mtm: MainThreadMarker, main_menu: &NSMenu) {
    // 6. View Menu
    let view_menu_item = NSMenuItem::new(mtm);
    let view_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("View"));
    add_item(
        mtm,
        &view_menu,
        "Toggle Side Bar",
        "b",
        Some(MenuAction::ToggleSidebar),
    );
    add_item(
        mtm,
        &view_menu,
        "Toggle Minimap",
        "",
        Some(MenuAction::ToggleMinimap),
    );
    add_item(
        mtm,
        &view_menu,
        "Toggle Terminal",
        "`",
        Some(MenuAction::ToggleTerminal),
    );
    add_item_with_mask(
        mtm,
        &view_menu,
        "Toggle Word Wrap",
        "z",
        524288,
        Some(MenuAction::ToggleWordWrap),
    );
    add_item_with_mask(
        mtm,
        &view_menu,
        "Enter Distraction Free Mode",
        "F",
        1048576 | 262144 | 131072,
        Some(MenuAction::ToggleDistractionFree),
    );
    add_separator(mtm, &view_menu);
    add_item_with_mask(
        mtm,
        &view_menu,
        "Split Editor Right",
        "2",
        1048576 | 524288,
        Some(MenuAction::SplitRight),
    );
    add_item_with_mask(
        mtm,
        &view_menu,
        "Split Editor Down",
        "@",
        1048576 | 524288 | 131072,
        Some(MenuAction::SplitDown),
    );
    add_item_with_mask(
        mtm,
        &view_menu,
        "Close Split Editor",
        "w",
        1048576 | 524288,
        Some(MenuAction::CloseSplit),
    );
    add_separator(mtm, &view_menu);
    add_submenu(
        mtm,
        &view_menu,
        "Layout",
        &[
            ("Single", "1", 1048576 | 524288, MenuAction::LayoutSingle),
            (
                "Columns: 2",
                "2",
                1048576 | 524288,
                MenuAction::LayoutColumns2,
            ),
            (
                "Columns: 3",
                "3",
                1048576 | 524288,
                MenuAction::LayoutColumns3,
            ),
            (
                "Columns: 4",
                "4",
                1048576 | 524288,
                MenuAction::LayoutColumns4,
            ),
            (
                "Rows: 2",
                "2",
                1048576 | 524288 | 131072,
                MenuAction::LayoutRows2,
            ),
            (
                "Rows: 3",
                "3",
                1048576 | 524288 | 131072,
                MenuAction::LayoutRows3,
            ),
            ("Grid: 4", "5", 1048576 | 524288, MenuAction::LayoutGrid2x2),
        ],
    );
    add_submenu(
        mtm,
        &view_menu,
        "Focus Group",
        &[
            ("Group 1", "1", 262144, MenuAction::FocusGroup1),
            ("Group 2", "2", 262144, MenuAction::FocusGroup2),
            ("Group 3", "3", 262144, MenuAction::FocusGroup3),
            ("Group 4", "4", 262144, MenuAction::FocusGroup4),
        ],
    );
    add_submenu(
        mtm,
        &view_menu,
        "Move File to Group",
        &[
            ("Group 1", "1", 262144 | 131072, MenuAction::MoveToGroup1),
            ("Group 2", "2", 262144 | 131072, MenuAction::MoveToGroup2),
            ("Group 3", "3", 262144 | 131072, MenuAction::MoveToGroup3),
            ("Group 4", "4", 262144 | 131072, MenuAction::MoveToGroup4),
        ],
    );
    add_separator(mtm, &view_menu);
    add_item_with_mask(
        mtm,
        &view_menu,
        "Change Language Mode...",
        "M",
        1048576 | 131072,
        Some(MenuAction::ChangeLanguageMode),
    );
    add_separator(mtm, &view_menu);
    add_item_with_mask(
        mtm,
        &view_menu,
        "Next Tab",
        "}",
        1048576 | 131072,
        Some(MenuAction::NextTab),
    );
    add_item_with_mask(
        mtm,
        &view_menu,
        "Previous Tab",
        "{",
        1048576 | 131072,
        Some(MenuAction::PrevTab),
    );
    main_menu.addItem(&view_menu_item);
    main_menu.setSubmenu_forItem(Some(&view_menu), &view_menu_item);

    // 7. Goto Menu
    let goto_menu_item = NSMenuItem::new(mtm);
    let goto_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Goto"));
    add_item(
        mtm,
        &goto_menu,
        "Goto Anything...",
        "p",
        Some(MenuAction::GotoAnything),
    );
    add_item_with_mask(
        mtm,
        &goto_menu,
        "Goto Symbol in Project...",
        "R",
        1048576 | 131072,
        Some(MenuAction::GotoSymbolInProject),
    );
    add_separator(mtm, &goto_menu);

    add_submenu(
        mtm,
        &goto_menu,
        "Bookmarks",
        &[
            ("Toggle Bookmark", "", 0, MenuAction::ToggleBookmark),
            ("Next Bookmark", "", 0, MenuAction::NextBookmark),
            ("Previous Bookmark", "", 0, MenuAction::PrevBookmark),
            ("Clear Bookmarks", "", 0, MenuAction::ClearBookmarks),
            (
                "Select All Bookmarks",
                "",
                0,
                MenuAction::SelectAllBookmarks,
            ),
        ],
    );

    main_menu.addItem(&goto_menu_item);
    main_menu.setSubmenu_forItem(Some(&goto_menu), &goto_menu_item);

    // 8. Tools Menu
    let tools_menu_item = NSMenuItem::new(mtm);
    let tools_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Tools"));
    add_item_with_mask(
        mtm,
        &tools_menu,
        "Build",
        "b",
        1048576,
        Some(MenuAction::Build),
    );
    add_item_with_mask(
        mtm,
        &tools_menu,
        "Build With...",
        "B",
        1048576 | 131072,
        Some(MenuAction::BuildWith),
    );
    add_item_with_mask(
        mtm,
        &tools_menu,
        "Cancel Build",
        "c",
        1048576 | 524288,
        Some(MenuAction::CancelBuild),
    );
    add_separator(mtm, &tools_menu);
    add_item_with_mask(
        mtm,
        &tools_menu,
        "Command Palette...",
        "P",
        1048576 | 131072,
        Some(MenuAction::CommandPalette),
    );
    main_menu.addItem(&tools_menu_item);
    main_menu.setSubmenu_forItem(Some(&tools_menu), &tools_menu_item);

    // Project Menu
    let proj_menu_item = NSMenuItem::new(mtm);
    let proj_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Project"));
    add_item(
        mtm,
        &proj_menu,
        "Open Project...",
        "",
        Some(MenuAction::OpenProject),
    );
    add_item_with_mask(
        mtm,
        &proj_menu,
        "Quick Switch Project...",
        "p",
        1048576 | 262144,
        Some(MenuAction::QuickSwitchProject),
    );
    add_item(
        mtm,
        &proj_menu,
        "Save Project As...",
        "",
        Some(MenuAction::SaveProjectAs),
    );
    add_item(
        mtm,
        &proj_menu,
        "Close Project",
        "",
        Some(MenuAction::CloseProject),
    );
    main_menu.addItem(&proj_menu_item);
    main_menu.setSubmenu_forItem(Some(&proj_menu), &proj_menu_item);

    // 9. Window Menu
    let win_menu_item = NSMenuItem::new(mtm);
    let win_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Window"));
    add_system_item(mtm, &win_menu, "Minimize", "m", sel!(performMiniaturize:));
    add_system_item(mtm, &win_menu, "Zoom", "", sel!(performZoom:));
    main_menu.addItem(&win_menu_item);
    main_menu.setSubmenu_forItem(Some(&win_menu), &win_menu_item);

    // 10. Help Menu
    let help_menu_item = NSMenuItem::new(mtm);
    let help_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Help"));
    add_item(
        mtm,
        &help_menu,
        "Lux Documentation",
        "",
        Some(MenuAction::About),
    );
    main_menu.addItem(&help_menu_item);
    main_menu.setSubmenu_forItem(Some(&help_menu), &help_menu_item);
}
