//! Chrome that actually calls domain crates (git, term links, plugins).
//! Terminal/LSP UI is desktop-only (ADR 0006).

use std::path::Path;

use lux_git::status::{RepositoryStatus, status_at};
use lux_plugin::PermissionSet;
use lux_term::detect_links;

use crate::focus::FocusTarget;

/// PTY + LSP panels exist only where child processes are allowed.
pub fn pty_and_lsp_ui_allowed() -> bool {
    cfg!(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    ))
}

pub fn git_status_line(path: &Path) -> Option<String> {
    let st: RepositoryStatus = status_at(path).ok()?;
    let branch = match &st.branch {
        lux_git::status::BranchState::Named(n) => n.clone(),
        lux_git::status::BranchState::Unborn(n) => n.clone(),
        lux_git::status::BranchState::Detached(id) => format!("detached {}", id),
    };
    Some(format!("{branch} · {}", st.changed_files))
}

pub fn links_in_output(text: &str) -> usize {
    detect_links(text).len()
}

pub fn default_plugin_grants() -> PermissionSet {
    PermissionSet::new([])
}

pub fn terminal_focus_allowed(target: FocusTarget) -> bool {
    target != FocusTarget::Terminal || pty_and_lsp_ui_allowed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_reads_this_repo() {
        let line = git_status_line(Path::new(env!("CARGO_MANIFEST_DIR")));
        assert!(line.is_some(), "lux-ui lives inside the lux git repo");
    }

    #[test]
    fn term_link_detection_is_live() {
        assert_eq!(links_in_output(" --> crates/lux-ui/src/lib.rs:1:1"), 1);
    }

    #[test]
    fn plugins_start_with_no_grants() {
        assert!(!default_plugin_grants().allows(lux_plugin::Capability::Network));
    }
}
