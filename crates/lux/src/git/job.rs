//! Git reads and writes run on `lux-git`, then the UI thread applies them.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

use lux_git::gutter::Hunk;

use super::snapshot::{self, FileShot};
use crate::app::App;

enum Event {
    File {
        generation: u64,
        shot: FileShot,
    },
    Status(snapshot::StatusShot),
    Text {
        title: String,
        body: String,
    },
    Branches {
        names: Vec<String>,
        creating: bool,
    },
    OnBranch {
        name: String,
        bytes: Option<Vec<u8>>,
    },
    Again,
    Failed(String),
}

fn mail() -> &'static (mpsc::Sender<Event>, Mutex<mpsc::Receiver<Event>>) {
    static MAIL: OnceLock<(mpsc::Sender<Event>, Mutex<mpsc::Receiver<Event>>)> = OnceLock::new();
    MAIL.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        (tx, Mutex::new(rx))
    })
}

type Wake = Arc<dyn Fn() + Send + Sync>;

fn waker_slot() -> &'static Mutex<Option<Wake>> {
    static WAKER: OnceLock<Mutex<Option<Wake>>> = OnceLock::new();
    WAKER.get_or_init(|| Mutex::new(None))
}

pub(crate) fn register_waker(waker: Wake) {
    if let Ok(mut slot) = waker_slot().lock() {
        *slot = Some(waker);
    }
}

fn spawn(work: impl FnOnce() -> Event + Send + 'static) {
    let tx = mail().0.clone();
    let wake = waker_slot().lock().ok().and_then(|slot| slot.clone());
    let _ = std::thread::Builder::new()
        .name("lux-git".into())
        .spawn(move || {
            let event = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
                .unwrap_or_else(|_| Event::Failed("git task failed".into()));
            if tx.send(event).is_ok()
                && let Some(wake) = wake
            {
                wake();
            }
        });
}

pub(crate) fn request_refresh(app: &mut App) {
    let generation = {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    };
    app.ui.git.generation = generation;
    app.ui.git.hunks.clear();
    app.ui.git.markers.clear();
    let path = app.documents.path.clone().or_else(|| {
        app.ui
            .sidebar
            .as_ref()
            .and_then(|sidebar| sidebar.workspace_root.clone())
    });
    let text = app.buffer.text();
    spawn(move || Event::File {
        generation,
        shot: snapshot::file_shot(path.as_deref(), &text),
    });
}

pub(crate) fn request_status(app: &mut App) {
    let Some(start) = start_path(app) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    spawn(move || match snapshot::status_of(&start) {
        Ok(shot) => Event::Status(shot),
        Err(error) => Event::Failed(error),
    });
}

pub(crate) fn request_diff(app: &mut App) {
    let Some((root, rel)) = root_and_rel(app) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    let title = format!(
        "Diff: {}",
        rel.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("diff")
    );
    spawn(move || match snapshot::diff_text(&root, &rel) {
        Ok(body) => Event::Text { title, body },
        Err(error) => Event::Failed(error),
    });
}

pub(crate) fn request_blame(app: &mut App) {
    let Some((root, rel)) = root_and_rel(app) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    spawn(move || match snapshot::blame_text(&root, &rel) {
        Ok(body) => Event::Text {
            title: "Blame".into(),
            body,
        },
        Err(error) => Event::Failed(error),
    });
}

pub(crate) fn request_history(app: &mut App) {
    let Some((root, rel)) = root_and_rel(app) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    spawn(move || match snapshot::history_text(&root, &rel) {
        Ok(body) => Event::Text {
            title: "History".into(),
            body,
        },
        Err(error) => Event::Failed(error),
    });
}

pub(crate) fn request_stage_file(app: &mut App, stage: bool) {
    let Some((root, rel)) = root_and_rel(app) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    spawn(move || {
        let repo = match super::discover_repo(&root) {
            Some(repo) => repo,
            None => return Event::Failed("not a git repository".into()),
        };
        let result = if stage {
            lux_git::staging::stage(&repo, &rel)
        } else {
            lux_git::staging::unstage(&repo, &rel)
        };
        match result {
            Ok(()) => Event::Again,
            Err(error) => Event::Failed(error.to_string()),
        }
    });
}

pub(crate) fn request_branch_list(app: &mut App, creating: bool) {
    let Some(start) = start_path(app) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    spawn(move || match snapshot::branch_names(&start) {
        Ok(names) => Event::Branches { names, creating },
        Err(error) => Event::Failed(error),
    });
}

pub(crate) fn request_switch_branch(app: &mut App, name: String) {
    let Some(root) = app.ui.git.repo_root.clone().or_else(|| start_path(app)) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    let open = app.documents.path.clone();
    spawn(move || match snapshot::switch_branch(&root, &name) {
        Ok(name) => Event::OnBranch {
            name,
            bytes: read_open(open),
        },
        Err(error) => Event::Failed(error),
    });
}

