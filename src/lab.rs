use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashMap;

use crate::engine::Evaluator;
use crate::embed;
use crate::error::{HypatiaError, Result};
use crate::hybrid::{merge_hybrid, HybridResult};
use crate::miner::{Chunker, MinerConfig, Scanner, Watcher};
use crate::miner::scanner::detect_lang;
use crate::model::*;
use crate::service::{KnowledgeService, StatementService};
use crate::storage::{ShelfManager, Storage};
use crate::vector::VectorStore;

/// Vector result for display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorSearchResult {
    pub source: String,
    pub lang: String,
    pub text: String,
    pub symbol: Option<String>,
    pub score: f64,
}

pub struct Lab {
    shelf_manager: ShelfManager,
    vector_stores: HashMap<String, VectorStore>,
}

impl Lab {
    pub fn new() -> Result<Self> {
        let shelf_manager = ShelfManager::new();
        Ok(Self {
            shelf_manager,
            vector_stores: HashMap::new(),
        })
    }

    /// Create Lab with a specific shelf pre-connected
    pub fn with_shelf(shelf_name: &str) -> Result<Self> {
        let mut lab = Self::new()?;
        lab.ensure_shelf(shelf_name)?;
        Ok(lab)
    }

    // --- Shelf operations ---

    pub fn connect_shelf(&mut self, path: &Path, name: Option<&str>) -> Result<String> {
        let shelf_name = self.shelf_manager.connect(path, name)?;
        // Also open vector store
        let vec_path = path.join("vectors.sqlite");
        if let Ok(store) = VectorStore::open(&vec_path) {
            self.vector_stores.insert(shelf_name.clone(), store);
        }
        Ok(shelf_name)
    }

    /// Ensure a shelf is connected (auto-create if needed).
    /// Shelf directory: ~/.hypatia/shelves/<name>/
    pub fn ensure_shelf(&mut self, name: &str) -> Result<()> {
        if self.shelf_manager.get(name).is_some() {
            return Ok(());
        }
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let shelf_path = home.join(".hypatia").join("shelves").join(name);
        std::fs::create_dir_all(&shelf_path)?;
        let shelf_name = self.shelf_manager.connect(&shelf_path, Some(name))?;
        let vec_path = shelf_path.join("vectors.sqlite");
        if let Ok(store) = VectorStore::open(&vec_path) {
            self.vector_stores.insert(shelf_name, store);
        }
        Ok(())
    }

    pub fn disconnect_shelf(&mut self, name: &str) -> Result<()> {
        self.shelf_manager.disconnect(name)
    }

    pub fn list_shelves(&self) -> Vec<String> {
        self.shelf_manager.list().iter().map(|id| id.name.clone()).collect()
    }

    pub fn export_shelf(&self, name: &str, dest: &Path) -> Result<()> {
        self.shelf_manager.export(name, dest)
    }

    // --- JSE Query ---

