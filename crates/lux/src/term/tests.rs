use super::*;

#[test]
fn terminal_state_initial_defaults() {
    let state = TerminalState::new();
    assert!(!state.visible);
    assert!(!state.focused);
    assert_eq!(state.active_tab, 0);
    assert!(state.session_ids.is_empty());
    assert!(state.session_names.is_empty());
    assert!(state.task_ids.is_empty());
    assert!(state.active_pty().is_none());
}

#[test]
fn spawn_and_close_session_lifecycle() {
    let mut state = TerminalState::new();
    let id1 = state.ensure_session(None).expect("spawn session 1");
    assert_eq!(state.session_ids.len(), 1);
    assert_eq!(state.active_id(), Some(id1));
    assert_eq!(state.active_name(), Some("Terminal 1"));

    let id2 = state
        .spawn_session(Some("custom-shell".into()), None, None)
        .expect("spawn session 2");
    assert_eq!(state.session_ids.len(), 2);
    assert_eq!(state.active_tab, 1);
    assert_eq!(state.active_id(), Some(id2));
    assert_eq!(state.active_name(), Some("custom-shell"));

    // Write some input to the active PTY
    if let Some(pty) = state.active_pty_mut() {
        let _ = pty.write(b"echo hello\n");
    }

    // Close session 2
    assert!(state.close_active_session());
    assert_eq!(state.session_ids.len(), 1);
    assert_eq!(state.active_tab, 0);
    assert_eq!(state.active_id(), Some(id1));

    // Close remaining session
    assert!(state.close_active_session());
    assert_eq!(state.session_ids.len(), 0);
    assert!(!state.visible);
    assert!(!state.focused);
}

#[test]
fn resize_adjusts_active_pty_size() {
    let mut state = TerminalState::new();
    let _id = state.ensure_session(None).expect("spawn session");
    state.resize(30, 100);
    if let Some(pty) = state.active_pty() {
        assert_eq!(pty.size(), (30, 100));
    }
    state.shutdown_all();
}

#[test]
fn task_runner_and_cancellation() {
    let mut state = TerminalState::new();
    #[cfg(unix)]
    let mut cmd = CommandBuilder::new("/bin/sh");
    #[cfg(windows)]
    let mut cmd = CommandBuilder::new("cmd.exe");

    #[cfg(unix)]
    cmd.args(["-c", "sleep 30"]);
    #[cfg(windows)]
    cmd.args(["/c", "timeout", "30"]);

    let task_id = state.run_task("sleep-job", cmd).expect("start task");
    assert_eq!(state.task_ids.len(), 1);
    assert_eq!(state.active_task, Some(task_id));
    assert!(state.show_tasks);
    assert!(state.visible);

    assert!(state.cancel_active_task());
    assert!(state.task_ids.is_empty());
    assert!(state.active_task.is_none());
}

#[test]
fn shutdown_all_cleans_up_all_children() {
    let mut state = TerminalState::new();
    let _ = state.ensure_session(None);
    let _ = state.spawn_session(Some("two".into()), None, None);
    assert_eq!(state.session_ids.len(), 2);
    state.shutdown_all();
    assert_eq!(state.session_ids.len(), 0);
    assert_eq!(state.session_names.len(), 0);
}
