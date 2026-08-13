//! One language server per (language, project root), as required by the
//! server-management deliverable.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::discovery;
use crate::session::ServerSession;

#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("no known language server for language `{0}`")]
    UnknownLanguage(String),
    #[error("project root {0:?} could not be resolved: {1}")]
    InvalidRoot(PathBuf, std::io::Error),
}

/// Owns every running language server session for a workspace.
#[derive(Default)]
pub struct ServerRegistry {
    sessions: Mutex<HashMap<(String, PathBuf), Arc<ServerSession>>>,
}

impl ServerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the running session for `language_id` rooted at `root`,
    /// starting one if none exists yet or the previous one has stopped.
    pub async fn get_or_start(
        &self,
        language_id: &str,
        root: &Path,
    ) -> Result<Arc<ServerSession>, LspError> {
        let root = root
            .canonicalize()
            .map_err(|err| LspError::InvalidRoot(root.to_path_buf(), err))?;
        let key = (language_id.to_string(), root.clone());

        let mut sessions = self.sessions.lock().await;
        if let Some(existing) = sessions.get(&key)
            && !existing.is_terminal()
        {
            return Ok(existing.clone());
        }

        let spec = discovery::spec_for_language(language_id)
            .copied()
            .ok_or_else(|| LspError::UnknownLanguage(language_id.to_string()))?;
        let session = Arc::new(ServerSession::spawn(spec, root.clone()));
        sessions.insert(key, session.clone());
        Ok(session)
    }

    /// Stop every managed session (used on workspace close / editor exit).
    pub async fn stop_all(&self) {
        let sessions: Vec<_> = self.sessions.lock().await.values().cloned().collect();
        for session in sessions {
            session.stop().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unknown_language_is_rejected_without_spawning_anything() {
        let registry = ServerRegistry::new();
        let root = std::env::current_dir().unwrap();
        match registry.get_or_start("not-a-real-language", &root).await {
            Err(LspError::UnknownLanguage(lang)) => assert_eq!(lang, "not-a-real-language"),
            Ok(_) => panic!("expected UnknownLanguage, got Ok"),
            Err(other) => panic!("expected UnknownLanguage, got {other}"),
        }
    }
}
