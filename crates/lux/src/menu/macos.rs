//! macOS Cocoa AppKit native menu bar implementation (T03).

use objc2::rc::Retained;
use objc2::{AnyThread, ClassType, MainThreadMarker, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem,
};
use objc2_foundation::{NSObject, NSString};
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
            if let Some(action) = MenuAction::from_tag(tag) {
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
