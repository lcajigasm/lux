//! Snippet registry, storage, scope resolution, and filesystem loader.

use super::builtins;
use super::parser::{RawSnippet, parse_json_snippets, parse_sublime_snippet};
use std::path::Path;

/// An indexed snippet available for tab expansion or palette insertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub name: String,
    pub trigger: String,
    pub content: String,
    pub scope: Option<String>,
    pub description: String,
}

/// The global registry of snippets available in the Lux editor.
#[derive(Debug, Clone, Default)]
pub struct SnippetRegistry {
    snippets: Vec<Snippet>,
}

impl SnippetRegistry {
    /// Creates a new registry initialized with the standard built-in snippets.
    pub fn new() -> Self {
        let mut reg = Self {
            snippets: Vec::new(),
        };
        for raw in builtins::default_snippets() {
            reg.add_raw(raw);
        }
        reg
    }

    /// Adds a raw snippet definition into the registry.
    pub fn add_raw(&mut self, raw: RawSnippet) {
        let description = raw.description.unwrap_or_else(|| raw.name.clone());
        self.snippets.push(Snippet {
            name: raw.name,
            trigger: raw.trigger,
            content: raw.content,
            scope: raw.scope,
            description,
        });
    }

    /// Returns a slice of all registered snippets.
    #[allow(dead_code)]
    pub fn all(&self) -> &[Snippet] {
        &self.snippets
    }

    /// Finds a snippet matching the given `trigger` that is valid for `doc_scope`.
    /// When multiple match, prefers language-scoped snippets over universal ones.
    pub fn find_by_trigger(&self, trigger: &str, doc_scope: Option<&str>) -> Option<&Snippet> {
        let mut best: Option<&Snippet> = None;

        for snippet in &self.snippets {
            if snippet.trigger == trigger && scope_matches(snippet.scope.as_deref(), doc_scope) {
                // If this is scoped and previous was unscoped, prefer the scoped one
                if snippet.scope.is_some() {
                    return Some(snippet);
                }
                if best.is_none() {
                    best = Some(snippet);
                }
            }
        }

        best
    }

    /// Returns all snippets available for the given document scope.
    pub fn for_scope(&self, doc_scope: Option<&str>) -> Vec<&Snippet> {
        let mut matched: Vec<&Snippet> = self
            .snippets
            .iter()
            .filter(|s| scope_matches(s.scope.as_deref(), doc_scope))
            .collect();
        matched.sort_by(|a, b| a.trigger.cmp(&b.trigger).then(a.name.cmp(&b.name)));
        matched
    }

    /// Scans a directory for `.sublime-snippet` and `.json` snippet files.
    pub fn load_from_dir(&mut self, dir: &Path) {
        if !dir.is_dir() {
            return;
        }
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                self.load_file(&path);
            } else if path.is_dir() {
                self.load_from_dir(&path);
            }
        }
    }

    /// Loads a single snippet file based on its extension.
    pub fn load_file(&mut self, path: &Path) {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("snippet");

        if path.extension().is_some_and(|ext| ext == "sublime-snippet") {
            if let Some(raw) = parse_sublime_snippet(&content, file_stem) {
                self.add_raw(raw);
            }
        } else if path.extension().is_some_and(|ext| ext == "json") {
            let parsed = parse_json_snippets(&content, file_stem);
            for raw in parsed {
                self.add_raw(raw);
            }
        }
    }

    /// Loads custom user and workspace snippets from standard directories.
    pub fn load_custom_snippets(&mut self, workspace_root: Option<&Path>) {
        if let Some(config_dir) = dirs_next_config_dir() {
            self.load_from_dir(&config_dir.join("lux").join("snippets"));
        }
        if let Some(home_dir) = std::env::var_os("HOME").map(std::path::PathBuf::from) {
            self.load_from_dir(&home_dir.join(".lux").join("snippets"));
        }
        if let Some(root) = workspace_root {
            self.load_from_dir(&root.join(".lux").join("snippets"));
        }
    }
}

