//! Tracks IME composition state across a stream of [`Ime`] events.
//!
//! `winit` (and every platform IME under it) delivers composition as a raw
//! `Enabled`/`Preedit`/`Commit`/`Disabled` sequence — see
//! [`crate::desktop`]'s conversion of `winit::event::Ime`. That sequence
//! alone doesn't tell a caller the two things an editor actually needs to
//! react to: "here is the text currently being composed and where its
//! cursor sits, show it inline instead of in the buffer" and "here is the
//! final text, insert it now." [`ImeComposition`] is that state machine,
//! kept in `lux-platform` because it's genuine platform-event logic, not
//! anything to do with rendering or text layout (which stay `lux-ui`'s job,
//! per `docs/ARCHITECTURE.md`).

use crate::event::Ime;

/// The composition currently in progress, if any. One instance per text
/// input surface (a real editor would keep one per focused view).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImeComposition {
    enabled: bool,
    preedit: String,
    cursor_range: Option<(usize, usize)>,
}

/// What applying one [`Ime`] event means for a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeUpdate {
    /// Nothing to show right now: composition was cleared, cancelled, or an
    /// `Enabled`/empty `Preedit` arrived with no text yet.
    Idle,
    /// Preedit text changed; show it inline at the cursor, not in the
    /// buffer — it isn't final until a `Committed` update arrives.
    Composing {
        text: String,
        cursor_range: Option<(usize, usize)>,
    },
    /// Composition finished; `text` is final and belongs in the buffer.
    Committed(String),
}

impl ImeComposition {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether IME composition is currently available for this surface
    /// (an `Enabled` event was seen more recently than any `Disabled`).
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Whether there is non-empty preedit text awaiting confirmation.
    pub fn is_composing(&self) -> bool {
        !self.preedit.is_empty()
    }

    pub fn preedit_text(&self) -> &str {
        &self.preedit
    }

    pub fn cursor_range(&self) -> Option<(usize, usize)> {
        self.cursor_range
    }

    /// Applies one event, updating internal state, and returns what a
    /// caller should do about it.
    pub fn apply(&mut self, event: Ime) -> ImeUpdate {
        match event {
            Ime::Enabled => {
                self.enabled = true;
                self.clear_preedit();
                ImeUpdate::Idle
            }
            Ime::Preedit { text, cursor_range } => {
                // Some platforms send `Preedit` without a preceding
                // `Enabled` for the very first composed character; treating
                // that as implicitly enabling composition matches what the
                // platform is actually doing, rather than dropping it.
                self.enabled = true;
                // `cursor_range` is a byte range, not a char count — `winit`
                // documents `Ime::Preedit("啊b", Some((3, 3)))` as landing
                // right after "啊" (a 3-byte character), not after char
                // index 1.
                let cursor_range = clamp_range(cursor_range, text.len());
                self.preedit = text.clone();
                self.cursor_range = cursor_range;
                if text.is_empty() {
                    ImeUpdate::Idle
                } else {
                    ImeUpdate::Composing { text, cursor_range }
                }
            }
            Ime::Commit(text) => {
                // A commit ends whatever was being composed, even a direct
                // commit with no preceding `Preedit` at all (e.g. a single
                // dead-key + base-letter combination some IMEs resolve in
                // one step).
                self.clear_preedit();
                ImeUpdate::Committed(text)
            }
            Ime::Disabled => {
                // Composition becoming unavailable (focus lost, IME
                // switched off) abandons any in-flight preedit — it must
                // never be silently promoted to a commit, or a user
                // clicking away mid-composition would insert text they
                // never confirmed.
                self.enabled = false;
                self.clear_preedit();
                ImeUpdate::Idle
            }
        }
    }

    fn clear_preedit(&mut self) {
        self.preedit.clear();
        self.cursor_range = None;
    }
}

