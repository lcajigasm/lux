//! The `--wait` reply stays open until the editor closes the buffer.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

struct Holding {
    #[cfg(unix)]
    streams: HashMap<u64, UnixStream>,
    #[cfg(not(unix))]
    streams: HashMap<u64, ()>,
}

fn holding() -> &'static Mutex<Holding> {
    static HOLDING: OnceLock<Mutex<Holding>> = OnceLock::new();
    HOLDING.get_or_init(|| {
        Mutex::new(Holding {
            streams: HashMap::new(),
        })
    })
}

#[cfg(unix)]
pub(crate) fn hold(token: u64, stream: UnixStream) {
    if let Ok(mut holding) = holding().lock() {
        holding.streams.insert(token, stream);
    }
}

pub(crate) fn finish(token: u64) {
    #[cfg(unix)]
    {
        if let Ok(mut holding) = holding().lock()
            && let Some(mut stream) = holding.streams.remove(&token)
        {
            use std::io::Write;
            let _ = stream.write_all(b"closed\n");
        }
    }
    #[cfg(not(unix))]
    {
        let _ = token;
    }
}

#[cfg(test)]
pub(crate) fn is_holding(token: u64) -> bool {
    holding()
        .lock()
        .map(|holding| holding.streams.contains_key(&token))
        .unwrap_or(false)
}