fn dirs_next_config_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
}

/// Checks whether a snippet's scope selector matches the active document scope.
pub fn scope_matches(snippet_scope: Option<&str>, doc_scope: Option<&str>) -> bool {
    let Some(snippet_scope) = snippet_scope else {
        return true; // No scope means universal
    };

    let Some(doc_scope) = doc_scope else {
        return false;
    };

    let doc_scope_lower = doc_scope.to_lowercase();
    let doc_variants = normalize_scope(&doc_scope_lower);

    for part in snippet_scope.split(',') {
        let part = part.trim().to_lowercase();
        if part.is_empty() {
            continue;
        }

        if doc_scope_lower == part
            || doc_scope_lower.starts_with(&part)
            || part.starts_with(&doc_scope_lower)
        {
            return true;
        }

        let part_variants = normalize_scope(&part);
        for pv in &part_variants {
            for dv in &doc_variants {
                if pv == dv || pv.starts_with(dv) || dv.starts_with(pv) {
                    return true;
                }
            }
        }
    }

    false
}

fn normalize_scope(scope: &str) -> Vec<&str> {
    let s = scope
        .strip_prefix("source.")
        .or_else(|| scope.strip_prefix("text."))
        .unwrap_or(scope);
    match s {
        "ts" | "typescript" => vec!["ts", "typescript"],
        "js" | "javascript" => vec!["js", "javascript"],
        "py" | "python" => vec!["py", "python"],
        "rs" | "rust" => vec!["rs", "rust"],
        "rb" | "ruby" => vec!["rb", "ruby"],
        "sh" | "bash" | "shell" => vec!["sh", "bash", "shell"],
        "cpp" | "c++" => vec!["cpp", "c++"],
        "md" | "markdown" => vec!["md", "markdown"],
        other => vec![other],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_matching_rules() {
        assert!(scope_matches(None, Some("source.rust")));
        assert!(scope_matches(Some("source.rust"), Some("source.rust")));
        assert!(scope_matches(Some("source.rust"), Some("rust")));
        assert!(scope_matches(
            Some("source.js, source.ts"),
            Some("typescript")
        ));
        assert!(scope_matches(
            Some("source.js, source.ts"),
            Some("source.ts")
        ));
        assert!(!scope_matches(Some("source.rust"), Some("python")));
    }

    #[test]
    fn registry_trigger_lookup_and_scope_filtering() {
        let reg = SnippetRegistry::new();

        // Rust snippet
        let fn_snippet = reg.find_by_trigger("fn", Some("source.rust"));
        assert!(fn_snippet.is_some());
        assert_eq!(fn_snippet.unwrap().trigger, "fn");
        assert!(fn_snippet.unwrap().content.contains("fn ${1:name}"));

        // Universal snippet
        let lorem = reg.find_by_trigger("lorem", Some("source.rust"));
        assert!(lorem.is_some());
        assert_eq!(lorem.unwrap().trigger, "lorem");

        // Non-matching trigger
        assert!(
            reg.find_by_trigger("nonexistent_snippet", Some("source.rust"))
                .is_none()
        );
    }

    #[test]
    fn load_custom_sublime_snippet_from_dir() {
        let temp = tempfile::tempdir().unwrap();
        let snippet_file = temp.path().join("my_macro.sublime-snippet");
        std::fs::write(
            &snippet_file,
            r#"<snippet>
    <content><![CDATA[macro_rules! ${1:name} { () => { $0 }; }]]></content>
    <tabTrigger>mac</tabTrigger>
    <scope>source.rust</scope>
    <description>Macro rules</description>
</snippet>"#,
        )
        .unwrap();

        let mut reg = SnippetRegistry::new();
        reg.load_from_dir(temp.path());

        let found = reg.find_by_trigger("mac", Some("source.rust")).unwrap();
        assert_eq!(found.trigger, "mac");
        assert_eq!(found.description, "Macro rules");
        assert_eq!(found.content, "macro_rules! ${1:name} { () => { $0 }; }");
    }
}
