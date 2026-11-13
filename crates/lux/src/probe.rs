//! Timings and a one-shot state dump for the macOS CI jobs.
//!
//! `--probe PATH` installs a SIGUSR2 handler. The first signal opens
//! `--bench-open` when that path is set. A later signal writes `PATH`.
//! The write happens on a background thread. The numbers are the live
//! session: first presented frame, key event to the next presented frame,
//! and redraw duration while a wheel event is in progress.

use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use lux_platform::PlatformContext;

use crate::app::App;

struct Slot {
    probe_path: Option<PathBuf>,
    bench_open: Option<PathBuf>,
    bench_started: Option<Instant>,
    bench_loaded: bool,
    bench_logged: bool,
    first_logged: bool,
    key_at: Option<Instant>,
    key_us: Vec<u64>,
    scrolling: bool,
    frame_us: Vec<u64>,
}

impl Slot {
    fn empty() -> Self {
        Self {
            probe_path: None,
            bench_open: None,
            bench_started: None,
            bench_loaded: false,
            bench_logged: false,
            first_logged: false,
            key_at: None,
            key_us: Vec::new(),
            scrolling: false,
            frame_us: Vec::new(),
        }
    }
}

static PROCESS_START: OnceLock<Instant> = OnceLock::new();
static LAST_INPUT: Mutex<String> = Mutex::new(String::new());
static SLOT: Mutex<Option<Slot>> = Mutex::new(None);
static ARMED: AtomicU32 = AtomicU32::new(0);
static TAKEN: AtomicU32 = AtomicU32::new(0);
static WAKE_WRITE: AtomicI32 = AtomicI32::new(-1);
static WAKE_STARTED: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn pipe(fds: *mut i32) -> i32;
    fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
}

extern "C" fn on_usr2(sig: i32) {
    unsafe {
        signal(sig, on_usr2);
    }
    ARMED.fetch_add(1, Ordering::Release);
    let fd = WAKE_WRITE.load(Ordering::Acquire);
    if fd >= 0 {
        let byte = [1u8];
        unsafe {
            write(fd, byte.as_ptr(), 1);
        }
    }
}

fn sigusr2() -> i32 {
    if cfg!(target_os = "macos") { 31 } else { 12 }
}

#[cfg(target_os = "macos")]
fn wake_main() {
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRunLoopGetMain() -> *mut std::ffi::c_void;
        fn CFRunLoopWakeUp(run_loop: *mut std::ffi::c_void);
    }
    unsafe { CFRunLoopWakeUp(CFRunLoopGetMain()) }
}

#[cfg(not(target_os = "macos"))]
fn wake_main() {}

pub(crate) fn mark_process_start() {
    let _ = PROCESS_START.set(Instant::now());
}

pub(crate) fn configure(probe: Option<PathBuf>, bench_open: Option<PathBuf>) {
    if probe.is_none() && bench_open.is_none() {
        return;
    }
    *SLOT.lock().expect("probe") = Some(Slot {
        probe_path: probe,
        bench_open,
        ..Slot::empty()
    });
}

