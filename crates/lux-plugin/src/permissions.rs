//! User-granted capabilities, persisted as JSON next to a workspace.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    FileRead,
    FileWrite,
    Network,
    Process,
}

#[derive(Debug, thiserror::Error)]
pub enum GrantError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionSet {
    granted: BTreeSet<Capability>,
}

impl PermissionSet {
    pub fn new(granted: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            granted: granted.into_iter().collect(),
        }
    }

    pub fn allows(&self, cap: Capability) -> bool {
        self.granted.contains(&cap)
    }

    pub fn grant(&mut self, cap: Capability) {
        self.granted.insert(cap);
    }

    pub fn load(path: &Path) -> Result<Self, GrantError> {
        let bytes = fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    pub fn save(&self, path: &Path) -> Result<(), GrantError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }

    pub fn workspace_file(workspace_root: &Path, plugin_id: &str) -> PathBuf {
        workspace_root
            .join(".lux")
            .join("plugin-grants")
            .join(format!("{plugin_id}.json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_deny_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = PermissionSet::workspace_file(dir.path(), "echo");
        let mut set = PermissionSet::new([]);
        assert!(!set.allows(Capability::FileRead));
        set.grant(Capability::FileRead);
        set.save(&path).unwrap();
        let loaded = PermissionSet::load(&path).unwrap();
        assert!(loaded.allows(Capability::FileRead));
        assert!(!loaded.allows(Capability::Network));
    }
}
