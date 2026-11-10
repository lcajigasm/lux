//! Unix socket between a second `lux` command and the running editor.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use super::{Session, Startup, wait};

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
    if private_process {
        return Ok(Startup::Run);
    }
    if std::env::var_os("LUX_SOCKET").is_some() || session.new_window {
        if session.new_window && std::env::var_os("LUX_SOCKET").is_none() {
            let path = unique_socket();
            // SAFETY: this runs before threads besides the runtime exist,
            // and the socket path is read by this process only.
            unsafe { std::env::set_var("LUX_SOCKET", &path) };
        }
        listen_default()?;
        return Ok(Startup::Run);
    }
    if bundle_cli_invocation() {
        return bundle_client(session);
    }
    match deliver_default(session) {
        Deliver::Accepted => Ok(Startup::Done),
        Deliver::Absent => listen_or_deliver(session),
        Deliver::Failed(message) => Err(message),
    }
}

fn bundle_client(session: &Session) -> Result<Startup, String> {
    if session.new_window {
        let socket = unique_socket();
        spawn_bundle_editor(Some(&socket), session.wait_token == 0)?;
        if session.wait_token != 0 {
            deliver_when_ready(&socket, session)?;
        }
        return Ok(Startup::Done);
    }
    match deliver_default(session) {
        Deliver::Accepted => Ok(Startup::Done),
        Deliver::Absent => {
            if session.wait_token != 0 {
                spawn_bundle_editor(None, false)?;
                deliver_when_ready(&socket_path(), session)?;
            } else {
                spawn_bundle_editor(None, true)?;
            }
            Ok(Startup::Done)
        }
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
    super::is_bundle_cli(&canonical)
}

fn socket_path() -> PathBuf {
    if let Some(path) = std::env::var_os("LUX_SOCKET") {
        return PathBuf::from(path);
    }
    let mut path = std::env::temp_dir();
    path.push(format!("lux-{}.sock", unix_uid()));
    path
}

fn unique_socket() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "lux-{}-{}-{}.sock",
        unix_uid(),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
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

fn deliver_when_ready(path: &Path, session: &Session) -> Result<(), String> {
    for _ in 0..100 {
        if socket_is_live(path) {
            return deliver(path, session);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err("Lux did not open its command socket".into())
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
        let reply = match read_session(&mut stream) {
            Ok(session) => {
                let token = session.wait_token;
                hub.push(session);
                let _ = stream.write_all(b"ok\n");
                if token != 0 {
                    let _ = stream.set_read_timeout(None);
                    let _ = stream.set_write_timeout(None);
                    wait::hold(token, stream);
                }
                continue;
            }
            Err(_) => "err\n",
        };
        let _ = stream.write_all(reply.as_bytes());
    }
}

fn read_session(stream: &mut UnixStream) -> Result<Session, ()> {
    let line = read_line(stream).map_err(|_| ())?;
    serde_json::from_slice(&line).map_err(|_| ())
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
    let reply = read_line(&mut stream)?;
    if reply != b"ok" {
        return Err(String::from_utf8_lossy(&reply).into_owned());
    }
    if session.wait_token != 0 {
        let _ = stream.set_read_timeout(None);
        let end = read_line(&mut stream).unwrap_or_default();
        if !end.is_empty() && end != b"closed" {
            return Err(String::from_utf8_lossy(&end).into_owned());
        }
    }
    Ok(())
}

fn read_line(stream: &mut UnixStream) -> Result<Vec<u8>, String> {
    let mut got = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if got.len() > 65_536 {
            return Err("command line is too long".into());
        }
        match stream.read(&mut byte) {
            Ok(0) => break,
            Ok(_) if byte[0] == b'\n' => break,
            Ok(_) => got.push(byte[0]),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(got)
}

fn spawn_bundle_editor(socket: Option<&Path>, forward_args: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let gui = super::bundle_gui(&exe.canonicalize().unwrap_or(exe))
        .ok_or_else(|| "Lux.app has no Contents/MacOS/lux".to_string())?;
    if !gui.is_file() {
        return Err(format!("missing {}", gui.display()));
    }
    let mut command = std::process::Command::new(gui);
    if let Some(socket) = socket {
        command.env("LUX_SOCKET", socket);
    }
    if forward_args {
        command.args(std::env::args_os().skip(1));
    }
    command.stdin(std::process::Stdio::null());
    command.process_group(0);
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}
