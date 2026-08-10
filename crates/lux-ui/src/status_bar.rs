//! Status bar (`LUX_PROMPT_MAESTRO.md` Phase 3 deliverable 9, status half):
//! cursor position, language, encoding, line ending, and indentation — all
//! clickable. Encoding/EOL/indent detection is `lux-core::buffer`, already
//! implemented in Phase 1 (`docs/STATUS.md`'s "EOL / indent / encoding
//! detection" row); this module only formats those already-detected values
//! for display and, for "clickable", ties each item to a real command.
//!
//! **"All clickable"** is enforced the same way `command_palette` enforces
//! "every menu command is in the palette": structurally, not by
//! convention. [`StatusBarItem`] has no way to exist without a
//! `command_id` — there's no optional/bare-label variant — and
//! [`register_commands`] registers the exact same ids into a
//! [`crate::command_palette::CommandRegistry`], so a click handler always
//! resolves to a real, registered command instead of a dangling string.

use crate::command_palette::{Command, CommandRegistry, DuplicateCommandId};
use lux_core::buffer::{Buffer, Encoding, Eol, IndentStyle};
use lux_core::position::Position;

pub const GOTO_LINE_COMMAND: &str = "status_bar.goto_line";
pub const CHANGE_LANGUAGE_COMMAND: &str = "status_bar.change_language";
pub const CHANGE_ENCODING_COMMAND: &str = "status_bar.change_encoding";
pub const CHANGE_EOL_COMMAND: &str = "status_bar.change_eol";
pub const CHANGE_INDENTATION_COMMAND: &str = "status_bar.change_indentation";

const COMMANDS: [(&str, &str); 5] = [
    (GOTO_LINE_COMMAND, "Go to Line"),
    (CHANGE_LANGUAGE_COMMAND, "Change Language Mode"),
    (CHANGE_ENCODING_COMMAND, "Change File Encoding"),
    (CHANGE_EOL_COMMAND, "Change Line Ending"),
    (CHANGE_INDENTATION_COMMAND, "Change Indentation"),
];

/// Registers every status bar item's command into `registry`, under the
/// "Status Bar" category — the single source of truth `StatusBar::build`'s
/// `command_id`s point into (see module docs).
pub fn register_commands(registry: &mut CommandRegistry) -> Result<(), DuplicateCommandId> {
    for (id, title) in COMMANDS {
        registry.register(Command::new(id, title, "Status Bar"))?;
    }
    Ok(())
}

/// One status bar segment: its display label and the id of the command a
/// click on it invokes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBarItem {
    pub label: String,
    pub command_id: &'static str,
}

/// A full status bar snapshot for one buffer/cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBar {
    pub cursor: StatusBarItem,
    pub language: StatusBarItem,
    pub encoding: StatusBarItem,
    pub eol: StatusBarItem,
    pub indent: StatusBarItem,
}

/// Builds a status bar snapshot from `buffer`'s already-detected
/// encoding/EOL/indent, `cursor`'s position, and a caller-supplied
/// `language` label (language detection isn't wired into `lux-ui` yet —
/// that's `lux-syntax`, not this deliverable's job).
pub fn build(buffer: &Buffer, cursor: Position, language: &str) -> StatusBar {
    StatusBar {
        cursor: StatusBarItem {
            label: format!("Ln {}, Col {}", cursor.line + 1, cursor.column + 1),
            command_id: GOTO_LINE_COMMAND,
        },
        language: StatusBarItem {
            label: language.to_string(),
            command_id: CHANGE_LANGUAGE_COMMAND,
        },
        encoding: StatusBarItem {
            label: encoding_label(buffer.encoding()).to_string(),
            command_id: CHANGE_ENCODING_COMMAND,
        },
        eol: StatusBarItem {
            label: eol_label(buffer.eol()).to_string(),
            command_id: CHANGE_EOL_COMMAND,
        },
        indent: StatusBarItem {
            label: indent_label(buffer),
            command_id: CHANGE_INDENTATION_COMMAND,
        },
    }
}

fn eol_label(eol: Eol) -> &'static str {
    match eol {
        Eol::Lf => "LF",
        Eol::Crlf => "CRLF",
        Eol::Cr => "CR",
    }
}

