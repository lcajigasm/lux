use std::cell::Cell;
use std::sync::Arc;

use lux_core::buffer::Buffer;
use lux_platform::dialog::SaveDecision;
use lux_platform::event::KeyEvent;
use lux_platform::{
    Clipboard, ElementState, FileDialogOptions, FileDialogs, Key, MouseButton, NamedKey,
    PlatformContext, PlatformError, PlatformWindow, WindowAttributes,
};
use lux_ui::tabs::TabId;

use super::{drag_tab, drop_slot, follow_active, layout, release_drag, scroll_tabs, tab_at};
use crate::app::App;
use crate::document::DocumentEntry;

struct Dialogs;
impl FileDialogs for Dialogs {
    fn pick_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn pick_files(&self, _: &FileDialogOptions) -> Vec<std::path::PathBuf> {
        Vec::new()
    }
    fn pick_folder(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn save_file(&self, _: &FileDialogOptions) -> Option<std::path::PathBuf> {
        None
    }
    fn confirm_save(&self, _: &str) -> SaveDecision {
        SaveDecision::Discard
    }
    fn confirm_trash(&self, _: &str) -> bool {
        false
    }
}

pub(crate) struct Ctx {
    dialogs: Arc<Dialogs>,
    exited: Cell<bool>,
}
impl Ctx {
    pub(crate) fn new() -> Self {
        Self {
            dialogs: Arc::new(Dialogs),
            exited: Cell::new(false),
        }
    }
}
impl Clipboard for Ctx {
    fn read_text(&self) -> Result<String, PlatformError> {
        Ok(String::new())
    }
    fn write_text(&self, _: &str) -> Result<(), PlatformError> {
        Ok(())
    }
}
impl PlatformContext for Ctx {
    fn background_waker(&self) -> Arc<dyn Fn() + Send + Sync> {
        Arc::new(|| {})
    }
    fn background_dialogs(&self) -> Arc<dyn FileDialogs> {
        self.dialogs.clone()
    }
    fn create_window(&self, _: WindowAttributes) -> Result<Box<dyn PlatformWindow>, PlatformError> {
        Err(PlatformError::Window("headless".into()))
    }
    fn clipboard(&self) -> &dyn Clipboard {
        self
    }
    fn file_dialogs(&self) -> &dyn FileDialogs {
        self.dialogs.as_ref()
    }
    fn exit(&self) {
        self.exited.set(true);
    }
    fn request_wakeup(&self, _: std::time::Instant) {}
}

fn app() -> App {
    App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec())
}

#[test]
fn middle_click_closes_the_inactive_tab() {
    let mut app = app();
    let second = app.documents.tabs.open("second");
    let width = app.tab_width();
    app.mouse.position.y = 8.0;
    app.mouse.position.x = (width * 1.5) as f64;
    assert!(app.handle_mouse_click_on_chrome(&Ctx::new(), MouseButton::Middle));
    assert!(app.documents.tabs.tabs().iter().all(|tab| tab.id != second));
}

#[test]
fn overflow_scroll_moves_which_tab_the_pointer_hits() {
    let mut app = app();
    for index in 0..12 {
        app.documents.tabs.open(format!("f{index}.txt"));
    }
    let width = app.tab_width();
    assert!(width * app.documents.tabs.tabs().len() as f32 > app.viewport.width);
    let before = tab_at(&app, width * 0.5).unwrap().0;
    scroll_tabs(&mut app, -width * 2.0);
    let after = tab_at(&app, width * 0.5).unwrap().0;
    assert!(after > before);
    assert!(app.documents.tabs.scroll() > 0.0);
}

/// Opens `count` documents as their own tabs, like File > Open does, and
/// returns their ids in strip order.
pub(crate) fn open_tabs(app: &mut App, count: usize) -> Vec<TabId> {
    (0..count)
        .map(|index| {
            let id = app.documents.tabs.open(format!("f{index}.txt"));
            let buffer = Buffer::from_bytes(format!("{index}").as_bytes()).unwrap();
            app.documents
                .entries
                .push(DocumentEntry::new(id, None, buffer));
            id
        })
        .collect()
}

fn key(key: Key, text: Option<&str>) -> KeyEvent {
    KeyEvent {
        state: ElementState::Pressed,
        key,
        text: text.map(Into::into),
        repeat: false,
        physical: None,
    }
}

