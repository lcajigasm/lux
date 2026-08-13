//! Integration tests against `lux-lsp-fake-server`
//! (`tests/support/fake_server_main.rs`) -- the "servidor de mentira que
//! ejerza el protocolo" Phase 7's acceptance criteria asks for as a
//! separate thing from the fuzzer. Real subprocess, real wire protocol;
//! only the fake server's *responses* are scripted, which is exactly
//! what lets these tests pin down things a real server can't: what this
//! client actually sent during `initialize`, an exact `$/progress`
//! sequence, and a server that exits on its own rather than being
//! killed.

mod protocol {
    use std::path::Path;
    use std::sync::Arc;
    use std::time::Duration;

    use lux_lsp::{ServerSession, ServerSpec, ServerState, ServerStatus};

    const FAKE_SERVER_SPEC: ServerSpec = ServerSpec {
        language_id: "fake",
        command: env!("CARGO_BIN_EXE_lux-lsp-fake-server"),
        args: &[],
    };

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

    fn spawn_fake(root: &Path) -> Arc<ServerSession> {
        Arc::new(ServerSession::spawn(FAKE_SERVER_SPEC, root.to_path_buf()))
    }

    #[tokio::test]
    async fn capability_handshake_receives_exactly_what_the_client_sent() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let session = spawn_fake(dir.path());

        let state = wait_for(
            &session,
            |s| {
                matches!(
                    s.status,
                    ServerStatus::Running | ServerStatus::Failed { .. }
                )
            },
            Duration::from_secs(30),
        )
        .await;
        assert_eq!(state.status, ServerStatus::Running);
        assert!(
            state
                .capabilities
                .as_ref()
                .is_some_and(|c| c.hover_provider.is_some()),
            "expected the fake server's scripted hover capability to round-trip"
        );

        let capture_path = dir.path().join("fake-lsp-initialize-capture.json");
        // The fake server writes this synchronously inside its `initialize`
        // handler, before it ever responds -- by the time our session sees
        // `Running` (which only happens after that response arrives), the
        // file is guaranteed to already be on disk.
        let captured: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&capture_path)
                .expect("fake server should have captured initialize params"),
        )
        .unwrap();

        // `ServerSession::spawn` is the raw primitive and does not
        // canonicalize its root itself (that's `ServerRegistry`'s job) --
        // match exactly what was actually passed in, not a resolved
        // version of it (macOS's `/var/folders` is itself a symlink to
        // `/private/var/folders`, so those legitimately differ). Built the
        // same way `session.rs` itself builds it (`Url::from_file_path`),
        // not a hand-rolled `format!` string -- a naive `file://{path}`
        // is wrong on Windows (backslashes, no leading slash before the
        // drive letter), which is exactly what `Url::from_file_path`
        // exists to get right.
        let expected_root_uri = async_lsp::lsp_types::Url::from_file_path(dir.path())
            .unwrap()
            .to_string();
        assert_eq!(
            captured["root_uri"], expected_root_uri,
            "root_uri sent to the server did not match the session's actual root"
        );
        let folders = captured["workspace_folders"]
            .as_array()
            .expect("workspace_folders should be a real array, not omitted");
        assert_eq!(folders.len(), 1, "expected exactly one workspace folder");
        assert_eq!(folders[0]["uri"], expected_root_uri);

        session.stop().await;
    }

    #[tokio::test]
    async fn a_scripted_progress_sequence_is_tracked_exactly() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("fake-lsp-send-progress"), "").unwrap();
        let session = spawn_fake(dir.path());

        wait_for(
            &session,
            |s| matches!(s.status, ServerStatus::Running),
            Duration::from_secs(30),
        )
        .await;

        // The fake server always sends exactly begin, report, end -- unlike
        // a real server's progress count, which varies run to run.
        let state = wait_for(
            &session,
            |s| s.progress_events_seen >= 3,
            Duration::from_secs(30),
        )
        .await;
        assert_eq!(
            state.progress_events_seen, 3,
            "expected exactly the scripted begin/report/end sequence, no more, no fewer"
        );
        // `End` clears the entry (same as a real server's progress ending).
        assert!(
            state.progress.is_empty(),
            "the completed progress entry should have been removed, not left stale"
        );

        session.stop().await;
    }

    #[tokio::test]
    async fn a_server_that_exits_on_its_own_triggers_a_restart_too() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("fake-lsp-exit-after-initialized"), "").unwrap();
        let session = spawn_fake(dir.path());

        let running = wait_for(
            &session,
            |s| matches!(s.status, ServerStatus::Running),
            Duration::from_secs(30),
        )
        .await;
        running.pid.expect("running session should expose a pid");

        // No signal sent here -- this server terminates itself. Restart
        // logic (deliverable 1) was only ever proven against a `SIGKILL`;
        // this is the other real way a language server goes away. The
        // exit marker is consumed on first use (see the fake server's own
        // logic), so the restarted instance comes back up and stays up --
        // not a crash loop.
        // `restarts >= 1` is the real signal that a genuine respawn
        // happened -- it only increments inside `supervise()`'s actual
        // restart path. A new process's pid differing from the old one
        // is *not* a safe invariant to assert on top of that: Linux (this
        // showed up on the `ubuntu-latest` CI runner, not locally on
        // macOS) can and does immediately reuse a just-exited process's
        // pid for the next one spawned, especially with few other
        // processes competing for the pid space.
        let _restarted = wait_for(
            &session,
            |s| s.restarts >= 1 && matches!(s.status, ServerStatus::Running),
            Duration::from_secs(30),
        )
        .await;

        // Confirm it actually stayed up rather than looping into another
        // restart right after this check ran.
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(session.status(), ServerStatus::Running);
        assert_eq!(
            session.watch().borrow().restarts,
            1,
            "expected exactly one restart, not a crash loop"
        );

        session.stop().await;
    }
}
