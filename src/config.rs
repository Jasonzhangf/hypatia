use crate::error::{HypatiaError, Result};
use crate::model::LocalProjectConfig;
use std::path::{Path, PathBuf};

/// Default global config path: ~/.hypatia/config.toml
pub fn global_config_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".hypatia").join("config.toml")
}

/// Try to load local project config from .hypatia/project.toml
pub fn load_local_config(root: &Path) -> Result<Option<LocalProjectConfig>> {
    let config_path = root.join(".hypatia").join("project.toml");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let config: LocalProjectConfig = toml::from_str(&content)
            .map_err(|e| HypatiaError::Toml(e.to_string()))?;
        Ok(Some(config))
    } else {
        Ok(None)
    }
}

/// Save local project config
pub fn save_local_config(root: &Path, config: &LocalProjectConfig) -> Result<()> {
    let dir = root.join(".hypatia");
    std::fs::create_dir_all(&dir)?;
    let content = toml::to_string_pretty(config)
        .map_err(|e| HypatiaError::Toml(e.to_string()))?;
    std::fs::write(dir.join("project.toml"), content)?;
    Ok(())
}

/// Generate a template config file for a new project
pub fn generate_template_config(root: &Path, name: &str) -> Result<LocalProjectConfig> {
    let config = LocalProjectConfig {
        name: name.to_string(),
        wing: None,
        room: None,
        skip_patterns: vec![
            "target/**".into(),
            "node_modules/**".into(),
            ".git/**".into(),
            "dist/**".into(),
            "*.lock".into(),
        ],
        extensions: vec![
            "rs".into(), "ts".into(), "js".into(), "py".into(),
            "md".into(), "json".into(), "yaml".into(), "yml".into(),
            "toml".into(), "txt".into(),
        ],
        max_file_size: 1024 * 1024,
        chunk_size: 512,
    };
    save_local_config(root, &config)?;
    Ok(config)
}
