//! macOS Cocoa AppKit native menu bar implementation (T03).

use objc2::rc::Retained;
use objc2::{AnyThread, ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem,
};
use objc2_foundation::{NSObject, NSString};
use std::cell::RefCell;
use std::sync::OnceLock;

use super::{MenuAction, post_action};
use crate::app::App;

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    #[name = "LuxMenuTarget"]
    pub(crate) struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(onMenuAction:))]
        fn on_menu_action(&self, sender: &NSMenuItem) {
            let tag = sender.tag();
            if let Some(index) = super::recent::index_from_tag(tag) {
                super::queue::post_recent(index);
            } else if let Some(action) = MenuAction::from_tag(tag) {
                post_action(action);
            }
        }
    }
);

static TARGET: OnceLock<Retained<MenuTarget>> = OnceLock::new();

pub(super) fn get_target() -> &'static Retained<MenuTarget> {
    TARGET.get_or_init(|| {
        let obj: Retained<MenuTarget> = unsafe { msg_send![MenuTarget::class(), new] };
        obj
    })
}

/// `+[NSEvent modifierFlags]`. A focus loss clears the modifiers Lux stored
/// while AppKit still has Command down, so Cmd+N is neither a command nor text.
pub(crate) fn command_key_is_down() -> bool {
    const COMMAND: usize = 1 << 20;
    let Some(class) = objc2::runtime::AnyClass::get(c"NSEvent") else {
        return false;
    };
    let flags: usize = unsafe { msg_send![class, modifierFlags] };
    flags & COMMAND != 0
}

pub(crate) fn install_main_menu(app: &App) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let ns_app = NSApplication::sharedApplication(mtm);
    let main_menu = NSMenu::new(mtm);

    super::macos_builder::build_all_menus(mtm, &main_menu);

    ns_app.setMainMenu(Some(&main_menu));
    update_view_checkmarks(app);
}

pub(crate) fn update_view_checkmarks(app: &App) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let ns_app = NSApplication::sharedApplication(mtm);
    let Some(main_menu) = ns_app.mainMenu() else {
        return;
    };

    let sidebar_visible = app
        .ui
        .sidebar
        .as_ref()
        .is_some_and(|s| s.visible && s.tree.is_some());
    let minimap_visible = app.ui.minimap.inner.enabled;
    let terminal_visible = app.ui.term.visible;

    if let Some(item) = main_menu.itemWithTag(MenuAction::ToggleSidebar.tag()) {
        item.setState(if sidebar_visible {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }
    if let Some(item) = main_menu.itemWithTag(MenuAction::ToggleMinimap.tag()) {
        item.setState(if minimap_visible {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }
    if let Some(item) = main_menu.itemWithTag(MenuAction::ToggleTerminal.tag()) {
        item.setState(if terminal_visible {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }
    let word_wrap = app.editor_view.metrics().word_wrap;
    if let Some(item) = main_menu.itemWithTag(MenuAction::ToggleWordWrap.tag()) {
        item.setState(if word_wrap {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }
    let df_active = app.is_distraction_free();
    if let Some(item) = main_menu.itemWithTag(MenuAction::ToggleDistractionFree.tag()) {
        item.setState(if df_active {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        item.setTitle(&NSString::from_str(if df_active {
            "Exit Distraction Free Mode"
        } else {
            "Enter Distraction Free Mode"
        }));
    }
}

thread_local! {
    static OPEN_RECENT: RefCell<Option<Retained<NSMenu>>> = RefCell::new(None);
}

pub(super) fn add_open_recent(mtm: MainThreadMarker, file_menu: &NSMenu) {
    let item = NSMenuItem::new(mtm);
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Open Recent"));
    // Disabled rows stay disabled. The default re-enables every item on open.
    menu.setAutoenablesItems(false);
    item.setTitle(&NSString::from_str("Open Recent"));
    file_menu.addItem(&item);
    file_menu.setSubmenu_forItem(Some(&menu), &item);
    OPEN_RECENT.with(|slot| *slot.borrow_mut() = Some(menu.clone()));
    rebuild_open_recent(mtm, &menu);
}

pub(crate) fn refresh_open_recent() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    OPEN_RECENT.with(|slot| {
        if let Some(menu) = slot.borrow().clone() {
            rebuild_open_recent(mtm, &menu);
        }
    });
}

fn rebuild_open_recent(mtm: MainThreadMarker, menu: &NSMenu) {
    menu.removeAllItems();
    let paths = super::recent::entries();
    if paths.is_empty() {
        let item = super::macos_builder::add_item(mtm, menu, "No Recent Files", "", None);
        item.setEnabled(false);
    } else {
        for (index, path) in paths.iter().enumerate().take(super::recent::SLOTS) {
            let title = super::recent::label(path, &paths);
            add_recent_item(mtm, menu, &title, super::recent::TAG_BASE + index as isize);
        }
    }
    super::macos_builder::add_separator(mtm, menu);
    let clear = super::macos_builder::add_item(
        mtm,
        menu,
        "Clear Recent",
        "",
        Some(MenuAction::ClearRecent),
    );
    if paths.is_empty() {
        clear.setEnabled(false);
    }
}

fn add_recent_item(mtm: MainThreadMarker, menu: &NSMenu, title: &str, tag: isize) {
    let target = get_target();
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(sel!(onMenuAction:)),
            &NSString::from_str(""),
        )
    };
    item.setTag(tag);
    unsafe {
        let _: () = msg_send![&*item, setTarget: &**target];
    }
    menu.addItem(&item);
}
