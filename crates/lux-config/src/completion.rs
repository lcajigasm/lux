//! Schema-aware completion for editing a Lux settings `.jsonc` file by hand
//! (Phase 5 deliverable 6 — "Panel de preferencias, o edición de fichero
//! con autocompletado del esquema (elige uno; documenta la elección)").
//!
//! **Choice: file editing with schema-aware completion, not a preferences
//! UI panel.** Every other Phase 5 deliverable in this crate is headless,
//! UI-free logic — `layering`, `keymap`, `editorconfig`, `gitattributes`,
//! `import`, `font_settings` all compile and are fully tested with zero
//! graphics dependency, matching golden rule #2's spirit even though the
//! rule's literal CI gate only checks `lux-core`. A preferences *panel*
//! would need to live in `lux-ui`/`lux` instead (real widgets, real
//! rendering, real input handling), on top of UI infrastructure that
//! doesn't exist yet for this specific purpose — building it here would
//! both violate that established pattern and pull `lux-config` toward a
//! graphics dependency it has never needed. File-editing completion, by
//! contrast, is exactly the kind of thing this crate already does: pure
//! text-and-schema logic a future editor surface can call into, the same
//! "produce the data, wiring it into the `lux` binary is a future
//! integration's job" shape every earlier loop this phase already used.
//!
//! The engine here is a small, single-pass, JSONC-comment-aware state
//! machine — not the full `jsonc-parser` AST this crate uses elsewhere.
//! That's deliberate: completion has to work on a document that's
//! *mid-edit*, which is very often syntactically incomplete exactly at the
//! cursor (an unterminated string, a key with no value yet) — a real
//! parser built for complete, valid documents would simply fail to parse
//! at all in that state. This scanner is explicitly tolerant of that:
//! it's built to answer "what's the cursor's grammatical position" even
//! when the text isn't valid JSON yet, at the cost of only understanding a
//! **flat, single-level object** — which is what every settings document
//! in this crate actually is (`layering`'s settings, every `import`
//! output, `font_settings`'s keys — none of them nest). A nested object or
//! array value would confuse this scanner; that's a real, documented
//! limitation, not a silent wrong answer for the flat case this crate
//! actually needs.

use std::collections::HashSet;
use std::ops::Range;

use crate::schema::{self, PropertyKind};

/// One completion candidate. `replace_range` is the span of already-typed
/// text (if any) this candidate replaces — `insert_text` is what should
/// go in its place, already including any quoting the position needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub detail: String,
    pub insert_text: String,
    pub replace_range: Range<usize>,
}

