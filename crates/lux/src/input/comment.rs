//! Line and block comments. HTML, CSS, and Markdown have no line comment;
//! Command+/ wraps the line. Command+Option+/ always uses the block pair.

use lux_core::edit_ops;
use lux_core::position::Position;
use lux_core::selection::Selection;
use lux_syntax::LanguageId;

use crate::app::App;

pub(crate) fn toggle_line_or_block(app: &mut App) {
    if let Some(token) = app.line_comment_token() {
        app.apply_edit_operation(app.cursors.clone(), |buffer, selection| {
            edit_ops::toggle_line_comment(buffer, selection, token)
        });
        return;
    }
    toggle_block(app);
}

pub(super) fn toggle_block(app: &mut App) {
    let Some(lang) = comment_language(app) else {
        return;
    };
    let Some((start, end)) = block_tokens(lang) else {
        return;
    };
    app.apply_edit_operation(app.cursors.clone(), |buffer, selection| {
        let selection = if selection.is_empty() {
            line_content_selection(buffer, selection)
        } else {
            selection
        };
        edit_ops::toggle_block_comment(buffer, selection, start, end)
    });
}

fn comment_language(app: &App) -> Option<LanguageId> {
    if let Some(lang) = app.documents.language() {
        return Some(lang);
    }
    let extension = app.documents.path.as_ref()?.extension()?.to_str()?;
    LanguageId::from_extension(extension)
}

fn block_tokens(lang: LanguageId) -> Option<(&'static str, &'static str)> {
    use LanguageId::*;
    match lang {
        Html | Xml | Markdown => Some(("<!--", "-->")),
        Css | Scss | Json | Sql | Rust | TypeScript | Tsx | JavaScript | Go | C | Cpp | Java
        | Kotlin | Swift | Php | CSharp | Zig | Graphql | Proto | Hcl | Prisma | Gleam | Scala
        | Dart => Some(("/*", "*/")),
        Ocaml => Some(("(*", "*)")),
        _ => None,
    }
}

fn line_content_selection(buffer: &lux_core::buffer::Buffer, selection: Selection) -> Selection {
    let line = selection.head.line;
    let start = buffer.rope().line_to_char(line);
    let content = buffer.line_content(line).unwrap_or_default();
    let end = start + content.chars().count();
    if start == end {
        return selection;
    }
    Selection::new(
        Position::from_char(buffer, start),
        Position::from_char(buffer, end),
    )
}
