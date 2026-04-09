use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMetadata {
    pub session_id: String,
    pub name: String,
    pub profile: String,
    pub rootfs_path: String,
    pub workspace: String,
    pub created_at: String,
    pub last_exec_at: Option<String>,
    pub pid: Option<u32>,
}

pub struct SessionDirs {
    pub root: PathBuf,
    pub workspace: PathBuf,
    pub home: PathBuf,
    pub cache: PathBuf,
    pub logs: PathBuf,
    pub metadata_path: PathBuf,
}

pub fn sessions_base_dir() -> PathBuf {
    let data_dir = std::env::var("NIXOSANDBOX_DATA_DIR")
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            format!("{}/.local/share/nixosandbox", home)
        });
    PathBuf::from(data_dir).join("sessions")
}

fn generate_session_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

pub fn create_session(
    name: &str, profile: &str, rootfs_path: &str, workspace: Option<&str>,
) -> Result<SessionMetadata, String> {
    let session_id = generate_session_id();
    let base = sessions_base_dir();
    let session_dir = base.join(&session_id);
    fs::create_dir_all(&session_dir).map_err(|e| format!("failed to create session dir: {e}"))?;
    let home_dir = session_dir.join("home");
    let cache_dir = session_dir.join("cache");
    let logs_dir = session_dir.join("logs");
    fs::create_dir_all(&home_dir).map_err(|e| format!("failed to create home dir: {e}"))?;
    fs::create_dir_all(&cache_dir).map_err(|e| format!("failed to create cache dir: {e}"))?;
    fs::create_dir_all(&logs_dir).map_err(|e| format!("failed to create logs dir: {e}"))?;

    let workspace_dir = session_dir.join("workspace");
    let workspace_path = if let Some(ws) = workspace {
        let ws_path = Path::new(ws);
        if !ws_path.exists() {
            return Err(format!("workspace path does not exist: {ws}"));
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(ws_path, &workspace_dir)
            .map_err(|e| format!("failed to symlink workspace: {e}"))?;
        ws.to_string()
    } else {
        fs::create_dir_all(&workspace_dir).map_err(|e| format!("failed to create workspace: {e}"))?;
        workspace_dir.to_string_lossy().to_string()
    };

    let metadata = SessionMetadata {
        session_id: session_id.clone(),
        name: name.to_string(),
        profile: profile.to_string(),
        rootfs_path: rootfs_path.to_string(),
        workspace: workspace_path,
        created_at: crate::timestamps::now_iso8601(),
        last_exec_at: None,
        pid: None,
    };
    let metadata_path = session_dir.join("metadata.json");
    let json = serde_json::to_string_pretty(&metadata).map_err(|e| format!("serialize: {e}"))?;
    fs::write(&metadata_path, json).map_err(|e| format!("write metadata: {e}"))?;
    Ok(metadata)
}

pub fn list_sessions() -> Result<Vec<SessionMetadata>, String> {
    let base = sessions_base_dir();
    if !base.exists() { return Ok(vec![]); }
    let mut sessions = Vec::new();
    let entries = fs::read_dir(&base).map_err(|e| format!("read sessions dir: {e}"))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("read dir entry: {e}"))?;
        let metadata_path = entry.path().join("metadata.json");
        if metadata_path.exists() {
            let content = fs::read_to_string(&metadata_path).map_err(|e| format!("read metadata: {e}"))?;
            if let Ok(meta) = serde_json::from_str::<SessionMetadata>(&content) {
                sessions.push(meta);
            }
        }
    }
    sessions.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(sessions)
}

pub fn load_session(session_id: &str) -> Result<SessionMetadata, String> {
    let path = sessions_base_dir().join(session_id).join("metadata.json");
    if !path.exists() { return Err(format!("session '{}' not found", session_id)); }
    let content = fs::read_to_string(&path).map_err(|e| format!("read metadata: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("parse metadata: {e}"))
}

pub fn session_dirs(session_id: &str) -> SessionDirs {
    let root = sessions_base_dir().join(session_id);
    SessionDirs {
        workspace: root.join("workspace"), home: root.join("home"),
        cache: root.join("cache"), logs: root.join("logs"),
        metadata_path: root.join("metadata.json"), root,
    }
}

pub fn touch_last_exec(session_id: &str) -> Result<(), String> {
    let mut meta = load_session(session_id)?;
    meta.last_exec_at = Some(crate::timestamps::now_iso8601());
    let dirs = session_dirs(session_id);
    let json = serde_json::to_string_pretty(&meta).map_err(|e| format!("serialize: {e}"))?;
    fs::write(&dirs.metadata_path, json).map_err(|e| format!("write metadata: {e}"))
}

pub fn destroy_session(session_id: &str) -> Result<(), String> {
    let dirs = session_dirs(session_id);
    if !dirs.root.exists() { return Err(format!("session '{}' not found", session_id)); }
    fs::remove_dir_all(&dirs.root).map_err(|e| format!("remove session dir: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_temp_data_dir<F: FnOnce()>(f: F) {
        let dir = std::env::temp_dir().join(format!("nixosandbox-test-{}", uuid::Uuid::new_v4()));
        std::env::set_var("NIXOSANDBOX_DATA_DIR", &dir);
        f();
        let _ = fs::remove_dir_all(&dir);
        std::env::remove_var("NIXOSANDBOX_DATA_DIR");
    }

    #[test]
    fn create_and_list_sessions() {
        with_temp_data_dir(|| {
            let meta = create_session("test-session", "strict", "/nix/store/fake", None).unwrap();
            assert_eq!(meta.name, "test-session");
            let sessions = list_sessions().unwrap();
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].session_id, meta.session_id);
        });
    }

    #[test]
    fn load_session_by_id() {
        with_temp_data_dir(|| {
            let meta = create_session("load-test", "strict", "/nix/store/fake", None).unwrap();
            let loaded = load_session(&meta.session_id).unwrap();
            assert_eq!(loaded.name, "load-test");
        });
    }

    #[test]
    fn destroy_session_removes_dir() {
        with_temp_data_dir(|| {
            let meta = create_session("rm-test", "strict", "/nix/store/fake", None).unwrap();
            let dirs = session_dirs(&meta.session_id);
            assert!(dirs.root.exists());
            destroy_session(&meta.session_id).unwrap();
            assert!(!dirs.root.exists());
        });
    }

    #[test]
    fn destroy_nonexistent_errors() {
        with_temp_data_dir(|| {
            assert!(destroy_session("nonexistent").is_err());
        });
    }

    #[test]
    fn create_with_external_workspace() {
        with_temp_data_dir(|| {
            let ws = std::env::temp_dir().join(format!("ws-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&ws).unwrap();
            let meta = create_session("ws-test", "strict", "/nix/store/fake", Some(ws.to_str().unwrap())).unwrap();
            let dirs = session_dirs(&meta.session_id);
            assert!(dirs.workspace.is_symlink());
            destroy_session(&meta.session_id).unwrap();
            assert!(ws.exists()); // external workspace preserved
            let _ = fs::remove_dir_all(&ws);
        });
    }

    #[test]
    fn metadata_roundtrip() {
        let meta = SessionMetadata {
            session_id: "abc".to_string(), name: "test".to_string(),
            profile: "strict".to_string(), rootfs_path: "/nix/store/fake".to_string(),
            workspace: "/tmp/ws".to_string(), created_at: "2026-04-08T12:00:00Z".to_string(),
            last_exec_at: None, pid: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let de: SessionMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(de.session_id, "abc");
    }
}
