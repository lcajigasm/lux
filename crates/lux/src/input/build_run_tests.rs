//! ST-BUILD-02 through the app: a real build child on a PTY, the exit
//! reported by the child-process notification, Cancel Build from the key.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use lux_platform::Modifiers;

use crate::app::App;
use crate::build::BuildVariant;
use crate::input::test_ctx::{TestCtx, app, chr, press};

/// An app whose event-loop wake sends on the returned channel.
fn app_with_wake() -> (App, Receiver<()>) {
    let mut app = app("");
    app.viewport.width = 800.0;
    app.viewport.height = 600.0;
    let (tx, rx) = mpsc::channel();
    app.ui.dialogs.jobs.set_wake(Arc::new(move || {
        let _ = tx.send(());
    }));
    (app, rx)
}

fn start(app: &mut App, script: &str) {
    let variant = BuildVariant {
        name: "Script".into(),
        command: "/bin/sh".into(),
        args: vec!["-c".into(), script.into()],
        file_regex: Some(r"^(\S+\.rs):(\d+):(\d+)".into()),
        ..Default::default()
    };
    assert!(app.execute_build_variant(variant, None));
    assert!(app.ui.build.is_running);
}

/// Waits for the next wake and does what `background_event` does with it.
fn next_wake(app: &mut App, wakes: &Receiver<()>) {
    wakes
        .recv_timeout(Duration::from_secs(10))
        .expect("the build must wake the event loop");
    crate::build::poll(app);
}

fn run_to_exit(app: &mut App, wakes: &Receiver<()>) {
    while app.ui.build.is_running {
        next_wake(app, wakes);
    }
}

fn output(app: &App) -> Vec<String> {
    let task = app.ui.build.task_id.expect("build task");
    let pty = app.ui.term.tasks.get(task).expect("output kept");
    pty.snapshot()
        .lines()
        .map(str::to_string)
        .filter(|line| !line.is_empty())
        .collect()
}

fn finished(line: &str, suffix: &str) -> bool {
    let Some(rest) = line.strip_prefix("[Finished in ") else {
        return false;
    };
    let Some(secs) = rest.strip_suffix(suffix) else {
        return false;
    };
    secs.strip_suffix('s').is_some_and(|n| {
        n.split_once('.').is_some_and(|(_, d)| d.len() == 1) && n.parse::<f64>().is_ok()
    })
}

#[test]
fn a_build_that_exits_prints_finished_and_stops_running() {
    let (mut app, wakes) = app_with_wake();
    start(&mut app, "printf 'src/main.rs:2:5: error\\n'; sleep 0.1");
    run_to_exit(&mut app, &wakes);

    let lines = output(&app);
    assert_eq!(lines[0], "src/main.rs:2:5: error");
    assert!(finished(&lines[1], "]"), "{lines:?}");
    assert!(!app.ui.build.cancelling);
    assert_eq!(
        app.ui.build.detected_errors.len(),
        1,
        "errors parsed at exit"
    );
    assert!(app.ui.term.show_tasks, "the dock shows the build output");
}

#[test]
fn a_failing_build_reports_its_exit_code() {
    let (mut app, wakes) = app_with_wake();
    start(&mut app, "echo failing; exit 101");
    run_to_exit(&mut app, &wakes);
    let lines = output(&app);
    assert_eq!(lines[0], "failing");
    assert!(finished(&lines[1], " with exit code 101]"), "{lines:?}");
}

#[test]
fn cmd_alt_c_stops_the_child_keeps_the_output_and_prints_cancelled() {
    let (mut app, wakes) = app_with_wake();
    let ctx = TestCtx::default();
    start(&mut app, "echo started; sleep 30");
    let pid = {
        let task = app.ui.build.task_id.unwrap();
        app.ui
            .term
            .tasks
            .get(task)
            .and_then(lux_term::Pty::pid)
            .unwrap()
    };
    while !output(&app).iter().any(|l| l == "started") {
        next_wake(&mut app, &wakes);
    }

    // A click in the output must not cancel anything.
    let y = app.tab_bar_height() + app.viewport.height - 10.0;
    crate::term::input::handle_term_click(&mut app, &ctx, 100.0, y);
    assert!(app.ui.build.is_running && !app.ui.build.cancelling);

    if cfg!(target_os = "macos") {
        let cmd_alt = Modifiers {
            meta: true,
            alt: true,
            ..Default::default()
        };
        press(&mut app, &ctx, chr("c"), cmd_alt);
    } else {
        // `ctrl+break` has no key the test context can name; run the
        // command that binding dispatches.
        crate::input::dispatch::execute_keymap_command(&mut app, "cancel_build", None, None);
    }
    assert!(
        app.ui.build.cancelling,
        "Cancel Build returns before the child is gone"
    );

    run_to_exit(&mut app, &wakes);
    assert_eq!(output(&app), ["started", "[Cancelled]"]);
    let alive = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(!alive, "the build child is gone");
    assert!(!app.cancel_build(), "nothing left to cancel");
}

fn pid_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}

#[test]
fn a_new_build_stops_the_running_one_and_frees_its_output() {
    let (mut app, wakes) = app_with_wake();
    start(&mut app, "trap '' HUP; echo first; sleep 30");
    let first = app.ui.build.task_id.unwrap();
    let pid = app
        .ui
        .term
        .tasks
        .get(first)
        .and_then(lux_term::Pty::pid)
        .unwrap();
    while !output(&app).iter().any(|l| l == "first") {
        next_wake(&mut app, &wakes);
    }

    start(&mut app, "echo second");
    let second = app.ui.build.task_id.unwrap();
    assert_ne!(first, second);
    assert!(app.ui.term.tasks.get(first).is_none(), "old output freed");
    assert_eq!(app.ui.term.tasks.len(), 1);

    // The old build's exit does not end the new one; the new one's does.
    run_to_exit(&mut app, &wakes);
    let lines = output(&app);
    assert_eq!(lines[0], "second");
    assert!(finished(&lines[1], "]"), "{lines:?}");

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while pid_alive(pid) {
        assert!(std::time::Instant::now() < deadline, "old build survived");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn only_the_latest_build_keeps_a_terminal() {
    let (mut app, wakes) = app_with_wake();
    for n in 0..3 {
        start(&mut app, &format!("echo run{n}"));
        run_to_exit(&mut app, &wakes);
    }
    assert_eq!(app.ui.term.tasks.len(), 1);
    assert_eq!(app.ui.term.task_ids.len(), 1);
    assert_eq!(output(&app)[0], "run2");
}
