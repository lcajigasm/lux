//! The `lux` command. A second invocation hands its paths to the editor
//! that is already running and exits. The macOS bundle ships that command
//! at `Contents/SharedSupport/bin/lux`; when no editor is running it starts
//! `Contents/MacOS/lux` and returns.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::app::App;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct Session {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) palette: bool,
    pub(crate) goto: bool,
    pub(crate) theme: bool,
    pub(crate) settings: bool,
    pub(crate) query: String,
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
    for path in &session.paths {
        app.documents.open_queue.push(path.clone());
    }
    if let Some(ctx) = ctx
        && !app.documents.open_queue.is_empty()
    {
        crate::document::open_queue::pump(app, ctx);
    }
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

#[cfg(unix)]
mod unix {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::os::unix::process::CommandExt;
    use std::time::Duration;

    pub(super) struct Hub {
        queue: Mutex<Vec<Session>>,
        waker: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    }

    impl Hub {
        pub(super) fn new() -> Self {
            Hub {
                queue: Mutex::new(Vec::new()),
                waker: Mutex::new(None),
            }
        }

        fn push(&self, session: Session) {
            if let Ok(mut queue) = self.queue.lock() {
                queue.push(session);
            }
            self.wake();
        }

        pub(super) fn take(&self) -> Vec<Session> {
            self.queue
                .lock()
                .map(|mut queue| std::mem::take(&mut *queue))
                .unwrap_or_default()
        }

        fn wake(&self) {
            if let Ok(slot) = self.waker.lock()
                && let Some(waker) = slot.as_ref()
            {
                waker();
            }
        }

        pub(super) fn set_waker(&self, waker: Arc<dyn Fn() + Send + Sync>) {
            let pending = self
                .queue
                .lock()
                .map(|queue| !queue.is_empty())
                .unwrap_or(false);
            if let Ok(mut slot) = self.waker.lock() {
                *slot = Some(waker);
            }
            if pending {
                self.wake();
            }
        }
    }

    pub(super) fn global_hub() -> &'static Hub {
        static HUB: OnceLock<Hub> = OnceLock::new();
        HUB.get_or_init(Hub::new)
    }

    enum Deliver {
        Accepted,
        Absent,
        Failed(String),
    }

    pub(super) fn startup(session: &Session, private_process: bool) -> Result<Startup, String> {
        if bundle_cli_invocation() {
            return match deliver_default(session) {
                Deliver::Accepted => Ok(Startup::Done),
                Deliver::Absent => {
                    spawn_bundle_editor()?;
                    Ok(Startup::Done)
                }
                Deliver::Failed(message) => Err(message),
            };
        }
        if private_process {
            return Ok(Startup::Run);
        }
        match deliver_default(session) {
            Deliver::Accepted => Ok(Startup::Done),
            Deliver::Absent => listen_or_deliver(session),
            Deliver::Failed(message) => Err(message),
        }
    }

    fn listen_or_deliver(session: &Session) -> Result<Startup, String> {
        match listen_default() {
            Ok(()) => Ok(Startup::Run),
            Err(_) => match deliver_default(session) {
                Deliver::Accepted => Ok(Startup::Done),
                Deliver::Absent => Err("Lux is already starting".into()),
                Deliver::Failed(message) => Err(message),
            },
        }
    }

    fn bundle_cli_invocation() -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let canonical = exe.canonicalize().unwrap_or(exe);
        is_bundle_cli(&canonical)
    }

    fn socket_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("lux-{}.sock", unix_uid()));
        path
    }

    fn unix_uid() -> u32 {
        unsafe extern "C" {
            safe fn getuid() -> u32;
        }
        getuid()
    }

    fn deliver_default(session: &Session) -> Deliver {
        let path = socket_path();
        if !socket_is_live(&path) {
            return Deliver::Absent;
        }
        match deliver(&path, session) {
            Ok(()) => Deliver::Accepted,
            Err(message) => Deliver::Failed(message),
        }
    }

    fn listen_default() -> Result<(), String> {
        static STARTED: OnceLock<()> = OnceLock::new();
        if STARTED.get().is_some() {
            return Ok(());
        }
        serve(&socket_path(), global_hub())?;
        let _ = STARTED.set(());
        Ok(())
    }

    pub(super) fn serve(path: &Path, hub: &'static Hub) -> Result<(), String> {
        let listener = bind_listener(path)?;
        std::thread::Builder::new()
            .name("lux-cli".into())
            .spawn(move || accept_loop(listener, hub))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn bind_listener(path: &Path) -> Result<UnixListener, String> {
        match UnixListener::bind(path) {
            Ok(listener) => {
                restrict(path);
                Ok(listener)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                if socket_is_live(path) {
                    return Err("Lux is already running".into());
                }
                let _ = std::fs::remove_file(path);
                let listener = UnixListener::bind(path).map_err(|error| error.to_string())?;
                restrict(path);
                Ok(listener)
            }
            Err(error) => Err(error.to_string()),
        }
    }

    fn restrict(path: &Path) {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }

    fn socket_is_live(path: &Path) -> bool {
        UnixStream::connect(path).is_ok()
    }

    fn accept_loop(listener: UnixListener, hub: &'static Hub) {
        while let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let reply = match read_session(&mut stream) {
                Ok(session) => {
                    hub.push(session);
                    "ok\n"
                }
                Err(_) => "err\n",
            };
            let _ = stream.write_all(reply.as_bytes());
        }
    }

    fn read_session(stream: &mut UnixStream) -> Result<Session, ()> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if buf.len() >= 65_536 {
                return Err(());
            }
            match stream.read(&mut byte) {
                Ok(0) => return Err(()),
                Ok(_) if byte[0] == b'\n' => break,
                Ok(_) => buf.push(byte[0]),
                Err(_) => return Err(()),
            }
        }
        serde_json::from_slice(&buf).map_err(|_| ())
    }

    pub(super) fn deliver(path: &Path, session: &Session) -> Result<(), String> {
        let mut stream = UnixStream::connect(path).map_err(|error| error.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| error.to_string())?;
        let mut line = serde_json::to_string(session).map_err(|error| error.to_string())?;
        line.push('\n');
        stream
            .write_all(line.as_bytes())
            .map_err(|error| error.to_string())?;
        let mut got = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            if got.len() > 16 {
                return Err("bad reply".into());
            }
            match stream.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => {
                    got.push(byte[0]);
                    if byte[0] == b'\n' {
                        break;
                    }
                }
                Err(error) => return Err(error.to_string()),
            }
        }
        if got == b"ok\n" {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&got).into_owned())
        }
    }

    fn spawn_bundle_editor() -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        let gui = bundle_gui(&exe.canonicalize().unwrap_or(exe))
            .ok_or_else(|| "Lux.app has no Contents/MacOS/lux".to_string())?;
        if !gui.is_file() {
            return Err(format!("missing {}", gui.display()));
        }
        let mut command = std::process::Command::new(gui);
        command.args(std::env::args_os().skip(1));
        command.stdin(std::process::Stdio::null());
        command.process_group(0);
        command
            .spawn()
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
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
}
