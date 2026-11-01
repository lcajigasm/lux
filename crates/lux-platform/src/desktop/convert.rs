//! `winit` type -> `lux_platform` type conversions. Isolated here so the
//! rest of the desktop backend, and everything above this crate, never
//! matches on a `winit` enum directly.

use crate::event::{
    ColorScheme, ElementState, Ime, Key, KeyEvent, Modifiers, MouseButton, NamedKey, PlatformEvent,
    ScrollDelta, Touch, TouchPhase,
};

pub(super) fn element_state(state: winit::event::ElementState) -> ElementState {
    match state {
        winit::event::ElementState::Pressed => ElementState::Pressed,
        winit::event::ElementState::Released => ElementState::Released,
    }
}

pub(super) fn modifiers(mods: winit::keyboard::ModifiersState) -> Modifiers {
    Modifiers {
        shift: mods.shift_key(),
        control: mods.control_key(),
        alt: mods.alt_key(),
        meta: mods.super_key(),
    }
}

pub(super) fn named_key(named: winit::keyboard::NamedKey) -> Option<NamedKey> {
    use winit::keyboard::NamedKey as W;
    Some(match named {
        W::Enter => NamedKey::Enter,
        W::Tab => NamedKey::Tab,
        W::Space => NamedKey::Space,
        W::Backspace => NamedKey::Backspace,
        W::Delete => NamedKey::Delete,
        W::Escape => NamedKey::Escape,
        W::ArrowUp => NamedKey::ArrowUp,
        W::ArrowDown => NamedKey::ArrowDown,
        W::ArrowLeft => NamedKey::ArrowLeft,
        W::ArrowRight => NamedKey::ArrowRight,
        W::Home => NamedKey::Home,
        W::End => NamedKey::End,
        W::PageUp => NamedKey::PageUp,
        W::PageDown => NamedKey::PageDown,
        W::Insert => NamedKey::Insert,
        W::CapsLock => NamedKey::CapsLock,
        W::Shift => NamedKey::Shift,
        W::Control => NamedKey::Control,
        W::Alt => NamedKey::Alt,
        // Command on macOS, the Windows key on Windows, Super on Linux all
        // arrive as one of these two in winit; an editor has no use for
        // telling them apart, so both collapse to `Meta`.
        W::Meta | W::Super => NamedKey::Meta,
        W::F1 => NamedKey::F(1),
        W::F2 => NamedKey::F(2),
        W::F3 => NamedKey::F(3),
        W::F4 => NamedKey::F(4),
        W::F5 => NamedKey::F(5),
        W::F6 => NamedKey::F(6),
        W::F7 => NamedKey::F(7),
        W::F8 => NamedKey::F(8),
        W::F9 => NamedKey::F(9),
        W::F10 => NamedKey::F(10),
        W::F11 => NamedKey::F(11),
        W::F12 => NamedKey::F(12),
        W::F13 => NamedKey::F(13),
        W::F14 => NamedKey::F(14),
        W::F15 => NamedKey::F(15),
        W::F16 => NamedKey::F(16),
        W::F17 => NamedKey::F(17),
        W::F18 => NamedKey::F(18),
        W::F19 => NamedKey::F(19),
        W::F20 => NamedKey::F(20),
        W::F21 => NamedKey::F(21),
        W::F22 => NamedKey::F(22),
        W::F23 => NamedKey::F(23),
        W::F24 => NamedKey::F(24),
        _ => return None,
    })
}

pub(super) fn key(logical: &winit::keyboard::Key) -> Key {
    match logical {
        winit::keyboard::Key::Named(n) => {
            named_key(*n).map(Key::Named).unwrap_or(Key::Unidentified)
        }
        winit::keyboard::Key::Character(s) => Key::Character(s.to_string()),
        winit::keyboard::Key::Unidentified(_) | winit::keyboard::Key::Dead(_) => Key::Unidentified,
    }
}

// `winit::event::KeyEvent` carries a private `platform_specific` field, so it
// can't be constructed outside `winit` (not even in our own tests). Taking
// the fields we actually use, rather than the whole struct, keeps this
// conversion unit-testable.
pub(super) fn key_event(
    logical_key: &winit::keyboard::Key,
    text: Option<&str>,
    state: winit::event::ElementState,
    repeat: bool,
) -> KeyEvent {
    KeyEvent {
        key: key(logical_key),
        text: text.map(str::to_string),
        state: element_state(state),
        repeat,
    }
}

