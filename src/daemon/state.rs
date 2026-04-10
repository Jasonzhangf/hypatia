use crate::error::{HypatiaError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;

/// State for the watch daemon - tracks all watched directories
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DaemonState {
    pub is_running: bool,
    pub pid: Option<u32>,
    pub watched: Vec<WatchedEntry>,
    pub last_check: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WatchedEntry {
    pub path: String,
    pub shelf: String,
    pub project_name: String,
    pub last_modified: Option<String>,
    pub files_indexed: usize,
}

impl DaemonState {
    /// Path to the daemon state file
    pub fn state_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".hypatia").join("daemon_state.json")
    }

    /// Load daemon state from disk
    pub fn load() -> Result<Self> {
        let path = Self::state_path();
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content).map_err(|e| {
                HypatiaError::IoMsg(format!("Failed to parse daemon state: {}", e))
            })
        } else {
            Ok(Self::default())
        }
    }

    /// Save daemon state to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::state_path();
        let parent = path.parent().ok_or_else(|| {
            HypatiaError::IoMsg("Invalid daemon state path".into())
        })?;
        fs::create_dir_all(parent)?;
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Add a watched entry
    pub fn add_watch(&mut self, path: &str, shelf: &str, project_name: &str) {
        // Remove existing entry for this path if present
        self.watched.retain(|e| e.path != path);
        self.watched.push(WatchedEntry {
            path: path.to_string(),
            shelf: shelf.to_string(),
            project_name: project_name.to_string(),
            last_modified: None,
            files_indexed: 0,
        });
    }

    /// Remove a watched entry
    pub fn remove_watch(&mut self, path: &str) -> bool {
        let len = self.watched.len();
        self.watched.retain(|e| e.path != path);
        self.watched.len() != len
    }

    /// Mark the daemon as running
    pub fn mark_running(&mut self, pid: u32) {
        self.is_running = true;
        self.pid = Some(pid);
        self.last_check = Some(chrono::Utc::now().to_rfc3339());
    }

    /// Mark the daemon as stopped
    pub fn mark_stopped(&mut self) {
        self.is_running = false;
        self.pid = None;
    }

    /// Check if the daemon process is actually running
    pub fn is_process_alive(&self) -> bool {
        if let Some(pid) = self.pid {
            // Check if process exists - on Unix, kill(pid, 0) checks without sending signal
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(pid as i32, 0) == 0
                }
            }
            #[cfg(not(unix))]
            {
                false
            }
        } else {
            false
        }
    }
}