fn command(app: &mut App, on: bool) {
    if cfg!(target_os = "macos") {
        app.modifiers.meta = on;
    } else {
        app.modifiers.control = on;
    }
}

fn press(app: &mut App, ctx: &Ctx, button: MouseButton, x: f32, y: f32) {
    app.mouse.position.x = x as f64;
    app.mouse.position.y = y as f64;
    app.handle_mouse_click_on_chrome(ctx, button);
}

fn move_to(app: &mut App, x: f32) -> bool {
    app.mouse.position.x = x as f64;
    drag_tab(app)
}

fn visible(app: &App, id: TabId) -> bool {
    let strip = layout(app);
    let index = app
        .documents
        .tabs
        .tabs()
        .iter()
        .position(|t| t.id == id)
        .unwrap();
    let left = strip.left + index as f32 * strip.tab_width - app.documents.tabs.scroll();
    left >= strip.left - 0.5 && left + strip.tab_width <= strip.right() + 0.5
}

fn ids(app: &App) -> Vec<TabId> {
    app.documents.tabs.tabs().iter().map(|t| t.id).collect()
}

#[test]
fn switching_by_shortcut_scrolls_the_new_tab_into_view() {
    let ctx = Ctx::new();
    let mut app = app();
    app.viewport.width = 800.0;
    let opened = open_tabs(&mut app, 30);
    let first = ids(&app)[0];
    let last = *opened.last().unwrap();
    assert!(app.switch_to_tab(last));
    follow_active(&mut app);
    assert!(visible(&app, last));
    assert!(!visible(&app, first), "30 tabs overflow an 800px strip");

    command(&mut app, true);
    app.handle_keyboard_event(&ctx, &key(Key::Character("1".into()), None));
    assert_eq!(app.documents.active_tab, Some(first));
    follow_active(&mut app); // what every frame does before painting
    assert!(visible(&app, first));
    assert_eq!(app.documents.tabs.scroll(), 0.0);

    app.handle_keyboard_event(&ctx, &key(Key::Character("9".into()), None));
    follow_active(&mut app);
    assert_eq!(app.documents.active_tab, Some(last));
    assert!(visible(&app, last));
}

#[test]
fn wheel_scroll_is_kept_while_the_active_tab_stays_the_same() {
    let mut app = app();
    app.viewport.width = 800.0;
    open_tabs(&mut app, 30);
    follow_active(&mut app);
    let width = app.tab_width();
    scroll_tabs(&mut app, -width * 5.0);
    let scrolled = app.documents.tabs.scroll();
    assert!(scrolled > 0.0);
    follow_active(&mut app);
    assert_eq!(app.documents.tabs.scroll(), scrolled);
}

#[test]
fn overflow_button_shows_only_when_tabs_overflow() {
    let mut app = app();
    app.viewport.width = 800.0;
    open_tabs(&mut app, 2);
    assert!(layout(&app).button.is_none());
    open_tabs(&mut app, 28);
    let strip = layout(&app);
    let [button_x, _] = strip.button.expect("30 tabs overflow");
    // The button covers the strip's right end, so no tab is hit there.
    assert!(tab_at(&app, button_x + 1.0).is_none());
}

#[test]
fn overflow_list_filters_by_typing_and_enter_switches() {
    let ctx = Ctx::new();
    let mut app = app();
    app.viewport.width = 800.0;
    let opened = open_tabs(&mut app, 30);
    follow_active(&mut app);
    let [button_x, button_w] = layout(&app).button.unwrap();
    press(
        &mut app,
        &ctx,
        MouseButton::Left,
        button_x + button_w * 0.5,
        8.0,
    );
    assert!(app.documents.strip.list.is_some());
    let rows = super::list::rows(&app.documents, false).unwrap();
    assert_eq!(rows.len(), 31, "every open tab is listed");

    for ch in ["f", "2", "7"] {
        app.handle_keyboard_event(&ctx, &key(Key::Character(ch.into()), Some(ch)));
    }
    let rows = super::list::rows(&app.documents, false).unwrap();
    assert_eq!(rows[0].title, "f27.txt");
    assert_eq!(
        rows[0].hits,
        vec![0..1, 1..2, 2..3],
        "the matched letters are lit"
    );
    assert!(
        !app.buffer.text().contains("f27"),
        "typing went to the list"
    );

    app.handle_keyboard_event(&ctx, &key(Key::Named(NamedKey::Enter), None));
    assert!(app.documents.strip.list.is_none());
    assert_eq!(app.documents.active_tab, Some(opened[27]));
    follow_active(&mut app);
    assert!(visible(&app, opened[27]));
}

