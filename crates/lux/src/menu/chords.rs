//! Menu key letters and the one-shot claim that stops a native menu
//! key equivalent from running the same command again through the keymap.

use std::cell::Cell;

use super::MenuAction;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Menu,
    Key,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
struct Row {
    action: MenuAction,
    key: &'static str,
}

/// Key equivalents the native menu installs. The macOS builder reads these
/// so the menu and this table cannot drift. A capital letter is the shifted
/// equivalent (`cmd+shift`).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const ROWS: &[Row] = &[
    Row {
        action: MenuAction::NewFile,
        key: "n",
    },
    Row {
        action: MenuAction::OpenFile,
        key: "o",
    },
    Row {
        action: MenuAction::Save,
        key: "s",
    },
    Row {
        action: MenuAction::SaveAs,
        key: "S",
    },
    Row {
        action: MenuAction::CloseTab,
        key: "w",
    },
    Row {
        action: MenuAction::Undo,
        key: "z",
    },
    Row {
        action: MenuAction::Redo,
        key: "Z",
    },
    Row {
        action: MenuAction::Cut,
        key: "x",
    },
    Row {
        action: MenuAction::Copy,
        key: "c",
    },
    Row {
        action: MenuAction::Paste,
        key: "v",
    },
    Row {
        action: MenuAction::SelectAll,
        key: "a",
    },
    Row {
        action: MenuAction::ToggleComment,
        key: "/",
    },
    Row {
        action: MenuAction::DuplicateLine,
        key: "D",
    },
    Row {
        action: MenuAction::DeleteLine,
        key: "K",
    },
    Row {
        action: MenuAction::JoinLines,
        key: "j",
    },
    Row {
        action: MenuAction::AddNextOccurrence,
        key: "d",
    },
    Row {
        action: MenuAction::Find,
        key: "f",
    },
    Row {
        action: MenuAction::GotoAnything,
        key: "p",
    },
    Row {
        action: MenuAction::CommandPalette,
        key: "P",
    },
    Row {
        action: MenuAction::Preferences,
        key: ",",
    },
];

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn key(action: MenuAction) -> &'static str {
    ROWS.iter()
        .find(|row| row.action == action)
        .map(|row| row.key)
        .unwrap_or("")
}

pub(crate) fn action_for_command(cmd: &str) -> Option<MenuAction> {
    Some(match cmd {
        "save" => MenuAction::Save,
        "undo" => MenuAction::Undo,
        "redo" => MenuAction::Redo,
        "cut" => MenuAction::Cut,
        "copy" => MenuAction::Copy,
        "paste" => MenuAction::Paste,
        "select_all" => MenuAction::SelectAll,
        "toggle_comment" => MenuAction::ToggleComment,
        "duplicate_line" => MenuAction::DuplicateLine,
        "delete_line" => MenuAction::DeleteLine,
        "join_lines" => MenuAction::JoinLines,
        "select_next_occurrence" => MenuAction::AddNextOccurrence,
        "find" => MenuAction::Find,
        "quick_open" => MenuAction::GotoAnything,
        "command_palette" => MenuAction::CommandPalette,
        "open_settings" | "preferences.open_settings_ui" => MenuAction::Preferences,
        _ => return None,
    })
}

/// `true` when the caller should run `request`. A menu chord that the menu
/// already ran in this window returns `false`.
pub(crate) fn claim_document_request(request: &crate::document::Request) -> bool {
    use crate::document::Request;
    let action = match request {
        Request::New => MenuAction::NewFile,
        Request::Open(None) => MenuAction::OpenFile,
        Request::Open(Some(_)) | Request::SaveAll(_) => return true,
        Request::Save { save_as: false } => MenuAction::Save,
        Request::Save { save_as: true } => MenuAction::SaveAs,
        Request::Close => MenuAction::CloseTab,
    };
    claim(action, Source::Key)
}

thread_local! {
    static RECENT: Cell<Option<(MenuAction, Source)>> = const { Cell::new(None) };
}

