//! Tree-sitter based incremental syntax highlighting.
//!
//! Grammars are vendored and compiled directly into this crate's binary
//! output (see `docs/adr/0008-tree-sitter-grammars-compiled-in.md`) rather
//! than pulled in as independently-versioned crates, to avoid tree-sitter
//! upstream issue #3095 (ABI/version drift between the core crate and
//! grammar crates).

use std::ops::Range;
use tree_sitter::{InputEdit, Language, Parser, Query, QueryCursor, StreamingIterator, Tree};

pub mod brackets;
pub mod folding;
pub mod indent_rules;
pub mod symbols;
pub mod theme;

pub use symbols::{
    DocumentSymbol, SymbolKind, document_symbols, extract_fallback_symbols,
    extract_symbols_from_source,
};

/// One highlighted span: a byte range plus the highlight query capture name
/// that matched it (e.g. `"keyword"`, `"function"`, `"string"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: String,
}

/// A language with a grammar compiled into this binary. Phase 3's fixed set
/// (see `docs/adr/0008-tree-sitter-grammars-compiled-in.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LanguageId {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    Go,
    C,
    Cpp,
    Java,
    Kotlin,
    Swift,
    Html,
    Css,
    Json,
    Yaml,
    Markdown,
    Bash,
    Toml,
}

impl LanguageId {
    /// Maps a file extension (without the leading dot) to its language, if
    /// this build has a grammar for it.
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Self::Rust),
            "ts" | "mts" | "cts" => Some(Self::TypeScript),
            "tsx" => Some(Self::Tsx),
            "js" | "mjs" | "cjs" | "jsx" => Some(Self::JavaScript),
            "py" | "pyi" => Some(Self::Python),
            "go" => Some(Self::Go),
            "c" | "h" => Some(Self::C),
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => Some(Self::Cpp),
            "java" => Some(Self::Java),
            "kt" | "kts" => Some(Self::Kotlin),
            "swift" => Some(Self::Swift),
            "html" | "htm" => Some(Self::Html),
            "css" => Some(Self::Css),
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "md" | "markdown" => Some(Self::Markdown),
            "sh" | "bash" => Some(Self::Bash),
            "toml" => Some(Self::Toml),
            _ => None,
        }
    }

    fn ts_language(self) -> Language {
        macro_rules! lang_fn {
            ($name:ident) => {{
                unsafe extern "C" {
                    fn $name() -> *const ();
                }
                Language::new(unsafe { tree_sitter_language::LanguageFn::from_raw($name) })
            }};
        }
        match self {
            Self::Rust => lang_fn!(tree_sitter_rust),
            Self::TypeScript => lang_fn!(tree_sitter_typescript),
            Self::Tsx => lang_fn!(tree_sitter_tsx),
            Self::JavaScript => lang_fn!(tree_sitter_javascript),
            Self::Python => lang_fn!(tree_sitter_python),
            Self::Go => lang_fn!(tree_sitter_go),
            Self::C => lang_fn!(tree_sitter_c),
            Self::Cpp => lang_fn!(tree_sitter_cpp),
            Self::Java => lang_fn!(tree_sitter_java),
            Self::Kotlin => lang_fn!(tree_sitter_kotlin),
            Self::Swift => lang_fn!(tree_sitter_swift),
            Self::Html => lang_fn!(tree_sitter_html),
            Self::Css => lang_fn!(tree_sitter_css),
            Self::Json => lang_fn!(tree_sitter_json),
            Self::Yaml => lang_fn!(tree_sitter_yaml),
            Self::Markdown => lang_fn!(tree_sitter_markdown),
            Self::Bash => lang_fn!(tree_sitter_bash),
            Self::Toml => lang_fn!(tree_sitter_toml),
        }
    }

    /// The highlight query source for this language. TypeScript, TSX and
    /// C++ ship grammar-repo `highlights.scm` files that only cover their
    /// *own* additions on top of a base grammar (JavaScript, JavaScript,
    /// and C respectively) — the same layering nvim-treesitter/helix apply
    /// at their query-loader level, done here at compile time instead via
    /// `concat!`, since both base and extension grammars share the same
    /// underlying node-type names.
    fn highlights_query(self) -> &'static str {
        match self {
            Self::Rust => include_str!("../queries/rust/highlights.scm"),
            Self::TypeScript => concat!(
                include_str!("../queries/javascript/highlights.scm"),
                "\n",
                include_str!("../queries/typescript/highlights.scm"),
            ),
            Self::Tsx => concat!(
                include_str!("../queries/javascript/highlights.scm"),
                "\n",
                include_str!("../queries/typescript/highlights.scm"),
                "\n",
                include_str!("../queries/tsx/highlights.scm"),
            ),
            Self::JavaScript => include_str!("../queries/javascript/highlights.scm"),
            Self::Python => include_str!("../queries/python/highlights.scm"),
            Self::Go => include_str!("../queries/go/highlights.scm"),
            Self::C => include_str!("../queries/c/highlights.scm"),
            Self::Cpp => concat!(
                include_str!("../queries/c/highlights.scm"),
                "\n",
                include_str!("../queries/cpp/highlights.scm"),
            ),
            Self::Java => include_str!("../queries/java/highlights.scm"),
            Self::Kotlin => include_str!("../queries/kotlin/highlights.scm"),
            Self::Swift => include_str!("../queries/swift/highlights.scm"),
            Self::Html => include_str!("../queries/html/highlights.scm"),
            Self::Css => include_str!("../queries/css/highlights.scm"),
            Self::Json => include_str!("../queries/json/highlights.scm"),
            Self::Yaml => include_str!("../queries/yaml/highlights.scm"),
            Self::Markdown => include_str!("../queries/markdown/highlights.scm"),
            Self::Bash => include_str!("../queries/bash/highlights.scm"),
            Self::Toml => include_str!("../queries/toml/highlights.scm"),
        }
    }
}

