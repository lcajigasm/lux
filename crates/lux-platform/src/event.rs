//! Input and lifecycle events, in backend-independent form.
//!
//! These mirror the shape every desktop and mobile platform already agrees
//! on (press/release, preedit/commit IME, touch phases) without borrowing
//! `winit`'s concrete types, so a future UIKit or Android backend can
//! produce the same [`PlatformEvent`] without depending on `winit` at all.
//! [`Key`] is the layout-aware logical key (what the user sees typed).
//! [`PhysicalKey`] is the key position, so a shortcut such as Command+/
//! matches the same physical key on a Spanish keyboard as on a US one.

use dpi::{PhysicalPosition, PhysicalSize};

/// A key press or release, in logical (layout-aware) form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    /// The text this key press inserts, if any (already layout-mapped, e.g.
    /// dead-key composition applied). `None` for releases and non-printing
    /// keys.
    pub text: Option<String>,
    pub state: ElementState,
    pub repeat: bool,
    /// Key position, independent of the characters the layout produces.
    /// `None` when the backend did not report one (tests that only care
    /// about the logical key).
    pub physical: Option<PhysicalKey>,
}

/// Layout-independent key position. Letters and digits use their US labels.
/// Punctuation variants are the US key that occupies that position, which is
/// what Sublime keymaps name (`cmd+/`, `cmd+[`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicalKey {
    Letter(char),
    Digit(u8),
    Slash,
    BracketLeft,
    BracketRight,
    Backslash,
    Minus,
    Equal,
    Semicolon,
    Quote,
    Comma,
    Period,
    Backquote,
}

impl PhysicalKey {
    /// Token used by keymap chords (`"f"`, `"/"`, `"["`).
    pub fn chord_token(self) -> String {
        match self {
            Self::Letter(c) => c.to_ascii_lowercase().to_string(),
            Self::Digit(n) => n.to_string(),
            Self::Slash => "/".to_string(),
            Self::BracketLeft => "[".to_string(),
            Self::BracketRight => "]".to_string(),
            Self::Backslash => "\\".to_string(),
            Self::Minus => "-".to_string(),
            Self::Equal => "=".to_string(),
            Self::Semicolon => ";".to_string(),
            Self::Quote => "'".to_string(),
            Self::Comma => ",".to_string(),
            Self::Period => ".".to_string(),
            Self::Backquote => "`".to_string(),
        }
    }
}

/// The logical key identity of a key event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    Named(NamedKey),
    /// A character-producing key, carrying the character it produces
    /// (before IME composition; composed text arrives via [`Ime::Commit`]).
    Character(String),
    Unidentified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NamedKey {
    Enter,
    Tab,
    Space,
    Backspace,
    Delete,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    CapsLock,
    Shift,
    Control,
    Alt,
    Meta,
    /// F1..=F24, carrying the number.
    F(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElementState {
    Pressed,
    Released,
}

/// Which modifier keys are currently held.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    /// Command on macOS, Windows key on Windows, Super on Linux.
    pub meta: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollDelta {
    /// Discrete lines/columns, as reported by a mouse wheel.
    Lines { x: f32, y: f32 },
    /// Precise pixel deltas, as reported by a trackpad.
    Pixels(PhysicalPosition<f64>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    /// Stable for the lifetime of one finger's contact; distinguishes
    /// concurrent touches.
    pub id: u64,
    pub phase: TouchPhase,
    pub position: PhysicalPosition<f64>,
    /// Pressure, 0.0-1.0, if the hardware reports it.
    pub force: Option<f64>,
}

/// IME composition state: preedit text (not yet committed, shown inline but
/// not part of the buffer) and commit (the final, buffer-bound text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ime {
    /// IME composition became available for this window.
    Enabled,
    /// The composition text changed; `cursor_range` is the byte range
    /// (start, end) of the composition cursor/selection within `text`, in
    /// the platform's reported units.
    Preedit {
        text: String,
        cursor_range: Option<(usize, usize)>,
    },
    /// The composition finished; `text` is final and belongs in the buffer.
    Commit(String),
    /// IME composition is no longer available for this window.
    Disabled,
}

/// Light or dark appearance reported by the OS. Distinct from the editor
/// theme the user picked: a named theme wins until the setting is `system`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Dark,
    Light,
}

/// A single input or lifecycle event delivered to
/// [`crate::PlatformEventHandler::window_event`].
#[derive(Debug, Clone, PartialEq)]
pub enum PlatformEvent {
    Resized(PhysicalSize<u32>),
    /// The window's outer origin changed, in physical pixels.
    WindowMoved(PhysicalPosition<i32>),
    /// The window's scale factor changed; a `Resized` event with the new
    /// physical size follows.
    ScaleFactorChanged {
        scale_factor: f64,
    },
    /// The OS appearance changed (macOS `ThemeChanged`, and the same event
    /// on Windows). Trackpad inertia is not a separate event: macOS delivers
    /// it as further [`PlatformEvent::MouseWheel`] pixel deltas.
    ColorSchemeChanged(ColorScheme),
    Focused(bool),
    Occluded(bool),
    CloseRequested,
    RedrawRequested,
    Keyboard(KeyEvent),
    ModifiersChanged(Modifiers),
    Ime(Ime),
    MouseMoved(PhysicalPosition<f64>),
    MouseButton {
        button: MouseButton,
        state: ElementState,
    },
    MouseWheel(ScrollDelta),
    MouseEntered,
    MouseLeft,
    Touch(Touch),
    /// The OS accessibility adapter queried this window for the first time
    /// and needs a real tree — respond with
    /// [`crate::PlatformWindow::update_accessibility_tree`] as soon as
    /// possible; until then, the platform's own adapter shows a temporary
    /// placeholder (see `docs/adr/0013-platform-accessibility-adapter-wiring.md`).
    AccessibilityInitialTreeRequested,
    /// A screen reader or other assistive technology invoked an action
    /// (e.g. move the caret, set a selection) against this window's
    /// accessibility tree.
    AccessibilityActionRequested(accesskit::ActionRequest),
    /// The OS accessibility adapter is no longer active for this window
    /// (e.g. the screen reader was turned off).
    AccessibilityDeactivated,
}
