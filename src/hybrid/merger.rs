use serde::{Deserialize, Serialize};

/// Unified result from hybrid search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridResult {
    pub source: String,
    pub lang: String,
    pub text: String,
    pub symbol: Option<String>,
    pub fts_score: Option<f64>,      // BM25 rank (negative, lower is better)
    pub vector_score: Option<f64>,    // Cosine similarity (0-1)
    pub combined_score: f64,          // Combined/normalized score
    pub matched_by: MatchType,        // How it was matched
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MatchType {
    FtsOnly,
    VectorOnly,
    Both,
}

/// Merge FTS results and vector results with RRF (Reciprocal Rank Fusion)
pub fn merge_hybrid(
    fts_results: Vec<(String, String, String, Option<String>, f64)>,  // (source, lang, text, symbol, bm25_rank)
    vector_results: Vec<(String, String, String, Option<String>, f64)>,  // (source, lang, text, symbol, cosine_sim)
    k: usize,
) -> Vec<HybridResult> {
    let mut results: std::collections::HashMap<String, HybridResult> = std::collections::HashMap::new();

    // Normalize BM25 ranks (lower is better, invert to higher-is-better)
    let max_bm25 = fts_results.iter().map(|r| r.4.abs()).fold(0.0f64, f64::max);

    for (i, (source, lang, text, symbol, rank)) in fts_results.iter().enumerate() {
        let key = format!("{}:{}", source, text.chars().take(50).collect::<String>());
        let normalized_score = if max_bm25 > 0.0 { rank.abs() / max_bm25 } else { 0.0 };
        let rrf = 1.0 / (k as f64 + (i + 1) as f64);

        results.entry(key.clone()).or_insert_with(|| HybridResult {
            source: source.clone(),
            lang: lang.clone(),
            text: text.clone(),
            symbol: symbol.clone(),
            fts_score: Some(*rank),
            vector_score: None,
            combined_score: rrf + normalized_score * 0.5,
            matched_by: MatchType::FtsOnly,
        });
    }

    for (i, (source, lang, text, symbol, sim)) in vector_results.iter().enumerate() {
        let key = format!("{}:{}", source, text.chars().take(50).collect::<String>());
        let rrf = 1.0 / (k as f64 + (i + 1) as f64);

        if let Some(existing) = results.get_mut(&key) {
            existing.vector_score = Some(*sim);
            existing.combined_score += rrf + sim * 0.5;
            existing.matched_by = MatchType::Both;
        } else {
            results.insert(key.clone(), HybridResult {
                source: source.clone(),
                lang: lang.clone(),
                text: text.clone(),
                symbol: symbol.clone(),
                fts_score: None,
                vector_score: Some(*sim),
                combined_score: rrf + sim * 0.5,
                matched_by: MatchType::VectorOnly,
            });
        }
    }

    let mut sorted: Vec<HybridResult> = results.into_values().collect();
    sorted.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap_or(std::cmp::Ordering::Equal));
    sorted
}