fn encoding_label(encoding: Encoding) -> &'static str {
    match encoding {
        Encoding::Utf8 => "UTF-8",
        Encoding::Utf8Bom => "UTF-8 BOM",
        Encoding::Utf16Le => "UTF-16 LE",
        Encoding::Utf16Be => "UTF-16 BE",
        Encoding::Latin1 => "Latin-1",
    }
}

fn indent_label(buffer: &Buffer) -> String {
    let indent = buffer.indent();
    match indent.style {
        IndentStyle::Tabs => "Tabs".to_string(),
        IndentStyle::Spaces => format!("Spaces: {}", indent.width),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eol_label_covers_every_variant() {
        assert_eq!(eol_label(Eol::Lf), "LF");
        assert_eq!(eol_label(Eol::Crlf), "CRLF");
        assert_eq!(eol_label(Eol::Cr), "CR");
    }

    #[test]
    fn encoding_label_covers_every_variant() {
        assert_eq!(encoding_label(Encoding::Utf8), "UTF-8");
        assert_eq!(encoding_label(Encoding::Utf8Bom), "UTF-8 BOM");
        assert_eq!(encoding_label(Encoding::Utf16Le), "UTF-16 LE");
        assert_eq!(encoding_label(Encoding::Utf16Be), "UTF-16 BE");
        assert_eq!(encoding_label(Encoding::Latin1), "Latin-1");
    }

    #[test]
    fn indent_label_formats_tabs_and_spaces() {
        let tabs_buffer = Buffer::from_bytes(b"fn f() {\n\tx();\n}\n").unwrap();
        assert_eq!(indent_label(&tabs_buffer), "Tabs");

        let spaces_buffer = Buffer::from_bytes(b"fn f() {\n  x();\n}\n").unwrap();
        assert_eq!(indent_label(&spaces_buffer), "Spaces: 2");
    }

    #[test]
    fn build_formats_cursor_position_as_one_based() {
        let buffer = Buffer::from_bytes(b"hello\nworld\n").unwrap();
        let cursor = Position::from_char(&buffer, 6); // start of "world", line 1 col 0 (0-based)
        let status = build(&buffer, cursor, "Rust");
        assert_eq!(status.cursor.label, "Ln 2, Col 1");
        assert_eq!(status.cursor.command_id, GOTO_LINE_COMMAND);
    }

    #[test]
    fn build_uses_the_buffers_detected_eol_and_encoding() {
        let buffer = Buffer::from_bytes(b"a\r\nb\r\n").unwrap();
        let status = build(&buffer, Position::document_start(), "Plain Text");
        assert_eq!(status.eol.label, "CRLF");
        assert_eq!(status.encoding.label, "UTF-8");
    }

    #[test]
    fn build_uses_the_caller_supplied_language_label() {
        let buffer = Buffer::from_bytes(b"fn main() {}\n").unwrap();
        let status = build(&buffer, Position::document_start(), "Rust");
        assert_eq!(status.language.label, "Rust");
        assert_eq!(status.language.command_id, CHANGE_LANGUAGE_COMMAND);
    }

    #[test]
    fn register_commands_registers_every_item() {
        let mut registry = CommandRegistry::new();
        register_commands(&mut registry).unwrap();
        let ids: Vec<&str> = registry.commands().map(|c| c.id.as_str()).collect();
        for (id, _) in COMMANDS {
            assert!(ids.contains(&id), "missing command {id}");
        }
    }

    #[test]
    fn register_commands_twice_on_the_same_registry_fails_on_duplicate_ids() {
        let mut registry = CommandRegistry::new();
        register_commands(&mut registry).unwrap();
        assert!(register_commands(&mut registry).is_err());
    }

    #[test]
    fn every_built_status_bar_item_resolves_to_a_registered_command() {
        let mut registry = CommandRegistry::new();
        register_commands(&mut registry).unwrap();

        let buffer = Buffer::from_bytes(b"fn main() {}\n").unwrap();
        let status = build(&buffer, Position::document_start(), "Rust");
        let registered_ids: Vec<&str> = registry.commands().map(|c| c.id.as_str()).collect();

        for item in [
            &status.cursor,
            &status.language,
            &status.encoding,
            &status.eol,
            &status.indent,
        ] {
            assert!(
                registered_ids.contains(&item.command_id),
                "status bar item {item:?} has no matching registered command"
            );
        }
    }
}
