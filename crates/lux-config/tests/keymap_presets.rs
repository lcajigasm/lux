//! Verifies the three shipped keymap presets against their documented
//! shortcuts (this phase's acceptance criterion: "los tres presets de
//! keymap pasan un test que verifica los atajos documentados"), plus the
//! platform-resolution and conflict-detection machinery they're built on.
//! See `crates/lux-config/keymaps/*.jsonc` for the data itself and
//! `docs/receipts/phase-05-keymaps-sublime-vscode-jetbrains-como.md` for
//! which shortcuts were confirmed against each vendor's own docs.

use lux_config::keymap::{self as km, Platform};

/// Compares by parsed structure, not by `Display`'s fixed rendering order —
/// real editor docs don't agree on modifier order across every command
/// (VS Code's own docs write "Shift+Alt+Down" but "Ctrl+Shift+P"), and a
/// `Chord`'s modifiers are boolean flags, not an ordered sequence, so
/// structural equality is what "matches the documented shortcut" actually
/// means here.
fn assert_binding(keymap: &km::Keymap, platform: Platform, command: &str, expected: &str) {
    let resolved = keymap
        .binding_for(command, platform)
        .unwrap_or_else(|| panic!("{} preset has no binding for {command:?}", keymap.name));
    let expected: km::ChordSequence = expected
        .parse()
        .unwrap_or_else(|e| panic!("bad expected chord {expected:?}: {e}"));
    assert_eq!(
        *resolved, expected,
        "{} preset: {command:?} on {platform:?}",
        keymap.name
    );
}

mod keymap {
    use super::*;

