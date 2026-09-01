//! Local install from a folder: copy wasm + manifest, require an explicit
//! trust marker (no implicit run-from-download).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("folder is not trusted (missing trust.json)")]
    Untrusted,
    #[error("plugin manifest missing or invalid")]
    Manifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPlugin {
    pub id: String,
    pub version: u32,
    pub wasm_path: PathBuf,
    pub commands: Vec<String>,
    pub grammars: Vec<String>,
    pub themes: Vec<String>,
    pub formatters: Vec<String>,
    pub lsp_servers: Vec<String>,
    pub decorations: Vec<String>,
}

#[derive(Deserialize)]
struct Manifest {
    id: String,
    version: u32,
    #[serde(default)]
    wasm: String,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    grammars: Vec<String>,
    #[serde(default)]
    themes: Vec<String>,
    #[serde(default)]
    formatters: Vec<String>,
    #[serde(default)]
    lsp_servers: Vec<String>,
    #[serde(default)]
    decorations: Vec<String>,
}

#[derive(Deserialize)]
struct Trust {
    trusted: bool,
}

/// Copy `src` into `dest_root/id` if `src/trust.json` says trusted.
pub fn install_from_dir(src: &Path, dest_root: &Path) -> Result<InstalledPlugin, InstallError> {
    let trust: Trust = serde_json::from_slice(&fs::read(src.join("trust.json"))?)
        .map_err(|_| InstallError::Untrusted)?;
    if !trust.trusted {
        return Err(InstallError::Untrusted);
    }
    let manifest: Manifest = serde_json::from_slice(&fs::read(src.join("lux-plugin.json"))?)
        .map_err(|_| InstallError::Manifest)?;
    let dest = dest_root.join(&manifest.id);
    fs::create_dir_all(&dest)?;
    let wasm_name = if manifest.wasm.is_empty() {
        "plugin.wasm".into()
    } else {
        manifest.wasm.clone()
    };
    let wasm_src = src.join(&wasm_name);
    let wasm_dest = dest.join(&wasm_name);
    if wasm_src.exists() {
        fs::copy(&wasm_src, &wasm_dest)?;
    }
    for extra in ["lux-plugin.json", "trust.json"] {
        fs::copy(src.join(extra), dest.join(extra))?;
    }
    // Theme/grammar files are data extension points — copy the rest.
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let dest_file = dest.join(&name);
        if dest_file.exists() {
            continue;
        }
        if entry.path().is_file() {
            fs::copy(entry.path(), dest_file)?;
        }
    }
    Ok(InstalledPlugin {
        id: manifest.id,
        version: manifest.version,
        wasm_path: wasm_dest,
        commands: manifest.commands,
        grammars: manifest.grammars,
        themes: manifest.themes,
        formatters: manifest.formatters,
        lsp_servers: manifest.lsp_servers,
        decorations: manifest.decorations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_untrusted_folder() {
        let src = tempfile::tempdir().unwrap();
        fs::write(
            src.path().join("lux-plugin.json"),
            r#"{"id":"x","version":1}"#,
        )
        .unwrap();
        fs::write(src.path().join("trust.json"), r#"{"trusted":false}"#).unwrap();
        let dest = tempfile::tempdir().unwrap();
        assert!(matches!(
            install_from_dir(src.path(), dest.path()),
            Err(InstallError::Untrusted)
        ));
    }

    #[test]
    fn copies_a_trusted_plugin() {
        let src = tempfile::tempdir().unwrap();
        fs::write(
            src.path().join("lux-plugin.json"),
            r#"{"id":"echo","version":1,"commands":["echo"],"wasm":"plugin.wasm"}"#,
        )
        .unwrap();
        fs::write(src.path().join("trust.json"), r#"{"trusted":true}"#).unwrap();
        fs::write(src.path().join("plugin.wasm"), b"\0asm").unwrap();
        let dest = tempfile::tempdir().unwrap();
        let installed = install_from_dir(src.path(), dest.path()).unwrap();
        assert_eq!(installed.id, "echo");
        assert!(installed.wasm_path.exists());
        assert_eq!(installed.commands, vec!["echo"]);
    }

    #[test]
    fn ships_three_example_plugins() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        for name in ["echo", "theme-lux-dark-plus", "identity-fmt"] {
            let dest = tempfile::tempdir().unwrap();
            let installed = install_from_dir(&root.join(name), dest.path()).unwrap();
            assert!(!installed.id.is_empty());
        }
    }
}