pub(crate) fn listen() {
    if SLOT.lock().expect("probe").is_none() {
        return;
    }
    if WAKE_STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let mut fds = [-1i32; 2];
    unsafe {
        if pipe(fds.as_mut_ptr()) != 0 {
            eprintln!("Lux: probe wake pipe failed");
            return;
        }
        WAKE_WRITE.store(fds[1], Ordering::Release);
        signal(sigusr2(), on_usr2);
    }
    let read_fd = fds[0];
    let _ = std::thread::Builder::new()
        .name("lux-probe-wake".into())
        .spawn(move || {
            let mut buf = [0u8; 16];
            loop {
                let n = unsafe { read(read_fd, buf.as_mut_ptr(), buf.len()) };
                if n <= 0 {
                    break;
                }
                // One wakeup can land while the main thread is already
                // inside an event and then be dropped. Keep waking until
                // that signal is consumed.
                for _ in 0..40 {
                    wake_main();
                    if TAKEN.load(Ordering::Acquire) >= ARMED.load(Ordering::Acquire) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        });
}

pub(crate) fn note_key() {
    let Ok(mut guard) = SLOT.lock() else {
        return;
    };
    let Some(slot) = guard.as_mut() else {
        return;
    };
    if slot.key_us.len() < 400 {
        slot.key_at = Some(Instant::now());
    }
}

pub(crate) fn note_scroll() {
    let Ok(mut guard) = SLOT.lock() else {
        return;
    };
    if let Some(slot) = guard.as_mut() {
        slot.scrolling = true;
    }
}

pub(crate) fn note_open_loaded() {
    let Ok(mut guard) = SLOT.lock() else {
        return;
    };
    if let Some(slot) = guard.as_mut()
        && slot.bench_started.is_some()
    {
        slot.bench_loaded = true;
    }
}

pub(crate) fn on_frame(app: &App, frame: Duration) {
    let Ok(mut guard) = SLOT.lock() else {
        return;
    };
    let Some(slot) = guard.as_mut() else {
        return;
    };
    if !slot.first_logged {
        slot.first_logged = true;
        let start = PROCESS_START.get().copied().unwrap_or(app.start);
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        eprintln!("Lux: first frame {ms:.1} ms");
    }
    if let Some(at) = slot.key_at.take() {
        slot.key_us.push(at.elapsed().as_micros() as u64);
    }
    if slot.scrolling && slot.frame_us.len() < 800 {
        slot.frame_us.push(frame.as_micros() as u64);
    }
    if slot.bench_loaded && !slot.bench_logged {
        slot.bench_logged = true;
        if let Some(started) = slot.bench_started {
            let ms = started.elapsed().as_secs_f64() * 1000.0;
            eprintln!("Lux: buffer frame {ms:.1} ms");
        }
    }
}

/// Handles queued SIGUSR2 signals. Opening a bench file consumes one
/// signal. Writing the probe file consumes one signal.
///
/// Redraw calls this before shaping. The wide-file check reads the dump
/// from `about_to_wait`, which cannot run while that redraw is on the stack.
pub(crate) fn poll(app: &mut App, ctx: &dyn PlatformContext) {
    loop {
        let armed = ARMED.load(Ordering::Acquire);
        let taken = TAKEN.load(Ordering::Acquire);
        if armed <= taken {
            return;
        }
        let open = {
            let Ok(mut guard) = SLOT.lock() else {
                return;
            };
            let Some(slot) = guard.as_mut() else {
                TAKEN.store(armed, Ordering::Release);
                return;
            };
            slot.bench_open.take()
        };
        if let Some(path) = open {
            if let Ok(mut guard) = SLOT.lock()
                && let Some(slot) = guard.as_mut()
            {
                slot.bench_started = Some(Instant::now());
            }
            TAKEN.fetch_add(1, Ordering::Release);
            app.request_document(ctx, crate::document::Request::Open(Some(path)));
            continue;
        }
        let body = {
            let Ok(guard) = SLOT.lock() else {
                return;
            };
            let Some(slot) = guard.as_ref() else {
                TAKEN.store(armed, Ordering::Release);
                return;
            };
            let Some(path) = slot.probe_path.clone() else {
                TAKEN.store(armed, Ordering::Release);
                return;
            };
            (path, render_dump(app, slot))
        };
        TAKEN.fetch_add(1, Ordering::Release);
        let _ = std::thread::Builder::new()
            .name("lux-probe-write".into())
            .spawn(move || {
                if let Ok(mut file) = std::fs::File::create(&body.0) {
                    let _ = file.write_all(body.1.as_bytes());
                }
            });
    }
}

fn render_dump(app: &App, slot: &Slot) -> String {
    let mut out = String::new();
    let chars = app.buffer.len_chars();
    if chars > 2000 {
        out.push_str(&format!("text_chars={chars}\n"));
    } else {
        out.push_str("text=");
        out.push_str(&escape(&app.buffer.text()));
        out.push('\n');
    }
    out.push_str("sel=");
    for (index, selection) in app.cursors.cursors().iter().take(8).enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{}:{}",
            selection.anchor.char, selection.head.char
        ));
    }
    out.push('\n');
    out.push_str(&format!("dirty={}\n", u8::from(app.document_dirty())));
    let (goto, goto_mode, goto_query) = match &app.ui.goto_anything {
        Some(state) => {
            let mode = match state.mode {
                crate::goto::GotoMode::Anything => "anything",
                crate::goto::GotoMode::ProjectSymbols => "project",
            };
            (1, mode, state.query.text())
        }
        None => (0, "off", ""),
    };
    out.push_str(&format!("goto={goto}\ngoto_mode={goto_mode}\n"));
    out.push_str("goto_query=");
    out.push_str(&escape(goto_query));
    out.push('\n');
    let palette_query = app
        .ui
        .command_palette
        .as_ref()
        .map(|palette| palette.query.text())
        .unwrap_or("");
    out.push_str(&format!(
        "palette={}\n",
        u8::from(app.ui.command_palette.is_some())
    ));
    out.push_str("palette_query=");
    out.push_str(&escape(palette_query));
    out.push('\n');
    out.push_str("announcement=");
    out.push_str(&escape(app.ui.announcement.as_deref().unwrap_or("")));
    out.push('\n');
    let (find, replace) = match &app.ui.search_bar {
        Some(search) => (1, u8::from(search.replace_mode)),
        None => (0, 0),
    };
    out.push_str(&format!(
        "find={find}\nreplace={replace}\nproject_search={}\nsettings={}\nsettings_scroll={}\n",
        u8::from(app.ui.project_search.is_some()),
        u8::from(app.ui.settings.panel.is_some()),
        app.ui
            .settings
            .panel
            .as_ref()
            .map(|panel| panel.scroll.round() as i64)
            .unwrap_or(0),
    ));
    out.push_str("edit_menu=");
    match &app.mouse.context_menu {
        Some(menu) => {
            out.push('1');
            for row in &menu.rows {
                out.push('\t');
                match row {
                    crate::edit_menu::EditMenuRow::Separator => out.push('-'),
                    crate::edit_menu::EditMenuRow::Action { action, enabled } => {
                        if !enabled {
                            out.push('!');
                        }
                        out.push_str(action.label());
                    }
                }
            }
        }
        None => out.push('0'),
    }
    out.push('\n');
    out.push_str("edit_menu_geom=");
    out.push_str(&crate::edit_menu::geometry_probe(app));
    out.push('\n');
    let path = app
        .documents
        .active_entry()
        .and_then(|entry| entry.path.as_ref())
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let language = app
        .documents
        .language()
        .map(|language| language.name())
        .unwrap_or("Plain Text");
    let preview_fences = if app
        .documents
        .active_entry()
        .is_some_and(|entry| entry.markdown_preview)
    {
        crate::render::markdown::rendered_code_blocks(&app.buffer.text(), app.active_syntax_theme())
    } else {
        String::new()
    };
    out.push_str("preview_fences=");
    out.push_str(&escape(&preview_fences));
    out.push('\n');
    out.push_str(&format!(
        "tabs={}\ndirty={}\nlanguage={language}\ntheme={}\nbranch={}\nbranch_picker={}\nactive_path=",
        app.documents.tabs.tabs().len(),
        u8::from(app.document_dirty()),
        app.chrome.theme.name,
        app.ui.git.branch_name.as_deref().unwrap_or(""),
        u8::from(app.ui.git.branch_picker.is_some()),
    ));
    out.push_str(&escape(&path));
    out.push('\n');
    out.push_str("git_marks=");
    out.push_str(&escape(&crate::git::git_marks_label(
        &app.ui.git.changed_files,
    )));
    out.push('\n');
    let chord = app
        .ui
        .pending_chord
        .as_ref()
        .map(|pending| pending.to_string())
        .unwrap_or_default();
    out.push_str("chord=");
    out.push_str(&escape(&chord));
    out.push('\n');
    out.push_str(&format!(
        "panes={}\ntabs={}\nsidebar={}\nsidebar_scroll={}\nscroll_y={}\nscroll_x={}\nword_wrap={}\n",
        app.ui.split.preset.num_panes(),
        app.documents.entries.len(),
        u8::from(app.ui.sidebar.is_some()),
        app.ui
            .sidebar
            .as_ref()
            .map(|sidebar| sidebar.scroll_y.round() as i64)
            .unwrap_or(0),
        app.viewport.scroll_y.round() as i64,
        app.viewport.scroll_x.round() as i64,
        u8::from(app.editor_view.metrics().word_wrap),
    ));
    out.push_str("roots=");
    if let Some(sidebar) = &app.ui.sidebar {
        out.push_str(&escape(&sidebar.root_names().join("|")));
    }
    out.push('\n');
    out.push_str("tree=");
    if let Some(sidebar) = &app.ui.sidebar {
        out.push_str(&escape(&crate::sidebar::tree_label(sidebar)));
    }
    out.push('\n');
    out.push_str(&format!(
        "key_n={}\nkey_p99_us={}\nframe_n={}\nframe_max_us={}\n",
        slot.key_us.len(),
        percentile_99(&slot.key_us).unwrap_or(0),
        slot.frame_us.len(),
        slot.frame_us.iter().copied().max().unwrap_or(0),
    ));
    let input = LAST_INPUT
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_default();
    out.push_str("input=");
    out.push_str(&escape(&input));
    out.push('\n');
    out
}

