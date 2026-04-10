use std::path::Path;
use rusqlite::{Connection, params};

use crate::error::{HypatiaError, Result, StorageError};
use crate::embed::EMBEDDING_DIM;
use serde::{Deserialize, Serialize};

/// A vector result with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorResult {
    pub id: i64,
    pub source: String,
    pub lang: String,
    pub text: String,
    pub symbol: Option<String>,
    pub score: f64,  // cosine similarity (0-1)
}

/// Vector store backed by SQLite
/// Uses SQLite's math functions for cosine similarity
pub struct VectorStore {
    conn: Connection,
}

impl VectorStore {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(StorageError::from)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS vectors (
                id INTEGER PRIMARY KEY,
                source TEXT NOT NULL,
                lang TEXT NOT NULL DEFAULT '',
                text TEXT NOT NULL,
                symbol TEXT DEFAULT NULL,
                sha256 TEXT NOT NULL DEFAULT '',
                chunk_start INTEGER DEFAULT 0,
                chunk_end INTEGER DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS vector_embeddings (
                vector_id INTEGER PRIMARY KEY REFERENCES vectors(id),
                embedding BLOB NOT NULL  -- 384 floats as f32 binary
            );

            CREATE INDEX IF NOT EXISTS idx_vectors_source ON vectors(source);
        "#).map_err(StorageError::from)?;

        Ok(())
    }

    /// Insert or update a vector entry with its embedding
    pub fn upsert(&self, source: &str, lang: &str, text: &str, symbol: Option<&str>, sha256: &str, chunk_start: usize, chunk_end: usize, embedding: &[f32]) -> Result<i64> {
        let existing_id: Option<i64> = self.conn.query_row(
            "SELECT id FROM vectors WHERE source = ? AND sha256 = ?",
            params![source, sha256],
            |row| row.get(0),
        ).ok();

        let id = if let Some(id) = existing_id {
            // Update
            self.conn.execute(
                "UPDATE vectors SET text = ?, lang = ?, symbol = ?, chunk_start = ?, chunk_end = ? WHERE id = ?",
                params![text, lang, symbol, chunk_start as i64, chunk_end as i64, id],
            ).map_err(StorageError::from)?;
            id
        } else {
            // Insert
            self.conn.execute(
                "INSERT INTO vectors (source, lang, text, symbol, sha256, chunk_start, chunk_end) VALUES (?, ?, ?, ?, ?, ?, ?)",
                params![source, lang, text, symbol, sha256, chunk_start as i64, chunk_end as i64],
            ).map_err(StorageError::from)?;
            self.conn.last_insert_rowid()
        };

        // Store embedding as binary blob
        let embedding_bytes = unsafe {
            std::slice::from_raw_parts(
                embedding.as_ptr() as *const u8,
                embedding.len() * std::mem::size_of::<f32>(),
            )
        };

        self.conn.execute(
            "INSERT OR REPLACE INTO vector_embeddings (vector_id, embedding) VALUES (?, ?)",
            params![id, embedding_bytes],
        ).map_err(StorageError::from)?;

        Ok(id)
    }

    /// Search for similar vectors using cosine similarity
    pub fn search(&self, query_embedding: &[f32], limit: i64, offset: i64) -> Result<Vec<VectorResult>> {
        if query_embedding.len() != EMBEDDING_DIM {
            return Err(HypatiaError::Validation(
                format!("Query embedding dimension {} does not match expected {}", query_embedding.len(), EMBEDDING_DIM),
            ));
        }

        // Load all embeddings and compute cosine similarity in Rust
        // SQLite doesn't have native vector operations, so we do it in Rust
        let mut stmt = self.conn.prepare(
            "SELECT v.id, v.source, v.lang, v.text, v.symbol, ve.embedding
             FROM vectors v
             JOIN vector_embeddings ve ON v.id = ve.vector_id
             LIMIT ? OFFSET ?"
        ).map_err(StorageError::from)?;

        let rows = stmt.query_map(params![limit, offset], |row| {
            let id: i64 = row.get(0)?;
            let source: String = row.get(1)?;
            let lang: String = row.get(2)?;
            let text: String = row.get(3)?;
            let symbol: Option<String> = row.get(4)?;
            let embedding_blob: Vec<u8> = row.get(5)?;
            Ok((id, source, lang, text, symbol, embedding_blob))
        }).map_err(StorageError::from)?;

        let query_vec: Vec<f32> = query_embedding.to_vec();
        let mut results: Vec<VectorResult> = Vec::new();

        for row in rows {
            let (id, source, lang, text, symbol, blob) = row.map_err(StorageError::from)?;
            let embedding = unsafe {
                std::slice::from_raw_parts(
                    blob.as_ptr() as *const f32,
                    blob.len() / std::mem::size_of::<f32>(),
                )
            };

            let score = cosine_similarity(&query_vec, embedding);
            results.push(VectorResult {
                id, source, lang, text, symbol,
                score,
            });
        }

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(results)
    }

    /// Delete vectors matching a source prefix
    pub fn delete_by_source_prefix(&self, prefix: &str) -> Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM vector_embeddings WHERE vector_id IN (SELECT id FROM vectors WHERE source LIKE ?)",
            params![format!("{}%", prefix)],
        ).map_err(StorageError::from)?;

        let _ = self.conn.execute(
            "DELETE FROM vectors WHERE source LIKE ?",
            params![format!("{}%", prefix)],
        ).map_err(StorageError::from)?;

        Ok(count)
    }

    /// Get the total number of vectors
    pub fn count(&self) -> Result<i64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM vectors",
            [],
            |row| row.get(0),
        ).map_err(StorageError::from)?;
        Ok(count)
    }
}

/// Compute cosine similarity between two vectors
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;

    for i in 0..a.len() {
        dot += (a[i] as f64) * (b[i] as f64);
        norm_a += (a[i] as f64) * (a[i] as f64);
        norm_b += (b[i] as f64) * (b[i] as f64);
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}
