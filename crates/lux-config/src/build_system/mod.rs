//! Sublime Text `.sublime-build` files: parsing, variable expansion and
//! `selector` matching. Pure data — reading the files and running the
//! commands is the editor's job, on its own workers.
//!
//! Supported keys: `cmd` (argv array), `shell_cmd` (one string for the
//! user's shell), `working_dir`, `env`, `file_regex`, `line_regex`,
//! `selector` and nested `variants`, which inherit every key they do not
//! set. Sublime's `target`, `syntax`, `file_patterns`, `keyfiles`,
//! `encoding`, `quiet` and `word_wrap` keys are accepted and ignored.

mod expand;
mod selector;

pub use expand::{ExpandContext, expand, expand_with};
pub use selector::score;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

/// The extension Sublime Text uses for build system files.
pub const SUBLIME_BUILD_EXT: &str = "sublime-build";

/// What a build runs: an argv (`cmd`) or a line for the shell (`shell_cmd`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildCommand {
    Argv(Vec<String>),
    Shell(String),
}

/// One runnable entry: the top level of a build system or one variant.
/// Unset keys stay `None` so a variant can inherit them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildSpec {
    pub command: Option<BuildCommand>,
    pub working_dir: Option<String>,
    pub env: Vec<(String, String)>,
    pub file_regex: Option<String>,
    pub line_regex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildVariantSpec {
    pub name: String,
    pub spec: BuildSpec,
}

/// A parsed `.sublime-build` file or a project `build_systems` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildSystem {
    pub name: String,
    pub source: Option<PathBuf>,
    pub selector: Option<String>,
    pub base: BuildSpec,
    pub variants: Vec<BuildVariantSpec>,
}

/// A build system or variant with every variable expanded, ready to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBuild {
    /// `System` for the top level, `System - Variant` for a variant.
    pub name: String,
    pub command: BuildCommand,
    pub working_dir: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub file_regex: Option<String>,
    pub line_regex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildParseError(pub String);

impl std::fmt::Display for BuildParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BuildParseError {}

impl BuildSystem {
    /// Parses a `.sublime-build` file's text (JSON with comments and
    /// trailing commas, as Sublime accepts). The name is the file stem.
    pub fn parse_file(path: &Path, text: &str) -> Result<Self, BuildParseError> {
        let value = crate::layering::jsonc::parse(text)
            .map_err(|e| BuildParseError(format!("{}: {e}", path.display())))?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut system = Self::from_value(&name, &value)
            .map_err(|e| BuildParseError(format!("{}: {}", path.display(), e.0)))?;
        system.source = Some(path.to_path_buf());
        Ok(system)
    }

    /// Builds a system from one JSON object (a file's root, or a
    /// `build_systems` entry of a project file, whose `name` wins).
    pub fn from_value(name: &str, value: &Value) -> Result<Self, BuildParseError> {
        let Some(object) = value.as_object() else {
            return Err(BuildParseError(
                "a build system must be a JSON object".into(),
            ));
        };
        let name = object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(name)
            .to_string();
        let base = parse_spec(object)?;
        let mut variants = Vec::new();
        if let Some(list) = object.get("variants") {
            let Some(list) = list.as_array() else {
                return Err(BuildParseError("`variants` must be an array".into()));
            };
            for entry in list {
                let Some(entry) = entry.as_object() else {
                    return Err(BuildParseError("each variant must be an object".into()));
                };
                let Some(variant_name) = entry.get("name").and_then(Value::as_str) else {
                    return Err(BuildParseError("each variant needs a `name`".into()));
                };
                variants.push(BuildVariantSpec {
                    name: variant_name.to_string(),
                    spec: parse_spec(entry)?,
                });
            }
        }
        Ok(BuildSystem {
            name,
            source: None,
            selector: string(object, "selector")?,
            base,
            variants,
        })
    }

    /// The top level and every variant, expanded against `context`. An
    /// entry with no command (also not inherited) is left out. Unknown
    /// names in `env` values come from this process's environment, the
    /// one the build inherits.
    pub fn resolve(&self, context: &ExpandContext) -> Vec<ResolvedBuild> {
        self.resolve_with_env(context, &|name| std::env::var(name).ok())
    }

    /// [`Self::resolve`] with the inherited environment given explicitly.
    pub fn resolve_with_env(
        &self,
        context: &ExpandContext,
        environment: &dyn Fn(&str) -> Option<String>,
    ) -> Vec<ResolvedBuild> {
        let mut out = Vec::new();
        if let Some(build) = resolve_spec(&self.name, &self.base, context, environment) {
            out.push(build);
        }
        for variant in &self.variants {
            let merged = inherit(&self.base, &variant.spec);
            let name = format!("{} - {}", self.name, variant.name);
            if let Some(build) = resolve_spec(&name, &merged, context, environment) {
                out.push(build);
            }
        }
        out
    }

    /// How well this system's `selector` matches the active buffer's base
    /// scope; `None` when it has no selector or does not match.
    pub fn selector_score(&self, scope: Option<&str>) -> Option<u32> {
        score(self.selector.as_deref()?, scope?)
    }
}

