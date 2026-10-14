//! English and Spanish string catalog plus locale date/number formats.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    En,
    Es,
}

pub const ALL_KEYS: &[&str] = &[
    "command.palette",
    "command.change_language",
    "command.change_encoding",
    "command.change_eol",
    "command.change_indent",
    "file.new",
    "file.open",
    "file.open_folder",
    "file.save",
    "file.save_as",
    "file.close_tab",
    "file.close_window",
    "file.untitled",
    "edit.undo",
    "edit.redo",
    "edit.cut",
    "edit.copy",
    "edit.paste",
    "edit.select_all",
    "edit.duplicate_line",
    "edit.delete_line",
    "edit.join_lines",
    "edit.indent",
    "edit.dedent",
    "edit.toggle_comment",
    "search.find",
    "search.replace",
    "search.replace_all",
    "search.match_case",
    "search.whole_word",
    "search.regex",
    "search.no_matches",
    "search.project_find",
    "search.project_replace",
    "nav.goto_anything",
    "nav.goto_line",
    "view.toggle_sidebar",
    "view.toggle_minimap",
    "view.toggle_terminal",
    "view.toggle_problems",
    "git.changes",
    "git.stage",
    "git.unstage",
    "git.revert",
    "git.commit",
    "git.diff",
    "git.conflict_current",
    "git.conflict_incoming",
    "git.conflict_both",
    "conflict.title",
    "conflict.compare",
    "conflict.overwrite",
    "conflict.reload",
    "term.panel",
    "term.new",
    "term.close",
    "term.tasks",
    "term.run_task",
    "term.cancel_task",
    "lsp.problems",
    "lsp.goto_definition",
    "lsp.find_references",
    "lsp.format_document",
    "lsp.hover",
];

impl Locale {
    pub fn parse(tag: &str) -> Self {
        let t = tag.to_ascii_lowercase();
        if t.starts_with("es") {
            Locale::Es
        } else {
            Locale::En
        }
    }

