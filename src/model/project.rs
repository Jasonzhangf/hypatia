use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Project configuration stored in ~/.hypatia/projects.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    /// Project name (unique identifier)
    pub name: String,
    /// Root directory path
    pub root: PathBuf,
    /// Shelf name for this project
    pub shelf: String,
    /// Wing name (optional, for hierarchical organization)
    pub wing: Option<String>,
    /// Room name (optional, for deeper hierarchy)
    pub room: Option<String>,
    /// Project-specific skip patterns (gitignore-style)
    pub skip_patterns: Vec<String>,
    /// File extensions to include
    pub extensions: Vec<String>,
    /// Maximum file size in bytes
    pub max_file_size: usize,
    /// Chunk size in characters
    pub chunk_size: usize,
    /// Auto-watch enabled
    pub auto_watch: bool,
    /// Created timestamp
    pub created_at: String,
    /// Last indexed timestamp
    pub last_indexed: Option<String>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            name: String::new(),
            root: PathBuf::from("."),
            shelf: "default".to_string(),
            wing: None,
            room: None,
            skip_patterns: vec![
                "target/**".into(),
                "node_modules/**".into(),
                ".git/**".into(),
                "dist/**".into(),
                "*.lock".into(),
                "*.log".into(),
            ],
            extensions: vec![
                "rs".into(), "ts".into(), "js".into(), "py".into(),
                "md".into(), "json".into(), "yaml".into(), "yml".into(),
                "toml".into(), "txt".into(),
            ],
            max_file_size: 1024 * 1024,  // 1MB
            chunk_size: 512,
            auto_watch: false,
            created_at: chrono::Utc::now().to_rfc3339(),
            last_indexed: None,
        }
    }
}

impl Project {
    pub fn new(name: String, root: PathBuf) -> Self {
        Self {
            name: name.clone(),
            root,
            shelf: name.clone(),  // Default shelf = project name
            ..Self::default()
        }
    }
    
    /// Create a .hypatia directory path for this project
    pub fn hypatia_dir(&self) -> PathBuf {
        self.root.join(".hypatia")
    }
    
    /// Create a project-local config file path
    pub fn config_path(&self) -> PathBuf {
        self.hypatia_dir().join("project.toml")
    }
}

/// Project registry stored in ~/.hypatia/projects.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectRegistry {
    pub projects: Vec<Project>,
}

impl ProjectRegistry {
    pub fn new() -> Self {
        Self { projects: Vec::new() }
    }
    
    pub fn add(&mut self, project: Project) {
        // Remove existing with same name
        self.projects.retain(|p| p.name != project.name);
        self.projects.push(project);
    }
    
    pub fn remove(&mut self, name: &str) -> Option<Project> {
        let idx = self.projects.iter().position(|p| p.name == name)?;
        Some(self.projects.remove(idx))
    }
    
    pub fn get(&self, name: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.name == name)
    }
    
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Project> {
        self.projects.iter_mut().find(|p| p.name == name)
    }
    
    pub fn list(&self) -> &[Project] {
        &self.projects
    }
}

/// Wing concept: hierarchical organization within a shelf
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wing {
    pub name: String,
    pub shelf: String,
    pub description: Option<String>,
}

impl Wing {
    pub fn new(name: String, shelf: String) -> Self {
        Self {
            name: name.clone(),
            shelf,
            description: None,
        }
    }
}

/// Room concept: sub-division within a wing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Room {
    pub name: String,
    pub wing: String,
    pub shelf: String,
    pub description: Option<String>,
}

impl Room {
    pub fn new(name: String, wing: String, shelf: String) -> Self {
        Self {
            name: name.clone(),
            wing,
            shelf,
            description: None,
        }
    }
}

/// Local project configuration file (.hypatia/project.toml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalProjectConfig {
    /// Project name
    pub name: String,
    /// Skip patterns (extends global defaults)
    #[serde(default)]
    pub skip_patterns: Vec<String>,
    /// Extensions (extends global defaults)
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Max file size override
    #[serde(default = "default_max_file_size")]
    pub max_file_size: usize,
    /// Chunk size override
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    /// Wing assignment
    pub wing: Option<String>,
    /// Room assignment
    pub room: Option<String>,
}

fn default_max_file_size() -> usize { 1024 * 1024 }
fn default_chunk_size() -> usize { 512 }

impl Default for LocalProjectConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            skip_patterns: Vec::new(),
            extensions: Vec::new(),
            max_file_size: default_max_file_size(),
            chunk_size: default_chunk_size(),
            wing: None,
            room: None,
        }
    }
}
