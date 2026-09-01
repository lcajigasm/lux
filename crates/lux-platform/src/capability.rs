//! Child-process capability: false on iOS/iPadOS (ADR 0006). Android
//! *can* spawn processes but v1.2.0 still does not ship PTY/LSP UI.

pub fn child_processes_allowed() -> bool {
    !cfg!(target_os = "ios")
}

pub fn pty_and_lsp_ui_shipped() -> bool {
    cfg!(any(
        target_os = "macos",
        target_os = "windows",
        target_os = "linux"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_ships_pty_ui_ios_does_not_allow_child_processes() {
        assert_eq!(child_processes_allowed(), !cfg!(target_os = "ios"));
        assert_eq!(
            pty_and_lsp_ui_shipped(),
            cfg!(any(
                target_os = "macos",
                target_os = "windows",
                target_os = "linux"
            ))
        );
    }
}
