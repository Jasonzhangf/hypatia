use crate::error::{HypatiaError, Result};
use crate::model::{LocalProjectConfig, Project, ProjectRegistry};
use crate::config::save_local_config;
use std::path::{Path, PathBuf};
use std::fs;

/// Project manager handles the project registry and project operations
pub struct ProjectManager {
    registry_path: PathBuf,
    registry: ProjectRegistry,
}

impl ProjectManager {
    /// Create a new project manager with the given hypatia home directory
    pub fn new(hypatia_home: &Path) -> Result<Self> {
        let registry_path = hypatia_home.join("projects.json");
        let registry = if registry_path.exists() {
            let content = fs::read_to_string(&registry_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            ProjectRegistry::default()
        };
        Ok(Self {
            registry_path,
            registry,
        })
    }
    
    /// Save registry to disk
    fn save_registry(&self) -> Result<()> {
        let parent = self.registry_path.parent()
            .ok_or_else(|| HypatiaError::IoMsg("Invalid registry path".into()))?;
        fs::create_dir_all(parent)?;
        let content = serde_json::to_string_pretty(&self.registry)?;
        fs::write(&self.registry_path, content)?;
        Ok(())
    }
    
    /// Add a new project
    pub fn add_project(&mut self, name: String, root: PathBuf, shelf: Option<String>, wing: Option<String>, room: Option<String>) -> Result<Project> {
        // Check if root exists
        let canonical_root = root.canonicalize()
            .map_err(|e| HypatiaError::IoMsg(format!("Project root does not exist: {}: {}", root.display(), e)))?;
        
        // Create project
        let project = Project {
            name: name.clone(),
            root: canonical_root.clone(),
            shelf: shelf.unwrap_or_else(|| name.clone()),
            wing,
            room,
            ..Project::default()
        };
        
        // Generate local config file if it doesn't exist
        let local_config_path = canonical_root.join(".hypatia").join("project.toml");
        if !local_config_path.exists() {
            let config = LocalProjectConfig {
                name: name.clone(),
                wing: project.wing.clone(),
                room: project.room.clone(),
                ..LocalProjectConfig::default()
            };
            save_local_config(&canonical_root, &config)?;
        }
        
        // Add to registry
        self.registry.add(project.clone());
        self.save_registry()?;
        
        Ok(project)
    }
    
    /// Remove a project from the registry
    pub fn remove_project(&mut self, name: &str) -> Result<Option<Project>> {
        let removed = self.registry.remove(name);
        if removed.is_some() {
            self.save_registry()?;
        }
        Ok(removed)
    }
    
    /// Get a project by name
    pub fn get_project(&self, name: &str) -> Option<&Project> {
        self.registry.get(name)
    }
    
    /// Get a mutable reference to a project by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Project> {
        self.registry.get_mut(name)
    }
    
    /// List all projects
    pub fn list_projects(&self) -> &[Project] {
        self.registry.list()
    }
    
    /// Update project's last indexed time
    pub fn update_last_indexed(&mut self, name: &str) -> Result<()> {
        if let Some(project) = self.registry.get_mut(name) {
            project.last_indexed = Some(chrono::Utc::now().to_rfc3339());
            self.save_registry()?;
        }
        Ok(())
    }
    
    /// Find project by root directory
    pub fn find_by_root(&self, root: &Path) -> Option<&Project> {
        let canonical = root.canonicalize().ok()?;
        self.registry.projects.iter()
            .find(|p| p.root == canonical)
    }
    
    /// Toggle auto-watch for a project
    pub fn toggle_auto_watch(&mut self, name: &str, enabled: bool) -> Result<()> {
        if let Some(project) = self.registry.get_mut(name) {
            project.auto_watch = enabled;
            self.save_registry()?;
        }
        Ok(())
    }
}
