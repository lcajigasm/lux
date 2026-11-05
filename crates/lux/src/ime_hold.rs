//! Holds an IME commit until the key that produced it is visible.
//!
//! On macOS, `interpretKeyEvents` queues `Ime::Commit` before the matching
//! `KeyboardInput`, and before that key's modifiers. Spanish ISO uses the
//! physical `[` key as a dead grave. Option+Command+[ therefore commits `` ` ``
//! into the document, and the following Option+Command+F (replace) is what
//! makes the grave show up in front of the existing text. A command chord
//! drops that commit. Plain typing inserts it once, on the key that carried
//! it, and that key's own character is not typed as well.

use std::cell::RefCell;

use lux_platform::Ime;

use crate::app::App;

thread_local! {
    static HELD: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(crate) fn observe(app: &mut App, event: Ime) {
    if let Ime::Commit(text) = event {
        HELD.with(|slot| *slot.borrow_mut() = Some(text));
        return;
    }
    app.apply_ime(event);
}

/// Inserts a held commit, or drops it when this key is a command chord.
///
/// Returns true when the commit was typed. The key event that carried it
/// must not also insert its own character, so `` ´ `` + `a` is one `á`.
pub(crate) fn settle(app: &mut App) -> bool {
    let held = HELD.with(|slot| slot.borrow_mut().take());
    if app.command_modifier() {
        if held.is_some() || app.ime.is_composing() {
            app.apply_ime(Ime::Disabled);
        }
        return false;
    }
    if let Some(text) = held {
        app.apply_ime(Ime::Commit(text));
        return true;
    }
    false
}
