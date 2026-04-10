pub mod chunker;
pub mod scanner;
pub mod watcher;

pub use chunker::{Chunk, Chunker};
pub use scanner::Scanner;
pub use watcher::Watcher;


/// Miner configuration
#[derive(Debug, Clone)]
pub struct MinerConfig {
    /// Root directory to scan
    pub root: std::path::PathBuf,
    /// Maximum file size to process (bytes)
    pub max_file_size: usize,
    /// Chunk size limit (characters)
    pub chunk_size: usize,
    /// Chunk overlap (characters)
    pub chunk_overlap: usize,
    /// File extensions to include
    pub extensions: Vec<String>,
    /// Skip patterns (gitignore-style)
    pub skip_patterns: Vec<String>,
}

impl Default for MinerConfig {
    fn default() -> Self {
        Self {
            root: std::path::PathBuf::from("."),
            max_file_size: 1024 * 1024,  // 1MB
            chunk_size: 512,
            chunk_overlap: 64,
            extensions: vec![
                "rs".into(), "ts".into(), "js".into(), "py".into(),
                "md".into(), "json".into(), "yaml".into(), "yml".into(),
                "toml".into(), "txt".into(),
            ],
            skip_patterns: vec![
                "target/**".into(),
                "node_modules/**".into(),
                ".git/**".into(),
                "dist/**".into(),
                "*.lock".into(),
            ],
        }
    }
}
