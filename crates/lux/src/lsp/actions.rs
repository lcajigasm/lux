use std::path::Path;
use std::thread;

use lux_core::buffer::Buffer;
use lux_lsp::async_lsp::lsp_types::{FormattingOptions, Position, TextEdit};
use lux_lsp::{Completion, Formatting, Hover, Navigation};
use lux_platform::PlatformContext;

use super::LspEvent;
use crate::app::App;
use crate::document::DocumentEntry;

pub(crate) fn request_completion(app: &mut App, ctx: &dyn PlatformContext) {
    let primary_head = app.cursors.primary().head;
    let lsp_pos = Position {
        line: primary_head.line as u32,
        character: primary_head.column as u32,
    };

    if let Some(doc_path) = app.documents.active_entry().and_then(|e| e.path.as_ref())
        && app.ui.settings.is_settings_path(doc_path)
    {
        let text = app.buffer.text();
        let byte_offset = position_to_byte_offset(&text, lsp_pos);
        let settings = std::mem::take(&mut app.ui.settings);
        let handled = settings.complete_buffer(app, &text, byte_offset);
        app.ui.settings = settings;
        if handled {
            return;
        }
    }

    let Some(session) = &app.ui.lsp.current_session else {
        return;
    };
    let Some(url) = &app.ui.lsp.current_url else {
        return;
    };

    let session = session.clone();
    let url = url.clone();
    let sender = app.ui.lsp.sender.clone();
    let waker = ctx.background_waker();

    thread::Builder::new()
        .name("lux-lsp-complete".into())
        .spawn(move || {
            let comp = Completion::new(session);
            if let Ok(items) = pollster::block_on(comp.complete(&url, lsp_pos, None))
                && sender.send(LspEvent::CompletionsReady { items }).is_ok()
            {
                waker();
            }
        })
        .ok();
}

pub(crate) fn request_hover(app: &mut App, ctx: &dyn PlatformContext) {
    let Some(session) = &app.ui.lsp.current_session else {
        return;
    };
    let Some(url) = &app.ui.lsp.current_url else {
        return;
    };

    let primary_head = app.cursors.primary().head;
    let lsp_pos = Position {
        line: primary_head.line as u32,
        character: primary_head.column as u32,
    };

    let session = session.clone();
    let url = url.clone();
    let sender = app.ui.lsp.sender.clone();
    let waker = ctx.background_waker();

    thread::Builder::new()
        .name("lux-lsp-hover".into())
        .spawn(move || {
            let h = Hover::new(session);
            if let Ok(hover_res) = pollster::block_on(h.hover(&url, lsp_pos)) {
                let text = hover_res.map(|h| h.text);
                if sender.send(LspEvent::HoverReady { text }).is_ok() {
                    waker();
                }
            }
        })
        .ok();
}

pub(crate) fn goto_definition(app: &mut App, ctx: &dyn PlatformContext) {
    let Some(session) = &app.ui.lsp.current_session else {
        return;
    };
    let Some(url) = &app.ui.lsp.current_url else {
        return;
    };

    let primary_head = app.cursors.primary().head;
    let lsp_pos = Position {
        line: primary_head.line as u32,
        character: primary_head.column as u32,
    };

    let session = session.clone();
    let url = url.clone();
    let sender = app.ui.lsp.sender.clone();
    let waker = ctx.background_waker();

    thread::Builder::new()
        .name("lux-lsp-definition".into())
        .spawn(move || {
            let nav = Navigation::new(session);
            if let Ok(locations) = pollster::block_on(nav.definition(&url, lsp_pos))
                && sender.send(LspEvent::DefinitionReady { locations }).is_ok()
            {
                waker();
            }
        })
        .ok();
}