    pub fn t(self, key: &str) -> &'static str {
        match (self, key) {
            (Locale::En, "command.palette") => "Command Palette",
            (Locale::Es, "command.palette") => "Paleta de comandos",
            (Locale::En, "command.change_language") => "Change Language Mode",
            (Locale::Es, "command.change_language") => "Cambiar modo de lenguaje",
            (Locale::En, "command.change_encoding") => "Change File Encoding",
            (Locale::Es, "command.change_encoding") => "Cambiar codificación de archivo",
            (Locale::En, "command.change_eol") => "Change Line Ending",
            (Locale::Es, "command.change_eol") => "Cambiar final de línea",
            (Locale::En, "command.change_indent") => "Change Indentation",
            (Locale::Es, "command.change_indent") => "Cambiar sangría",

            (Locale::En, "file.new") => "New File",
            (Locale::Es, "file.new") => "Nuevo archivo",
            (Locale::En, "file.open") => "Open File...",
            (Locale::Es, "file.open") => "Abrir archivo...",
            (Locale::En, "file.open_folder") => "Open Folder...",
            (Locale::Es, "file.open_folder") => "Abrir carpeta...",
            (Locale::En, "file.save") => "Save",
            (Locale::Es, "file.save") => "Guardar",
            (Locale::En, "file.save_as") => "Save As...",
            (Locale::Es, "file.save_as") => "Guardar como...",
            (Locale::En, "file.close_tab") => "Close Tab",
            (Locale::Es, "file.close_tab") => "Cerrar pestaña",
            (Locale::En, "file.close_window") => "Close Window",
            (Locale::Es, "file.close_window") => "Cerrar ventana",
            (Locale::En, "file.untitled") => "Untitled",
            (Locale::Es, "file.untitled") => "Sin título",

            (Locale::En, "edit.undo") => "Undo",
            (Locale::Es, "edit.undo") => "Deshacer",
            (Locale::En, "edit.redo") => "Redo",
            (Locale::Es, "edit.redo") => "Rehacer",
            (Locale::En, "edit.cut") => "Cut",
            (Locale::Es, "edit.cut") => "Cortar",
            (Locale::En, "edit.copy") => "Copy",
            (Locale::Es, "edit.copy") => "Copiar",
            (Locale::En, "edit.paste") => "Paste",
            (Locale::Es, "edit.paste") => "Pegar",
            (Locale::En, "edit.select_all") => "Select All",
            (Locale::Es, "edit.select_all") => "Seleccionar todo",
            (Locale::En, "edit.duplicate_line") => "Duplicate Line",
            (Locale::Es, "edit.duplicate_line") => "Duplicar línea",
            (Locale::En, "edit.delete_line") => "Delete Line",
            (Locale::Es, "edit.delete_line") => "Eliminar línea",
            (Locale::En, "edit.join_lines") => "Join Lines",
            (Locale::Es, "edit.join_lines") => "Unir líneas",
            (Locale::En, "edit.indent") => "Indent",
            (Locale::Es, "edit.indent") => "Sangrar",
            (Locale::En, "edit.dedent") => "Dedent",
            (Locale::Es, "edit.dedent") => "Reducir sangría",
            (Locale::En, "edit.toggle_comment") => "Toggle Line Comment",
            (Locale::Es, "edit.toggle_comment") => "Alternar comentario de línea",

            (Locale::En, "search.find") => "Find",
            (Locale::Es, "search.find") => "Buscar",
            (Locale::En, "search.replace") => "Replace",
            (Locale::Es, "search.replace") => "Reemplazar",
            (Locale::En, "search.replace_all") => "Replace All",
            (Locale::Es, "search.replace_all") => "Reemplazar todo",
            (Locale::En, "search.match_case") => "Match Case",
            (Locale::Es, "search.match_case") => "Coincidir mayúsculas y minúsculas",
            (Locale::En, "search.whole_word") => "Whole Word",
            (Locale::Es, "search.whole_word") => "Palabra completa",
            (Locale::En, "search.regex") => "Regular Expression",
            (Locale::Es, "search.regex") => "Expresión regular",
            (Locale::En, "search.no_matches") => "No matches",
            (Locale::Es, "search.no_matches") => "Sin coincidencias",
            (Locale::En, "search.project_find") => "Find in Files",
            (Locale::Es, "search.project_find") => "Buscar en archivos",
            (Locale::En, "search.project_replace") => "Replace in Files",
            (Locale::Es, "search.project_replace") => "Reemplazar en archivos",
            (Locale::En, "nav.goto_anything") => "Goto Anything",
            (Locale::Es, "nav.goto_anything") => "Ir a cualquier parte",
            (Locale::En, "nav.goto_line") => "Go to Line...",
            (Locale::Es, "nav.goto_line") => "Ir a línea...",

            (Locale::En, "view.toggle_sidebar") => "Toggle Sidebar",
            (Locale::Es, "view.toggle_sidebar") => "Alternar barra lateral",
            (Locale::En, "view.toggle_minimap") => "Toggle Minimap",
            (Locale::Es, "view.toggle_minimap") => "Alternar minimapa",
            (Locale::En, "view.toggle_terminal") => "Toggle Terminal Panel",
            (Locale::Es, "view.toggle_terminal") => "Alternar panel de terminal",
            (Locale::En, "view.toggle_problems") => "Toggle Problems Panel",
            (Locale::Es, "view.toggle_problems") => "Alternar panel de problemas",

            (Locale::En, "git.changes") => "Source Control",
            (Locale::Es, "git.changes") => "Control de versiones",
            (Locale::En, "git.stage") => "Stage",
            (Locale::Es, "git.stage") => "Preparar",
            (Locale::En, "git.unstage") => "Unstage",
            (Locale::Es, "git.unstage") => "Despreparar",
            (Locale::En, "git.revert") => "Revert",
            (Locale::Es, "git.revert") => "Revertir",
            (Locale::En, "git.commit") => "Commit",
            (Locale::Es, "git.commit") => "Confirmar cambios",
            (Locale::En, "git.diff") => "View Diff",
            (Locale::Es, "git.diff") => "Ver diferencias",
            (Locale::En, "git.conflict_current") => "Accept Current",
            (Locale::Es, "git.conflict_current") => "Aceptar actual",
            (Locale::En, "git.conflict_incoming") => "Accept Incoming",
            (Locale::Es, "git.conflict_incoming") => "Aceptar entrante",
            (Locale::En, "git.conflict_both") => "Accept Both",
            (Locale::Es, "git.conflict_both") => "Aceptar ambos",

            (Locale::En, "conflict.title") => "File modified on disk",
            (Locale::Es, "conflict.title") => "Archivo modificado en el disco",
            (Locale::En, "conflict.compare") => "Compare Diff",
            (Locale::Es, "conflict.compare") => "Comparar diferencias",
            (Locale::En, "conflict.overwrite") => "Overwrite Disk",
            (Locale::Es, "conflict.overwrite") => "Sobrescribir disco",
            (Locale::En, "conflict.reload") => "Reload Disk",
            (Locale::Es, "conflict.reload") => "Recargar disco",

            (Locale::En, "term.panel") => "Terminal",
            (Locale::Es, "term.panel") => "Terminal",
            (Locale::En, "term.new") => "New Terminal",
            (Locale::Es, "term.new") => "Nueva terminal",
            (Locale::En, "term.close") => "Close Terminal",
            (Locale::Es, "term.close") => "Cerrar terminal",
            (Locale::En, "term.tasks") => "Tasks",
            (Locale::Es, "term.tasks") => "Tareas",
            (Locale::En, "term.run_task") => "Run Task",
            (Locale::Es, "term.run_task") => "Ejecutar tarea",
            (Locale::En, "term.cancel_task") => "Cancel Task",
            (Locale::Es, "term.cancel_task") => "Cancelar tarea",

            (Locale::En, "lsp.problems") => "Problems",
            (Locale::Es, "lsp.problems") => "Problemas",
            (Locale::En, "lsp.goto_definition") => "Go to Definition",
            (Locale::Es, "lsp.goto_definition") => "Ir a definición",
            (Locale::En, "lsp.find_references") => "Find References",
            (Locale::Es, "lsp.find_references") => "Buscar referencias",
            (Locale::En, "lsp.format_document") => "Format Document",
            (Locale::Es, "lsp.format_document") => "Formatear documento",
            (Locale::En, "lsp.hover") => "Show Hover",
            (Locale::Es, "lsp.hover") => "Mostrar documentación",

            _ => "",
        }
    }

    pub fn format_int(self, n: i64) -> String {
        match self {
            Locale::En => n.to_string(),
            Locale::Es => {
                let s = n.abs().to_string();
                let mut out = String::new();
                for (i, c) in s.chars().rev().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        out.push('.');
                    }
                    out.push(c);
                }
                let mut grouped: String = out.chars().rev().collect();
                if n < 0 {
                    grouped.insert(0, '-');
                }
                grouped
            }
        }
    }

    pub fn format_iso_date(self, y: i32, m: u8, d: u8) -> String {
        match self {
            Locale::En => format!("{y:04}-{m:02}-{d:02}"),
            Locale::Es => format!("{d:02}/{m:02}/{y:04}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spanish_and_english_catalog() {
        assert_eq!(Locale::En.t("command.palette"), "Command Palette");
        assert_eq!(Locale::Es.t("command.palette"), "Paleta de comandos");
        assert_eq!(Locale::Es.format_int(1234), "1.234");
        assert_eq!(Locale::Es.format_iso_date(2026, 9, 1), "01/09/2026");
        assert_eq!(Locale::En.format_iso_date(2026, 9, 1), "2026-09-01");
    }

    #[test]
    fn all_catalog_keys_present_in_both_languages() {
        for &key in ALL_KEYS {
            let en_val = Locale::En.t(key);
            let es_val = Locale::Es.t(key);
            assert!(!en_val.is_empty(), "Missing English translation for {key}");
            assert!(!es_val.is_empty(), "Missing Spanish translation for {key}");
        }
    }

    #[test]
    fn locale_parsing() {
        assert_eq!(Locale::parse("es-ES"), Locale::Es);
        assert_eq!(Locale::parse("es"), Locale::Es);
        assert_eq!(Locale::parse("en-US"), Locale::En);
        assert_eq!(Locale::parse("fr-FR"), Locale::En);
    }
}