pub(crate) fn note_input(key: &lux_platform::KeyEvent, modifiers: &lux_platform::Modifiers) {
    if key.state != lux_platform::ElementState::Pressed {
        return;
    }
    let physical = key
        .physical
        .map(|key| key.chord_token())
        .unwrap_or_default();
    let logical = match &key.key {
        lux_platform::Key::Character(text) => text.clone(),
        lux_platform::Key::Named(named) => format!("{named:?}"),
        lux_platform::Key::Unidentified => "unidentified".into(),
    };
    let line = format!(
        "{logical}/{physical} meta={} ctrl={} alt={} shift={}",
        u8::from(modifiers.meta),
        u8::from(modifiers.control),
        u8::from(modifiers.alt),
        u8::from(modifiers.shift),
    );
    if let Ok(mut slot) = LAST_INPUT.lock() {
        *slot = line;
    }
}

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out
}

pub(crate) fn percentile_99(samples: &[u64]) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    let index = (ordered.len() - 1) * 99 / 100;
    ordered.get(index).copied()
}

#[cfg(test)]
mod tests {
    use super::percentile_99;

    #[test]
    fn p99_uses_the_high_end_of_the_sample() {
        let samples: Vec<u64> = (1..=100).collect();
        assert_eq!(percentile_99(&samples), Some(99));
        assert_eq!(percentile_99(&[]), None);
        assert_eq!(percentile_99(&[8]), Some(8));
    }
}
