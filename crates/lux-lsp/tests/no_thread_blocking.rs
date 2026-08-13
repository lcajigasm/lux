//! Phase 7's acceptance criteria: "ninguna operación LSP bloquea el hilo
//! de UI, verificado con el monitor de frame time." `lux-lsp` has no UI
//! thread of its own to measure -- there is no `lux-ui`/`lux` wiring yet
//! (every Phase 7 receipt says so), so the literal criterion (a real
//! frame-time monitor inside a running app) genuinely can't be checked
//! from this crate. What *can* be checked, honestly, is the mechanism
//! golden rule 6 ("no I/O on the UI thread, ever") and this criterion
//! both depend on: that a slow, in-flight LSP request never blocks other
//! concurrent async work sharing its executor thread. If that held,
//! wiring this client into a real single-threaded UI event loop later
//! would not need a fix here to keep frames flowing during a slow
//! request; if it didn't, no amount of `lux-ui` wiring could fix it
//! either -- the block would be in this crate. This test proves the
//! premise, not the full end-to-end criterion.
//!
//! Runs on a `current_thread` runtime specifically -- the same shape a
//! single UI thread driving both rendering and async LSP calls would
//! have. A multi-thread runtime would hide a real blocking bug by simply
//! running the "frame" ticks on a different OS thread.

mod blocking {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{Duration, Instant};

    use lux_lsp::{DocumentSync, Hover, ServerSession, ServerSpec, ServerStatus};

    const FAKE_SERVER_SPEC: ServerSpec = ServerSpec {
        language_id: "fake",
        command: env!("CARGO_BIN_EXE_lux-lsp-fake-server"),
        args: &[],
    };

    #[tokio::test(flavor = "current_thread")]
    async fn a_slow_hover_request_does_not_stall_a_concurrent_tick_loop() {
        let dir = tempfile::tempdir().expect("create temp dir");
        // Comfortably longer than the tick budget below, so a real stall
        // would be unmistakable rather than a coin flip against scheduler
        // jitter.
        std::fs::write(dir.path().join("fake-lsp-hover-delay-ms"), "1000").unwrap();

        let session = Arc::new(ServerSession::spawn(
            FAKE_SERVER_SPEC,
            dir.path().to_path_buf(),
        ));
        tokio::time::timeout(Duration::from_secs(30), async {
            let mut rx = session.watch();
            loop {
                if matches!(rx.borrow().status, ServerStatus::Running) {
                    return;
                }
                rx.changed().await.unwrap();
            }
        })
        .await
        .expect("fake server should reach Running");

        let sync = DocumentSync::new(session.clone());
        let hover = Hover::new(session.clone());
        let uri = async_lsp::lsp_types::Url::from_file_path(
            dir.path().canonicalize().unwrap().join("fixture.txt"),
        )
        .unwrap();
        sync.open(uri.clone(), "fake", "hello\n".to_string())
            .expect("didOpen should succeed");

        // A tokio task on the *same* current-thread runtime, ticking every
        // 5ms -- this is what starves if the hover request below blocks
        // the thread instead of yielding while it waits on real I/O.
        let max_gap_ms = Arc::new(AtomicU32::new(0));
        let ticking = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let tick_task = tokio::spawn({
            let max_gap_ms = max_gap_ms.clone();
            let ticking = ticking.clone();
            async move {
                let mut last = Instant::now();
                while ticking.load(Ordering::Relaxed) {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    let now = Instant::now();
                    let gap = now.duration_since(last).as_millis() as u32;
                    max_gap_ms.fetch_max(gap, Ordering::Relaxed);
                    last = now;
                }
            }
        });

        let position = async_lsp::lsp_types::Position {
            line: 0,
            character: 0,
        };
        hover
            .hover(&uri, position)
            .await
            .expect("hover request itself should succeed")
            .expect("fake server always returns a hover result");

        ticking.store(false, Ordering::Relaxed);
        tick_task.await.unwrap();

        let max_gap = max_gap_ms.load(Ordering::Relaxed);
        assert!(
            max_gap < 200,
            "tick loop stalled for {max_gap}ms while a slow hover request was in flight -- \
             the request blocked the executor thread instead of yielding on real I/O"
        );

        session.stop().await;
    }
}
