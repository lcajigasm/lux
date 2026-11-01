//! The unified Lux desktop binary — the first real assembly of
//! `lux-platform` + `lux-render` + `lux-ui` into one running application,
//! closing out Phase 2 (see `docs/receipts/phase-02-lux-app-binario-unificado.md`).

mod about;
mod accessibility;
mod app;
mod build;
mod chrome;
mod color_scheme;
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
mod overlay_motion;
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
mod snapshot;
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
    let mut open_palette = false;
    let mut open_goto = false;
    let mut open_theme = false;
    let mut query = String::new();
    let mut snapshot: Option<std::path::PathBuf> = None;
    let mut snapshot_scale = 2.0_f32;
    let mut positional: Option<std::ffi::OsString> = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let arg_str = arg.to_string_lossy();
        if arg_str == "--help" || arg_str == "-h" {
            println!("Lux — Sublime-grade high-performance text editor");
            println!();
            println!("Usage: lux [OPTIONS] [FILE_OR_DIRECTORY]");
            println!();
            println!("Options:");
            println!("  -h, --help     Print help information");
            println!("  -v, --version  Print version information");
            println!("  --palette      Open the command palette");
            println!("  --goto         Open go to file");
            println!("  --theme        Open the color scheme picker");
            println!("  --query TEXT   Prefill the open selector");
            println!("  --snapshot PATH  Write one offscreen PNG of this window");
            println!("  --snapshot-scale N  Scale factor for that PNG (default 2)");
            return;
        }
        if arg_str == "--version" || arg_str == "-v" {
            println!("lux {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        if arg_str == "--palette" {
            open_palette = true;
            continue;
        }
        if arg_str == "--goto" {
            open_goto = true;
            continue;
        }
        if arg_str == "--theme" {
            open_theme = true;
            continue;
        }
        if arg_str == "--query" {
            let Some(value) = args.next() else {
                eprintln!("Usage: lux --query TEXT");
                std::process::exit(2);
            };
            query = value.to_string_lossy().into_owned();
            continue;
        }
        if arg_str == "--snapshot" {
            let Some(value) = args.next() else {
                eprintln!("Usage: lux --snapshot PATH");
                std::process::exit(2);
            };
            snapshot = Some(std::path::PathBuf::from(value));
            continue;
        }
        if arg_str == "--snapshot-scale" {
            let Some(value) = args.next() else {
                eprintln!("Usage: lux --snapshot-scale N");
                std::process::exit(2);
            };
            snapshot_scale = value.to_string_lossy().parse().unwrap_or(2.0);
            continue;
        }
        if positional.is_some() {
            eprintln!("Usage: lux [--palette|--goto|--theme] [--query TEXT] [file]");
            std::process::exit(2);
        }
        positional = Some(arg);
    }
    if let Some(arg) = positional {
        let arg_str = arg.to_string_lossy();
        let target_path = std::path::PathBuf::from(&arg);
        match launch_kind(&arg_str, target_path.is_dir()) {
            LaunchKind::Workspace => app.open_workspace(target_path),
            LaunchKind::ProjectFile => {
                app.open_project_file(&target_path, None);
            }
            LaunchKind::Document => app.documents.initial_path = Some(target_path),
            LaunchKind::Help | LaunchKind::Version => {}
        }
    }
    if open_palette || (!open_goto && !open_theme && !query.is_empty()) {
        app.open_command_palette(&query);
    } else if open_goto {
        app.open_goto_anything(&query);
    } else if open_theme {
        app.open_theme_picker_query(&query);
    }
    if let Some(path) = snapshot {
        snapshot::request(path, snapshot_scale);
    }
    lux_platform::desktop::run(app).expect("desktop event loop failed");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchKind {
    Help,
    Version,
    Workspace,
    ProjectFile,
    Document,
}

/// `lux .` and any other directory open the workspace. Finder passes a
/// folder path the same way when the bundle's document type includes
/// `public.folder`.
fn launch_kind(arg: &str, is_dir: bool) -> LaunchKind {
    if arg == "--help" || arg == "-h" {
        LaunchKind::Help
    } else if arg == "--version" || arg == "-v" {
        LaunchKind::Version
    } else if is_dir {
        LaunchKind::Workspace
    } else if std::path::Path::new(arg)
        .extension()
        .is_some_and(|ext| ext == "sublime-project" || ext == "lux-project")
    {
        LaunchKind::ProjectFile
    } else {
        LaunchKind::Document
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_opens_a_workspace_and_flags_stay_flags() {
        assert_eq!(launch_kind(".", true), LaunchKind::Workspace);
        assert_eq!(launch_kind("/tmp/proj", true), LaunchKind::Workspace);
        assert_eq!(launch_kind("--help", true), LaunchKind::Help);
        assert_eq!(launch_kind("-v", false), LaunchKind::Version);
        assert_eq!(launch_kind("main.rs", false), LaunchKind::Document);
        assert_eq!(
            launch_kind("app.lux-project", false),
            LaunchKind::ProjectFile
        );
    }
}
