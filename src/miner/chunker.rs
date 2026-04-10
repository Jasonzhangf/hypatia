use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

/// A single chunk of text from a source file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// Unique identifier: sha256 of "filepath:start_line:end_line"
    pub id: String,
    /// Source file path (relative to scan root)
    pub source: String,
    /// Language detected from extension
    pub lang: String,
    /// Start line number (1-based)
    pub start_line: usize,
    /// End line number (1-based)
    pub end_line: usize,
    /// The text content of this chunk
    pub text: String,
    /// Optional: function/class name if detectable
    pub symbol: Option<String>,
}

struct CodeBlock {
    start: usize,
    end: usize,
    symbol: Option<String>,
}

/// Code-aware text chunker
pub struct Chunker {
    max_chars: usize,
    overlap_chars: usize,
}

impl Chunker {
    pub fn new(max_chars: usize, overlap_chars: usize) -> Self {
        Self { max_chars, overlap_chars }
    }

    pub fn chunk(&self, source: &Path, lang: &str, content: &str) -> Vec<Chunk> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return Vec::new();
        }
        let blocks = self.detect_code_blocks(&lines, lang);
        let mut chunks = Vec::new();
        for block in blocks {
            let block_text: String = lines[block.start..=block.end].join("\n");
            if block_text.len() <= self.max_chars {
                let id = Self::compute_id(source, block.start + 1, block.end + 1);
                chunks.push(Chunk {
                    id,
                    source: source.to_string_lossy().to_string(),
                    lang: lang.to_string(),
                    start_line: block.start + 1,
                    end_line: block.end + 1,
                    text: block_text,
                    symbol: block.symbol,
                });
            } else {
                chunks.extend(self.split_large_block(source, lang, &lines, &block));
            }
        }
        chunks
    }

    fn compute_id(source: &Path, start: usize, end: usize) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}:{}", source.display(), start, end));
        format!("{:x}", hasher.finalize())
    }

    fn detect_code_blocks(&self, lines: &[&str], lang: &str) -> Vec<CodeBlock> {
        let mut blocks = Vec::new();
        let mut current_block: Option<CodeBlock> = None;
        let patterns: Vec<&str> = match lang {
            "rs" => vec!["fn ", "pub fn ", "async fn ", "struct ", "pub struct ", "enum ", "impl ", "trait "],
            "py" => vec!["def ", "async def ", "class "],
            "ts" | "js" => vec!["function ", "async function ", "class ", "export function ", "const ", "let "],
            _ => vec!["fn ", "function ", "def ", "class ", "struct "],
        };
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            let is_start = patterns.iter().any(|p| trimmed.starts_with(p));
            if is_start {
                if let Some(b) = current_block.take() {
                    blocks.push(b);
                }
                current_block = Some(CodeBlock { start: i, end: i, symbol: self.extract_symbol(trimmed) });
            } else if let Some(ref mut b) = current_block {
                b.end = i;
            }
        }
        if let Some(b) = current_block {
            blocks.push(b);
        }
        if blocks.is_empty() {
            blocks.push(CodeBlock { start: 0, end: lines.len().saturating_sub(1), symbol: None });
        }
        blocks
    }

    fn extract_symbol(&self, line: &str) -> Option<String> {
        for kw in ["fn ", "pub fn ", "struct ", "enum ", "impl ", "def ", "class ", "function ", "const "] {
            if line.starts_with(kw) {
                let rest = &line[kw.len()..];
                let name = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
                if !name.is_empty() { return Some(name); }
            }
        }
        None
    }

    fn split_large_block(&self, source: &Path, lang: &str, lines: &[&str], block: &CodeBlock) -> Vec<Chunk> {
        let block_lines = &lines[block.start..=block.end];
        let mut chunks = Vec::new();
        let mut current: Vec<&str> = Vec::new();
        let mut chars: usize = 0;
        let mut start = block.start;
        let overlap_lines = self.overlap_chars / 80 + 1;

        for (i, line) in block_lines.iter().enumerate() {
            let lc = line.len() + 1;
            if chars + lc > self.max_chars && !current.is_empty() {
                let text = current.join("\n");
                let id = Self::compute_id(source, start + 1, start + current.len());
                chunks.push(Chunk {
                    id, source: source.to_string_lossy().to_string(), lang: lang.to_string(),
                    start_line: start + 1, end_line: start + current.len(), text,
                    symbol: if start == block.start { block.symbol.clone() } else { None },
                });
                let keep = overlap_lines.min(current.len());
                current = current[current.len() - keep..].to_vec();
                chars = current.iter().map(|l| l.len() + 1).sum();
                start = block.start + i - keep;
            }
            current.push(line);
            chars += lc;
        }
        if !current.is_empty() {
            let text = current.join("\n");
            let end = start + current.len();
            chunks.push(Chunk {
                id: Self::compute_id(source, start + 1, end),
                source: source.to_string_lossy().to_string(),
                lang: lang.to_string(),
                start_line: start + 1,
                end_line: end,
                text,
                symbol: if start == block.start { block.symbol.clone() } else { None },
            });
        }
        chunks
    }
}
