//! Languages compiled into this binary. See `vendor/SOURCES.md` for which
//! grammars are included and which were left out.

use std::path::Path;

use tree_sitter::Language;

/// A language with a grammar compiled into this binary.
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
    Php,
    Ruby,
    Sql,
    Xml,
    Dockerfile,
    Make,
    Lua,
    CSharp,
    Diff,
    Zig,
    Graphql,
    Ini,
    Proto,
    Hcl,
    Nix,
    Cmake,
    Gitignore,
    Scss,
    Prisma,
    Fish,
    Scheme,
    Clojure,
    Regex,
    Erlang,
    Gleam,
    R,
    Elixir,
    Scala,
    Dart,
    Julia,
    Haskell,
    Ocaml,
    CommonLisp,
}

impl LanguageId {
    /// Picker and status-bar order. Rust stays first so the language
    /// picker's second row is unchanged.
    pub const ALL: &[Self] = &[
        Self::Rust,
        Self::TypeScript,
        Self::Tsx,
        Self::JavaScript,
        Self::Python,
        Self::Go,
        Self::C,
        Self::Cpp,
        Self::Java,
        Self::Kotlin,
        Self::Swift,
        Self::Html,
        Self::Css,
        Self::Json,
        Self::Yaml,
        Self::Markdown,
        Self::Bash,
        Self::Toml,
        Self::Php,
        Self::Ruby,
        Self::Sql,
        Self::Xml,
        Self::Dockerfile,
        Self::Make,
        Self::Lua,
        Self::CSharp,
        Self::Diff,
        Self::Zig,
        Self::Graphql,
        Self::Ini,
        Self::Proto,
        Self::Hcl,
        Self::Nix,
        Self::Cmake,
        Self::Gitignore,
        Self::Scss,
        Self::Prisma,
        Self::Fish,
        Self::Scheme,
        Self::Clojure,
        Self::Regex,
        Self::Erlang,
        Self::Gleam,
        Self::R,
        Self::Elixir,
        Self::Scala,
        Self::Dart,
        Self::Julia,
        Self::Haskell,
        Self::Ocaml,
        Self::CommonLisp,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::TypeScript => "TypeScript",
            Self::Tsx => "TypeScript (TSX)",
            Self::JavaScript => "JavaScript",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::Java => "Java",
            Self::Kotlin => "Kotlin",
            Self::Swift => "Swift",
            Self::Html => "HTML",
            Self::Css => "CSS",
            Self::Json => "JSON",
            Self::Yaml => "YAML",
            Self::Markdown => "Markdown",
            Self::Bash => "Shell Script",
            Self::Toml => "TOML",
            Self::Php => "PHP",
            Self::Ruby => "Ruby",
            Self::Sql => "SQL",
            Self::Xml => "XML",
            Self::Dockerfile => "Dockerfile",
            Self::Make => "Makefile",
            Self::Lua => "Lua",
            Self::CSharp => "C#",
            Self::Diff => "Diff",
            Self::Zig => "Zig",
            Self::Graphql => "GraphQL",
            Self::Ini => "INI",
            Self::Proto => "Protocol Buffers",
            Self::Hcl => "HCL",
            Self::Nix => "Nix",
            Self::Cmake => "CMake",
            Self::Gitignore => "Gitignore",
            Self::Scss => "SCSS",
            Self::Prisma => "Prisma",
            Self::Fish => "Fish",
            Self::Scheme => "Scheme",
            Self::Clojure => "Clojure",
            Self::Regex => "Regex",
            Self::Erlang => "Erlang",
            Self::Gleam => "Gleam",
            Self::R => "R",
            Self::Elixir => "Elixir",
            Self::Scala => "Scala",
            Self::Dart => "Dart",
            Self::Julia => "Julia",
            Self::Haskell => "Haskell",
            Self::Ocaml => "OCaml",
            Self::CommonLisp => "Common Lisp",
        }
    }

    /// TextMate scope used by snippet selectors.
    #[must_use]
    pub fn scope(self) -> &'static str {
        match self {
            Self::Rust => "source.rust",
            Self::TypeScript => "source.ts",
            Self::Tsx => "source.tsx",
            Self::JavaScript => "source.js",
            Self::Python => "source.python",
            Self::Go => "source.go",
            Self::C => "source.c",
            Self::Cpp => "source.cpp",
            Self::Java => "source.java",
            Self::Kotlin => "source.kotlin",
            Self::Swift => "source.swift",
            Self::Html => "text.html",
            Self::Css => "source.css",
            Self::Json => "source.json",
            Self::Yaml => "source.yaml",
            Self::Markdown => "text.markdown",
            Self::Bash => "source.shell",
            Self::Toml => "source.toml",
            Self::Php => "source.php",
            Self::Ruby => "source.ruby",
            Self::Sql => "source.sql",
            Self::Xml => "text.xml",
            Self::Dockerfile => "source.dockerfile",
            Self::Make => "source.makefile",
            Self::Lua => "source.lua",
            Self::CSharp => "source.cs",
            Self::Diff => "source.diff",
            Self::Zig => "source.zig",
            Self::Graphql => "source.graphql",
            Self::Ini => "source.ini",
            Self::Proto => "source.proto",
            Self::Hcl => "source.hcl",
            Self::Nix => "source.nix",
            Self::Cmake => "source.cmake",
            Self::Gitignore => "source.gitignore",
            Self::Scss => "source.css.scss",
            Self::Prisma => "source.prisma",
            Self::Fish => "source.fish",
            Self::Scheme => "source.scheme",
            Self::Clojure => "source.clojure",
            Self::Regex => "source.regex",
            Self::Erlang => "source.erlang",
            Self::Gleam => "source.gleam",
            Self::R => "source.r",
            Self::Elixir => "source.elixir",
            Self::Scala => "source.scala",
            Self::Dart => "source.dart",
            Self::Julia => "source.julia",
            Self::Haskell => "source.haskell",
            Self::Ocaml => "source.ocaml",
            Self::CommonLisp => "source.lisp",
        }
    }

    /// Line-comment prefix for `toggle_comment`. Languages whose comment is
    /// a block (`/* */`, `<!-- -->`, `(* *)`) return `None`.
    #[must_use]
    pub fn line_comment(self) -> Option<&'static str> {
        match self {
            Self::Rust
            | Self::TypeScript
            | Self::Tsx
            | Self::JavaScript
            | Self::Go
            | Self::C
            | Self::Cpp
            | Self::Java
            | Self::Kotlin
            | Self::Swift
            | Self::Php
            | Self::CSharp
            | Self::Zig
            | Self::Graphql
            | Self::Proto
            | Self::Hcl
            | Self::Scss
            | Self::Prisma
            | Self::Gleam
            | Self::Scala
            | Self::Dart => Some("//"),
            Self::Python
            | Self::Bash
            | Self::Toml
            | Self::Yaml
            | Self::Ruby
            | Self::Dockerfile
            | Self::Make
            | Self::Nix
            | Self::Cmake
            | Self::Gitignore
            | Self::Fish
            | Self::R
            | Self::Elixir
            | Self::Ini
            | Self::Julia => Some("#"),
            Self::Sql | Self::Lua | Self::Haskell => Some("--"),
            Self::Erlang => Some("%"),
            Self::Scheme | Self::Clojure | Self::CommonLisp => Some(";"),
            Self::Html
            | Self::Css
            | Self::Json
            | Self::Markdown
            | Self::Xml
            | Self::Diff
            | Self::Regex
            | Self::Ocaml => None,
        }
    }

    /// Maps a file extension (with or without the leading dot) to its
    /// language, if this build has a grammar for it.
    #[must_use]
    pub fn from_extension(ext: &str) -> Option<Self> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        EXTENSIONS
            .iter()
            .find(|(name, _)| *name == ext)
            .map(|(_, language)| *language)
    }

    /// Maps a file name (`Makefile`, `Dockerfile`, `.gitignore`) to a language.
    #[must_use]
    pub fn from_filename(name: &str) -> Option<Self> {
        match name {
            "Makefile" | "makefile" | "GNUmakefile" => Some(Self::Make),
            "Dockerfile" | "dockerfile" | "Containerfile" => Some(Self::Dockerfile),
            "CMakeLists.txt" => Some(Self::Cmake),
            "Gemfile" | "Rakefile" | "Capfile" => Some(Self::Ruby),
            ".gitignore" => Some(Self::Gitignore),
            "Cargo.toml" => Some(Self::Toml),
            _ => None,
        }
    }

    /// Maps a shebang line to a language. A `zsh` shebang stays Bash: there
    /// is no zsh grammar in this build.
    #[must_use]
    pub fn from_shebang(line: &str) -> Option<Self> {
        if !line.starts_with("#!") {
            return None;
        }
        if line.contains("python") {
            Some(Self::Python)
        } else if line.contains("ruby") {
            Some(Self::Ruby)
        } else if line.contains("php") {
            Some(Self::Php)
        } else if line.contains("lua") {
            Some(Self::Lua)
        } else if line.contains("fish") {
            Some(Self::Fish)
        } else if line.contains("node") || line.contains("deno") {
            Some(Self::JavaScript)
        } else if line.contains("Rscript") {
            Some(Self::R)
        } else if line.contains("elixir") {
            Some(Self::Elixir)
        } else if line.contains("bash") || line.contains("zsh") || line.contains("sh") {
            Some(Self::Bash)
        } else {
            None
        }
    }

    /// File name, then extension, then the first line's shebang.
    #[must_use]
    pub fn detect(path: Option<&Path>, first_line: Option<&str>) -> Option<Self> {
        if let Some(path) = path {
            if let Some(name) = path.file_name().and_then(|name| name.to_str())
                && let Some(language) = Self::from_filename(name)
            {
                return Some(language);
            }
            if let Some(ext) = path.extension().and_then(|ext| ext.to_str())
                && let Some(language) = Self::from_extension(ext)
            {
                return Some(language);
            }
        }
        first_line.and_then(Self::from_shebang)
    }

    pub(crate) fn ts_language(self) -> Language {
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
            Self::Php => lang_fn!(tree_sitter_php),
            Self::Ruby => lang_fn!(tree_sitter_ruby),
            Self::Sql => lang_fn!(tree_sitter_sql),
            Self::Xml => lang_fn!(tree_sitter_xml),
            Self::Dockerfile => lang_fn!(tree_sitter_dockerfile),
            Self::Make => lang_fn!(tree_sitter_make),
            Self::Lua => lang_fn!(tree_sitter_lua),
            Self::CSharp => lang_fn!(tree_sitter_c_sharp),
            Self::Diff => lang_fn!(tree_sitter_diff),
            Self::Zig => lang_fn!(tree_sitter_zig),
            Self::Graphql => lang_fn!(tree_sitter_graphql),
            Self::Ini => lang_fn!(tree_sitter_ini),
            Self::Proto => lang_fn!(tree_sitter_proto),
            Self::Hcl => lang_fn!(tree_sitter_hcl),
            Self::Nix => lang_fn!(tree_sitter_nix),
            Self::Cmake => lang_fn!(tree_sitter_cmake),
            Self::Gitignore => lang_fn!(tree_sitter_gitignore),
            Self::Scss => lang_fn!(tree_sitter_scss),
            Self::Prisma => lang_fn!(tree_sitter_prisma),
            Self::Fish => lang_fn!(tree_sitter_fish),
            Self::Scheme => lang_fn!(tree_sitter_scheme),
            Self::Clojure => lang_fn!(tree_sitter_clojure),
            Self::Regex => lang_fn!(tree_sitter_regex),
            Self::Erlang => lang_fn!(tree_sitter_erlang),
            Self::Gleam => lang_fn!(tree_sitter_gleam),
            Self::R => lang_fn!(tree_sitter_r),
            Self::Elixir => lang_fn!(tree_sitter_elixir),
            Self::Scala => lang_fn!(tree_sitter_scala),
            Self::Dart => lang_fn!(tree_sitter_dart),
            Self::Julia => lang_fn!(tree_sitter_julia),
            Self::Haskell => lang_fn!(tree_sitter_haskell),
            Self::Ocaml => lang_fn!(tree_sitter_ocaml),
            Self::CommonLisp => lang_fn!(tree_sitter_commonlisp),
        }
    }

    pub(crate) fn highlights_query(self) -> &'static str {
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
            Self::Php => include_str!("../queries/php/highlights.scm"),
            Self::Ruby => include_str!("../queries/ruby/highlights.scm"),
            Self::Sql => include_str!("../queries/sql/highlights.scm"),
            Self::Xml => include_str!("../queries/xml/highlights.scm"),
            Self::Dockerfile => include_str!("../queries/dockerfile/highlights.scm"),
            Self::Make => include_str!("../queries/make/highlights.scm"),
            Self::Lua => include_str!("../queries/lua/highlights.scm"),
            Self::CSharp => include_str!("../queries/c_sharp/highlights.scm"),
            Self::Diff => include_str!("../queries/diff/highlights.scm"),
            Self::Zig => include_str!("../queries/zig/highlights.scm"),
            Self::Graphql => include_str!("../queries/graphql/highlights.scm"),
            Self::Ini => include_str!("../queries/ini/highlights.scm"),
            Self::Proto => include_str!("../queries/proto/highlights.scm"),
            Self::Hcl => include_str!("../queries/hcl/highlights.scm"),
            Self::Nix => include_str!("../queries/nix/highlights.scm"),
            Self::Cmake => include_str!("../queries/cmake/highlights.scm"),
            Self::Gitignore => include_str!("../queries/gitignore/highlights.scm"),
            Self::Scss => include_str!("../queries/scss/highlights.scm"),
            Self::Prisma => include_str!("../queries/prisma/highlights.scm"),
            Self::Fish => include_str!("../queries/fish/highlights.scm"),
            Self::Scheme => include_str!("../queries/scheme/highlights.scm"),
            Self::Clojure => include_str!("../queries/clojure/highlights.scm"),
            Self::Regex => include_str!("../queries/regex/highlights.scm"),
            Self::Erlang => include_str!("../queries/erlang/highlights.scm"),
            Self::Gleam => include_str!("../queries/gleam/highlights.scm"),
            Self::R => include_str!("../queries/r/highlights.scm"),
            Self::Elixir => include_str!("../queries/elixir/highlights.scm"),
            Self::Scala => include_str!("../queries/scala/highlights.scm"),
            Self::Dart => include_str!("../queries/dart/highlights.scm"),
            Self::Julia => include_str!("../queries/julia/highlights.scm"),
            Self::Haskell => include_str!("../queries/haskell/highlights.scm"),
            Self::Ocaml => include_str!("../queries/ocaml/highlights.scm"),
            Self::CommonLisp => include_str!("../queries/commonlisp/highlights.scm"),
        }
    }
}

