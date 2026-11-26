use std::path::{Path, PathBuf};

use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "lux-build-system-{label}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn context() -> ExpandContext {
    ExpandContext {
        file: Some(PathBuf::from("/work/app/src/main.rs")),
        folder: Some(PathBuf::from("/work/app")),
        project: Some(PathBuf::from("/work/app/app.sublime-project")),
    }
}

#[test]
fn expands_the_documented_file_and_project_variables() {
    let ctx = context();
    let cases = [
        ("${file}", "/work/app/src/main.rs"),
        ("${file_path}", "/work/app/src"),
        ("${file_name}", "main.rs"),
        ("${file_base_name}", "main"),
        ("${file_extension}", "rs"),
        ("${folder}", "/work/app"),
        ("${project}", "/work/app/app.sublime-project"),
        ("${project_path}", "/work/app"),
        ("${project_name}", "app.sublime-project"),
        ("${project_base_name}", "app"),
        ("${project_extension}", "sublime-project"),
        ("$file_base_name.o", "main.o"),
        ("-o ${file_path}/${file_base_name}", "-o /work/app/src/main"),
    ];
    for (input, expected) in cases {
        assert_eq!(expand(input, &ctx), expected, "{input}");
    }
}

#[test]
fn defaults_escapes_and_unknown_names() {
    let empty = ExpandContext::default();
    assert_eq!(expand("${file_name:untitled}", &empty), "untitled");
    assert_eq!(expand("${project_path:${folder}}", &context()), "/work/app");
    assert_eq!(
        expand(
            "${project_path:${file_path}}",
            &ExpandContext {
                file: Some(PathBuf::from("/a/b.py")),
                ..ExpandContext::default()
            }
        ),
        "/a"
    );
    assert_eq!(expand("echo \\$HOME", &context()), "echo $HOME");
    assert_eq!(expand("${packages}/x", &context()), "/x", "no Packages dir");
    assert_eq!(expand("cost: 5$ ok", &context()), "cost: 5$ ok");
    assert_eq!(expand("${unclosed", &context()), "${unclosed");
}

#[test]
fn parses_cmd_selector_regexes_and_variants() {
    let text = r#"{
        // JSONC: comments and trailing commas, like Sublime accepts.
        "cmd": ["cargo", "check"],
        "file_regex": "^(..[^:]*):([0-9]+):?([0-9]+)?:? (.*)$",
        "line_regex": "^line ([0-9]+)",
        "working_dir": "${folder}",
        "selector": "source.rust",
        "env": { "RUST_BACKTRACE": "1" },
        "variants": [
            { "name": "Test", "cmd": ["cargo", "test"] },
            { "name": "Shell", "shell_cmd": "make ${file_base_name}", "env": { "CC": "clang" } },
        ],
    }"#;
    let system = BuildSystem::parse_file(Path::new("/cfg/builds/Rust Check.sublime-build"), text)
        .expect("parses");
    assert_eq!(system.name, "Rust Check");
    assert_eq!(system.selector.as_deref(), Some("source.rust"));
    assert_eq!(system.variants.len(), 2);

    let resolved = system.resolve(&context());
    let names: Vec<&str> = resolved.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(
        names,
        ["Rust Check", "Rust Check - Test", "Rust Check - Shell"]
    );

    let test = &resolved[1];
    assert_eq!(
        test.command,
        BuildCommand::Argv(vec!["cargo".into(), "test".into()])
    );
    assert_eq!(test.working_dir.as_deref(), Some(Path::new("/work/app")));
    assert_eq!(test.file_regex, resolved[0].file_regex, "inherited");
    assert_eq!(test.line_regex.as_deref(), Some("^line ([0-9]+)"));

    let shell = &resolved[2];
    assert_eq!(shell.command, BuildCommand::Shell("make main".into()));
    assert_eq!(
        shell.env,
        vec![
            ("RUST_BACKTRACE".to_string(), "1".to_string()),
            ("CC".to_string(), "clang".to_string())
        ]
    );
}

#[test]
fn working_dir_defaults_to_the_file_folder_like_sublime_exec() {
    let system = BuildSystem::from_value("Run", &serde_json::json!({ "cmd": ["true"] })).unwrap();
    let resolved = system.resolve(&context());
    assert_eq!(
        resolved[0].working_dir.as_deref(),
        Some(Path::new("/work/app/src"))
    );

    let no_file = ExpandContext {
        folder: Some(PathBuf::from("/root")),
        ..ExpandContext::default()
    };
    assert_eq!(
        system.resolve(&no_file)[0].working_dir.as_deref(),
        Some(Path::new("/root"))
    );
}

#[test]
fn a_system_without_a_command_lists_only_its_runnable_variants() {
    let system = BuildSystem::from_value(
        "Only Variants",
        &serde_json::json!({ "variants": [ { "name": "Go", "cmd": ["go", "run", "$file"] } ] }),
    )
    .unwrap();
    let resolved = system.resolve(&context());
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].name, "Only Variants - Go");
}

#[test]
fn rejects_malformed_files() {
    for bad in [
        r#"[]"#,
        r#"{ "cmd": [1, 2] }"#,
        r#"{ "cmd": true }"#,
        r#"{ "env": [] }"#,
        r#"{ "variants": [ { "cmd": ["x"] } ] }"#,
        r#"{ "selector": 3 }"#,
        r#"{ "cmd": ["x"] "#,
    ] {
        assert!(
            BuildSystem::parse_file(Path::new("bad.sublime-build"), bad).is_err(),
            "{bad}"
        );
    }
}

#[test]
fn selector_scores_by_specificity_and_honours_exclusions() {
    assert_eq!(score("source.rust", "source.rust"), Some(2));
    assert_eq!(score("source", "source.rust"), Some(1));
    assert_eq!(score("source.rust", "source.rustacean"), None);
    assert_eq!(score("source.python", "source.rust"), None);
    assert_eq!(score("source.c, source.c++", "source.c++"), Some(2));
    assert_eq!(score("source.c | source.c++", "source.c"), Some(2));
    assert_eq!(
        score("text.html - text.html.markdown", "text.html.markdown"),
        None
    );
    assert_eq!(
        score("text.html - text.html.markdown", "text.html.basic"),
        Some(2)
    );
    assert_eq!(score("text.html source.js", "source.js"), None);
    assert_eq!(score("", "source.rust"), None);
}

#[test]
fn load_dir_reads_nested_build_files_and_reports_bad_ones() {
    let dir = TempDir::new("load-dir");
    let nested = dir.path().join("Rust");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(
        nested.join("Cargo.sublime-build"),
        r#"{ "cmd": ["cargo", "build"] }"#,
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Echo.sublime-build"),
        r#"{ "shell_cmd": "echo hi" }"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("Broken.sublime-build"), "{").unwrap();
    std::fs::write(dir.path().join("notes.txt"), "ignored").unwrap();

    let (systems, errors) = load_dir(dir.path());
    let mut names: Vec<&str> = systems.iter().map(|s| s.name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["Cargo", "Echo"]);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].0.contains("Broken.sublime-build"));

    let (shallow, _) = load_dir_shallow(dir.path());
    let names: Vec<&str> = shallow.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["Echo"]);

    let (missing, missing_errors) = load_dir(&dir.path().join("nope"));
    assert!(missing.is_empty() && missing_errors.is_empty());
}
