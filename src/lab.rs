use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::engine::Evaluator;
use crate::error::{HypatiaError, Result};
use crate::hybrid::{merge_hybrid, HybridResult};
use crate::miner::{Chunker, MinerConfig, Scanner, Watcher};
use crate::miner::scanner::detect_lang;
use crate::model::*;
use crate::service::{KnowledgeService, StatementService};
use crate::storage::{ShelfManager, Storage};

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
}

impl Lab {
    pub fn new() -> Result<Self> {
        let mut shelf_manager = ShelfManager::new();
        shelf_manager.ensure_default()?;
        Ok(Self { shelf_manager })
    }

    // --- Shelf operations ---

    pub fn connect_shelf(&mut self, path: &Path, name: Option<&str>) -> Result<String> {
        self.shelf_manager.connect(path, name)
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

    // --- Hybrid Search (FTS + Vector placeholder) ---

    pub fn hybrid_search(&self, shelf: &str, query: &str, limit: i64) -> Result<Vec<HybridResult>> {
        // 1. FTS search
        let shelf_ref = self.shelf_manager.get(shelf).ok_or_else(|| {
            HypatiaError::Shelf(format!("shelf '{shelf}' is not connected"))
        })?;
        let fts_results = shelf_ref.execute_search(query, &SearchOpts {
            catalog: None,
            limit,
            offset: 0,
        })?;

        // Convert FTS results to hybrid format
        // Note: row is already a Map<String, Value>, not a Value that needs as_object()
        let fts_items: Vec<(String, String, String, Option<String>, f64)> = fts_results.rows.iter()
            .filter_map(|row| {
                let source = row.get("name").or_else(|| row.get("key")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                let text = row.get("data").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let lang = row.get("tags").and_then(|v| v.as_array()).map(|a| a.iter().take(1).map(|v| v.as_str().unwrap_or("")).collect::<Vec<_>>().join(",")).unwrap_or_default();
                Some((source, lang, text, None, 0.0))
            })
            .collect();

        // 2. Vector search (placeholder - returns empty for now)
        let vec_items: Vec<(String, String, String, Option<String>, f64)> = Vec::new();

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

        for file in files {
            let content = std::fs::read_to_string(&file.path)?;
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

        let mut output = String::new();
        output.push_str("═══ Hypatia Status ═══\n");
        output.push_str(&format!("Shelves: {}\n", shelf_count));
        output.push_str(&format!("Knowledge entries: {}\n", knowledge_count));
        output.push_str(&format!("Statements: {}\n", statement_count));
        output.push_str(&format!("FTS indexed docs: {}\n", fts_count));
        output.push_str("═══════════════════════");
        Ok(output)
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

        report.push_str("[INFO] Vector search: requires embedding model setup\n");
        report.push_str("════════════════════════");
        Ok(report)
    }

    // --- Vector Search (placeholder) ---

    pub fn vector_search(&self, _shelf: &str, _query: &str, _limit: i64) -> Result<Vec<VectorSearchResult>> {
        // Placeholder - requires embedding model setup
        // TODO: integrate with VectorStore once embedding is configured
        Ok(Vec::new())
    }

    // --- Init Library ---

    pub fn init_library(&mut self, path: &Path) -> Result<()> {
        // Ensure default shelf is connected at the given path
        self.shelf_manager.connect(path, Some("default"))?;
        Ok(())
    }
}
