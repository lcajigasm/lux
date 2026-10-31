//! The unified Lux desktop binary — the first real assembly of
//! `lux-platform` + `lux-render` + `lux-ui` into one running application,
//! closing out Phase 2 (see `docs/receipts/phase-02-lux-app-binario-unificado.md`).

mod about;
mod accessibility;
mod app;
mod build;
mod chrome;
mod conflict;
mod dialog;
mod distraction_free;
mod document;
mod find_results;
mod focus;
mod font;
mod git;
mod goto;
mod input;
mod language_picker;
mod lsp;
mod macros;
mod menu;
mod minimap;
mod modal;
mod palette;
mod pane_view;
mod plugin;
mod presentation;
mod project;
mod project_search;
mod render;
mod scale;
mod search;
mod session;
mod settings;
mod sidebar;
mod snippets;
mod split;
mod status_bar;
mod term;
mod theme_picker;
mod ui;

#[cfg(test)]
mod desktop_pass;

fn main() {
    lux_pack::install_panic_hook();
    let (path, font_bytes) = font::load_system_monospace_font().expect(
        "no known system monospace font was found — see \
         crates/lux/src/font.rs's candidate list",
    );
    println!("Lux: using system font {path}");
    let mut app = app::App::new(font_bytes);
    let mut args = std::env::args_os().skip(1);
    if let Some(arg) = args.next() {
        let arg_str = arg.to_string_lossy();
        if arg_str == "--help" || arg_str == "-h" {
            println!("Lux — Sublime-grade high-performance text editor");
            println!();
            println!("Usage: lux [OPTIONS] [FILE_OR_DIRECTORY]");
            println!();
            println!("Options:");
            println!("  -h, --help     Print help information");
            println!("  -v, --version  Print version information");
            return;
        }
        if arg_str == "--version" || arg_str == "-v" {
            println!("lux {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        let target_path = std::path::PathBuf::from(arg);
        if target_path.is_dir() {
            app.open_workspace(target_path);
        } else if target_path
            .extension()
            .is_some_and(|ext| ext == "sublime-project" || ext == "lux-project")
        {
            app.open_project_file(&target_path, None);
        } else {
            app.documents.initial_path = Some(target_path);
        }
    }
    if args.next().is_some() {
        eprintln!("Usage: lux [file_or_directory]");
        std::process::exit(2);
    }
    lux_platform::desktop::run(app).expect("desktop event loop failed");
}