const EXTENSIONS: &[(&str, LanguageId)] = &[
    ("rs", LanguageId::Rust),
    ("ts", LanguageId::TypeScript),
    ("mts", LanguageId::TypeScript),
    ("cts", LanguageId::TypeScript),
    ("tsx", LanguageId::Tsx),
    ("js", LanguageId::JavaScript),
    ("mjs", LanguageId::JavaScript),
    ("cjs", LanguageId::JavaScript),
    ("jsx", LanguageId::JavaScript),
    ("py", LanguageId::Python),
    ("pyi", LanguageId::Python),
    ("go", LanguageId::Go),
    ("c", LanguageId::C),
    ("h", LanguageId::C),
    ("cpp", LanguageId::Cpp),
    ("cc", LanguageId::Cpp),
    ("cxx", LanguageId::Cpp),
    ("hpp", LanguageId::Cpp),
    ("hh", LanguageId::Cpp),
    ("hxx", LanguageId::Cpp),
    ("java", LanguageId::Java),
    ("kt", LanguageId::Kotlin),
    ("kts", LanguageId::Kotlin),
    ("swift", LanguageId::Swift),
    ("html", LanguageId::Html),
    ("htm", LanguageId::Html),
    ("css", LanguageId::Css),
    ("json", LanguageId::Json),
    ("yaml", LanguageId::Yaml),
    ("yml", LanguageId::Yaml),
    ("md", LanguageId::Markdown),
    ("markdown", LanguageId::Markdown),
    ("sh", LanguageId::Bash),
    ("bash", LanguageId::Bash),
    ("toml", LanguageId::Toml),
    ("php", LanguageId::Php),
    ("phtml", LanguageId::Php),
    ("rb", LanguageId::Ruby),
    ("rake", LanguageId::Ruby),
    ("gemspec", LanguageId::Ruby),
    ("sql", LanguageId::Sql),
    ("xml", LanguageId::Xml),
    ("svg", LanguageId::Xml),
    ("xsl", LanguageId::Xml),
    ("xsd", LanguageId::Xml),
    ("plist", LanguageId::Xml),
    ("dockerfile", LanguageId::Dockerfile),
    ("mk", LanguageId::Make),
    ("mak", LanguageId::Make),
    ("lua", LanguageId::Lua),
    ("cs", LanguageId::CSharp),
    ("diff", LanguageId::Diff),
    ("patch", LanguageId::Diff),
    ("zig", LanguageId::Zig),
    ("graphql", LanguageId::Graphql),
    ("gql", LanguageId::Graphql),
    ("ini", LanguageId::Ini),
    ("proto", LanguageId::Proto),
    ("hcl", LanguageId::Hcl),
    ("tf", LanguageId::Hcl),
    ("tfvars", LanguageId::Hcl),
    ("nix", LanguageId::Nix),
    ("cmake", LanguageId::Cmake),
    ("scss", LanguageId::Scss),
    ("prisma", LanguageId::Prisma),
    ("fish", LanguageId::Fish),
    ("scm", LanguageId::Scheme),
    ("ss", LanguageId::Scheme),
    ("clj", LanguageId::Clojure),
    ("cljs", LanguageId::Clojure),
    ("cljc", LanguageId::Clojure),
    ("edn", LanguageId::Clojure),
    ("regex", LanguageId::Regex),
    ("erl", LanguageId::Erlang),
    ("hrl", LanguageId::Erlang),
    ("gleam", LanguageId::Gleam),
    ("r", LanguageId::R),
    ("ex", LanguageId::Elixir),
    ("exs", LanguageId::Elixir),
    ("scala", LanguageId::Scala),
    ("sc", LanguageId::Scala),
    ("dart", LanguageId::Dart),
    ("jl", LanguageId::Julia),
    ("hs", LanguageId::Haskell),
    ("lhs", LanguageId::Haskell),
    ("ml", LanguageId::Ocaml),
    ("lisp", LanguageId::CommonLisp),
];
