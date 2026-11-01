//! Unit tests for LSP desktop integration.

use lux_core::buffer::Buffer;
use lux_lsp::async_lsp::lsp_types::{CompletionItem, Position, Range, TextEdit};

use super::*;
use crate::app::App;

#[test]
fn lsp_language_mapping() {
    assert_eq!(
        lsp_language_for_path(Path::new("src/main.rs")),
        Some("rust")
    );
    assert_eq!(
        lsp_language_for_path(Path::new("index.ts")),
        Some("typescript")
    );
    assert_eq!(
        lsp_language_for_path(Path::new("script.py")),
        Some("python")
    );
    assert_eq!(lsp_language_for_path(Path::new("main.go")), Some("go"));
    assert_eq!(lsp_language_for_path(Path::new("unknown.txt")), None);
}

#[test]
fn completion_popup_selection_and_clamping() {
    let mut state = CompletionPopupState::showing(vec![
        CompletionItem {
            label: "alpha".into(),
            ..Default::default()
        },
        CompletionItem {
            label: "beta".into(),
            ..Default::default()
        },
    ]);

    assert_eq!(state.selected_index, 0);
    state.selected_index += 1;
    assert_eq!(state.selected_index, 1);
}

#[test]
fn apply_lsp_text_edits_replaces_correct_ranges() {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(b"fn old_name() {}\n").unwrap();

    let edits = vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 3,
            },
            end: Position {
                line: 0,
                character: 11,
            },
        },
        new_text: "new_func".into(),
    }];

    actions::apply_lsp_edits(&mut app, &edits);
    assert_eq!(app.buffer.text(), "fn new_func() {}\n");
}

#[test]
fn diagnostic_navigation_and_counts() {
    let mut app = App::new(include_bytes!("../../../lux-render/tests/fixtures/ahem.ttf").to_vec());
    app.buffer = Buffer::from_bytes(b"line 0\nline 1\nline 2\n").unwrap();

    app.ui.lsp.diagnostics = vec![
        DiagnosticItem {
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 5,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            message: "first error".into(),
        },
        DiagnosticItem {
            range: Range {
                start: Position {
                    line: 2,
                    character: 0,
                },
                end: Position {
                    line: 2,
                    character: 5,
                },
            },
            severity: Some(DiagnosticSeverity::WARNING),
            message: "second warning".into(),
        },
    ];

    assert_eq!(app.ui.lsp.error_count(), 1);
    assert_eq!(app.ui.lsp.warning_count(), 1);

    // Jump to next diagnostic from line 0 -> jumps to line 2
    assert!(jump_to_next_diagnostic(&mut app));
    assert_eq!(app.cursors.primary().head.line, 2);

    // Jump to prev diagnostic from line 2 -> jumps to line 0
    assert!(jump_to_prev_diagnostic(&mut app));
    assert_eq!(app.cursors.primary().head.line, 0);
}
