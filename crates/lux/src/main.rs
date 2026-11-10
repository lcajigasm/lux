//! The unified Lux desktop binary — the first real assembly of
//! `lux-platform` + `lux-render` + `lux-ui` into one running application,
//! closing out Phase 2 (see `docs/receipts/phase-02-lux-app-binario-unificado.md`).

mod about;
mod accessibility;
mod app;
mod build;
mod chrome;
mod cli;
mod color_scheme;
mod conflict;
mod dialog;
mod distraction_free;
mod document;
mod field;
mod find_results;
mod focus;
mod font;
mod git;
mod goto;
mod ime_hold;
mod input;
mod language_picker;
mod lsp;
mod macros;
mod markdown_preview;
mod menu;
mod minimap;
mod modal;
mod overlay_motion;
mod overlay_wheel;
mod palette;
mod pane_view;
mod place;
mod plugin;
mod presentation;
mod probe;
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
    probe::mark_process_start();
    lux_pack::install_panic_hook();
    if std::env::args_os()
        .skip(1)
        .any(|arg| arg == "--help" || arg == "-h")
    {
        print_help();
        return;
    }
    if std::env::args_os()
        .skip(1)
        .any(|arg| arg == "--version" || arg == "-v")
    {
        println!("lux {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let (path, font_bytes) = font::load_system_monospace_font().expect(
        "no known system monospace font was found — see \
         crates/lux/src/font.rs's candidate list",
    );
    println!("Lux: using system font {path}");
    let mut app = app::App::new(font_bytes);
    let mut open_palette = false;
    let mut open_goto = false;
    let mut open_theme = false;
    let mut open_settings = false;
    let mut query = String::new();
    let mut snapshot: Option<std::path::PathBuf> = None;
    let mut snapshot_on_usr1: Vec<std::path::PathBuf> = Vec::new();
    let mut snapshot_scale = 2.0_f32;
    let mut wait = false;
    let mut new_window = false;
    let mut probe: Option<std::path::PathBuf> = None;
    let mut bench_open: Option<std::path::PathBuf> = None;
    let mut positionals: Vec<std::path::PathBuf> = Vec::new();
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let arg_str = arg.to_string_lossy();
        if arg_str == "--help" || arg_str == "-h" {
            print_help();
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
        if arg_str == "--settings" {
            open_settings = true;
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
        if arg_str == "--snapshot-on-usr1" {
            let Some(value) = args.next() else {
                eprintln!("Usage: lux --snapshot-on-usr1 PATH");
                std::process::exit(2);
            };
            snapshot_on_usr1.push(std::path::PathBuf::from(value));
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
        if arg_str == "--probe" {
            let Some(value) = args.next() else {
                eprintln!("Usage: lux --probe PATH");
                std::process::exit(2);
            };
            probe = Some(std::path::PathBuf::from(value));
            continue;
        }
        if arg_str == "--wait" {
            wait = true;
            continue;
        }
        if arg_str == "--new-window" {
            new_window = true;
            continue;
        }
        if arg_str == "--bench-open" {
            let Some(value) = args.next() else {
                eprintln!("Usage: lux --bench-open PATH");
                std::process::exit(2);
            };
            bench_open = Some(std::path::PathBuf::from(value));
            continue;
        }
        positionals.push(std::path::PathBuf::from(arg));
    }
    let scratch = if positionals.iter().any(|path| path.as_os_str() == "-") {
        Some(cli::read_stdin_scratch().unwrap_or_else(|message| {
            eprintln!("{message}");
            std::process::exit(1);
        }))
    } else {
        None
    };
    let (paths, mut locations) = cli::prepare(&positionals, scratch).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    let wait_token = if wait { cli::fresh_token() } else { 0 };
    if wait_token != 0 {
        for location in &mut locations {
            location.wait_token = wait_token;
        }
    }
    let session = cli::Session {
        paths: paths.clone(),
        locations,
        palette: open_palette,
        goto: open_goto,
        theme: open_theme,
        settings: open_settings,
        query: query.clone(),
        new_window,
        wait_token,
    };
    let private_process = snapshot.is_some()
        || !snapshot_on_usr1.is_empty()
        || probe.is_some()
        || bench_open.is_some();
    match cli::startup(&session, private_process) {
        Ok(cli::Startup::Done) => return,
        Ok(cli::Startup::Run) => {}
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
    cli::register_locations(&session.locations);
    if let Err(message) = apply_positionals(&mut app, &paths) {
        eprintln!("{message}");
        std::process::exit(2);
    }
    cli::open_requested_surface(&mut app, &session);
    if !snapshot_on_usr1.is_empty() {
        snapshot::listen_usr1(snapshot_on_usr1, snapshot_scale);
    } else if let Some(path) = snapshot {
        snapshot::request(path, snapshot_scale);
    }
    probe::configure(probe, bench_open);
    probe::listen();
    lux_platform::desktop::run(app).expect("desktop event loop failed");
}

fn print_help() {
    println!("{}", cli::help_text());
    println!("  --snapshot PATH  Write one offscreen PNG of this window");
    println!("  --snapshot-on-usr1 PATH  Write that PNG when SIGUSR1 arrives");
    println!("  --snapshot-scale N  Scale factor for that PNG (default 2)");
    println!("  --probe PATH  Write editor state when SIGUSR2 arrives");
    println!("  --bench-open PATH  Open that file on the first SIGUSR2");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchKind {
    Help,
    Version,
    Workspace,
    ProjectFile,
    Document,
}

/// The first directory opens the workspace. Later paths are files, each
/// opened as its own tab. A project file is only recognized when it is
/// the sole argument.
fn apply_positionals(app: &mut app::App, paths: &[std::path::PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    if paths.len() == 1 {
        let path = &paths[0];
        let arg = path.to_string_lossy();
        return match launch_kind(&arg, path.is_dir()) {
            LaunchKind::Workspace => {
                app.open_workspace(path.clone());
                Ok(())
            }
            LaunchKind::ProjectFile => {
                app.open_project_file(path, None);
                Ok(())
            }
            LaunchKind::Document => {
                app.documents.initial_path = Some(path.clone());
                Ok(())
            }
            LaunchKind::Help | LaunchKind::Version => Ok(()),
        };
    }
    let mut files = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        if path.is_dir() {
            if index != 0 {
                return Err("only the first path may be a directory".into());
            }
            app.open_workspace(path.clone());
        } else {
            files.push(path.clone());
        }
    }
    if let Some(first) = files.first().cloned() {
        app.documents.initial_path = Some(first);
        app.documents.open_queue.extend(files.into_iter().skip(1));
    }
    Ok(())
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

    #[test]
    fn two_files_queue_a_second_tab_and_a_folder_opens_the_tree() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("monokai.rs");
        let second = dir.path().join("characters.txt");
        std::fs::write(&first, "fn main() {}\n").unwrap();
        std::fs::write(&second, "note\n").unwrap();
        let font = include_bytes!("../../lux-render/tests/fixtures/ahem.ttf").to_vec();
        let mut app = app::App::new(font.clone());
        apply_positionals(&mut app, &[first.clone(), second.clone()]).unwrap();
        assert_eq!(app.documents.initial_path.as_ref(), Some(&first));
        assert_eq!(app.documents.open_queue, vec![second.clone()]);
        assert!(app.ui.sidebar.is_none());

        let mut app = app::App::new(font);
        apply_positionals(&mut app, &[dir.path().to_path_buf(), first.clone()]).unwrap();
        assert!(app.ui.sidebar.as_ref().is_some_and(|s| s.tree.is_some()));
        assert_eq!(app.documents.initial_path.as_ref(), Some(&first));
    }
}
