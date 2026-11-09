//! The language-server client needs a Tokio reactor.
//!
//! The editor starts a server from an ordinary thread (`lux-lsp-spawn`).
//! `tokio::spawn` there panics with "there is no reactor running". Tests
//! already run inside a runtime, so those keep using it. A plain thread
//! gets one process-wide runtime instead.

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::{Builder, Handle, Runtime};
use tokio::task::JoinHandle;

pub(crate) fn spawn<F>(future: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle().spawn(future)
}

/// Drive `future` on the shared runtime. The caller must not already be
/// inside a Tokio runtime.
pub fn block_on<F: Future>(future: F) -> F::Output {
    shared().block_on(future)
}

fn handle() -> Handle {
    if let Ok(current) = Handle::try_current() {
        return current;
    }
    shared().handle().clone()
}

fn shared() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        Builder::new_multi_thread()
            .thread_name("lux-lsp")
            .enable_all()
            .build()
            .expect("start the language server runtime")
    })
}

#[cfg(test)]
mod tests {
    use super::{block_on, spawn};

    #[test]
    fn a_plain_thread_spawns_on_the_lsp_runtime() {
        let value = std::thread::spawn(|| {
            let task = spawn(async { 41 });
            block_on(task).unwrap()
        })
        .join()
        .expect("lsp thread panicked");
        assert_eq!(value, 41);
    }
}