pub(crate) fn request_create_branch(app: &mut App, name: String) {
    let Some(root) = app.ui.git.repo_root.clone().or_else(|| start_path(app)) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    spawn(move || match snapshot::create_branch(&root, &name) {
        Ok(name) => Event::OnBranch { name, bytes: None },
        Err(error) => Event::Failed(error),
    });
}

pub(crate) fn request_commit(app: &mut App, message: String) {
    let Some(root) = app.ui.git.repo_root.clone().or_else(|| start_path(app)) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    spawn(move || match snapshot::commit_index(&root, &message) {
        Ok(()) => Event::Again,
        Err(error) => Event::Failed(error),
    });
}

pub(crate) fn defer_stage_hunk(app: &mut App, hunk: Hunk) {
    let Some((root, file)) = root_and_file(app) else {
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    spawn(
        move || match snapshot::stage_hunk_on_disk(&root, &file, &hunk) {
            Ok(()) => Event::Again,
            Err(error) => Event::Failed(error),
        },
    );
}

pub(crate) fn request_unstage_cursor_hunk(app: &mut App) {
    let Some((root, file)) = root_and_file(app) else {
        eprintln!("palette-enter id=git.unstage_hunk result=Err(Open a repository file first)");
        app.ui.announcement = Some("Open a repository file first".into());
        return;
    };
    let line = app.cursors.primary().head.line as u32 + 1;
    spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            snapshot::unstage_hunk_at_line(&root, &file, line)
        }));
        match &result {
            Ok(Ok(())) => eprintln!("palette-enter id=git.unstage_hunk result=Ok(())"),
            Ok(Err(error)) => {
                eprintln!("palette-enter id=git.unstage_hunk result=Err({error})")
            }
            Err(_) => eprintln!("palette-enter id=git.unstage_hunk result=Err(panic)"),
        }
        match result {
            Ok(Ok(())) => Event::Again,
            Ok(Err(error)) => Event::Failed(error),
            Err(_) => Event::Failed("git task failed".into()),
        }
    });
}

pub(crate) fn request_stage_cursor_hunk(app: &mut App) {
    let line = app.cursors.primary().head.line as u32 + 1;
    let Some(hunk) = app.ui.git.hunks.iter().copied().find(|hunk| {
        let start = hunk.after_start;
        let end = start + hunk.after_len.max(1);
        line >= start && line < end
    }) else {
        app.ui.announcement = Some("No hunk on this line".into());
        return;
    };
    defer_stage_hunk(app, hunk);
}

pub(crate) fn poll(app: &mut App) {
    let Ok(rx) = mail().1.lock() else {
        return;
    };
    while let Ok(event) = rx.try_recv() {
        match event {
            Event::File { generation, shot } if generation == app.ui.git.generation => {
                app.ui.git.repo_root = shot.repo_root;
                app.ui.git.branch_name = shot.branch_name;
                app.ui.git.hunks = shot.hunks;
                app.ui.git.markers = shot.markers;
                app.scheduler.invalidate_all();
            }
            Event::Status(shot) => {
                app.ui.git.repo_root = Some(shot.root);
                app.ui.git.branch_name = shot.branch;
                app.ui.git.changed_files = shot.files;
                super::publish_git_marks(app);
                app.scheduler.invalidate_all();
            }
            Event::Text { title, body } => {
                super::open_text_tab(app, &title, &body);
                app.scheduler.invalidate_all();
            }
            Event::Branches { names, creating } => {
                super::open_branch_picker(app, names, creating);
            }
            Event::OnBranch { name, bytes } => {
                app.ui.git.branch_name = Some(name.clone());
                app.ui.announcement = Some(format!("On branch {name}"));
                if let Some(bytes) = bytes {
                    super::apply_switched_file(app, &bytes);
                }
                super::refresh_git_state(app);
            }
            Event::Again => super::refresh_git_state(app),
            Event::Failed(message) => {
                app.ui.announcement = Some(message);
            }
            Event::File { .. } => {}
        }
    }
}

fn read_open(path: Option<PathBuf>) -> Option<Vec<u8>> {
    std::fs::read(path?).ok()
}

fn start_path(app: &App) -> Option<PathBuf> {
    app.documents.path.clone().or_else(|| {
        app.ui
            .sidebar
            .as_ref()
            .and_then(|sidebar| sidebar.workspace_root.clone())
    })
}

fn root_and_file(app: &App) -> Option<(PathBuf, PathBuf)> {
    let file = app.documents.path.clone()?;
    let root = app
        .ui
        .git
        .repo_root
        .clone()
        .or_else(|| file.parent().map(Path::to_path_buf))?;
    Some((root, file))
}

fn root_and_rel(app: &App) -> Option<(PathBuf, PathBuf)> {
    let (root, file) = root_and_file(app)?;
    let rel = file.strip_prefix(&root).ok()?.to_path_buf();
    Some((root, rel))
}
