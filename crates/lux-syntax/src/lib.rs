//! Tree-sitter based incremental syntax highlighting.
//!
//! Grammars are vendored and compiled directly into this crate's binary
//! output (see `docs/adr/0008-tree-sitter-grammars-compiled-in.md`) rather
//! than pulled in as independently-versioned crates, to avoid tree-sitter
//! upstream issue #3095 (ABI/version drift between the core crate and
//! grammar crates).

use std::ops::Range;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator, Tree};

pub mod brackets;
pub mod folding;
pub mod indent_rules;
pub mod symbols;
pub mod theme;

pub use symbols::{
    DocumentSymbol, SymbolKind, document_symbols, extract_fallback_symbols,
    extract_symbols_from_source,
};
pub use tree_sitter::{InputEdit, Point};

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

    /// Parses from a `ropey::Rope` using chunk-based streaming without materializing
    /// full text copies.
    pub fn set_rope(&mut self, rope: &ropey::Rope) {
        self.tree = self.parser.parse_with_options(
            &mut |offset, _| {
                if offset >= rope.len_bytes() {
                    return &[] as &[u8];
                }
                let (chunk, chunk_start, _, _) = rope.chunk_at_byte(offset);
                &chunk.as_bytes()[offset - chunk_start..]
            },
            None,
            None,
        );
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

    /// Applies `edit` to the previous tree, then reparses `rope` incrementally
    /// using chunk-based streaming without materializing full text copies.
    pub fn edit_rope(&mut self, edit: &InputEdit, rope: &ropey::Rope) {
        if let Some(tree) = self.tree.as_mut() {
            tree.edit(edit);
        }
        self.tree = self.parser.parse_with_options(
            &mut |offset, _| {
                if offset >= rope.len_bytes() {
                    return &[] as &[u8];
                }
                let (chunk, chunk_start, _, _) = rope.chunk_at_byte(offset);
                &chunk.as_bytes()[offset - chunk_start..]
            },
            self.tree.as_ref(),
            None,
        );
    }

    /// Applies multiple `edits` to the previous tree in sequence, then reparses `rope`
    /// incrementally using chunk-based streaming.
    pub fn edit_rope_batch(&mut self, edits: &[InputEdit], rope: &ropey::Rope) {
        if let Some(tree) = self.tree.as_mut() {
            for edit in edits {
                tree.edit(edit);
            }
        }
        self.tree = self.parser.parse_with_options(
            &mut |offset, _| {
                if offset >= rope.len_bytes() {
                    return &[] as &[u8];
                }
                let (chunk, chunk_start, _, _) = rope.chunk_at_byte(offset);
                &chunk.as_bytes()[offset - chunk_start..]
            },
            self.tree.as_ref(),
            None,
        );
    }

    /// Runs this document's language's highlight query over the current
    /// tree, returning every capture as a token ordered by position.
    #[must_use]
    pub fn highlight(&self, source: &str) -> Vec<Token> {
        self.highlight_range(source, 0..source.len())
    }

    /// Scans the entire tree with highlight queries over `rope` chunks.
    #[must_use]
    pub fn highlight_rope(&self, rope: &ropey::Rope) -> Vec<Token> {
        self.highlight_range_rope(rope, 0..rope.len_bytes())
    }

    /// Like [`Self::highlight`], scoped to `byte_range`.
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

    /// Scoped highlight over `rope` chunks without intermediate string allocations.
    #[must_use]
    pub fn highlight_range_rope(&self, rope: &ropey::Rope, byte_range: Range<usize>) -> Vec<Token> {
        let Some(tree) = self.tree.as_ref() else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(byte_range);
        let mut tokens = Vec::new();
        let mut text_provider = |node: tree_sitter::Node<'_>| {
            let range = node.byte_range();
            let start = range.start.min(rope.len_bytes());
            let end = range.end.min(rope.len_bytes());
            let slice_range = if start <= end {
                start..end
            } else {
                start..start
            };
            rope.byte_slice(slice_range).chunks()
        };
        let mut captures =
            cursor.captures(&self.highlight_query, tree.root_node(), &mut text_provider);
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
    pub fn tree(&self) -> Option<&Tree> {
        self.tree.as_ref()
    }

    /// This document's language, for modules that need per-grammar
    /// configuration (`indent_rules`).
    pub fn language(&self) -> LanguageId {
        self.language
    }
}

/// Result returned by the background parse worker to the UI thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightResult {
    pub tokens: Vec<Token>,
    pub fold_ranges: Vec<folding::FoldRange>,
    pub version: usize,
}

