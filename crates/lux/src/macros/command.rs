//! Sublime Text Macro Command representation and JSON serialization (ST19).

use serde::{Deserialize, Serialize};

/// Movement granularity for cursor navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveBy {
    Characters,
    Words,
    Lines,
    Stops,
}

/// Movement target position for line or document boundaries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoveTo {
    Bol,
    Eol,
    Bof,
    Eof,
}

/// Strongly typed editor command executed during macro recording and playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroCommand {
    /// Inserts text at each cursor.
    InsertText { text: String },
    /// Inserts a newline at each cursor.
    InsertNewline,
    /// Backspace at each cursor.
    DeleteBackward,
    /// Forward delete at each cursor.
    DeleteForward,
    /// Word delete backward or forward.
    DeleteWord { forward: bool },
    /// Cursor movement by character, word, or line.
    Move {
        by: MoveBy,
        forward: bool,
        extend: bool,
    },
    /// Move cursor to line or document boundary.
    MoveTo { to: MoveTo, extend: bool },
    /// Any editor action command (e.g. duplicate_line, toggle_comment, etc.).
    Action {
        name: String,
        args: Option<serde_json::Value>,
    },
}

/// Raw JSON representation of a Sublime Text macro step: `{"command": "...", "args": {...}}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawMacroStep {
    pub command: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
}

impl MacroCommand {
    /// Converts a strongly-typed `MacroCommand` into a standard Sublime `.sublime-macro` JSON step.
    pub fn to_raw(&self) -> RawMacroStep {
        match self {
            Self::InsertText { text } => RawMacroStep {
                command: "insert".to_string(),
                args: Some(serde_json::json!({ "characters": text })),
            },
            Self::InsertNewline => RawMacroStep {
                command: "insert".to_string(),
                args: Some(serde_json::json!({ "characters": "\n" })),
            },
            Self::DeleteBackward => RawMacroStep {
                command: "left_delete".to_string(),
                args: None,
            },
            Self::DeleteForward => RawMacroStep {
                command: "right_delete".to_string(),
                args: None,
            },
            Self::DeleteWord { forward } => RawMacroStep {
                command: "delete_word".to_string(),
                args: Some(serde_json::json!({ "forward": forward })),
            },
            Self::Move {
                by,
                forward,
                extend,
            } => {
                let by_str = match by {
                    MoveBy::Characters => "characters",
                    MoveBy::Words => "words",
                    MoveBy::Lines => "lines",
                    MoveBy::Stops => "stops",
                };
                RawMacroStep {
                    command: "move".to_string(),
                    args: Some(serde_json::json!({
                        "by": by_str,
                        "forward": forward,
                        "extend": extend,
                    })),
                }
            }
            Self::MoveTo { to, extend } => {
                let to_str = match to {
                    MoveTo::Bol => "bol",
                    MoveTo::Eol => "eol",
                    MoveTo::Bof => "bof",
                    MoveTo::Eof => "eof",
                };
                RawMacroStep {
                    command: "move_to".to_string(),
                    args: Some(serde_json::json!({
                        "to": to_str,
                        "extend": extend,
                    })),
                }
            }
            Self::Action { name, args } => RawMacroStep {
                command: name.clone(),
                args: args.clone(),
            },
        }
    }

    /// Parses a Sublime Text raw JSON step into a strongly-typed `MacroCommand`.
    #[allow(dead_code)]
    pub fn from_raw(raw: RawMacroStep) -> Self {
        match raw.command.as_str() {
            "insert" => {
                let chars = raw
                    .args
                    .as_ref()
                    .and_then(|a| a.get("characters"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                if chars == "\n" {
                    Self::InsertNewline
                } else {
                    Self::InsertText {
                        text: chars.to_string(),
                    }
                }
            }
            "left_delete" => Self::DeleteBackward,
            "right_delete" => Self::DeleteForward,
            "delete_word" => {
                let forward = raw
                    .args
                    .as_ref()
                    .and_then(|a| a.get("forward"))
                    .and_then(|f| f.as_bool())
                    .unwrap_or(false);
                Self::DeleteWord { forward }
            }
            "move" => {
                let by_str = raw
                    .args
                    .as_ref()
                    .and_then(|a| a.get("by"))
                    .and_then(|b| b.as_str())
                    .unwrap_or("characters");
                let by = match by_str {
                    "words" => MoveBy::Words,
                    "lines" => MoveBy::Lines,
                    "stops" => MoveBy::Stops,
                    _ => MoveBy::Characters,
                };
                let forward = raw
                    .args
                    .as_ref()
                    .and_then(|a| a.get("forward"))
                    .and_then(|f| f.as_bool())
                    .unwrap_or(true);
                let extend = raw
                    .args
                    .as_ref()
                    .and_then(|a| a.get("extend"))
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                Self::Move {
                    by,
                    forward,
                    extend,
                }
            }
            "move_to" => {
                let to_str = raw
                    .args
                    .as_ref()
                    .and_then(|a| a.get("to"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("bol");
                let to = match to_str {
                    "eol" => MoveTo::Eol,
                    "bof" => MoveTo::Bof,
                    "eof" => MoveTo::Eof,
                    _ => MoveTo::Bol,
                };
                let extend = raw
                    .args
                    .as_ref()
                    .and_then(|a| a.get("extend"))
                    .and_then(|e| e.as_bool())
                    .unwrap_or(false);
                Self::MoveTo { to, extend }
            }
            other => Self::Action {
                name: other.to_string(),
                args: raw.args,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macro_command_raw_roundtrip() {
        let commands = vec![
            MacroCommand::InsertText {
                text: "let x = 42;".to_string(),
            },
            MacroCommand::InsertNewline,
            MacroCommand::DeleteBackward,
            MacroCommand::DeleteForward,
            MacroCommand::DeleteWord { forward: true },
            MacroCommand::Move {
                by: MoveBy::Lines,
                forward: true,
                extend: false,
            },
            MacroCommand::MoveTo {
                to: MoveTo::Eol,
                extend: true,
            },
            MacroCommand::Action {
                name: "duplicate_line".to_string(),
                args: None,
            },
        ];

        for cmd in commands {
            let raw = cmd.to_raw();
            let roundtrip = MacroCommand::from_raw(raw);
            assert_eq!(cmd, roundtrip);
        }
    }
}
