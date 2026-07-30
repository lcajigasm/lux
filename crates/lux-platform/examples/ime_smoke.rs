//! Manual verification harness for Phase 2 deliverable 5 ("IME de
//! escritorio: preedit, área de cursor"). Not an automated test — IME
//! composition requires a real OS input source and a human typing into a
//! real window, which is exactly what this opens.
//!
//! Run with `cargo run --example ime_smoke -p lux-platform`, then see
//! `docs/receipts/phase-02-ime-escritorio-preedit-area-cursor.md` for the
//! exact step-by-step script (which input sources to switch to, what to
//! type, and what you should observe).
//!
//! There is no `wgpu` text-rendering pipeline yet (a separate, later
//! `lux-render` deliverable), so "preedit visible in its place" is shown
//! two honest ways that don't require one: the window title live-updates
//! with the current preedit text, and every `Ime` event is logged to the
//! terminal via [`lux_platform::ImeComposition`] — the same composition
//! state machine `cargo test -p lux-platform ime::` verifies headlessly.
//! What this example adds that the unit tests can't: a real
//! `set_ime_cursor_area` call, so you can see whether the OS actually
//! anchors its candidate window there.

use dpi::{PhysicalPosition, PhysicalSize};
use lux_platform::{
    ImeComposition, ImeUpdate, PlatformContext, PlatformEvent, PlatformEventHandler,
    PlatformWindow, WindowAttributes, WindowId,
};

/// Where the IME candidate window should anchor, and roughly the size of
/// one character cell — stand-ins for "wherever the text cursor currently
/// is," which a real editor would compute from `lux-ui`'s layout instead of
/// hardcoding.
const CURSOR_AREA_POSITION: PhysicalPosition<i32> = PhysicalPosition::new(120, 160);
const CURSOR_AREA_SIZE: PhysicalSize<u32> = PhysicalSize::new(2, 22);

struct ImeSmokeHandler {
    window: Option<Box<dyn PlatformWindow>>,
    composition: ImeComposition,
}

impl ImeSmokeHandler {
    fn new() -> Self {
        Self {
            window: None,
            composition: ImeComposition::new(),
        }
    }

    fn set_title(&self, suffix: &str) {
        if let Some(window) = &self.window {
            window.set_title(&format!("Lux IME smoke — {suffix}"));
        }
    }
}

impl PlatformEventHandler for ImeSmokeHandler {
    fn resumed(&mut self, ctx: &dyn PlatformContext) {
        let window = ctx
            .create_window(WindowAttributes {
                title: "Lux IME smoke — waiting for input".to_string(),
                ..Default::default()
            })
            .expect("failed to create the smoke-test window");

        // Off by default (a real editor only enables it while a text input
        // has focus); this example's whole window is the "text input."
        window.set_ime_allowed(true);
        window.set_ime_cursor_area(CURSOR_AREA_POSITION, CURSOR_AREA_SIZE);

        println!("=== Lux IME smoke test ===");
        println!(
            "IME candidate window should anchor near physical ({}, {}).",
            CURSOR_AREA_POSITION.x, CURSOR_AREA_POSITION.y
        );
        println!(
            "Switch to a Japanese input source and type e.g. 'kanji', then Enter/Space to confirm."
        );
        println!(
            "Then switch to a Spanish (or any accent-dead-key) layout and type e.g. option+e, e for 'é'."
        );
        println!(
            "Watch this terminal for Ime events and the window's title bar for live preedit text."
        );
        println!("Close the window (or Ctrl+C here) to exit.");

        self.window = Some(window);
    }

    fn window_event(&mut self, ctx: &dyn PlatformContext, _window: WindowId, event: PlatformEvent) {
        match event {
            PlatformEvent::CloseRequested => ctx.exit(),
            PlatformEvent::Ime(ime_event) => {
                println!("Ime event: {ime_event:?}");
                match self.composition.apply(ime_event) {
                    ImeUpdate::Idle => self.set_title("waiting for input"),
                    ImeUpdate::Composing { text, cursor_range } => {
                        println!("  -> composing {text:?}, cursor_range={cursor_range:?}");
                        self.set_title(&format!("composing: {text}"));
                    }
                    ImeUpdate::Committed(text) => {
                        println!("  -> committed {text:?}");
                        self.set_title(&format!("committed: {text}"));
                    }
                }
            }
            PlatformEvent::Keyboard(key_event) => {
                if let Some(text) = &key_event.text {
                    println!("Keyboard text (non-IME path): {text:?}");
                }
            }
            _ => {}
        }
    }
}

fn main() {
    lux_platform::desktop::run(ImeSmokeHandler::new()).expect("desktop event loop failed");
}
