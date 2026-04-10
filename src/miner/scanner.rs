use ignore::gitignore::GitignoreBuilder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use super::MinerConfig;

/// Metadata for a scanned file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: usize,
    pub sha256: String,
    pub modified: std::time::SystemTime,
}

/// Incremental file scanner
pub struct Scanner {
    config: MinerConfig,
    gitignore: ignore::gitignore::Gitignore,
}

impl Scanner {
    pub fn new(config: MinerConfig) -> Self {
        let mut builder = GitignoreBuilder::new(&config.root);
        for pat in &config.skip_patterns {
            let _ = builder.add_line(None, pat);
        }
        let gitignore = builder.build().unwrap_or_else(|_| {
            GitignoreBuilder::new(&config.root).build().unwrap()
        });
        Self { config, gitignore }
    }

    /// Scan directory, returning Vec<FileEntry> for files that should be indexed.
    /// Respects skip_patterns and extension filters.
    pub fn scan(&self) -> Vec<FileEntry> {
        let mut entries = Vec::new();
        for entry in WalkDir::new(&self.config.root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if self.should_skip(path) {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                if meta.len() as usize > self.config.max_file_size {
                    continue;
                }
                let sha256 = Self::compute_sha256(path);
                entries.push(FileEntry {
                    path: path.to_path_buf(),
                    size: meta.len() as usize,
                    sha256,
                    modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                });
            }
        }
        entries
    }

    /// Returns FileEntry if file has changed (different sha256), None otherwise.
    /// Used for incremental indexing.
    pub fn check_changes(&self, path: &Path, known_sha256: &str) -> Option<FileEntry> {
        let sha256 = Self::compute_sha256(path);
        if sha256 != known_sha256 {
            if let Ok(meta) = std::fs::metadata(path) {
                return Some(FileEntry {
                    path: path.to_path_buf(),
                    size: meta.len() as usize,
                    sha256,
                    modified: meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                });
            }
        }
        None
    }

    fn should_skip(&self, path: &Path) -> bool {
        // Check gitignore
        if self.gitignore.matched(path, false).is_ignore() {
            return true;
        }
        // Check extension
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if !self.config.extensions.contains(&ext.to_lowercase()) {
                return true;
            }
        } else {
            return true;
        }
        false
    }

    fn compute_sha256(path: &Path) -> String {
        let mut hasher = Sha256::new();
        if let Ok(mut f) = std::fs::File::open(path) {
            let mut buf = [0u8; 8192];
            loop {
                match f.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => hasher.update(&buf[..n]),
                    Err(_) => break,
                }
            }
        }
        format!("{:x}", hasher.finalize())
    }
}

pub fn detect_lang(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("unknown")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_lang() {
        assert_eq!(detect_lang(Path::new("foo.rs")), "rs");
        assert_eq!(detect_lang(Path::new("bar.PY")), "py");
        assert_eq!(detect_lang(Path::new("noext")), "unknown");
    }
}