/// An incrementally-parsed document: owns the parser, the last parse tree
/// (so an edit only reparses the changed range instead of the whole file —
/// `Parser::parse` reuses the unchanged subtrees of the old tree), and the
/// compiled highlight query. The query is compiled once here, not on every
/// `highlight`/`highlight_range` call: `Query::new` parses and validates
/// the language's entire `highlights.scm` pattern set (hundreds of lines),
/// which measured as the dominant cost in `highlight_range`'s benchmark —
/// recompiling it per keystroke would undo most of the benefit of scoping
/// the query to the visible range in the first place.
pub struct Document {
    language: LanguageId,
    parser: Parser,
    tree: Option<Tree>,
    highlight_query: Query,
}

impl Document {
    #[must_use]
    pub fn new(language: LanguageId) -> Self {
        let mut parser = Parser::new();
        parser
            .set_language(&language.ts_language())
            .expect("vendored grammar ABI must match the compiled-in tree-sitter core version");
        let highlight_query = Query::new(&language.ts_language(), language.highlights_query())
            .expect("vendored highlights.scm must be valid for its own grammar");
        Self {
            language,
            parser,
            tree: None,
            highlight_query,
        }
    }

    /// Parses `source` from scratch, discarding any previous tree.
    pub fn set_text(&mut self, source: &str) {
        self.tree = self.parser.parse(source, None);
    }

    /// Applies `edit` to the previous tree, then reparses `new_source`.
    /// Tree-sitter reuses the subtrees outside the edited range, so this is
    /// a range-local reparse, not a full-file one.
    pub fn edit(&mut self, edit: &InputEdit, new_source: &str) {
        if let Some(tree) = self.tree.as_mut() {
            tree.edit(edit);
        }
        self.tree = self.parser.parse(new_source, self.tree.as_ref());
    }

    /// Runs this document's language's highlight query over the current
    /// tree, returning every capture as a token ordered by position.
    ///
    /// Scans the *whole* tree — correct, but expensive on a large file (see
    /// `highlight_range`'s doc comment for the actual measured cost and why
    /// it exists). Fine for one-shot uses (golden token tests, opening a
    /// file); a per-keystroke UI should call `highlight_range` with the
    /// visible byte range instead.
    #[must_use]
    pub fn highlight(&self, source: &str) -> Vec<Token> {
        self.highlight_range(source, 0..source.len())
    }

