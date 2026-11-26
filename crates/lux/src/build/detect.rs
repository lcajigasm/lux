//! Build variants Lux offers without any build file: Cargo, npm, Python,
//! Go, Make, Bash and rustc, chosen from marker files in the workspace and
//! the active file's extension.

use std::path::Path;

use super::BuildVariant;

pub(crate) fn detect_build_variants(
    workspace_root: Option<&Path>,
    current_file: Option<&Path>,
) -> Vec<BuildVariant> {
    let mut variants = Vec::new();

    let has_cargo = workspace_root.is_some_and(|r| r.join("Cargo.toml").exists())
        || current_file.is_some_and(|f| has_ancestor_file(f, "Cargo.toml"));

    if has_cargo {
        variants.push(BuildVariant {
            name: "Cargo: Check".into(),
            command: "cargo".into(),
            args: vec!["check".into()],
            ..BuildVariant::default()
        });
        variants.push(BuildVariant {
            name: "Cargo: Build".into(),
            command: "cargo".into(),
            args: vec!["build".into()],
            ..BuildVariant::default()
        });
        variants.push(BuildVariant {
            name: "Cargo: Test".into(),
            command: "cargo".into(),
            args: vec!["test".into()],
            ..BuildVariant::default()
        });
        variants.push(BuildVariant {
            name: "Cargo: Clippy".into(),
            command: "cargo".into(),
            args: vec!["clippy".into()],
            ..BuildVariant::default()
        });
        variants.push(BuildVariant {
            name: "Cargo: Run".into(),
            command: "cargo".into(),
            args: vec!["run".into()],
            ..BuildVariant::default()
        });
    }

    let has_package_json = workspace_root.is_some_and(|r| r.join("package.json").exists())
        || current_file.is_some_and(|f| has_ancestor_file(f, "package.json"));

    if has_package_json {
        variants.push(BuildVariant {
            name: "npm: test".into(),
            command: "npm".into(),
            args: vec!["test".into()],
            ..BuildVariant::default()
        });
        variants.push(BuildVariant {
            name: "npm: build".into(),
            command: "npm".into(),
            args: vec!["run".into(), "build".into()],
            ..BuildVariant::default()
        });
    }

    if let Some(file) = current_file {
        let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
        let file_str = file.to_string_lossy().to_string();

        match ext {
            "js" | "mjs" | "cjs" | "ts" => {
                variants.push(BuildVariant {
                    name: "Node: Run File".into(),
                    command: "node".into(),
                    args: vec![file_str],
                    ..BuildVariant::default()
                });
            }
            "py" => {
                variants.push(BuildVariant {
                    name: "Python: Run File".into(),
                    command: "python3".into(),
                    args: vec![file_str],
                    ..BuildVariant::default()
                });
            }
            "go" => {
                variants.push(BuildVariant {
                    name: "Go: Run File".into(),
                    command: "go".into(),
                    args: vec!["run".into(), file_str],
                    ..BuildVariant::default()
                });
            }
            "sh" | "bash" => {
                variants.push(BuildVariant {
                    name: "Bash: Run File".into(),
                    command: "bash".into(),
                    args: vec![file_str],
                    ..BuildVariant::default()
                });
            }
            "rs" => {
                variants.push(BuildVariant {
                    name: "Rust: Compile File".into(),
                    command: "rustc".into(),
                    args: vec![file_str],
                    ..BuildVariant::default()
                });
            }
            _ => {}
        }
    }

    let has_pyproject = workspace_root
        .is_some_and(|r| r.join("pyproject.toml").exists() || r.join("pytest.ini").exists());
    if has_pyproject {
        variants.push(BuildVariant {
            name: "Python: Pytest".into(),
            command: "pytest".into(),
            args: vec![],
            ..BuildVariant::default()
        });
    }

    let has_go = workspace_root.is_some_and(|r| r.join("go.mod").exists())
        || current_file.is_some_and(|f| has_ancestor_file(f, "go.mod"));
    if has_go {
        variants.push(BuildVariant {
            name: "Go: Build".into(),
            command: "go".into(),
            args: vec!["build".into()],
            ..BuildVariant::default()
        });
        variants.push(BuildVariant {
            name: "Go: Test".into(),
            command: "go".into(),
            args: vec!["test".into(), "./...".into()],
            ..BuildVariant::default()
        });
    }

    let has_makefile =
        workspace_root.is_some_and(|r| r.join("Makefile").exists() || r.join("makefile").exists());
    if has_makefile {
        variants.push(BuildVariant {
            name: "Make".into(),
            command: "make".into(),
            args: vec![],
            ..BuildVariant::default()
        });
        variants.push(BuildVariant {
            name: "Make: Clean".into(),
            command: "make".into(),
            args: vec!["clean".into()],
            ..BuildVariant::default()
        });
    }

    variants
}

fn has_ancestor_file(path: &Path, file_name: &str) -> bool {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir.join(file_name).exists() {
            return true;
        }
        current = dir.parent();
    }
    false
}
