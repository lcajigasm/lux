//! The unified macOS title bar: content under a transparent title bar,
//! traffic lights centered on the app's tab row, the user's title-bar
//! double-click setting, and fullscreen notifications.
//!
//! winit 0.30.13 creates the transparent, full-size-content window but has
//! no API for the button frames, their vertical position, or fullscreen
//! transitions finishing. Those are the AppKit calls below.

use std::cell::Cell;
use std::sync::atomic::{AtomicU8, Ordering};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSApplication, NSColor, NSView, NSWindow, NSWindowButton,
    NSWindowDidEnterFullScreenNotification, NSWindowDidExitFullScreenNotification,
    NSWindowDidResizeNotification, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSObject, NSPoint, NSRect, NSSize, NSString,
    NSUserDefaults,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::event_loop::EventLoopProxy;
use winit::platform::macos::WindowAttributesExtMacOS;

use super::app::AccessibilityEvent;
use crate::title_bar::{
    DoubleClickAction, centered_offset, leading_inset_physical, leading_inset_points,
};

/// The tab row's height until the app reports its own.
const DEFAULT_ROW_HEIGHT: f64 = 32.0;

/// Transparent title bar, content under it, no title text drawn. The
/// title is still set, so Mission Control and the Window menu show it.
pub(super) fn unified(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    attrs
        .with_titlebar_transparent(true)
        .with_fullsize_content_view(true)
        .with_title_hidden(true)
}

fn ns_window(window: &winit::window::Window) -> Option<Retained<NSWindow>> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    // SAFETY: winit's AppKit handle is its live NSView, used on the main
    // thread that owns it.
    let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
    view.window()
}

fn is_fullscreen(window: &NSWindow) -> bool {
    window.styleMask().contains(NSWindowStyleMask::FullScreen)
}

/// Inset of the tab row, in physical pixels at `scale`.
pub(super) fn leading_inset(window: &winit::window::Window) -> f64 {
    let Some(ns) = ns_window(window) else {
        return 0.0;
    };
    let zoom_max_x = ns
        .standardWindowButton(NSWindowButton::ZoomButton)
        .map(|button| {
            let frame = button.frame();
            frame.origin.x + frame.size.width
        });
    let points = leading_inset_points(zoom_max_x, is_fullscreen(&ns));
    leading_inset_physical(points, window.scale_factor())
}

/// Moves the traffic lights so they are centered on a top row of
/// `row_height` points. AppKit lays the title bar out again on resize,
/// fullscreen exit, and title changes, so every one of those calls this.
fn center_buttons(window: &NSWindow, row_height: f64) {
    if is_fullscreen(window) {
        return;
    }
    let Some(close) = window.standardWindowButton(NSWindowButton::CloseButton) else {
        return;
    };
    // close -> NSTitlebarView -> NSTitlebarContainerView -> NSThemeFrame.
    // SAFETY: plain view-hierarchy getters on the main thread.
    let Some(titlebar) = (unsafe { close.superview() }) else {
        return;
    };
    let Some(container) = (unsafe { titlebar.superview() }) else {
        return;
    };
    let Some(frame_view) = (unsafe { container.superview() }) else {
        return;
    };
    let width = frame_view.frame().size.width;
    let window_height = frame_view.frame().size.height;
    let y = if frame_view.isFlipped() {
        0.0
    } else {
        window_height - row_height
    };
    container.setFrame(NSRect::new(
        NSPoint::new(0.0, y),
        NSSize::new(width, row_height),
    ));
    titlebar.setFrame(NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(width, row_height),
    ));
    for kind in [
        NSWindowButton::CloseButton,
        NSWindowButton::MiniaturizeButton,
        NSWindowButton::ZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(kind) {
            let frame = button.frame();
            // Centered, so the offset is the same from the top or bottom.
            let offset = centered_offset(row_height, frame.size.height);
            button.setFrameOrigin(NSPoint::new(frame.origin.x, offset));
        }
    }
}

/// Paints whatever AppKit shows of the window itself (the band a
/// fullscreen window keeps above its content on displays with a camera
/// housing, the area behind the transparent title bar) in the tab row's
/// color, and gives the window the matching light or dark appearance so
/// the traffic lights, including their inactive gray, read on that color.
pub(super) fn set_title_bar_color(window: &winit::window::Window, rgb: [u8; 3], dark: bool) {
    let Some(ns) = ns_window(window) else {
        return;
    };
    let [r, g, b] = rgb.map(|c| f64::from(c) / 255.0);
    let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0);
    ns.setBackgroundColor(Some(&color));
    // SAFETY: AppKit's named-appearance constants.
    let name = unsafe {
        if dark {
            NSAppearanceNameDarkAqua
        } else {
            NSAppearanceNameAqua
        }
    };
    ns.setAppearance(NSAppearance::appearanceNamed(name).as_deref());
}

static DOUBLE_CLICK: AtomicU8 = AtomicU8::new(0);

fn encode(action: DoubleClickAction) -> u8 {
    match action {
        DoubleClickAction::Zoom => 0,
        DoubleClickAction::Minimize => 1,
        DoubleClickAction::None => 2,
    }
}

fn decode(value: u8) -> DoubleClickAction {
    match value {
        1 => DoubleClickAction::Minimize,
        2 => DoubleClickAction::None,
        _ => DoubleClickAction::Zoom,
    }
}

