//! Locating language server binaries and mapping languages to them.

use std::env;
use std::path::{Path, PathBuf};

/// How to launch the language server for one language.
#[derive(Debug, Clone, Copy)]
pub struct ServerSpec {
    pub language_id: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
}

/// The five servers Phase 7's acceptance criteria require support for.
pub const KNOWN_SERVERS: &[ServerSpec] = &[
    ServerSpec {
        language_id: "rust",
        command: "rust-analyzer",
        args: &[],
    },
    ServerSpec {
        language_id: "typescript",
        command: "typescript-language-server",
        args: &["--stdio"],
    },
    ServerSpec {
        language_id: "python",
        command: "pyright-langserver",
        args: &["--stdio"],
    },
    ServerSpec {
        language_id: "go",
        command: "gopls",
        args: &[],
    },
    ServerSpec {
        language_id: "c",
        command: "clangd",
        args: &[],
    },
];

/// Look up the launch spec for a language id (e.g. `"rust"`).
pub fn spec_for_language(language_id: &str) -> Option<&'static ServerSpec> {
    KNOWN_SERVERS
        .iter()
        .find(|spec| spec.language_id == language_id)
}

/// Search `PATH` for an executable named `command`, the same way the OS
/// would resolve it when spawning — but ahead of time, so a missing server
/// can be reported as a clear status instead of an opaque spawn failure.
pub fn discover_command(command: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var).find_map(|dir| {
        let candidate = dir.join(command);
        is_executable(&candidate).then_some(candidate)
    })
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    // ponytail: PATHEXT-aware lookup (.exe/.cmd/.bat) if a Windows server
    // ever ships without the extension pre-attached to `ServerSpec::command`.
    path.with_extension("exe").is_file() || path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_binary_known_to_be_on_path() {
        let found = discover_command("cargo").expect("cargo should be on PATH for tests");
        assert!(found.is_file());
    }

    #[test]
    fn reports_missing_binary_as_none() {
        assert!(discover_command("lux-lsp-definitely-not-a-real-binary").is_none());
    }

    #[test]
    fn known_servers_cover_the_five_required_languages() {
        for lang in ["rust", "typescript", "python", "go", "c"] {
            assert!(spec_for_language(lang).is_some(), "missing spec for {lang}");
        }
    }
}
