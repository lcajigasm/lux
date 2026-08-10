//! Conflict detection: two bindings in the same [`Keymap`] resolving to the
//! same chord sequence on the same platform, in contexts that can't be told
//! apart. `when` is treated as opaque (see `binding.rs`) — a real
//! overlapping-context analysis would need to evaluate and compare `when`
//! expressions, a much bigger feature this deliverable's text doesn't ask
//! for ("resolución de conflictos con diagnóstico visible", not "a `when`
//! expression engine"). So two bindings only count as conflicting when
//! their `when` clauses are identical (including both being unset) —
//! genuinely indistinguishable at runtime — not merely unproven to differ.

use std::collections::HashMap;
use std::fmt;

use super::binding::{Keymap, Platform};
use super::chord::ChordSequence;

/// Two or more commands bound to the same chord sequence in the same
/// context on one platform — a real, visible diagnostic a caller can
/// surface, not just an internal `bool`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub chords: ChordSequence,
    pub commands: Vec<String>,
    pub when: Option<String>,
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} is bound to multiple commands: {}",
            self.chords,
            self.commands.join(", ")
        )?;
        if let Some(when) = &self.when {
            write!(f, " (when {when})")?;
        }
        Ok(())
    }
}

/// Every conflict in `keymap` as resolved on `platform`, sorted by chord
/// sequence (then by `when`) so the result is deterministic regardless of
/// hash-map iteration order.
pub fn conflicts(keymap: &Keymap, platform: Platform) -> Vec<Conflict> {
    let mut groups: HashMap<(ChordSequence, Option<String>), Vec<String>> = HashMap::new();
    for binding in &keymap.bindings {
        let chords = binding.resolve(platform).clone();
        groups
            .entry((chords, binding.when.clone()))
            .or_default()
            .push(binding.command.clone());
    }

    let mut found: Vec<Conflict> = groups
        .into_iter()
        .filter(|(_, commands)| commands.len() > 1)
        .map(|((chords, when), commands)| Conflict {
            chords,
            commands,
            when,
        })
        .collect();
    found.sort_by(|a, b| {
        (a.chords.to_string(), a.when.clone()).cmp(&(b.chords.to_string(), b.when.clone()))
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::binding::KeyBinding;

    fn binding(command: &str, key: &str, when: Option<&str>) -> KeyBinding {
        KeyBinding {
            command: command.to_string(),
            key: key.parse().unwrap(),
            mac: None,
            win: None,
            linux: None,
            when: when.map(str::to_string),
        }
    }

    #[test]
    fn two_bindings_on_the_same_chord_with_no_when_clause_conflict() {
        let keymap = Keymap {
            name: "test".to_string(),
            bindings: vec![
                binding("duplicate_line", "ctrl+d", None),
                binding("select_next_occurrence", "ctrl+d", None),
            ],
        };
        let found = conflicts(&keymap, Platform::Windows);
        assert_eq!(found.len(), 1);
        assert_eq!(
            found[0].commands,
            vec!["duplicate_line", "select_next_occurrence"]
        );
        assert!(found[0].to_string().contains("ctrl+d"));
        assert!(found[0].to_string().contains("duplicate_line"));
        assert!(found[0].to_string().contains("select_next_occurrence"));
    }

    #[test]
    fn bindings_scoped_to_different_when_clauses_do_not_conflict() {
        let keymap = Keymap {
            name: "test".to_string(),
            bindings: vec![
                binding("terminal_paste", "ctrl+v", Some("terminalFocus")),
                binding("editor_paste", "ctrl+v", Some("editorFocus")),
            ],
        };
        assert!(conflicts(&keymap, Platform::Windows).is_empty());
    }

    #[test]
    fn bindings_on_different_chords_do_not_conflict() {
        let keymap = Keymap {
            name: "test".to_string(),
            bindings: vec![binding("a", "ctrl+a", None), binding("b", "ctrl+b", None)],
        };
        assert!(conflicts(&keymap, Platform::Windows).is_empty());
    }

    #[test]
    fn platform_overrides_are_conflict_checked_independently() {
        let keymap = Keymap {
            name: "test".to_string(),
            bindings: vec![
                KeyBinding {
                    command: "a".to_string(),
                    key: "ctrl+k".parse().unwrap(),
                    mac: Some("cmd+k".parse().unwrap()),
                    win: None,
                    linux: None,
                    when: None,
                },
                KeyBinding {
                    command: "b".to_string(),
                    key: "ctrl+j".parse().unwrap(),
                    mac: Some("cmd+k".parse().unwrap()),
                    win: None,
                    linux: None,
                    when: None,
                },
            ],
        };
        // On mac both resolve to cmd+k: a real conflict.
        assert_eq!(conflicts(&keymap, Platform::Mac).len(), 1);
        // On windows they resolve to different chords: no conflict there,
        // even though the same keymap has a mac-only conflict.
        assert!(conflicts(&keymap, Platform::Windows).is_empty());
    }
}
