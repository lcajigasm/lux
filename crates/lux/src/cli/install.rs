//! Install and remove the `lux` command on PATH.
//!
//! The link points at `Lux.app/Contents/SharedSupport/bin/lux` when this
//! process is the app, and at the current executable otherwise.
//! `/usr/local/bin` is used when it is writable. Otherwise the link is
//! `~/.local/bin/lux`.

use std::path::{Path, PathBuf};

use super::is_bundle_cli;

pub(crate) fn command_source() -> PathBuf {
    let Ok(exe) = std::env::current_exe() else {
        return PathBuf::from("lux");
    };
    let exe = exe.canonicalize().unwrap_or(exe);
    if is_bundle_cli(&exe) {
        return exe;
    }
    // Contents/MacOS/lux -> Contents/SharedSupport/bin/lux
    if let Some(macos) = exe.parent()
        && macos.file_name().is_some_and(|name| name == "MacOS")
        && let Some(contents) = macos.parent()
    {
        let cli = contents.join("SharedSupport").join("bin").join("lux");
        if cli.is_file() {
            return cli;
        }
    }
    exe
}

pub(crate) fn install_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LUX_BIN_DIR") {
        return PathBuf::from(dir);
    }
    let local = PathBuf::from("/usr/local/bin");
    if unix_writable(&local) {
        return local;
    }
    home_bin()
}

fn home_bin() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join(".local")
        .join("bin")
}

fn unix_writable(dir: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(dir)
            .map(|meta| meta.permissions().mode() & 0o200 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        let _ = dir;
        false
    }
}

pub(crate) fn install_link(dir: &Path, source: &Path) -> Result<PathBuf, String> {
    #[cfg(not(unix))]
    {
        let _ = (dir, source);
        return Err("The lux command is installed from the macOS or Linux build.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        let dest = dir.join("lux");
        if dest.symlink_metadata().is_ok() {
            let linked = std::fs::read_link(&dest).unwrap_or_default();
            if linked != source {
                if !dest
                    .symlink_metadata()
                    .map(|meta| meta.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    return Err(format!("{} exists and is not a symlink", dest.display()));
                }
                std::fs::remove_file(&dest).map_err(|error| error.to_string())?;
            } else {
                return Ok(dest);
            }
        }
        symlink(source, &dest).map_err(|error| error.to_string())?;
        Ok(dest)
    }
}

pub(crate) fn uninstall_link(dir: &Path) -> Result<PathBuf, String> {
    let dest = dir.join("lux");
    let meta = std::fs::symlink_metadata(&dest)
        .map_err(|_| format!("no lux command in {}", dir.display()))?;
    if !meta.file_type().is_symlink() {
        return Err(format!("{} is not a symlink", dest.display()));
    }
    std::fs::remove_file(&dest).map_err(|error| error.to_string())?;
    Ok(dest)
}

pub(crate) fn install_command() -> Result<String, String> {
    let source = command_source();
    if !source.is_file() {
        return Err(format!("missing {}", source.display()));
    }
    match install_link(&install_dir(), &source) {
        Ok(path) => Ok(format!("Installed lux at {}", path.display())),
        Err(first) => {
            let path =
                install_link(&home_bin(), &source).map_err(|error| format!("{first}; {error}"))?;
            Ok(format!(
                "Installed lux at {}. Add that directory to PATH.",
                path.display()
            ))
        }
    }
}

pub(crate) fn uninstall_command() -> Result<String, String> {
    let mut removed = Vec::new();
    for dir in [install_dir(), home_bin(), PathBuf::from("/usr/local/bin")] {
        if let Ok(path) = uninstall_link(&dir) {
            removed.push(path.display().to_string());
        }
    }
    if removed.is_empty() {
        return Err("no lux command link was installed".into());
    }
    Ok(format!("Removed {}", removed.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_uninstall_round_trip_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("SharedSupport-lux");
        std::fs::write(&source, b"#!/bin/sh\n").unwrap();
        let bin = dir.path().join("bin");
        let linked = install_link(&bin, &source).unwrap();
        assert_eq!(std::fs::read_link(&linked).unwrap(), source);
        let again = install_link(&bin, &source).unwrap();
        assert_eq!(again, linked);
        let removed = uninstall_link(&bin).unwrap();
        assert_eq!(removed, linked);
        assert!(!linked.exists());
    }

    #[test]
    fn the_packaged_cask_links_the_bundle_cli() {
        let text = include_str!("../../../../packaging/homebrew/lux.rb");
        assert!(text.contains("binary"));
        assert!(text.contains("Contents/SharedSupport/bin/lux"));
    }
}