/// Reads the user's double-click setting on a worker thread: defaults go
/// through `cfprefsd`, which is I/O the UI thread must not wait on. Run at
/// window creation and whenever the app regains focus, which is when the
/// setting can have changed in System Settings.
pub(super) fn refresh_double_click_action() {
    let _ = std::thread::Builder::new()
        .name("lux-title-bar-defaults".into())
        .spawn(|| {
            objc2::rc::autoreleasepool(|_| {
                let defaults = NSUserDefaults::standardUserDefaults();
                let action = defaults
                    .stringForKey(&NSString::from_str("AppleActionOnDoubleClick"))
                    .map(|value| value.to_string());
                let legacy_key = NSString::from_str("AppleMiniaturizeOnDoubleClick");
                let legacy = defaults
                    .objectForKey(&legacy_key)
                    .map(|_| defaults.boolForKey(&legacy_key));
                let resolved = DoubleClickAction::from_defaults(action.as_deref(), legacy);
                DOUBLE_CLICK.store(encode(resolved), Ordering::Relaxed);
            });
        });
}

/// The press that is being handled right now: a double click does the
/// user's action, anything else starts a native window drag.
pub(super) fn title_bar_pressed(window: &winit::window::Window) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let clicks = NSApplication::sharedApplication(mtm)
        .currentEvent()
        .map_or(1, |event| event.clickCount());
    if clicks == 2 {
        let Some(ns) = ns_window(window) else {
            return;
        };
        match decode(DOUBLE_CLICK.load(Ordering::Relaxed)) {
            DoubleClickAction::Zoom => ns.zoom(None),
            DoubleClickAction::Minimize => ns.miniaturize(None),
            DoubleClickAction::None => {}
        }
    } else {
        let _ = window.drag_window();
    }
}

pub(super) struct ObserverIvars {
    window: Retained<NSWindow>,
    winit_id: winit::window::WindowId,
    proxy: EventLoopProxy<AccessibilityEvent>,
    row_height: Cell<f64>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "LuxTitleBarObserver"]
    #[ivars = ObserverIvars]
    pub(super) struct TitleBarObserver;

    impl TitleBarObserver {
        #[unsafe(method(windowDidResize:))]
        fn did_resize(&self, _note: &NSNotification) {
            self.recenter();
        }

        #[unsafe(method(windowDidEnterFullScreen:))]
        fn did_enter_fullscreen(&self, _note: &NSNotification) {
            self.notify(true);
        }

        #[unsafe(method(windowDidExitFullScreen:))]
        fn did_exit_fullscreen(&self, _note: &NSNotification) {
            self.recenter();
            self.notify(false);
        }
    }
);

impl TitleBarObserver {
    fn recenter(&self) {
        let ivars = self.ivars();
        center_buttons(&ivars.window, ivars.row_height.get());
    }

    fn notify(&self, fullscreen: bool) {
        let ivars = self.ivars();
        let _ = ivars.proxy.send_event(AccessibilityEvent::Fullscreen {
            window: ivars.winit_id,
            fullscreen,
        });
    }
}

/// Per-window AppKit state. Dropping it stops the notifications.
pub(super) struct TitleBar {
    observer: Retained<TitleBarObserver>,
}

impl TitleBar {
    pub(super) fn attach(
        window: &winit::window::Window,
        proxy: EventLoopProxy<AccessibilityEvent>,
    ) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let ns = ns_window(window)?;
        let observer = TitleBarObserver::alloc(mtm).set_ivars(ObserverIvars {
            window: ns.clone(),
            winit_id: window.id(),
            proxy,
            row_height: Cell::new(DEFAULT_ROW_HEIGHT),
        });
        // SAFETY: NSObject's designated initializer.
        let observer: Retained<TitleBarObserver> = unsafe { msg_send![super(observer), init] };
        let center = NSNotificationCenter::defaultCenter();
        let object: &AnyObject = &ns;
        // SAFETY: each selector exists on the observer and takes one
        // NSNotification. The observer is removed in `Drop`.
        unsafe {
            center.addObserver_selector_name_object(
                &observer,
                sel!(windowDidResize:),
                Some(NSWindowDidResizeNotification),
                Some(object),
            );
            center.addObserver_selector_name_object(
                &observer,
                sel!(windowDidEnterFullScreen:),
                Some(NSWindowDidEnterFullScreenNotification),
                Some(object),
            );
            center.addObserver_selector_name_object(
                &observer,
                sel!(windowDidExitFullScreen:),
                Some(NSWindowDidExitFullScreenNotification),
                Some(object),
            );
        }
        refresh_double_click_action();
        let title_bar = TitleBar { observer };
        title_bar.recenter();
        Some(title_bar)
    }

    pub(super) fn set_row_height(&self, height: f64) {
        if height > 0.0 {
            self.observer.ivars().row_height.set(height);
        }
        self.recenter();
    }

    pub(super) fn recenter(&self) {
        self.observer.recenter();
    }
}

impl Drop for TitleBar {
    fn drop(&mut self) {
        let center = NSNotificationCenter::defaultCenter();
        // SAFETY: removes the registrations made in `attach`.
        unsafe { center.removeObserver(&self.observer) };
    }
}
