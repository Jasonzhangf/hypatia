use serde::{Deserialize, Serialize};
use serde_json::Map;

#[derive(Debug, Clone, Default)]
pub struct QueryOpts {
    pub catalog: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

pub type ResultSetRow = Map<String, serde_json::Value>;

/// Target table for queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryTarget {
    Knowledge,
    Statement,
}

impl QueryTarget {
    pub fn table_name(&self) -> &'static str {
        match self {
            QueryTarget::Knowledge => "knowledge",
            QueryTarget::Statement => "statement",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub rows: Vec<ResultSetRow>,
    pub total_count: Option<i64>,
}

impl QueryResult {
    pub fn new(rows: Vec<ResultSetRow>) -> Self {
        Self { rows, total_count: None }
    }

    pub fn with_total_count(mut self, count: i64) -> Self {
        self.total_count = Some(count);
        self
    }
}

/// Options for search operations.
#[derive(Debug, Clone)]
pub struct SearchOpts {
    pub catalog: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            catalog: None,
            limit: 100,
            offset: 0,
        }
    }
}
