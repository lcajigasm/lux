//! The open list popup, as the probe dump reports it.

pub(crate) fn open_picker(app: &crate::app::App) -> (&'static str, usize, usize, String) {
    if let Some(picker) = &app.ui.language_picker {
        let filtered = picker.filtered();
        let label = filtered
            .get(picker.selected_index)
            .map(|item| item.1.to_string())
            .unwrap_or_default();
        return ("language", picker.selected_index, filtered.len(), label);
    }
    if let Some(picker) = &app.ui.theme_picker {
        let filtered = picker.filtered();
        let label = filtered
            .get(picker.selected_index)
            .copied()
            .unwrap_or("")
            .to_string();
        return ("theme", picker.selected_index, filtered.len(), label);
    }
    if let Some(palette) = &app.ui.command_palette {
        let mut context = lux_ui::command_palette::Context::new();
        context.set("editorFocus", true);
        let results = palette.results(&context);
        let label = results
            .get(palette.selected_index)
            .map(|entry| entry.command.title.clone())
            .unwrap_or_default();
        return ("palette", palette.selected_index, results.len(), label);
    }
    if let Some(goto) = &app.ui.goto_anything {
        let label = goto
            .items
            .get(goto.selected_index)
            .map(|item| item.label.clone())
            .unwrap_or_default();
        return ("goto", goto.selected_index, goto.items.len(), label);
    }
    ("off", 0, 0, String::new())
}
