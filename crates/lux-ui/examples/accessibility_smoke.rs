//! Manual verification harness for Phase 2 deliverable 6 ("accesskit
//! conectado"). Opens a real window, wires a real `accesskit_winit`
//! adapter to `lux_ui::accessibility::build_tree`, and lets a screen
//! reader (VoiceOver/NVDA/Orca) read the exposed multiline text control.
//!
//! Deliberately uses raw `winit` + `accesskit_winit` instead of
//! `lux_platform`: no accessibility-adapter wiring exists in
//! `lux-platform` yet (a different, already-closed deliverable — adding it
//! there is out of this one's scope), and `accesskit_winit` is the
//! sanctioned way to attach a real platform accessibility adapter to a
//! winit window without hand-rolling NSView/HWND subclassing ourselves
//! (which is what a real adapter needs to actually receive OS
//! accessibility queries — a raw `accesskit_macos::Adapter` alone doesn't
//! do that). What's under test here is `lux_ui::accessibility::build_tree`
//! — already fully exercised headlessly by
//! `cargo test -p lux-ui accessibility::` — this example only proves a
//! real screen reader can consume its output.
//!
//! Run with `cargo run --example accessibility_smoke -p lux-ui`. See
//! `docs/receipts/phase-02-accesskit-conectado.md` for the exact
//! step-by-step verification script.

use std::sync::{Arc, Mutex};

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};
use accesskit_winit::Adapter;
use lux_core::buffer::Buffer;
use lux_core::multicursor::MultiCursor;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_render::TextLayoutContext;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

const AHEM_TTF: &[u8] = include_bytes!("../tests/fixtures/ahem.ttf");
const FONT_SIZE: f32 = 16.0;

struct EditorState {
    text_layout: TextLayoutContext,
    family: String,
    buffer: Buffer,
    cursors: MultiCursor,
}

impl EditorState {
    fn new() -> Self {
        let mut text_layout = TextLayoutContext::new();
        let family = text_layout
            .register_font(AHEM_TTF)
            .expect("ahem.ttf must register");
        let buffer = Buffer::from_bytes(
            b"Lux accessibility smoke test.\n\
              This is the second line of the document.\n\
              Move the caret with the Left and Right arrow keys.",
        )
        .unwrap();
        let cursors = MultiCursor::new(Selection::cursor(Position::document_start()));
        Self {
            text_layout,
            family,
            buffer,
            cursors,
        }
    }

    fn build_tree(&mut self) -> TreeUpdate {
        lux_ui::accessibility::build_tree(
            &mut self.text_layout,
            &self.family,
            FONT_SIZE,
            &self.buffer,
            &self.cursors,
        )
    }

    /// The only "editing" this smoke test does: move a single collapsed
    /// caret one grapheme at a time, real `lux_core` movement, so a screen
    /// reader has something real to track changing.
    fn move_caret(&mut self, forward: bool) {
        let pos = self.cursors.primary().head;
        let next = if forward {
            pos.next_grapheme(&self.buffer)
        } else {
            pos.prev_grapheme(&self.buffer)
        };
        self.cursors = MultiCursor::new(Selection::cursor(next));
    }
}

struct NoopActionHandler;
impl ActionHandler for NoopActionHandler {
    fn do_action(&mut self, _request: ActionRequest) {}
}

struct NoopDeactivationHandler;
impl DeactivationHandler for NoopDeactivationHandler {
    fn deactivate_accessibility(&mut self) {}
}

/// Returns the tree synchronously (no placeholder tree needed) — see
/// `Adapter::with_direct_handlers`'s own doc comment on why that's
/// preferable to the event-loop-proxy constructor for an app whose state
/// (unlike a real editor spread across threads) is trivially available
/// right here.
struct SyncActivationHandler {
    state: Arc<Mutex<EditorState>>,
}
impl ActivationHandler for SyncActivationHandler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        Some(self.state.lock().unwrap().build_tree())
    }
}

struct App {
    state: Arc<Mutex<EditorState>>,
    window: Option<Window>,
    adapter: Option<Adapter>,
}

impl App {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(EditorState::new())),
            window: None,
            adapter: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // `accesskit_winit` requires the window to still be invisible when
        // the adapter is created, then shown right after.
        let attrs = WindowAttributes::default()
            .with_title("Lux accessibility smoke test")
            .with_visible(false);
        let window = event_loop
            .create_window(attrs)
            .expect("failed to create the smoke-test window");

        let activation_handler = SyncActivationHandler {
            state: Arc::clone(&self.state),
        };
        let adapter = Adapter::with_direct_handlers(
            event_loop,
            &window,
            activation_handler,
            NoopActionHandler,
            NoopDeactivationHandler,
        );
        window.set_visible(true);

        println!("=== Lux accessibility smoke test ===");
        println!("Turn on VoiceOver (Cmd+F5) / NVDA / Orca, then click this window to focus it.");
        println!(
            "You should hear it announced as a multi-line text field with 3 lines of content."
        );
        println!(
            "Use the Left/Right arrow keys to move the caret; the screen reader should announce the new position."
        );
        println!("Close the window (or Ctrl+C here) to exit.");

        self.window = Some(window);
        self.adapter = Some(adapter);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let (Some(window), Some(adapter)) = (&self.window, &mut self.adapter) else {
            return;
        };
        // Must run before our own handling, per `Adapter::process_event`'s
        // own doc comment.
        adapter.process_event(window, &event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if key_event.state != ElementState::Pressed {
                    return;
                }
                let moved = match key_event.logical_key {
                    Key::Named(NamedKey::ArrowRight) => {
                        self.state.lock().unwrap().move_caret(true);
                        true
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.state.lock().unwrap().move_caret(false);
                        true
                    }
                    _ => false,
                };
                if moved {
                    let update = self.state.lock().unwrap().build_tree();
                    adapter.update_if_active(|| update);
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("failed to create the winit event loop");
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("event loop failed");
}