pub(super) fn mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Back => MouseButton::Back,
        winit::event::MouseButton::Forward => MouseButton::Forward,
        winit::event::MouseButton::Other(code) => MouseButton::Other(code),
    }
}

pub(super) fn color_scheme(theme: winit::window::Theme) -> ColorScheme {
    match theme {
        winit::window::Theme::Dark => ColorScheme::Dark,
        winit::window::Theme::Light => ColorScheme::Light,
    }
}

pub(super) fn scroll_delta(delta: winit::event::MouseScrollDelta) -> ScrollDelta {
    match delta {
        winit::event::MouseScrollDelta::LineDelta(x, y) => ScrollDelta::Lines { x, y },
        winit::event::MouseScrollDelta::PixelDelta(p) => {
            ScrollDelta::Pixels(dpi::PhysicalPosition::new(p.x, p.y))
        }
    }
}

// `winit::event::Touch` carries a private `device_id` field with no public
// constructor, so — same reasoning as `key_event` above — convert from the
// individual fields instead of the struct.
pub(super) fn touch(
    id: u64,
    phase: winit::event::TouchPhase,
    location: winit::dpi::PhysicalPosition<f64>,
    force: Option<winit::event::Force>,
) -> Touch {
    Touch {
        id,
        phase: touch_phase(phase),
        position: dpi::PhysicalPosition::new(location.x, location.y),
        force: force.map(|f| f.normalized()),
    }
}

fn touch_phase(phase: winit::event::TouchPhase) -> TouchPhase {
    match phase {
        winit::event::TouchPhase::Started => TouchPhase::Started,
        winit::event::TouchPhase::Moved => TouchPhase::Moved,
        winit::event::TouchPhase::Ended => TouchPhase::Ended,
        winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
    }
}

pub(super) fn ime(event: winit::event::Ime) -> Ime {
    match event {
        winit::event::Ime::Enabled => Ime::Enabled,
        winit::event::Ime::Preedit(text, cursor_range) => Ime::Preedit { text, cursor_range },
        winit::event::Ime::Commit(text) => Ime::Commit(text),
        winit::event::Ime::Disabled => Ime::Disabled,
    }
}

