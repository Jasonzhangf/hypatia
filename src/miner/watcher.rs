use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::MinerConfig;
use super::scanner::FileEntry;
use super::Scanner;

/// Tracks known file hashes for incremental re-scanning
pub struct Watcher {
    scanner: Scanner,
    /// Maps relative path -> sha256
    known: HashMap<String, String>,
}

impl Watcher {
    pub fn new(config: MinerConfig) -> Self {
        Self {
            scanner: Scanner::new(config.clone()),
            known: HashMap::new(),
        }
    }

    /// Load known hashes from a cache file
    pub fn load_cache(&mut self, path: &Path) {
        if let Ok(data) = std::fs::read_to_string(path) {
            if let Ok(map) = serde_json::from_str(&data) {
                self.known = map;
            }
        }
    }

    /// Save known hashes to a cache file
    pub fn save_cache(&self, path: &Path) {
        if let Ok(data) = serde_json::to_string(&self.known) {
            let _ = std::fs::write(path, data);
        }
    }

    /// Incremental scan: returns (new, modified, deleted) file entries
    pub fn scan_incremental(&mut self, root: &Path) -> (Vec<FileEntry>, Vec<FileEntry>, Vec<PathBuf>) {
        let current = self.scanner.scan();
        let current_map: HashMap<String, FileEntry> = current
            .into_iter()
            .map(|e| {
                let rel = e.path.strip_prefix(root).unwrap_or(&e.path).to_string_lossy().to_string();
                (rel, e)
            })
            .collect();

        let mut new_files = Vec::new();
        let mut modified = Vec::new();

        for (rel, entry) in &current_map {
            match self.known.get(rel) {
                None => new_files.push(entry.clone()),
                Some(sha) => {
                    if &entry.sha256 != sha {
                        modified.push(entry.clone());
                    }
                }
            }
        }

        let deleted: Vec<PathBuf> = self.known
            .keys()
            .filter(|k| !current_map.contains_key(*k))
            .map(|k| root.join(k))
            .collect();

        // Update known
        for (rel, entry) in &current_map {
            self.known.insert(rel.clone(), entry.sha256.clone());
        }
        for rel in &deleted {
            let key = rel.strip_prefix(root).unwrap_or(rel).to_string_lossy().to_string();
            self.known.remove(&key);
        }

        (new_files, modified, deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_new() {
        let config = MinerConfig::default();
        let _watcher = Watcher::new(config);
    }
}