/// The menu key equivalent and the keymap both deliver one press. The first
/// source runs it. The other source of that same press is swallowed. A second
/// press from the same source runs again, so Command+D can add the next match.
fn claim(action: MenuAction, source: Source) -> bool {
    RECENT.with(|slot| {
        if let Some((prev, prev_source)) = slot.get()
            && prev == action
            && prev_source != source
        {
            slot.set(None);
            return false;
        }
        slot.set(Some((action, source)));
        true
    })
}

pub(crate) fn claim_menu(action: MenuAction) -> bool {
    claim(action, Source::Menu)
}

/// The command a pending chord plus this menu key completes, if any.
///
/// The menu key equivalent runs before the view sees the key. `Cmd+D` is
/// both "select next" and the second half of `Cmd+K Cmd+D`, so the menu
/// has to finish the chord instead of selecting another match.
pub(crate) fn menu_completes_pending(
    keymap: &lux_config::Keymap,
    pending: &lux_config::Chord,
    action: MenuAction,
) -> Option<String> {
    let letter = key(action);
    if letter.is_empty() {
        return None;
    }
    let shift = letter.chars().next()?.is_ascii_uppercase();
    let second = lux_config::Chord {
        ctrl: false,
        alt: false,
        shift,
        cmd: true,
        key: letter.to_ascii_lowercase(),
    };
    let sequence = lux_config::ChordSequence(vec![pending.clone(), second]);
    keymap
        .command_for(&sequence, lux_config::Platform::Mac)
        .map(str::to_owned)
}

pub(crate) fn claim_key(action: MenuAction) -> bool {
    claim(action, Source::Key)
}

#[cfg(test)]
pub(crate) fn clear_claim() {
    RECENT.with(|slot| slot.set(None));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_letters_match_the_sublime_keymap() {
        let map = lux_config::keymap::presets::sublime();
        let mac = lux_config::Platform::Mac;
        let pairs = [
            (MenuAction::Save, "save"),
            (MenuAction::Undo, "undo"),
            (MenuAction::Redo, "redo"),
            (MenuAction::ToggleComment, "toggle_comment"),
            (MenuAction::DuplicateLine, "duplicate_line"),
            (MenuAction::DeleteLine, "delete_line"),
            (MenuAction::JoinLines, "join_lines"),
            (MenuAction::AddNextOccurrence, "select_next_occurrence"),
            (MenuAction::Find, "find"),
            (MenuAction::GotoAnything, "quick_open"),
            (MenuAction::CommandPalette, "command_palette"),
            (MenuAction::Preferences, "open_settings"),
        ];
        for (action, command) in pairs {
            let chord = map
                .binding_for(command, mac)
                .unwrap_or_else(|| panic!("{command} has no mac binding"))
                .to_string();
            let letter = key(action);
            let token = chord.split('+').next_back().unwrap_or("");
            assert!(
                token.eq_ignore_ascii_case(letter),
                "{command} is {chord}, menu letter is {letter}"
            );
            assert_eq!(action_for_command(command), Some(action));
        }
        assert_eq!(key(MenuAction::NewFile), "n");
        assert_eq!(key(MenuAction::Cut), "x");
        assert_eq!(key(MenuAction::Copy), "c");
        assert_eq!(key(MenuAction::Paste), "v");
        assert_eq!(key(MenuAction::SelectAll), "a");
    }

    #[test]
    fn command_d_after_command_k_is_skip_not_another_match() {
        let keymap = lux_config::keymap::presets::sublime();
        let pending: lux_config::Chord = "cmd+k".parse().unwrap();
        let command =
            menu_completes_pending(&keymap, &pending, MenuAction::AddNextOccurrence).unwrap();
        assert_eq!(command, "skip_next_occurrence");
        let other: lux_config::Chord = "cmd+b".parse().unwrap();
        assert!(menu_completes_pending(&keymap, &other, MenuAction::AddNextOccurrence).is_none());
    }

    #[test]
    fn the_other_source_is_swallowed_and_a_repeat_is_not() {
        clear_claim();
        assert!(claim(MenuAction::NewFile, Source::Menu));
        assert!(!claim(MenuAction::NewFile, Source::Key));
        assert!(claim(MenuAction::NewFile, Source::Key));
        assert!(claim(MenuAction::NewFile, Source::Key));
        assert!(!claim(MenuAction::NewFile, Source::Menu));
    }
}
