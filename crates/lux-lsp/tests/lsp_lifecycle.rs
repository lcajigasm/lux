//! Integration tests for server-management lifecycle, run against a real
//! `rust-analyzer` on `PATH` -- no fake/mock language server. Slow: each
//! test spawns a real process and waits for a real capability handshake.

mod session {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use lux_lsp::{ServerRegistry, ServerSession, ServerState, ServerStatus};

    fn rust_project_root() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/lib.rs"),
            "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
        )
        .unwrap();
        dir
    }

    /// Wait (event-driven, via the state `watch` channel) until `predicate`
    /// holds, or panic after `budget`.
    async fn wait_for(
        session: &ServerSession,
        mut predicate: impl FnMut(&ServerState) -> bool,
        budget: Duration,
    ) -> ServerState {
        tokio::time::timeout(budget, async {
            let mut rx = session.watch();
            loop {
                {
                    let state = rx.borrow();
                    if predicate(&state) {
                        return state.clone();
                    }
                }
                rx.changed()
                    .await
                    .expect("session state channel closed unexpectedly");
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for expected session state"))
    }

    async fn start_rust_session(root: &Path) -> (ServerRegistry, Arc<ServerSession>) {
        let registry = ServerRegistry::new();
        let session = registry
            .get_or_start("rust", root)
            .await
            .expect("rust-analyzer should be discoverable on PATH");
        (registry, session)
    }

    #[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
    #[tokio::test]
    async fn discovers_starts_and_completes_the_capability_handshake() {
        let root = rust_project_root();
        let (_registry, session) = start_rust_session(root.path()).await;

        let state = wait_for(
            &session,
            |s| {
                matches!(
                    s.status,
                    ServerStatus::Running | ServerStatus::Failed { .. }
                )
            },
            Duration::from_secs(60),
        )
        .await;

        assert_eq!(
            state.status,
            ServerStatus::Running,
            "rust-analyzer did not reach Running"
        );
        let capabilities = state
            .capabilities
            .expect("capabilities must be recorded after initialize");
        assert!(
            capabilities.hover_provider.is_some(),
            "expected rust-analyzer to advertise hover support"
        );
        assert!(
            state.pid.is_some(),
            "a running session should expose the server's pid"
        );

        session.stop().await;
    }

    #[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
    #[tokio::test]
    async fn reports_dollar_progress_while_starting_up() {
        let root = rust_project_root();
        let (_registry, session) = start_rust_session(root.path()).await;

        let state = wait_for(
            &session,
            |s| s.progress_events_seen > 0 || matches!(s.status, ServerStatus::Failed { .. }),
            Duration::from_secs(60),
        )
        .await;

        assert!(
            state.progress_events_seen > 0,
            "expected at least one $/progress notification"
        );

        session.stop().await;
    }

    #[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
    #[tokio::test]
    async fn clean_shutdown_stops_the_session() {
        let root = rust_project_root();
        let (_registry, session) = start_rust_session(root.path()).await;
        wait_for(
            &session,
            |s| matches!(s.status, ServerStatus::Running),
            Duration::from_secs(60),
        )
        .await;

        session.stop().await;
        assert_eq!(session.status(), ServerStatus::Stopped);

        // Idempotent: stopping an already-stopped session does not hang or error.
        session.stop().await;
        assert_eq!(session.status(), ServerStatus::Stopped);
    }

    #[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
    #[tokio::test]
    async fn one_server_per_language_and_root() {
        let root = rust_project_root();
        let other_root = rust_project_root();
        let registry = ServerRegistry::new();

        let a = registry.get_or_start("rust", root.path()).await.unwrap();
        let b = registry.get_or_start("rust", root.path()).await.unwrap();
        assert!(
            Arc::ptr_eq(&a, &b),
            "the same language+root must reuse the same session"
        );

        let c = registry
            .get_or_start("rust", other_root.path())
            .await
            .unwrap();
        assert!(
            !Arc::ptr_eq(&a, &c),
            "a distinct root must get a distinct session"
        );

        registry.stop_all().await;
    }

    #[ignore = "requires a real language server on PATH -- not installed in CI, run manually"]
    #[tokio::test]
    async fn crash_triggers_automatic_restart() {
        let root = rust_project_root();
        let (_registry, session) = start_rust_session(root.path()).await;
        let running = wait_for(
            &session,
            |s| matches!(s.status, ServerStatus::Running),
            Duration::from_secs(60),
        )
        .await;
        let pid = running.pid.expect("running session should expose a pid");

        // Kill the real process out from under the session, exactly like an
        // OS-level crash would -- not a simulated failure.
        let killed = unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) };
        assert_eq!(killed, 0, "failed to signal rust-analyzer (pid {pid})");

        let restarted = wait_for(
            &session,
            |s| s.restarts >= 1 && matches!(s.status, ServerStatus::Running),
            Duration::from_secs(60),
        )
        .await;

        assert_ne!(
            restarted.pid,
            Some(pid),
            "expected a new process after the crash restart"
        );
        assert!(
            restarted.capabilities.is_some(),
            "restarted session should re-complete the handshake"
        );

        session.stop().await;
    }
}
