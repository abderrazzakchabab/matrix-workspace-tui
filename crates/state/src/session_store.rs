use api_client::WorkspaceSelection;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Everything persisted on disk for the TUI session.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionData {
    pub cookie: Option<String>,
    /// Workspaces this client created (the backend has no list endpoint).
    pub workspaces: Vec<WorkspaceSelection>,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("could not determine the config directory")]
    NoConfigDir,
    #[error("session file {path} is corrupted: {source}")]
    Corrupted {
        path: String,
        source: serde_json::Error,
    },
    #[error("could not read/write {path}: {source}")]
    Io { path: String, source: io::Error },
    #[error("could not serialize session data: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub struct SessionStore {
    path: PathBuf,
}

impl SessionStore {
    /// ~/.config/matrix-workspace-tui/session.json (via the `dirs` crate).
    pub fn default_path() -> Result<PathBuf, StateError> {
        let dir = dirs::config_dir().ok_or(StateError::NoConfigDir)?;
        Ok(dir.join("matrix-workspace-tui").join("session.json"))
    }

    pub fn at_path(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<SessionData, StateError> {
        match fs::read(&self.path) {
            Ok(bytes) => {
                let data: SessionData = serde_json::from_slice(&bytes).map_err(|source| {
                    StateError::Corrupted {
                        path: self.path.display().to_string(),
                        source,
                    }
                })?;
                Ok(data)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SessionData::default()),
            Err(source) => Err(StateError::Io {
                path: self.path.display().to_string(),
                source,
            }),
        }
    }

    /// Write the session file (creating parent dirs) and chmod it 0600.
    pub fn save(&self, data: &SessionData) -> Result<(), StateError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| StateError::Io {
                path: parent.display().to_string(),
                source,
            })?;
        }
        let json = serde_json::to_vec_pretty(data)?;
        fs::write(&self.path, &json).map_err(|source| StateError::Io {
            path: self.path.display().to_string(),
            source,
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600)).map_err(
                |source| StateError::Io {
                    path: self.path.display().to_string(),
                    source,
                },
            )?;
        }
        Ok(())
    }

    /// Remove the session file. Missing file is not an error.
    pub fn clear(&self) -> Result<(), StateError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(StateError::Io {
                path: self.path.display().to_string(),
                source,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use api_client::WorkspaceSelection;
    use tempfile::tempdir;

    fn workspace(id: &str) -> WorkspaceSelection {
        WorkspaceSelection {
            workspace_id: id.to_string(),
            name: "ws".to_string(),
            owner_id: "@u:example.org".to_string(),
            status: "active".to_string(),
            created_at: "2026-08-15T00:00:00.000Z".to_string(),
        }
    }

    #[test]
    fn save_then_load_round_trips_cookie_and_workspaces() {
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));

        let mut data = SessionData::default();
        data.cookie = Some("cp_session=abc123".to_string());
        data.workspaces.push(workspace("ws_1"));
        data.workspaces.push(workspace("ws_2"));

        store.save(&data).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded.cookie.as_deref(), Some("cp_session=abc123"));
        assert_eq!(loaded.workspaces.len(), 2);
        assert_eq!(loaded.workspaces[1].workspace_id, "ws_2");
    }

    #[test]
    fn missing_file_loads_as_default() {
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        let loaded = store.load().unwrap();
        assert_eq!(loaded, SessionData::default());
    }

    #[test]
    fn save_creates_parent_directories() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b");
        let store = SessionStore::at_path(nested.join("session.json"));
        store.save(&SessionData::default()).unwrap();
        assert!(nested.join("session.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn save_sets_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let store = SessionStore::at_path(dir.path().join("session.json"));
        store.save(&SessionData::default()).unwrap();
        let mode = fs::metadata(dir.path().join("session.json")).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "session file must not be world-readable");
    }

    #[test]
    fn clear_removes_the_file_and_missing_file_is_not_an_error() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        let store = SessionStore::at_path(&path);
        store.save(&SessionData::default()).unwrap();
        store.clear().unwrap();
        assert!(!path.exists());
        store.clear().unwrap(); // second clear on a missing file is fine
    }

    #[test]
    fn corrupted_file_surfaces_corrupted_error_then_clear_recovers() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("session.json");
        fs::write(&path, "{not valid json").unwrap();
        let store = SessionStore::at_path(&path);

        let error = store.load().unwrap_err();
        assert!(matches!(error, StateError::Corrupted { .. }));

        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), SessionData::default());
    }

    #[test]
    fn default_path_points_into_config_dir() {
        let path = SessionStore::default_path().unwrap();
        let expected = dirs::config_dir().unwrap().join("matrix-workspace-tui").join("session.json");
        assert_eq!(path, expected);
    }
}