/// A unit of work for the background parse worker: either a fresh file/rope or
/// an edit applied to the document it already holds.
#[derive(Debug, Clone)]
pub enum Job {
    SetRope {
        rope: ropey::Rope,
        version: usize,
    },
    Edit {
        edits: Vec<InputEdit>,
        rope: ropey::Rope,
        version: usize,
    },
}

/// Runs parsing and highlighting on a dedicated thread, so the caller (the
/// UI thread, per golden rule #6) never blocks on tree-sitter work. Driven
/// entirely by non-blocking / event-driven channels (golden rule #5).
pub struct BackgroundHighlighter {
    jobs: Option<crossbeam_channel::Sender<Job>>,
    results: crossbeam_channel::Receiver<HighlightResult>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl BackgroundHighlighter {
    #[must_use]
    pub fn spawn(
        language: LanguageId,
        waker: Option<std::sync::Arc<dyn Fn() + Send + Sync + 'static>>,
    ) -> Self {
        let (job_tx, job_rx) = crossbeam_channel::unbounded::<Job>();
        let (result_tx, result_rx) = crossbeam_channel::unbounded::<HighlightResult>();
        let worker = std::thread::spawn(move || {
            let mut doc = Document::new(language);
            while let Ok(job) = job_rx.recv() {
                let mut current_job = Some(job);
                let mut current_rope = ropey::Rope::new();
                let mut latest_version = 0;
                while let Some(j) = current_job {
                    match j {
                        Job::SetRope { rope, version } => {
                            doc.set_rope(&rope);
                            current_rope = rope;
                            latest_version = version;
                        }
                        Job::Edit {
                            edits,
                            rope,
                            version,
                        } => {
                            doc.edit_rope_batch(&edits, &rope);
                            current_rope = rope;
                            latest_version = version;
                        }
                    }
                    current_job = job_rx.try_recv().ok();
                }
                let tokens = doc.highlight_rope(&current_rope);
                let fold_ranges = folding::folding_ranges(&doc);
                if result_tx
                    .send(HighlightResult {
                        tokens,
                        fold_ranges,
                        version: latest_version,
                    })
                    .is_err()
                {
                    break;
                }
                if let Some(waker) = &waker {
                    waker();
                }
            }
        });
        Self {
            jobs: Some(job_tx),
            results: result_rx,
            worker: Some(worker),
        }
    }

    /// Queues a job via non-blocking channel. Never blocks the caller.
    pub fn submit(&self, job: Job) -> bool {
        if let Some(jobs) = self.jobs.as_ref() {
            jobs.send(job).is_ok()
        } else {
            false
        }
    }

    /// Non-blocking check for the latest parse result, draining any intermediate
    /// stale results in the queue.
    pub fn try_recv(&self) -> Option<HighlightResult> {
        let mut latest = None;
        while let Ok(res) = self.results.try_recv() {
            latest = Some(res);
        }
        latest
    }

    /// Blocks until the worker finishes the next queued job (for initial loads or tests).
    pub fn recv(&self) -> Option<HighlightResult> {
        self.results.recv().ok()
    }
}

impl Drop for BackgroundHighlighter {
    fn drop(&mut self) {
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

    #[test]
    fn rope_based_parsing_matches_str_parsing() {
        let source = "fn add(a: i32, b: i32) -> i32 { a + b }";
        let rope = ropey::Rope::from_str(source);
        let mut doc = Document::new(LanguageId::Rust);
        doc.set_rope(&rope);

        let tokens_rope = doc.highlight_rope(&rope);
        let mut doc_str = Document::new(LanguageId::Rust);
        doc_str.set_text(source);
        let tokens_str = doc_str.highlight(source);

        assert_eq!(tokens_rope, tokens_str);
    }

    #[test]
    fn background_highlighter_processes_rope_jobs_and_waker() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let woke = std::sync::Arc::new(AtomicBool::new(false));
        let woke_clone = woke.clone();
        let highlighter = BackgroundHighlighter::spawn(
            LanguageId::Rust,
            Some(std::sync::Arc::new(move || {
                woke_clone.store(true, Ordering::SeqCst);
            })),
        );

        let source = "fn test() { let val = 123; }";
        let rope = ropey::Rope::from_str(source);
        assert!(highlighter.submit(Job::SetRope { rope, version: 1 }));

        let result = highlighter.recv().expect("should receive parse result");
        assert_eq!(result.version, 1);
        assert!(!result.tokens.is_empty());
        assert!(woke.load(Ordering::SeqCst));
    }
}