    #[test]
    fn sublime_preset_matches_its_documented_shortcuts() {
        let preset = km::presets::sublime();
        assert_binding(
            &preset,
            Platform::Windows,
            "command_palette",
            "ctrl+shift+p",
        );
        assert_binding(&preset, Platform::Mac, "command_palette", "cmd+shift+p");
        assert_binding(&preset, Platform::Windows, "quick_open", "ctrl+p");
        assert_binding(&preset, Platform::Mac, "quick_open", "cmd+p");
        assert_binding(&preset, Platform::Windows, "find_in_files", "ctrl+shift+f");
        assert_binding(&preset, Platform::Mac, "find_in_files", "cmd+shift+f");
        assert_binding(&preset, Platform::Windows, "next_result", "f4");
        assert_binding(&preset, Platform::Mac, "next_result", "f4");
        assert_binding(&preset, Platform::Windows, "prev_result", "shift+f4");
        assert_binding(&preset, Platform::Mac, "prev_result", "shift+f4");
        // Documented exception: kept as Ctrl on mac too, not remapped to Cmd.
        assert_binding(&preset, Platform::Mac, "goto_line", "ctrl+g");
        assert_binding(&preset, Platform::Windows, "duplicate_line", "ctrl+shift+d");
        assert_binding(&preset, Platform::Mac, "duplicate_line", "cmd+shift+d");
        assert_binding(&preset, Platform::Windows, "delete_line", "ctrl+shift+k");
        assert_binding(&preset, Platform::Mac, "delete_line", "cmd+shift+k");
        assert_binding(&preset, Platform::Windows, "swap_line_up", "ctrl+shift+up");
        assert_binding(&preset, Platform::Mac, "swap_line_up", "ctrl+cmd+up");
        assert_binding(
            &preset,
            Platform::Windows,
            "swap_line_down",
            "ctrl+shift+down",
        );
        assert_binding(&preset, Platform::Mac, "swap_line_down", "ctrl+cmd+down");
        assert_binding(&preset, Platform::Windows, "join_lines", "ctrl+j");
        assert_binding(&preset, Platform::Mac, "join_lines", "cmd+j");
        assert_binding(&preset, Platform::Windows, "toggle_comment", "ctrl+/");
        assert_binding(
            &preset,
            Platform::Windows,
            "select_next_occurrence",
            "ctrl+d",
        );
        assert_binding(&preset, Platform::Mac, "select_next_occurrence", "cmd+d");
        assert_binding(
            &preset,
            Platform::Windows,
            "skip_next_occurrence",
            "ctrl+k ctrl+d",
        );
        assert_binding(
            &preset,
            Platform::Mac,
            "skip_next_occurrence",
            "cmd+k cmd+d",
        );
        assert_binding(
            &preset,
            Platform::Windows,
            "select_all_occurrences",
            "alt+f3",
        );
        assert_binding(
            &preset,
            Platform::Mac,
            "select_all_occurrences",
            "ctrl+cmd+g",
        );
        assert_binding(
            &preset,
            Platform::Windows,
            "split_selection_into_lines",
            "ctrl+shift+l",
        );
        assert_binding(
            &preset,
            Platform::Mac,
            "split_selection_into_lines",
            "cmd+shift+l",
        );
        assert_binding(
            &preset,
            Platform::Windows,
            "add_cursor_above",
            "ctrl+alt+up",
        );
        assert_binding(&preset, Platform::Mac, "add_cursor_above", "ctrl+shift+up");
        assert_binding(
            &preset,
            Platform::Windows,
            "add_cursor_below",
            "ctrl+alt+down",
        );
        assert_binding(
            &preset,
            Platform::Mac,
            "add_cursor_below",
            "ctrl+shift+down",
        );
        assert_binding(&preset, Platform::Windows, "expand_to_line", "ctrl+l");
        assert_binding(&preset, Platform::Mac, "expand_to_line", "cmd+l");
        assert_binding(
            &preset,
            Platform::Windows,
            "expand_to_scope",
            "ctrl+shift+space",
        );
        assert_binding(&preset, Platform::Mac, "expand_to_scope", "cmd+shift+space");
        assert_binding(&preset, Platform::Windows, "expand_to_tag", "ctrl+shift+a");
        assert_binding(&preset, Platform::Mac, "expand_to_tag", "cmd+shift+a");
        assert_binding(&preset, Platform::Windows, "undo", "ctrl+z");
        assert_binding(&preset, Platform::Windows, "redo", "ctrl+shift+z");
        assert_binding(
            &preset,
            Platform::Windows,
            "jump_to_matching_bracket",
            "ctrl+m",
        );
        assert_binding(&preset, Platform::Mac, "jump_to_matching_bracket", "ctrl+m");
        assert_binding(&preset, Platform::Windows, "toggle_bookmark", "ctrl+f2");
        assert_binding(&preset, Platform::Mac, "toggle_bookmark", "cmd+f2");
        assert_binding(&preset, Platform::Windows, "next_bookmark", "f2");
        assert_binding(&preset, Platform::Mac, "next_bookmark", "f2");
        assert_binding(&preset, Platform::Windows, "prev_bookmark", "shift+f2");
        assert_binding(&preset, Platform::Mac, "prev_bookmark", "shift+f2");
        assert_binding(
            &preset,
            Platform::Windows,
            "clear_bookmarks",
            "ctrl+shift+f2",
        );
        assert_binding(&preset, Platform::Mac, "clear_bookmarks", "cmd+shift+f2");
        assert_binding(&preset, Platform::Windows, "select_all_bookmarks", "alt+f2");
        assert_binding(&preset, Platform::Mac, "select_all_bookmarks", "alt+f2");
        assert_binding(
            &preset,
            Platform::Windows,
            "set_layout_single",
            "alt+shift+1",
        );
        assert_binding(&preset, Platform::Mac, "set_layout_single", "cmd+alt+1");
        assert_binding(
            &preset,
            Platform::Windows,
            "set_layout_columns_2",
            "alt+shift+2",
        );
        assert_binding(&preset, Platform::Mac, "set_layout_columns_2", "cmd+alt+2");
        assert_binding(
            &preset,
            Platform::Windows,
            "set_layout_grid_2x2",
            "alt+shift+5",
        );
        assert_binding(&preset, Platform::Mac, "set_layout_grid_2x2", "cmd+alt+5");
        assert_binding(&preset, Platform::Mac, "focus_group_1", "ctrl+1");
        assert_binding(&preset, Platform::Mac, "focus_group_2", "ctrl+2");
        assert_binding(&preset, Platform::Mac, "move_to_group_1", "ctrl+shift+1");
        assert_binding(&preset, Platform::Mac, "move_to_group_2", "ctrl+shift+2");
        assert_binding(&preset, Platform::Windows, "build", "ctrl+b");
        assert_binding(&preset, Platform::Mac, "build", "cmd+b");
        assert_binding(&preset, Platform::Windows, "build_with", "ctrl+shift+b");
        assert_binding(&preset, Platform::Mac, "build_with", "cmd+shift+b");
        assert_binding(&preset, Platform::Mac, "cancel_build", "cmd+alt+c");
        assert_binding(
            &preset,
            Platform::Windows,
            "toggle_sidebar",
            "ctrl+k ctrl+b",
        );
        assert_binding(&preset, Platform::Mac, "toggle_sidebar", "cmd+k cmd+b");
        assert_binding(
            &preset,
            Platform::Windows,
            "quick_switch_project",
            "ctrl+alt+p",
        );
        assert_binding(&preset, Platform::Mac, "quick_switch_project", "cmd+ctrl+p");
        assert_binding(
            &preset,
            Platform::Windows,
            "toggle_distraction_free",
            "shift+f11",
        );
        assert_binding(
            &preset,
            Platform::Mac,
            "toggle_distraction_free",
            "cmd+ctrl+shift+f",
        );
        assert_binding(&preset, Platform::Windows, "insert_snippet", "ctrl+alt+s");
        assert_binding(&preset, Platform::Mac, "insert_snippet", "cmd+alt+s");
        assert_binding(&preset, Platform::Windows, "toggle_record_macro", "ctrl+q");
        assert_binding(&preset, Platform::Mac, "toggle_record_macro", "ctrl+q");
        assert_binding(&preset, Platform::Windows, "playback_macro", "ctrl+shift+q");
        assert_binding(&preset, Platform::Mac, "playback_macro", "cmd+shift+q");
    }