    pub fn query(&mut self, shelf_name: &str, jse: &serde_json::Value) -> Result<QueryResult> {
        let shelf = self.shelf_manager.get(shelf_name).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf_name}' is not connected"))
        })?;
        Evaluator::execute(jse, shelf)
    }

    // --- Knowledge CRUD ---

    pub fn create_knowledge(&mut self, shelf: &str, name: &str, content: Content) -> Result<Knowledge> {
        let shelf_ref = self.shelf_manager.get_mut(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        let mut svc = KnowledgeService::new(shelf_ref);
        svc.create(name, content)
    }

    pub fn get_knowledge(&self, shelf: &str, name: &str) -> Result<Option<Knowledge>> {
        let shelf_ref = self.shelf_manager.get(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        Ok(shelf_ref.duckdb.get_knowledge(name)?)
    }

    pub fn update_knowledge(&mut self, shelf: &str, name: &str, content: Content) -> Result<Knowledge> {
        let shelf_ref = self.shelf_manager.get_mut(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        let mut svc = KnowledgeService::new(shelf_ref);
        svc.update(name, content)
    }

    pub fn delete_knowledge(&mut self, shelf: &str, name: &str) -> Result<()> {
        let shelf_ref = self.shelf_manager.get_mut(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        let mut svc = KnowledgeService::new(shelf_ref);
        svc.delete(name)
    }

    // --- Statement CRUD ---

    pub fn create_statement(
        &mut self,
        shelf: &str,
        key: &StatementKey,
        content: Content,
        tr_start: Option<NaiveDateTime>,
        tr_end: Option<NaiveDateTime>,
    ) -> Result<Statement> {
        let shelf_ref = self.shelf_manager.get_mut(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        let mut svc = StatementService::new(shelf_ref);
        svc.create(key, content, tr_start, tr_end)
    }

    pub fn get_statement(&self, shelf: &str, key: &StatementKey) -> Result<Option<Statement>> {
        let shelf_ref = self.shelf_manager.get(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        Ok(shelf_ref.duckdb.get_statement(key)?)
    }

    pub fn delete_statement(&mut self, shelf: &str, key: &StatementKey) -> Result<()> {
        let shelf_ref = self.shelf_manager.get_mut(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        let mut svc = StatementService::new(shelf_ref);
        svc.delete(key)
    }

    // --- Search ---

    pub fn search(&self, shelf: &str, query: &str, opts: &SearchOpts) -> Result<QueryResult> {
        let shelf_ref = self.shelf_manager.get(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        shelf_ref.execute_search(query, opts)
    }

    // --- Hybrid Search (FTS + Vector) ---

    pub fn hybrid_search(&mut self, shelf: &str, query: &str, limit: i64) -> Result<Vec<HybridResult>> {
        self.ensure_shelf(shelf)?;

        // 1. FTS search
        let fts_results = self.search(shelf, query, &SearchOpts {
            catalog: None,
            limit,
            offset: 0,
        })?;

        let fts_items: Vec<(String, String, String, Option<String>, f64)> = fts_results.rows.iter()
            .filter_map(|row| {
                let source = row.get("name").or_else(|| row.get("key")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let text = row.get("data").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let lang = row.get("tags").and_then(|v| v.as_array()).map(|a| a.iter().take(1).map(|v| v.as_str().unwrap_or("")).collect::<Vec<_>>().join(",")).unwrap_or_default();
                Some((source, lang, text, None, 0.0))
            })
            .collect();

        // 2. Vector search
        let vec_items = if let Some(vec_store) = self.vector_stores.get(shelf) {
            let query_emb = embed::embed(query);
            match vec_store.search(&query_emb, limit, 0) {
                Ok(results) => results.into_iter()
                    .map(|r| (r.source, r.lang, r.text, r.symbol, r.score))
                    .collect(),
                Err(e) => {
                    eprintln!("[hypatia] Vector search error: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // 3. Merge with RRF
        Ok(merge_hybrid(fts_items, vec_items, 60))
    }

    // --- Mining ---

    pub fn mine_directory(
        &mut self,
        shelf: &str,
        path: &Path,
        max_size: usize,
        chunk_size: usize,
        _include_hidden: bool,
    ) -> Result<usize> {
        self.ensure_shelf(shelf)?;

        let config = MinerConfig {
            root: path.to_path_buf(),
            max_file_size: max_size,
            chunk_size,
            chunk_overlap: 64,
            ..MinerConfig::default()
        };

        let scanner = Scanner::new(config);
        let chunker = Chunker::new(chunk_size, 64);

        let files = scanner.scan();
        let mut total_chunks = 0;

        let shelf_ref = self.shelf_manager.get_mut(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        let mut svc = KnowledgeService::new(shelf_ref);

        // Get vector store for this shelf (if available)
        let vec_store = self.vector_stores.get(shelf);

        for file in files {
            let content = match std::fs::read_to_string(&file.path) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let lang = detect_lang(&file.path);
            let source_rel = file.path.strip_prefix(path)
                .unwrap_or(&file.path)
                .to_string_lossy()
                .to_string();

            let chunks = chunker.chunk(&file.path, &lang, &content);

            for chunk in chunks {
                let kn_content = Content::new(&chunk.text)
                    .with_tags(vec!["code".to_string(), lang.clone(), source_rel.clone()]);
                let kn_name = format!("code:{}:{}-{}", source_rel, chunk.start_line, chunk.end_line);

                // Upsert: delete if exists then create
                if svc.get(&kn_name).is_ok() && svc.get(&kn_name)?.is_some() {
                    let _ = svc.delete(&kn_name);
                }

                let _ = svc.create(&kn_name, kn_content);

                // Store embedding in vector store
                if let Some(vs) = vec_store {
                    let embedding = embed::embed(&chunk.text);
                    let sha = {
                        use std::hash::{Hash, Hasher};
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        chunk.text.hash(&mut hasher);
                        format!("{:x}", hasher.finish())
                    };
                    let _ = vs.upsert(
                        &source_rel,
                        &lang,
                        &chunk.text,
                        None,
                        &sha,
                        chunk.start_line,
                        chunk.end_line,
                        &embedding,
                    );
                }

                total_chunks += 1;
            }
        }

        Ok(total_chunks)
    }

    // --- Incremental Watch ---

    pub fn incremental_scan(&self, _shelf: &str, path: &Path) -> Result<(usize, usize, usize)> {
        let config = MinerConfig {
            root: path.to_path_buf(),
            ..MinerConfig::default()
        };

        let mut watcher = Watcher::new(config);
        let cache_path = path.join(".hypatia_cache.json");
        watcher.load_cache(&cache_path);

        let (new, modified, deleted) = watcher.scan_incremental(path);

        watcher.save_cache(&cache_path);

        Ok((new.len(), modified.len(), deleted.len()))
    }

    // --- Status ---

    pub fn get_status(&self, shelf: &str) -> Result<String> {
        let shelves = self.list_shelves();
        let shelf_count = shelves.len();

        let shelf_ref = self.shelf_manager.get(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;

        let knowledge_count = shelf_ref.duckdb.knowledge_count()?;
        let statement_count = shelf_ref.duckdb.statement_count()?;
        let fts_count = shelf_ref.sqlite.doc_count()?;

        let vec_count = if let Some(vs) = self.vector_stores.get(shelf) {
            vs.count().unwrap_or(0)
        } else {
            0
        };

        let mut report = String::new();
        report.push_str("════════════════════════\n");
        report.push_str(&format!("Shelves connected: {}\n", shelf_count));
        for s in &shelves {
            report.push_str(&format!("  • {s}\n"));
        }
        report.push_str(&format!("Knowledge entries: {}\n", knowledge_count));
        report.push_str(&format!("Statements: {}\n", statement_count));
        report.push_str(&format!("FTS documents: {}\n", fts_count));
        report.push_str(&format!("Vector entries: {}\n", vec_count));

        if knowledge_count != fts_count {
            report.push_str(&format!("[WARN] FTS count ({}) != Knowledge count ({})\n", fts_count, knowledge_count));
            report.push_str("  → Run: hypatia rebuild-fts <shelf>\n");
        } else {
            report.push_str("[OK] FTS index matches knowledge count\n");
        }

        if vec_count > 0 {
            report.push_str(&format!("[OK] Vector search: {} embeddings indexed\n", vec_count));
        } else {
            report.push_str("[INFO] Vector search: no embeddings yet (run mine to generate)\n");
        }

        report.push_str("════════════════════════");
        Ok(report)
    }

    // --- Doctor ---

    pub fn run_doctor(&self, shelf: &str) -> Result<String> {
        let mut report = String::new();
        report.push_str("═══ Hypatia Doctor ═══\n");

        // Check 1: FTS index integrity
        let shelf_ref = self.shelf_manager.get(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;

        let knowledge_count = shelf_ref.duckdb.knowledge_count()?;
        let fts_count = shelf_ref.sqlite.doc_count()?;

        if knowledge_count != fts_count {
            report.push_str(&format!("[WARN] FTS count ({}) != Knowledge count ({})\n", fts_count, knowledge_count));
            report.push_str("  → Run: hypatia rebuild-fts <shelf>\n");
        } else {
            report.push_str("[OK] FTS index matches knowledge count\n");
        }

        // Check 2: Vector store
        let vec_count = if let Some(vs) = self.vector_stores.get(shelf) {
            vs.count().unwrap_or(0)
        } else {
            0
        };

        if vec_count > 0 {
            report.push_str(&format!("[OK] Vector store: {} embeddings\n", vec_count));
        } else {
            report.push_str("[INFO] Vector store: no embeddings (run 'mine' to generate)\n");
        }

        report.push_str("════════════════════════");
        Ok(report)
    }

    // --- Vector Search ---

    pub fn vector_search(&mut self, shelf: &str, query: &str, limit: i64) -> Result<Vec<VectorSearchResult>> {
        self.ensure_shelf(shelf)?;

        let vec_store = self.vector_stores.get(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("No vector store for shelf '{shelf}'"))
        })?;

        let query_embedding = embed::embed(query);
        let results = vec_store.search(&query_embedding, limit, 0)?;

        Ok(results.into_iter().map(|r| VectorSearchResult {
            source: r.source,
            lang: r.lang,
            text: r.text,
            symbol: r.symbol,
            score: r.score,
        }).collect())
    }

    // --- Init Library ---

    pub fn init_library(&mut self, path: &Path) -> Result<()> {
        self.shelf_manager.connect(path, Some("default"))?;
        Ok(())
    }

    /// Rebuild FTS index from existing docs_meta
    pub fn rebuild_fts(&self, shelf: &str) -> Result<(usize, usize)> {
        let shelf_ref = self.shelf_manager.get(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        shelf_ref.sqlite.rebuild_fts()
    }
}