/// Returns every completion that fits at `offset` (a byte offset into
/// `text`, which must land on a `char` boundary — the same precondition
/// `str` slicing already has everywhere else). Empty when the cursor isn't
/// at a position this engine understands (e.g. between a value and its
/// following comma, or inside a comment).
pub fn complete(text: &str, offset: usize) -> Vec<CompletionItem> {
    match classify(text, offset) {
        Some(Context::Key {
            prefix,
            replace_range,
        }) => {
            let present = existing_keys(text);
            schema::PROPERTIES
                .iter()
                .filter(|p| p.key.starts_with(prefix.as_str()) && !present.contains(p.key))
                .map(|p| CompletionItem {
                    label: p.key.to_string(),
                    detail: p.description.to_string(),
                    insert_text: p.key.to_string(),
                    replace_range: replace_range.clone(),
                })
                .collect()
        }
        Some(Context::Value {
            key,
            prefix,
            replace_range,
            already_quoted,
        }) => {
            let Some(found) = schema::find(&key) else {
                return Vec::new();
            };
            match found.kind {
                PropertyKind::Enum(values) => values
                    .iter()
                    .filter(|v| v.starts_with(prefix.as_str()))
                    .map(|v| CompletionItem {
                        label: (*v).to_string(),
                        detail: found.description.to_string(),
                        insert_text: if already_quoted {
                            (*v).to_string()
                        } else {
                            format!("\"{v}\"")
                        },
                        replace_range: replace_range.clone(),
                    })
                    .collect(),
                // A boolean is a bare JSON literal (`true`, not `"true"`):
                // only offered outside an already-open string, where
                // inserting it verbatim would otherwise become a string
                // value instead of the boolean this key actually needs.
                PropertyKind::Boolean if !already_quoted => ["true", "false"]
                    .into_iter()
                    .filter(|v| v.starts_with(prefix.as_str()))
                    .map(|v| CompletionItem {
                        label: (*v).to_string(),
                        detail: found.description.to_string(),
                        insert_text: (*v).to_string(),
                        replace_range: replace_range.clone(),
                    })
                    .collect(),
                // String/Integer/Number, or a Boolean already inside an
                // open string: open-ended or mismatched — nothing honest
                // to suggest.
                _ => Vec::new(),
            }
        }
        None => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Context {
    Key {
        prefix: String,
        replace_range: Range<usize>,
    },
    Value {
        key: String,
        prefix: String,
        replace_range: Range<usize>,
        already_quoted: bool,
    },
}

fn classify(text: &str, offset: usize) -> Option<Context> {
    match scan(text, offset).mode {
        Mode::Neutral => Some(Context::Key {
            prefix: String::new(),
            replace_range: offset..offset,
        }),
        Mode::InKeyString { start } => Some(Context::Key {
            prefix: text[start + 1..offset].to_string(),
            replace_range: (start + 1)..offset,
        }),
        Mode::AfterColon { key } => Some(Context::Value {
            key,
            prefix: String::new(),
            replace_range: offset..offset,
            already_quoted: false,
        }),
        Mode::InValueString { start, key } => Some(Context::Value {
            key,
            prefix: text[start + 1..offset].to_string(),
            replace_range: (start + 1)..offset,
            already_quoted: true,
        }),
        Mode::InValueLiteral { start, key } => Some(Context::Value {
            key,
            prefix: text[start..offset].to_string(),
            replace_range: start..offset,
            already_quoted: false,
        }),
        Mode::AfterKeyString { .. } | Mode::AfterValue | Mode::InComment => None,
    }
}

/// Every key that has a `:` after it anywhere in `text` — used to exclude
/// keys the document already has from key-position suggestions.
fn existing_keys(text: &str) -> HashSet<String> {
    scan(text, text.len()).keys_seen.into_iter().collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Mode {
    /// Between tokens: a key, `}`, or (after the object opens) nothing yet
    /// is expected here.
    Neutral,
    InKeyString {
        start: usize,
    },
    /// A key string just closed; waiting for `:`.
    AfterKeyString {
        key: String,
    },
    /// `:` just seen; waiting for the value to start.
    AfterColon {
        key: String,
    },
    InValueString {
        start: usize,
        key: String,
    },
    /// A bare (unquoted) value token in progress — `true`, `false`, or a
    /// number.
    InValueLiteral {
        start: usize,
        key: String,
    },
    /// A value just closed; waiting for `,` or `}`.
    AfterValue,
    /// Inside a `//` or `/* */` comment.
    InComment,
}

struct ScanState {
    mode: Mode,
    escape: bool,
    keys_seen: Vec<String>,
}

/// Runs the state machine over `text` up to (not including) `stop_at`,
/// which must be a `char` boundary. `stop_at == text.len()` scans the
/// whole document (used by [`existing_keys`]); any smaller value is where
/// [`classify`] wants to know the cursor's grammatical position.
fn scan(text: &str, stop_at: usize) -> ScanState {
    let mut state = ScanState {
        mode: Mode::Neutral,
        escape: false,
        keys_seen: Vec::new(),
    };
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut idx = 0;
    while idx < chars.len() {
        let (i, ch) = chars[idx];
        if i >= stop_at {
            break;
        }

        let in_string = matches!(
            state.mode,
            Mode::InKeyString { .. } | Mode::InValueString { .. }
        );
        if !in_string && ch == '/' {
            let next = chars.get(idx + 1).map(|&(_, c)| c);
            if next == Some('/') {
                idx = skip_line_comment(&chars, idx, stop_at, &mut state);
                continue;
            }
            if next == Some('*') {
                idx = skip_block_comment(&chars, idx, stop_at, &mut state);
                continue;
            }
        }

        step(&mut state, text, i, ch);
        idx += 1;
    }
    state
}

/// Advances past a `//` line comment, or leaves `state.mode` as
/// [`Mode::InComment`] and returns `chars.len()` if `stop_at` falls inside
/// it — the cursor sitting inside a comment has no valid completion.
fn skip_line_comment(
    chars: &[(usize, char)],
    start: usize,
    stop_at: usize,
    state: &mut ScanState,
) -> usize {
    let mut idx = start + 2;
    while idx < chars.len() {
        let (i, ch) = chars[idx];
        if i >= stop_at {
            state.mode = Mode::InComment;
            return chars.len();
        }
        if ch == '\n' {
            return idx + 1;
        }
        idx += 1;
    }
    // Ran out of text while still inside the comment (an unterminated
    // trailing `//` with no final newline) — everything up to `stop_at`
    // was comment content.
    state.mode = Mode::InComment;
    chars.len()
}

/// Same as [`skip_line_comment`] but for `/* ... */`.
fn skip_block_comment(
    chars: &[(usize, char)],
    start: usize,
    stop_at: usize,
    state: &mut ScanState,
) -> usize {
    let mut idx = start + 2;
    while idx < chars.len() {
        let (i, ch) = chars[idx];
        if i >= stop_at {
            state.mode = Mode::InComment;
            return chars.len();
        }
        if ch == '*' && chars.get(idx + 1).map(|&(_, c)| c) == Some('/') {
            return idx + 2;
        }
        idx += 1;
    }
    // Ran out of text while still inside the comment (an unterminated
    // `/*` with no matching `*/`) — everything up to `stop_at` was
    // comment content.
    state.mode = Mode::InComment;
    chars.len()
}

fn step(state: &mut ScanState, text: &str, i: usize, ch: char) {
    state.mode = match std::mem::replace(&mut state.mode, Mode::Neutral) {
        Mode::Neutral => match ch {
            '"' => Mode::InKeyString { start: i },
            _ => Mode::Neutral,
        },
        Mode::InKeyString { start } => {
            if state.escape {
                state.escape = false;
                Mode::InKeyString { start }
            } else if ch == '\\' {
                state.escape = true;
                Mode::InKeyString { start }
            } else if ch == '"' {
                Mode::AfterKeyString {
                    key: text[start + 1..i].to_string(),
                }
            } else {
                Mode::InKeyString { start }
            }
        }
        Mode::AfterKeyString { key } => match ch {
            ':' => {
                state.keys_seen.push(key.clone());
                Mode::AfterColon { key }
            }
            _ => Mode::AfterKeyString { key },
        },
        Mode::AfterColon { key } => match ch {
            '"' => Mode::InValueString { start: i, key },
            c if c.is_whitespace() => Mode::AfterColon { key },
            ',' | '}' => Mode::Neutral,
            _ => Mode::InValueLiteral { start: i, key },
        },
        Mode::InValueString { start, key } => {
            if state.escape {
                state.escape = false;
                Mode::InValueString { start, key }
            } else if ch == '\\' {
                state.escape = true;
                Mode::InValueString { start, key }
            } else if ch == '"' {
                Mode::AfterValue
            } else {
                Mode::InValueString { start, key }
            }
        }
        Mode::InValueLiteral { start, key } => match ch {
            ',' | '}' => Mode::Neutral,
            c if c.is_whitespace() => Mode::AfterValue,
            _ => Mode::InValueLiteral { start, key },
        },
        Mode::AfterValue => match ch {
            ',' | '}' => Mode::Neutral,
            _ => Mode::AfterValue,
        },
        Mode::InComment => Mode::InComment,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        let mut labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        labels.sort_unstable();
        labels
    }

    #[test]
    fn offers_every_known_key_right_after_the_opening_brace() {
        let text = "{}";
        let items = complete(text, 1);
        assert_eq!(items.len(), schema::PROPERTIES.len());
        assert!(labels(&items).contains(&"editor.fontFamily"));
    }

    #[test]
    fn filters_key_completions_by_the_typed_prefix() {
        let text = r#"{"editor.font"#;
        let items = complete(text, text.len());
        assert_eq!(
            labels(&items),
            vec![
                "editor.fontFamily",
                "editor.fontLigatures",
                "editor.fontSize"
            ]
        );
    }

    #[test]
    fn excludes_a_key_the_document_already_has() {
        let text = r#"{"editor.fontFamily": "Menlo", "editor."#;
        let items = complete(text, text.len());
        assert!(!labels(&items).contains(&"editor.fontFamily"));
        assert!(labels(&items).contains(&"editor.fontSize"));
    }

    #[test]
    fn key_insert_text_is_the_bare_key_no_quotes_or_colon() {
        let text = "{";
        let items = complete(text, 1);
        let family = items
            .iter()
            .find(|i| i.label == "editor.fontFamily")
            .unwrap();
        assert_eq!(family.insert_text, "editor.fontFamily");
        assert_eq!(family.replace_range, 1..1);
    }

    #[test]
    fn offers_quoted_enum_values_right_after_the_colon() {
        let text = r#"{"editor.indentStyle": "#;
        let items = complete(text, text.len());
        assert_eq!(labels(&items), vec!["space", "tab"]);
        let tab = items.iter().find(|i| i.label == "tab").unwrap();
        assert_eq!(tab.insert_text, "\"tab\"");
    }

    #[test]
    fn offers_unquoted_enum_values_inside_an_already_open_string() {
        let text = r#"{"editor.indentStyle": "ta"#;
        let items = complete(text, text.len());
        assert_eq!(labels(&items), vec!["tab"]);
        assert_eq!(items[0].insert_text, "tab");
        // Replaces just the typed "ta", not the surrounding quotes.
        let quote_pos = text.rfind('"').unwrap();
        assert_eq!(items[0].replace_range, (quote_pos + 1)..text.len());
    }

    #[test]
    fn offers_bare_boolean_literals_for_a_boolean_key() {
        let text = r#"{"editor.fontLigatures": "#;
        let items = complete(text, text.len());
        assert_eq!(labels(&items), vec!["false", "true"]);
        assert_eq!(items[0].insert_text, "true");
    }

    #[test]
    fn does_not_offer_a_boolean_literal_inside_an_open_string() {
        // Inserting `true` here would produce the string "true", not the
        // boolean this key needs — a real mismatch, so nothing is offered.
        let text = r#"{"editor.fontLigatures": ""#;
        let items = complete(text, text.len());
        assert!(items.is_empty());
    }

    #[test]
    fn open_ended_kinds_offer_no_value_completions() {
        let text = r#"{"editor.fontFamily": "#;
        assert!(complete(text, text.len()).is_empty());
    }

    #[test]
    fn an_unknown_key_offers_no_value_completions() {
        let text = r#"{"editor.doesNotExist": "#;
        assert!(complete(text, text.len()).is_empty());
    }

    #[test]
    fn no_completions_right_after_a_complete_value_waiting_for_a_comma() {
        let text = r#"{"editor.fontFamily": "Menlo""#;
        assert!(complete(text, text.len()).is_empty());
    }

    #[test]
    fn resumes_key_completion_after_a_comma() {
        let text = r#"{"editor.fontFamily": "Menlo", "#;
        let items = complete(text, text.len());
        assert!(!items.is_empty());
        assert!(!labels(&items).contains(&"editor.fontFamily"));
    }

    #[test]
    fn a_line_comment_before_the_cursor_does_not_confuse_key_detection() {
        let text = "{\n  // pick a font\n  \"editor.font";
        let items = complete(text, text.len());
        assert!(labels(&items).contains(&"editor.fontFamily"));
    }

    #[test]
    fn a_block_comment_between_colon_and_value_does_not_confuse_value_detection() {
        let text = r#"{"editor.indentStyle": /* pick one */ ""#;
        let items = complete(text, text.len());
        assert_eq!(labels(&items), vec!["space", "tab"]);
    }

    #[test]
    fn the_cursor_inside_a_comment_offers_nothing() {
        let text = "{ // editor.font";
        assert!(complete(text, text.len()).is_empty());
    }

    #[test]
    fn an_escaped_quote_inside_a_key_does_not_end_the_string_early() {
        let text = r#"{"weird\"key": "#;
        // Not a schema key, so no value completions — but the point of
        // this test is that classify() doesn't panic or misparse.
        assert!(complete(text, text.len()).is_empty());
    }
}
