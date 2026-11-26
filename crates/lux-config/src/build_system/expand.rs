//! Sublime build variables: `$name`, `${name}` and `${name:default}`,
//! with `\$` for a literal dollar sign. Unknown names expand to the empty
//! string, as in Sublime, so shell variables need `\\$HOME` in a
//! `.sublime-build` file. `${packages}` is not supported: Lux has no
//! Packages directory. The `${var/regex/replace/}` form is not supported.

use std::path::{Path, PathBuf};

/// What the variables expand from: the active file and the open project.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExpandContext {
    pub file: Option<PathBuf>,
    /// The first folder of the window (the workspace root).
    pub folder: Option<PathBuf>,
    /// The `.sublime-project` / `.lux-project` file, when one is open.
    pub project: Option<PathBuf>,
}

impl ExpandContext {
    /// The value of one variable, or `None` when it is unknown or unset.
    pub fn lookup(&self, name: &str) -> Option<String> {
        let file = self.file.as_deref();
        let project = self.project.as_deref();
        match name {
            "file" => file.map(display),
            "file_path" => file.and_then(Path::parent).map(display),
            "file_name" => file.and_then(Path::file_name).map(os),
            "file_base_name" => file.and_then(Path::file_stem).map(os),
            "file_extension" => file.and_then(Path::extension).map(os),
            "folder" => self.folder.as_deref().map(display),
            "project" => project.map(display),
            "project_path" => project.and_then(Path::parent).map(display),
            "project_name" => project.and_then(Path::file_name).map(os),
            "project_base_name" => project.and_then(Path::file_stem).map(os),
            "project_extension" => project.and_then(Path::extension).map(os),
            "platform" => Some(platform().to_string()),
            _ => None,
        }
    }
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn os(part: &std::ffi::OsStr) -> String {
    part.to_string_lossy().into_owned()
}

/// Sublime's names: `OSX`, `Windows`, `Linux`.
fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "OSX"
    } else if cfg!(windows) {
        "Windows"
    } else {
        "Linux"
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Expands every variable in `text` against `context`.
pub fn expand(text: &str, context: &ExpandContext) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' if text[i + 1..].starts_with('$') => {
                out.push('$');
                chars.next();
            }
            '$' => {
                let rest = &text[i + 1..];
                if let Some(body) = rest.strip_prefix('{')
                    && let Some(end) = closing_brace(body)
                {
                    let inner = &body[..end];
                    let (name, default) = match inner.split_once(':') {
                        Some((name, default)) => (name, Some(default)),
                        None => (inner, None),
                    };
                    match context.lookup(name).filter(|v| !v.is_empty()) {
                        Some(value) => out.push_str(&value),
                        None => out.push_str(&expand(default.unwrap_or(""), context)),
                    }
                    // Skip `{`, the body and `}`.
                    let skip = end + 2;
                    while chars.peek().is_some_and(|(j, _)| *j <= i + skip) {
                        chars.next();
                    }
                } else {
                    let len = rest.find(|c| !is_name_char(c)).unwrap_or(rest.len());
                    if len == 0 {
                        out.push('$');
                        continue;
                    }
                    out.push_str(&context.lookup(&rest[..len]).unwrap_or_default());
                    while chars.peek().is_some_and(|(j, _)| *j <= i + len) {
                        chars.next();
                    }
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Byte index of the `}` closing a `${`, allowing nested `${...}` in a
/// default value.
fn closing_brace(body: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in body.char_indices() {
        match c {
            '{' => depth += 1,
            '}' if depth == 0 => return Some(i),
            '}' => depth -= 1,
            _ => {}
        }
    }
    None
}
