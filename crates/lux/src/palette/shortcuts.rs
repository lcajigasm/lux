//! Palette command ids and keymap command ids differ (`file.save` versus
//! `save`). This map is only the bindings that exist in the preset. A row
//! with no entry draws no right-hand label rather than inventing one.

use std::collections::HashMap;

use lux_config::keymap::Platform;

const MAP: &[(&str, &str)] = &[
    ("file.save", "save"),
    ("edit.undo", "undo"),
    ("edit.redo", "redo"),
    ("edit.duplicate_line", "duplicate_line"),
    ("edit.delete_line", "delete_line"),
    ("edit.swap_line_up", "swap_line_up"),
    ("edit.swap_line_down", "swap_line_down"),
    ("edit.join_lines", "join_lines"),
    ("edit.toggle_comment", "toggle_comment"),
    ("find", "find"),
    ("find.find", "find"),
    ("find.replace", "replace"),
    ("find.next", "find_next"),
    ("find.prev", "find_prev"),
    ("find.all", "find_all"),
    ("find.find_in_files", "find_in_files"),
    ("find.next_result", "next_result"),
    ("find.prev_result", "prev_result"),
    ("selection.next_occurrence", "select_next_occurrence"),
    ("selection.skip_occurrence", "skip_next_occurrence"),
    ("selection.all_occurrences", "select_all_occurrences"),
    ("selection.split_into_lines", "split_selection_into_lines"),
    ("selection.add_cursor_above", "add_cursor_above"),
    ("selection.add_cursor_below", "add_cursor_below"),
    ("selection.expand_to_line", "expand_to_line"),
    ("selection.expand_to_scope", "expand_to_scope"),
    ("selection.expand_to_tag", "expand_to_tag"),
    (
        "selection.jump_to_matching_bracket",
        "jump_to_matching_bracket",
    ),
    ("bookmark.toggle", "toggle_bookmark"),
    ("bookmark.next", "next_bookmark"),
    ("bookmark.prev", "prev_bookmark"),
    ("bookmark.clear", "clear_bookmarks"),
    ("bookmark.select_all", "select_all_bookmarks"),
    ("code.fold", "fold"),
    ("code.unfold", "unfold"),
    ("code.fold_all", "fold_all"),
    ("code.unfold_all", "unfold_all"),
    ("code.fold_level_2", "fold_level_2"),
    ("code.fold_level_3", "fold_level_3"),
    ("code.fold_level_4", "fold_level_4"),
    ("code.fold_level_5", "fold_level_5"),
    ("goto.anything", "quick_open"),
    ("goto.line", "goto_line"),
    ("goto.symbol", "goto_symbol"),
    ("goto.symbol_in_project", "goto_symbol_in_project"),
    ("goto.word", "goto_word"),
    ("view.toggle_side_bar", "toggle_sidebar"),
    ("view.layout_single", "set_layout_single"),
    ("view.layout_columns_2", "set_layout_columns_2"),
    ("view.layout_columns_3", "set_layout_columns_3"),
    ("view.layout_columns_4", "set_layout_columns_4"),
    ("view.layout_rows_2", "set_layout_rows_2"),
    ("view.layout_rows_3", "set_layout_rows_3"),
    ("view.layout_grid_2x2", "set_layout_grid_2x2"),
    ("view.focus_group_1", "focus_group_1"),
    ("view.focus_group_2", "focus_group_2"),
    ("view.focus_group_3", "focus_group_3"),
    ("view.focus_group_4", "focus_group_4"),
    ("view.move_to_group_1", "move_to_group_1"),
    ("view.move_to_group_2", "move_to_group_2"),
    ("view.move_to_group_3", "move_to_group_3"),
    ("view.move_to_group_4", "move_to_group_4"),
    ("view.toggle_distraction_free", "toggle_distraction_free"),
    ("build.build", "build"),
    ("build.build_with", "build_with"),
    ("build.cancel", "cancel_build"),
    ("project.quick_switch", "quick_switch_project"),
    ("snippet.insert", "insert_snippet"),
    ("macro.toggle_record", "toggle_record_macro"),
    ("macro.playback", "playback_macro"),
    ("macro.save_as", "save_macro"),
];

pub(crate) fn shortcut_labels(keymap: &lux_config::Keymap) -> HashMap<String, String> {
    labels_for(keymap, host_platform())
}

pub(crate) fn host_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

pub(crate) fn labels_for(
    keymap: &lux_config::Keymap,
    platform: Platform,
) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    for (palette_id, command) in MAP {
        if let Some(binding) = keymap.binding_for(command, platform) {
            labels.insert((*palette_id).to_string(), pretty(&binding.to_string()));
        }
    }
    labels
}

fn pretty(sequence: &str) -> String {
    sequence
        .split_whitespace()
        .map(pretty_chord)
        .collect::<Vec<_>>()
        .join(" ")
}

fn pretty_chord(chord: &str) -> String {
    let mut cmd = false;
    let mut shift = false;
    let mut alt = false;
    let mut ctrl = false;
    let mut key = String::new();
    for part in chord.split('+') {
        match part {
            "cmd" | "super" | "meta" | "win" => cmd = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            "ctrl" | "control" => ctrl = true,
            other if other.chars().count() <= 3 => key.push_str(&other.to_uppercase()),
            other => key.push_str(other),
        }
    }
    let mut out = String::new();
    if cmd {
        out.push('⌘');
    }
    if shift {
        out.push('⇧');
    }
    if alt {
        out.push('⌥');
    }
    if ctrl {
        out.push('⌃');
    }
    out.push_str(&key);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_save_and_palette_chords_use_symbols() {
        let keymap = lux_config::keymap::presets::sublime();
        let labels = labels_for(&keymap, Platform::Mac);
        assert_eq!(labels.get("file.save").map(String::as_str), Some("⌘S"));
        assert_eq!(labels.get("goto.anything").map(String::as_str), Some("⌘P"));
        assert_eq!(
            labels.get("edit.duplicate_line").map(String::as_str),
            Some("⌘⇧D")
        );
        assert!(!labels.contains_key("help.about"));
    }

    #[test]
    fn linux_save_uses_control() {
        let keymap = lux_config::keymap::presets::sublime();
        let labels = labels_for(&keymap, Platform::Linux);
        assert_eq!(labels.get("file.save").map(String::as_str), Some("⌃S"));
    }
}