/// Clamps a reported cursor range to the actual preedit text length. A
/// well-behaved platform never reports an out-of-range offset, but nothing
/// downstream (byte-slicing the preedit text to render it) should be able
/// to panic if one ever did.
fn clamp_range(range: Option<(usize, usize)>, len: usize) -> Option<(usize, usize)> {
    range.map(|(start, end)| (start.min(len), end.min(len)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_resets_any_stale_composition_state() {
        let mut ime = ImeComposition::new();
        ime.apply(Ime::Preedit {
            text: "k".to_string(),
            cursor_range: Some((0, 1)),
        });
        ime.apply(Ime::Enabled);
        assert!(ime.is_enabled());
        assert!(!ime.is_composing());
        assert_eq!(ime.preedit_text(), "");
        assert_eq!(ime.cursor_range(), None);
    }

    #[test]
    fn preedit_reports_composing_with_its_text_and_cursor_range() {
        let mut ime = ImeComposition::new();
        let update = ime.apply(Ime::Preedit {
            text: "ka".to_string(),
            cursor_range: Some((0, 2)),
        });
        assert_eq!(
            update,
            ImeUpdate::Composing {
                text: "ka".to_string(),
                cursor_range: Some((0, 2)),
            }
        );
        assert!(ime.is_composing());
        assert_eq!(ime.preedit_text(), "ka");
    }

    #[test]
    fn preedit_replaces_the_previous_text_rather_than_appending() {
        let mut ime = ImeComposition::new();
        // "あ" and "い" are 3 bytes each in UTF-8; cursor ranges below are
        // byte offsets, not char counts (see `apply`'s doc comment).
        ime.apply(Ime::Preedit {
            text: "あ".to_string(),
            cursor_range: Some((0, 3)),
        });
        ime.apply(Ime::Preedit {
            text: "あい".to_string(),
            cursor_range: Some((0, 6)),
        });
        assert_eq!(ime.preedit_text(), "あい");
    }

    #[test]
    fn cursor_range_is_a_byte_offset_not_a_char_count_for_multibyte_text() {
        // The exact example `winit`'s own docs give for `Ime::Preedit`:
        // "啊" is a 3-byte CJK character, so the cursor landing right after
        // it is byte offset 3, not char index 1.
        let mut ime = ImeComposition::new();
        ime.apply(Ime::Preedit {
            text: "啊b".to_string(),
            cursor_range: Some((3, 3)),
        });
        assert_eq!(ime.cursor_range(), Some((3, 3)));
    }

    #[test]
    fn empty_preedit_is_idle_not_composing() {
        let mut ime = ImeComposition::new();
        ime.apply(Ime::Preedit {
            text: "a".to_string(),
            cursor_range: Some((0, 1)),
        });
        let update = ime.apply(Ime::Preedit {
            text: String::new(),
            cursor_range: None,
        });
        assert_eq!(update, ImeUpdate::Idle);
        assert!(!ime.is_composing());
    }

    #[test]
    fn commit_clears_preedit_state_and_returns_the_final_text() {
        let mut ime = ImeComposition::new();
        ime.apply(Ime::Preedit {
            text: "かん".to_string(),
            cursor_range: Some((0, 6)), // 2 chars x 3 bytes each
        });
        let update = ime.apply(Ime::Commit("感".to_string()));
        assert_eq!(update, ImeUpdate::Committed("感".to_string()));
        assert!(!ime.is_composing());
        assert_eq!(ime.cursor_range(), None);
    }

    #[test]
    fn commit_without_any_prior_preedit_still_works() {
        // A single dead-key + base-letter sequence some IME implementations
        // resolve in one step, with no intermediate `Preedit` at all.
        let mut ime = ImeComposition::new();
        let update = ime.apply(Ime::Commit("á".to_string()));
        assert_eq!(update, ImeUpdate::Committed("á".to_string()));
        assert!(!ime.is_composing());
    }

    #[test]
    fn disabled_abandons_in_flight_composition_without_committing_it() {
        let mut ime = ImeComposition::new();
        ime.apply(Ime::Enabled);
        ime.apply(Ime::Preedit {
            text: "partial".to_string(),
            cursor_range: Some((0, 7)),
        });
        let update = ime.apply(Ime::Disabled);
        assert_eq!(
            update,
            ImeUpdate::Idle,
            "an abandoned composition must never turn into a Committed update"
        );
        assert!(!ime.is_enabled());
        assert!(!ime.is_composing());
    }

    #[test]
    fn cursor_range_is_clamped_to_the_preedit_texts_length() {
        let mut ime = ImeComposition::new();
        ime.apply(Ime::Preedit {
            text: "ab".to_string(),
            cursor_range: Some((0, 99)),
        });
        assert_eq!(ime.cursor_range(), Some((0, 2)));
    }

    #[test]
    fn a_realistic_japanese_composition_sequence_ends_in_one_commit() {
        let mut ime = ImeComposition::new();
        ime.apply(Ime::Enabled);
        ime.apply(Ime::Preedit {
            text: "k".to_string(),
            cursor_range: Some((0, 1)),
        });
        ime.apply(Ime::Preedit {
            text: "ka".to_string(),
            cursor_range: Some((0, 2)),
        });
        ime.apply(Ime::Preedit {
            text: "かん".to_string(),
            cursor_range: Some((0, 6)), // 2 chars x 3 bytes each
        });
        assert!(ime.is_composing());
        let update = ime.apply(Ime::Commit("感じ".to_string()));
        assert_eq!(update, ImeUpdate::Committed("感じ".to_string()));
        assert!(!ime.is_composing());
    }

    #[test]
    fn a_realistic_spanish_dead_key_sequence_shows_the_mark_then_commits() {
        // macOS's dead-key accent (´ then e) reports the bare accent as
        // preedit before committing the combined character.
        let mut ime = ImeComposition::new();
        let composing = ime.apply(Ime::Preedit {
            text: "´".to_string(),
            cursor_range: Some((0, 1)),
        });
        assert_eq!(
            composing,
            ImeUpdate::Composing {
                text: "´".to_string(),
                cursor_range: Some((0, 1)),
            }
        );
        let update = ime.apply(Ime::Commit("é".to_string()));
        assert_eq!(update, ImeUpdate::Committed("é".to_string()));
        assert!(!ime.is_composing());
    }
}