/// Translates a `winit::event::WindowEvent` into our [`PlatformEvent`].
/// `None` means the event has no equivalent worth exposing (e.g. dropped
/// files, which nothing in this crate's scope consumes yet).
pub(super) fn window_event(event: winit::event::WindowEvent) -> Option<PlatformEvent> {
    use winit::event::WindowEvent as W;
    Some(match event {
        W::Resized(size) => PlatformEvent::Resized(size),
        // `inner_size_writer` lets a caller request a specific resize during
        // this callback; left untouched, winit resizes to the OS-suggested
        // size and emits a separate `Resized` event, which is all this crate
        // needs today.
        W::ScaleFactorChanged {
            scale_factor,
            inner_size_writer: _,
        } => PlatformEvent::ScaleFactorChanged { scale_factor },
        W::ThemeChanged(theme) => PlatformEvent::ColorSchemeChanged(color_scheme(theme)),
        W::CloseRequested => PlatformEvent::CloseRequested,
        W::Focused(f) => PlatformEvent::Focused(f),
        W::Occluded(o) => PlatformEvent::Occluded(o),
        W::RedrawRequested => PlatformEvent::RedrawRequested,
        W::KeyboardInput {
            event,
            is_synthetic: false,
            ..
        } => PlatformEvent::Keyboard(key_event(
            &event.logical_key,
            event.text.as_deref(),
            event.state,
            event.repeat,
        )),
        W::ModifiersChanged(m) => PlatformEvent::ModifiersChanged(modifiers(m.state())),
        W::Ime(e) => PlatformEvent::Ime(ime(e)),
        W::CursorMoved { position, .. } => PlatformEvent::MouseMoved(position),
        W::CursorEntered { .. } => PlatformEvent::MouseEntered,
        W::CursorLeft { .. } => PlatformEvent::MouseLeft,
        W::MouseInput { state, button, .. } => PlatformEvent::MouseButton {
            button: mouse_button(button),
            state: element_state(state),
        },
        W::MouseWheel { delta, .. } => PlatformEvent::MouseWheel(scroll_delta(delta)),
        W::Touch(t) => PlatformEvent::Touch(touch(t.id, t.phase, t.location, t.force)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_map_each_bit_independently() {
        let m = winit::keyboard::ModifiersState::SHIFT | winit::keyboard::ModifiersState::CONTROL;
        let converted = modifiers(m);
        assert!(converted.shift);
        assert!(converted.control);
        assert!(!converted.alt);
        assert!(!converted.meta);
    }

    #[test]
    fn super_and_meta_both_collapse_to_meta() {
        assert_eq!(
            named_key(winit::keyboard::NamedKey::Meta),
            Some(NamedKey::Meta)
        );
        assert_eq!(
            named_key(winit::keyboard::NamedKey::Super),
            Some(NamedKey::Meta)
        );
    }

    #[test]
    fn function_keys_carry_their_number() {
        assert_eq!(
            named_key(winit::keyboard::NamedKey::F1),
            Some(NamedKey::F(1))
        );
        assert_eq!(
            named_key(winit::keyboard::NamedKey::F24),
            Some(NamedKey::F(24))
        );
    }

    #[test]
    fn an_uncovered_named_key_falls_back_to_unidentified() {
        assert_eq!(named_key(winit::keyboard::NamedKey::ZoomToggle), None);
        assert_eq!(
            key(&winit::keyboard::Key::Named(
                winit::keyboard::NamedKey::ZoomToggle
            )),
            Key::Unidentified
        );
    }

    #[test]
    fn character_keys_carry_their_text() {
        assert_eq!(
            key(&winit::keyboard::Key::Character("a".into())),
            Key::Character("a".to_string())
        );
    }

    #[test]
    fn ime_preedit_keeps_its_cursor_range() {
        let converted = ime(winit::event::Ime::Preedit(
            "nihongo".to_string(),
            Some((2, 4)),
        ));
        assert_eq!(
            converted,
            Ime::Preedit {
                text: "nihongo".to_string(),
                cursor_range: Some((2, 4))
            }
        );
    }

    #[test]
    fn touch_force_is_normalized_to_0_1() {
        let converted = touch(
            7,
            winit::event::TouchPhase::Moved,
            winit::dpi::PhysicalPosition::new(1.0, 2.0),
            Some(winit::event::Force::Normalized(0.5)),
        );
        assert_eq!(converted.id, 7);
        assert_eq!(converted.phase, TouchPhase::Moved);
        assert_eq!(converted.force, Some(0.5));
        assert_eq!(converted.position, dpi::PhysicalPosition::new(1.0, 2.0));
    }

    #[test]
    fn key_event_carries_its_text_state_and_repeat_flag() {
        let converted = key_event(
            &winit::keyboard::Key::Character("a".into()),
            Some("a"),
            winit::event::ElementState::Pressed,
            false,
        );
        assert_eq!(converted.key, Key::Character("a".to_string()));
        assert_eq!(converted.text, Some("a".to_string()));
        assert_eq!(converted.state, ElementState::Pressed);
        assert!(!converted.repeat);
    }

    #[test]
    fn line_and_pixel_scroll_deltas_stay_distinguishable() {
        assert_eq!(
            scroll_delta(winit::event::MouseScrollDelta::LineDelta(1.0, -2.0)),
            ScrollDelta::Lines { x: 1.0, y: -2.0 }
        );
        assert_eq!(
            scroll_delta(winit::event::MouseScrollDelta::PixelDelta(
                winit::dpi::PhysicalPosition::new(3.0, 4.0)
            )),
            ScrollDelta::Pixels(dpi::PhysicalPosition::new(3.0, 4.0))
        );
    }

    #[test]
    fn theme_changed_maps_to_color_scheme() {
        assert_eq!(
            window_event(winit::event::WindowEvent::ThemeChanged(
                winit::window::Theme::Light
            )),
            Some(PlatformEvent::ColorSchemeChanged(ColorScheme::Light))
        );
        assert_eq!(color_scheme(winit::window::Theme::Dark), ColorScheme::Dark);
    }
}