#[test]
fn overflow_list_marks_dirty_tabs_and_switches_on_click() {
    let ctx = Ctx::new();
    let mut app = app();
    app.viewport.width = 800.0;
    let opened = open_tabs(&mut app, 30);
    let entry = app
        .documents
        .entries
        .iter_mut()
        .find(|e| e.tab_id == opened[3])
        .unwrap();
    entry.saved_version = entry.history.state_id().wrapping_add(1);
    super::list::open(&mut app);
    let rows = super::list::rows(&app.documents, false).unwrap();
    assert!(rows.iter().find(|r| r.id == opened[3]).unwrap().dirty);
    assert_eq!(rows.iter().filter(|r| r.dirty).count(), 1);

    let geometry = super::list::card(&app, 0, rows.len());
    let y = geometry.rows_top + geometry.row_height * 2.5;
    let x = geometry.left + geometry.width * 0.5;
    press(&mut app, &ctx, MouseButton::Left, x, y);
    assert!(app.documents.strip.list.is_none());
    assert_eq!(app.documents.active_tab, Some(rows[2].id));
}

#[test]
fn escape_and_outside_click_close_the_overflow_list() {
    let ctx = Ctx::new();
    let mut app = app();
    app.viewport.width = 800.0;
    open_tabs(&mut app, 30);
    let active = app.documents.active_tab;
    super::list::open(&mut app);
    app.handle_keyboard_event(&ctx, &key(Key::Named(NamedKey::Escape), None));
    assert!(app.documents.strip.list.is_none());
    super::list::open(&mut app);
    press(&mut app, &ctx, MouseButton::Left, 2.0, 300.0);
    assert!(app.documents.strip.list.is_none());
    assert_eq!(app.documents.active_tab, active);
}

#[test]
fn dragging_tab_zero_to_position_three_commits_on_release() {
    let ctx = Ctx::new();
    let mut app = app();
    app.viewport.width = 1600.0;
    open_tabs(&mut app, 4);
    let before = ids(&app);
    let width = app.tab_width();
    assert!(layout(&app).button.is_none());

    press(&mut app, &ctx, MouseButton::Left, width * 0.5, 8.0);
    assert!(move_to(&mut app, width * 2.0));
    assert!(move_to(&mut app, width * 3.9));
    // The strip only shows the marker; the order holds until release.
    assert_eq!(ids(&app), before);
    assert_eq!(drop_slot(&app), Some(4));

    release_drag(&mut app);
    let after = ids(&app);
    assert_eq!(after[3], before[0]);
    assert_eq!(
        after,
        vec![before[1], before[2], before[3], before[0], before[4]]
    );
    assert!(app.documents.strip.drag.is_none());
}

#[test]
fn escape_cancels_a_tab_drag() {
    let ctx = Ctx::new();
    let mut app = app();
    app.viewport.width = 1600.0;
    open_tabs(&mut app, 4);
    let before = ids(&app);
    let width = app.tab_width();
    press(&mut app, &ctx, MouseButton::Left, width * 0.5, 8.0);
    assert!(move_to(&mut app, width * 3.9));
    app.handle_keyboard_event(&ctx, &key(Key::Named(NamedKey::Escape), None));
    assert!(app.documents.strip.drag.is_none());
    release_drag(&mut app);
    assert_eq!(ids(&app), before);
}

#[test]
fn a_click_without_travel_does_not_reorder() {
    let ctx = Ctx::new();
    let mut app = app();
    app.viewport.width = 1600.0;
    open_tabs(&mut app, 3);
    let before = ids(&app);
    let width = app.tab_width();
    press(&mut app, &ctx, MouseButton::Left, width * 1.5, 8.0);
    assert!(!move_to(&mut app, width * 1.5 + 2.0));
    release_drag(&mut app);
    assert_eq!(ids(&app), before);
    assert_eq!(app.documents.active_tab, Some(before[1]));
}