    #[test]
    fn vscode_preset_matches_its_documented_shortcuts() {
        let preset = km::presets::vscode();
        assert_binding(
            &preset,
            Platform::Windows,
            "command_palette",
            "ctrl+shift+p",
        );
        assert_binding(&preset, Platform::Mac, "command_palette", "cmd+shift+p");
        assert_binding(&preset, Platform::Windows, "quick_open", "ctrl+p");
        assert_binding(&preset, Platform::Windows, "find", "ctrl+f");
        assert_binding(&preset, Platform::Windows, "toggle_comment", "ctrl+/");
        assert_binding(&preset, Platform::Windows, "delete_line", "ctrl+shift+k");
        // Same on every platform: no mac override needed.
        assert_binding(&preset, Platform::Mac, "duplicate_line", "shift+alt+down");
        assert_binding(
            &preset,
            Platform::Windows,
            "select_next_occurrence",
            "ctrl+d",
        );
        assert_binding(&preset, Platform::Mac, "select_next_occurrence", "cmd+d");
    }

    #[test]
    fn vscode_preset_has_documented_two_chord_sequences() {
        let preset = km::presets::vscode();
        assert_binding(
            &preset,
            Platform::Windows,
            "keyboard_shortcuts",
            "ctrl+k ctrl+s",
        );
        assert_binding(&preset, Platform::Mac, "keyboard_shortcuts", "cmd+k cmd+s");
        assert_binding(
            &preset,
            Platform::Windows,
            "close_all_editors",
            "ctrl+k ctrl+w",
        );

        let chords = preset
            .binding_for("keyboard_shortcuts", Platform::Windows)
            .unwrap();
        assert_eq!(chords.0.len(), 2, "expected a genuine two-chord sequence");
    }

    #[test]
    fn jetbrains_preset_matches_its_documented_shortcuts() {
        let preset = km::presets::jetbrains();
        assert_binding(&preset, Platform::Windows, "find_action", "ctrl+shift+a");
        assert_binding(&preset, Platform::Mac, "find_action", "cmd+shift+a");
        // Windows and mac land on different letters here, not just a
        // Ctrl-for-Cmd swap.
        assert_binding(&preset, Platform::Windows, "quick_open", "ctrl+shift+n");
        assert_binding(&preset, Platform::Mac, "quick_open", "cmd+shift+o");
        assert_binding(&preset, Platform::Windows, "goto_line", "ctrl+g");
        assert_binding(&preset, Platform::Mac, "goto_line", "cmd+l");
        assert_binding(&preset, Platform::Windows, "duplicate_line", "ctrl+d");
        assert_binding(&preset, Platform::Windows, "delete_line", "ctrl+y");
        assert_binding(&preset, Platform::Mac, "delete_line", "cmd+backspace");
        assert_binding(
            &preset,
            Platform::Windows,
            "select_next_occurrence",
            "alt+j",
        );
        // Ctrl, not Cmd, even on mac — would otherwise collide with "Find
        // Next".
        assert_binding(&preset, Platform::Mac, "select_next_occurrence", "ctrl+g");
        assert_binding(&preset, Platform::Windows, "save_all", "ctrl+s");
        assert_binding(&preset, Platform::Mac, "save_all", "cmd+s");
    }

    #[test]
    fn linux_falls_back_to_the_windows_binding_when_no_linux_override_exists() {
        for preset in [
            km::presets::sublime(),
            km::presets::vscode(),
            km::presets::jetbrains(),
        ] {
            for binding in &preset.bindings {
                assert_eq!(
                    binding.resolve(Platform::Linux),
                    binding.resolve(Platform::Windows),
                    "{}: {:?} has a linux binding that diverges from windows without an explicit override",
                    preset.name,
                    binding.command
                );
            }
        }
    }

    #[test]
    fn none_of_the_three_shipped_presets_have_an_internal_conflict() {
        for preset in [
            km::presets::sublime(),
            km::presets::vscode(),
            km::presets::jetbrains(),
        ] {
            for platform in [Platform::Windows, Platform::Mac, Platform::Linux] {
                let found = km::conflicts(&preset, platform);
                assert!(
                    found.is_empty(),
                    "{} has conflicts on {platform:?}: {}",
                    preset.name,
                    found
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join("; ")
                );
            }
        }
    }

    #[test]
    fn a_real_conflict_is_detected_with_a_readable_diagnostic() {
        let conflicting = km::Keymap {
            name: "conflicting".to_string(),
            bindings: vec![
                km::KeyBinding {
                    command: "duplicate_line".to_string(),
                    key: "ctrl+d".parse().unwrap(),
                    mac: None,
                    win: None,
                    linux: None,
                    when: None,
                },
                km::KeyBinding {
                    command: "select_next_occurrence".to_string(),
                    key: "ctrl+d".parse().unwrap(),
                    mac: None,
                    win: None,
                    linux: None,
                    when: None,
                },
            ],
        };

        let found = km::conflicts(&conflicting, Platform::Windows);
        assert_eq!(found.len(), 1);
        let message = found[0].to_string();
        assert!(message.contains("ctrl+d"), "message was: {message}");
        assert!(message.contains("duplicate_line"), "message was: {message}");
        assert!(
            message.contains("select_next_occurrence"),
            "message was: {message}"
        );
    }
}
