//! The `lux` command. A second invocation hands its paths to the editor
//! that is already running and exits. The macOS bundle ships that command
//! at `Contents/SharedSupport/bin/lux`; when no editor is running it starts
//! `Contents/MacOS/lux` and returns.
//!
//! `--wait` keeps the client until those buffers close. `--new-window`
//! starts another window. `-` reads stdin into an untitled buffer.
//! `file:line:col` places the cursor. The Tools menu and the palette
//! install or remove the PATH link.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::app::App;

mod install;
mod opens;
mod parse;
#[cfg(unix)]
mod unix;
mod wait;

pub(crate) use parse::help_text;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct Location {
    pub(crate) path: PathBuf,
    #[serde(default)]
    pub(crate) line: Option<u32>,
    #[serde(default)]
    pub(crate) column: Option<u32>,
    #[serde(default)]
    pub(crate) scratch: bool,
    #[serde(default)]
    pub(crate) wait_token: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct Session {
    pub(crate) paths: Vec<PathBuf>,
    #[serde(default)]
    pub(crate) locations: Vec<Location>,
    pub(crate) palette: bool,
    pub(crate) goto: bool,
    pub(crate) theme: bool,
    pub(crate) settings: bool,
    pub(crate) query: String,
    #[serde(default)]
    pub(crate) new_window: bool,
    /// Non-zero while a client is blocked in `--wait`.
    #[serde(default)]
    pub(crate) wait_token: u64,
}

pub(crate) enum Startup {
    /// This process is the editor.
    Run,
    /// The command finished. The editor is the other process.
    Done,
}

pub(crate) fn startup(session: &Session, private_process: bool) -> Result<Startup, String> {
    #[cfg(unix)]
    {
        unix::startup(session, private_process)
    }
    #[cfg(not(unix))]
    {
        let _ = (session, private_process);
        if session.wait_token != 0 {
            return Err("--wait is available on macOS and Linux".into());
        }
        Ok(Startup::Run)
    }
}

pub(crate) fn register_waker(waker: Arc<dyn Fn() + Send + Sync>) {
    #[cfg(unix)]
    unix::global_hub().set_waker(waker);
    #[cfg(not(unix))]
    let _ = waker;
}

pub(crate) fn dispatch(app: &mut App, ctx: &dyn lux_platform::PlatformContext) {
    #[cfg(unix)]
    {
        for session in unix::global_hub().take() {
            stage(app, Some(ctx), &session);
        }
    }
    #[cfg(not(unix))]
    let _ = (app, ctx);
}

pub(crate) fn prepare(
    args: &[PathBuf],
    scratch: Option<PathBuf>,
) -> Result<(Vec<PathBuf>, Vec<Location>), String> {
    parse::prepare(args, scratch)
}

pub(crate) fn read_stdin_scratch() -> Result<PathBuf, String> {
    parse::read_stdin_scratch()
}

pub(crate) fn register_locations(locations: &[Location]) {
    for location in locations {
        opens::register(location);
    }
}

pub(crate) fn note_open_failed(request: &crate::document::Request) {
    opens::note_open_failed(request);
}

pub(crate) fn apply_opened(app: &mut App) {
    opens::apply_opened(app);
}

pub(crate) fn release_tab(tab: lux_ui::tabs::TabId) {
    opens::release_tab(tab);
}

pub(crate) fn release_all() {
    opens::release_all();
}

pub(crate) fn install_command() -> Result<String, String> {
    install::install_command()
}

pub(crate) fn uninstall_command() -> Result<String, String> {
    install::uninstall_command()
}

pub(crate) fn open_requested_surface(app: &mut App, session: &Session) {
    if session.palette
        || (!session.goto && !session.theme && !session.settings && !session.query.is_empty())
    {
        app.open_command_palette(&session.query);
    } else if session.goto {
        app.open_goto_anything(&session.query);
    } else if session.theme {
        app.open_theme_picker_query(&session.query);
    } else if session.settings && app.ui.settings.panel.is_none() {
        app.ui.settings.open_settings_ui();
    }
}

pub(crate) fn stage(
    app: &mut App,
    ctx: Option<&dyn lux_platform::PlatformContext>,
    session: &Session,
) {
    let locations = if session.locations.is_empty() {
        session
            .paths
            .iter()
            .cloned()
            .map(|path| Location {
                path,
                wait_token: session.wait_token,
                ..Location::default()
            })
            .collect::<Vec<_>>()
    } else {
        session.locations.clone()
    };
    for location in &locations {
        opens::register(location);
        app.documents.open_queue.push(location.path.clone());
    }
    if let Some(ctx) = ctx
        && !app.documents.open_queue.is_empty()
    {
        crate::document::open_queue::pump(app, ctx);
    }
    opens::settle(session.wait_token);
    open_requested_surface(app, session);
    if let Some(gpu) = &app.gpu_window {
        gpu.window.focus();
        gpu.window.request_redraw();
    }
}

pub(crate) fn is_bundle_cli(path: &Path) -> bool {
    let parts: Vec<_> = path.components().map(|c| c.as_os_str()).collect();
    parts.len() >= 4
        && parts[parts.len() - 4] == "Contents"
        && parts[parts.len() - 3] == "SharedSupport"
        && parts[parts.len() - 2] == "bin"
        && parts[parts.len() - 1] == "lux"
}

pub(crate) fn bundle_gui(cli: &Path) -> Option<PathBuf> {
    if !is_bundle_cli(cli) {
        return None;
    }
    Some(cli.parent()?.parent()?.parent()?.join("MacOS").join("lux"))
}

pub(crate) fn fresh_token() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(1)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_support_bin_is_the_command_and_points_at_the_app() {
        let cli = Path::new("/Applications/Lux.app/Contents/SharedSupport/bin/lux");
        let gui = Path::new("/Applications/Lux.app/Contents/MacOS/lux");
        assert!(is_bundle_cli(cli));
        assert!(!is_bundle_cli(gui));
        assert_eq!(bundle_gui(cli).as_deref(), Some(gui));
        assert!(bundle_gui(gui).is_none());
    }

    #[test]
    fn a_query_without_a_selector_opens_the_palette() {
        let (_path, font_bytes) = crate::font::load_system_monospace_font().expect("font");
        let mut app = App::new(font_bytes);
        let session = Session {
            query: "sel".into(),
            ..Session::default()
        };
        stage(&mut app, None, &session);
        assert!(app.ui.command_palette.is_some());
        assert!(app.ui.settings.panel.is_none());
    }

    #[test]
    fn a_location_places_the_cursor_and_stdin_drops_the_path() {
        let (_path, font_bytes) = crate::font::load_system_monospace_font().expect("font");
        let mut app = App::new(font_bytes);
        let file = PathBuf::from("/tmp/lux-cli-jump.txt");
        app.buffer = lux_core::buffer::Buffer::from_bytes(b"alpha\nbeta\ngamma\n").unwrap();
        app.documents.path = Some(file.clone());
        register_locations(&[Location {
            path: file,
            line: Some(2),
            column: Some(2),
            ..Location::default()
        }]);
        apply_opened(&mut app);
        let head = app.cursors.primary().head;
        assert_eq!((head.line, head.column), (1, 1));

        let scratch = PathBuf::from("/tmp/lux-cli-scratch.txt");
        std::fs::write(&scratch, b"from stdin\n").unwrap();
        app.documents.path = Some(scratch.clone());
        app.buffer = lux_core::buffer::Buffer::from_bytes(b"from stdin\n").unwrap();
        register_locations(&[Location {
            path: scratch,
            scratch: true,
            ..Location::default()
        }]);
        apply_opened(&mut app);
        assert!(app.documents.path.is_none());
        assert!(app.document_dirty());
        assert_eq!(app.buffer.text(), "from stdin\n");
    }

    #[test]
    fn paths_wait_in_the_open_queue_until_the_editor_pumps_them() {
        let (_path, font_bytes) = crate::font::load_system_monospace_font().expect("font");
        let mut app = App::new(font_bytes);
        let file = PathBuf::from("/tmp/lux-cli-note.txt");
        let session = Session {
            paths: vec![file.clone()],
            ..Session::default()
        };
        stage(&mut app, None, &session);
        assert_eq!(app.documents.open_queue, vec![file]);
    }

    #[cfg(unix)]
    #[test]
    fn a_second_command_delivers_its_session_to_the_listener() {
        let dir = tempfile::tempdir().expect("temp");
        let sock = dir.path().join("gui.sock");
        drop(std::os::unix::net::UnixListener::bind(&sock).expect("stale"));
        let hub = Box::leak(Box::new(super::unix::Hub::new()));
        super::unix::serve(&sock, hub).expect("serve");
        let session = Session {
            paths: vec![PathBuf::from("src/main.rs")],
            palette: true,
            ..Session::default()
        };
        super::unix::deliver(&sock, &session).expect("deliver");
        assert_eq!(hub.take(), vec![session]);
        std::mem::forget(dir);
    }

    #[cfg(unix)]
    #[test]
    fn wait_blocks_until_the_editor_closes_the_buffer() {
        let dir = tempfile::tempdir().expect("temp");
        let sock = dir.path().join("wait.sock");
        let hub = Box::leak(Box::new(super::unix::Hub::new()));
        super::unix::serve(&sock, hub).expect("serve");
        let token = 42;
        let session = Session {
            paths: vec![PathBuf::from("note.txt")],
            wait_token: token,
            ..Session::default()
        };
        let sock2 = sock.clone();
        let client = std::thread::spawn(move || super::unix::deliver(&sock2, &session));
        for _ in 0..50 {
            if super::wait::is_holding(token) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(super::wait::is_holding(token));
        super::wait::finish(token);
        client.join().expect("client").expect("deliver");
        assert_eq!(
            hub.take(),
            vec![Session {
                paths: vec![PathBuf::from("note.txt")],
                wait_token: token,
                ..Session::default()
            }]
        );
        std::mem::forget(dir);
    }
}
