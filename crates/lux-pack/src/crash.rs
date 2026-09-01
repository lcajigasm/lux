//! Panic hook that writes a local log. Never sends the file anywhere.

use std::fs;
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn crash_dir() -> PathBuf {
    let base = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    let dir = base.join("Library/Logs/Lux");
    #[cfg(not(target_os = "macos"))]
    let dir = base.join(".lux").join("crashes");
    dir
}

pub fn write_crash_text(text: &str) -> PathBuf {
    let dir = crash_dir();
    let _ = fs::create_dir_all(&dir);
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let path = dir.join(format!("crash-{ts}.log"));
    if let Ok(mut f) = fs::File::create(&path) {
        let _ = writeln!(f, "{text}");
    }
    path
}

pub fn write_crash(info: &PanicHookInfo<'_>) -> PathBuf {
    write_crash_text(&info.to_string())
}

pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = write_crash(info);
        prev(info);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_local_file_and_nothing_is_uploaded() {
        let p = write_crash_text("boom");
        assert!(p.is_absolute());
        assert_eq!(fs::read_to_string(&p).unwrap().trim(), "boom");
        let s = p.to_string_lossy();
        assert!(!s.contains("http://") && !s.contains("https://"));
        let _ = fs::remove_file(&p);
    }
}