/// A variant's own keys over its parent's; `env` merges key by key.
fn inherit(base: &BuildSpec, own: &BuildSpec) -> BuildSpec {
    let mut env = base.env.clone();
    for (key, value) in &own.env {
        env.retain(|(k, _)| k != key);
        env.push((key.clone(), value.clone()));
    }
    BuildSpec {
        command: own.command.clone().or_else(|| base.command.clone()),
        working_dir: own.working_dir.clone().or_else(|| base.working_dir.clone()),
        env,
        file_regex: own.file_regex.clone().or_else(|| base.file_regex.clone()),
        line_regex: own.line_regex.clone().or_else(|| base.line_regex.clone()),
    }
}

fn resolve_spec(
    name: &str,
    spec: &BuildSpec,
    context: &ExpandContext,
    environment: &dyn Fn(&str) -> Option<String>,
) -> Option<ResolvedBuild> {
    let command = match spec.command.as_ref()? {
        BuildCommand::Argv(argv) => {
            BuildCommand::Argv(argv.iter().map(|a| expand(a, context)).collect())
        }
        BuildCommand::Shell(line) => BuildCommand::Shell(expand(line, context)),
    };
    // Sublime's `exec` runs in the current file's folder when no
    // `working_dir` is given.
    let working_dir = match &spec.working_dir {
        Some(dir) => {
            Some(PathBuf::from(expand(dir, context))).filter(|d| !d.as_os_str().is_empty())
        }
        None => context
            .file
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| context.folder.clone()),
    };
    Some(ResolvedBuild {
        name: name.to_string(),
        command,
        working_dir,
        env: spec
            .env
            .iter()
            .map(|(k, v)| (k.clone(), expand_with(v, context, environment)))
            .collect(),
        file_regex: spec.file_regex.clone(),
        line_regex: spec.line_regex.clone(),
    })
}

fn parse_spec(object: &Map<String, Value>) -> Result<BuildSpec, BuildParseError> {
    let shell_cmd = string(object, "shell_cmd")?;
    let command = match (object.get("cmd"), shell_cmd) {
        (_, Some(line)) => Some(BuildCommand::Shell(line)),
        (None | Some(Value::Null), None) => None,
        (Some(Value::Array(items)), None) if items.is_empty() => None,
        (Some(Value::Array(items)), None) => {
            let argv = items
                .iter()
                .map(|item| item.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| BuildParseError("`cmd` must be an array of strings".into()))?;
            Some(BuildCommand::Argv(argv))
        }
        // Sublime runs a string `cmd` through the shell.
        (Some(Value::String(line)), None) => Some(BuildCommand::Shell(line.clone())),
        (Some(_), None) => {
            return Err(BuildParseError("`cmd` must be an array of strings".into()));
        }
    };
    let mut env = Vec::new();
    if let Some(map) = object.get("env") {
        let Some(map) = map.as_object() else {
            return Err(BuildParseError("`env` must be an object".into()));
        };
        for (key, value) in map {
            let Some(value) = value.as_str() else {
                return Err(BuildParseError(format!("`env.{key}` must be a string")));
            };
            env.push((key.clone(), value.to_string()));
        }
    }
    Ok(BuildSpec {
        command,
        working_dir: string(object, "working_dir")?,
        env,
        file_regex: string(object, "file_regex")?,
        line_regex: string(object, "line_regex")?,
    })
}

fn string(object: &Map<String, Value>, key: &str) -> Result<Option<String>, BuildParseError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(BuildParseError(format!("`{key}` must be a string"))),
    }
}

/// How deep `load_dir` descends below the folder it was given.
pub const MAX_LOAD_DEPTH: usize = 8;

/// Reads every `*.sublime-build` under `dir` (recursively, as Sublime
/// scans its packages, at most [`MAX_LOAD_DEPTH`] folders down). A
/// missing directory yields nothing; files that fail to read or parse are
/// reported in the second list and skipped. Symlinked folders are not
/// followed, so a link loop cannot trap the scan; symlinked files are
/// read.
pub fn load_dir(dir: &Path) -> (Vec<BuildSystem>, Vec<BuildParseError>) {
    let mut systems = Vec::new();
    let mut errors = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<(PathBuf, std::fs::FileType)> = entries
            .flatten()
            .filter_map(|e| Some((e.path(), e.file_type().ok()?)))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (path, kind) in entries {
            if kind.is_dir() {
                if depth < MAX_LOAD_DEPTH {
                    stack.push((path, depth + 1));
                }
            } else if kind.is_file() || (kind.is_symlink() && path.is_file()) {
                load_file_into(&path, &mut systems, &mut errors);
            }
        }
    }
    (systems, errors)
}

/// `*.sublime-build` files directly inside `dir`, not its subfolders.
pub fn load_dir_shallow(dir: &Path) -> (Vec<BuildSystem>, Vec<BuildParseError>) {
    let mut systems = Vec::new();
    let mut errors = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths.iter().filter(|p| p.is_file()) {
            load_file_into(path, &mut systems, &mut errors);
        }
    }
    (systems, errors)
}

fn load_file_into(path: &Path, systems: &mut Vec<BuildSystem>, errors: &mut Vec<BuildParseError>) {
    if path.extension().is_none_or(|ext| ext != SUBLIME_BUILD_EXT) {
        return;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => match BuildSystem::parse_file(path, &text) {
            Ok(system) => systems.push(system),
            Err(error) => errors.push(error),
        },
        Err(error) => errors.push(BuildParseError(format!("{}: {error}", path.display()))),
    }
}

#[cfg(test)]
mod tests;