    /// Like [`Self::highlight`], scoped to `byte_range` — the tree isn't
    /// re-walked outside it (`tree_sitter::QueryCursor::set_byte_range`,
    /// not a post-hoc filter). Exists because `highlight`'s whole-tree scan
    /// measured at ~26 ms reparsing + re-highlighting after one keystroke in
    /// a 20k-line file (`benches/benches/phase3_budgets.rs`), well over the
    /// 16 ms frame budget `LUX_PROMPT_MAESTRO.md` sets for exactly that
    /// scenario — a real UI must re-highlight only the visible range after
    /// an edit, not the whole document, the same reason `EditorView`
    /// (Phase 2) only shapes glyphs for `visible_line_range`.
    #[must_use]
    pub fn highlight_range(&self, source: &str, byte_range: Range<usize>) -> Vec<Token> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(byte_range);
        let mut tokens = Vec::new();
        let mut captures =
            cursor.captures(&self.highlight_query, tree.root_node(), source.as_bytes());
        while let Some((mat, index)) = captures.next() {
            let capture = mat.captures[*index];
            tokens.push(Token {
                start_byte: capture.node.start_byte(),
                end_byte: capture.node.end_byte(),
                kind: self.highlight_query.capture_names()[capture.index as usize].to_string(),
            });
        }
        tokens.sort_by_key(|t| (t.start_byte, t.end_byte));
        tokens
    }

    /// The current parse tree, for modules (`brackets`, `folding`,
    /// `indent_rules`) that walk its structure directly instead of going
    /// through a highlight query. `None` before the first [`Self::set_text`].
    pub(crate) fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    /// This document's language, for modules that need per-grammar
    /// configuration (`indent_rules`).
    pub fn language(&self) -> LanguageId {
        self.language
    }
}

/// A unit of work for the background parse worker: either a fresh file or
/// an edit applied to the document it already holds.
pub enum Job {
    SetText(String),
    Edit { edit: InputEdit, source: String },
}

/// Runs parsing and highlighting on a dedicated thread, so the caller (the
/// UI thread, per golden rule #6) never blocks on tree-sitter work. Driven
/// entirely by a blocking channel receive — no polling (golden rule #5).
pub struct BackgroundHighlighter {
    jobs: Option<std::sync::mpsc::Sender<Job>>,
    results: std::sync::mpsc::Receiver<Vec<Token>>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl BackgroundHighlighter {
    #[must_use]
    pub fn spawn(language: LanguageId) -> Self {
        let (job_tx, job_rx) = std::sync::mpsc::channel::<Job>();
        let (result_tx, result_rx) = std::sync::mpsc::channel::<Vec<Token>>();
        let worker = std::thread::spawn(move || {
            let mut doc = Document::new(language);
            for job in job_rx {
                let source = match job {
                    Job::SetText(source) => {
                        doc.set_text(&source);
                        source
                    }
                    Job::Edit { edit, source } => {
                        doc.edit(&edit, &source);
                        source
                    }
                };
                if result_tx.send(doc.highlight(&source)).is_err() {
                    break;
                }
            }
        });
        Self {
            jobs: Some(job_tx),
            results: result_rx,
            worker: Some(worker),
        }
    }

    /// Queues a job. Never blocks the caller.
    pub fn submit(&self, job: Job) {
        if let Some(jobs) = self.jobs.as_ref() {
            let _ = jobs.send(job);
        }
    }

    /// Blocks until the worker finishes the next queued job.
    pub fn recv(&self) -> Option<Vec<Token>> {
        self.results.recv().ok()
    }
}

impl Drop for BackgroundHighlighter {
    fn drop(&mut self) {
        // Drop the sender first: it closes the channel, which ends the
        // worker's `for job in job_rx` loop so the join below doesn't block
        // forever waiting on a thread that's still waiting for more jobs.
        self.jobs.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_delegates_to_highlight_range_over_the_whole_source() {
        let source = "fn main() {\n    let x = 1;\n}\n";
        let mut doc = Document::new(LanguageId::Rust);
        doc.set_text(source);
        assert_eq!(
            doc.highlight(source),
            doc.highlight_range(source, 0..source.len())
        );
    }

    #[test]
    fn highlight_range_excludes_tokens_outside_the_range() {
        let source = "fn one() {}\nfn two() {}\nfn three() {}\n";
        let mut doc = Document::new(LanguageId::Rust);
        doc.set_text(source);

        let full = doc.highlight(source);
        assert!(
            full.len() > 3,
            "expected multiple tokens across 3 functions"
        );

        // Scope to just the second line ("fn two() {}").
        let second_line_start = source.find("fn two").unwrap();
        let second_line_end = source.find("fn three").unwrap();
        let scoped = doc.highlight_range(source, second_line_start..second_line_end);

        assert!(!scoped.is_empty());
        assert!(scoped.len() < full.len());
        for token in &scoped {
            assert!(token.start_byte >= second_line_start);
            assert!(token.end_byte <= second_line_end);
        }
    }

    #[test]
    fn highlight_range_on_a_never_parsed_document_is_empty() {
        let doc = Document::new(LanguageId::Rust);
        assert_eq!(doc.highlight_range("fn f() {}", 0..9), Vec::new());
    }
}
