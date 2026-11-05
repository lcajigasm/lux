//! Golden token tests: each language's highlight query is run over a fixture
//! file and the resulting token dump is compared against a checked-in
//! golden file. A grammar that compiles but was never run through this test
//! is not verified (see `.claude/skills/add-tree-sitter-grammar/SKILL.md`).
//!
//! Set `LUX_SYNTAX_BLESS=1` to (re)write the golden file from the actual
//! output instead of asserting against it.

use lux_syntax::{Document, LanguageId, Token};

fn dump(tokens: &[Token]) -> String {
    tokens
        .iter()
        .map(|t| format!("{}..{} {}\n", t.start_byte, t.end_byte, t.kind))
        .collect()
}

fn check(language: LanguageId, fixture: &str) {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let fixture_path = format!("{manifest_dir}/tests/fixtures/{fixture}");
    let golden_path = format!("{fixture_path}.tokens");

    let source = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("reading fixture {fixture_path}: {e}"));
    let mut doc = Document::new(language);
    doc.set_text(&source);
    let actual = dump(&doc.highlight(&source));

    if std::env::var("LUX_SYNTAX_BLESS").is_ok() {
        std::fs::write(&golden_path, &actual)
            .unwrap_or_else(|e| panic!("writing golden {golden_path}: {e}"));
        return;
    }

    let expected = std::fs::read_to_string(&golden_path).unwrap_or_else(|e| {
        panic!("reading golden {golden_path}: {e} (run with LUX_SYNTAX_BLESS=1 to create it)")
    });
    assert_eq!(actual, expected, "golden token mismatch for {fixture}");
}

mod highlight {
    use super::*;

    #[test]
    fn rust() {
        check(LanguageId::Rust, "rust/sample.rs");
    }

    #[test]
    fn typescript() {
        check(LanguageId::TypeScript, "typescript/sample.ts");
    }

    #[test]
    fn tsx() {
        check(LanguageId::Tsx, "tsx/sample.tsx");
    }

    #[test]
    fn javascript() {
        check(LanguageId::JavaScript, "javascript/sample.js");
    }

    #[test]
    fn python() {
        check(LanguageId::Python, "python/sample.py");
    }

    #[test]
    fn go() {
        check(LanguageId::Go, "go/sample.go");
    }

    #[test]
    fn c() {
        check(LanguageId::C, "c/sample.c");
    }

    #[test]
    fn cpp() {
        check(LanguageId::Cpp, "cpp/sample.cpp");
    }

    #[test]
    fn java() {
        check(LanguageId::Java, "java/sample.java");
    }

    #[test]
    fn kotlin() {
        check(LanguageId::Kotlin, "kotlin/sample.kt");
    }

    #[test]
    fn swift() {
        check(LanguageId::Swift, "swift/sample.swift");
    }

    #[test]
    fn html() {
        check(LanguageId::Html, "html/sample.html");
    }

    #[test]
    fn css() {
        check(LanguageId::Css, "css/sample.css");
    }

    #[test]
    fn json() {
        check(LanguageId::Json, "json/sample.json");
    }

    #[test]
    fn yaml() {
        check(LanguageId::Yaml, "yaml/sample.yaml");
    }

    #[test]
    fn markdown() {
        check(LanguageId::Markdown, "markdown/sample.md");
    }

    #[test]
    fn bash() {
        check(LanguageId::Bash, "bash/sample.sh");
    }

    #[test]
    fn toml() {
        check(LanguageId::Toml, "toml/sample.toml");
    }

    #[test]
    fn php() {
        check(LanguageId::Php, "php/sample.php");
    }

    #[test]
    fn ruby() {
        check(LanguageId::Ruby, "ruby/sample.rb");
    }

    #[test]
    fn sql() {
        check(LanguageId::Sql, "sql/sample.sql");
    }

    #[test]
    fn xml() {
        check(LanguageId::Xml, "xml/sample.xml");
    }

    #[test]
    fn dockerfile() {
        check(LanguageId::Dockerfile, "dockerfile/sample.dockerfile");
    }

    #[test]
    fn make() {
        check(LanguageId::Make, "make/sample.mk");
    }

    #[test]
    fn lua() {
        check(LanguageId::Lua, "lua/sample.lua");
    }

    #[test]
    fn c_sharp() {
        check(LanguageId::CSharp, "c_sharp/sample.cs");
    }

    #[test]
    fn diff() {
        check(LanguageId::Diff, "diff/sample.diff");
    }
}
