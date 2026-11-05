//! Keyboard and mouse interaction for the integrated terminal dock.

use lux_platform::event::KeyEvent;
use lux_platform::{ElementState, Key, NamedKey, PlatformContext};
use lux_term::SourceLocation;

use crate::app::App;

pub(crate) fn handle_term_key(app: &mut App, _ctx: &dyn PlatformContext, event: &KeyEvent) -> bool {
    if event.state != ElementState::Pressed {
        return false;
    }

    // Ctrl+` / Cmd+` toggles the dock. A Spanish Shift+Command+[ can report
    // the same grave character while the physical key is the bracket, and
    // that chord switches tabs.
    let is_grave = match event.physical {
        Some(lux_platform::event::PhysicalKey::Backquote) => true,
        Some(_) => false,
        None => matches!(&event.key, Key::Character(c) if c == "`" || c == "~"),
    };
    let is_toggle = is_grave && (app.modifiers.control || app.modifiers.meta);

    if is_toggle {
        app.ui.term.visible = !app.ui.term.visible;
        if app.ui.term.visible {
            app.ui.term.focused = true;
            let root = app.workspace_root();
            let _ = app.ui.term.ensure_session(root.as_deref());
        } else {
            app.ui.term.focused = false;
        }
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
        return true;
    }

    if !app.ui.term.visible || !app.ui.term.focused {
        return false;
    }

    // 2. Escape: unfocus terminal and return to editor
    if matches!(&event.key, Key::Named(NamedKey::Escape)) {
        app.ui.term.focused = false;
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
        return true;
    }

    // 3. Write input to active PTY
    let bytes: Option<Vec<u8>> = match &event.key {
        Key::Named(NamedKey::Enter) => Some(vec![b'\r']),
        Key::Named(NamedKey::Backspace) => Some(vec![0x7f]),
        Key::Named(NamedKey::Tab) => Some(vec![b'\t']),
        Key::Named(NamedKey::ArrowUp) => Some(b"\x1b[A".to_vec()),
        Key::Named(NamedKey::ArrowDown) => Some(b"\x1b[B".to_vec()),
        Key::Named(NamedKey::ArrowRight) => Some(b"\x1b[C".to_vec()),
        Key::Named(NamedKey::ArrowLeft) => Some(b"\x1b[D".to_vec()),
        Key::Named(NamedKey::Home) => Some(b"\x1b[H".to_vec()),
        Key::Named(NamedKey::End) => Some(b"\x1b[F".to_vec()),
        Key::Named(NamedKey::PageUp) => Some(b"\x1b[5~".to_vec()),
        Key::Named(NamedKey::PageDown) => Some(b"\x1b[6~".to_vec()),
        Key::Named(NamedKey::Delete) => Some(b"\x1b[3~".to_vec()),
        Key::Character(c) => {
            if app.modifiers.control {
                match c.chars().next() {
                    Some('c' | 'C') => Some(vec![0x03]),
                    Some('d' | 'D') => Some(vec![0x04]),
                    Some('z' | 'Z') => Some(vec![0x1a]),
                    Some('l' | 'L') => Some(vec![0x0c]),
                    Some('a' | 'A') => Some(vec![0x01]),
                    Some('e' | 'E') => Some(vec![0x05]),
                    Some('k' | 'K') => Some(vec![0x0b]),
                    Some('u' | 'U') => Some(vec![0x15]),
                    Some('w' | 'W') => Some(vec![0x17]),
                    Some('r' | 'R') => Some(vec![0x12]),
                    _ => None,
                }
            } else if !app.modifiers.meta {
                Some(c.as_bytes().to_vec())
            } else {
                None
            }
        }
        _ => None,
    };

    if let Some(bytes) = bytes
        && let Some(pty) = app.ui.term.active_pty_mut()
    {
        let _ = pty.write(&bytes);
        app.scheduler.invalidate_all();
        if let Some(gpu) = &app.gpu_window {
            gpu.window.request_redraw();
        }
        return true;
    }

    false
}

/// Handles mouse clicks in the bottom terminal panel.
pub(crate) fn handle_term_click(app: &mut App, x: f32, y: f32) -> bool {
    if !app.ui.term.visible {
        return false;
    }

    let scale = app.scale_factor;
    let window_h = app.viewport.height + app.tab_bar_height() + app.status_bar_height();
    let term_h = app.ui.term.height * scale;
    let status_h = app.status_bar_height();
    let term_top = (window_h - status_h - term_h).max(0.0);
    let term_bottom = window_h - status_h;

    if y < term_top || y >= term_bottom {
        return false;
    }

    let header_h = 26.0 * scale;
    if y < term_top + header_h {
        // Click on header bar: tabs, +, Tasks, Close
        let mut tab_x = 8.0 * scale;
        let tab_w = 90.0 * scale;

        for (i, _) in app.ui.term.session_ids.iter().enumerate() {
            if x >= tab_x && x < tab_x + tab_w {
                // Check if close 'x' clicked (right side of tab)
                if x >= tab_x + tab_w - 20.0 * scale {
                    app.ui.term.close_session(i);
                } else {
                    app.ui.term.active_tab = i;
                    app.ui.term.show_tasks = false;
                    app.ui.term.focused = true;
                }
                app.scheduler.invalidate_all();
                return true;
            }
            tab_x += tab_w + 4.0 * scale;
        }

        // '+' new tab button
        if x >= tab_x && x < tab_x + 28.0 * scale {
            let root = app.workspace_root();
            let _ = app.ui.term.spawn_session(None, None, root.as_deref());
            app.ui.term.focused = true;
            app.scheduler.invalidate_all();
            return true;
        }
        tab_x += 32.0 * scale;

        // 'Tasks' button
        if x >= tab_x && x < tab_x + 60.0 * scale {
            app.ui.term.show_tasks = !app.ui.term.show_tasks;
            app.scheduler.invalidate_all();
            return true;
        }

        // Close panel 'x' button (top right)
        let close_x = app.viewport.width - 28.0 * scale;
        if x >= close_x {
            app.ui.term.visible = false;
            app.ui.term.focused = false;
            app.scheduler.invalidate_all();
            return true;
        }

        return true;
    }

    // Clicked in grid area: focus terminal and check for link jumps
    app.ui.term.focused = true;

    if app.ui.term.show_tasks {
        // In tasks view, check if Cancel button was clicked
        if let Some(task_id) = app.ui.term.active_task {
            app.ui.term.cancel_task(task_id);
            app.scheduler.invalidate_all();
        }
        return true;
    }

    // Detect clickable error links on clicked line
    let row_h = 18.0 * scale;
    let grid_y = y - (term_top + header_h);
    let clicked_row = (grid_y / row_h).floor() as usize;

    if let Some(pty) = app.ui.term.active_pty() {
        let links = pty.detect_links();
        let snap = pty.snapshot();
        let lines: Vec<&str> = snap.lines().collect();
        if let Some(clicked_line) = lines.get(clicked_row) {
            for link in &links {
                let filename = link.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if clicked_line.contains(filename) {
                    jump_to_link(app, link);
                    break;
                }
            }
        }
    }

    app.scheduler.invalidate_all();
    true
}

fn jump_to_link(app: &mut App, link: &SourceLocation) {
    let resolved_path = if link.path.is_absolute() {
        link.path.clone()
    } else if let Some(root) = app.workspace_root() {
        root.join(&link.path)
    } else {
        link.path.clone()
    };

    if resolved_path.exists() {
        crate::lsp::jump_to_location(app, &resolved_path, link.line.saturating_sub(1) as usize);
        app.ui.term.focused = false;
    }
}
