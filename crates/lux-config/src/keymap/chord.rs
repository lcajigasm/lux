//! `Chord` and `ChordSequence`: a key combination and a sequence of them,
//! parsed from the same string convention VS Code's real `keybindings.json`
//! uses — `"+"`-joined modifiers ending in a key, chords in a sequence
//! separated by whitespace (e.g. `"ctrl+k ctrl+s"`). Reusing that exact,
//! already-real format (rather than inventing a nested JSON struct for it)
//! is what makes a chained shortcut like that one representable as one
//! string field instead of a special case.

use std::fmt;
use std::str::FromStr;

/// One simultaneous key combination, e.g. `ctrl+shift+p`. Fields are the
/// four modifiers this crate's presets actually use; equality/hashing are
/// derived, so two `Chord`s parsed from differently-ordered or
/// differently-cased strings (`"shift+ctrl+P"` vs `"Ctrl+Shift+p"`) compare
/// equal because parsing normalizes both before storing them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// macOS's Command key. Windows/Linux presets simply never set it.
    pub cmd: bool,
    /// The non-modifier key, lowercased (`"p"`, `"/"`, `"enter"`,
    /// `"down"`).
    pub key: String,
}

/// A parse failure for a chord or chord-sequence string, carrying enough of
/// the input to point at the mistake — this crate's data lives in checked-in
/// `.jsonc` preset files, so a bad chord string here is a real authoring
/// error worth a specific message, not a generic "invalid input".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChordParseError {
    pub input: String,
    pub reason: String,
}

impl fmt::Display for ChordParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid chord {:?}: {}", self.input, self.reason)
    }
}

impl std::error::Error for ChordParseError {}

impl FromStr for Chord {
    type Err = ChordParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = |reason: &str| ChordParseError {
            input: s.to_string(),
            reason: reason.to_string(),
        };
        let mut parts: Vec<&str> = s.split('+').map(str::trim).collect();
        if parts.iter().any(|p| p.is_empty()) {
            return Err(err("empty modifier or key segment"));
        }
        let key = parts
            .pop()
            .ok_or_else(|| err("no key given"))?
            .to_lowercase();
        if key.is_empty() {
            return Err(err("no key given"));
        }
        let mut chord = Chord {
            ctrl: false,
            alt: false,
            shift: false,
            cmd: false,
            key,
        };
        for modifier in parts {
            match modifier.to_lowercase().as_str() {
                "ctrl" | "control" => chord.ctrl = true,
                "alt" | "option" => chord.alt = true,
                "shift" => chord.shift = true,
                "cmd" | "super" | "meta" | "win" => chord.cmd = true,
                other => return Err(err(&format!("unknown modifier {other:?}"))),
            }
        }
        Ok(chord)
    }
}

impl fmt::Display for Chord {
    /// Canonical form, fixed modifier order — used to build readable
    /// conflict diagnostics, not just for round-tripping.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            write!(f, "ctrl+")?;
        }
        if self.alt {
            write!(f, "alt+")?;
        }
        if self.shift {
            write!(f, "shift+")?;
        }
        if self.cmd {
            write!(f, "cmd+")?;
        }
        write!(f, "{}", self.key)
    }
}

/// One or more [`Chord`]s pressed in order — length 1 for an ordinary
/// shortcut, length 2+ for a chained one like `ctrl+k ctrl+s`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChordSequence(pub Vec<Chord>);

impl FromStr for ChordSequence {
    type Err = ChordParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chords: Result<Vec<Chord>, ChordParseError> =
            s.split_whitespace().map(Chord::from_str).collect();
        let chords = chords?;
        if chords.is_empty() {
            return Err(ChordParseError {
                input: s.to_string(),
                reason: "empty chord sequence".to_string(),
            });
        }
        Ok(ChordSequence(chords))
    }
}

impl fmt::Display for ChordSequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rendered: Vec<String> = self.0.iter().map(Chord::to_string).collect();
        write!(f, "{}", rendered.join(" "))
    }
}

impl<'de> serde::Deserialize<'de> for ChordSequence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        raw.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_single_modifier_and_key() {
        let chord: Chord = "ctrl+p".parse().unwrap();
        assert!(chord.ctrl && !chord.alt && !chord.shift && !chord.cmd);
        assert_eq!(chord.key, "p");
    }

    #[test]
    fn parses_multiple_modifiers_in_any_order_and_case() {
        let a: Chord = "Ctrl+Shift+P".parse().unwrap();
        let b: Chord = "shift+ctrl+p".parse().unwrap();
        assert_eq!(a, b);
        assert!(a.ctrl && a.shift && !a.alt && !a.cmd);
    }

    #[test]
    fn parses_a_bare_key_with_no_modifiers() {
        let chord: Chord = "enter".parse().unwrap();
        assert!(!chord.ctrl && !chord.alt && !chord.shift && !chord.cmd);
        assert_eq!(chord.key, "enter");
    }

    #[test]
    fn rejects_an_unknown_modifier() {
        let err = "hyper+p".parse::<Chord>().unwrap_err();
        assert!(err.reason.contains("hyper"));
    }

    #[test]
    fn rejects_a_trailing_plus() {
        assert!("ctrl+".parse::<Chord>().is_err());
    }

    #[test]
    fn parses_a_two_chord_sequence() {
        let seq: ChordSequence = "ctrl+k ctrl+s".parse().unwrap();
        assert_eq!(seq.0.len(), 2);
        assert_eq!(seq.0[0].key, "k");
        assert!(seq.0[0].ctrl);
        assert_eq!(seq.0[1].key, "s");
        assert!(seq.0[1].ctrl);
    }

    #[test]
    fn displays_in_canonical_modifier_order() {
        let chord: Chord = "shift+cmd+alt+ctrl+p".parse().unwrap();
        assert_eq!(chord.to_string(), "ctrl+alt+shift+cmd+p");
    }

    #[test]
    fn rejects_an_empty_sequence() {
        assert!("   ".parse::<ChordSequence>().is_err());
    }
}