pub(crate) fn find_references(app: &mut App, ctx: &dyn PlatformContext) {
    let Some(session) = &app.ui.lsp.current_session else {
        return;
    };
    let Some(url) = &app.ui.lsp.current_url else {
        return;
    };

    let primary_head = app.cursors.primary().head;
    let lsp_pos = Position {
        line: primary_head.line as u32,
        character: primary_head.column as u32,
    };

    let session = session.clone();
    let url = url.clone();
    let sender = app.ui.lsp.sender.clone();
    let waker = ctx.background_waker();

    thread::Builder::new()
        .name("lux-lsp-references".into())
        .spawn(move || {
            let nav = Navigation::new(session);
            if let Ok(locations) = pollster::block_on(nav.references(&url, lsp_pos, true))
                && sender.send(LspEvent::ReferencesReady { locations }).is_ok()
            {
                waker();
            }
        })
        .ok();
}

pub(crate) fn format_document(app: &mut App, ctx: &dyn PlatformContext) {
    let Some(session) = &app.ui.lsp.current_session else {
        return;
    };
    let Some(url) = &app.ui.lsp.current_url else {
        return;
    };

    let session = session.clone();
    let url = url.clone();
    let sender = app.ui.lsp.sender.clone();
    let waker = ctx.background_waker();

    thread::Builder::new()
        .name("lux-lsp-format".into())
        .spawn(move || {
            let fmt = Formatting::new(session);
            if let Ok(edits) = pollster::block_on(fmt.document(
                &url,
                FormattingOptions {
                    tab_size: 4,
                    insert_spaces: true,
                    ..Default::default()
                },
            )) && sender.send(LspEvent::FormattingReady { edits }).is_ok()
            {
                waker();
            }
        })
        .ok();
}

pub(crate) fn jump_to_location(app: &mut App, target_path: &Path, target_line: usize) {
    if app.documents.path.as_deref() == Some(target_path) {
        crate::goto::jump_to_line(app, target_line as u32 + 1);
        return;
    }

    if let Some(entry) = app
        .documents
        .entries
        .iter()
        .find(|e| e.path.as_deref() == Some(target_path))
    {
        let tab_id = entry.tab_id;
        app.switch_to_tab(tab_id);
        crate::goto::jump_to_line(app, target_line as u32 + 1);
    } else if let Ok(buf) = Buffer::open(target_path) {
        let title = target_path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("Untitled");
        let tab_id = app.documents.tabs.open(title);
        let entry = DocumentEntry::new(tab_id, Some(target_path.to_path_buf()), buf);
        app.documents.entries.push(entry);
        app.switch_to_tab(tab_id);
        crate::goto::jump_to_line(app, target_line as u32 + 1);
    }
}

pub(crate) fn apply_lsp_edits(app: &mut App, edits: &[TextEdit]) {
    if edits.is_empty() {
        return;
    }
    let mut sorted = edits.to_vec();
    sorted.sort_by_key(|e| std::cmp::Reverse(e.range.start));

    let mut current_text = app.buffer.text();
    for edit in sorted {
        let start_byte = position_to_byte_offset(&current_text, edit.range.start);
        let end_byte = position_to_byte_offset(&current_text, edit.range.end);
        let mut new_text = String::with_capacity(current_text.len() + edit.new_text.len());
        new_text.push_str(&current_text[..start_byte]);
        new_text.push_str(&edit.new_text);
        new_text.push_str(&current_text[end_byte..]);
        current_text = new_text;
    }

    if let Ok(new_buf) = Buffer::from_bytes(current_text.as_bytes()) {
        app.buffer = new_buf;
        app.update_syntax_on_open();
    }
}

pub(crate) fn position_to_byte_offset(text: &str, position: Position) -> usize {
    let mut byte_offset = 0;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        if line_index as u32 == position.line {
            let mut utf16_units = 0u32;
            for (offset_in_line, ch) in line.char_indices() {
                if utf16_units >= position.character {
                    return byte_offset + offset_in_line;
                }
                utf16_units += ch.len_utf16() as u32;
            }
            return byte_offset + line.trim_end_matches('\n').len();
        }
        byte_offset += line.len();
    }
    byte_offset
}
