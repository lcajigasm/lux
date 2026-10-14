//! Macro definition and file persistence (ST19).

use std::fs;
use std::path::Path;

use super::command::{MacroCommand, RawMacroStep};

/// A recorded or loaded Sublime Text keyboard macro.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Macro {
    pub name: Option<String>,
    pub commands: Vec<MacroCommand>,
}

impl Macro {
    /// Creates a new empty macro.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a macro from a sequence of commands.
    pub fn from_commands(commands: Vec<MacroCommand>) -> Self {
        Self {
            name: None,
            commands,
        }
    }

    /// Parses a Sublime Text `.sublime-macro` JSON string.
    pub fn from_sublime_json(json_str: &str) -> Result<Self, String> {
        let raw_steps: Vec<RawMacroStep> = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse macro JSON: {e}"))?;
        let commands = raw_steps.into_iter().map(MacroCommand::from_raw).collect();
        Ok(Self {
            name: None,
            commands,
        })
    }

    /// Serializes the macro to Sublime Text `.sublime-macro` JSON format.
    pub fn to_sublime_json(&self) -> Result<String, String> {
        let raw_steps: Vec<RawMacroStep> = self.commands.iter().map(|c| c.to_raw()).collect();
        serde_json::to_string_pretty(&raw_steps)
            .map_err(|e| format!("Failed to serialize macro to JSON: {e}"))
    }

    /// Saves the macro to disk at the given path.
    pub fn save_to_file(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = self
            .to_sublime_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, json)
    }

    /// Loads a macro from disk from a `.sublime-macro` file.
    pub fn load_from_file(path: &Path) -> std::io::Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut m = Self::from_sublime_json(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
        m.name = name;
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sublime_macro_json_roundtrip() {
        let json = r#"[
  {
    "args": {
      "characters": "// "
    },
    "command": "insert"
  },
  {
    "args": {
      "by": "lines",
      "extend": false,
      "forward": true
    },
    "command": "move"
  },
  {
    "args": {
      "extend": false,
      "to": "bol"
    },
    "command": "move_to"
  }
]"#;

        let m = Macro::from_sublime_json(json).expect("valid macro json");
        assert_eq!(m.commands.len(), 3);

        let roundtrip_json = m.to_sublime_json().expect("valid serialization");
        let m2 = Macro::from_sublime_json(&roundtrip_json).expect("valid parsed roundtrip");
        assert_eq!(m.commands, m2.commands);
    }
}
